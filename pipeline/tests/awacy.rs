//! Tests for reading AreWeAntiCheatYet's games.json into anti-cheat status
//! per Steam game.

use rigcheck_core::AntiCheat;
use rigcheck_pipeline::awacy::{AntiCheatInfo, statuses};

const GAMES: &str = include_str!("../../data/fixtures/awacy/games.json");

#[test]
fn reads_each_steam_games_anti_cheat_status() {
    let statuses = statuses(GAMES).unwrap();

    assert_eq!(
        statuses[&1085660], // Destiny 2
        AntiCheatInfo {
            status: AntiCheat::Denied,
            name: "BattlEye".to_owned()
        }
    );
    assert_eq!(
        statuses[&578080], // PUBG
        AntiCheatInfo {
            status: AntiCheat::Broken,
            name: "Zakynthos, BattlEye, UNCHEATER".to_owned()
        }
    );
    assert_eq!(statuses[&976730].status, AntiCheat::Supported); // Halo MCC
    assert_eq!(statuses[&444090].status, AntiCheat::Running); // Paladins
    assert_eq!(statuses[&753650].status, AntiCheat::Planned); // Due Process
    assert_eq!(statuses.len(), 5, "Fortnite has no Steam ID");
}
