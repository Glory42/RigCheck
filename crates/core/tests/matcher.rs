//! Tests for `evaluate(game, specs, linux)`, which turns requirements and the
//! user's specs into a verdict (ROADMAP §2 "Verdict levels", §8.3).

use rigcheck_core::{
    AntiCheat, Blocker, Component, Confidence, GameRequirements, HardwareRef, Level, LinuxInfo,
    ProtonTier, Reported, Requirement, Specs, Vendor, Warning, evaluate,
};

/// A game that only states RAM, in MB.
fn needs_ram(minimum: u32, recommended: u32) -> GameRequirements {
    GameRequirements {
        minimum: Some(Requirement {
            ram_mb: Some(minimum),
            ..Requirement::default()
        }),
        recommended: Some(Requirement {
            ram_mb: Some(recommended),
            ..Requirement::default()
        }),
    }
}

fn with_ram(ram_mb: u32) -> Specs {
    Specs {
        ram_mb: Some(ram_mb),
        ..Specs::default()
    }
}

#[test]
fn meeting_recommended_specs_is_recommended() {
    let verdict = evaluate(
        &needs_ram(8192, 16384),
        &with_ram(32768),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Recommended);
}

#[test]
fn meeting_only_minimum_specs_is_minimum() {
    let verdict = evaluate(
        &needs_ram(8192, 16384),
        &with_ram(12288),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Minimum);
}

#[test]
fn falling_short_of_minimum_is_below_and_names_the_component() {
    let verdict = evaluate(
        &needs_ram(8192, 16384),
        &with_ram(4096),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Below);
    assert_eq!(verdict.bottleneck, Some(Component::Ram));
}

/// The kernel and the iGPU keep part of installed RAM, so a 16 GB laptop
/// reports 15,746,968 kB (15,377 MiB) in /proc/meminfo.
#[test]
fn a_16_gb_laptop_meets_a_16_gb_requirement() {
    let verdict = evaluate(
        &needs_ram(8192, 16384),
        &with_ram(15377),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Recommended);
}

#[test]
fn a_14_gb_machine_does_not_meet_16_gb() {
    let verdict = evaluate(
        &needs_ram(8192, 16384),
        &with_ram(14336),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Minimum);
}

/// A game whose tiers each list GPU alternatives ("GTX 1060 or RX 580").
fn needs_gpu(minimum: &[HardwareRef], recommended: &[HardwareRef]) -> GameRequirements {
    GameRequirements {
        minimum: Some(Requirement {
            gpus: minimum.to_vec(),
            ..Requirement::default()
        }),
        recommended: Some(Requirement {
            gpus: recommended.to_vec(),
            ..Requirement::default()
        }),
    }
}

fn hw(vendor: Vendor, score: u32) -> HardwareRef {
    HardwareRef { vendor, score }
}

fn with_gpu(gpu: HardwareRef) -> Specs {
    Specs {
        gpu: Some(gpu),
        ..Specs::default()
    }
}

#[test]
fn a_gpu_slower_than_the_minimum_is_the_bottleneck() {
    let game = needs_gpu(&[hw(Vendor::Nvidia, 100)], &[hw(Vendor::Nvidia, 200)]);

    let verdict = evaluate(&game, &with_gpu(hw(Vendor::Amd, 60)), &LinuxInfo::default());

    assert_eq!(verdict.overall, Level::Below);
    assert_eq!(verdict.bottleneck, Some(Component::Gpu));
}

#[test]
fn gpus_are_compared_with_the_same_vendors_alternative() {
    // "GTX 1060 or RX 580", where our scores put the RX 580 higher.
    let game = needs_gpu(
        &[hw(Vendor::Nvidia, 80), hw(Vendor::Amd, 100)],
        &[hw(Vendor::Nvidia, 160), hw(Vendor::Amd, 200)],
    );

    let verdict = evaluate(&game, &with_gpu(hw(Vendor::Amd, 90)), &LinuxInfo::default());

    assert_eq!(verdict.overall, Level::Below);
}

#[test]
fn without_a_same_vendor_alternative_the_lowest_one_counts() {
    let game = needs_gpu(
        &[hw(Vendor::Nvidia, 80), hw(Vendor::Amd, 100)],
        &[hw(Vendor::Nvidia, 160), hw(Vendor::Amd, 200)],
    );

    let verdict = evaluate(
        &game,
        &with_gpu(hw(Vendor::Intel, 90)),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Minimum);
}

#[test]
fn a_cpu_slower_than_the_minimum_is_the_bottleneck() {
    let game = GameRequirements {
        minimum: Some(Requirement {
            cpus: vec![hw(Vendor::Intel, 100), hw(Vendor::Amd, 90)],
            ..Requirement::default()
        }),
        recommended: None,
    };
    let specs = Specs {
        cpu: Some(hw(Vendor::Amd, 70)),
        ..Specs::default()
    };

    let verdict = evaluate(&game, &specs, &LinuxInfo::default());

    assert_eq!(verdict.overall, Level::Below);
    assert_eq!(verdict.bottleneck, Some(Component::Cpu));
}

#[test]
fn a_game_without_requirements_is_unknown() {
    let verdict = evaluate(
        &GameRequirements::default(),
        &with_ram(16384),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Unknown);
    assert_eq!(verdict.confidence, Confidence::Low);
}

#[test]
fn a_verdict_from_some_components_has_partial_confidence() {
    // The store page lists RAM, but its CPU and GPU weren't recognized.
    let verdict = evaluate(
        &needs_ram(8192, 16384),
        &with_ram(32768),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Recommended);
    assert_eq!(verdict.confidence, Confidence::Partial);
}

#[test]
fn anti_cheat_that_denies_linux_blocks_the_game() {
    let linux = LinuxInfo {
        anticheat: Some(AntiCheat::Denied),
        ..LinuxInfo::default()
    };

    let verdict = evaluate(&needs_ram(8192, 16384), &with_ram(32768), &linux);

    assert_eq!(verdict.overall, Level::Blocked);
    assert_eq!(verdict.blocker, Some(Blocker::AntiCheat));
}

#[test]
fn anti_cheat_that_is_broken_on_linux_blocks_the_game() {
    let linux = LinuxInfo {
        anticheat: Some(AntiCheat::Broken),
        ..LinuxInfo::default()
    };

    let verdict = evaluate(&needs_ram(8192, 16384), &with_ram(32768), &linux);

    assert_eq!(verdict.blocker, Some(Blocker::AntiCheat));
}

#[test]
fn a_borked_protondb_rating_blocks_the_game() {
    let linux = LinuxInfo {
        protondb: Some(ProtonTier::Borked),
        ..LinuxInfo::default()
    };

    let verdict = evaluate(&needs_ram(8192, 16384), &with_ram(32768), &linux);

    assert_eq!(verdict.overall, Level::Blocked);
    assert_eq!(verdict.blocker, Some(Blocker::Proton));
}

#[test]
fn a_borked_proton_rating_does_not_block_a_native_linux_game() {
    let linux = LinuxInfo {
        native: true,
        protondb: Some(ProtonTier::Borked),
        ..LinuxInfo::default()
    };

    let verdict = evaluate(&needs_ram(8192, 16384), &with_ram(32768), &linux);

    assert_eq!(verdict.overall, Level::Recommended);
    assert_eq!(verdict.blocker, None);
}

fn needs_vram(minimum: u32, recommended: u32) -> GameRequirements {
    GameRequirements {
        minimum: Some(Requirement {
            vram_mb: Some(minimum),
            ..Requirement::default()
        }),
        recommended: Some(Requirement {
            vram_mb: Some(recommended),
            ..Requirement::default()
        }),
    }
}

#[test]
fn too_little_vram_is_the_bottleneck() {
    let specs = Specs {
        vram_mb: Some(2048),
        ..Specs::default()
    };

    let verdict = evaluate(&needs_vram(4096, 8192), &specs, &LinuxInfo::default());

    assert_eq!(verdict.overall, Level::Below);
    assert_eq!(verdict.bottleneck, Some(Component::Vram));
}

#[test]
fn shared_memory_gpus_skip_the_vram_check() {
    // An iGPU has no VRAM of its own, so Specs::vram_mb is None.
    let verdict = evaluate(
        &needs_vram(4096, 8192),
        &Specs::default(),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Unknown);
}

#[test]
fn igpu_users_get_high_confidence_when_cpu_gpu_and_ram_compare() {
    let tier = |cpu, gpu, ram_mb, vram_mb| Requirement {
        cpus: vec![hw(Vendor::Amd, cpu)],
        gpus: vec![hw(Vendor::Amd, gpu)],
        ram_mb: Some(ram_mb),
        vram_mb: Some(vram_mb),
        ..Requirement::default()
    };
    let game = GameRequirements {
        minimum: Some(tier(50, 50, 8192, 2048)),
        recommended: Some(tier(100, 100, 16384, 4096)),
    };
    let specs = Specs {
        cpu: Some(hw(Vendor::Amd, 120)),
        gpu: Some(hw(Vendor::Amd, 120)),
        ram_mb: Some(15377),
        vram_mb: None,
        ..Specs::default()
    };

    let verdict = evaluate(&game, &specs, &LinuxInfo::default());

    assert_eq!(verdict.overall, Level::Recommended);
    assert_eq!(verdict.confidence, Confidence::High);
}

#[test]
fn too_little_free_disk_space_is_the_bottleneck() {
    let tier = Requirement {
        storage_mb: Some(70_000),
        ..Requirement::default()
    };
    let game = GameRequirements {
        minimum: Some(tier.clone()),
        recommended: Some(tier),
    };
    let specs = Specs {
        free_storage_mb: Some(50_000),
        ..Specs::default()
    };

    let verdict = evaluate(&game, &specs, &LinuxInfo::default());

    assert_eq!(verdict.overall, Level::Below);
    assert_eq!(verdict.bottleneck, Some(Component::Storage));
}

fn needs_directx(version: u8) -> GameRequirements {
    let tier = Requirement {
        directx: Some(version),
        ..Requirement::default()
    };
    GameRequirements {
        minimum: Some(tier.clone()),
        recommended: Some(tier),
    }
}

fn with_vulkan(major: u32, minor: u32) -> Specs {
    Specs {
        vulkan: Some((major, minor)),
        ..Specs::default()
    }
}

#[test]
fn directx_games_need_vulkan_1_3_for_proton() {
    let verdict = evaluate(
        &needs_directx(11),
        &with_vulkan(1, 2),
        &LinuxInfo::default(),
    );

    assert_eq!(verdict.overall, Level::Below);
    assert_eq!(verdict.bottleneck, Some(Component::GraphicsApi));
}

#[test]
fn native_linux_games_skip_the_directx_check() {
    let linux = LinuxInfo {
        native: true,
        ..LinuxInfo::default()
    };

    let verdict = evaluate(&needs_directx(11), &with_vulkan(1, 2), &linux);

    assert_eq!(verdict.overall, Level::Unknown);
}

#[test]
fn the_nouveau_driver_adds_a_warning_without_changing_the_level() {
    let specs = Specs {
        ram_mb: Some(32768),
        nouveau: true,
        ..Specs::default()
    };

    let verdict = evaluate(&needs_ram(8192, 16384), &specs, &LinuxInfo::default());

    assert_eq!(verdict.overall, Level::Recommended);
    assert_eq!(verdict.warnings, [Warning::Nouveau]);
}

/// ROADMAP §2: Minimum means "at least one falls short of recommended". With no
/// recommended tier there's nothing to fall short of.
#[test]
fn meeting_the_only_tier_listed_is_recommended() {
    let game = GameRequirements {
        minimum: Some(Requirement {
            ram_mb: Some(256),
            ..Requirement::default()
        }),
        recommended: None,
    };

    let verdict = evaluate(&game, &with_ram(15377), &LinuxInfo::default());

    assert_eq!(verdict.overall, Level::Recommended);
}

#[test]
fn the_verdict_lists_each_compared_components_level() {
    let mut game = needs_gpu(&[hw(Vendor::Nvidia, 100)], &[hw(Vendor::Nvidia, 200)]);
    for (tier, ram_mb) in [(&mut game.minimum, 8192), (&mut game.recommended, 16384)] {
        tier.as_mut().unwrap().ram_mb = Some(ram_mb);
    }
    let specs = Specs {
        gpu: Some(hw(Vendor::Nvidia, 60)),
        ram_mb: Some(12288),
        ..Specs::default()
    };

    let verdict = evaluate(&game, &specs, &LinuxInfo::default());

    // CPU, VRAM, storage and the graphics API weren't stated or known.
    assert_eq!(
        verdict.components,
        [
            (Component::Gpu, Level::Below),
            (Component::Ram, Level::Minimum)
        ]
    );
}

/// ProtonDB players ran a game on GPUs scored `weakest` and up, `typical`
/// in the middle.
fn players_ran_gpus(weakest: u32, typical: u32) -> LinuxInfo {
    LinuxInfo {
        reported_gpu: Some(Reported {
            weakest_name: "Radeon Vega 8".to_owned(),
            weakest,
            typical,
        }),
        ..LinuxInfo::default()
    }
}

fn with_ram_and_gpu(ram_mb: u32, gpu: u32) -> Specs {
    Specs {
        gpu: Some(HardwareRef {
            vendor: Vendor::Amd,
            score: gpu,
        }),
        ..with_ram(ram_mb)
    }
}

#[test]
fn a_gpu_steam_doesnt_name_is_compared_with_what_players_ran_the_game_on() {
    let game = needs_ram(4096, 8192);
    let linux = players_ran_gpus(30, 100);
    let level = |gpu| {
        let verdict = evaluate(&game, &with_ram_and_gpu(16384, gpu), &linux);
        let level = verdict
            .components
            .iter()
            .find(|(component, _)| *component == Component::Gpu)
            .map(|(_, level)| *level);
        (level, verdict.reported)
    };

    assert_eq!(level(120), (Some(Level::Recommended), vec![Component::Gpu]));
    assert_eq!(level(100), (Some(Level::Recommended), vec![Component::Gpu]));
    assert_eq!(level(50), (Some(Level::Minimum), vec![Component::Gpu]));
    // Weaker than what players ran it on says nothing either way.
    assert_eq!(level(20), (None, vec![]));
}

#[test]
fn steams_own_requirement_comes_before_what_players_ran_it_on() {
    let mut game = needs_ram(4096, 8192);
    game.minimum.as_mut().unwrap().gpus = vec![HardwareRef {
        vendor: Vendor::Amd,
        score: 80,
    }];

    let verdict = evaluate(
        &game,
        &with_ram_and_gpu(16384, 50),
        &players_ran_gpus(10, 40),
    );

    assert_eq!(verdict.overall, Level::Below);
    assert_eq!(verdict.bottleneck, Some(Component::Gpu));
    assert!(verdict.reported.is_empty());
}

#[test]
fn parts_compared_with_players_reports_count_for_confidence() {
    let linux = LinuxInfo {
        reported_cpu: Some(Reported {
            weakest_name: "Core i3-6100".to_owned(),
            weakest: 60,
            typical: 150,
        }),
        ..players_ran_gpus(30, 100)
    };
    let specs = Specs {
        cpu: Some(HardwareRef {
            vendor: Vendor::Amd,
            score: 190,
        }),
        ..with_ram_and_gpu(16384, 50)
    };

    let verdict = evaluate(&needs_ram(4096, 8192), &specs, &linux);

    assert_eq!(verdict.confidence, Confidence::High);
    assert_eq!(verdict.overall, Level::Minimum);
    assert_eq!(verdict.bottleneck, Some(Component::Gpu));
    assert_eq!(verdict.reported, [Component::Cpu, Component::Gpu]);
}
