//! The About page: version, licenses, links and the data-source credits
//! ROADMAP §9 asks for.

use iced::widget::{button, column, container, row, text};
use iced::{Element, Fill};
use rigcheck_core::Level;

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors, level_color, scroll};

const REPOSITORY: &str = "https://github.com/Glory42/rigcheck";

/// Every data source with its license (ROADMAP §9). Names and license IDs,
/// so the same in every language.
const CREDITS: [&str; 6] = [
    "Steam store data: © Valve Corporation",
    "ProtonDB reports: ODbL 1.0, via bdefore/protondb-data",
    "AreWeAntiCheatYet: MIT, © AreWeAntiCheatYet contributors",
    "Steam Deck compatibility reports: © Valve Corporation",
    "Blender Open Data: CC0",
    "pci.ids: BSD-3-Clause or GPL-2.0",
];

/// Each stamp in its color, with what it means.
fn legend(i18n: &I18n, colors: Colors) -> Element<'static, Message> {
    let entry = |stamp: String, level: Level, meaning: &str| {
        row![
            container(text(stamp).font(BOLD).size(12))
                .padding([3, 6])
                .width(110)
                .style(move |_| container::Style {
                    text_color: Some(colors.line),
                    ..container::Style::default().background(level_color(level))
                }),
            text(i18n.t(meaning)),
        ]
        .spacing(12)
        .align_y(iced::Center)
        .into()
    };
    let partial = format!(
        "{} {}",
        i18n.t("stamp-recommended-partial"),
        i18n.t("stamp-minimum-partial")
    );
    column![text(i18n.t("legend-title")).font(BOLD).size(11)]
        .extend([
            entry(
                i18n.t("stamp-recommended"),
                Level::Recommended,
                "legend-recommended",
            ),
            entry(i18n.t("stamp-minimum"), Level::Minimum, "legend-minimum"),
            entry(partial, Level::Minimum, "legend-partial"),
            entry(i18n.t("stamp-below"), Level::Below, "legend-below"),
            entry(i18n.t("stamp-blocked"), Level::Blocked, "legend-blocked"),
            entry(i18n.t("stamp-unknown"), Level::Unknown, "legend-unknown"),
            entry(
                i18n.t("stamp-not-checked"),
                Level::Unknown,
                "legend-not-checked",
            ),
        ])
        .spacing(8)
        .into()
}

pub fn view(i18n: &I18n, colors: Colors) -> Element<'static, Message> {
    let link = |id: &str, url: String| {
        button(text(i18n.t(id)).font(BOLD).size(13))
            .padding([7, 12])
            .style(colors.button(false))
            .on_press(Message::OpenUrl(url))
    };
    let boxed = |content: Element<'static, Message>| {
        container(content)
            .padding(14)
            .width(Fill)
            .style(move |_| colors.boxed(colors.panel))
    };
    let credits = CREDITS.map(|line| text(line).into());
    let page = column![
        text("RIGCHECK").font(BOLD).size(24),
        text(i18n.t_with("about-version", &[("version", env!("CARGO_PKG_VERSION"))])),
        text(i18n.t("about-tagline")),
        row![
            link("about-source", REPOSITORY.to_owned()),
            link("about-bugs", format!("{REPOSITORY}/issues")),
        ]
        .spacing(12),
        boxed(
            column![
                text(i18n.t("about-licenses")).font(BOLD).size(11),
                text("rigcheck: MIT License"),
                text(i18n.t("about-database-license")),
            ]
            .spacing(6)
            .into()
        ),
        boxed(legend(i18n, colors)),
        boxed(
            column![
                text(i18n.t("about-keyboard")).font(BOLD).size(11),
                text(i18n.t("about-keys-tabs")),
                text(i18n.t("about-keys-search")),
                text(i18n.t("about-keys-move")),
                text(i18n.t("about-keys-back")),
            ]
            .spacing(6)
            .into()
        ),
        boxed(
            column![text(i18n.t("about-credits")).font(BOLD).size(11)]
                .extend(credits)
                .spacing(6)
                .into()
        ),
    ]
    .spacing(16);
    scroll(page.padding([0, 6]), colors).into()
}
