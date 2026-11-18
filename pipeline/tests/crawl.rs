//! Tests for crawl planning: which apps to fetch next.
//!
//! ROADMAP Phase 4: popular games first, then new releases, then the rest;
//! resumable across runs. §9: never re-crawl a game whose store page hasn't
//! changed. A changed page waits out a cooldown (a week for games people play
//! on Linux, a month for the rest), so pages that change every day don't eat
//! the budget that games never fetched need.

use std::collections::HashMap;

use rigcheck_core::{Catalog, ParsedTier};
use rigcheck_gamedb::{GameDb, GameRecord};
use rigcheck_pipeline::applist::App;
use rigcheck_pipeline::crawl::plan;

const DAY: i64 = 24 * 60 * 60;
use tempfile::TempDir;

/// An app-list entry whose store page last changed at `last_modified`.
fn app(app_id: u32, last_modified: i64) -> App {
    App {
        app_id,
        name: format!("Game {app_id}"),
        last_modified,
    }
}

#[test]
fn popular_games_come_first_then_the_newest() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    let apps = [
        app(1, 100),
        app(2, 500),
        app(3, 300),
        app(4, 200),
        app(5, 400),
    ];
    // ProtonDB report counts.
    let popularity = HashMap::from([(4, 900), (1, 50)]);

    let next = plan(&apps, &db, &popularity, 4, 1000).unwrap();

    assert_eq!(next, [4, 1, 2, 5]);
}

#[test]
fn a_store_page_is_fetched_again_only_after_it_changes() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    let apps = [
        app(1, 100),
        app(2, 500),
        app(3, 300),
        app(4, 200),
        app(5, 400),
    ];
    db.record_fetch(2, 600).unwrap(); // fetched after its last change: skip
    db.record_fetch(3, 250).unwrap(); // changed at 300, after the fetch: again

    let next = plan(&apps, &db, &HashMap::new(), 10, 250 + 31 * DAY).unwrap();

    // Never fetched first (newest change first), then the changed one.
    assert_eq!(next, [5, 4, 1, 3]);
}

#[test]
fn each_run_continues_where_the_last_one_stopped() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    let apps = [
        app(1, 100),
        app(2, 500),
        app(3, 300),
        app(4, 200),
        app(5, 400),
    ];
    let popularity = HashMap::from([(4, 900), (1, 50)]);

    let mut runs = Vec::new();
    for run_at in [1000, 2000, 3000, 4000] {
        let batch = plan(&apps, &db, &popularity, 2, run_at).unwrap();
        for &app_id in &batch {
            db.record_fetch(app_id, run_at).unwrap();
        }
        runs.push(batch);
    }

    assert_eq!(runs, [vec![4, 1], vec![2, 5], vec![3], vec![]]);
}

#[test]
fn a_game_stored_without_its_requirement_text_is_fetched_again() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    let tier = || {
        Some(ParsedTier {
            ram_mb: Some(2048),
            ..ParsedTier::default()
        })
    };
    // Crawled before the text was kept.
    let old = GameRecord {
        app_id: 1,
        name: "Old".to_owned(),
        minimum: tier(),
        ..GameRecord::default()
    };
    let new = GameRecord {
        app_id: 2,
        name: "New".to_owned(),
        minimum: tier(),
        minimum_text: Some("Memory: 2 GB RAM".to_owned()),
        ..GameRecord::default()
    };
    for record in [&old, &new] {
        db.store(record, &Catalog::default()).unwrap();
        db.record_fetch(record.app_id, 600).unwrap(); // after the last change
    }

    let next = plan(&[app(1, 100), app(2, 100)], &db, &HashMap::new(), 10, 700).unwrap();

    assert_eq!(next, [1]);
}

#[test]
fn a_changed_store_page_waits_a_month_before_it_is_fetched_again() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    let fetched_at = 1_000 * DAY;
    db.record_fetch(1, fetched_at).unwrap();
    let apps = [app(1, fetched_at + DAY)]; // changed the day after

    let next_day = plan(&apps, &db, &HashMap::new(), 10, fetched_at + 2 * DAY).unwrap();
    let a_month_on = plan(&apps, &db, &HashMap::new(), 10, fetched_at + 30 * DAY).unwrap();

    assert_eq!(next_day, Vec::<u32>::new());
    assert_eq!(a_month_on, [1]);
}

#[test]
fn a_game_people_play_on_linux_waits_only_a_week() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    let fetched_at = 1_000 * DAY;
    db.record_fetch(1, fetched_at).unwrap();
    db.record_fetch(2, fetched_at).unwrap();
    let apps = [app(1, fetched_at + DAY), app(2, fetched_at + DAY)];
    let popularity = HashMap::from([(1, 30)]); // ProtonDB reports for 1 only

    let next = plan(&apps, &db, &popularity, 10, fetched_at + 7 * DAY).unwrap();

    assert_eq!(next, [1]);
}

#[test]
fn games_never_fetched_come_before_changed_ones() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    db.record_fetch(1, 0).unwrap();
    // 1 changed most recently, but 2 and 3 have never been fetched.
    let apps = [app(1, 900), app(2, 100), app(3, 200)];

    let next = plan(&apps, &db, &HashMap::new(), 2, 60 * DAY).unwrap();

    assert_eq!(next, [3, 2]);
}
