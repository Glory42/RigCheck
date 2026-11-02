//! Tests for ingesting one game: a real `appdetails` response from Steam
//! (data/fixtures/appdetails) → a game stored in games.db.

use rigcheck_core::Catalog;
use rigcheck_gamedb::GameDb;
use rigcheck_pipeline::ingest::{Ingested, ingest};
use tempfile::TempDir;

fn fixture(app_id: u32) -> String {
    let path = format!(
        "{}/../data/fixtures/appdetails/{app_id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

fn catalog() -> Catalog {
    Catalog::from_toml(include_str!("../../data/hardware.toml")).unwrap()
}

/// A fresh games.db in a temp dir, with the hardware table.
fn database(dir: &TempDir) -> GameDb {
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    db.store_catalog(&catalog()).unwrap();
    db
}

#[test]
fn stores_a_native_linux_game_with_its_requirements() {
    let dir = TempDir::new().unwrap();
    let db = database(&dir);

    let Ingested::Game(record) = ingest(413150, &fixture(413150)).unwrap() else {
        panic!("Stardew Valley is a game");
    };
    db.store(&record, &catalog()).unwrap();

    let game = db.game(413150).unwrap().expect("stored");
    assert_eq!(game.name, "Stardew Valley");
    assert!(game.linux.native);
    let minimum = game.minimum.expect("a minimum tier");
    assert_eq!(minimum.ram_mb, Some(2048));
    assert_eq!(minimum.storage_mb, Some(500));
}

#[test]
fn dlc_and_hardware_are_not_games() {
    // Cyberpunk 2077: Phantom Liberty; the Steam Deck's store page, whose
    // pc_requirements is `[]` instead of an object.
    for (app_id, kind) in [(2138330, "dlc"), (1675200, "hardware")] {
        assert_eq!(
            ingest(app_id, &fixture(app_id)).unwrap(),
            Ingested::NotAGame {
                kind: kind.to_owned()
            }
        );
    }
}

#[test]
fn an_app_the_store_has_no_page_for_is_unavailable() {
    // Steamworks Common Redistributables: {"228980": {"success": false}}
    assert_eq!(
        ingest(228980, &fixture(228980)).unwrap(),
        Ingested::Unavailable
    );
}

#[test]
fn keeps_steams_requirement_text_for_the_game_page() {
    let dir = TempDir::new().unwrap();
    let db = database(&dir);
    let Ingested::Game(record) = ingest(413150, &fixture(413150)).unwrap() else {
        panic!("Stardew Valley is a game");
    };

    db.store(&record, &catalog()).unwrap();

    let minimum = db.game(413150).unwrap().unwrap().minimum.unwrap();
    assert_eq!(
        minimum.text.as_deref(),
        Some(
            "Minimum:\n\
             OS *: Windows Vista or greater\n\
             Processor: 2 Ghz\n\
             Memory: 2 GB RAM\n\
             Graphics: 256 mb video memory, shader model 3.0+\n\
             DirectX: Version 10\n\
             Storage: 500 MB available space"
        )
    );
}
