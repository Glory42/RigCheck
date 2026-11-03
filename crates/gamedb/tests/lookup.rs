//! Live lookups: a game games.db only knows by name, looked up on Steam and
//! kept in fetched.db, read back in ROADMAP §7.1's order (games.db →
//! fetched.db → live fetch).

use rigcheck_core::{Catalog, ProtonTier};
use rigcheck_gamedb::ingest::{Ingested, ingest};
use rigcheck_gamedb::{Fetched, GameDb, Records};
use tempfile::TempDir;

const BG3: u32 = 1086940;
const NOW: i64 = 1_793_600_000; // 2026-11-02
const DAY: i64 = 24 * 60 * 60;

fn saved_response(app_id: u32) -> String {
    let path = format!(
        "{}/../../data/fixtures/appdetails/{app_id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

/// games.db with Baldur's Gate 3 by name only, rated Platinum on ProtonDB;
/// and an empty fetched.db.
fn databases(dir: &TempDir) -> (GameDb, Fetched) {
    let games = GameDb::create(&dir.path().join("games.db")).unwrap();
    games
        .store_catalog(&Catalog::from_toml(include_str!("../../../data/hardware.toml")).unwrap())
        .unwrap();
    games.store_names(&[(BG3, "Baldur's Gate 3")]).unwrap();
    games
        .set_protondb(&[(BG3, Some(ProtonTier::Platinum), 900)])
        .unwrap();
    let fetched = Fetched::open(&dir.path().join("cache/fetched.db")).unwrap();
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
    let records = Records::new(&games, &fetched, NOW);
    assert!(records.needs_lookup(BG3));

    records.store_lookup(looked_up(BG3)).unwrap();

    assert!(!records.needs_lookup(BG3));
    let game = records.get(BG3).unwrap().expect("just looked up");
    // From Steam's store page, matched with games.db's hardware table…
    let minimum = game.minimum.unwrap();
    assert_eq!(minimum.ram_mb, Some(8192));
    let gpus: Vec<&str> = minimum.gpus.iter().map(|gpu| gpu.name.as_str()).collect();
    // "Nvidia GTX 970 / RX 480 / Intel Arc A380 / Qualcomm Adreno X1": the
    // table has no Adreno.
    assert_eq!(gpus, ["GeForce GTX 970", "Radeon RX 480", "Arc A380"]);
    // …and from games.db.
    assert_eq!(game.linux.protondb, Some(ProtonTier::Platinum));
    assert!(game.crawled);
}

#[test]
fn a_lookup_older_than_30_days_is_asked_again() {
    let dir = TempDir::new().unwrap();
    let (games, fetched) = databases(&dir);
    Records::new(&games, &fetched, NOW)
        .store_lookup(looked_up(BG3))
        .unwrap();

    let soon = Records::new(&games, &fetched, NOW + 29 * DAY);
    assert!(soon.get(BG3).unwrap().unwrap().crawled);
    assert!(!soon.needs_lookup(BG3));

    let later = Records::new(&games, &fetched, NOW + 31 * DAY);
    let name_only = later.get(BG3).unwrap().expect("games.db's name");
    assert!(!name_only.crawled);
    assert!(later.needs_lookup(BG3));
}

#[test]
fn games_db_comes_first_once_it_has_checked_the_game() {
    let dir = TempDir::new().unwrap();
    let (games, fetched) = databases(&dir);
    let records = Records::new(&games, &fetched, NOW);
    records.store_lookup(looked_up(BG3)).unwrap();

    let mut crawled = looked_up(BG3);
    crawled.name = "Baldur's Gate 3 (games.db)".to_owned();
    games.store(&crawled, &games.catalog().unwrap()).unwrap();

    assert_eq!(
        records.get(BG3).unwrap().unwrap().name,
        "Baldur's Gate 3 (games.db)"
    );
}

#[test]
fn pruning_drops_stale_lookups_and_games_games_db_has_checked() {
    let dir = TempDir::new().unwrap();
    let (games, fetched) = databases(&dir);
    let stardew = 413150;
    Records::new(&games, &fetched, NOW - 40 * DAY)
        .store_lookup(looked_up(stardew))
        .unwrap();
    let records = Records::new(&games, &fetched, NOW);
    records.store_lookup(looked_up(BG3)).unwrap();
    assert_eq!(fetched.prune(&games, NOW).unwrap(), 1, "the stale one");

    games
        .store(&looked_up(BG3), &games.catalog().unwrap())
        .unwrap();
    assert_eq!(
        fetched.prune(&games, NOW).unwrap(),
        1,
        "games.db has it now"
    );
    assert_eq!(fetched.prune(&games, NOW).unwrap(), 0);
}

#[test]
fn clearing_forgets_every_lookup_and_keeps_working() {
    let dir = TempDir::new().unwrap();
    let (games, mut fetched) = databases(&dir);
    Records::new(&games, &fetched, NOW)
        .store_lookup(looked_up(BG3))
        .unwrap();

    fetched.clear().unwrap();

    let records = Records::new(&games, &fetched, NOW);
    assert!(records.needs_lookup(BG3));
    records.store_lookup(looked_up(BG3)).unwrap();
    assert!(!records.needs_lookup(BG3));
}

#[test]
fn a_lookup_lasts_the_session_without_a_cache_file() {
    let dir = TempDir::new().unwrap();
    let (games, _) = databases(&dir);
    let fetched = Fetched::in_memory();
    let records = Records::new(&games, &fetched, NOW);

    records.store_lookup(looked_up(BG3)).unwrap();

    assert!(records.get(BG3).unwrap().unwrap().crawled);
}
