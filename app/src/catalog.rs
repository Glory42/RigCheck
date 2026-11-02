//! The Catalog page: every game in games.db, most popular first, searchable.
//! The list is virtual: only the rows in view are fetched and drawn.

use std::ops::Range;

use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Element, Fill};
use rigcheck_core::Specs;
use rigcheck_gamedb::{Game, GameDb};

use crate::i18n::I18n;
use crate::library::{Status, stamp, status};
use crate::style::{BOLD, Colors};
use crate::{Message, Page};

/// Every row is this tall, so a scroll position maps straight to rows.
pub const ROW_HEIGHT: f32 = 48.0;

/// The list's state: what's searched, where it's scrolled, and the rows in
/// view (fetched from games.db as they come into view).
pub struct List {
    query: String,
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
    pub fn scrolled(&mut self, offset: f32, height: f32, db: &GameDb) {
        self.scroll = offset;
        self.viewport = height;
        self.refresh(db);
    }

    /// Fetches the rows now in view.
    pub fn refresh(&mut self, db: &GameDb) {
        self.total = db.games_count(&self.query).unwrap_or(0);
        let range = visible_rows(self.scroll, self.viewport, ROW_HEIGHT, self.total);
        let page = db
            .games_page(&self.query, range.start, range.len())
            .unwrap_or_default();
        self.first = range.start;
        self.rows = page
            .into_iter()
            .filter_map(|(app_id, _)| db.game(app_id).ok().flatten())
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
    scrollable(
        column![space().height(above)]
            .extend(rows)
            .push(space().height(below))
            .width(Fill),
    )
    .on_scroll(|viewport| Message::CatalogScrolled {
        offset: viewport.absolute_offset().y,
        height: viewport.bounds().height,
    })
    .height(Fill)
    .into()
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
