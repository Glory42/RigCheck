//! Tests for the crate's public seam, `installed_games(home)`.
//!
//! Each test builds a fake home directory in a temp dir and fills it with real
//! Steam files from `data/fixtures/steam/`.

use std::fs;
use std::path::{Path, PathBuf};

use rigcheck_steam::{InstalledGame, installed_games};
use tempfile::TempDir;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/fixtures/steam");

/// Creates a Steam install at `home/relative` whose `libraryfolders.vdf`
/// lists the install itself followed by `extra_libraries`.
fn steam_install(home: &Path, relative: &str, extra_libraries: &[&Path]) -> PathBuf {
    let steam = home.join(relative);
    fs::create_dir_all(steam.join("steamapps")).unwrap();

    let mut vdf = String::from("\"libraryfolders\"\n{\n");
    for (i, library) in [steam.as_path()].iter().chain(extra_libraries).enumerate() {
        vdf += &format!(
            "\t\"{i}\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n",
            library.display()
        );
    }
    vdf += "}\n";
    fs::write(steam.join("steamapps/libraryfolders.vdf"), vdf).unwrap();

    steam
}

/// Installs the fixture manifest for `app_id` into `library`.
fn install(library: &Path, app_id: u32) {
    let manifest = format!("appmanifest_{app_id}.acf");
    fs::create_dir_all(library.join("steamapps")).unwrap();
    fs::copy(
        Path::new(FIXTURES).join(&manifest),
        library.join("steamapps").join(&manifest),
    )
    .unwrap();
}

#[test]
fn finds_a_game_in_the_default_steam_install() {
    let home = TempDir::new().unwrap();
    let steam = steam_install(home.path(), ".local/share/Steam", &[]);
    install(&steam, 1150690);

    let games = installed_games(home.path());

    assert_eq!(
        games,
        [InstalledGame {
            app_id: 1150690,
            name: "OMORI".to_owned(),
            size_on_disk: 2_039_368_158,
            install_dir: steam.join("steamapps/common/OMORI"),
        }]
    );
}

#[test]
fn skips_proton_runtimes_and_redistributables() {
    let home = TempDir::new().unwrap();
    let steam = steam_install(home.path(), ".local/share/Steam", &[]);
    for app_id in [1150690, 1493710, 4183110, 228980] {
        install(&steam, app_id);
    }

    let games = installed_games(home.path());

    assert_eq!(app_ids(&games), [1150690]);
}

#[test]
fn finds_games_in_libraries_on_other_drives() {
    let home = TempDir::new().unwrap();
    let drive = TempDir::new().unwrap();
    let steam = steam_install(home.path(), ".local/share/Steam", &[drive.path()]);
    install(&steam, 1150690);
    install(drive.path(), 21000);

    let games = installed_games(home.path());

    assert_eq!(app_ids(&games), [21000, 1150690]);
}

#[test]
fn finds_games_in_a_flatpak_steam_install() {
    let home = TempDir::new().unwrap();
    let steam = steam_install(
        home.path(),
        ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        &[],
    );
    install(&steam, 1150690);

    let games = installed_games(home.path());

    assert_eq!(app_ids(&games), [1150690]);
}

#[test]
fn finds_games_in_a_steam_install_under_dot_steam() {
    let home = TempDir::new().unwrap();
    let steam = steam_install(home.path(), ".steam/steam", &[]);
    install(&steam, 1150690);

    let games = installed_games(home.path());

    assert_eq!(app_ids(&games), [1150690]);
}

#[test]
fn lists_each_game_once_when_dot_steam_links_to_the_install() {
    let home = TempDir::new().unwrap();
    let steam = steam_install(home.path(), ".local/share/Steam", &[]);
    install(&steam, 1150690);
    fs::create_dir(home.path().join(".steam")).unwrap();
    std::os::unix::fs::symlink(&steam, home.path().join(".steam/steam")).unwrap();

    let games = installed_games(home.path());

    assert_eq!(app_ids(&games), [1150690]);
}

/// The app IDs of `games`, sorted so tests don't depend on directory order.
fn app_ids(games: &[InstalledGame]) -> Vec<u32> {
    let mut ids: Vec<u32> = games.iter().map(|game| game.app_id).collect();
    ids.sort_unstable();
    ids
}
