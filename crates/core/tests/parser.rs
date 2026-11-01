//! Tests for `parse(html)`, which reads one tier of a Steam store page's
//! requirements. Inputs are real strings from data/fixtures/requirements/.

use std::sync::LazyLock;

use rigcheck_core::parse;
use serde_json::Value;

static CORPUS: LazyLock<Vec<Value>> = LazyLock::new(|| {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/fixtures/requirements/corpus.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
});

/// The raw HTML of a game's tier ("minimum" or "recommended").
fn tier(app_id: u64, tier: &str) -> &'static str {
    CORPUS
        .iter()
        .find(|game| game["app_id"] == app_id)
        .and_then(|game| game[tier].as_str())
        .unwrap_or_else(|| panic!("{app_id} has no {tier} in the corpus"))
}

const BALDURS_GATE_3: u64 = 1086940;

#[test]
fn reads_memory() {
    // "Memory: 8 GB RAM"
    let parsed = parse(tier(BALDURS_GATE_3, "minimum"));

    assert_eq!(parsed.ram_mb, Some(8 * 1024));
}

#[test]
fn reads_storage_under_any_label() {
    let cases = [
        (
            BALDURS_GATE_3,
            "Storage: 150 GB available space",
            150 * 1024,
        ),
        (500, "Hard Drive: At least 7.5 GB of free space", 7680),
        (
            2280,
            "Disk Space: Minimum 2GB free space on hard drive",
            2048,
        ),
        (3590, "Hard Drive: 65+MB of free hard drive space", 65),
        (22380, "Hard Disk Space: 10GB free space", 10 * 1024),
    ];
    for (app_id, line, mb) in cases {
        assert_eq!(
            parse(tier(app_id, "minimum")).storage_mb,
            Some(mb),
            "{line}"
        );
    }
}

#[test]
fn reads_the_directx_version_under_any_label() {
    let cases = [
        (BALDURS_GATE_3, "minimum", "DirectX: Version 11", 11),
        (2280, "minimum", "Direct X: Version 12", 12),
        (8930, "minimum", "DirectX®: DirectX® version 9.0c", 9),
        (8930, "recommended", "DirectX®: DirectX® version 11", 11),
    ];
    for (app_id, which, line, version) in cases {
        assert_eq!(parse(tier(app_id, which)).directx, Some(version), "{line}");
    }
}

#[test]
fn keeps_the_cpu_and_gpu_text() {
    let parsed = parse(tier(BALDURS_GATE_3, "minimum"));

    assert_eq!(
        parsed.cpu.as_deref(),
        Some("Intel I5 4690 / AMD FX 8350 / Snapdragon X Elite")
    );
    assert_eq!(
        parsed.gpu.as_deref(),
        Some("Nvidia GTX 970 / RX 480 / Intel Arc A380 / Qualcomm Adreno X1 (4GB+ of VRAM)")
    );
}

#[test]
fn reads_the_gpu_under_video_card() {
    // Fallout: New Vegas
    let parsed = parse(tier(22380, "minimum"));

    assert_eq!(
        parsed.gpu.as_deref(),
        Some("NVIDIA GeForce 6 series, ATI 1300XT series")
    );
}

#[test]
fn reads_the_gpu_under_video() {
    // Warframe
    let parsed = parse(tier(230410, "minimum"));

    assert_eq!(
        parsed.gpu.as_deref(),
        Some("DirectX 11+ capable Graphics Card")
    );
}

#[test]
fn decodes_html_entities() {
    // PAYDAY 2: "NVIDIA &amp; AMD (1GB VRAM ..."
    let gpu = parse(tier(218620, "minimum")).gpu.unwrap();

    assert!(gpu.starts_with("NVIDIA & AMD"), "{gpu}");
}

#[test]
fn reads_vram_from_the_graphics_text() {
    let cases = [
        (
            BALDURS_GATE_3,
            "... Qualcomm Adreno X1 (4GB+ of VRAM)",
            Some(4096),
        ),
        // Cyberpunk 2077: the smallest card listed sets the bar.
        (
            1091500,
            "GeForce GTX 1060 6GB or Radeon RX 580 8GB or Arc A380",
            Some(6144),
        ),
        // Skyrim
        (
            72850,
            "Direct X 9.0c compliant video card with 512 MB of RAM",
            Some(512),
        ),
        // Fallout: New Vegas
        (22380, "NVIDIA GeForce 6 series, ATI 1300XT series", None),
    ];
    for (app_id, line, mb) in cases {
        assert_eq!(parse(tier(app_id, "minimum")).vram_mb, mb, "{line}");
    }
}

/// ROADMAP Phase 3: at least 90% of the corpus parses. A tier counts when
/// its CPU, GPU and RAM all come out; the matcher needs those three.
#[test]
fn at_least_90_percent_of_the_corpus_parses() {
    let tiers: Vec<(String, &str)> = CORPUS
        .iter()
        .flat_map(|game| {
            ["minimum", "recommended"]
                .into_iter()
                .filter_map(move |which| {
                    let html = game[which].as_str()?;
                    Some((format!("{} {which}", game["name"]), html))
                })
        })
        .collect();

    let failed: Vec<&String> = tiers
        .iter()
        .filter(|(_, html)| {
            let parsed = parse(html);
            parsed.cpu.is_none() || parsed.gpu.is_none() || parsed.ram_mb.is_none()
        })
        .map(|(name, _)| name)
        .collect();

    let rate = 1.0 - failed.len() as f64 / tiers.len() as f64;
    assert!(
        rate >= 0.9,
        "{:.1}% parsed; failed: {failed:#?}",
        rate * 100.0
    );
}

#[test]
fn finds_ram_in_unlabelled_text() {
    let cases = [
        // Half-Life: "500 mhz processor, 96mb ram, 16mb video card, ..."
        (70, 96),
        // Medieval II: "... 512MB RAM ..." in an unlabelled list
        (4700, 512),
    ];
    for (app_id, mb) in cases {
        assert_eq!(parse(tier(app_id, "minimum")).ram_mb, Some(mb), "{app_id}");
    }
}

/// What the parser makes of every tier in the corpus, reviewed by hand once.
/// A parser change that moves any value shows up as a diff in
/// `snapshots/parser__corpus.snap`; accept it with `cargo insta review`.
#[test]
fn corpus() {
    let parsed: std::collections::BTreeMap<String, _> = CORPUS
        .iter()
        .flat_map(|game| {
            ["minimum", "recommended"]
                .into_iter()
                .filter_map(move |which| {
                    let html = game[which].as_str()?;
                    let app_id = game["app_id"].as_u64()?;
                    let key = format!("{app_id:07} {} ({which})", game["name"].as_str()?);
                    Some((key, parse(html)))
                })
        })
        .collect();

    insta::assert_debug_snapshot!(parsed);
}
