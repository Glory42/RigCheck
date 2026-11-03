//! The override picker on My Rig: "my GPU is actually X" or "what if I had X?".

use iced::widget::{Row, button, column, combo_box, container, row, text};
use iced::{Center, Element, Fill};

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors};

/// Widget IDs for the pickers.
pub const GPU: &str = "override-gpu";
pub const CPU: &str = "override-cpu";

/// The hardware-table names to pick from.
#[derive(Default)]
pub struct Choices {
    pub gpus: combo_box::State<String>,
    pub cpus: combo_box::State<String>,
}

impl Choices {
    /// Every GPU and CPU in the hardware table, by name.
    pub fn of(catalog: &rigcheck_core::Catalog) -> Choices {
        let names = |entries: &[rigcheck_core::CatalogEntry]| {
            entries.iter().map(|entry| entry.name.clone()).collect()
        };
        Choices {
            gpus: combo_box::State::new(names(catalog.gpus())),
            cpus: combo_box::State::new(names(catalog.cpus())),
        }
    }
}

pub struct Picked<'a> {
    pub gpu: Option<&'a String>,
    pub cpu: Option<&'a String>,
    /// What verdicts use now: the pick, or the detected hardware's name.
    pub gpu_in_use: String,
    pub cpu_in_use: String,
}

pub fn view<'a>(
    choices: &'a Choices,
    picked: Picked<'a>,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let gpu = Picker {
        label: "GPU",
        in_use: picked.gpu_in_use,
        id: GPU,
        state: &choices.gpus,
        selection: picked.gpu,
        on_pick: Message::OverrideGpu,
    };
    let cpu = Picker {
        label: "CPU",
        in_use: picked.cpu_in_use,
        id: CPU,
        state: &choices.cpus,
        selection: picked.cpu,
        on_pick: Message::OverrideCpu,
    };
    container(
        column![
            text(i18n.t("override-title")).font(BOLD).size(16),
            text(i18n.t("override-description")).color(colors.muted),
            gpu.view(i18n, colors),
            cpu.view(i18n, colors),
        ]
        .spacing(10),
    )
    .padding(14)
    .width(Fill)
    .style(move |_| colors.boxed(colors.panel))
    .into()
}

/// One row: what's in use, a searchable list of the table, and a reset
/// button. Also used by "What if I upgrade?".
pub struct Picker<'a> {
    pub label: &'static str,
    pub in_use: String,
    pub id: &'static str,
    pub state: &'a combo_box::State<String>,
    pub selection: Option<&'a String>,
    pub on_pick: fn(Option<String>) -> Message,
}

impl<'a> Picker<'a> {
    pub fn view(self, i18n: &I18n, colors: Colors) -> Row<'a, Message> {
        let on_pick = self.on_pick;
        let input = combo_box(
            self.state,
            &i18n.t("override-placeholder"),
            self.selection,
            move |name| on_pick(Some(name)),
        )
        .padding(8)
        .font(BOLD);
        let reset = button(text(i18n.t("override-reset")).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(false))
            .on_press_maybe(self.selection.map(|_| on_pick(None)));
        row![
            text(self.label).font(BOLD).width(60),
            text(self.in_use).font(BOLD).width(Fill),
            container(input).id(self.id).width(Fill),
            reset,
        ]
        .spacing(12)
        .align_y(Center)
    }
}
