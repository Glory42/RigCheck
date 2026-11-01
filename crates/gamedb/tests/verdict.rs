//! End to end: a machine from data/fixtures/hardware + a seed game → verdict.
//! Expected verdicts are worked out by hand from each game's Steam text and
//! the scores in data/hardware.toml.

use std::fs;
use std::path::{Path, PathBuf};

use rigcheck_core::{Blocker, Component, Level, Verdict};
use rigcheck_gamedb::{GameDb, specs};
use rigcheck_hardware::{Commands, Hardware, detect};
use tempfile::TempDir;

/// Replays a fixture's captured command output.
struct Captured(PathBuf);

impl Commands for Captured {
    fn run(&self, program: &str, _args: &[&str]) -> Option<String> {
        fs::read_to_string(self.0.join("commands").join(format!("{program}.txt"))).ok()
    }
}

fn machine(name: &str) -> Hardware {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/fixtures/hardware")
        .join(name);
    detect(&root, &Captured(root.clone()))
}

fn verdict(db: &GameDb, hardware: &Hardware, app_id: u32) -> Verdict {
    let specs = specs(&db.catalog().unwrap(), hardware);
    db.verdict(app_id, &specs)
        .unwrap()
        .expect("game is in the seed")
}

#[test]
fn verdicts_for_the_amd_laptop() {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create_seed(&dir.path().join("games.db")).unwrap();
    let laptop = machine("amd-laptop");

    // Radeon Vega 7 (32) vs the same-vendor RX 580 (100).
    let cyberpunk = verdict(&db, &laptop, 1091500);
    assert_eq!(
        (cyberpunk.overall, cyberpunk.bottleneck),
        (Level::Below, Some(Component::Gpu))
    );
    let elden_ring = verdict(&db, &laptop, 1245620);
    assert_eq!(
        (elden_ring.overall, elden_ring.bottleneck),
        (Level::Below, Some(Component::Gpu))
    );

    // RAM meets the recommended 8 GB; CPU and GPU aren't in the table.
    assert_eq!(verdict(&db, &laptop, 1150690).overall, Level::Recommended); // OMORI
    // Only a minimum tier (256 MB RAM), which the laptop meets.
    assert_eq!(verdict(&db, &laptop, 21000).overall, Level::Recommended); // LEGO Batman

    let destiny = verdict(&db, &laptop, 1085660);
    assert_eq!(
        (destiny.overall, destiny.blocker),
        (Level::Blocked, Some(Blocker::AntiCheat))
    );
}
