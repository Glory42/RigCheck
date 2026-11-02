//! Downloading a new games.db: the manifest check and the safe swap
//! (ROADMAP §7.3).

use std::fs::File;
use std::path::{Path, PathBuf};

use rigcheck_gamedb::{GameDb, Manifest, install};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// The manifest the first scheduled build published to `db-latest`.
const PUBLISHED: &str = r#"{
  "schema_version": 1,
  "db_version": "2026-11-01.1",
  "built_at": "2026-11-01T18:26:21Z",
  "url": "https://github.com/Glory42/RigCheck/releases/download/db-latest/games.db.zst",
  "sha256": "4aae6cfa92b25ade6478620c60823abd1309051d53905b3fadd426f50f02146b",
  "size_bytes": 6357008,
  "min_app_version": "0.4.0"
}
"#;

#[test]
fn reads_the_published_manifest() {
    let manifest = Manifest::from_json(PUBLISHED).unwrap();

    assert_eq!(manifest.db_version, "2026-11-01.1");
    assert_eq!(manifest.size_bytes, 6_357_008);
    assert!(!manifest.needs_newer_app());
}

#[test]
fn a_newer_schema_needs_a_newer_app() {
    let json = PUBLISHED.replace(r#""schema_version": 1"#, r#""schema_version": 2"#);

    let manifest = Manifest::from_json(&json).unwrap();

    assert!(manifest.needs_newer_app());
}

/// The seed packaged the way `pipeline publish` packages a build: zstd
/// file in `dir`, plus a manifest naming its real SHA-256.
fn published(dir: &Path) -> (PathBuf, Manifest) {
    let build = dir.join("built.db");
    drop(GameDb::create_seed(&build).unwrap());
    package(dir, &build)
}

fn package(dir: &Path, build: &Path) -> (PathBuf, Manifest) {
    let zst = dir.join("games.db.zst");
    zstd::stream::copy_encode(File::open(build).unwrap(), File::create(&zst).unwrap(), 3).unwrap();
    let bytes = std::fs::read(&zst).unwrap();
    let mut manifest = Manifest::from_json(PUBLISHED).unwrap();
    manifest.sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    manifest.size_bytes = bytes.len() as u64;
    (zst, manifest)
}

#[test]
fn installs_a_verified_download() {
    let dir = TempDir::new().unwrap();
    let (zst, manifest) = published(dir.path());
    let dest = dir.path().join("data/games.db");

    let db = install(&zst, &manifest, &dest).unwrap();

    let game = db.game(1086940).unwrap().expect("Baldur's Gate 3");
    assert_eq!(game.name, "Baldur's Gate 3");
    assert!(
        GameDb::open(&dest)
            .unwrap()
            .game(1086940)
            .unwrap()
            .is_some()
    );
}

/// A games.db already at `dest`, told apart by a `marker` meta row.
fn old_db(dest: &Path) {
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    GameDb::create(dest)
        .unwrap()
        .set_meta("marker", "old")
        .unwrap();
}

fn still_old(dest: &Path) -> bool {
    GameDb::open(dest)
        .unwrap()
        .meta("marker")
        .unwrap()
        .as_deref()
        == Some("old")
}

#[test]
fn a_download_with_the_wrong_checksum_keeps_the_old_db() {
    let dir = TempDir::new().unwrap();
    let (zst, mut manifest) = published(dir.path());
    manifest.sha256 = "0".repeat(64);
    let dest = dir.path().join("data/games.db");
    old_db(&dest);

    assert!(install(&zst, &manifest, &dest).is_err());

    assert!(still_old(&dest));
}

#[test]
fn a_download_that_isnt_a_games_db_keeps_the_old_db() {
    let dir = TempDir::new().unwrap();
    let build = dir.path().join("built.db");
    std::fs::write(&build, "not a database").unwrap();
    let (zst, manifest) = package(dir.path(), &build);
    let dest = dir.path().join("data/games.db");
    old_db(&dest);

    assert!(install(&zst, &manifest, &dest).is_err());

    assert!(still_old(&dest));
}
