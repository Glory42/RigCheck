//! Colors for each appearance, from the round 2 prototype
//! (branch `prototype/ui-variants`).

use iced::border::Border;
use iced::font::{Font, Weight};
use iced::theme::Palette;
use iced::widget::{button, container};
use iced::{Color, Shadow, Theme, Vector};

use crate::settings::Appearance;

/// The colors design D draws with.
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub bg: Color,
    /// The top bar.
    pub hud: Color,
    pub panel: Color,
    /// Alternate rows and labels.
    pub panel2: Color,
    /// Outlines and hard shadows.
    pub line: Color,
    /// The thick line under the top bar.
    pub rule: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    /// Text on top of `accent`.
    pub on_accent: Color,
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Appearance {
    pub fn name(self) -> &'static str {
        match self {
            Appearance::Ink => "Ink",
            Appearance::Midnight => "Midnight",
            Appearance::Plum => "Plum",
            Appearance::Forest => "Forest",
            Appearance::Paper => "Paper",
        }
    }

    pub fn colors(self) -> Colors {
        match self {
            Appearance::Ink => Colors {
                bg: rgb(0x0c0c0e),
                hud: rgb(0x0c0c0e),
                panel: rgb(0x18181c),
                panel2: rgb(0x222228),
                line: rgb(0x000000),
                rule: rgb(0xf58a1f),
                text: rgb(0xf2f2f2),
                muted: rgb(0x9a9aa3),
                accent: rgb(0xf58a1f),
                on_accent: rgb(0x111111),
            },
            Appearance::Midnight => Colors {
                bg: rgb(0x0a0f24),
                hud: rgb(0x121a3d),
                panel: rgb(0x141d42),
                panel2: rgb(0x1c2754),
                line: rgb(0x04061a),
                rule: rgb(0x04061a),
                text: rgb(0xeef1ff),
                muted: rgb(0x9aa6d6),
                accent: rgb(0xffb020),
                on_accent: rgb(0x1a1300),
            },
            Appearance::Plum => Colors {
                bg: rgb(0x150b1f),
                hud: rgb(0x241237),
                panel: rgb(0x23123a),
                panel2: rgb(0x2f1a4b),
                line: rgb(0x07020c),
                rule: rgb(0x07020c),
                text: rgb(0xf6ecff),
                muted: rgb(0xb9a3d6),
                accent: rgb(0xff6b9a),
                on_accent: rgb(0x1d0710),
            },
            Appearance::Forest => Colors {
                bg: rgb(0x06120c),
                hud: rgb(0x0b2216),
                panel: rgb(0x0e2419),
                panel2: rgb(0x143222),
                line: rgb(0x010603),
                rule: rgb(0x010603),
                text: rgb(0xe8f7ec),
                muted: rgb(0x8fb89b),
                accent: rgb(0x2ea043),
                on_accent: rgb(0x04130a),
            },
            Appearance::Paper => Colors {
                bg: rgb(0xf2e8d5),
                hud: rgb(0xf58a1f),
                panel: rgb(0xfffaf0),
                panel2: rgb(0xf7ecd6),
                line: rgb(0x1b1410),
                rule: rgb(0x1b1410),
                text: rgb(0x1b1410),
                muted: rgb(0x6b5a48),
                accent: rgb(0x2ea043),
                on_accent: rgb(0x08160c),
            },
        }
    }

    /// The iced theme for built-in widgets, named after the appearance.
    pub fn theme(self) -> Theme {
        let colors = self.colors();
        Theme::custom(
            self.name(),
            Palette {
                background: colors.bg,
                text: colors.text,
                primary: colors.accent,
                success: rgb(0x2ea043),
                warning: rgb(0xd29922),
                danger: rgb(0xe5534b),
            },
        )
    }
}

/// Design D's lettering: bold monospace.
pub const BOLD: Font = Font {
    weight: Weight::Bold,
    ..Font::MONOSPACE
};

impl Colors {
    /// A chunky box: thick outline and a hard shadow with no blur.
    pub fn boxed(self, background: Color) -> container::Style {
        container::Style {
            text_color: Some(self.text),
            background: Some(background.into()),
            border: Border {
                color: self.line,
                width: 3.0,
                radius: 0.0.into(),
            },
            shadow: self.hard_shadow(4.0),
            snap: true,
        }
    }

    /// A tab or button; `on` paints it in the accent color.
    pub fn button(self, on: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |_, status| {
            let (background, text_color) = if on {
                (self.accent, self.on_accent)
            } else {
                (self.panel, self.text)
            };
            let pressed = status == button::Status::Pressed;
            button::Style {
                background: Some(background.into()),
                text_color,
                border: Border {
                    color: self.line,
                    width: 3.0,
                    radius: 0.0.into(),
                },
                // Pressing flattens the shadow, like pushing a physical key.
                shadow: self.hard_shadow(if pressed { 0.0 } else { 3.0 }),
                snap: true,
            }
        }
    }

    fn hard_shadow(self, offset: f32) -> Shadow {
        Shadow {
            color: self.line,
            offset: Vector::new(offset, offset),
            blur_radius: 0.0,
        }
    }
}

/// The stamp color for a verdict level.
pub fn level_color(level: rigcheck_core::Level) -> Color {
    use rigcheck_core::Level;
    match level {
        Level::Recommended => rgb(0x2ea043),
        Level::Minimum => rgb(0xd29922),
        Level::Below => rgb(0xe5534b),
        Level::Blocked => rgb(0xa371f7),
        Level::Unknown => rgb(0x8b949e),
    }
}
