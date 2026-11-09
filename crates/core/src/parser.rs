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
    let gpu = gpu_field(&fields).or_else(|| unlabelled(looks_like_gpu).map(str::to_owned));
    ParsedTier {
        cpu: cpu.map(str::to_owned),
        vram_mb: gpu.as_deref().and_then(|text| sizes_mb(text).min()),
        gpu,
        ram_mb: field(&["memory", "ram", "system memory"])
            .and_then(|text| size_mb(text).or_else(|| unitless_ram_mb(text)))
            .or_else(|| unlabelled_ram_mb(html))
            .or_else(|| items.iter().find_map(|item| memory_item_mb(item))),
        storage_mb: field(&["storage", "hard drive", "hard disk space", "disk space"])
            .and_then(size_mb),
        directx: field(&["directx", "direct x", "directx®"]).and_then(directx_version),
    }
}

/// `("label", "value")` for each `Label: value` line, labels lowercased
/// and without a qualifier: "Video Card (ATI)" → "video card".
fn fields(html: &str) -> Vec<(String, String)> {
    static QUALIFIER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());
    plain_lines(html)
        .into_iter()
        .filter_map(|line| {
            let (label, value) = line.split_once(':')?;
            let label = QUALIFIER.replace_all(label, "").trim().to_lowercase();
            Some((label, value.trim().to_owned()))
        })
        .collect()
}

/// The graphics field. Pages that give each vendor its own line ("Video
/// Card (ATI): Radeon HD 5450", "Video Card (NVidia): GeForce 260") list
/// alternatives, so a run of graphics lines joins with "or". Only a run:
/// a page holding both tiers repeats the label further down.
fn gpu_field(fields: &[(String, String)]) -> Option<String> {
    const LABELS: [&str; 5] = ["graphics", "graphics card", "video card", "video", "gpu"];
    let is_gpu = |(label, value): &(String, String)| {
        LABELS.contains(&label.as_str()) && !is_placeholder(value)
    };
    let start = fields.iter().position(is_gpu)?;
    let run: Vec<&str> = fields[start..]
        .iter()
        .take_while(|field| is_gpu(field))
        .map(|(_, value)| value.as_str())
        .collect();
    Some(run.join(" or "))
}

/// A value that stands for "not stated yet": "Processor: TBD".
fn is_placeholder(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "" | "tbd" | "tba" | "n/a" | "-" | "?"
    )
}

/// Requirements written without labels, item by item: each line that
/// doesn't start with a "Label:" (after a leading "Minimum:" or
/// "Recommended:"), split into items. Quake: "Intel Core i5-3570 @3.4 GHz
/// or AMD Ryzen 3 1300X"; Counter-Strike: "500 mhz processor, 96mb ram,
/// 16mb video card, …"; Geometry Wars ends its list with "WEI Rating: 4.0".
fn unlabelled_items(html: &str) -> Vec<String> {
    static HEADING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(minimum|recommended|required)\s*:\s*").unwrap());
    static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^,;:]+:").unwrap());
    plain_lines(html)
        .into_iter()
        .map(|line| HEADING.replace(&line, "").into_owned())
        .filter(|line| !LABEL.is_match(line))
        .flat_map(|line| split_items(&line))
        .collect()
}

/// A line's items: split at ", ", "; " and ". " outside parentheses, so
/// "Graphics adaptor (ATI, NVIDIA, Intel, etc.) with 32 mb" stays whole.
/// Gish separates with semicolons, Ghost Master with sentences. A line
/// whose parentheses don't balance (Return to Castle Wolfenstein opens one
/// and never closes it) splits everywhere.
fn split_items(line: &str) -> Vec<String> {
    let balanced = line.matches('(').count() == line.matches(')').count();
    let mut items = Vec::new();
    let mut item = String::new();
    let mut depth = 0usize;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' if balanced => depth += 1,
            ')' if balanced => depth = depth.saturating_sub(1),
            ',' | ';' | '.' if depth == 0 && chars.peek() == Some(&' ') => {
                items.push(std::mem::take(&mut item));
                continue;
            }
            _ => {}
        }
        item.push(c);
    }
    items.push(item);
    items
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

/// "1.2 GHz Pentium® III processor", "1 GHz 32-bit (x86) or 64-bit (x64)
/// CPU". Not Steam's "Requires a 64-bit processor and operating system".
fn looks_like_cpu(item: &str) -> bool {
    static FAMILY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(processor|cpu|core\s*i[3579]|core\s*2|ryzen|athlon|pentium|phenom|celeron|xeon|amd\s+fx)\b",
        )
        .unwrap()
    });
    !item.contains("operating system") && FAMILY.is_match(item) && !looks_like_gpu(item)
}

/// "64 MB video card", "NVIDIA GeForce GTX 650 TI (2GB) or AMD HD 7750",
/// "64 MB video memory", "3D Accelerator card". Not "DirectX 8 compatible
/// sound- and graphics card": a sound card before the graphics is the
/// sound requirement.
fn looks_like_gpu(item: &str) -> bool {
    static GPU: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(video card|video memory|graphics card|graphics adapt[eo]r|3-?d (hardware )?accelerator|\bgeforce\b|\bradeon\b|\bnvidia\b|\bgtx\b|\brtx\b|intel\s+u?hd\b|\biris\b)",
        )
        .unwrap()
    });
    static SOUND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)sound").unwrap());
    GPU.find(item).is_some_and(|gpu| {
        SOUND
            .find(item)
            .is_none_or(|sound| gpu.start() < sound.start())
    })
}

/// Steam's requirement HTML as plain text, one line per item, the way the
/// store page reads: what the game page shows next to the parsed values.
pub fn requirement_text(html: &str) -> String {
    plain_lines(html).join("\n")
}

/// The HTML's lines with tags removed, entities decoded and full-width
/// digits made ASCII ("４ GB RAM"), blank ones dropped.
fn plain_lines(html: &str) -> Vec<String> {
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
    static BREAK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)<(br|li|/li|p|/p)\b[^>]*>").unwrap());

    BREAK
        .replace_all(html, "\n")
        .lines()
        .map(|line| {
            ascii_digits(&decode_entities(&TAG.replace_all(line, "")))
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

/// "４" → "4": some pages type numbers in full-width.
fn ascii_digits(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '０'..='９' => char::from(b'0' + (c as u32 - '０' as u32) as u8),
            c => c,
        })
        .collect()
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

/// "Memory: 256 RAM": a number without a unit. Pages of the 256 MB era
/// mean megabytes; "8 RAM" can only mean gigabytes.
fn unitless_ram_mb(text: &str) -> Option<u32> {
    static BARE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\d+)\s*ram\b").unwrap());
    let number: u32 = BARE.captures(text)?[1].parse().ok()?;
    Some(if number >= 64 { number } else { number * 1024 })
}

/// An unlabelled item that starts with the system memory: "256MB System
/// Memory." (Just Cause), "256MB of memory" (Gish). Not "64 MB video memory".
fn memory_item_mb(item: &str) -> Option<u32> {
    static MEMORY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(\d+(?:\.\d+)?\s*(?:gb|mb))\s+(?:of\s+)?(?:system\s+)?memory\b").unwrap()
    });
    size_mb(&MEMORY.captures(item)?[1])
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

/// Every size in `text`, in MB: "8 GB", "1+ gigs", "16(Dual-channel) GB".
fn sizes_mb(text: &str) -> impl Iterator<Item = u32> {
    static SIZE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(\d+(?:\.\d+)?)\s*\+?\s*(?:\([a-z -]*\)\s*)?(gigabytes?|gigs?|gb|megabytes?|mb)",
        )
        .unwrap()
    });
    SIZE.captures_iter(text).filter_map(|caps| {
        let number: f64 = caps[1].parse().ok()?;
        let mb = if caps[2].to_lowercase().starts_with('g') {
            number * 1024.0
        } else {
            number
        };
        Some(mb.round() as u32)
    })
}
