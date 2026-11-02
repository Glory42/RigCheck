//! Live lookups: a game games.db only knows by name, looked up on Steam and
//! kept in fetched.db (ROADMAP §7.1: games.db → fetched.db → live fetch).

use rigcheck_core::{Catalog, ProtonTier};
use rigcheck_gamedb::GameDb;
use rigcheck_gamedb::ingest::{Ingested, ingest};
use tempfile::TempDir;

const BG3: u32 = 1086940;
const NOW: i64 = 1_793_600_000; // 2026-11-02

fn saved_response(app_id: u32) -> String {
    let path = format!(
        "{}/../../data/fixtures/appdetails/{app_id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

/// games.db with Baldur's Gate 3 by name only, rated Platinum on ProtonDB;
/// and an empty fetched.db.
fn databases(dir: &TempDir) -> (GameDb, GameDb) {
    let games = GameDb::create(&dir.path().join("games.db")).unwrap();
    games
        .store_catalog(&Catalog::from_toml(include_str!("../../../data/hardware.toml")).unwrap())
        .unwrap();
    games.store_names(&[(BG3, "Baldur's Gate 3")]).unwrap();
    games
        .set_protondb(&[(BG3, Some(ProtonTier::Platinum), 900)])
        .unwrap();
    let fetched = GameDb::create(&dir.path().join("fetched.db")).unwrap();
    (games, fetched)
}

fn looked_up(app_id: u32) -> rigcheck_gamedb::GameRecord {
    match ingest(app_id, &saved_response(app_id)).unwrap() {
        Ingested::Game(record) => *record,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_lookup_keeps_what_games_db_already_knows() {
    let dir = TempDir::new().unwrap();
    let (games, fetched) = databases(&dir);

    fetched.store_lookup(looked_up(BG3), &games, NOW).unwrap();

    let game = fetched.lookup(BG3, NOW).unwrap().expect("just looked up");
    // From Steam's store page…
    assert_eq!(game.minimum.unwrap().ram_mb, Some(8192));
    // …and from games.db.
    assert_eq!(game.linux.protondb, Some(ProtonTier::Platinum));
    assert!(game.crawled);
}

#[test]
fn a_lookup_older_than_30_days_is_stale() {
    let dir = TempDir::new().unwrap();
    let (games, fetched) = databases(&dir);
    fetched.store_lookup(looked_up(BG3), &games, NOW).unwrap();
    let day = 24 * 60 * 60;

    assert!(fetched.lookup(BG3, NOW + 29 * day).unwrap().is_some());
    assert!(fetched.lookup(BG3, NOW + 31 * day).unwrap().is_none());
}
