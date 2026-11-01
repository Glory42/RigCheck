//! Fixture helpers shared by the test files.
//!
//! Each fixture in `data/fixtures/hardware/` is a fake filesystem root plus
//! captured command output in `commands/<program>.txt`. A missing file means
//! the program isn't installed.

use std::fs;
use std::path::{Path, PathBuf};

use rigcheck_hardware::{Commands, Hardware, detect};

/// Replays a fixture's captured command output instead of running programs.
struct Captured(PathBuf);

impl Commands for Captured {
    fn run(&self, program: &str, _args: &[&str]) -> Option<String> {
        fs::read_to_string(self.0.join("commands").join(format!("{program}.txt"))).ok()
    }
}

/// A machine where none of the programs are installed.
struct NothingInstalled;

impl Commands for NothingInstalled {
    fn run(&self, _program: &str, _args: &[&str]) -> Option<String> {
        None
    }
}

fn root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/fixtures/hardware")
        .join(name)
}

/// Detects a fixture machine, replaying its captured command output.
pub fn detect_fixture(name: &str) -> Hardware {
    let root = root(name);
    detect(&root, &Captured(root.clone()))
}

/// Detects a fixture machine as if `vulkaninfo` and `nvidia-smi` were missing.
#[allow(dead_code)] // each test file compiles this module separately
pub fn detect_fixture_without_programs(name: &str) -> Hardware {
    detect(&root(name), &NothingInstalled)
}
