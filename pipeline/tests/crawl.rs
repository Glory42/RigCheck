//! Tests for crawl planning: which apps to fetch next.
//!
//! ROADMAP Phase 4: popular games first, then new releases, then the rest;
//! resumable across runs. §9: never re-crawl a game whose store page hasn't
//! changed.

use std::collections::HashMap;

use rigcheck_gamedb::GameDb;
use rigcheck_pipeline::applist::App;
use rigcheck_pipeline::crawl::plan;
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

    let next = plan(&apps, &db, &popularity, 4).unwrap();

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

    let next = plan(&apps, &db, &HashMap::new(), 10).unwrap();

    assert_eq!(next, [5, 3, 4, 1]);
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
        let batch = plan(&apps, &db, &popularity, 2).unwrap();
        for &app_id in &batch {
            db.record_fetch(app_id, run_at).unwrap();
        }
        runs.push(batch);
    }

    assert_eq!(runs, [vec![4, 1], vec![2, 5], vec![3], vec![]]);
}
