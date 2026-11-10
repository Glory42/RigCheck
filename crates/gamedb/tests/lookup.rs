//! Live lookups: a game games.db only knows by name, looked up on Steam and
//! kept in fetched.db, read back in ROADMAP §7.1's order (games.db →
//! fetched.db → live fetch).

use std::path::{Path, PathBuf};

use rigcheck_core::{Catalog, ProtonTier, Specs};
use rigcheck_gamedb::ingest::{Ingested, ingest};
use rigcheck_gamedb::{Fetched, Filters, GameDb, Records};
use rigcheck_hardware::{Commands, Rig, detect};
use tempfile::TempDir;

const BG3: u32 = 1086940;
const STARDEW: u32 = 413150;
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
    Records::new(&games, &fetched, NOW - 40 * DAY)
        .store_lookup(looked_up(STARDEW))
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

struct Captured(PathBuf);

impl Commands for Captured {
    fn run(&self, program: &str, _args: &[&str]) -> Option<String> {
        std::fs::read_to_string(self.0.join("commands").join(format!("{program}.txt"))).ok()
    }
}

/// The AMD laptop's specs, read against `games`' hardware table.
fn amd_laptop(games: &GameDb) -> Specs {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/hardware/amd-laptop");
    let hardware = detect(&root, &Captured(root.clone()));
    Rig::new(&games.catalog().unwrap(), &hardware)
        .specs()
        .clone()
}

/// Names in the Catalog for `filters`, checking the count agrees.
fn listed(records: &Records, filters: &Filters) -> Vec<String> {
    let page = records.page("", filters, 0, 100).unwrap();
    assert_eq!(records.count("", filters).unwrap(), page.len());
    page.into_iter().map(|(_, name)| name).collect()
}

#[test]
fn the_catalogs_filters_read_a_lookup_like_its_tile() {
    let dir = TempDir::new().unwrap();
    let (games, fetched) = databases(&dir);
    games.store_names(&[(STARDEW, "Stardew Valley")]).unwrap();
    let records = Records::new(&games, &fetched, NOW);
    let can_run = Filters {
        runs_on: Some(amd_laptop(&games)),
        ..Filters::default()
    };
    let native = Filters {
        native_linux: true,
        ..Filters::default()
    };
    // games.db knows both by name only: no filter keeps them.
    assert!(listed(&records, &can_run).is_empty());
    assert!(listed(&records, &native).is_empty());

    records.store_lookup(looked_up(STARDEW)).unwrap();
    records.store_lookup(looked_up(BG3)).unwrap();

    // Stardew Valley runs on the laptop and has a Linux build; Baldur's
    // Gate 3 is too much for its Radeon Vega 7.
    assert_eq!(listed(&records, &can_run), ["Stardew Valley"]);
    assert_eq!(listed(&records, &native), ["Stardew Valley"]);
    // Without filters, every game is listed either way.
    assert_eq!(listed(&records, &Filters::default()).len(), 2);
}

#[test]
fn a_stale_or_cleared_lookup_leaves_the_catalogs_filters() {
    let dir = TempDir::new().unwrap();
    let (games, mut fetched) = databases(&dir);
    games.store_names(&[(STARDEW, "Stardew Valley")]).unwrap();
    let native = Filters {
        native_linux: true,
        ..Filters::default()
    };
    let records = Records::new(&games, &fetched, NOW);
    records.store_lookup(looked_up(STARDEW)).unwrap();
    assert_eq!(listed(&records, &native), ["Stardew Valley"]);

    let later = Records::new(&games, &fetched, NOW + 31 * DAY);
    assert!(listed(&later, &native).is_empty());

    fetched.clear().unwrap();
    assert!(listed(&Records::new(&games, &fetched, NOW), &native).is_empty());
}

#[test]
fn a_lookup_keeps_what_players_ran_the_game_on() {
    let dir = TempDir::new().unwrap();
    let (games, fetched) = databases(&dir);
    let gpu = rigcheck_core::Reported {
        weakest_name: "Radeon RX 580".to_owned(),
        weakest: 90,
        typical: 210,
    };
    games
        .set_reported(&[(BG3, None, Some(gpu.clone()))])
        .unwrap();
    let records = Records::new(&games, &fetched, NOW);

    records.store_lookup(looked_up(BG3)).unwrap();

    assert_eq!(
        records.get(BG3).unwrap().unwrap().linux.reported_gpu,
        Some(gpu)
    );
}
