//! Tests for `build`: merging ProtonDB and AreWeAntiCheatYet into the crawled
//! games and making them searchable by name.

use std::collections::HashMap;

use rigcheck_core::{AntiCheat, Catalog, CatalogEntry, ParsedTier, ProtonTier, Vendor};
use rigcheck_gamedb::{GameDb, GameRecord};
use rigcheck_pipeline::applist::App;
use rigcheck_pipeline::awacy::AntiCheatInfo;
use rigcheck_pipeline::blender::GapScores;
use rigcheck_pipeline::build::{Sources, build};
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

    build(
        &db,
        &Sources {
            protondb,
            ..Sources::default()
        },
    )
    .unwrap();

    let found: Vec<u32> = db
        .games_page("witcher", &Default::default(), 0, 10)
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

    build(
        &db,
        &Sources {
            protondb,
            anticheat,
            ..Sources::default()
        },
    )
    .unwrap();

    let destiny = db.game(1085660).unwrap().unwrap();
    assert_eq!(destiny.linux.protondb, Some(ProtonTier::Borked));
    assert_eq!(destiny.linux.anticheat, Some(AntiCheat::Denied));
    assert_eq!(destiny.anticheat_name.as_deref(), Some("BattlEye"));
}

#[test]
fn the_built_database_names_its_license_and_sources() {
    let dir = TempDir::new().unwrap();
    let db = crawled(&dir);

    build(&db, &Sources::default()).unwrap();

    // ROADMAP §1.2: games.db includes ProtonDB data (ODbL, share-alike).
    let license = db.meta("license").unwrap().expect("a license notice");
    assert!(license.contains("ODbL"), "{license}");
    let sources = db.meta("sources").unwrap().expect("source credits");
    for source in ["Steam", "ProtonDB", "AreWeAntiCheatYet"] {
        assert!(sources.contains(source), "{source} missing from: {sources}");
    }
}

#[test]
fn every_game_in_the_app_list_is_searchable_before_its_crawled() {
    let dir = TempDir::new().unwrap();
    let db = crawled(&dir);
    let witcher = GameRecord {
        app_id: 292030,
        name: "The Witcher 3: Wild Hunt".to_owned(),
        minimum: Some(ParsedTier {
            ram_mb: Some(6144),
            ..ParsedTier::default()
        }),
        ..GameRecord::default()
    };
    db.store(&witcher, &catalog()).unwrap();
    let apps = [
        App {
            app_id: 292030,
            name: "The Witcher 3: Wild Hunt".to_owned(),
            last_modified: 0,
        },
        App {
            app_id: 367520,
            name: "Hollow Knight".to_owned(),
            last_modified: 0,
        },
    ];

    build(
        &db,
        &Sources {
            apps: apps.to_vec(),
            ..Sources::default()
        },
    )
    .unwrap();

    let found: Vec<u32> = db
        .games_page("hollow", &Default::default(), 0, 10)
        .unwrap()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(found, [367520], "not crawled yet, but findable");
    let witcher = db.game(292030).unwrap().unwrap();
    assert_eq!(
        witcher.minimum.unwrap().ram_mb,
        Some(6144),
        "crawled data is kept"
    );
}

#[test]
fn new_hardware_applies_to_games_crawled_before_it_existed() {
    let dir = TempDir::new().unwrap();
    let db = crawled(&dir);
    let game = GameRecord {
        app_id: 1,
        name: "A game".to_owned(),
        minimum: Some(ParsedTier {
            gpu: Some("NVIDIA GeForce RTX 3050 or Radeon RX 580".to_owned()),
            ..ParsedTier::default()
        }),
        ..GameRecord::default()
    };
    db.store(&game, &catalog()).unwrap(); // the table has no RTX 3050 yet
    let rtx_3050 = CatalogEntry {
        name: "NVIDIA GeForce RTX 3050".to_owned(),
        vendor: Vendor::Nvidia,
        score: 106,
        vram_mb: None,
        aliases: Vec::new(),
        pci: Vec::new(),
    };
    let sources = Sources {
        gaps: GapScores {
            cpus: Vec::new(),
            gpus: vec![rtx_3050],
        },
        ..Sources::default()
    };

    build(&db, &sources).unwrap();

    let minimum = db.game(1).unwrap().unwrap().minimum.unwrap();
    let names: Vec<&str> = minimum.gpus.iter().map(|gpu| gpu.name.as_str()).collect();
    assert_eq!(names, ["NVIDIA GeForce RTX 3050", "Radeon RX 580"]);
}

#[test]
fn games_keep_what_players_ran_them_on() {
    let dir = TempDir::new().unwrap();
    let db = crawled(&dir);
    let played = rigcheck_pipeline::protondb::Played {
        cpus: Vec::new(),
        gpus: ["GTX 750 Ti", "GTX 660", "GTX 970"]
            .map(String::from)
            .to_vec(),
    };

    build(
        &db,
        &Sources {
            played: HashMap::from([(1085660, played)]),
            ..Sources::default()
        },
    )
    .unwrap();

    let destiny = db.game(1085660).unwrap().unwrap();
    assert_eq!(
        destiny.linux.reported_gpu.map(|gpu| gpu.weakest_name),
        Some("GeForce GTX 660".to_owned())
    );
    assert_eq!(destiny.linux.reported_cpu, None);
}

#[test]
fn each_build_adds_blenders_parts_once() {
    let dir = TempDir::new().unwrap();
    let db = crawled(&dir);
    let part = CatalogEntry {
        name: "AMD A10-7800 Radeon R7, 12 Compute Cores 4C+8G".to_owned(),
        vendor: Vendor::Amd,
        score: 40,
        vram_mb: None,
        aliases: Vec::new(),
        pci: Vec::new(),
    };
    let sources = Sources {
        gaps: GapScores {
            cpus: vec![part],
            gpus: Vec::new(),
        },
        ..Sources::default()
    };
    let cpus = |db: &GameDb| db.catalog().unwrap().cpus().len();

    build(&db, &sources).unwrap();
    let once = cpus(&db);
    build(&db, &sources).unwrap();

    assert_eq!(cpus(&db), once);
}
