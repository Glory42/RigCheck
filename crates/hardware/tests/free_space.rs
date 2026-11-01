//! Tests for `free_space(path)`. Free space can't be faked, so these compare
//! against `df` on a real temp dir.

use std::path::Path;
use std::process::Command;

use rigcheck_hardware::free_space;
use tempfile::TempDir;

#[test]
fn matches_what_df_reports() {
    let dir = TempDir::new().unwrap();
    let df = Command::new("df")
        .args(["--output=avail", "-B1"])
        .arg(dir.path())
        .output()
        .unwrap();
    let df_bytes: u64 = String::from_utf8(df.stdout)
        .unwrap()
        .lines()
        .nth(1)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    let free = free_space(dir.path()).expect("free space");

    // Other programs can write to the disk between the two calls.
    assert!(
        free.abs_diff(df_bytes) < 64 << 20,
        "free_space {free} vs df {df_bytes}"
    );
}

#[test]
fn is_unknown_for_a_missing_folder() {
    assert_eq!(free_space(Path::new("/nonexistent/rigcheck")), None);
}
