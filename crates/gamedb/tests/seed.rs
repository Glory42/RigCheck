//! Tests for the seed games.db: built from data/seed/ into a temp file, then
//! queried the way the app does.

use rigcheck_core::{AntiCheat, CatalogEntry, ProtonTier};
use rigcheck_gamedb::GameDb;
use tempfile::TempDir;

fn seed() -> (TempDir, GameDb) {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create_seed(&dir.path().join("games.db")).unwrap();
    (dir, db)
}

fn names(entries: &[CatalogEntry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.name.as_str()).collect()
}

#[test]
fn looks_up_a_games_requirements_and_linux_facts() {
    let (_dir, db) = seed();

    let game = db
        .game(1086940)
        .unwrap()
        .expect("Baldur's Gate 3 is in the seed");

    assert_eq!(game.name, "Baldur's Gate 3");
    let minimum = game.minimum.expect("a minimum tier");
    assert_eq!(minimum.ram_mb, Some(8192));
    assert_eq!(
        minimum.gpu_text.as_deref(),
        Some("Nvidia GTX 970 / RX 480 / Intel Arc A380 / Qualcomm Adreno X1 (4GB+ of VRAM)")
    );
    assert_eq!(
        names(&minimum.gpus),
        ["GeForce GTX 970", "Radeon RX 480", "Arc A380"]
    );
    assert_eq!(game.linux.protondb, Some(ProtonTier::Platinum));
    assert!(!game.linux.native);
}

#[test]
fn keeps_anti_cheat_facts() {
    let (_dir, db) = seed();

    let game = db.game(1085660).unwrap().expect("Destiny 2 is in the seed");

    assert_eq!(game.linux.anticheat, Some(AntiCheat::Denied));
    assert_eq!(game.anticheat_name.as_deref(), Some("BattlEye"));
}

#[test]
fn a_game_missing_from_the_database_is_none() {
    let (_dir, db) = seed();

    assert_eq!(db.game(4).unwrap(), None);
}

#[test]
fn the_hardware_table_ships_inside_the_database() {
    let (_dir, db) = seed();

    let catalog = db.catalog().unwrap();

    let gpu = catalog
        .gpu_by_pci(0x1002, 0x15e7)
        .map(|gpu| gpu.name.as_str());
    assert_eq!(gpu, Some("Radeon Vega 7 (Ryzen 5000/7000U)"));
    let cpu = catalog.match_cpus("AMD Ryzen 5 7530U with Radeon Graphics");
    assert_eq!(
        names(&cpu.into_iter().cloned().collect::<Vec<_>>()),
        ["Ryzen 5 7530U"]
    );
}
