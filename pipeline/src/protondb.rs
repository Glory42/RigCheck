//! ProtonDB's raw reports (ODbL dump) → a tier and a report count per game.
//! ProtonDB's own tier formula isn't published, so this rule is ours
//! (ROADMAP §9) and the app labels it "ProtonDB reports".

use std::collections::HashMap;
use std::io::Read;

use rigcheck_core::{Catalog, CatalogEntry, ProtonTier, Reported};
use serde::Deserialize;

use crate::Error;

/// One report, with only the fields the rule reads.
#[derive(Debug, Clone, Deserialize)]
pub struct Report {
    app: ReportApp,
    pub timestamp: i64,
    responses: Responses,
    #[serde(rename = "systemInfo", default)]
    system_info: Option<SystemInfo>,
}

/// The player's machine, as ProtonDB's form read it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct SystemInfo {
    cpu: Option<String>,
    gpu: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReportApp {
    steam: SteamApp,
}

#[derive(Debug, Clone, Deserialize)]
struct SteamApp {
    #[serde(rename = "appId")]
    app_id: AppId,
}

/// Most reports write the ID as a string, some as a number.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum AppId {
    Text(String),
    Number(u32),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Responses {
    verdict: Option<String>,
    verdict_oob: Option<String>,
    variant: Option<String>,
    custom_proton_version: Option<String>,
    launch_options: Option<String>,
    audio_faults: Option<String>,
    graphical_faults: Option<String>,
    input_faults: Option<String>,
    performance_faults: Option<String>,
    save_game_faults: Option<String>,
    stability_faults: Option<String>,
    windowing_faults: Option<String>,
    significant_bugs: Option<String>,
}

impl Report {
    pub fn app_id(&self) -> Option<u32> {
        match &self.app.steam.app_id {
            AppId::Text(text) => text.parse().ok(),
            AppId::Number(number) => Some(*number),
        }
    }
}

/// What ProtonDB's reports say about a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    /// `None` if no report is about running it through Proton.
    pub tier: Option<ProtonTier>,
    /// Every report, native ones included: how much Linux players care.
    pub reports: u32,
}

/// The CPUs and GPUs players ran a game on, as their reports name them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Played {
    pub cpus: Vec<String>,
    pub gpus: Vec<String>,
}

/// What every game was played on, from the reports where it worked without
/// performance problems: one that didn't run, or ran poorly, says nothing
/// about what hardware is enough.
pub fn played_on(reports: &[Report]) -> HashMap<u32, Played> {
    let mut played: HashMap<u32, Played> = HashMap::new();
    for report in reports {
        let worked = report.responses.verdict.as_deref() == Some("yes")
            && report.responses.performance_faults.as_deref() != Some("yes");
        let (Some(app_id), Some(system), true) = (report.app_id(), &report.system_info, worked)
        else {
            continue;
        };
        let game = played.entry(app_id).or_default();
        game.cpus.extend(system.cpu.clone());
        game.gpus.extend(system.gpu.clone());
    }
    played
}

/// The weakest and typical CPU and GPU in `played` that the hardware table
/// knows. Fewer than three known parts of a kind say nothing about it.
pub fn reported(played: &Played, catalog: &Catalog) -> (Option<Reported>, Option<Reported>) {
    let cpus = played
        .cpus
        .iter()
        .filter_map(|cpu| catalog.match_cpus(cpu).first().copied());
    let gpus = played
        .gpus
        .iter()
        .filter_map(|gpu| catalog.match_gpus(gpu).first().copied());
    (spread(cpus), spread(gpus))
}

/// The 20th percentile (at least the second weakest, so one report naming
/// the wrong part doesn't decide it) and the median of `parts`, by score.
fn spread<'a>(parts: impl Iterator<Item = &'a CatalogEntry>) -> Option<Reported> {
    let mut parts: Vec<&CatalogEntry> = parts.collect();
    if parts.len() < 3 {
        return None;
    }
    parts.sort_by_key(|part| part.score);
    let weakest = parts[(parts.len() / 5).max(1)];
    Some(Reported {
        weakest_name: weakest.name.clone(),
        weakest: weakest.score,
        typical: parts[parts.len() / 2].score,
    })
}

/// Reads the dump's `reports_piiremoved.json` (a JSON array).
pub fn read_reports(reader: impl Read) -> Result<Vec<Report>, Error> {
    Ok(serde_json::from_reader(reader)?)
}

const YEAR: i64 = 365 * 24 * 60 * 60;

/// A tier and report count for every game with reports, as of `now`.
pub fn summarize(reports: &[Report], now: i64) -> HashMap<u32, Summary> {
    let mut by_app: HashMap<u32, Vec<&Report>> = HashMap::new();
    for report in reports {
        if let Some(app_id) = report.app_id() {
            by_app.entry(app_id).or_default().push(report);
        }
    }
    by_app
        .into_iter()
        .map(|(app_id, reports)| {
            let proton: Vec<&Report> = reports
                .iter()
                .copied()
                .filter(|r| r.responses.variant.as_deref() != Some("native"))
                .collect();
            let recent: Vec<&Report> = proton
                .iter()
                .copied()
                .filter(|r| r.timestamp >= now - YEAR)
                .collect();
            let counted = if recent.len() >= 3 { recent } else { proton };
            let summary = Summary {
                tier: most_common(counted.iter().map(|r| rate(&r.responses))),
                reports: reports.len() as u32,
            };
            (app_id, summary)
        })
        .collect()
}

/// The most common tier; ties go to the worse one.
fn most_common(tiers: impl Iterator<Item = ProtonTier>) -> Option<ProtonTier> {
    let mut counts: HashMap<ProtonTier, usize> = HashMap::new();
    for tier in tiers {
        *counts.entry(tier).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(tier, count)| (count, rank(tier)))
        .map(|(tier, _)| tier)
}

/// Higher is worse, so ties pick the worse tier.
fn rank(tier: ProtonTier) -> u8 {
    match tier {
        ProtonTier::Platinum => 0,
        ProtonTier::Gold => 1,
        ProtonTier::Silver => 2,
        ProtonTier::Bronze => 3,
        ProtonTier::Borked => 4,
    }
}

/// One report's rating (ROADMAP §9).
fn rate(r: &Responses) -> ProtonTier {
    let yes = |answer: &Option<String>| answer.as_deref() == Some("yes");
    if r.verdict.as_deref() == Some("no") {
        return ProtonTier::Borked;
    }
    if yes(&r.stability_faults) || yes(&r.significant_bugs) {
        return ProtonTier::Bronze;
    }
    let faults = [
        &r.audio_faults,
        &r.graphical_faults,
        &r.input_faults,
        &r.performance_faults,
        &r.save_game_faults,
        &r.windowing_faults,
    ];
    if faults.into_iter().any(yes) {
        return ProtonTier::Silver;
    }
    let tweaked = r.verdict_oob.as_deref() == Some("no")
        || r.custom_proton_version
            .as_deref()
            .is_some_and(|v| !v.is_empty())
        || r.launch_options
            .as_deref()
            .is_some_and(|o| !o.trim().is_empty())
        || matches!(r.variant.as_deref(), Some("ge" | "notListed"));
    if tweaked {
        ProtonTier::Gold
    } else {
        ProtonTier::Platinum
    }
}
