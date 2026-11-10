//! Tests for the hardware table (`Catalog`): matching the CPU and GPU text
//! from requirements and from /proc to entries in data/hardware.toml.

use rigcheck_core::{Catalog, CatalogEntry};

fn catalog() -> Catalog {
    Catalog::from_toml(include_str!("../../../data/hardware.toml")).expect("valid hardware.toml")
}

fn names(entries: Vec<&CatalogEntry>) -> Vec<&str> {
    entries
        .into_iter()
        .map(|entry| entry.name.as_str())
        .collect()
}

#[test]
fn matches_every_known_gpu_alternative() {
    let catalog = catalog();

    // Baldur's Gate 3, minimum
    let gpus = catalog
        .match_gpus("Nvidia GTX 970 / RX 480 / Intel Arc A380 / Qualcomm Adreno X1 (4GB+ of VRAM)");

    assert_eq!(
        names(gpus),
        ["GeForce GTX 970", "Radeon RX 480", "Arc A380"]
    );
}

#[test]
fn matches_gpus_with_and_without_their_memory_size() {
    let catalog = catalog();

    // Cyberpunk 2077, minimum
    let gpus = catalog.match_gpus("GeForce GTX 1060 6GB or Radeon RX 580 8GB or Arc A380");

    assert_eq!(
        names(gpus),
        ["GeForce GTX 1060 6GB", "Radeon RX 580", "Arc A380"]
    );
}

#[test]
fn matches_cpus_in_requirement_text() {
    let catalog = catalog();
    let cases = [
        // Baldur's Gate 3
        (
            "Intel I5 4690 / AMD FX 8350 / Snapdragon X Elite",
            vec!["Core i5-4690", "FX-8350"],
        ),
        // Days Gone
        (
            "Intel Core i5-2500K@3.3GHz or AMD FX 6300@3.5GHz",
            vec!["Core i5-2500K", "FX-6300"],
        ),
        // Baldur's Gate 3, recommended
        (
            "Intel i7 8700K / AMD r5 3600",
            vec!["Core i7-8700K", "Ryzen 5 3600"],
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(names(catalog.match_cpus(text)), expected, "{text}");
    }
}

#[test]
fn finds_cpus_by_their_proc_cpuinfo_name() {
    let catalog = catalog();
    let cases = [
        ("AMD Ryzen 5 7530U with Radeon Graphics", "Ryzen 5 7530U"),
        ("Intel(R) Core(TM) i5-8400 CPU @ 2.80GHz", "Core i5-8400"),
        ("AMD Custom APU 0405", "Custom APU 0405 (Steam Deck)"),
    ];
    for (model, expected) in cases {
        let entry = catalog.match_cpus(model).into_iter().next();
        assert_eq!(entry.map(|e| e.name.as_str()), Some(expected), "{model}");
    }
}

#[test]
fn finds_detected_gpus_by_pci_id() {
    let catalog = catalog();
    let cases = [
        ((0x1002, 0x15e7), Some("Radeon Vega 7 (Ryzen 5000/7000U)")),
        ((0x10de, 0x1c03), Some("GeForce GTX 1060 6GB")),
        ((0x1002, 0x163f), Some("Steam Deck GPU (Van Gogh)")),
        ((0xffff, 0x0001), None),
    ];
    for ((vendor, device), expected) in cases {
        let entry = catalog.gpu_by_pci(vendor, device);
        assert_eq!(
            entry.map(|e| e.name.as_str()),
            expected,
            "{vendor:04x}:{device:04x}"
        );
    }
}

#[test]
fn memory_sizes_written_with_a_space_still_match() {
    let catalog = catalog();

    // ELDEN RING, minimum
    let gpus = catalog.match_gpus("NVIDIA GEFORCE GTX 1060 3 GB or AMD RADEON RX 580 4 GB");

    assert_eq!(names(gpus), ["GeForce GTX 1060 3GB", "Radeon RX 580"]);
}

#[test]
fn a_bare_model_number_reuses_the_previous_names_series() {
    let catalog = catalog();

    // Call of Duty, minimum: "GTX 970 / 1060" means GTX 970 or GTX 1060.
    let gpus = catalog
        .match_gpus("AMD Radeon™ RX 470, NVIDIA® GeForce® GTX 970 / 1060 or Intel® Arc™ A580");

    assert_eq!(
        names(gpus),
        [
            "Radeon RX 470",
            "GeForce GTX 970",
            "GeForce GTX 1060 6GB",
            "Arc A580"
        ]
    );
}

#[test]
fn matches_gpus_as_protondb_reports_name_them() {
    let catalog = catalog();
    let first = |text| names(catalog.match_gpus(text)).first().copied();

    assert_eq!(
        first("AMD Custom GPU 0405 (vangogh, LLVM 15.0.7, DRM 3.54, 6.5.0-valve22-1-neptune-65)"),
        Some("Steam Deck GPU (Van Gogh)")
    );
    // Mesa's driver name before the GPU's.
    assert_eq!(
        first("Intel Mesa Intel UHD 620 (KBL GT2)"),
        Some("UHD Graphics 620")
    );
}

/// A GPU entry named `name`, as Blender Open Data's gap scores add them.
fn gpu(name: &str, score: u32) -> CatalogEntry {
    CatalogEntry {
        name: name.to_owned(),
        vendor: rigcheck_core::Vendor::Amd,
        score,
        vram_mb: None,
        aliases: Vec::new(),
        pci: Vec::new(),
    }
}

#[test]
fn a_name_without_a_model_matches_nothing() {
    // Names Blender Open Data has: nothing left once brand words go.
    let catalog = Catalog::new(
        Vec::new(),
        vec![
            gpu("", 234),
            gpu("AMD Radeon Graphics", 59),
            gpu("Intel(R) Graphics", 29),
        ],
    );

    // Dota Underlords' minimum: "w/128 MB or better" is nothing but a size.
    assert!(
        catalog
            .match_gpus("Integrated HD Graphics 520 w/128 MB or better")
            .is_empty()
    );
    // A Ryzen iGPU as Vulkan names it.
    assert!(
        catalog
            .match_gpus("AMD Radeon Graphics (RADV RENOIR)")
            .is_empty()
    );
}
