//! Tests for filling gaps in the hardware table from Blender Open Data.
//!
//! Hand scores in data/hardware.toml win. Blender only scores parts the
//! table lacks, rescaled per vendor by the median of our-score/Blender-score
//! over parts both have. NVIDIA uses CUDA runs (OptiX overweights RT cores).

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
    // RTX 3060 1350.2494, RTX 3070 2055.1828 → our/Blender ratios 0.1367,
    // 0.1231, 0.1296, 0.1265 → median 0.12806. RTX 3050: 823.8554 × 0.12806
    // = 105.5 → 106.
    let [gpu] = gaps.gpus.as_slice() else {
        panic!("expected only the RTX 3050, got {:?}", gaps.gpus);
    };
    assert_eq!(gpu.name, "NVIDIA GeForce RTX 3050");
    assert_eq!(gpu.vendor, Vendor::Nvidia);
    assert_eq!(gpu.score, 106);
    assert!(gaps.cpus.is_empty());
}
