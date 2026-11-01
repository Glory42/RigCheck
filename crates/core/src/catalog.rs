//! The hardware table: known CPUs and GPUs with scores, and matching the
//! names requirements and /proc use to entries (ROADMAP §8.2 steps 2–5).

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::matcher::{HardwareRef, Vendor};

/// A known CPU or GPU.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CatalogEntry {
    /// Display name, e.g. "GeForce GTX 1060 6GB".
    pub name: String,
    pub vendor: Vendor,
    pub score: u32,
    /// GPUs with their own memory.
    #[serde(default)]
    pub vram_mb: Option<u32>,
    /// Extra spellings to match, besides the name.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// PCI IDs as "vendor:device" hex, e.g. "10de:1c03". GPUs only.
    #[serde(default)]
    pub pci: Vec<String>,
}

impl CatalogEntry {
    /// What the matcher compares.
    pub fn to_ref(&self) -> HardwareRef {
        HardwareRef {
            vendor: self.vendor,
            score: self.score,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    cpus: Vec<CatalogEntry>,
    cpu_keys: HashMap<String, usize>,
    gpus: Vec<CatalogEntry>,
    gpu_keys: HashMap<String, usize>,
}

impl Catalog {
    pub fn new(cpus: Vec<CatalogEntry>, gpus: Vec<CatalogEntry>) -> Catalog {
        Catalog {
            cpu_keys: keys(&cpus),
            cpus,
            gpu_keys: keys(&gpus),
            gpus,
        }
    }

    /// Reads the format of `data/hardware.toml`.
    pub fn from_toml(text: &str) -> Result<Catalog, toml::de::Error> {
        #[derive(Deserialize)]
        struct File {
            #[serde(default)]
            cpu: Vec<CatalogEntry>,
            #[serde(default)]
            gpu: Vec<CatalogEntry>,
        }
        let file: File = toml::from_str(text)?;
        Ok(Catalog::new(file.cpu, file.gpu))
    }

    pub fn cpus(&self) -> &[CatalogEntry] {
        &self.cpus
    }

    pub fn gpus(&self) -> &[CatalogEntry] {
        &self.gpus
    }

    /// Every CPU the text names that the table knows, in order. Also finds
    /// /proc/cpuinfo names like "Intel(R) Core(TM) i5-8400 CPU @ 2.80GHz".
    pub fn match_cpus(&self, text: &str) -> Vec<&CatalogEntry> {
        matches(text, &self.cpus, &self.cpu_keys)
    }

    /// The GPU with this PCI ID, the exact way to identify a detected GPU.
    pub fn gpu_by_pci(&self, vendor_id: u16, device_id: u16) -> Option<&CatalogEntry> {
        let id = format!("{vendor_id:04x}:{device_id:04x}");
        self.gpus.iter().find(|gpu| gpu.pci.contains(&id))
    }

    /// Every GPU the text names that the table knows, in order.
    pub fn match_gpus(&self, text: &str) -> Vec<&CatalogEntry> {
        matches(text, &self.gpus, &self.gpu_keys)
    }
}

/// Lookup keys for each entry: its name and aliases, normalized.
fn keys(entries: &[CatalogEntry]) -> HashMap<String, usize> {
    let mut keys = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        for name in std::iter::once(&entry.name).chain(&entry.aliases) {
            keys.insert(normalize(name, false), index);
        }
    }
    keys
}

fn matches<'a>(
    text: &str,
    entries: &'a [CatalogEntry],
    keys: &HashMap<String, usize>,
) -> Vec<&'a CatalogEntry> {
    static ALTERNATIVES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\s*(?:/|,|;|\||\bor\b)\s*").unwrap());
    let mut found: Vec<&CatalogEntry> = Vec::new();
    for token in ALTERNATIVES.split(&strip_parentheses(text)) {
        let index = keys
            .get(&normalize(token, false))
            .or_else(|| keys.get(&normalize(token, true)));
        if let Some(entry) = index.map(|&index| &entries[index])
            && !found.contains(&entry)
        {
            found.push(entry);
        }
    }
    found
}

/// Words that don't tell models apart.
const NOISE: &[&str] = &[
    "nvidia",
    "geforce",
    "amd",
    "ati",
    "radeon",
    "intel",
    "core",
    "processor",
    "cpu",
    "graphics",
    "card",
    "better",
    "equivalent",
    "higher",
    "series",
    "with",
    "and",
    "similar",
];

/// A comparable key: "Intel I5 4690" and "Core i5-4690" both become "i54690".
/// `drop_sizes` also drops memory sizes, so "RX 580 8GB" finds "RX 580".
fn normalize(text: &str, drop_sizes: bool) -> String {
    static SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+(gb|mb)$").unwrap());
    static SPACED_SIZE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(\d)\s+(gb|mb)\b").unwrap());
    let text = strip_parentheses(&text.to_lowercase());
    let text = SPACED_SIZE.replace_all(&text, "$1$2"); // "4 gb" → "4gb"
    let text = text.split('@').next().unwrap_or_default(); // "i5-2500K@3.3GHz"
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty() && !NOISE.contains(word))
        .filter(|word| !(drop_sizes && SIZE.is_match(word)))
        .collect()
}

/// Drops "(...)" asides like "(4GB+ of VRAM)" or "(R)".
fn strip_parentheses(text: &str) -> String {
    static ASIDE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());
    ASIDE.replace_all(text, " ").into_owned()
}
