//! Keyboard shortcuts (ROADMAP Phase 8). iced's buttons can't take focus,
//! so keys stand in: tabs, the search box, going back, and moving through
//! the Catalog.

use iced::keyboard::{Key, Modifiers, key::Named};

/// What a key press asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    /// Ctrl+1…5: Library, Catalog, My Rig, Settings, About.
    Tab(u8),
    /// Ctrl+F: the page's search box.
    Search,
    /// Escape: back from a game page or a comparison.
    Back,
    /// Arrows and Page Up/Down: move the Catalog's highlighted row.
    Move(i32),
    /// Enter: open the highlighted row.
    Open,
}

/// The shortcut for a key no widget used, if it's one.
pub fn shortcut(key: &Key, modifiers: Modifiers) -> Option<Shortcut> {
    match key {
        Key::Named(Named::Escape) => Some(Shortcut::Back),
        Key::Named(Named::ArrowDown) => Some(Shortcut::Move(1)),
        Key::Named(Named::ArrowUp) => Some(Shortcut::Move(-1)),
        Key::Named(Named::PageDown) => Some(Shortcut::Move(PAGE)),
        Key::Named(Named::PageUp) => Some(Shortcut::Move(-PAGE)),
        Key::Named(Named::Enter) => Some(Shortcut::Open),
        Key::Character(c) if modifiers.command() => match c.as_str() {
            "f" => Some(Shortcut::Search),
            tab @ ("1" | "2" | "3" | "4" | "5") => tab.parse().ok().map(Shortcut::Tab),
            _ => None,
        },
        _ => None,
    }
}

/// Rows Page Up/Down move.
const PAGE: i32 = 10;

#[cfg(test)]
mod tests {
    use super::*;

    fn character(c: &str) -> Key {
        Key::Character(c.into())
    }

    #[test]
    fn ctrl_and_a_number_switch_tabs() {
        assert_eq!(
            shortcut(&character("1"), Modifiers::CTRL),
            Some(Shortcut::Tab(1))
        );
        assert_eq!(
            shortcut(&character("5"), Modifiers::CTRL),
            Some(Shortcut::Tab(5))
        );
        assert_eq!(shortcut(&character("6"), Modifiers::CTRL), None);
        // A plain number is just a number.
        assert_eq!(shortcut(&character("2"), Modifiers::empty()), None);
    }

    #[test]
    fn ctrl_f_searches_and_escape_goes_back() {
        assert_eq!(
            shortcut(&character("f"), Modifiers::CTRL),
            Some(Shortcut::Search)
        );
        assert_eq!(shortcut(&character("f"), Modifiers::empty()), None);
        assert_eq!(
            shortcut(&Key::Named(Named::Escape), Modifiers::empty()),
            Some(Shortcut::Back)
        );
    }

    #[test]
    fn arrows_and_pages_move_and_enter_opens() {
        let named = |key| shortcut(&Key::Named(key), Modifiers::empty());
        assert_eq!(named(Named::ArrowDown), Some(Shortcut::Move(1)));
        assert_eq!(named(Named::ArrowUp), Some(Shortcut::Move(-1)));
        assert_eq!(named(Named::PageDown), Some(Shortcut::Move(10)));
        assert_eq!(named(Named::PageUp), Some(Shortcut::Move(-10)));
        assert_eq!(named(Named::Enter), Some(Shortcut::Open));
    }
}
