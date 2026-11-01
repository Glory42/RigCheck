//! Compares a game's requirements with the user's specs (ROADMAP §8.3).

/// What a game asks for. Either tier can be missing from the store page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameRequirements {
    pub minimum: Option<Requirement>,
    pub recommended: Option<Requirement>,
}

/// One tier of requirements. `None` means the store page doesn't say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Requirement {
    pub ram_mb: Option<u32>,
    pub vram_mb: Option<u32>,
    pub storage_mb: Option<u32>,
    /// DirectX version, e.g. 11 or 12.
    pub directx: Option<u8>,
    /// Alternatives, as in "i5-4460 or FX-6300". Empty if none matched.
    pub cpus: Vec<HardwareRef>,
    /// Alternatives, as in "GTX 1060 or RX 580". Empty if none matched.
    pub gpus: Vec<HardwareRef>,
}

/// A CPU or GPU found in the hardware table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HardwareRef {
    pub vendor: Vendor,
    /// Performance score; higher is faster. Comparable within CPUs or GPUs.
    pub score: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Amd,
    Intel,
    Nvidia,
}

/// The user's machine, already resolved to numbers the matcher compares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Specs {
    pub ram_mb: Option<u32>,
    /// `None` for GPUs that share system RAM, or when unknown.
    pub vram_mb: Option<u32>,
    /// Free space on the Steam library with the most room.
    pub free_storage_mb: Option<u32>,
    /// Highest Vulkan version the gaming GPU supports, as (major, minor).
    pub vulkan: Option<(u32, u32)>,
    /// The gaming GPU runs on the open-source `nouveau` driver.
    pub nouveau: bool,
    pub cpu: Option<HardwareRef>,
    pub gpu: Option<HardwareRef>,
}

/// Linux-specific facts about a game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinuxInfo {
    /// `None` if the game has no anti-cheat.
    pub anticheat: Option<AntiCheat>,
    /// `None` if ProtonDB has too few reports.
    pub protondb: Option<ProtonTier>,
    /// Has a Linux build, so Proton isn't needed.
    pub native: bool,
}

/// ProtonDB's rating for running the Windows version through Proton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtonTier {
    Platinum,
    Gold,
    Silver,
    Bronze,
    Borked,
}

/// AreWeAntiCheatYet's status for a game's anti-cheat on Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiCheat {
    Supported,
    Running,
    Planned,
    Broken,
    Denied,
}

/// Something worth telling the user that doesn't change the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    /// `nouveau` runs most 3D games poorly.
    Nouveau,
}

/// Why a game can't run on Linux whatever the hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocker {
    AntiCheat,
    /// ProtonDB rates the game "Borked".
    Proton,
}

/// How well a game should run. Variants go from best to worst, so the
/// derived `Ord` makes `max()` the worst level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Recommended,
    Minimum,
    Below,
    /// Not enough data to say.
    Unknown,
    /// Can't run on Linux whatever the hardware. See [`Verdict::blocker`].
    Blocked,
}

/// How much of the verdict rests on data rather than gaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// Every component was compared.
    High,
    /// Some components were compared.
    Partial,
    /// Nothing could be compared.
    Low,
}

/// A part of the machine that requirements talk about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// DirectX through Proton, which needs Vulkan.
    GraphicsApi,
    Cpu,
    Gpu,
    Ram,
    Storage,
    Vram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// The worst component's level.
    pub overall: Level,
    /// The component holding the verdict down, if any.
    pub bottleneck: Option<Component>,
    pub confidence: Confidence,
    pub blocker: Option<Blocker>,
    pub warnings: Vec<Warning>,
}

pub fn evaluate(game: &GameRequirements, specs: &Specs, linux: &LinuxInfo) -> Verdict {
    let tiers = |need: &dyn Fn(&Requirement) -> Option<u32>| {
        (
            game.minimum.as_ref().and_then(need),
            game.recommended.as_ref().and_then(need),
        )
    };

    // Usable RAM is below installed RAM: the kernel and an iGPU keep some.
    let (min, rec) = tiers(&|r| r.ram_mb);
    let ram = compare(specs.ram_mb.map(|mb| mb + mb / 10), min, rec);

    let (min, rec) = tiers(&|r| r.storage_mb);
    let storage = compare(specs.free_storage_mb, min, rec);

    let (min, rec) = tiers(&|r| r.vram_mb);
    let vram = compare(specs.vram_mb, min, rec);

    let (min, rec) = tiers(&|r| needed_score(&r.cpus, specs.cpu));
    let cpu = compare(specs.cpu.map(|cpu| cpu.score), min, rec);

    let (min, rec) = tiers(&|r| needed_score(&r.gpus, specs.gpu));
    let gpu = compare(specs.gpu.map(|gpu| gpu.score), min, rec);

    // Native games don't go through Proton; their DirectX line is for Windows.
    let api = if linux.native {
        None
    } else {
        graphics_api(game, specs)
    };

    let components = [
        (Component::GraphicsApi, api),
        (Component::Cpu, cpu),
        (Component::Gpu, gpu),
        (Component::Ram, ram),
        (Component::Vram, vram),
        (Component::Storage, storage),
    ];
    let known: Vec<(Component, Level)> = components
        .into_iter()
        .filter_map(|(component, level)| Some((component, level?)))
        .collect();
    // Confidence rests on the core three; VRAM doesn't apply to iGPUs.
    let core = [Component::Cpu, Component::Gpu, Component::Ram];
    let compared = known.iter().filter(|(c, _)| core.contains(c)).count();
    let confidence = match compared {
        0 => Confidence::Low,
        3 => Confidence::High,
        _ => Confidence::Partial,
    };
    let worst = known.into_iter().max_by_key(|(_, level)| *level);
    let blocker = if matches!(linux.anticheat, Some(AntiCheat::Denied | AntiCheat::Broken)) {
        Some(Blocker::AntiCheat)
    } else if !linux.native && linux.protondb == Some(ProtonTier::Borked) {
        Some(Blocker::Proton)
    } else {
        None
    };
    Verdict {
        overall: match blocker {
            Some(_) => Level::Blocked,
            None => worst.map_or(Level::Unknown, |(_, level)| level),
        },
        bottleneck: worst
            .filter(|(_, level)| *level != Level::Recommended)
            .map(|(component, _)| component),
        confidence,
        blocker,
        warnings: specs
            .nouveau
            .then_some(Warning::Nouveau)
            .into_iter()
            .collect(),
    }
}

/// Proton runs DirectX on Vulkan: DXVK (DX9–11) and VKD3D-Proton (DX12) both
/// need Vulkan 1.3.
fn graphics_api(game: &GameRequirements, specs: &Specs) -> Option<Level> {
    let tiers = [&game.minimum, &game.recommended];
    tiers
        .iter()
        .flat_map(|tier| tier.as_ref()?.directx)
        .next()?;
    let vulkan = specs.vulkan?;
    Some(if vulkan >= (1, 3) {
        Level::Recommended
    } else {
        Level::Below
    })
}

/// The score to beat among alternatives: the one from the user's vendor if
/// listed, since publishers pair cards they consider equal; else the lowest.
fn needed_score(alternatives: &[HardwareRef], have: Option<HardwareRef>) -> Option<u32> {
    let same_vendor = have.and_then(|have| alternatives.iter().find(|a| a.vendor == have.vendor));
    same_vendor
        .or_else(|| alternatives.iter().min_by_key(|a| a.score))
        .map(|a| a.score)
}

/// Where `have` falls against the tiers the game lists. `None` if either
/// side is unknown.
fn compare(have: Option<u32>, minimum: Option<u32>, recommended: Option<u32>) -> Option<Level> {
    let have = have?;
    match (minimum, recommended) {
        (None, None) => None,
        (_, Some(recommended)) if have >= recommended => Some(Level::Recommended),
        (Some(minimum), _) if have < minimum => Some(Level::Below),
        // Meets minimum, or misses a recommended tier that's the only one listed.
        _ => Some(Level::Minimum),
    }
}
