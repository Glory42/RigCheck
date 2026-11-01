//! The Settings page: Appearance and Language.

use iced::border::Border;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Color, Element};

use crate::Message;
use crate::i18n::{I18n, Language};
use crate::settings::{Appearance, Settings};
use crate::style::{BOLD, Colors};

pub fn view(settings: &Settings, i18n: &I18n, colors: Colors) -> Element<'static, Message> {
    let appearances = Appearance::ALL.map(|appearance| {
        appearance_option(appearance, appearance == settings.appearance, i18n, colors)
    });
    let languages = [None]
        .into_iter()
        .chain(Language::ALL.map(Some))
        .map(|language| language_option(language, settings.language == language, i18n, colors));

    column![
        section(
            i18n.t("appearance-title"),
            i18n.t("appearance-description"),
            colors
        ),
        row(appearances).spacing(18),
        section(
            i18n.t("language-title"),
            i18n.t("language-description"),
            colors
        ),
        row(languages).spacing(18),
    ]
    .spacing(12)
    .into()
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
    let preview = row![
        swatch(own.bg),
        swatch(own.hud),
        swatch(own.panel),
        swatch(own.accent)
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
