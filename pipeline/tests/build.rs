//! Tests for `build`: merging ProtonDB and AreWeAntiCheatYet into the crawled
//! games and making them searchable by name.

use std::collections::HashMap;

use rigcheck_core::{AntiCheat, Catalog, ProtonTier};
use rigcheck_gamedb::{GameDb, GameRecord};
use rigcheck_pipeline::awacy::AntiCheatInfo;
use rigcheck_pipeline::build::build;
use rigcheck_pipeline::protondb::Summary;
use tempfile::TempDir;

fn catalog() -> Catalog {
    Catalog::from_toml(include_str!("../../data/hardware.toml")).unwrap()
}

/// A crawled games.db with three games and no Linux facts yet.
fn crawled(dir: &TempDir) -> GameDb {
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    db.store_catalog(&catalog()).unwrap();
    for (app_id, name) in [
        (292030, "The Witcher 3: Wild Hunt"),
        (20900, "The Witcher: Enhanced Edition"),
        (1085660, "Destiny 2"),
    ] {
        let record = GameRecord {
            app_id,
            name: name.to_owned(),
            ..GameRecord::default()
        };
        db.store(&record, &catalog()).unwrap();
    }
    db
}

#[test]
fn games_are_searchable_by_name_most_popular_first() {
    let dir = TempDir::new().unwrap();
    let db = crawled(&dir);
    let protondb = HashMap::from([
        (
            292030,
            Summary {
                tier: Some(ProtonTier::Platinum),
                reports: 3000,
            },
        ),
        (
            20900,
            Summary {
                tier: Some(ProtonTier::Gold),
                reports: 200,
            },
        ),
    ]);

    build(&db, &protondb, &HashMap::new()).unwrap();

    let found: Vec<u32> = db
        .search("witcher", 10)
        .unwrap()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(found, [292030, 20900]);
}

#[test]
fn games_get_their_proton_tier_and_anti_cheat_status() {
    let dir = TempDir::new().unwrap();
    let db = crawled(&dir);
    let protondb = HashMap::from([(
        1085660,
        Summary {
            tier: Some(ProtonTier::Borked),
            reports: 900,
        },
    )]);
    let anticheat = HashMap::from([(
        1085660,
        AntiCheatInfo {
            status: AntiCheat::Denied,
            name: "BattlEye".to_owned(),
        },
    )]);

    build(&db, &protondb, &anticheat).unwrap();

    let destiny = db.game(1085660).unwrap().unwrap();
    assert_eq!(destiny.linux.protondb, Some(ProtonTier::Borked));
    assert_eq!(destiny.linux.anticheat, Some(AntiCheat::Denied));
    assert_eq!(destiny.anticheat_name.as_deref(), Some("BattlEye"));
}
