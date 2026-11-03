//! The Rig: detected parts read against the hardware table, and swapped for
//! an override or What if.

use rigcheck_core::Catalog;
use rigcheck_hardware::{Gpu, Hardware, Rig, Swap, Vram, VulkanDevice};

fn table() -> Catalog {
    Catalog::from_toml(include_str!("../../../data/hardware.toml")).unwrap()
}

/// A PC with one GPU, known to Vulkan as `vulkan_name`.
fn pc(vendor_id: u16, device_id: u16, vulkan_name: &str, vram: Vram) -> Hardware {
    Hardware {
        cpu: Some("AMD Ryzen 5 7530U with Radeon Graphics".to_owned()),
        ram_bytes: Some(16 << 30),
        gpus: vec![Gpu {
            vendor_id,
            device_id,
            kernel_driver: Some("amdgpu".to_owned()),
            vram,
            vulkan: Some(VulkanDevice {
                name: vulkan_name.to_owned(),
                api_version: "1.3.280".to_owned(),
                driver: "Mesa".to_owned(),
                integrated: false,
            }),
        }],
        distro: None,
        kernel: None,
    }
}

fn score(rig: &Rig) -> Option<u32> {
    rig.specs().gpu.map(|gpu| gpu.score)
}

#[test]
fn a_gpu_is_found_by_its_pci_id() {
    let laptop = pc(
        0x1002,
        0x15e7,
        "AMD Radeon Graphics (RADV RENOIR)",
        Vram::Shared,
    );

    let rig = Rig::new(&table(), &laptop);

    assert_eq!(rig.gpu_name(), Some("Radeon Vega 7 (Ryzen 5000/7000U)"));
    assert_eq!(score(&rig), Some(32));
    assert_eq!(rig.specs().ram_mb, Some(16 * 1024));
    assert_eq!(rig.specs().vulkan, Some((1, 3)));
}

#[test]
fn the_gpu_shown_is_the_gpu_scored_when_only_its_name_matches() {
    // A PCI ID the table doesn't list: matched by Vulkan's name instead.
    let desktop = pc(
        0x10de,
        0xffff,
        "NVIDIA GeForce GTX 1060 6GB",
        Vram::Dedicated(6 << 30),
    );

    let rig = Rig::new(&table(), &desktop);

    assert_eq!(rig.gpu_name(), Some("GeForce GTX 1060 6GB"));
    assert_eq!(score(&rig), Some(100));
}

#[test]
fn a_part_the_table_doesnt_know_keeps_its_detected_name_and_no_score() {
    let odd = pc(0x5143, 0x0001, "Qualcomm Adreno X1-85", Vram::Unknown);

    let rig = Rig::new(&table(), &odd);

    assert_eq!(rig.gpu_name(), Some("Qualcomm Adreno X1-85"));
    assert_eq!(score(&rig), None);
}

#[test]
fn a_swapped_gpu_brings_its_own_vram_and_name() {
    let laptop = pc(
        0x1002,
        0x15e7,
        "AMD Radeon Graphics (RADV RENOIR)",
        Vram::Shared,
    );
    let table = table();
    let rig = Rig::new(&table, &laptop);

    let upgraded = rig.swapped(
        &table,
        Swap {
            gpu: Some("GeForce GTX 1060 6GB"),
            ram_gb: Some(32),
            ..Swap::default()
        },
    );

    assert_eq!(upgraded.gpu_name(), Some("GeForce GTX 1060 6GB"));
    assert_eq!(score(&upgraded), Some(100));
    assert_eq!(upgraded.specs().vram_mb, Some(6144));
    assert_eq!(upgraded.specs().ram_mb, Some(32 * 1024));
    assert_eq!(upgraded.cpu_name(), rig.cpu_name(), "the CPU stays");
}

#[test]
fn a_swap_to_a_name_the_table_doesnt_have_changes_nothing() {
    let laptop = pc(
        0x1002,
        0x15e7,
        "AMD Radeon Graphics (RADV RENOIR)",
        Vram::Shared,
    );
    let table = table();
    let rig = Rig::new(&table, &laptop);

    let swapped = rig.swapped(
        &table,
        Swap {
            gpu: Some("GeForce RTX 9090"),
            cpu: Some("Pentium 4"),
            ram_gb: None,
        },
    );

    assert_eq!(swapped, rig);
}
