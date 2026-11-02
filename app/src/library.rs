//! The Library page: installed games as cover tiles with verdict stamps.

use std::path::PathBuf;

use iced::widget::grid::Sizing;
use iced::widget::image::Handle;
use iced::widget::{
    Space, button, center, column, container, grid, image, scrollable, text, text_input,
};
use iced::{ContentFit, Element, Fill};
use rigcheck_core::{Level, Specs, Verdict};
use rigcheck_gamedb::Game;
use rigcheck_steam::InstalledGame;

use crate::i18n::I18n;
use crate::style::{BOLD, Colors, level_color};
use crate::{Message, Page};

/// Steam's library art is 600×900; tiles show it at this size.
const COVER_WIDTH: f32 = 164.0;
const COVER_HEIGHT: f32 = 246.0;

/// The search box's widget ID.
pub const SEARCH: &str = "library-search";

/// What the app knows about an installed game's chances.
#[derive(Debug, Clone)]
pub enum Status {
    /// Waiting for hardware detection.
    Checking,
    /// Not in games.db.
    NoData,
    /// In games.db by name only: its store page hasn't been read yet.
    NotChecked,
    Verdict(Verdict),
}

/// The status of `game` (its games.db record, if any) on a PC with `specs`
/// (`None` until hardware detection finishes).
pub fn status(game: Option<&Game>, specs: Option<&Specs>) -> Status {
    match (game, specs) {
        (None, _) => Status::NoData,
        (Some(game), _) if !game.crawled => Status::NotChecked,
        (Some(_), None) => Status::Checking,
        (Some(game), Some(specs)) => Status::Verdict(game.verdict(specs)),
    }
}

pub struct Entry<'a> {
    pub game: &'a InstalledGame,
    pub status: Status,
    pub cover: Option<&'a PathBuf>,
}

/// `games` is `None` while Steam is being scanned.
pub fn view<'a>(
    games: Option<Vec<Entry<'a>>>,
    search: &str,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let Some(games) = games else {
        return center(text(i18n.t("library-scanning"))).into();
    };
    let search_box = text_input(&i18n.t("library-search"), search)
        .id(SEARCH)
        .on_input(Message::Search)
        .padding(10)
        .font(BOLD);
    let query = search.to_lowercase();
    let tiles = games
        .into_iter()
        .filter(|entry| entry.game.name.to_lowercase().contains(&query))
        .map(|entry| tile(entry, i18n, colors));

    column![
        search_box,
        // Shrink: each tile is as tall as its content, not square.
        scrollable(
            grid(tiles)
                .fluid(COVER_WIDTH + 30.0)
                .height(Sizing::EvenlyDistribute(iced::Length::Shrink))
                .spacing(18),
        )
        .height(Fill),
    ]
    .spacing(16)
    .into()
}

fn tile<'a>(entry: Entry<'a>, i18n: &I18n, colors: Colors) -> Element<'a, Message> {
    let cover: Element<'a, Message> = match entry.cover {
        // Exactly the art's 2:3 shape, so filling the box never spills over.
        Some(path) => image(Handle::from_path(path))
            .width(COVER_WIDTH)
            .height(COVER_HEIGHT)
            .content_fit(ContentFit::Cover)
            .into(),
        None => container(Space::new())
            .width(COVER_WIDTH)
            .height(COVER_HEIGHT)
            .style(move |_| container::Style::default().background(colors.panel2))
            .into(),
    };
    let (label, color) = stamp(&entry.status, i18n);
    // Fixed heights: the grid sizes cells to fit, and would squash the stamp.
    let stamp = container(text(label).font(BOLD).size(12))
        .padding([3, 6])
        .width(Fill)
        .height(24)
        .style(move |_| container::Style {
            text_color: Some(colors.line),
            ..container::Style::default().background(color)
        });
    let name = container(text(entry.game.name.clone()).font(BOLD).size(12))
        .padding([6, 6])
        .height(50);

    let tile = button(column![cover, stamp, name].width(COVER_WIDTH))
        .width(COVER_WIDTH + 6.0) // + the padding inside the outline
        .padding(3)
        .style(move |_, _| {
            let boxed = colors.boxed(colors.panel);
            button::Style {
                background: boxed.background,
                text_color: colors.text,
                border: boxed.border,
                shadow: boxed.shadow,
                snap: true,
            }
        })
        .on_press(Message::Open(Page::Game(entry.game.app_id)));
    // The grid stretches cells to its column width; the container takes that,
    // so the tile keeps the cover's width.
    container(tile).width(Fill).into()
}

/// The stamp's text and color.
pub fn stamp(status: &Status, i18n: &I18n) -> (String, iced::Color) {
    let level = match status {
        Status::Checking => return (i18n.t("stamp-checking"), level_color(Level::Unknown)),
        Status::NoData => return (i18n.t("stamp-no-data"), level_color(Level::Unknown)),
        Status::NotChecked => {
            return (i18n.t("stamp-not-checked"), level_color(Level::Unknown));
        }
        Status::Verdict(verdict) => verdict.overall,
    };
    let id = match level {
        Level::Recommended => "stamp-recommended",
        Level::Minimum => "stamp-minimum",
        Level::Below => "stamp-below",
        Level::Blocked => "stamp-blocked",
        Level::Unknown => "stamp-unknown",
    };
    (i18n.t(id), level_color(level))
}
