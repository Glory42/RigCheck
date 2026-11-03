//! The Catalog page's queries: every game in games.db, most popular first,
//! a page at a time, narrowed by a search.

use rigcheck_gamedb::{Filters, GameDb};
use tempfile::TempDir;

/// Five games with ProtonDB report counts (their popularity), like a
/// pipeline build before any store page is crawled.
fn db() -> (TempDir, GameDb) {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    db.store_names(&[
        (400, "Portal"),
        (620, "Portal 2"),
        (70, "Half-Life"),
        (220, "Half-Life 2"),
        (2012840, "Portal with RTX"),
    ])
    .unwrap();
    db.set_protondb(&[
        (400, None, 900),
        (620, None, 1500),
        (70, None, 300),
        (220, None, 900),
        (2012840, None, 40),
    ])
    .unwrap();
    (dir, db)
}

fn names(page: Vec<(u32, String)>) -> Vec<String> {
    page.into_iter().map(|(_, name)| name).collect()
}

#[test]
fn without_a_search_every_game_is_listed_most_popular_first() {
    let (_dir, db) = db();

    let page = db.games_page("", &Filters::default(), 0, 10).unwrap();

    // Half-Life 2 and Portal tie at 900 reports: by name.
    assert_eq!(
        names(page),
        [
            "Portal 2",
            "Half-Life 2",
            "Portal",
            "Half-Life",
            "Portal with RTX"
        ]
    );
    assert_eq!(db.games_count("", &Filters::default()).unwrap(), 5);
}

#[test]
fn a_search_lists_matching_games_a_page_at_a_time() {
    let (_dir, db) = db();

    let first = db.games_page("port", &Filters::default(), 0, 2).unwrap();
    let second = db.games_page("port", &Filters::default(), 2, 2).unwrap();

    assert_eq!(names(first), ["Portal 2", "Portal"]);
    assert_eq!(names(second), ["Portal with RTX"]);
    assert_eq!(db.games_count("port", &Filters::default()).unwrap(), 3);
    assert_eq!(db.games_count("half 2", &Filters::default()).unwrap(), 1);
}

#[test]
fn the_search_follows_names_as_they_are_written_and_changed() {
    let (_dir, db) = db();
    db.store_names(&[(1145360, "Hades")]).unwrap();
    let search = |query: &str| names(db.games_page(query, &Filters::default(), 0, 10).unwrap());
    assert_eq!(search("hades"), ["Hades"]);

    let renamed = rigcheck_gamedb::GameRecord {
        app_id: 1145360,
        name: "Hades II".to_owned(),
        ..Default::default()
    };
    db.store(&renamed, &rigcheck_core::Catalog::default())
        .unwrap();

    assert_eq!(search("hades"), ["Hades II"]);
    assert_eq!(search("ii"), ["Hades II"]);
}
