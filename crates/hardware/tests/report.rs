//! Tests for `Hardware::report()`, the text behind "Copy as text".

mod common;

use common::{detect_fixture, detect_fixture_without_programs};

#[test]
fn reports_a_single_gpu_machine() {
    let report = detect_fixture("amd-laptop").report();

    assert_eq!(
        report,
        "\
CPU:    AMD Ryzen 5 7530U with Radeon Graphics
RAM:    15.0 GiB
GPU:    AMD Radeon Graphics (RADV RENOIR) [1002:15e7], amdgpu, shared memory
        Vulkan 1.4.354, Mesa 26.2.6-arch1.1
Distro: Arch Linux
Kernel: 7.2.11-arch1-1
"
    );
}

#[test]
fn reports_every_gpu_of_a_hybrid_laptop() {
    let report = detect_fixture("hybrid-laptop").report();

    assert_eq!(
        report,
        "\
CPU:    Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz
RAM:    15.5 GiB
GPU:    Intel(R) UHD Graphics 630 (CFL GT2) [8086:3e9b], i915, shared memory
        Vulkan 1.3.289, Mesa 24.2.5
GPU:    NVIDIA GeForce GTX 1650 [10de:1f91], nvidia, 4.0 GiB VRAM
        Vulkan 1.4.312, 580.95.05
Distro: Arch Linux
Kernel: 6.11.5-arch1-1
"
    );
}

#[test]
fn says_when_vulkan_is_missing() {
    let report = detect_fixture_without_programs("amd-laptop").report();

    assert!(
        report.contains(
            "\
GPU:    Unknown GPU [1002:15e7], amdgpu, VRAM unknown
        Vulkan not detected (is vulkan-tools installed?)
"
        ),
        "{report}"
    );
}
