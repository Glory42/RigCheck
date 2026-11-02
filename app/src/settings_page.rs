//! The Settings page: Appearance, Language, Storage and Privacy.

use iced::border::Border;
use iced::widget::{Space, button, column, container, row, scrollable, space, text};
use iced::{Color, Element, Fill};

use crate::Message;
use crate::i18n::{I18n, Language};
use crate::settings::{Appearance, Settings};
use crate::style::{BOLD, Colors};

/// What the app keeps on disk, in bytes (ROADMAP §7.1).
#[derive(Debug, Clone, Copy, Default)]
pub struct Storage {
    pub games_db: u64,
    pub fetched_db: u64,
    pub images: u64,
}

/// The image cache limits to pick from, in MB.
const IMAGE_LIMITS: [u32; 4] = [100, 250, 500, 1000];

/// `account` is the Steam account section ([`crate::account::view`]).
pub fn view(
    settings: &Settings,
    storage: Storage,
    account: Element<'static, Message>,
    i18n: &I18n,
    colors: Colors,
) -> Element<'static, Message> {
    let appearances = Appearance::ALL.map(|appearance| {
        appearance_option(appearance, appearance == settings.appearance, i18n, colors)
    });
    let languages = [None]
        .into_iter()
        .chain(Language::ALL.map(Some))
        .map(|language| language_option(language, settings.language == language, i18n, colors));

    let page = column![
        section(
            i18n.t("appearance-title"),
            i18n.t("appearance-description"),
            colors
        ),
        // Wraps in a narrow window instead of running off the side.
        row(appearances).spacing(18).wrap(),
        section(
            i18n.t("language-title"),
            i18n.t("language-description"),
            colors
        ),
        row(languages).spacing(18).wrap(),
        section(
            i18n.t("account-title"),
            i18n.t("account-description"),
            colors
        ),
        account,
        section(
            i18n.t("storage-title"),
            i18n.t("storage-description"),
            colors
        ),
        storage_rows(settings, storage, i18n, colors),
        section(
            i18n.t("privacy-title"),
            i18n.t("privacy-description"),
            colors
        ),
        privacy(settings, i18n, colors),
    ]
    .spacing(12);
    // Room on the right and bottom so hard shadows aren't clipped.
    scrollable(page.padding([0, 6])).width(Fill).into()
}

/// Sizes and what can be done about them.
fn storage_rows(
    settings: &Settings,
    storage: Storage,
    i18n: &I18n,
    colors: Colors,
) -> Element<'static, Message> {
    let action = |id: &str, message: Message| {
        button(text(i18n.t(id)).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(false))
            .on_press(message)
    };
    let line = |id: &str, bytes: u64, actions: Vec<Element<'static, Message>>| {
        row![
            text(i18n.t(id)).font(BOLD).width(180),
            text(megabytes(bytes)).width(90),
            row(actions).spacing(8),
        ]
        .spacing(12)
        .align_y(iced::Center)
    };
    let limits = IMAGE_LIMITS.map(|mb| {
        let label = if mb >= 1000 {
            format!("{} GB", mb / 1000)
        } else {
            format!("{mb} MB")
        };
        let on = settings.image_cache_bytes() == u64::from(mb) * 1_000_000;
        button(text(label).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(on))
            .on_press(Message::ImageCacheLimit(mb))
            .into()
    });
    column![
        line(
            "storage-games-db",
            storage.games_db,
            vec![
                action("storage-check", Message::CheckForUpdates).into(),
                action("storage-redownload", Message::Redownload).into(),
            ],
        ),
        line(
            "storage-fetched",
            storage.fetched_db,
            vec![action("storage-clear-fetched", Message::ClearFetched).into()],
        ),
        line(
            "storage-images",
            storage.images,
            vec![action("storage-clear-images", Message::ClearImages).into()],
        ),
        row![
            text(i18n.t("storage-image-limit")).font(BOLD).width(180),
            space::horizontal().width(90),
            row(limits).spacing(8),
        ]
        .spacing(12)
        .align_y(iced::Center),
    ]
    .spacing(10)
    .into()
}

/// The update choice and every place the app sends requests (ROADMAP §6).
fn privacy(settings: &Settings, i18n: &I18n, colors: Colors) -> Element<'static, Message> {
    let choice = |id: &str, manual: bool| {
        button(
            column![
                text(i18n.t(id)).font(BOLD),
                status(settings.manual_updates == manual, i18n)
            ]
            .spacing(8)
            .padding(6),
        )
        .style(colors.button(settings.manual_updates == manual))
        .on_press(Message::SetManualUpdates(manual))
    };
    let destinations = ["privacy-github", "privacy-steam-store", "privacy-steam-cdn"]
        .map(|id| text(i18n.t(id)).into());
    column![
        text(i18n.t("privacy-updates")).font(BOLD),
        row![
            choice("privacy-automatic", false),
            choice("privacy-manual", true)
        ]
        .spacing(18),
        text(i18n.t("privacy-destinations")).font(BOLD),
        column(destinations).spacing(4),
    ]
    .spacing(12)
    .into()
}

/// `bytes` in megabytes with one decimal, e.g. "18.3 MB".
fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

fn section(title: String, description: String, colors: Colors) -> Element<'static, Message> {
    column![
        text(title).font(BOLD).size(20),
        text(description).color(colors.muted),
    ]
    .spacing(6)
    .padding([10, 0])
    .into()
}

/// A button showing an appearance's name and its main colors.
fn appearance_option(
    appearance: Appearance,
    selected: bool,
    i18n: &I18n,
    colors: Colors,
) -> Element<'static, Message> {
    let own = appearance.colors();
    let swatch = |color: Color| {
        container(Space::new().width(22).height(22)).style(move |_| container::Style {
            background: Some(color.into()),
            border: Border {
                color: own.line,
                width: 2.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
    };
    // System: two of Paper's colors and two of Ink's, light and dark.
    let (light, dark) = (Appearance::Paper.colors(), Appearance::Ink.colors());
    let shown = match appearance {
        Appearance::System => [light.bg, light.hud, dark.bg, dark.accent],
        _ => [own.bg, own.hud, own.panel, own.accent],
    };
    let preview = row![
        swatch(shown[0]),
        swatch(shown[1]),
        swatch(shown[2]),
        swatch(shown[3])
    ]
    .spacing(4);
    let name = i18n.t(&format!("theme-{}", appearance.name().to_lowercase()));

    button(
        column![preview, text(name).font(BOLD), status(selected, i18n)]
            .spacing(8)
            .padding(6),
    )
    .style(colors.button(selected))
    .on_press(Message::AppearanceSelected(appearance))
    .into()
}

/// A button for one language, or for following the system (`None`).
fn language_option(
    language: Option<Language>,
    selected: bool,
    i18n: &I18n,
    colors: Colors,
) -> Element<'static, Message> {
    let name = match language {
        Some(language) => language.native_name().to_owned(),
        None => i18n.t("language-system"),
    };
    button(
        column![text(name).font(BOLD), status(selected, i18n)]
            .spacing(8)
            .padding(6),
    )
    .style(colors.button(selected))
    .on_press(Message::LanguageSelected(language))
    .into()
}

fn status(selected: bool, i18n: &I18n) -> Element<'static, Message> {
    let label = if selected {
        i18n.t("selected")
    } else {
        " ".to_owned()
    };
    text(label).size(11).font(BOLD).into()
}
