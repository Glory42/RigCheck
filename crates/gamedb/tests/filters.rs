//! The Catalog's filters, on the seed games.db. Expected games come from
//! the facts in data/seed/games.toml. The seed has no popularity, so ties
//! list by name.

use std::path::{Path, PathBuf};

use rigcheck_core::{AntiCheat, ProtonTier};
use rigcheck_gamedb::{Filters, GameDb};
use rigcheck_hardware::{Commands, Hardware, Rig, detect};
use tempfile::TempDir;

fn seed() -> (TempDir, GameDb) {
    let dir = TempDir::new().unwrap();
    let db = GameDb::create_seed(&dir.path().join("games.db")).unwrap();
    (dir, db)
}

fn listed(db: &GameDb, query: &str, filters: &Filters) -> Vec<String> {
    let page = db.games_page(query, filters, 0, 100).unwrap();
    assert_eq!(db.games_count(query, filters).unwrap(), page.len());
    page.into_iter().map(|(_, name)| name).collect()
}

#[test]
fn native_linux_keeps_games_with_a_linux_build() {
    let (_dir, db) = seed();
    let filters = Filters {
        native_linux: true,
        ..Filters::default()
    };

    assert_eq!(
        listed(&db, "", &filters),
        [
            "Counter-Strike 2",
            "Dota 2",
            "Half-Life",
            "Portal 2",
            "Stardew Valley",
            "Terraria"
        ]
    );
}

#[test]
fn deck_verified_keeps_only_verified_games() {
    let (_dir, db) = seed();
    let filters = Filters {
        deck_verified: true,
        ..Filters::default()
    };

    assert_eq!(
        listed(&db, "", &filters),
        [
            "Balatro",
            "Baldur's Gate 3",
            "Counter-Strike 2",
            "Cyberpunk 2077",
            "Dota 2",
            "ELDEN RING",
            "Hades",
            "Half-Life",
            "Halo: The Master Chief Collection",
            "Hogwarts Legacy",
            "OMORI",
            "Portal 2",
            "Stardew Valley",
            "Terraria"
        ]
    );
}

#[test]
fn a_protondb_tier_keeps_games_rated_at_least_that() {
    let (_dir, db) = seed();
    let at_least = |tier| Filters {
        protondb_at_least: Some(tier),
        ..Filters::default()
    };

    assert_eq!(
        listed(&db, "g", &at_least(ProtonTier::Gold)),
        ["Baldur's Gate 3"]
    );
    assert_eq!(
        listed(&db, "g", &at_least(ProtonTier::Silver)),
        ["Baldur's Gate 3", "Crab Game", "Grand Theft Auto III"]
    );
}

#[test]
fn no_anticheat_issues_hides_denied_broken_and_planned_games() {
    let (_dir, db) = seed();
    db.set_anticheat(&[
        (1782210, AntiCheat::Broken, "Easy Anti-Cheat".to_owned()),
        (12100, AntiCheat::Planned, "BattlEye".to_owned()),
    ])
    .unwrap();
    let filters = Filters {
        no_anticheat_issues: true,
        ..Filters::default()
    };

    let names = listed(&db, "", &filters);

    // 22 seed games, less 3 denied, 1 broken and 1 planned.
    assert_eq!(names.len(), 17);
    for hidden in [
        "Destiny 2",
        "PUBG: BATTLEGROUNDS",
        "Apex Legends™",
        "Crab Game",
        "Grand Theft Auto III",
    ] {
        assert!(!names.iter().any(|name| name == hidden), "{hidden} listed");
    }
    for kept in ["ELDEN RING", "Halo: The Master Chief Collection"] {
        assert!(names.iter().any(|name| name == kept), "{kept} missing");
    }
}

/// Replays a hardware fixture's captured command output.
struct Captured(PathBuf);

impl Commands for Captured {
    fn run(&self, program: &str, _args: &[&str]) -> Option<String> {
        std::fs::read_to_string(self.0.join("commands").join(format!("{program}.txt"))).ok()
    }
}

fn amd_laptop() -> Hardware {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/hardware/amd-laptop");
    detect(&root, &Captured(root.clone()))
}

#[test]
fn can_run_keeps_games_that_meet_at_least_the_minimum() {
    let (_dir, db) = seed();
    db.store_names(&[(30, "Day of Defeat")]).unwrap();
    let filters = Filters {
        runs_on: Some(
            Rig::new(&db.catalog().unwrap(), &amd_laptop())
                .specs()
                .clone(),
        ),
        ..Filters::default()
    };

    let names = listed(&db, "", &filters);

    // Worked out by hand in tests/verdict.rs.
    for kept in ["OMORI", "LEGO® Batman™: The Videogame"] {
        assert!(names.iter().any(|name| name == kept), "{kept} missing");
    }
    // Below the minimum, blocked by anti-cheat, or never checked.
    for hidden in ["Cyberpunk 2077", "ELDEN RING", "Destiny 2", "Day of Defeat"] {
        assert!(!names.iter().any(|name| name == hidden), "{hidden} listed");
    }
}
