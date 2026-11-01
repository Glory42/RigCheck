//! Tests for the crate's public seam, `detect(root, commands)`.
//!
//! Each fixture in `data/fixtures/hardware/` is a fake filesystem root plus
//! captured command output in `commands/<program>.txt`. A missing file means
//! the program isn't installed.

use std::fs;
use std::path::{Path, PathBuf};

use rigcheck_hardware::{Commands, Hardware, Vram, VulkanDevice, detect};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/fixtures/hardware");

/// Replays a fixture's captured command output instead of running programs.
struct Captured(PathBuf);

impl Commands for Captured {
    fn run(&self, program: &str, _args: &[&str]) -> Option<String> {
        fs::read_to_string(self.0.join("commands").join(format!("{program}.txt"))).ok()
    }
}

fn detect_fixture(name: &str) -> Hardware {
    let root = Path::new(FIXTURES).join(name);
    detect(&root, &Captured(root.clone()))
}

#[test]
fn reads_cpu_model_and_ram() {
    let hardware = detect_fixture("amd-laptop");

    assert_eq!(
        hardware.cpu.as_deref(),
        Some("AMD Ryzen 5 7530U with Radeon Graphics")
    );
    assert_eq!(hardware.ram_bytes, Some(15_746_968 * 1024));
}

#[test]
fn identifies_gpus_by_pci_id_and_kernel_driver() {
    let hardware = detect_fixture("amd-laptop");

    let [gpu] = hardware.gpus.as_slice() else {
        panic!("expected one GPU, got {:?}", hardware.gpus);
    };
    assert_eq!((gpu.vendor_id, gpu.device_id), (0x1002, 0x15e7));
    assert_eq!(gpu.kernel_driver.as_deref(), Some("amdgpu"));
}

#[test]
fn reads_each_gpus_vulkan_details_from_vulkaninfo() {
    let hardware = detect_fixture("amd-laptop");

    assert_eq!(
        hardware.gpus[0].vulkan,
        Some(VulkanDevice {
            name: "AMD Radeon Graphics (RADV RENOIR)".to_owned(),
            api_version: "1.4.354".to_owned(),
            driver: "Mesa 26.2.6-arch1.1".to_owned(),
            integrated: true,
        })
    );
}

#[test]
fn integrated_gpus_share_system_ram() {
    let hardware = detect_fixture("amd-laptop");

    assert_eq!(hardware.gpus[0].vram, Vram::Shared);
}

#[test]
fn reads_distro_and_kernel() {
    let hardware = detect_fixture("amd-laptop");

    assert_eq!(hardware.distro.as_deref(), Some("Arch Linux"));
    assert_eq!(hardware.kernel.as_deref(), Some("7.2.11-arch1-1"));
}

/// A machine where none of the programs are installed.
struct NothingInstalled;

impl Commands for NothingInstalled {
    fn run(&self, _program: &str, _args: &[&str]) -> Option<String> {
        None
    }
}

#[test]
fn still_finds_gpus_without_vulkaninfo() {
    let root = Path::new(FIXTURES).join("amd-laptop");

    let hardware = detect(&root, &NothingInstalled);

    let [gpu] = hardware.gpus.as_slice() else {
        panic!("expected one GPU, got {:?}", hardware.gpus);
    };
    assert_eq!((gpu.vendor_id, gpu.device_id), (0x1002, 0x15e7));
    assert_eq!(gpu.vulkan, None);
    assert_eq!(gpu.vram, Vram::Unknown);
}

#[test]
fn reads_nvidia_vram_from_nvidia_smi() {
    let hardware = detect_fixture("nvidia-desktop");

    assert_eq!(hardware.gpus[0].vram, Vram::Dedicated(6144 * 1024 * 1024));
}

#[test]
fn reads_amd_vram_from_sysfs() {
    let hardware = detect_fixture("amd-desktop");

    assert_eq!(
        hardware.gpus[0].vram,
        Vram::Dedicated(8 * 1024 * 1024 * 1024)
    );
}

#[test]
fn hybrid_laptops_game_on_the_discrete_gpu() {
    let hardware = detect_fixture("hybrid-laptop");

    assert_eq!(hardware.gpus.len(), 2);
    let gpu = hardware.gaming_gpu().expect("a GPU");
    assert_eq!((gpu.vendor_id, gpu.device_id), (0x10de, 0x1f91));
}

#[test]
fn every_fixture_finds_its_gaming_gpu() {
    let cases = [
        ("amd-laptop", (0x1002, 0x15e7), Vram::Shared),
        ("amd-desktop", (0x1002, 0x73ff), Vram::Dedicated(8 << 30)),
        ("nvidia-desktop", (0x10de, 0x1c03), Vram::Dedicated(6 << 30)),
        ("intel-laptop", (0x8086, 0x5917), Vram::Shared),
        ("hybrid-laptop", (0x10de, 0x1f91), Vram::Dedicated(4 << 30)),
        ("steam-deck", (0x1002, 0x163f), Vram::Shared),
    ];
    for (fixture, pci_id, vram) in cases {
        let hardware = detect_fixture(fixture);
        let gpu = hardware.gaming_gpu().expect(fixture);
        assert_eq!((gpu.vendor_id, gpu.device_id), pci_id, "{fixture}");
        assert_eq!(gpu.vram, vram, "{fixture}");
    }
}
