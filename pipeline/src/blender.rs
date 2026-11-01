//! Filling gaps in the hardware table from Blender Open Data (CC0).
//!
//! Blender's benchmark measures rendering, not games, so hand scores in
//! data/hardware.toml always win. Blender only scores parts the table lacks:
//! each device's median score (sum of samples/minute over the three standard
//! scenes, ≥ 3 runs) is rescaled per vendor by the median ratio of our score
//! to Blender's over parts both have (≥ 3 of them). NVIDIA uses CUDA runs,
//! because OptiX also uses RT cores and overrates RTX cards against GTX ones.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};

use rigcheck_core::{Catalog, CatalogEntry, Vendor};
use serde::Deserialize;

use crate::Error;

const SCENES: [&str; 3] = ["monster", "junkshop", "classroom"];
const MIN_RUNS: usize = 3;
const MIN_ANCHORS: usize = 3;

/// Parts the table lacks, with rescaled scores.
#[derive(Debug, Clone, Default)]
pub struct GapScores {
    pub cpus: Vec<CatalogEntry>,
    pub gpus: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
struct Record {
    schema_version: String,
    data: Vec<Run>,
}

#[derive(Deserialize)]
struct Run {
    device_info: DeviceInfo,
    scene: Scene,
    stats: Stats,
}

#[derive(Deserialize)]
struct DeviceInfo {
    device_type: String,
    compute_devices: Vec<Device>,
}

#[derive(Deserialize)]
struct Device {
    name: String,
}

#[derive(Deserialize)]
struct Scene {
    label: String,
}

#[derive(Deserialize)]
struct Stats {
    samples_per_minute: Option<f64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Cpu,
    Gpu,
}

/// Reads a snapshot (one JSON record per line) and scores missing parts.
pub fn gap_scores(snapshot: impl Read, catalog: &Catalog) -> Result<GapScores, Error> {
    let mut runs: HashMap<(Kind, Vendor, String), Vec<f64>> = HashMap::new();
    for line in BufReader::new(snapshot).lines() {
        let Ok(record) = serde_json::from_str::<Record>(&line?) else {
            continue; // other schemas and malformed lines
        };
        if let Some((device, score)) = benchmark(&record) {
            runs.entry(device).or_default().push(score);
        }
    }

    // Median score per device with enough runs.
    let medians: Vec<((Kind, Vendor, String), f64)> = runs
        .into_iter()
        .filter(|(_, scores)| scores.len() >= MIN_RUNS)
        .map(|(device, scores)| (device, median(scores)))
        .collect();

    // Per kind and vendor: our-score / Blender-score over parts both have.
    let mut ratios: HashMap<(Kind, Vendor), Vec<f64>> = HashMap::new();
    let mut missing = Vec::new();
    for ((kind, vendor, name), score) in medians {
        let matches = match kind {
            Kind::Cpu => catalog.match_cpus(&name),
            Kind::Gpu => catalog.match_gpus(&name),
        };
        match matches.as_slice() {
            [entry] => ratios
                .entry((kind, vendor))
                .or_default()
                .push(f64::from(entry.score) / score),
            [] => missing.push((kind, vendor, name, score)),
            _ => {} // ambiguous name
        }
    }

    let mut gaps = GapScores::default();
    for (kind, vendor, name, score) in missing {
        let Some(anchors) = ratios
            .get(&(kind, vendor))
            .filter(|r| r.len() >= MIN_ANCHORS)
        else {
            continue;
        };
        let entry = CatalogEntry {
            name,
            vendor,
            score: (score * median(anchors.clone())).round() as u32,
            vram_mb: None,
            aliases: Vec::new(),
            pci: Vec::new(),
        };
        match kind {
            Kind::Cpu => gaps.cpus.push(entry),
            Kind::Gpu => gaps.gpus.push(entry),
        }
    }
    gaps.cpus.sort_by(|a, b| a.name.cmp(&b.name));
    gaps.gpus.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(gaps)
}

/// The device a v4 record benchmarked and its score, if it ran all three
/// scenes on one device through a backend we use.
fn benchmark(record: &Record) -> Option<((Kind, Vendor, String), f64)> {
    if record.schema_version != "v4" {
        return None;
    }
    let first = record.data.first()?;
    let [device] = first.device_info.compute_devices.as_slice() else {
        return None;
    };
    let (kind, vendor) = match first.device_info.device_type.as_str() {
        "CUDA" => (Kind::Gpu, Vendor::Nvidia),
        "HIP" => (Kind::Gpu, Vendor::Amd),
        "ONEAPI" => (Kind::Gpu, Vendor::Intel),
        "CPU" if device.name.contains("AMD") => (Kind::Cpu, Vendor::Amd),
        "CPU" if device.name.contains("Intel") => (Kind::Cpu, Vendor::Intel),
        _ => return None,
    };
    let mut score = 0.0;
    for scene in SCENES {
        let run = record.data.iter().find(|run| run.scene.label == scene)?;
        score += run.stats.samples_per_minute?;
    }
    Some(((kind, vendor, device.name.clone()), score))
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}
