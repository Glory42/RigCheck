//! Your PC as verdicts see it: the detected parts matched once against the
//! hardware table, so the names shown and the scores compared always come
//! from the same entries. Parts can be swapped by table name, for an
//! override ("my GPU is actually X") or What if ("what if I had X?").

use rigcheck_core::{Catalog, CatalogEntry, Specs};

use crate::{Hardware, Vram};

/// The specs the matcher compares, and the names of the CPU and GPU behind
/// them.
#[derive(Debug, Clone, PartialEq)]
pub struct Rig {
    specs: Specs,
    cpu: Option<String>,
    gpu: Option<String>,
}

/// Parts to use instead of the detected ones. CPU and GPU are hardware-table
/// names; a name the table doesn't have is ignored.
#[derive(Debug, Clone, Copy, Default)]
pub struct Swap<'a> {
    pub gpu: Option<&'a str>,
    pub cpu: Option<&'a str>,
    pub ram_gb: Option<u32>,
}

impl Rig {
    /// `hardware` read against `catalog`: the GPU by PCI ID, else by its
    /// Vulkan name; the CPU by its model name. A part the table doesn't
    /// know keeps its detected name and has no score.
    pub fn new(catalog: &Catalog, hardware: &Hardware) -> Rig {
        let gpu = hardware.gaming_gpu();
        let vulkan = gpu.and_then(|gpu| gpu.vulkan.as_ref());
        let gpu_entry = gpu.and_then(|gpu| {
            catalog
                .gpu_by_pci(gpu.vendor_id, gpu.device_id)
                .or_else(|| catalog.match_gpus(&vulkan?.name).into_iter().next())
        });
        let cpu_entry = hardware
            .cpu
            .as_deref()
            .and_then(|model| catalog.match_cpus(model).into_iter().next());
        let specs = Specs {
            ram_mb: hardware.ram_bytes.map(|bytes| (bytes >> 20) as u32),
            vram_mb: gpu.and_then(|gpu| match gpu.vram {
                Vram::Dedicated(bytes) => Some((bytes >> 20) as u32),
                Vram::Shared | Vram::Unknown => None,
            }),
            // Installed games are already on disk.
            free_storage_mb: None,
            vulkan: vulkan.and_then(|vulkan| {
                let mut parts = vulkan.api_version.split('.').map(str::parse::<u32>);
                Some((parts.next()?.ok()?, parts.next()?.ok()?))
            }),
            nouveau: gpu.is_some_and(|gpu| gpu.kernel_driver.as_deref() == Some("nouveau")),
            cpu: cpu_entry.map(CatalogEntry::to_ref),
            gpu: gpu_entry.map(CatalogEntry::to_ref),
        };
        Rig {
            specs,
            cpu: cpu_entry
                .map(|entry| entry.name.clone())
                .or_else(|| hardware.cpu.clone()),
            gpu: gpu_entry
                .map(|entry| entry.name.clone())
                .or_else(|| vulkan.map(|vulkan| vulkan.name.clone())),
        }
    }

    /// This rig with `swap`'s parts in place of its own. A GPU brings its
    /// own VRAM.
    pub fn swapped(&self, catalog: &Catalog, swap: Swap) -> Rig {
        let named = |entries: &'_ [CatalogEntry], name: Option<&str>| {
            let name = name?;
            entries.iter().find(|entry| entry.name == name).cloned()
        };
        let mut rig = self.clone();
        if let Some(gpu) = named(catalog.gpus(), swap.gpu) {
            rig.specs.gpu = Some(gpu.to_ref());
            rig.specs.vram_mb = gpu.vram_mb;
            rig.gpu = Some(gpu.name);
        }
        if let Some(cpu) = named(catalog.cpus(), swap.cpu) {
            rig.specs.cpu = Some(cpu.to_ref());
            rig.cpu = Some(cpu.name);
        }
        if let Some(gb) = swap.ram_gb {
            rig.specs.ram_mb = Some(gb * 1024);
        }
        rig
    }

    /// What the matcher compares.
    pub fn specs(&self) -> &Specs {
        &self.specs
    }

    /// The CPU's hardware-table name, else its detected model.
    pub fn cpu_name(&self) -> Option<&str> {
        self.cpu.as_deref()
    }

    /// The GPU's hardware-table name, else Vulkan's name for it.
    pub fn gpu_name(&self) -> Option<&str> {
        self.gpu.as_deref()
    }
}
