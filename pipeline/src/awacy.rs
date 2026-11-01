//! AreWeAntiCheatYet's games.json → anti-cheat status per Steam game.

use std::collections::HashMap;

use rigcheck_core::AntiCheat;
use serde::Deserialize;

use crate::Error;

/// A game's anti-cheat and whether it works on Linux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AntiCheatInfo {
    pub status: AntiCheat,
    /// The anti-cheats it uses, e.g. "BattlEye" or "Easy Anti-Cheat".
    pub name: String,
}

/// Status per Steam app ID. Games without a Steam ID are skipped.
pub fn statuses(json: &str) -> Result<HashMap<u32, AntiCheatInfo>, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Game {
        status: String,
        #[serde(default)]
        anticheats: Vec<String>,
        #[serde(default)]
        store_ids: StoreIds,
    }
    #[derive(Deserialize, Default)]
    struct StoreIds {
        steam: Option<String>,
    }

    let games: Vec<Game> = serde_json::from_str(json)?;
    Ok(games
        .into_iter()
        .filter_map(|game| {
            let app_id = game.store_ids.steam?.parse().ok()?;
            let status = match game.status.as_str() {
                "Supported" => AntiCheat::Supported,
                "Running" => AntiCheat::Running,
                "Planned" => AntiCheat::Planned,
                "Broken" => AntiCheat::Broken,
                "Denied" => AntiCheat::Denied,
                _ => return None,
            };
            let name = game.anticheats.join(", ");
            Some((app_id, AntiCheatInfo { status, name }))
        })
        .collect())
}
