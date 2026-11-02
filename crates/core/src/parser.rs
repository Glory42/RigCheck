//! Reads one tier of Steam's requirement HTML (ROADMAP §8.2).

use std::sync::LazyLock;

use regex::Regex;

/// What a requirement tier says, before hardware names are matched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedTier {
    /// As written, e.g. "Core i7-6700 or Ryzen 5 1600". Matched to hardware later.
    pub cpu: Option<String>,
    /// As written, e.g. "GeForce GTX 1060 6GB or Radeon RX 580 8GB".
    pub gpu: Option<String>,
    pub ram_mb: Option<u32>,
    /// The smallest video memory the graphics text mentions.
    pub vram_mb: Option<u32>,
    pub storage_mb: Option<u32>,
    /// Major version: "Version 9.0c" → 9.
    pub directx: Option<u8>,
}

pub fn parse(html: &str) -> ParsedTier {
    let fields = fields(html);
    // Steam pages name the same field in several ways.
    let field = |labels: &[&str]| {
        fields
            .iter()
            .find(|(label, _)| labels.contains(&label.as_str()))
            .map(|(_, value)| value.as_str())
    };
    let gpu = field(&["graphics", "video card", "video"]);
    ParsedTier {
        cpu: field(&["processor"]).map(str::to_owned),
        gpu: gpu.map(str::to_owned),
        vram_mb: gpu.and_then(|text| sizes_mb(text).min()),
        ram_mb: field(&["memory"])
            .and_then(size_mb)
            .or_else(|| unlabelled_ram_mb(html)),
        storage_mb: field(&["storage", "hard drive", "hard disk space", "disk space"])
            .and_then(size_mb),
        directx: field(&["directx", "direct x", "directx®"]).and_then(directx_version),
    }
}

/// `("label", "value")` for each `Label: value` line, labels lowercased.
fn fields(html: &str) -> Vec<(String, String)> {
    plain_lines(html)
        .into_iter()
        .filter_map(|line| {
            let (label, value) = line.split_once(':')?;
            Some((label.trim().to_lowercase(), value.trim().to_owned()))
        })
        .collect()
}

/// Steam's requirement HTML as plain text, one line per item, the way the
/// store page reads: what the game page shows next to the parsed values.
pub fn requirement_text(html: &str) -> String {
    plain_lines(html).join("\n")
}

/// The HTML's lines with tags removed and entities decoded, blank ones dropped.
fn plain_lines(html: &str) -> Vec<String> {
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
    static BREAK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)<(br|li|/li|p|/p)\b[^>]*>").unwrap());

    BREAK
        .replace_all(html, "\n")
        .lines()
        .map(|line| {
            decode_entities(&TAG.replace_all(line, ""))
                .trim()
                .to_owned()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

/// The entities Steam's requirement HTML uses. Decoded after tags are
/// stripped, so a decoded `<` can't start a tag.
fn decode_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&reg;", "®")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// RAM on old pages without labels: "96mb ram", "512MB RAM".
fn unlabelled_ram_mb(html: &str) -> Option<u32> {
    static RAM: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(\d+(?:\.\d+)?\s*(?:gb|mb))\s+(?:of\s+)?ram\b").unwrap());
    size_mb(&RAM.captures(html)?[1])
}

/// The first version number in a DirectX field: "DirectX® version 9.0c" → 9.
fn directx_version(text: &str) -> Option<u8> {
    static VERSION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b(\d{1,2})(?:\.\d)?").unwrap());
    VERSION.captures(text)?[1].parse().ok()
}

/// The first size in `text`, in MB: "8 GB RAM" → 8192, "7.5 GB" → 7680.
fn size_mb(text: &str) -> Option<u32> {
    sizes_mb(text).next()
}

/// Every size in `text`, in MB.
fn sizes_mb(text: &str) -> impl Iterator<Item = u32> {
    static SIZE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*\+?\s*(gb|mb)").unwrap());
    SIZE.captures_iter(text).filter_map(|caps| {
        let number: f64 = caps[1].parse().ok()?;
        let mb = match caps[2].to_lowercase().as_str() {
            "gb" => number * 1024.0,
            _ => number,
        };
        Some(mb.round() as u32)
    })
}
