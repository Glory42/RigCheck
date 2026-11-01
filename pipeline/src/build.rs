//! `build`: merging the data sources into the crawled games.db.

use std::collections::HashMap;

use rigcheck_gamedb::GameDb;

use crate::Error;
use crate::awacy::AntiCheatInfo;
use crate::protondb::Summary;

/// Adds ProtonDB tiers and popularity and anti-cheat status to the stored
/// games, then rebuilds the name search.
pub fn build(
    db: &GameDb,
    protondb: &HashMap<u32, Summary>,
    anticheat: &HashMap<u32, AntiCheatInfo>,
) -> Result<(), Error> {
    let proton: Vec<_> = protondb
        .iter()
        .map(|(&app_id, summary)| (app_id, summary.tier, summary.reports))
        .collect();
    db.set_protondb(&proton)?;
    let anticheat: Vec<_> = anticheat
        .iter()
        .map(|(&app_id, info)| (app_id, info.status, info.name.clone()))
        .collect();
    db.set_anticheat(&anticheat)?;
    db.rebuild_search()?;
    Ok(())
}
