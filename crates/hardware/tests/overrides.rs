//! Tests for `Hardware::with_overrides`, which lets users correct detection
//! or ask "what if I had GPU X?".

mod common;

use common::detect_fixture;
use rigcheck_hardware::{GpuOverride, Overrides, Vram};

#[test]
fn a_gpu_override_becomes_the_gaming_gpu() {
    let overrides = Overrides {
        gpu: Some(GpuOverride {
            vendor_id: 0x10de,
            device_id: 0x2504, // RTX 3060
            vram_bytes: 12 << 30,
        }),
        ..Overrides::default()
    };

    let hardware = detect_fixture("amd-laptop").with_overrides(&overrides);

    let gpu = hardware.gaming_gpu().expect("a GPU");
    assert_eq!((gpu.vendor_id, gpu.device_id), (0x10de, 0x2504));
    assert_eq!(gpu.vram, Vram::Dedicated(12 << 30));
}

#[test]
fn cpu_and_ram_overrides_replace_detected_values() {
    let overrides = Overrides {
        cpu: Some("AMD Ryzen 7 5800X3D 8-Core Processor".to_owned()),
        ram_bytes: Some(32 << 30),
        ..Overrides::default()
    };

    let hardware = detect_fixture("amd-laptop").with_overrides(&overrides);

    assert_eq!(
        hardware.cpu.as_deref(),
        Some("AMD Ryzen 7 5800X3D 8-Core Processor")
    );
    assert_eq!(hardware.ram_bytes, Some(32 << 30));
    // Untouched values keep what was detected.
    assert_eq!(hardware.gaming_gpu().map(|gpu| gpu.device_id), Some(0x15e7));
}
