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

/// Labels some store pages use instead of Processor / Graphics / Memory.
#[test]
fn reads_cpu_gpu_and_ram_under_other_labels() {
    // Call of Duty: Ghosts: "CPU:", "RAM:"
    let ghosts = parse(tier(209160, "minimum"));
    assert_eq!(
        ghosts.cpu.as_deref(),
        Some("Intel® Core™ 2 Duo E8200 2.66 GHZ / AMD Phenom™ X3 8750 2.4 GHZ or better")
    );
    assert_eq!(ghosts.ram_mb, Some(6144));
    // Realm of the Mad God Exalt: "GPU:"
    assert_eq!(
        parse(tier(200210, "minimum")).gpu.as_deref(),
        Some("NVIDIA GeForce GTX 650 / AMD Radeon HD 7750 (1GB VRAM)")
    );
    // Call of Duty: World at War: "Graphics Card:"
    assert_eq!(
        parse(tier(10090, "minimum")).gpu.as_deref(),
        Some("256 MB (nVidia GeForce 6600/ATI Radeon X1600)")
    );
    // LEGO Harry Potter: Years 1-4: "RAM: 512MB"
    assert_eq!(parse(tier(21130, "minimum")).ram_mb, Some(512));
}

/// Quake lists its parts on lines without labels.
#[test]
fn finds_cpu_gpu_and_ram_lines_without_labels() {
    let quake = parse(tier(2310, "minimum"));

    assert_eq!(
        quake.cpu.as_deref(),
        Some("Intel Core i5-3570 @3.4 GHz or AMD Ryzen 3 1300X @3.5 GHz")
    );
    assert_eq!(
        quake.gpu.as_deref(),
        Some("NVIDIA GeForce GTX 650 TI (2GB) or AMD HD 7750 (1GB)")
    );
    assert_eq!(quake.ram_mb, Some(8192)); // "8GB System RAM"
}

/// Older pages write everything on one line, separated by commas.
#[test]
fn reads_comma_separated_requirements() {
    // Counter-Strike: "500 mhz processor, 96mb ram, 16mb video card, …"
    let counter_strike = parse(tier(10, "minimum"));
    assert_eq!(counter_strike.cpu.as_deref(), Some("500 mhz processor"));
    assert_eq!(counter_strike.gpu.as_deref(), Some("16mb video card"));
    // FlatOut 2: "…, 256 MB RAM, 64 MB video card, 2.0 GHz Pentium® 4 or AMD® 2000+ processor, …"
    let flatout = parse(tier(2990, "minimum"));
    assert_eq!(
        flatout.cpu.as_deref(),
        Some("2.0 GHz Pentium® 4 or AMD® 2000+ processor")
    );
    assert_eq!(flatout.gpu.as_deref(), Some("64 MB video card"));
    assert_eq!(flatout.ram_mb, Some(256));
}

/// "TBD" isn't a CPU; nor is Steam's "Requires a 64-bit processor and
/// operating system" line.
#[test]
fn placeholders_and_the_64_bit_line_are_not_parts() {
    let deadlock = parse(tier(1422450, "minimum"));

    assert_eq!(deadlock.cpu, None);
    assert_eq!(deadlock.gpu, None);
}

/// Suicide Squad: Kill the Justice League has no Memory line; its RAM is
/// in "Additional Notes: RAM: 16 GB (2x8); SSD Recommended".
#[test]
fn finds_ram_written_as_ram_colon() {
    assert_eq!(parse(tier(315210, "minimum")).ram_mb, Some(16384));
}

/// KOTOR II and Stubbs the Zombie give each vendor's card its own line:
/// "Video Card (ATI): Radeon HD 5450", "Video Card (NVidia): GeForce 260".
#[test]
fn joins_gpu_lines_given_per_vendor() {
    assert_eq!(
        parse(tier(208580, "minimum")).gpu.as_deref(),
        Some("Radeon HD 5450 or GeForce 260 or HD 4400")
    );
    assert_eq!(
        parse(tier(7800, "minimum")).gpu.as_deref(),
        Some("Radeon R9 M270 or GeForce GTX 660")
    );
}

/// RAM that pages state plainly, just not as "8 GB RAM".
#[test]
fn reads_memory_written_in_other_ways() {
    let cases = [
        (32370, "Memory: 256 RAM", 256),
        (219740, "Memory: 1+ gigs of RAM GB RAM", 1024),
        (2009100, "Memory: 16(Dual-channel) GB RAM", 16 * 1024),
        (2190220, "Memory: ４ GB RAM (a full-width 4)", 4 * 1024),
        (6880, "256MB System Memory.", 256),
        (6980, "…, 256 MB system memory, 64 MB video memory, …", 256),
        (9500, "…; 256MB of memory", 256),
    ];
    for (app_id, line, mb) in cases {
        assert_eq!(parse(tier(app_id, "minimum")).ram_mb, Some(mb), "{line}");
    }
}

/// Old pages write one paragraph: items split at commas, semicolons or
/// sentences, with a "Label:" somewhere inside.
#[test]
fn reads_parts_from_one_paragraph_pages() {
    // "Windows XP, 1 GHz 32-bit (x86) or 64-bit (x64) CPU, …, WEI Rating: 4.0 (3.0 required)"
    let geometry_wars = parse(tier(8400, "minimum"));
    assert_eq!(
        geometry_wars.cpu.as_deref(),
        Some("1 GHz 32-bit (x86) or 64-bit (x64) CPU")
    );
    assert_eq!(
        geometry_wars.gpu.as_deref(),
        Some("DirectX Video Card (128MB memory - Shader Model 2.0 support required)")
    );
    // "…, AMD, Intel or G3 1000+ Mhz processor; OpenGL Compatible 3D Graphics adaptor (ATI, NVIDIA, Intel, etc.) with …"
    let gish = parse(tier(9500, "minimum"));
    assert_eq!(gish.cpu.as_deref(), Some("Intel or G3 1000+ Mhz processor"));
    assert_eq!(
        gish.gpu.as_deref(),
        Some(
            "OpenGL Compatible 3D Graphics adaptor (ATI, NVIDIA, Intel, etc.) with 32 mb of video memory"
        )
    );
    // "Pentium® III 450. Windows® 98,ME,XP,2000. NVIDIA® TNT2 3D card. 128MB of RAM. …"
    let ghost_master = parse(tier(6200, "minimum"));
    assert_eq!(ghost_master.cpu.as_deref(), Some("Pentium® III 450"));
    assert_eq!(ghost_master.gpu.as_deref(), Some("NVIDIA® TNT2 3D card"));
    // "…, 2.2 GHz Pentium® 4, …, nVidia: GF6600, ATI: Radeon 9800 or better video card*, …"
    let call_of_juarez = parse(tier(3020, "minimum"));
    assert_eq!(call_of_juarez.cpu.as_deref(), Some("2.2 GHz Pentium® 4"));
    assert_eq!(call_of_juarez.gpu.as_deref(), Some("nVidia: GF6600"));
}

/// Older ways to name the graphics requirement without a label.
#[test]
fn finds_video_memory_and_3d_accelerators_as_the_gpu() {
    // Thief: Deadly Shadows: "…, 256 MB system memory, 64 MB video memory, …"
    let thief = parse(tier(6980, "minimum"));
    assert_eq!(thief.gpu.as_deref(), Some("64 MB video memory"));
    assert_eq!(thief.vram_mb, Some(64));
    // Return to Castle Wolfenstein: "3-D Hardware Accelerator (with 16MB VRAM
    // with full OpenGL® support; Pentium® II 400 Mhz processor or …" never
    // closes its parenthesis.
    let wolfenstein = parse(tier(9010, "minimum"));
    assert_eq!(
        wolfenstein.cpu.as_deref(),
        Some("Pentium® II 400 Mhz processor or Athlon® processor")
    );
    assert_eq!(
        wolfenstein.gpu.as_deref(),
        Some("3-D Hardware Accelerator (with 16MB VRAM with full OpenGL® support")
    );
    // Commandos: the sound card shares the line, after the graphics card.
    assert_eq!(
        parse(tier(6800, "minimum")).gpu.as_deref(),
        Some(
            "100% DirectX® 8 compatible 3D Accelerator card with at least 12 MB VRAM 100% DirectX® 8 compatible (or higher) Sound card"
        )
    );
}
