//! What ProtonDB players ran each game on (`reported_hardware`): kept by
//! the pipeline, read with every record, and used where Steam's text names
//! no part.

use std::path::{Path, PathBuf};

use rigcheck_core::{Component, Confidence, Level, Reported, Specs};
use rigcheck_gamedb::{Filters, GameDb};
use rigcheck_hardware::{Commands, Rig, detect};
use tempfile::TempDir;

/// OMORI: the seed's minimum names an Intel N4100 (not in the table) and
/// "DirectX 9/OpenGL 4.1 capable GPU", so only its RAM compares on its own.
const OMORI: u32 = 1150690;

struct Captured(PathBuf);

impl Commands for Captured {
    fn run(&self, program: &str, _args: &[&str]) -> Option<String> {
        std::fs::read_to_string(self.0.join("commands").join(format!("{program}.txt"))).ok()
    }
}

/// The AMD laptop: Ryzen 5 7530U (190), Radeon Vega 7 (32).
fn amd_laptop(db: &GameDb) -> Specs {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/hardware/amd-laptop");
    let hardware = detect(&root, &Captured(root.clone()));
    Rig::new(&db.catalog().unwrap(), &hardware).specs().clone()
}

fn part(name: &str, weakest: u32, typical: u32) -> Reported {
    Reported {
        weakest_name: name.to_owned(),
        weakest,
        typical,
    }
}

fn seed() -> (TempDir, GameDb) {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create_seed(&dir.path().join("games.db")).unwrap();
    (dir, db)
}

#[test]
fn a_record_carries_what_players_ran_it_on() {
    let (_dir, db) = seed();
    let gpu = part("Radeon Vega 8", 30, 100);

    db.set_reported(&[(OMORI, None, Some(gpu.clone()))])
        .unwrap();

    let game = db.game(OMORI).unwrap().unwrap();
    assert_eq!(game.linux.reported_gpu, Some(gpu));
    assert_eq!(game.linux.reported_cpu, None);
}

#[test]
fn players_reports_fill_in_the_parts_steam_doesnt_name() {
    let (_dir, db) = seed();
    let specs = amd_laptop(&db);
    let before = db.game(OMORI).unwrap().unwrap().verdict(&specs);
    assert_eq!(before.confidence, Confidence::Partial);

    db.set_reported(&[(
        OMORI,
        Some(part("Core i3-6100", 60, 150)),
        Some(part("Radeon Vega 8", 30, 100)),
    )])
    .unwrap();

    let after = db.game(OMORI).unwrap().unwrap().verdict(&specs);
    // The CPU (190) beats the typical player's; the GPU (32) only the weakest.
    assert_eq!(after.confidence, Confidence::High);
    assert_eq!(after.reported, [Component::Cpu, Component::Gpu]);
    assert_eq!(after.overall, Level::Minimum);
    assert_eq!(after.bottleneck, Some(Component::Gpu));
}

#[test]
fn a_new_build_replaces_every_games_reports() {
    let (_dir, db) = seed();
    db.set_reported(&[(OMORI, None, Some(part("Radeon Vega 8", 30, 100)))])
        .unwrap();

    db.set_reported(&[(OMORI, Some(part("Core i3-6100", 60, 150)), None)])
        .unwrap();

    let game = db.game(OMORI).unwrap().unwrap();
    assert_eq!(game.linux.reported_gpu, None);
    assert!(game.linux.reported_cpu.is_some());
}

#[test]
fn a_games_db_from_before_reports_opens_without_them() {
    let (dir, db) = seed();
    drop(db);
    let path = dir.path().join("games.db");
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("DROP TABLE reported_hardware")
        .unwrap();

    let db = GameDb::open(&path).unwrap();

    let game = db.game(OMORI).unwrap().unwrap();
    assert_eq!(game.linux.reported_gpu, None);
    assert!(db.games_count("", &Filters::default()).unwrap() > 0);
}
