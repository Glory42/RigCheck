//! The Catalog page: every game in games.db, most popular first, searchable.
//! The list is virtual: only the rows in view are fetched and drawn.

use std::collections::HashMap;
use std::ops::Range;

use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Element, Fill};
use rigcheck_core::Specs;
use rigcheck_gamedb::{Filters, Game, GameDb};

use crate::i18n::I18n;
use crate::library::{Status, stamp, status};
use crate::style::{BOLD, Colors};
use crate::{Message, Page};

/// The search box's and the list's widget IDs.
pub const SEARCH: &str = "catalog-search";
pub const LIST: &str = "catalog-list";

/// Every row is this tall, so a scroll position maps straight to rows.
pub const ROW_HEIGHT: f32 = 48.0;

/// The list's state: what's searched, where it's scrolled, and the rows in
/// view (fetched from games.db as they come into view).
pub struct List {
    query: String,
    filters: Filters,
    scroll: f32,
    viewport: f32,
    total: usize,
    /// The index of `rows[0]` in the whole list.
    first: usize,
    rows: Vec<Game>,
}

impl Default for List {
    fn default() -> List {
        List {
            query: String::new(),
            filters: Filters::default(),
            scroll: 0.0,
            // Until the list reports its size: enough for a tall window.
            viewport: 1200.0,
            total: 0,
            first: 0,
            rows: Vec::new(),
        }
    }
}

impl List {
    /// Fetches the rows that scrolled into view. The count stays: it
    /// only changes with the search (and costs ~30 ms on a broad one).
    pub fn scrolled(&mut self, offset: f32, height: f32, db: &GameDb) {
        self.scroll = offset;
        self.viewport = height;
        self.fetch_rows(db);
    }

    /// Lists `query`'s matches from the top. The caller scrolls the list
    /// widget back up too ([`LIST`]).
    pub fn search(&mut self, query: String, db: &GameDb) {
        self.query = query;
        self.scroll = 0.0;
        self.refresh(db);
    }

    /// Counts the matches and fetches the rows in view, e.g. for a new games.db.
    pub fn refresh(&mut self, db: &GameDb) {
        self.total = db.games_count(&self.query, &self.filters).unwrap_or(0);
        self.rows.clear();
        self.fetch_rows(db);
    }

    fn fetch_rows(&mut self, db: &GameDb) {
        let range = visible_rows(self.scroll, self.viewport, ROW_HEIGHT, self.total);
        if range.start == self.first && range.len() == self.rows.len() && !self.rows.is_empty() {
            return; // Same rows as before.
        }
        let page = db
            .games_page(&self.query, &self.filters, range.start, range.len())
            .unwrap_or_default();
        self.first = range.start;
        // Scrolling a little keeps most rows: reuse them and only load the
        // records that came into view (a full record takes ~1.5 ms).
        let mut loaded: HashMap<u32, Game> = self
            .rows
            .drain(..)
            .map(|game| (game.app_id, game))
            .collect();
        self.rows = page
            .into_iter()
            .filter_map(|(app_id, _)| {
                loaded
                    .remove(&app_id)
                    .or_else(|| db.game(app_id).ok().flatten())
            })
            .collect();
    }
}

/// `specs` is `None` until hardware detection finishes.
pub fn view<'a>(
    list: &'a List,
    specs: Option<&Specs>,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let above = list.first as f32 * ROW_HEIGHT;
    let below = list.total.saturating_sub(list.first + list.rows.len()) as f32 * ROW_HEIGHT;
    let rows = list
        .rows
        .iter()
        .map(|game| game_row(game, &status(Some(game), specs), i18n, colors));
    let search_box = text_input(&i18n.t("catalog-search"), &list.query)
        .id(SEARCH)
        .on_input(Message::CatalogSearch)
        .padding(10)
        .font(BOLD);
    let list_view = scrollable(
        column![space().height(above)]
            .extend(rows)
            .push(space().height(below))
            .width(Fill),
    )
    .on_scroll(|viewport| Message::CatalogScrolled {
        offset: viewport.absolute_offset().y,
        height: viewport.bounds().height,
    })
    .id(LIST)
    .height(Fill);
    column![search_box, list_view].spacing(16).into()
}

fn game_row<'a>(
    game: &'a Game,
    status: &Status,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    let (label, color) = stamp(status, i18n);
    let stamp = container(text(label).font(BOLD).size(12))
        .padding([3, 8])
        .style(move |_| container::Style {
            text_color: Some(colors.line),
            ..container::Style::default().background(color)
        });
    button(
        row![
            text(game.name.as_str()).font(BOLD).size(14),
            space::horizontal(),
            stamp
        ]
        .align_y(iced::Center),
    )
    .height(ROW_HEIGHT)
    .width(Fill)
    .padding([0, 12])
    .style(colors.button(false))
    .on_press(Message::Open(Page::Game(game.app_id)))
    .into()
}

/// The rows at least partly visible when the list is scrolled `scroll`
/// pixels down a `viewport` that tall, with `total` rows of `row_height`.
pub fn visible_rows(scroll: f32, viewport: f32, row_height: f32, total: usize) -> Range<usize> {
    let first = (scroll.max(0.0) / row_height).floor() as usize;
    let end = ((scroll.max(0.0) + viewport) / row_height).ceil() as usize;
    first.min(total)..end.min(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_the_top_the_first_rows_that_fit_are_drawn() {
        // Rows at 0–40, 40–80 and 80–120 px; the third is cut off at 100.
        assert_eq!(visible_rows(0.0, 100.0, 40.0, 1000), 0..3);
    }

    #[test]
    fn scrolled_down_the_rows_in_view_are_drawn() {
        // 410–510 px: row 10 (400–440) to row 12 (480–520).
        assert_eq!(visible_rows(410.0, 100.0, 40.0, 1000), 10..13);
    }

    #[test]
    fn a_short_list_draws_only_the_rows_it_has() {
        assert_eq!(visible_rows(0.0, 400.0, 40.0, 5), 0..5);
        // Scrolled further than a narrowed search reaches: nothing to draw.
        assert_eq!(visible_rows(4000.0, 400.0, 40.0, 5), 5..5);
    }
}
