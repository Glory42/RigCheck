//! `build`: merging the data sources into the crawled games.db.

use std::collections::HashMap;

use rigcheck_gamedb::GameDb;

use crate::Error;
use crate::applist::App;
use crate::awacy::AntiCheatInfo;
use crate::protondb::Summary;

/// ROADMAP §1.2: ProtonDB's data is ODbL (share-alike), so games.db is too.
const LICENSE: &str = "This database is made available under the Open Database License \
(ODbL 1.0): https://opendatacommons.org/licenses/odbl/1-0/. Its contents are under the \
Database Contents License: https://opendatacommons.org/licenses/dbcl/1-0/.";

/// Credits for every data source (ROADMAP §9).
const SOURCES: &str = "Steam store data: Valve Corporation (store.steampowered.com). \
ProtonDB reports: ProtonDB (protondb.com), ODbL, via github.com/bdefore/protondb-data; \
tiers computed by rigcheck's own rule. \
Anti-cheat status: AreWeAntiCheatYet (areweanticheatyet.com), MIT, © AreWeAntiCheatYet contributors. \
Hardware table: rigcheck (github.com/Glory42/rigcheck).";

/// Adds every app-list game not crawled yet (by name), ProtonDB tiers and
/// popularity and anti-cheat status, then rebuilds the name search.
pub fn build(
    db: &GameDb,
    apps: &[App],
    protondb: &HashMap<u32, Summary>,
    anticheat: &HashMap<u32, AntiCheatInfo>,
) -> Result<(), Error> {
    let names: Vec<(u32, &str)> = apps
        .iter()
        .map(|app| (app.app_id, app.name.as_str()))
        .collect();
    db.store_names(&names)?;
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
    db.set_meta("license", LICENSE)?;
    db.set_meta("sources", SOURCES)?;
    Ok(())
}
