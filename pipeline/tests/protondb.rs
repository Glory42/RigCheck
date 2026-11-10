//! Tests for turning ProtonDB's raw reports into a tier per game (ROADMAP §9).
//!
//! Per report: Borked if the verdict is "no"; Bronze with stability faults or
//! significant bugs; Silver with other faults; Gold if it needed tweaks (not
//! working out of the box, custom or GE Proton, launch options); else Platinum.
//! Per game: the most common rating in the last 12 months (all reports if
//! fewer than 3 are that recent), ties going to the worse tier. Reports on
//! native Linux builds don't count toward the tier.

use std::fs::File;
use std::io::BufReader;

use rigcheck_core::ProtonTier;
use rigcheck_pipeline::protondb::{Report, played_on, read_reports, reported, summarize};

/// 2026-10-01, the dump's date.
const NOW: i64 = 1_790_812_800;

fn reports() -> Vec<Report> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../data/fixtures/protondb/reports.json"
    );
    read_reports(BufReader::new(File::open(path).unwrap())).unwrap()
}

#[test]
fn a_game_gets_its_most_common_recent_rating() {
    // Lara Croft GO, last 12 months: Borked, Silver, Borked, Gold, Platinum.
    let summaries = summarize(&reports(), NOW);

    assert_eq!(summaries[&540840].tier, Some(ProtonTier::Borked));
}

#[test]
fn ties_go_to_the_worse_tier() {
    // Bye Sweet Carole: Platinum, Gold, Platinum, Gold.
    assert_eq!(
        summarize(&reports(), NOW)[&2428980].tier,
        Some(ProtonTier::Gold)
    );
}

#[test]
fn with_fewer_than_3_recent_reports_older_ones_count_too() {
    // Chompy Chomp Chomp: recent Gold and Platinum would tie (→ Gold); with
    // the 2019 Platinum report it's Platinum 2 to 1.
    assert_eq!(
        summarize(&reports(), NOW)[&292570].tier,
        Some(ProtonTier::Platinum)
    );
}

#[test]
fn native_reports_count_for_popularity_but_not_the_tier() {
    let summaries = summarize(&reports(), NOW);

    assert_eq!(summaries[&3493540].tier, None);
    assert_eq!(summaries[&3493540].reports, 1);
    assert_eq!(summaries[&540840].reports, 7);
}

#[test]
fn each_report_is_rated_by_the_rule() {
    let all = reports();
    let cases = [
        // Lara Croft GO
        (1762124894, ProtonTier::Borked),   // verdict "no"
        (1766589519, ProtonTier::Silver),   // input faults
        (1779432570, ProtonTier::Gold),     // an unlisted Proton build
        (1782658133, ProtonTier::Platinum), // official Proton, no faults
        // Act of War: High Treason
        (1753653100, ProtonTier::Bronze), // stability faults
        (1669894058, ProtonTier::Gold),   // GE-Proton
    ];
    for (timestamp, tier) in cases {
        let one: Vec<Report> = all
            .iter()
            .filter(|r| r.timestamp == timestamp)
            .cloned()
            .collect();
        let summary = summarize(&one, NOW).into_values().next().unwrap();
        assert_eq!(summary.tier, Some(tier), "report at {timestamp}");
    }
}

#[test]
fn reports_with_a_numeric_app_id_count_too() {
    // About 8% of the dump writes "appId": 427510 instead of "427510".
    let summaries = summarize(&reports(), NOW);

    assert_eq!(summaries[&427510].reports, 1);
}

/// A report on app 1 from a player with `gpu` and `cpu`.
fn report(verdict: &str, performance_faults: &str, cpu: &str, gpu: &str) -> String {
    format!(
        r#"{{"app": {{"steam": {{"appId": "1"}}}}, "timestamp": {NOW},
            "responses": {{"verdict": "{verdict}", "performanceFaults": "{performance_faults}"}},
            "systemInfo": {{"cpu": "{cpu}", "gpu": "{gpu}"}}}}"#
    )
}

fn catalog() -> rigcheck_core::Catalog {
    rigcheck_core::Catalog::from_toml(include_str!("../../data/hardware.toml")).unwrap()
}

#[test]
fn players_hardware_comes_from_reports_where_the_game_worked_well() {
    let working = |gpu| report("yes", "no", "Intel Core i5-4460", gpu);
    let reports = [
        working("NVIDIA GeForce GTX 750 Ti"), // 35
        working("NVIDIA GeForce GTX 660"),    // 38
        working("NVIDIA GeForce GTX 970"),    // 83
        working("NVIDIA GeForce GTX 1060 6GB"),
        working("NVIDIA GeForce RTX 4070"),
        working("Some GPU the table doesn't know"),
        // Didn't run, or ran poorly: they say nothing about what's enough.
        report("no", "no", "Intel Core i5-4460", "Intel UHD Graphics 620"),
        report("yes", "yes", "Intel Core i5-4460", "NVIDIA GeForce GTS 450"),
    ];
    let reports = read_reports(format!("[{}]", reports.join(",")).as_bytes()).unwrap();

    let (cpu, gpu) = reported(&played_on(&reports)[&1], &catalog());

    // Five GPUs the table knows: the weakest is the 20th percentile (the
    // second, so one odd report doesn't decide it), the typical the median.
    let gpu = gpu.unwrap();
    assert_eq!(
        (gpu.weakest_name.as_str(), gpu.weakest),
        ("GeForce GTX 660", 38)
    );
    assert_eq!(gpu.typical, 83);
    assert_eq!(cpu.unwrap().weakest_name, "Core i5-4460");
}

#[test]
fn fewer_than_three_known_parts_say_nothing() {
    let reports = [
        report("yes", "no", "Intel Core i5-4460", "NVIDIA GeForce GTX 970"),
        report(
            "yes",
            "no",
            "Intel Core i5-4460",
            "NVIDIA GeForce GTX 1060 6GB",
        ),
    ];
    let reports = read_reports(format!("[{}]", reports.join(",")).as_bytes()).unwrap();

    assert_eq!(reported(&played_on(&reports)[&1], &catalog()), (None, None));
}
