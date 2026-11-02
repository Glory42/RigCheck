//! The game page: the verdict, why, and the requirements next to your PC.

use iced::widget::{Column, button, column, container, row, scrollable, text};
use iced::{Center, Element, Fill};
use rigcheck_core::{Blocker, Component, Level};
use rigcheck_gamedb::{Game, Tier};

use crate::i18n::I18n;
use crate::library::{Cover, Status, cover_box, stamp};
use crate::style::{BOLD, Colors, level_color};
use crate::{Message, Page};

/// The user's machine as the page shows it.
pub struct Rig {
    pub cpu: String,
    pub gpu: String,
    pub ram: String,
    pub vram: String,
    pub api: String,
}

/// `back` is the tab the page was opened from.
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    app_id: u32,
    name: String,
    // About a live lookup on Steam, under the header.
    note: Option<Element<'a, Message>>,
    record: Option<&'a Game>,
    status: &Status,
    rig: &Rig,
    cover: Cover,
    back: Page,
    // The game picked to compare with, if any: its app ID and name.
    compare: Option<(u32, String)>,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let mut compare_buttons = row![
        button(text(i18n.t("compare")).font(BOLD).size(13))
            .padding([7, 12])
            .style(colors.button(compare.as_ref().is_some_and(|(id, _)| *id == app_id)))
            .on_press(Message::PickToCompare(app_id))
    ]
    .spacing(12);
    if let Some((other, other_name)) = compare.filter(|(id, _)| *id != app_id) {
        compare_buttons = compare_buttons.push(
            button(
                text(i18n.t_with("compare-with", &[("name", &other_name)]))
                    .font(BOLD)
                    .size(13),
            )
            .padding([7, 12])
            .style(colors.button(false))
            .on_press(Message::Open(Page::Compare(other, app_id))),
        );
    }
    let (label, color) = stamp(status, i18n);
    let stamp = container(text(label).font(BOLD).size(18))
        .padding([4, 10])
        .style(move |_| container::Style {
            text_color: Some(colors.line),
            ..container::Style::default().background(color)
        });
    let back_label = match back {
        Page::Catalog => i18n.t("game-back-catalog"),
        _ => i18n.t("game-back"),
    };
    let back = button(text(back_label).font(BOLD).size(13))
        .padding([7, 12])
        .style(colors.button(false))
        .on_press(Message::Open(back));
    let mut header = row![].spacing(18).align_y(Center);
    header = header.push(cover_box(cover, 110.0, 165.0, colors));
    header = header.push(
        column![
            text(name).font(BOLD).size(24),
            row![
                stamp,
                button(text(i18n.t("game-store")).font(BOLD).size(13))
                    .padding([7, 12])
                    .style(colors.button(false))
                    .on_press(Message::OpenUrl(format!(
                        "https://store.steampowered.com/app/{app_id}/"
                    ))),
                compare_buttons,
            ]
            .spacing(12)
            .align_y(Center),
            text(reason(status, record, i18n)),
        ]
        .spacing(10),
    );

    let mut page = column![back, header].spacing(18);
    if let Some(note) = note {
        page = page.push(note);
    }
    if let Some(record) = record {
        page = page.push(requirements(record, status, rig, i18n, colors));
        page = page.push(linux(record, i18n, colors));
        if let Some(text) = steam_text(record, i18n, colors) {
            page = page.push(text);
        }
    }
    scrollable(page.padding([0, 6])).into()
}

/// One sentence on why the game got its stamp.
fn reason(status: &Status, record: Option<&Game>, i18n: &I18n) -> String {
    let verdict = match status {
        Status::Verdict(verdict) => verdict,
        Status::NoData => return i18n.t("why-no-data"),
        Status::NotChecked => return i18n.t("why-not-checked"),
        Status::Checking => return i18n.t("detecting-hardware"),
    };
    let part = |component| i18n.t(part_id(component));
    match (verdict.overall, verdict.bottleneck, verdict.blocker) {
        (Level::Blocked, _, Some(Blocker::Proton)) => i18n.t("why-blocked-proton"),
        (Level::Blocked, _, _) => {
            let name = record
                .and_then(|game| game.anticheat_name.clone())
                .unwrap_or_else(|| "Anti-cheat".to_owned());
            i18n.t_with("why-blocked-anticheat", &[("name", &name)])
        }
        (Level::Below, Some(component), _) => {
            i18n.t_with("why-below", &[("part", &part(component))])
        }
        (Level::Minimum, Some(component), _) => {
            i18n.t_with("why-minimum", &[("part", &part(component))])
        }
        (Level::Recommended, _, _) => i18n.t("why-recommended"),
        _ => i18n.t("why-unknown"),
    }
}

/// A size as Steam writes it: "256 MB", "8 GB", "1.5 GB".
fn size(mb: u32) -> String {
    match mb {
        mb if mb < 1024 => format!("{mb} MB"),
        mb if mb % 1024 == 0 => format!("{} GB", mb / 1024),
        mb => format!("{:.1} GB", f64::from(mb) / 1024.0),
    }
}

/// The requirement rows, in order.
pub const ROWS: [Component; 5] = [
    Component::Cpu,
    Component::Gpu,
    Component::Ram,
    Component::Vram,
    Component::GraphicsApi,
];

/// What `tier` asks of `component`, as Steam writes it; "—" if nothing.
pub fn requirement(tier: Option<&Tier>, component: Component) -> String {
    let text = tier.and_then(|tier| match component {
        Component::Cpu => tier.cpu_text.clone(),
        Component::Gpu => tier.gpu_text.clone(),
        Component::Ram => tier.ram_mb.map(size),
        Component::Vram => tier.vram_mb.map(size),
        Component::Storage => tier.storage_mb.map(size),
        Component::GraphicsApi => tier.directx.map(|dx| format!("DirectX {dx}")),
    });
    text.unwrap_or_else(|| "—".to_owned())
}

/// How this PC did on one row; "—" until there's a verdict.
pub fn result(status: &Status, component: Component, i18n: &I18n) -> String {
    let Status::Verdict(verdict) = status else {
        return "—".to_owned();
    };
    let level = verdict
        .components
        .iter()
        .find(|(compared, _)| *compared == component)
        .map(|(_, level)| *level);
    i18n.t(match level {
        Some(Level::Recommended) => "row-meets-recommended",
        Some(Level::Minimum) => "row-meets-minimum",
        Some(Level::Below) => "row-below-minimum",
        _ => "row-not-compared",
    })
}

/// ProtonDB, anti-cheat and Steam Deck: label and value.
pub fn linux_facts(game: &Game, i18n: &I18n) -> [(String, String); 3] {
    let proton = if game.linux.native {
        i18n.t("game-native-linux")
    } else {
        game.linux
            .protondb
            .map(|tier| format!("{tier:?}").to_uppercase())
            .unwrap_or_else(|| "—".to_owned())
    };
    let anticheat = match (&game.anticheat_name, game.linux.anticheat) {
        (Some(name), Some(status)) => format!("{name}: {}", format!("{status:?}").to_uppercase()),
        _ => i18n.t("game-none"),
    };
    let deck = game
        .deck
        .clone()
        .map(|deck| deck.to_uppercase())
        .unwrap_or_else(|| "—".to_owned());
    [
        (i18n.t("game-protondb"), proton),
        (i18n.t("game-anticheat"), anticheat),
        (i18n.t("game-deck"), deck),
    ]
}

/// Your PC next to each tier, with the bottleneck row highlighted.
fn requirements<'a>(
    game: &'a Game,
    status: &Status,
    rig: &Rig,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let verdict = match status {
        Status::Verdict(verdict) => Some(verdict),
        _ => None,
    };
    let bottleneck = verdict.and_then(|verdict| verdict.bottleneck);
    let (minimum, recommended) = (game.minimum.as_ref(), game.recommended.as_ref());
    let rows = ROWS.map(|component| {
        let yours = match component {
            Component::Cpu => rig.cpu.clone(),
            Component::Gpu => rig.gpu.clone(),
            Component::Ram => rig.ram.clone(),
            Component::Vram => rig.vram.clone(),
            _ => rig.api.clone(),
        };
        (component, yours)
    });

    let cell = |content: String, bold: bool, width: f32| {
        let label = text(content).size(13);
        container(if bold { label.font(BOLD) } else { label })
            .padding([8, 10])
            .width(iced::Length::FillPortion(width as u16))
    };
    let mut table = Column::new().push(
        row![
            cell(String::new(), true, 2.0),
            cell(i18n.t("game-your-pc"), true, 3.0),
            cell(i18n.t("game-minimum"), true, 4.0),
            cell(i18n.t("game-recommended"), true, 4.0),
            cell(i18n.t("game-result"), true, 3.0),
        ]
        .width(Fill),
    );
    for (component, yours) in rows {
        let highlight = bottleneck == Some(component);
        // The bottleneck in its level's color (red below the minimum), not
        // the accent, which is green in some themes and reads as "fine".
        let level = verdict
            .and_then(|verdict| verdict.components.iter().find(|(c, _)| *c == component))
            .map_or(Level::Below, |(_, level)| *level);
        let line = row![
            cell(i18n.t(row_id(component)), true, 2.0),
            cell(yours, highlight, 3.0),
            cell(requirement(minimum, component), false, 4.0),
            cell(requirement(recommended, component), false, 4.0),
            cell(result(status, component, i18n), true, 3.0),
        ]
        .width(Fill);
        table = table.push(container(line).style(move |_| {
            let background = if highlight {
                level_color(level)
            } else {
                colors.panel
            };
            container::Style {
                text_color: Some(if highlight { colors.line } else { colors.text }),
                ..container::Style::default().background(background)
            }
        }));
    }
    container(table)
        .padding(3)
        .width(Fill)
        .style(move |_| colors.boxed(colors.panel))
        .into()
}

/// The requirements as Steam's store page words them, next to each other,
/// so anything the parser missed is still there to read (ROADMAP §1.5).
/// `None` if the store page lists none.
fn steam_text<'a>(game: &'a Game, i18n: &I18n, colors: Colors) -> Option<Element<'a, Message>> {
    let tiers: Vec<&str> = [&game.minimum, &game.recommended]
        .into_iter()
        .filter_map(|tier| tier.as_ref()?.text.as_deref())
        .collect();
    if tiers.is_empty() {
        return None;
    }
    let columns = tiers.into_iter().map(|tier| {
        Column::with_children(tier.lines().map(|line| text(line).size(13).into()))
            .spacing(4)
            .width(Fill)
            .into()
    });
    Some(
        container(
            column![
                text(i18n.t("game-steam-text")).size(11).font(BOLD),
                row(columns).spacing(18),
            ]
            .spacing(10),
        )
        .padding(12)
        .width(Fill)
        .style(move |_| colors.boxed(colors.panel))
        .into(),
    )
}

/// ProtonDB, anti-cheat and Steam Deck boxes.
fn linux<'a>(game: &'a Game, i18n: &I18n, colors: Colors) -> Element<'a, Message> {
    let [proton, anticheat, deck] = linux_facts(game, i18n);
    let tile = |label: String, value: String| {
        container(column![text(label).size(11).font(BOLD), text(value).font(BOLD)].spacing(4))
            .padding(12)
            .width(Fill)
            .style(move |_| colors.boxed(colors.panel))
    };
    row![
        tile(proton.0, proton.1),
        tile(anticheat.0, anticheat.1),
        tile(deck.0, deck.1),
    ]
    .spacing(16)
    .into()
}

fn part_id(component: Component) -> &'static str {
    match component {
        Component::Cpu => "part-cpu",
        Component::Gpu => "part-gpu",
        Component::Ram => "part-ram",
        Component::Vram => "part-vram",
        Component::Storage => "part-storage",
        Component::GraphicsApi => "part-api",
    }
}

pub fn row_id(component: Component) -> &'static str {
    match component {
        Component::Cpu => "row-cpu",
        Component::Gpu => "row-gpu",
        Component::Ram => "row-ram",
        Component::Vram => "row-vram",
        Component::Storage => "row-storage",
        Component::GraphicsApi => "row-api",
    }
}
