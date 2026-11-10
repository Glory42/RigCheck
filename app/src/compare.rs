//! Two games side by side: verdict, what each asks of every part and how
//! this PC does, and the Linux facts (ROADMAP Phase 8).

use iced::widget::{Column, button, column, container, row, text};
use iced::{Element, Fill};
use rigcheck_gamedb::Game;

use crate::game_page::{ROWS, linux_facts, requirement, result, row_id};
use crate::i18n::I18n;
use crate::library::{Status, stamp};
use crate::style::{BOLD, Colors, scroll};
use crate::{Message, Page};

/// One of the two games.
pub struct Side {
    pub app_id: u32,
    pub name: String,
    pub record: Option<Game>,
    pub status: Status,
}

pub fn view(a: Side, b: Side, i18n: &I18n, colors: Colors) -> Element<'static, Message> {
    let back = button(text(i18n.t("compare-back")).font(BOLD).size(13))
        .padding([7, 12])
        .style(colors.button(false))
        .on_press(Message::Open(Page::Game(b.app_id)));
    let label = |content: String| container(text(content).font(BOLD).size(13)).width(160);
    let cell = |content: Element<'static, Message>| container(content).width(Fill).padding([8, 10]);
    let line = |name: String, left: Element<'static, Message>, right: Element<'static, Message>| {
        row![label(name), cell(left), cell(right)].align_y(iced::Center)
    };
    let stamped = |side: &Side| -> Element<'static, Message> {
        let (text_label, color) = stamp(&side.status, i18n);
        container(text(text_label).font(BOLD).size(14))
            .padding([3, 8])
            .style(move |_| container::Style {
                text_color: Some(colors.line),
                ..container::Style::default().background(color)
            })
            .into()
    };

    let mut table = Column::new()
        .push(line(
            String::new(),
            text(a.name.clone()).font(BOLD).size(18).into(),
            text(b.name.clone()).font(BOLD).size(18).into(),
        ))
        .push(line(i18n.t("compare-verdict"), stamped(&a), stamped(&b)));
    for component in ROWS {
        let side = |side: &Side| -> Element<'static, Message> {
            let minimum = side.record.as_ref().and_then(|game| game.minimum.as_ref());
            column![
                text(requirement(minimum, component)).size(13),
                text(result(&side.status, component, i18n))
                    .font(BOLD)
                    .size(12),
            ]
            .spacing(2)
            .into()
        };
        table = table.push(line(i18n.t(row_id(component)), side(&a), side(&b)));
    }
    let facts = |side: &Side| side.record.as_ref().map(|game| linux_facts(game, i18n));
    if let (Some(left), Some(right)) = (facts(&a), facts(&b)) {
        for ((name, left), (_, right)) in left.into_iter().zip(right) {
            table = table.push(line(
                name,
                text(left).font(BOLD).size(13).into(),
                text(right).font(BOLD).size(13).into(),
            ));
        }
    }

    scroll(
        column![
            back,
            text(i18n.t("compare-minimum-note")).color(colors.muted),
            container(table.spacing(4))
                .padding(10)
                .width(Fill)
                .style(move |_| colors.boxed(colors.panel)),
        ]
        .spacing(16)
        .padding([0, 6]),
        colors,
    )
    .into()
}
