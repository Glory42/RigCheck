//! Shared test data for the app's UI tests.

use std::path::PathBuf;

use rigcheck_hardware::{Gpu, Hardware, Vram, VulkanDevice};
use rigcheck_steam::InstalledGame;

/// The dev machine: Ryzen 5 7530U with its Radeon iGPU (1002:15e7).
pub fn amd_laptop() -> Hardware {
    Hardware {
        cpu: Some("AMD Ryzen 5 7530U with Radeon Graphics".to_owned()),
        ram_bytes: Some(15_746_968 * 1024),
        gpus: vec![Gpu {
            vendor_id: 0x1002,
            device_id: 0x15e7,
            kernel_driver: Some("amdgpu".to_owned()),
            vram: Vram::Shared,
            vulkan: Some(VulkanDevice {
                name: "AMD Radeon Graphics (RADV RENOIR)".to_owned(),
                api_version: "1.4.354".to_owned(),
                driver: "Mesa 26.2.6-arch1.1".to_owned(),
                integrated: true,
            }),
        }],
        distro: Some("Arch Linux".to_owned()),
        kernel: Some("7.2.11-arch1-1".to_owned()),
    }
}

pub fn installed(app_id: u32, name: &str) -> InstalledGame {
    InstalledGame {
        app_id,
        name: name.to_owned(),
        size_on_disk: 1 << 30,
        install_dir: PathBuf::from(format!("/games/{app_id}")),
    }
}

/// A real cover `size` bytes long: a 2×3 JPEG, padded after its end so
/// covers can take up space without being big images.
pub fn jpeg(size: usize) -> Vec<u8> {
    let mut jpeg = include_bytes!("../../data/fixtures/cover.jpg").to_vec();
    jpeg.resize(size.max(jpeg.len()), 0);
    jpeg
}
