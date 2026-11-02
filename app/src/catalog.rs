//! The Catalog page: every game in games.db, most popular first, searchable.
//! A grid of game tiles like the Library's, but virtual: only the rows in
//! view are fetched and drawn, and only their covers downloaded.

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;

use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Element, Fill};
use rigcheck_core::{ProtonTier, Specs};
use rigcheck_gamedb::{Filters, Game, GameDb};

use crate::Message;
use crate::i18n::I18n;
use crate::library::{TILE_HEIGHT, TILE_WIDTH, game_tile, status};
use crate::style::{BOLD, Colors};

/// The search box's and the list's widget IDs.
pub const SEARCH: &str = "catalog-search";
pub const LIST: &str = "catalog-list";

/// Space between tiles, across and down.
pub const GAP: f32 = 18.0;
/// A row of tiles and the gap under it, so a scroll position maps straight
/// to rows.
pub const ROW_HEIGHT: f32 = TILE_HEIGHT + GAP;

/// A filter that's on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip {
    /// Only games this PC (with any override) reaches the minimum for.
    CanRun,
    NativeLinux,
    DeckVerified,
    NoAntiCheatIssues,
}

impl Chip {
    const ALL: [Chip; 4] = [
        Chip::CanRun,
        Chip::NativeLinux,
        Chip::DeckVerified,
        Chip::NoAntiCheatIssues,
    ];

    fn label_id(self) -> &'static str {
        match self {
            Chip::CanRun => "filter-can-run",
            Chip::NativeLinux => "filter-native-linux",
            Chip::DeckVerified => "filter-deck-verified",
            Chip::NoAntiCheatIssues => "filter-no-anticheat-issues",
        }
    }

    fn is_on(self, list: &List) -> bool {
        match self {
            Chip::CanRun => list.can_run,
            Chip::NativeLinux => list.filters.native_linux,
            Chip::DeckVerified => list.filters.deck_verified,
            Chip::NoAntiCheatIssues => list.filters.no_anticheat_issues,
        }
    }
}

/// The list's state: what's searched, where it's scrolled, and the rows in
/// view (fetched from games.db as they come into view).
pub struct List {
    query: String,
    filters: Filters,
    /// Whether CAN RUN is on. `filters.runs_on` holds the specs it uses
    /// once hardware detection has finished.
    can_run: bool,
    scroll: f32,
    viewport: f32,
    /// The grid's width, for how many tiles fit in a row.
    width: f32,
    total: usize,
    /// The index of `rows[0]` in the whole list.
    first: usize,
    /// The tile the keyboard has highlighted…
    selected: usize,
    /// …shown once the keyboard has moved it, not before.
    keyboard: bool,
    rows: Vec<Game>,
}

impl Default for List {
    fn default() -> List {
        List {
            query: String::new(),
            filters: Filters::default(),
            can_run: false,
            scroll: 0.0,
            // Until the list reports its size: enough for a tall window.
            viewport: 1200.0,
            width: 1060.0,
            total: 0,
            first: 0,
            selected: 0,
            keyboard: false,
            rows: Vec::new(),
        }
    }
}

impl List {
    /// Fetches the rows that scrolled into view. The count stays: it
    /// only changes with the search (and costs ~30 ms on a broad one).
    pub fn scrolled(&mut self, offset: f32, height: f32, width: f32, db: &GameDb) {
        self.scroll = offset;
        self.viewport = height;
        self.width = width;
        self.fetch_rows(db);
    }

    /// Lists `query`'s matches from the top. The caller scrolls the list
    /// widget back up too ([`LIST`]), as for every change of what's listed.
    pub fn search(&mut self, query: String, db: &GameDb) {
        self.query = query;
        self.restart(db);
    }

    /// Turns `chip` on or off. `specs` are this PC's, for CAN RUN.
    pub fn toggle(&mut self, chip: Chip, specs: Option<Specs>, db: &GameDb) {
        let on = !chip.is_on(self);
        match chip {
            Chip::CanRun => {
                self.can_run = on;
                self.filters.runs_on = specs.filter(|_| on);
            }
            Chip::NativeLinux => self.filters.native_linux = on,
            Chip::DeckVerified => self.filters.deck_verified = on,
            Chip::NoAntiCheatIssues => self.filters.no_anticheat_issues = on,
        }
        self.restart(db);
    }

    /// Keeps games ProtonDB rates `lowest` or better; `None` keeps all.
    pub fn protondb(&mut self, lowest: Option<ProtonTier>, db: &GameDb) {
        self.filters.protondb_at_least = lowest;
        self.restart(db);
    }

    /// Uses new specs (detection finished, or an override changed) for
    /// CAN RUN, if it's on.
    pub fn use_specs(&mut self, specs: Option<Specs>, db: &GameDb) {
        if self.can_run && self.filters.runs_on != specs {
            self.filters.runs_on = specs;
            self.restart(db);
        }
    }

    /// Lists from the first row again, after what's listed changed.
    fn restart(&mut self, db: &GameDb) {
        self.scroll = 0.0;
        self.selected = 0;
        self.keyboard = false;
        self.refresh(db);
    }

    /// Counts the matches and fetches the rows in view, e.g. for a new games.db.
    pub fn refresh(&mut self, db: &GameDb) {
        self.total = db.games_count(&self.query, &self.filters).unwrap_or(0);
        self.rows.clear();
        self.fetch_rows(db);
    }

    /// Tiles in a row.
    pub fn columns(&self) -> usize {
        columns(self.width)
    }

    /// Whole rows of tiles the window shows; at least one.
    pub fn rows_in_view(&self) -> usize {
        ((self.viewport / ROW_HEIGHT).floor() as usize).max(1)
    }

    /// The games in view, for their covers.
    pub fn visible_ids(&self) -> Vec<u32> {
        self.rows.iter().map(|game| game.app_id).collect()
    }

    /// Moves the highlight `by` tiles and scrolls its row into view.
    /// Returns the new scroll offset, for the caller to move the grid too.
    pub fn move_selection(&mut self, by: i64, db: &GameDb) -> f32 {
        if self.total == 0 {
            return self.scroll;
        }
        let last = self.total - 1;
        self.selected = (self.selected as i64 + by).clamp(0, last as i64) as usize;
        self.keyboard = true;
        let top = (self.selected / self.columns()) as f32 * ROW_HEIGHT;
        if top < self.scroll {
            self.scroll = top;
        } else if top + ROW_HEIGHT > self.scroll + self.viewport {
            self.scroll = top + ROW_HEIGHT - self.viewport;
        }
        self.fetch_rows(db);
        self.scroll
    }

    /// The highlighted game's app ID.
    pub fn selected_game(&self, db: &GameDb) -> Option<u32> {
        match self.selected.checked_sub(self.first) {
            Some(index) if index < self.rows.len() => Some(self.rows[index].app_id),
            _ => db
                .games_page(&self.query, &self.filters, self.selected, 1)
                .ok()?
                .first()
                .map(|(app_id, _)| *app_id),
        }
    }

    fn fetch_rows(&mut self, db: &GameDb) {
        let range = visible_tiles(self.scroll, self.viewport, self.width, self.total);
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
    covers: &HashMap<u32, PathBuf>,
    i18n: &I18n,
    colors: Colors,
) -> Element<'a, Message> {
    // Whole rows of tiles, with spacers standing in for the rows above and
    // below the window.
    let columns = list.columns();
    let first_row = list.first / columns;
    let shown_rows = list.rows.len().div_ceil(columns);
    let rows_after = list
        .total
        .div_ceil(columns)
        .saturating_sub(first_row + shown_rows);
    let tiles = list
        .rows
        .chunks(columns)
        .enumerate()
        .map(|(row_index, games)| {
            let tiles = games.iter().enumerate().map(|(index, game)| {
                let position = list.first + row_index * columns + index;
                game_tile(
                    game.app_id,
                    &game.name,
                    &status(Some(game), specs),
                    covers.get(&game.app_id),
                    list.keyboard && position == list.selected,
                    i18n,
                    colors,
                )
            });
            container(row(tiles).spacing(GAP)).height(ROW_HEIGHT).into()
        });
    let search_box = text_input(&i18n.t("catalog-search"), &list.query)
        .id(SEARCH)
        .on_input(Message::CatalogSearch)
        // Enter opens the highlighted row (the first, after a new search).
        .on_submit(Message::Shortcut(crate::keys::Shortcut::Open))
        .padding(10)
        .font(BOLD);
    let list_view = scrollable(
        column![space().height(first_row as f32 * ROW_HEIGHT)]
            .extend(tiles)
            .push(space().height(rows_after as f32 * ROW_HEIGHT))
            .width(Fill),
    )
    .on_scroll(|viewport| Message::CatalogScrolled {
        offset: viewport.absolute_offset().y,
        height: viewport.bounds().height,
        width: viewport.bounds().width,
    })
    .id(LIST)
    .height(Fill);
    let chips = Chip::ALL.map(|chip| {
        button(text(i18n.t(chip.label_id())).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(chip.is_on(list)))
            // CAN RUN needs the hardware first.
            .on_press_maybe(
                (chip != Chip::CanRun || specs.is_some()).then_some(Message::CatalogFilter(chip)),
            )
            .into()
    });
    let tier_button = |lowest: Option<ProtonTier>| {
        let label = match lowest {
            None => i18n.t("filter-protondb-any"),
            // ProtonDB's own tier names, as on the game page.
            Some(ProtonTier::Platinum) => "PLATINUM".to_owned(),
            Some(tier) => format!("{}+", format!("{tier:?}").to_uppercase()),
        };
        button(text(label).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(list.filters.protondb_at_least == lowest))
            .on_press(Message::CatalogProtonDb(lowest))
            .into()
    };
    let tiers = row![text(i18n.t("game-protondb")).font(BOLD).size(12)]
        .extend(
            [
                None,
                Some(ProtonTier::Platinum),
                Some(ProtonTier::Gold),
                Some(ProtonTier::Silver),
                Some(ProtonTier::Bronze),
            ]
            .map(tier_button),
        )
        .spacing(8)
        .align_y(iced::Center);
    column![search_box, row(chips).spacing(8).wrap(), tiers, list_view]
        .spacing(16)
        .into()
}

/// The rows at least partly visible when the list is scrolled `scroll`
/// pixels down a `viewport` that tall, with `total` rows of `row_height`.
/// How many tiles fit side by side in `width` pixels; at least one.
pub fn columns(width: f32) -> usize {
    (((width + GAP) / (TILE_WIDTH + GAP)).floor() as usize).max(1)
}

/// The tiles (by index in the whole list) in rows at least partly visible
/// when the grid is scrolled `scroll` pixels down a `viewport` that tall and
/// `width` wide, with `total` tiles.
pub fn visible_tiles(scroll: f32, viewport: f32, width: f32, total: usize) -> Range<usize> {
    let columns = columns(width);
    let rows = visible_rows(scroll, viewport, ROW_HEIGHT, total.div_ceil(columns));
    (rows.start * columns).min(total)..(rows.end * columns).min(total)
}

pub fn visible_rows(scroll: f32, viewport: f32, row_height: f32, total: usize) -> Range<usize> {
    let first = (scroll.max(0.0) / row_height).floor() as usize;
    let end = ((scroll.max(0.0) + viewport) / row_height).ceil() as usize;
    first.min(total)..end.min(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::library::TILE_WIDTH;

    /// Exactly `n` tiles and the gaps between them.
    fn width_for(n: usize) -> f32 {
        n as f32 * TILE_WIDTH + (n - 1) as f32 * GAP
    }

    #[test]
    fn as_many_columns_as_whole_tiles_fit() {
        assert_eq!(columns(width_for(3)), 3);
        assert_eq!(columns(width_for(3) - 1.0), 2);
        assert_eq!(columns(10.0), 1, "a tiny window still shows a column");
    }

    #[test]
    fn the_tiles_in_view_are_whole_rows_of_them() {
        let width = width_for(3);
        assert_eq!(visible_tiles(0.0, ROW_HEIGHT, width, 100), 0..3);
        assert_eq!(visible_tiles(ROW_HEIGHT, ROW_HEIGHT, width, 100), 3..6);
        // Five games: the second row has two.
        assert_eq!(visible_tiles(ROW_HEIGHT, ROW_HEIGHT, width, 5), 3..5);
    }

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
