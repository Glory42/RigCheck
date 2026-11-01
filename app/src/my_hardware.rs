//! The My Rig page: what rigcheck detected about this machine, drawn as
//! design D's "character sheet".

use iced::widget::{Column, button, column, container, row, space, text};
use iced::{Center, Element, Fill};
use rigcheck_hardware::{Hardware, Vram, gib};

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors};

pub fn view<'a>(hardware: &'a Hardware, i18n: &I18n, colors: Colors) -> Element<'a, Message> {
    let unknown = || i18n.t("unknown");
    let mut rows = vec![
        (
            "CPU".to_owned(),
            hardware.cpu.clone().unwrap_or_else(unknown),
        ),
        (
            "RAM".to_owned(),
            hardware.ram_bytes.map(gib).unwrap_or_else(unknown),
        ),
    ];
    let mut notes = Vec::new();
    for gpu in &hardware.gpus {
        let name = gpu.vulkan.as_ref().map_or_else(
            || {
                let name = i18n.t("unknown-gpu");
                format!("{name} [{:04x}:{:04x}]", gpu.vendor_id, gpu.device_id)
            },
            |vulkan| vulkan.name.clone(),
        );
        let memory = match gpu.vram {
            Vram::Shared => i18n.t("vram-shared"),
            Vram::Dedicated(bytes) => i18n.t_with("vram-dedicated", &[("size", &gib(bytes))]),
            Vram::Unknown => i18n.t("vram-unknown"),
        };
        rows.push(("GPU".to_owned(), name));
        rows.push((i18n.t("label-video-memory"), memory));
        if let Some(vulkan) = &gpu.vulkan {
            rows.push((
                i18n.t("label-vulkan"),
                format!("Vulkan {}, {}", vulkan.api_version, vulkan.driver),
            ));
        }
        if gpu.kernel_driver.as_deref() == Some("nouveau") {
            notes.push(i18n.t("note-nouveau"));
        }
    }
    if hardware.gpus.iter().any(|gpu| gpu.vulkan.is_none()) {
        notes.push(i18n.t("note-no-vulkan"));
    }
    rows.push((
        i18n.t("label-distro"),
        hardware.distro.clone().unwrap_or_else(unknown),
    ));
    rows.push((
        i18n.t("label-kernel"),
        hardware.kernel.clone().unwrap_or_else(unknown),
    ));

    let header = row![
        text(i18n.t("my-rig-title")).font(BOLD).size(20),
        space::horizontal(),
        button(text(i18n.t("copy-as-text")).font(BOLD).size(13))
            .padding([7, 12])
            .style(colors.button(true))
            .on_press(Message::CopyHardwareReport),
    ]
    .align_y(Center);
    let sheet = container(Column::with_children(
        rows.into_iter()
            .map(|(label, value)| sheet_row(label, value, colors)),
    ))
    .padding(3) // the outline's width, so cells don't paint over it
    .style(move |_| colors.boxed(colors.panel));

    let mut page = column![header, sheet].spacing(18);
    for note in notes {
        page = page.push(
            container(text(note))
                .padding(12)
                .width(Fill)
                .style(move |_| colors.boxed(colors.panel2)),
        );
    }
    page.into()
}

/// One line of the sheet: a bold label cell and its value.
fn sheet_row(label: String, value: String, colors: Colors) -> Element<'static, Message> {
    row![
        container(text(label).font(BOLD))
            .width(170)
            .padding([10, 14])
            .style(move |_| container::Style::default().background(colors.panel2)),
        container(text(value)).padding([10, 14]).width(Fill),
    ]
    .into()
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
        let mut ui = iced_test::simulator(super::view(
            &hardware,
            &crate::i18n::I18n::new(crate::i18n::Language::English),
            crate::settings::Appearance::Paper.colors(),
        ));

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
        let mut ui = iced_test::simulator(super::view(
            &hardware,
            &crate::i18n::I18n::new(crate::i18n::Language::English),
            crate::settings::Appearance::Paper.colors(),
        ));

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
        let mut ui = iced_test::simulator(super::view(
            &hardware,
            &crate::i18n::I18n::new(crate::i18n::Language::English),
            crate::settings::Appearance::Paper.colors(),
        ));

        assert!(ui.find(NOUVEAU_WARNING).is_ok());
    }

    #[test]
    fn no_nouveau_warning_with_other_drivers() {
        let hardware = laptop();
        let mut ui = iced_test::simulator(super::view(
            &hardware,
            &crate::i18n::I18n::new(crate::i18n::Language::English),
            crate::settings::Appearance::Paper.colors(),
        ));

        assert!(ui.find(NOUVEAU_WARNING).is_err());
    }

    const NOUVEAU_WARNING: &str = "This GPU uses the open-source nouveau driver, which runs most 3D games poorly. Install NVIDIA's driver to play.";
}
