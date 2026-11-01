//! Steam install and library discovery.
//!
//! `steamlocate` parses Steam's files; this crate decides where to look and
//! what counts as a game.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use steamlocate::{Library, SteamDir};

/// A game installed in one of the user's Steam libraries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledGame {
    pub app_id: u32,
    pub name: String,
    pub size_on_disk: u64,
    pub install_dir: PathBuf,
}

/// Where Steam installs itself, relative to the home directory.
const INSTALL_DIRS: &[&str] = &[
    ".local/share/Steam",
    ".steam/steam", // older installs and Debian's package
    ".var/app/com.valvesoftware.Steam/.local/share/Steam", // Flatpak
];

/// Lists the games installed by every Steam install under `home`.
pub fn installed_games(home: &Path) -> Vec<InstalledGame> {
    libraries(home).iter().flat_map(games_in).collect()
}

/// Every library of every Steam install under `home`, each listed once.
fn libraries(home: &Path) -> Vec<Library> {
    let mut seen = HashSet::new();
    let mut libraries = Vec::new();
    for dir in INSTALL_DIRS {
        let Ok(steam) = SteamDir::from_dir(&home.join(dir)) else {
            continue;
        };
        let Ok(found) = steam.libraries() else {
            continue;
        };
        for library in found.flatten() {
            // Symlinks and shared drives can make two installs list the same library.
            if let Ok(real_path) = library.path().canonicalize()
                && seen.insert(real_path)
            {
                libraries.push(library);
            }
        }
    }
    libraries
}

fn games_in(library: &Library) -> Vec<InstalledGame> {
    let mut games = Vec::new();
    for app in library.apps().flatten() {
        let name = app.name.clone().unwrap_or_default();
        if is_steam_tool(&name) {
            continue;
        }
        games.push(InstalledGame {
            install_dir: library.resolve_app_dir(&app),
            app_id: app.app_id,
            name,
            size_on_disk: app.size_on_disk.unwrap_or_default(),
        });
    }
    games
}

/// Steam installs these like games, but they're compatibility tools and shared
/// libraries. Matching names catches new Proton versions without an ID list.
fn is_steam_tool(name: &str) -> bool {
    name.starts_with("Proton ")
        || name.starts_with("Steam Linux Runtime")
        || name == "Steamworks Common Redistributables"
}

/// Steam's cached cover art for an app (`appcache/librarycache/<id>/library_600x900.jpg`,
/// sometimes one folder deeper), so covers show without any download.
pub fn cover_art(home: &Path, app_id: u32) -> Option<PathBuf> {
    const FILE: &str = "library_600x900.jpg";
    INSTALL_DIRS.iter().find_map(|install| {
        let dir = home
            .join(install)
            .join("appcache/librarycache")
            .join(app_id.to_string());
        let direct = dir.join(FILE);
        if direct.is_file() {
            return Some(direct);
        }
        std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .map(|entry| entry.path().join(FILE))
            .find(|path| path.is_file())
    })
}
