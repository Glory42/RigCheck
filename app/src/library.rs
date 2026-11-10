//! The Library page: installed games as cover tiles with verdict stamps.

use std::path::PathBuf;

use iced::widget::grid::Sizing;
use iced::widget::image::Handle;
use iced::widget::{
    Space, button, center, column, container, grid, image, row, space, text, text_input,
};
use iced::{ContentFit, Element, Fill};
use rigcheck_core::{Confidence, Level, Specs, Verdict};
use rigcheck_gamedb::Game;

use crate::i18n::I18n;
use crate::style::{BOLD, Colors, level_color, scroll};
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

/// Whether `verdict` compared the CPU, GPU and RAM all three. Only then is
/// a game called READY or LOW without a question mark: one part meeting
/// the specs says nothing about the others (ROADMAP §1, honest results).
pub fn fully_compared(verdict: &Verdict) -> bool {
    verdict.confidence == Confidence::High
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

/// A game in the Library: installed, or (with a Steam key) just owned.
#[derive(Clone)]
pub struct Entry<'a> {
    pub app_id: u32,
    pub name: &'a str,
    pub status: Status,
    pub cover: Cover<'a>,
}

/// A cover as drawn: the art (fading in as it arrives), a pulsing skeleton
/// while it's on its way, or a still placeholder when Steam has none.
#[derive(Debug, Clone, Copy)]
pub enum Cover<'a> {
    Ready {
        path: &'a PathBuf,
        opacity: f32,
    },
    /// `pulse` is 0…1 ([`crate::motion::pulse`]).
    Loading {
        pulse: f32,
    },
    Missing,
}

/// `cover` at `width` × `height`. The art fills the box exactly (Steam's
/// covers are 2:3, like the box), so it never spills over.
pub fn cover_box(
    cover: Cover,
    width: f32,
    height: f32,
    colors: Colors,
) -> Element<'static, Message> {
    let placeholder = |color: iced::Color| -> Element<'static, Message> {
        container(Space::new())
            .width(width)
            .height(height)
            .style(move |_| container::Style::default().background(color))
            .into()
    };
    match cover {
        Cover::Ready { path, opacity } => image(Handle::from_path(path))
            .width(width)
            .height(height)
            .content_fit(ContentFit::Cover)
            .opacity(opacity)
            .into(),
        // Breathes a little toward the muted text color, in every theme.
        Cover::Loading { pulse } => {
            placeholder(mix(colors.panel2, colors.muted, 0.06 + 0.14 * pulse))
        }
        Cover::Missing => placeholder(colors.panel2),
    }
}

/// `a`, moved `t` (0…1) of the way to `b`.
fn mix(a: iced::Color, b: iced::Color, t: f32) -> iced::Color {
    iced::Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

/// Which games the Library lists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Installed,
    /// Every game the account owns, from Steam's Web API.
    Owned,
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
/// `games` is `None` while Steam is being scanned. `note` replaces the
/// tiles when there's nothing to list and a reason to say (no Steam key, a
/// private profile…).
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    games: Option<Vec<Entry<'a>>>,
    search: &str,
    order: Order,
    scope: Scope,
    note: Option<String>,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let scope_button = |id: &str, this: Scope| {
        button(text(i18n.t(id)).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(scope == this))
            .on_press(Message::LibraryScope(this))
    };
    let scopes = row![
        scope_button("library-installed", Scope::Installed),
        scope_button("library-owned", Scope::Owned),
    ]
    .spacing(8);
    if let Some(note) = note {
        return column![scopes, center(text(note).font(BOLD))]
            .spacing(16)
            .into();
    }
    let Some(mut games) = games else {
        return column![scopes, center(text(i18n.t("library-scanning")))]
            .spacing(16)
            .into();
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
        scopes,
        space::horizontal().width(12),
        order_button("library-by-name", Order::Name),
        order_button("library-by-verdict", Order::Verdict),
    ]
    .spacing(8)
    .align_y(iced::Center);
    if let Some(summary) = summary(&games, scope, i18n) {
        header = header
            .push(space::horizontal())
            .push(text(summary).font(BOLD));
    }

    let query = search.to_lowercase();
    games.retain(|entry| entry.name.to_lowercase().contains(&query));
    games.sort_by_key(|entry| entry.name.to_lowercase());
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

    column![search_box, header, scroll(content, colors).height(Fill)]
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
fn summary(games: &[Entry], scope: Scope, i18n: &I18n) -> Option<String> {
    if games.is_empty()
        || games
            .iter()
            .any(|entry| matches!(entry.status, Status::Checking))
    {
        return None;
    }
    let ready = games
        .iter()
        .filter(|entry| {
            matches!(&entry.status, Status::Verdict(v)
                if v.overall == Level::Recommended && fully_compared(v))
        })
        .count();
    let id = match scope {
        Scope::Installed => "library-summary",
        Scope::Owned => "library-summary-owned",
    };
    Some(i18n.t_with(
        id,
        &[
            ("ready", &ready.to_string()),
            ("total", &games.len().to_string()),
        ],
    ))
}

/// Best first: the order verdict groups are listed in. READY? comes after
/// READY, LOW? after LOW.
fn rank(status: &Status) -> u8 {
    match status {
        Status::Verdict(verdict) => {
            let partly = u8::from(!fully_compared(verdict));
            match verdict.overall {
                Level::Recommended => partly,
                Level::Minimum => 2 + partly,
                Level::Below => 4,
                Level::Blocked => 5,
                Level::Unknown => 6,
            }
        }
        Status::NotChecked => 7,
        Status::NoData => 8,
        Status::Checking => 9,
    }
}

fn tile<'a>(entry: Entry<'a>, i18n: &I18n, colors: Colors) -> Element<'a, Message> {
    // The grid stretches cells to its column width; the container takes that,
    // so the tile keeps the cover's width.
    container(game_tile(
        entry.app_id,
        entry.name,
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
    cover: Cover,
    selected: bool,
    i18n: &I18n,
    colors: Colors,
) -> Element<'static, Message> {
    let cover = cover_box(cover, COVER_WIDTH, COVER_HEIGHT, colors);
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
        Status::Verdict(verdict) => verdict,
    };
    let partly = !fully_compared(level);
    let level = level.overall;
    let id = match level {
        Level::Recommended if partly => "stamp-recommended-partial",
        Level::Recommended => "stamp-recommended",
        Level::Minimum if partly => "stamp-minimum-partial",
        Level::Minimum => "stamp-minimum",
        Level::Below => "stamp-below",
        Level::Blocked => "stamp-blocked",
        Level::Unknown => "stamp-unknown",
    };
    (i18n.t(id), level_color(level))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    fn verdict(overall: Level, confidence: Confidence) -> Status {
        Status::Verdict(Verdict {
            overall,
            bottleneck: None,
            components: Vec::new(),
            confidence,
            reported: Vec::new(),
            blocker: None,
            warnings: Vec::new(),
        })
    }

    #[test]
    fn ready_and_low_ask_unless_cpu_gpu_and_ram_were_all_compared() {
        let i18n = I18n::new(Language::English);
        let label = |overall, confidence| stamp(&verdict(overall, confidence), &i18n).0;

        assert_eq!(label(Level::Recommended, Confidence::High), "READY");
        assert_eq!(label(Level::Recommended, Confidence::Partial), "READY?");
        assert_eq!(label(Level::Recommended, Confidence::Low), "READY?");
        assert_eq!(label(Level::Minimum, Confidence::High), "LOW");
        assert_eq!(label(Level::Minimum, Confidence::Partial), "LOW?");
        // One part too weak is enough to say so.
        assert_eq!(label(Level::Below, Confidence::Partial), "TOO WEAK");
    }

    #[test]
    fn partly_compared_verdicts_list_after_their_level() {
        let order = [
            verdict(Level::Recommended, Confidence::High),
            verdict(Level::Recommended, Confidence::Partial),
            verdict(Level::Minimum, Confidence::High),
            verdict(Level::Minimum, Confidence::Partial),
            verdict(Level::Below, Confidence::High),
        ];

        assert!(order.windows(2).all(|pair| rank(&pair[0]) < rank(&pair[1])));
    }
}
