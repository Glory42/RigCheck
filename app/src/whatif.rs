//! "What if I upgrade?" on My Rig: pick a GPU, CPU or amount of RAM and see
//! which installed games would change verdict. Nothing is saved, and the
//! rest of the app keeps using your PC (with its overrides).

use iced::widget::{button, column, container, row, text};
use iced::{Element, Fill};

use crate::Message;
use crate::i18n::I18n;
use crate::overrides::{Choices, Picker};
use crate::style::{BOLD, Colors};

/// Widget IDs for the pickers.
pub const GPU: &str = "whatif-gpu";
pub const CPU: &str = "whatif-cpu";

/// The RAM amounts to pick from, in GB.
const RAM_GB: [u32; 4] = [8, 16, 32, 64];

/// The hypothetical parts; `None` keeps your own.
#[derive(Debug, Clone, Default)]
pub struct Picks {
    pub gpu: Option<String>,
    pub cpu: Option<String>,
    pub ram_gb: Option<u32>,
}

impl Picks {
    fn any(&self) -> bool {
        self.gpu.is_some() || self.cpu.is_some() || self.ram_gb.is_some()
    }
}

/// `changes`: one line per installed game whose verdict would change, e.g.
/// "Cyberpunk 2077: TOO WEAK → LOW".
pub fn view<'a>(
    choices: &'a Choices,
    picks: &'a Picks,
    in_use: (String, String),
    changes: Vec<String>,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let (gpu_in_use, cpu_in_use) = in_use;
    let gpu = Picker {
        label: "GPU",
        in_use: gpu_in_use,
        id: GPU,
        state: &choices.gpus,
        selection: picks.gpu.as_ref(),
        on_pick: Message::WhatIfGpu,
    };
    let cpu = Picker {
        label: "CPU",
        in_use: cpu_in_use,
        id: CPU,
        state: &choices.cpus,
        selection: picks.cpu.as_ref(),
        on_pick: Message::WhatIfCpu,
    };
    let ram = RAM_GB.map(|gb| {
        let on = picks.ram_gb == Some(gb);
        button(text(format!("{gb} GB")).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(on))
            .on_press(Message::WhatIfRam((!on).then_some(gb)))
            .into()
    });
    let result: Element<'a, Message> = if !picks.any() {
        text(i18n.t("whatif-hint")).color(colors.muted).into()
    } else if changes.is_empty() {
        text(i18n.t("whatif-no-change")).font(BOLD).into()
    } else {
        column(changes.into_iter().map(|line| text(line).font(BOLD).into()))
            .spacing(4)
            .into()
    };
    container(
        column![
            text(i18n.t("whatif-title")).font(BOLD).size(16),
            text(i18n.t("whatif-description")).color(colors.muted),
            gpu.view(i18n, colors),
            cpu.view(i18n, colors),
            row![text("RAM").font(BOLD).width(60), row(ram).spacing(8)]
                .spacing(12)
                .align_y(iced::Center),
            result,
        ]
        .spacing(10),
    )
    .padding(14)
    .width(Fill)
    .style(move |_| colors.boxed(colors.panel))
    .into()
}
