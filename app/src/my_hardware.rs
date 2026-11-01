//! The My Hardware page: what rigcheck detected about this machine.

use iced::Element;
use iced::widget::{Column, button, column, row, scrollable, text};
use rigcheck_hardware::{Hardware, Vram, gib};

use crate::Message;

pub fn view(hardware: &Hardware) -> Element<'_, Message> {
    let unknown = || "Unknown".to_owned();
    let mut rows = Column::new().spacing(8);
    rows = rows.push(field("CPU", hardware.cpu.clone().unwrap_or_else(unknown)));
    rows = rows.push(field(
        "RAM",
        hardware.ram_bytes.map(gib).unwrap_or_else(unknown),
    ));
    for gpu in &hardware.gpus {
        let name = gpu.vulkan.as_ref().map_or_else(
            || format!("Unknown GPU [{:04x}:{:04x}]", gpu.vendor_id, gpu.device_id),
            |vulkan| vulkan.name.clone(),
        );
        let memory = match gpu.vram {
            Vram::Shared => "Shares system RAM".to_owned(),
            Vram::Dedicated(bytes) => format!("{} VRAM", gib(bytes)),
            Vram::Unknown => "VRAM unknown".to_owned(),
        };
        rows = rows.push(field("GPU", name));
        rows = rows.push(field("", memory));
        if let Some(vulkan) = &gpu.vulkan {
            rows = rows.push(field(
                "",
                format!("Vulkan {}, {}", vulkan.api_version, vulkan.driver),
            ));
        }
        if gpu.kernel_driver.as_deref() == Some("nouveau") {
            rows = rows.push(text(
                "This GPU uses the open-source nouveau driver, which runs most 3D games poorly. Install NVIDIA's driver to play.",
            ));
        }
    }
    if hardware.gpus.iter().any(|gpu| gpu.vulkan.is_none()) {
        rows = rows.push(text(
            "Vulkan wasn't detected. Install vulkan-tools (it provides vulkaninfo) and restart rigcheck.",
        ));
    }
    rows = rows.push(field(
        "Distro",
        hardware.distro.clone().unwrap_or_else(unknown),
    ));
    rows = rows.push(field(
        "Kernel",
        hardware.kernel.clone().unwrap_or_else(unknown),
    ));

    let header = row![
        text("My Hardware").size(24),
        button("Copy as text").on_press(Message::CopyHardwareReport),
    ]
    .spacing(16);
    scrollable(column![header, rows].spacing(16).padding(24)).into()
}

/// A label and its value on one line.
fn field(label: &'static str, value: String) -> Element<'static, Message> {
    row![text(label).width(80), text(value)].spacing(12).into()
}

#[cfg(test)]
mod tests {
    use rigcheck_hardware::{Gpu, Hardware, Vram, VulkanDevice};

    fn laptop() -> Hardware {
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

    #[test]
    fn shows_the_detected_hardware() {
        let hardware = laptop();
        let mut ui = iced_test::simulator(super::view(&hardware));

        for value in [
            "AMD Ryzen 5 7530U with Radeon Graphics",
            "15.0 GiB",
            "AMD Radeon Graphics (RADV RENOIR)",
            "Arch Linux",
            "7.2.11-arch1-1",
        ] {
            assert!(ui.find(value).is_ok(), "{value} not shown");
        }
    }

    #[test]
    fn hints_at_vulkan_tools_when_vulkan_is_missing() {
        let mut hardware = laptop();
        hardware.gpus[0].vulkan = None;
        hardware.gpus[0].vram = Vram::Unknown;
        let mut ui = iced_test::simulator(super::view(&hardware));

        for value in [
            "Unknown GPU [1002:15e7]",
            "VRAM unknown",
            "Vulkan wasn't detected. Install vulkan-tools (it provides vulkaninfo) and restart rigcheck.",
        ] {
            assert!(ui.find(value).is_ok(), "{value} not shown");
        }
    }

    #[test]
    fn warns_about_the_nouveau_driver() {
        let mut hardware = laptop();
        hardware.gpus[0].kernel_driver = Some("nouveau".to_owned());
        let mut ui = iced_test::simulator(super::view(&hardware));

        assert!(ui.find(NOUVEAU_WARNING).is_ok());
    }

    #[test]
    fn no_nouveau_warning_with_other_drivers() {
        let hardware = laptop();
        let mut ui = iced_test::simulator(super::view(&hardware));

        assert!(ui.find(NOUVEAU_WARNING).is_err());
    }

    const NOUVEAU_WARNING: &str = "This GPU uses the open-source nouveau driver, which runs most 3D games poorly. Install NVIDIA's driver to play.";
}
