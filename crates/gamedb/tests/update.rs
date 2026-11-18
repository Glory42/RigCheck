//! Downloading a new games.db: the manifest check and the safe swap
//! (ROADMAP §7.3).

use std::fs::File;
use std::path::{Path, PathBuf};

use rigcheck_gamedb::{GameDb, Limits, Manifest, install, install_within};
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

#[test]
fn an_installed_db_knows_which_build_it_is() {
    let dir = TempDir::new().unwrap();
    let (zst, manifest) = published(dir.path());

    let db = install(&zst, &manifest, &dir.path().join("data/games.db")).unwrap();

    assert_eq!(
        db.meta("db_version").unwrap().as_deref(),
        Some("2026-11-01.1")
    );
}

#[test]
fn only_a_newer_build_is_an_update() {
    let manifest = Manifest::from_json(PUBLISHED).unwrap(); // 2026-11-01.1

    assert!(manifest.is_update_for(None));
    assert!(manifest.is_update_for(Some("2026-10-31.1")));
    assert!(!manifest.is_update_for(Some("2026-11-01.1")));
    // An older build served again (a rollback) isn't taken.
    assert!(!manifest.is_update_for(Some("2026-11-01.2")));
    assert!(!manifest.is_update_for(Some("2026-11-02.1")));
}

#[test]
fn build_numbers_compare_as_numbers() {
    let json = PUBLISHED.replace("2026-11-01.1", "2026-11-01.10");
    let manifest = Manifest::from_json(&json).unwrap();

    assert!(manifest.is_update_for(Some("2026-11-01.9")));
}

#[test]
fn a_build_named_some_other_way_is_an_update() {
    let manifest = Manifest::from_json(PUBLISHED).unwrap();

    assert!(manifest.is_update_for(Some("seed")));
}

/// The app was killed mid-update: a half-downloaded file and a
/// half-unpacked one are left next to games.db.
#[test]
fn an_update_killed_halfway_leaves_the_db_working_and_the_next_one_succeeds() {
    let dir = TempDir::new().unwrap();
    let dest = dir.path().join("data/games.db");
    old_db(&dest);
    std::fs::write(dest.with_extension("db.zst.part"), b"half a downl").unwrap();
    std::fs::write(dest.with_extension("db.new"), b"half an unpa").unwrap();

    assert!(still_old(&dest));
    let (zst, manifest) = published(dir.path());
    let db = install(&zst, &manifest, &dest).unwrap();
    assert!(db.game(1086940).unwrap().is_some());
}

/// Installed databases search a faster table built on install; the results
/// must be the same.
#[test]
fn an_installed_db_finds_games_by_a_one_letter_prefix() {
    let dir = TempDir::new().unwrap();
    let (zst, manifest) = published(dir.path());
    let db = install(&zst, &manifest, &dir.path().join("data/games.db")).unwrap();

    let names: Vec<String> = db
        .games_page("b", &rigcheck_gamedb::Filters::default(), 0, 10)
        .unwrap()
        .into_iter()
        .map(|(_, name)| name)
        .collect();

    // Seed names with a word starting with "b", by name (no popularity).
    assert_eq!(
        names,
        [
            "Balatro",
            "Baldur's Gate 3",
            "LEGO® Batman™: The Videogame",
            "PUBG: BATTLEGROUNDS"
        ]
    );
}

#[test]
fn a_download_of_another_size_than_the_manifest_says_keeps_the_old_db() {
    let dir = TempDir::new().unwrap();
    let (zst, mut manifest) = published(dir.path());
    manifest.size_bytes -= 1; // the SHA-256 still matches
    let dest = dir.path().join("data/games.db");
    old_db(&dest);

    assert!(install(&zst, &manifest, &dest).is_err());

    assert!(still_old(&dest));
}

#[test]
fn a_download_over_the_size_limit_keeps_the_old_db() {
    let dir = TempDir::new().unwrap();
    let (zst, manifest) = published(dir.path());
    let dest = dir.path().join("data/games.db");
    old_db(&dest);
    let limits = Limits {
        download_bytes: manifest.size_bytes - 1,
        ..Limits::default()
    };

    assert!(install_within(&zst, &manifest, &dest, &limits).is_err());

    assert!(still_old(&dest));
}

/// A decompression bomb: a small download that unpacks to far more.
#[test]
fn a_download_that_unpacks_past_the_limit_keeps_the_old_db() {
    let dir = TempDir::new().unwrap();
    let (zst, manifest) = published(dir.path());
    let dest = dir.path().join("data/games.db");
    old_db(&dest);
    let limits = Limits {
        unpacked_bytes: 4096,
        ..Limits::default()
    };

    assert!(install_within(&zst, &manifest, &dest, &limits).is_err());

    assert!(still_old(&dest));
    assert!(!dest.with_extension("db.new").exists());
}

/// A built games.db with `sql` run on it, packaged like a release.
fn tampered(dir: &Path, sql: &str) -> (PathBuf, Manifest) {
    let build = dir.join("built.db");
    drop(GameDb::create_seed(&build).unwrap());
    rusqlite::Connection::open(&build)
        .unwrap()
        .execute_batch(sql)
        .unwrap();
    package(dir, &build)
}

/// SQL in a downloaded file runs when the app writes to it on install, so
/// only the schema the pipeline itself writes is accepted.
#[test]
fn a_download_with_sql_of_its_own_keeps_the_old_db() {
    let attacks = [
        // Runs when the app records the build in `meta`.
        "CREATE TRIGGER planted AFTER INSERT ON meta BEGIN DELETE FROM games; END;",
        "CREATE VIEW planted AS SELECT * FROM games;",
        // The search trigger, rewritten.
        "DROP TRIGGER games_fts_insert;
         CREATE TRIGGER games_fts_insert AFTER INSERT ON games BEGIN DELETE FROM games; END;",
        // A table the app builds itself on install, already there.
        "CREATE VIRTUAL TABLE games_search USING fts5(name);",
    ];
    for sql in attacks {
        let dir = TempDir::new().unwrap();
        let (zst, manifest) = tampered(dir.path(), sql);
        let dest = dir.path().join("data/games.db");
        old_db(&dest);

        assert!(install(&zst, &manifest, &dest).is_err(), "accepted: {sql}");

        assert!(still_old(&dest));
    }
}

/// The checks don't reject what the pipeline legitimately adds over time:
/// tables, columns and indexes.
#[test]
fn a_download_with_another_table_or_index_installs() {
    let dir = TempDir::new().unwrap();
    let (zst, manifest) = tampered(
        dir.path(),
        "CREATE TABLE future (x INTEGER); CREATE INDEX games_by_name ON games (name);",
    );

    assert!(install(&zst, &manifest, &dir.path().join("data/games.db")).is_ok());
}
