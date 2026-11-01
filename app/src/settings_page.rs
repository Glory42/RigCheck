//! The Settings page. For now it holds Appearance.

use iced::border::Border;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Color, Element};

use crate::Message;
use crate::settings::Appearance;
use crate::style::{BOLD, Colors};

pub fn view(current: Appearance, colors: Colors) -> Element<'static, Message> {
    let options =
        Appearance::ALL.map(|appearance| option(appearance, appearance == current, colors));
    column![
        text("APPEARANCE").font(BOLD).size(20),
        text("Colors for the whole app. Saved for next time.").color(colors.muted),
        row(options).spacing(18),
    ]
    .spacing(12)
    .into()
}

/// A button showing an appearance's name and its main colors.
fn option(appearance: Appearance, selected: bool, colors: Colors) -> Element<'static, Message> {
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
    let status = if selected { "SELECTED" } else { " " };

    button(
        column![
            preview,
            text(appearance.name().to_uppercase()).font(BOLD),
            text(status).size(11).font(BOLD),
        ]
        .spacing(8)
        .padding(6),
    )
    .style(colors.button(selected))
    .on_press(Message::AppearanceSelected(appearance))
    .into()
}
