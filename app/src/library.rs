//! The Library page: installed games as cover tiles with verdict stamps.

use std::path::PathBuf;

use iced::widget::grid::Sizing;
use iced::widget::image::Handle;
use iced::widget::{
    Space, button, center, column, container, grid, image, row, scrollable, space, text, text_input,
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

/// A game tile's size: the cover, the stamp (24) and the name (50), inside
/// a 3 px padding. Fixed, so the Catalog's grid maps scroll to rows.
pub const TILE_WIDTH: f32 = COVER_WIDTH + 6.0;
pub const TILE_HEIGHT: f32 = COVER_HEIGHT + 24.0 + 50.0 + 6.0;

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

#[derive(Clone)]
pub struct Entry<'a> {
    pub game: &'a InstalledGame,
    pub status: Status,
    pub cover: Option<&'a PathBuf>,
}

/// `games` is `None` while Steam is being scanned.
/// How the Library lists games.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Name,
    /// Grouped under each verdict, best first.
    Verdict,
}

/// `games` is `None` while Steam is being scanned.
pub fn view<'a>(
    games: Option<Vec<Entry<'a>>>,
    search: &str,
    order: Order,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let Some(mut games) = games else {
        return center(text(i18n.t("library-scanning"))).into();
    };
    let search_box = text_input(&i18n.t("library-search"), search)
        .id(SEARCH)
        .on_input(Message::Search)
        // Enter opens the first game listed.
        .on_submit(Message::LibraryOpenFirst)
        .padding(10)
        .font(BOLD);
    let order_button = |id: &str, this: Order| {
        button(text(i18n.t(id)).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(order == this))
            .on_press(Message::LibraryOrder(this))
    };
    let mut header = row![
        order_button("library-by-name", Order::Name),
        order_button("library-by-verdict", Order::Verdict),
    ]
    .spacing(8)
    .align_y(iced::Center);
    if let Some(summary) = summary(&games, i18n) {
        header = header
            .push(space::horizontal())
            .push(text(summary).font(BOLD));
    }

    let query = search.to_lowercase();
    games.retain(|entry| entry.game.name.to_lowercase().contains(&query));
    games.sort_by_key(|entry| entry.game.name.to_lowercase());
    let content: Element<'a, Message> = match order {
        Order::Name => tiles(games, i18n, colors),
        Order::Verdict => {
            games.sort_by_key(|entry| rank(&entry.status));
            let mut groups = column![].spacing(18);
            for group in games.chunk_by(|a, b| rank(&a.status) == rank(&b.status)) {
                let (label, _) = stamp(&group[0].status, i18n);
                groups = groups.push(
                    text(format!("{label} · {}", group.len()))
                        .font(BOLD)
                        .size(16),
                );
                let group: Vec<Entry<'a>> = group.iter().map(Entry::clone).collect();
                groups = groups.push(tiles(group, i18n, colors));
            }
            groups.into()
        }
    };

    column![search_box, header, scrollable(content).height(Fill)]
        .spacing(16)
        .into()
}

/// The tile grid. Shrink: each tile is as tall as its content, not square.
fn tiles<'a>(games: Vec<Entry<'a>>, i18n: &I18n, colors: Colors) -> Element<'a, Message> {
    grid(games.into_iter().map(|entry| tile(entry, i18n, colors)))
        .fluid(COVER_WIDTH + 30.0)
        .height(Sizing::EvenlyDistribute(iced::Length::Shrink))
        .spacing(18)
        .into()
}

/// "38 of 42 installed games meet the recommended specs.", once every
/// game has a verdict or is known to have none.
fn summary(games: &[Entry], i18n: &I18n) -> Option<String> {
    if games.is_empty()
        || games
            .iter()
            .any(|entry| matches!(entry.status, Status::Checking))
    {
        return None;
    }
    let ready = games
        .iter()
        .filter(
            |entry| matches!(&entry.status, Status::Verdict(v) if v.overall == Level::Recommended),
        )
        .count();
    Some(i18n.t_with(
        "library-summary",
        &[
            ("ready", &ready.to_string()),
            ("total", &games.len().to_string()),
        ],
    ))
}

/// Best first: the order verdict groups are listed in.
fn rank(status: &Status) -> u8 {
    match status {
        Status::Verdict(verdict) => match verdict.overall {
            Level::Recommended => 0,
            Level::Minimum => 1,
            Level::Below => 2,
            Level::Blocked => 3,
            Level::Unknown => 4,
        },
        Status::NotChecked => 5,
        Status::NoData => 6,
        Status::Checking => 7,
    }
}

fn tile<'a>(entry: Entry<'a>, i18n: &I18n, colors: Colors) -> Element<'a, Message> {
    let game = entry.game;
    // The grid stretches cells to its column width; the container takes that,
    // so the tile keeps the cover's width.
    container(game_tile(
        game.app_id,
        &game.name,
        &entry.status,
        entry.cover,
        false,
        i18n,
        colors,
    ))
    .width(Fill)
    .into()
}

/// A game as a tile: cover, verdict stamp and name, opening its page.
/// [`TILE_WIDTH`] × [`TILE_HEIGHT`]; `selected` outlines it for the keyboard.
pub fn game_tile(
    app_id: u32,
    name: &str,
    status: &Status,
    cover: Option<&PathBuf>,
    selected: bool,
    i18n: &I18n,
    colors: Colors,
) -> Element<'static, Message> {
    let cover: Element<'static, Message> = match cover {
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
    let (label, color) = stamp(status, i18n);
    // Fixed heights: the grid sizes cells to fit, and would squash the stamp.
    let stamp = container(text(label).font(BOLD).size(12))
        .padding([3, 6])
        .width(Fill)
        .height(24)
        .style(move |_| container::Style {
            text_color: Some(colors.line),
            ..container::Style::default().background(color)
        });
    let name = container(text(name.to_owned()).font(BOLD).size(12))
        .padding([6, 6])
        .height(50);

    button(column![cover, stamp, name].width(COVER_WIDTH))
        .width(TILE_WIDTH)
        .height(TILE_HEIGHT)
        .padding(3)
        .style(move |_, _| {
            let boxed = colors.boxed(if selected {
                colors.accent
            } else {
                colors.panel
            });
            button::Style {
                background: boxed.background,
                text_color: colors.text,
                border: boxed.border,
                shadow: boxed.shadow,
                snap: true,
            }
        })
        .on_press(Message::Open(Page::Game(app_id)))
        .into()
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
