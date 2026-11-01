//! Tests for the unmatched-hardware report: CPU/GPU names the hardware table
//! doesn't know, by how many games mention them, to grow data/hardware.toml.

use rigcheck_core::{Catalog, ParsedTier};
use rigcheck_gamedb::{GameDb, GameRecord};
use rigcheck_pipeline::report::{Unmatched, unmatched};
use tempfile::TempDir;

fn catalog() -> Catalog {
    Catalog::from_toml(include_str!("../../data/hardware.toml")).unwrap()
}

#[test]
fn lists_unknown_hardware_by_how_many_games_mention_it() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create(&dir.path().join("games.db")).unwrap();
    db.store_catalog(&catalog()).unwrap();
    // Real requirement text: Baldur's Gate 3, a game with the same Adreno,
    // OMORI, and Cyberpunk 2077 (all known).
    let games = [
        (
            1086940,
            None,
            "Nvidia GTX 970 / RX 480 / Intel Arc A380 / Qualcomm Adreno X1 (4GB+ of VRAM)",
        ),
        (2, None, "GeForce GTX 1060 or Qualcomm Adreno X1"),
        (
            1150690,
            Some("Intel N4100 (or similar) or better"),
            "GeForce GTX 1060 6GB",
        ),
        (
            1091500,
            Some("Core i7-6700 or Ryzen 5 1600"),
            "GeForce GTX 1060 6GB or Radeon RX 580 8GB or Arc A380",
        ),
    ];
    for (app_id, cpu, gpu) in games {
        let record = GameRecord {
            app_id,
            name: format!("Game {app_id}"),
            minimum: Some(ParsedTier {
                cpu: cpu.map(str::to_owned),
                gpu: Some(gpu.to_owned()),
                ..ParsedTier::default()
            }),
            ..GameRecord::default()
        };
        db.store(&record, &catalog()).unwrap();
    }

    let report = unmatched(&db, &catalog()).unwrap();

    assert_eq!(
        report,
        [
            Unmatched {
                kind: "gpu",
                text: "Qualcomm Adreno X1".to_owned(),
                games: 2
            },
            Unmatched {
                kind: "cpu",
                text: "Intel N4100".to_owned(),
                games: 1
            },
        ]
    );
}
