//! `report`: CPU and GPU names the hardware table doesn't know, by how many
//! games mention them, so data/hardware.toml can grow where it matters.

use std::collections::{HashMap, HashSet};

use rigcheck_core::Catalog;
use rigcheck_gamedb::GameDb;

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmatched {
    /// "cpu" or "gpu".
    pub kind: &'static str,
    /// As a requirement wrote it, e.g. "Qualcomm Adreno X1".
    pub text: String,
    /// How many games mention it.
    pub games: usize,
}

/// Unknown names, most mentioned first.
pub fn unmatched(db: &GameDb, catalog: &Catalog) -> Result<Vec<Unmatched>, Error> {
    // Each (kind, name) once per game, however many tiers mention it.
    let mut seen: HashSet<(u32, &'static str, String)> = HashSet::new();
    for tier in db.requirement_text()? {
        let cpus = tier
            .cpu
            .map(|text| catalog.unmatched_cpus(&text))
            .unwrap_or_default();
        let gpus = tier
            .gpu
            .map(|text| catalog.unmatched_gpus(&text))
            .unwrap_or_default();
        for name in cpus {
            seen.insert((tier.app_id, "cpu", name));
        }
        for name in gpus {
            seen.insert((tier.app_id, "gpu", name));
        }
    }
    let mut counts: HashMap<(&'static str, String), usize> = HashMap::new();
    for (_, kind, name) in seen {
        *counts.entry((kind, name)).or_default() += 1;
    }
    let mut report: Vec<Unmatched> = counts
        .into_iter()
        .map(|((kind, text), games)| Unmatched { kind, text, games })
        .collect();
    report.sort_by(|a, b| b.games.cmp(&a.games).then_with(|| a.text.cmp(&b.text)));
    Ok(report)
}
