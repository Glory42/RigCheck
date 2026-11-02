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
            .find(|(label, value)| labels.contains(&label.as_str()) && !is_placeholder(value))
            .map(|(_, value)| value.as_str())
    };
    // Pages without labels: the first item that reads like each part.
    let items = unlabelled_items(html);
    let unlabelled = |looks_like: fn(&str) -> bool| {
        items
            .iter()
            .find(|item| looks_like(item))
            .map(String::as_str)
    };
    let cpu = field(&["processor", "cpu"]).or_else(|| unlabelled(looks_like_cpu));
    let gpu = field(&["graphics", "graphics card", "video card", "video", "gpu"])
        .or_else(|| unlabelled(looks_like_gpu));
    ParsedTier {
        cpu: cpu.map(str::to_owned),
        gpu: gpu.map(str::to_owned),
        vram_mb: gpu.and_then(|text| sizes_mb(text).min()),
        ram_mb: field(&["memory", "ram", "system memory"])
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

/// A value that stands for "not stated yet": "Processor: TBD".
fn is_placeholder(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "" | "tbd" | "tba" | "n/a" | "-" | "?"
    )
}

/// Requirements written without labels, item by item: each line that has
/// no "Label:" (after a leading "Minimum:" or "Recommended:"), split at
/// commas. Quake: "Intel Core i5-3570 @3.4 GHz or AMD Ryzen 3 1300X";
/// Counter-Strike: "500 mhz processor, 96mb ram, 16mb video card, …".
fn unlabelled_items(html: &str) -> Vec<String> {
    static HEADING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(minimum|recommended|required)\s*:\s*").unwrap());
    plain_lines(html)
        .into_iter()
        .map(|line| HEADING.replace(&line, "").into_owned())
        .filter(|line| !line.contains(':'))
        .flat_map(|line| {
            line.split(", ")
                .map(|item| item.trim().to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|item| !item.is_empty())
        .collect()
}

/// "1.2 GHz Pentium® III processor", "Intel Core i5-3570 @3.4 GHz or …".
/// Not Steam's "Requires a 64-bit processor and operating system".
fn looks_like_cpu(item: &str) -> bool {
    static FAMILY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(processor|cpu|core\s*i[3579]|core\s*2|ryzen|athlon|pentium|phenom|celeron|xeon|amd\s+fx)\b",
        )
        .unwrap()
    });
    !item.contains("64-bit") && FAMILY.is_match(item) && !looks_like_gpu(item)
}

/// "64 MB video card", "NVIDIA GeForce GTX 650 TI (2GB) or AMD HD 7750".
fn looks_like_gpu(item: &str) -> bool {
    static GPU: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(video card|graphics card|\bgeforce\b|\bradeon\b|\bnvidia\b|\bgtx\b|\brtx\b|intel\s+u?hd\b|\biris\b)",
        )
        .unwrap()
    });
    let lower = item.to_lowercase();
    !lower.contains("sound") && GPU.is_match(item)
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

/// RAM on pages without a Memory line: "96mb ram", "512MB RAM", "8GB
/// System RAM", or "RAM: 16 GB" inside another field.
fn unlabelled_ram_mb(html: &str) -> Option<u32> {
    static RAM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
                r"(?i)(\d+(?:\.\d+)?\s*(?:gb|mb))\s+(?:of\s+)?(?:system\s+)?ram\b|\bram\s*:\s*(\d+(?:\.\d+)?\s*(?:gb|mb))",
            )
            .unwrap()
    });
    let caps = RAM.captures(html)?;
    size_mb(caps.get(1).or_else(|| caps.get(2))?.as_str())
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
