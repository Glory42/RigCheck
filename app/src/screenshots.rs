//! The README's screenshots, rendered offscreen by iced's test renderer:
//! the real app on a real games.db, with the dev machine's hardware.
//!
//! ```sh
//! RIGCHECK_SCREENSHOT_DB=~/.local/share/rigcheck/games.db \
//!     cargo test -p rigcheck screenshots -- --ignored
//! ```
//!
//! PNGs land in target/screenshots/; convert the ones to keep to WebP in
//! assets/screenshots/ (`magick x.png -quality 90 x.webp`).

use std::path::PathBuf;

use crate::{App, Language, Message, Page, Paths, account, catalog, test_support};

const SIZE: (f32, f32) = (1100.0, 720.0);

#[test]
#[ignore = "writes target/screenshots/; needs RIGCHECK_SCREENSHOT_DB"]
fn screenshots() {
    let db = std::env::var("RIGCHECK_SCREENSHOT_DB").expect("RIGCHECK_SCREENSHOT_DB");
    let dir = tempfile::TempDir::new().unwrap();
    let mut paths = Paths::under(dir.path());
    // The real home: installed games and Steam's cover art.
    paths.home = std::env::var("HOME").unwrap().into();
    std::fs::create_dir_all(&paths.data).unwrap();
    std::fs::copy(&db, paths.data.join("games.db")).unwrap();
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/screenshots");
    // The renderer adds its name to each file (`game-wgpu.png`): start clean.
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let (mut app, _) = App::new(paths.clone(), Language::English);
    let _ = app.update(Message::HardwareDetected(test_support::amd_laptop()));
    let installed = rigcheck_steam::installed_games(&paths.home);
    let _ = app.update(Message::GamesFound(installed));

    shoot(&mut app, &out, "library");
    let _ = app.update(Message::Open(Page::Catalog));
    let rows = SIZE.1;
    let _ = app.update(Message::CatalogScrolled {
        offset: 0.0,
        height: rows,
        width: SIZE.0 - 40.0,
    });
    // Asked for, not arrived: the tiles' skeletons pulse.
    shoot(&mut app, &out, "catalog-loading");
    // The tiles in view with their covers, as the app downloads them.
    for app_id in app.catalog_list.visible_ids() {
        app.art.download_now(app_id);
    }
    shoot(&mut app, &out, "catalog");
    let _ = app.update(Message::CatalogFilter(catalog::Chip::CanRun));
    let _ = app.update(Message::CatalogScrolled {
        offset: 0.0,
        height: rows,
        width: SIZE.0 - 40.0,
    });
    shoot(&mut app, &out, "catalog-can-run");
    let _ = app.update(Message::Open(Page::Game(1086940))); // Baldur's Gate 3
    shoot(&mut app, &out, "game");
    let _ = app.update(Message::PickToCompare(1086940));
    let _ = app.update(Message::Open(Page::Compare(1086940, 1091500))); // vs Cyberpunk 2077
    shoot(&mut app, &out, "compare");
    let _ = app.update(Message::Open(Page::Settings));
    shoot(&mut app, &out, "settings");
    // The Library's ALL MY GAMES, from a saved Steam answer to a made-up key
    // (saved in the temp config; nothing is sent).
    let owned = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/fixtures/steam/owned_games.json"),
    )
    .unwrap();
    let key = "0123456789ABCDEF0123456789ABCDEF".to_owned();
    for message in [
        account::Message::Input(key),
        account::Message::Save,
        account::Message::Owned(Ok(owned)),
    ] {
        let _ = app.update(Message::Account(message));
    }
    let _ = app.update(Message::Open(Page::Library));
    let _ = app.update(Message::LibraryScope(crate::library::Scope::Owned));
    shoot(&mut app, &out, "library-owned");
    let _ = app.update(Message::Open(Page::About));
    shoot(&mut app, &out, "about");
}

/// Saves the app as it looks now to `out/<name>.png`, replacing any old one.
fn shoot(app: &mut App, out: &std::path::Path, name: &str) {
    let path = out.join(format!("{name}.png"));
    let mut ui =
        iced_test::Simulator::with_size(iced_test::core::Settings::default(), SIZE, app.view());
    // Images load in the background over the first frames this renderer
    // draws: draw a few, then the one to keep.
    for _ in 0..12 {
        let _ = ui.snapshot(&app.theme());
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let snapshot = ui.snapshot(&app.theme()).unwrap();
    assert!(snapshot.matches_image(&path).unwrap());
    println!("{}", path.display());
}
