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
use rigcheck_pipeline::protondb::{Report, read_reports, summarize};

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
