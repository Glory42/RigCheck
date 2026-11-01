//! Tests for `validate`: what stops a games.db from being published.

use rigcheck_core::{Catalog, ParsedTier, ProtonTier};
use rigcheck_gamedb::{GameDb, GameRecord};
use rigcheck_pipeline::validate::{Problem, Rules, validate};
use tempfile::TempDir;

fn catalog() -> Catalog {
    Catalog::from_toml(include_str!("../../data/hardware.toml")).unwrap()
}

/// A requirement tier with CPU, GPU and RAM: "parsed".
fn parsed() -> ParsedTier {
    ParsedTier {
        cpu: Some("Core i5-4460".to_owned()),
        gpu: Some("GeForce GTX 970".to_owned()),
        ram_mb: Some(8192),
        ..ParsedTier::default()
    }
}

/// A games.db with `games` as (app ID, popularity, parsed?).
fn database(dir: &TempDir, games: &[(u32, u32, bool)]) -> GameDb {
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    db.store_catalog(&catalog()).unwrap();
    let mut popularity = Vec::new();
    for &(app_id, reports, is_parsed) in games {
        let record = GameRecord {
            app_id,
            name: format!("Game {app_id}"),
            minimum: is_parsed.then(parsed),
            ..GameRecord::default()
        };
        db.store(&record, &catalog()).unwrap();
        popularity.push((app_id, Some(ProtonTier::Gold), reports));
    }
    db.set_protondb(&popularity).unwrap();
    db
}

const RULES: Rules = Rules {
    top: 10,
    min_parsed_share: 0.9,
    max_bytes: 50 * 1024 * 1024,
};

#[test]
fn a_database_whose_popular_games_are_parsed_passes() {
    let dir = TempDir::new().unwrap();
    // 9 of the top 10 parsed (90%); an unpopular game unparsed doesn't matter.
    let mut games: Vec<(u32, u32, bool)> = (1..=10).map(|id| (id, 1000 - id, id != 7)).collect();
    games.push((99, 1, false));
    let db = database(&dir, &games);

    assert_eq!(
        validate(&db, &dir.path().join("games.db"), &RULES).unwrap(),
        []
    );
}

#[test]
fn too_few_parsed_popular_games_fail() {
    let dir = TempDir::new().unwrap();
    // 8 of the top 10 parsed.
    let games: Vec<(u32, u32, bool)> = (1..=10).map(|id| (id, 1000 - id, id > 2)).collect();
    let db = database(&dir, &games);

    assert_eq!(
        validate(&db, &dir.path().join("games.db"), &RULES).unwrap(),
        [Problem::TooFewParsed { parsed: 8, top: 10 }]
    );
}

#[test]
fn a_database_over_the_size_limit_fails() {
    let dir = TempDir::new().unwrap();
    let games: Vec<(u32, u32, bool)> = (1..=10).map(|id| (id, 1000 - id, true)).collect();
    let db = database(&dir, &games);
    let path = dir.path().join("games.db");
    let bytes = std::fs::metadata(&path).unwrap().len();
    let rules = Rules {
        max_bytes: bytes - 1,
        ..RULES
    };

    assert_eq!(
        validate(&db, &path, &rules).unwrap(),
        [Problem::TooBig {
            bytes,
            max: bytes - 1
        }]
    );
}

#[test]
fn an_empty_database_fails() {
    let dir = TempDir::new().unwrap();
    let db = database(&dir, &[]);

    assert_eq!(
        validate(&db, &dir.path().join("games.db"), &RULES).unwrap(),
        [Problem::NoGames]
    );
}
