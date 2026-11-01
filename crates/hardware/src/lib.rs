//! Linux hardware detection.
//!
//! Everything is read relative to a `root` directory (`/` in the app) and
//! external programs run through [`Commands`], so tests can replay fixtures
//! captured from other machines.

use std::fs;
use std::path::Path;

/// Runs external programs such as `vulkaninfo`.
pub trait Commands {
    /// The program's stdout, or `None` if it isn't installed or fails.
    fn run(&self, program: &str, args: &[&str]) -> Option<String>;
}

/// What rigcheck knows about the user's machine. `None` means not detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hardware {
    /// CPU model as the kernel reports it, e.g. "AMD Ryzen 5 7530U with Radeon Graphics".
    pub cpu: Option<String>,
    /// Total RAM usable by the OS. Slightly less than what's installed.
    pub ram_bytes: Option<u64>,
    pub gpus: Vec<Gpu>,
    /// e.g. "Arch Linux". Shown for information only.
    pub distro: Option<String>,
    /// e.g. "7.2.11-arch1-1". Shown for information only.
    pub kernel: Option<String>,
}

impl Hardware {
    /// The GPU games will run on: a discrete GPU if there is one, since
    /// hybrid laptops offload games to it (PRIME).
    pub fn gaming_gpu(&self) -> Option<&Gpu> {
        self.gpus
            .iter()
            .max_by_key(|gpu| gpu.vulkan.as_ref().is_some_and(|v| !v.integrated))
    }
}

/// A graphics card or integrated GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    /// PCI IDs identify the exact model, unlike marketing names.
    pub vendor_id: u16,
    pub device_id: u16,
    /// Kernel driver, e.g. "amdgpu", "i915", "nvidia" or "nouveau".
    pub kernel_driver: Option<String>,
    pub vram: Vram,
    /// `None` when `vulkaninfo` is missing or doesn't list this GPU.
    pub vulkan: Option<VulkanDevice>,
}

/// How much video memory a GPU has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vram {
    /// An integrated GPU borrowing system RAM.
    Shared,
    Dedicated(u64),
    Unknown,
}

/// Vulkan's view of a GPU, from `vulkaninfo --summary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VulkanDevice {
    pub name: String,
    pub api_version: String,
    /// User-space driver, e.g. "Mesa 26.2.6" or an NVIDIA version number.
    pub driver: String,
    /// Integrated GPUs share system RAM instead of having their own VRAM.
    pub integrated: bool,
}

/// Detects the hardware of the system mounted at `root`.
pub fn detect(root: &Path, commands: &impl Commands) -> Hardware {
    let probes = Probes::run(commands);
    Hardware {
        cpu: cpu_model(root),
        ram_bytes: ram_bytes(root),
        gpus: gpus(root, &probes),
        distro: distro(root),
        kernel: read_trimmed(&root.join("proc/sys/kernel/osrelease")),
    }
}

/// `PRETTY_NAME` from os-release, e.g. `PRETTY_NAME="Arch Linux"`.
fn distro(root: &Path) -> Option<String> {
    let os_release = fs::read_to_string(root.join("etc/os-release")).ok()?;
    let value = os_release
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))?;
    Some(value.trim_matches('"').to_owned())
}

fn read_trimmed(path: &Path) -> Option<String> {
    Some(fs::read_to_string(path).ok()?.trim().to_owned())
}

fn cpu_model(root: &Path) -> Option<String> {
    let cpuinfo = fs::read_to_string(root.join("proc/cpuinfo")).ok()?;
    field(&cpuinfo, "model name").map(str::to_owned)
}

fn ram_bytes(root: &Path) -> Option<u64> {
    let meminfo = fs::read_to_string(root.join("proc/meminfo")).ok()?;
    let kib: u64 = field(&meminfo, "MemTotal")?
        .strip_suffix(" kB")?
        .parse()
        .ok()?;
    Some(kib * 1024)
}

/// GPUs from `/sys/class/drm`. Connector entries like `card1-eDP-1` have no
/// PCI ID, so [`gpu`] skips them.
fn gpus(root: &Path, probes: &Probes) -> Vec<Gpu> {
    let Ok(entries) = fs::read_dir(root.join("sys/class/drm")) else {
        return Vec::new();
    };
    let mut cards: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
    cards.sort();
    cards.iter().filter_map(|card| gpu(card, probes)).collect()
}

/// Reads a card's `device/uevent`, which holds lines like `PCI_ID=1002:15E7`.
fn gpu(card: &Path, probes: &Probes) -> Option<Gpu> {
    let uevent = fs::read_to_string(card.join("device/uevent")).ok()?;
    let value = |key: &str| {
        uevent
            .lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
    };
    let (vendor, device) = value("PCI_ID")?.split_once(':')?;
    let vendor_id = u16::from_str_radix(vendor, 16).ok()?;
    let device_id = u16::from_str_radix(device, 16).ok()?;
    let pci_id = (vendor_id, device_id);
    let vulkan = probes
        .vulkan
        .iter()
        .find(|(id, _)| *id == pci_id)
        .map(|(_, device)| device.clone());
    // amdgpu reports VRAM in sysfs; NVIDIA's driver only through nvidia-smi.
    let dedicated = read_trimmed(&card.join("device/mem_info_vram_total"))
        .and_then(|bytes| bytes.parse().ok())
        .or_else(|| {
            let (_, bytes) = probes.nvidia_vram.iter().find(|(id, _)| *id == pci_id)?;
            Some(*bytes)
        });
    // An iGPU's RAM carve-out also shows up as VRAM, so only Vulkan's
    // integrated/discrete flag can say which kind a number is.
    let vram = match (&vulkan, dedicated) {
        (Some(device), _) if device.integrated => Vram::Shared,
        (Some(_), Some(bytes)) => Vram::Dedicated(bytes),
        _ => Vram::Unknown,
    };
    Some(Gpu {
        vendor_id,
        device_id,
        kernel_driver: value("DRIVER").map(str::to_owned),
        vram,
        vulkan,
    })
}

/// (vendor, device) PCI IDs.
type PciId = (u16, u16);

/// What external programs report, keyed by PCI ID. Empty if not installed.
struct Probes {
    vulkan: Vec<(PciId, VulkanDevice)>,
    nvidia_vram: Vec<(PciId, u64)>,
}

impl Probes {
    fn run(commands: &impl Commands) -> Self {
        let vulkaninfo = commands.run("vulkaninfo", &["--summary"]);
        let nvidia_smi = commands.run(
            "nvidia-smi",
            &[
                "--query-gpu=pci.device_id,memory.total",
                "--format=csv,noheader,nounits",
            ],
        );
        Probes {
            vulkan: vulkan_devices(&vulkaninfo.unwrap_or_default()),
            nvidia_vram: nvidia_vram(&nvidia_smi.unwrap_or_default()),
        }
    }
}

/// Parses `nvidia-smi` CSV lines like `0x1C0310DE, 6144`: device and vendor
/// IDs packed into one number, then memory in MiB.
fn nvidia_vram(csv: &str) -> Vec<(PciId, u64)> {
    csv.lines()
        .filter_map(|line| {
            let (id, mib) = line.split_once(',')?;
            let id = u32::from_str_radix(id.trim().strip_prefix("0x")?, 16).ok()?;
            let mib: u64 = mib.trim().parse().ok()?;
            Some((((id & 0xffff) as u16, (id >> 16) as u16), mib * 1024 * 1024))
        })
        .collect()
}

/// Parses the `GPU<n>:` blocks of `vulkaninfo --summary`.
/// Software renderers like llvmpipe have no PCI ID and drop out here.
fn vulkan_devices(summary: &str) -> Vec<(PciId, VulkanDevice)> {
    summary
        .split("\nGPU")
        .skip(1)
        .filter_map(|block| {
            let value = |key: &str| {
                block.lines().find_map(|line| {
                    let (k, v) = line.split_once('=')?;
                    (k.trim() == key).then(|| v.trim())
                })
            };
            let hex = |key: &str| u16::from_str_radix(value(key)?.strip_prefix("0x")?, 16).ok();
            Some((
                (hex("vendorID")?, hex("deviceID")?),
                VulkanDevice {
                    name: value("deviceName")?.to_owned(),
                    api_version: value("apiVersion")?.to_owned(),
                    driver: value("driverInfo")?.to_owned(),
                    integrated: value("deviceType")? == "PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU",
                },
            ))
        })
        .collect()
}

/// The value of the first `key: value` line in a /proc file.
fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        (k.trim() == key).then(|| v.trim())
    })
}
