//! Re-parsing stored requirement text: a better parser reaches every crawled
//! game at the next build, without fetching their store pages again.

use rigcheck_core::{Catalog, ParsedTier, requirement_text};
use rigcheck_gamedb::{GameDb, GameRecord};
use tempfile::TempDir;

const QUAKE: u32 = 2310;

/// Quake's minimum tier as its store page writes it (the requirement corpus).
fn quake_text() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/fixtures/requirements/corpus.json"
    );
    let corpus: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let quake = corpus
        .as_array()
        .unwrap()
        .iter()
        .find(|game| game["app_id"] == QUAKE)
        .unwrap();
    requirement_text(quake["minimum"].as_str().unwrap())
}

#[test]
fn reparsing_applies_a_better_parser_to_stored_text() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    let catalog = Catalog::from_toml(include_str!("../../../data/hardware.toml")).unwrap();
    db.store_catalog(&catalog).unwrap();
    // How an older parser left it: only the RAM, so "partial".
    let stale = GameRecord {
        app_id: QUAKE,
        name: "Quake".to_owned(),
        minimum: Some(ParsedTier {
            ram_mb: Some(8192),
            ..ParsedTier::default()
        }),
        minimum_text: Some(quake_text()),
        ..GameRecord::default()
    };
    db.store(&stale, &catalog).unwrap();
    assert_eq!(db.most_popular(1).unwrap(), [(QUAKE, "partial".to_owned())]);

    db.reparse().unwrap();
    db.rematch().unwrap();

    assert_eq!(db.most_popular(1).unwrap(), [(QUAKE, "parsed".to_owned())]);
    let minimum = db.game(QUAKE).unwrap().unwrap().minimum.unwrap();
    assert_eq!(
        minimum.cpu_text.as_deref(),
        Some("Intel Core i5-3570 @3.4 GHz or AMD Ryzen 3 1300X @3.5 GHz")
    );
}
