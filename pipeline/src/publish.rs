//! Packaging games.db for download: `games.db.zst` and `manifest.json`
//! (ROADMAP §7.3). The app checks the manifest, downloads the file, verifies
//! its SHA-256 and swaps it in.

use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use rigcheck_gamedb::{GameDb, Manifest};
use sha2::{Digest, Sha256};

use crate::Error;

/// What this build is, from the workflow.
#[derive(Debug, Clone)]
pub struct Release {
    /// e.g. "2026-11-01.1": date and run number.
    pub db_version: String,
    /// RFC 3339, e.g. "2026-11-01T04:00:00Z".
    pub built_at: String,
    /// Where `games.db.zst` will be downloaded from.
    pub url: String,
    /// The oldest app version that can read this schema.
    pub min_app_version: String,
}

/// Compresses `db` into `out/games.db.zst` and writes `out/manifest.json`.
pub fn package(db: &Path, out: &Path, release: &Release) -> Result<Manifest, Error> {
    let schema_version = GameDb::open(db)?
        .meta("schema_version")?
        .ok_or("games.db has no schema_version")?
        .parse()?;
    fs::create_dir_all(out)?;
    let zst = out.join("games.db.zst");
    // Level 19: a smaller download is worth a slower build.
    zstd::stream::copy_encode(BufReader::new(File::open(db)?), File::create(&zst)?, 19)?;
    let bytes = fs::read(&zst)?;
    let sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let manifest = Manifest {
        schema_version,
        db_version: release.db_version.clone(),
        built_at: release.built_at.clone(),
        url: release.url.clone(),
        sha256,
        size_bytes: bytes.len() as u64,
        min_app_version: release.min_app_version.clone(),
    };
    fs::write(
        out.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    Ok(manifest)
}
