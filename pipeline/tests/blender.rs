//! Tests for filling gaps in the hardware table from Blender Open Data.
//!
//! Hand scores in data/hardware.toml win. Blender only scores parts the
//! table lacks, rescaled by the median our-score/Blender-score ratio of the 3
//! same-vendor parts both have whose Blender scores are closest. NVIDIA uses
//! CUDA runs (OptiX overweights RT cores).

use std::fs::File;
use std::io::BufReader;

use rigcheck_core::{Catalog, Vendor};
use rigcheck_pipeline::blender::gap_scores;

#[test]
fn a_gpu_missing_from_the_table_gets_a_rescaled_score() {
    let catalog = Catalog::from_toml(include_str!("../../data/hardware.toml")).unwrap();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../data/fixtures/blender.jsonl"
    );

    let gaps = gap_scores(BufReader::new(File::open(path).unwrap()), &catalog).unwrap();

    // Medians of 3 CUDA runs: GTX 1650 512.2311, RTX 2060 1137.3496,
    // RTX 3060 1350.2494, RTX 3070 2055.1828. Closest to the RTX 3050's
    // 823.8554: GTX 1650, RTX 2060, RTX 3060, with our/Blender ratios 0.1367,
    // 0.1231, 0.1296 → median 0.1296 → 823.8554 × 175/1350.2494 = 106.8 → 107.
    let [gpu] = gaps.gpus.as_slice() else {
        panic!("expected only the RTX 3050, got {:?}", gaps.gpus);
    };
    assert_eq!(gpu.name, "NVIDIA GeForce RTX 3050");
    assert_eq!(gpu.vendor, Vendor::Nvidia);
    assert_eq!(gpu.score, 107);
    assert!(gaps.cpus.is_empty());
}
