//! `build`: merging the data sources into the crawled games.db.

use std::collections::HashMap;

use rigcheck_gamedb::GameDb;

use rigcheck_core::Catalog;

use crate::Error;
use crate::applist::App;
use crate::awacy::AntiCheatInfo;
use crate::blender::GapScores;
use crate::pci::PciId;
use crate::protondb::Summary;

/// Everything `build` merges into the crawled games.
#[derive(Debug, Clone, Default)]
pub struct Sources {
    /// Every game on Steam, for name search.
    pub apps: Vec<App>,
    pub protondb: HashMap<u32, Summary>,
    pub anticheat: HashMap<u32, AntiCheatInfo>,
    /// Parts missing from data/hardware.toml, scored from Blender Open Data.
    pub gaps: GapScores,
    pub pci_ids: Vec<PciId>,
}

/// ROADMAP §1.2: ProtonDB's data is ODbL (share-alike), so games.db is too.
const LICENSE: &str = "This database is made available under the Open Database License \
(ODbL 1.0): https://opendatacommons.org/licenses/odbl/1-0/. Its contents are under the \
Database Contents License: https://opendatacommons.org/licenses/dbcl/1-0/.";

/// Credits for every data source (ROADMAP §9).
const SOURCES: &str = "Steam store data: Valve Corporation (store.steampowered.com). \
ProtonDB reports: ProtonDB (protondb.com), ODbL, via github.com/bdefore/protondb-data; \
tiers computed by rigcheck's own rule. \
Anti-cheat status: AreWeAntiCheatYet (areweanticheatyet.com), MIT, © AreWeAntiCheatYet contributors. \
Steam Deck compatibility: Valve Corporation's Deck compatibility reports. \
Hardware scores: rigcheck's table (github.com/Glory42/rigcheck), gaps from Blender Open Data \
(opendata.blender.org), CC0. GPU PCI IDs: pci.ids (pci-ids.ucw.cz), BSD-3-Clause or GPL-2.0.";

/// Adds missing hardware and GPU PCI IDs and re-matches every game against
/// the grown table; adds every app-list game not crawled yet (by name),
/// ProtonDB tiers and popularity and anti-cheat status; rebuilds the name
/// search; and writes the license and source credits.
pub fn build(db: &GameDb, sources: &Sources) -> Result<(), Error> {
    let Sources {
        apps,
        protondb,
        anticheat,
        gaps,
        pci_ids,
    } = sources;
    db.store_extra_hardware(
        &Catalog::new(gaps.cpus.clone(), gaps.gpus.clone()),
        "blender",
    )?;
    let pci_rows: Vec<(u16, u16, &str)> = pci_ids
        .iter()
        .map(|id| (id.vendor_id, id.device_id, id.name.as_str()))
        .collect();
    db.add_pci_ids(&pci_rows)?;
    // A parser fix reaches every crawled game, not just re-crawled ones.
    db.reparse()?;
    db.rematch()?;
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
