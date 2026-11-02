//! The Phase 8 performance targets, checked on a real games.db: the app's
//! start-up work under 1 s, and Catalog searches under 50 ms on ~200k games.
//! Timing depends on the machine, so it's run by hand, in release mode:
//!
//! ```sh
//! RIGCHECK_PERF_DB=~/.local/share/rigcheck/games.db \
//!     cargo test --release -p rigcheck perf -- --ignored --nocapture
//! ```
//!
//! A games.db installed by the app has the prefix-indexed name search the
//! search target needs; one straight from the pipeline doesn't.

use std::time::{Duration, Instant};

use crate::{App, Language, Message, Paths};

#[test]
#[ignore = "timing; needs RIGCHECK_PERF_DB and a release build"]
fn perf() {
    let db = std::env::var("RIGCHECK_PERF_DB").expect("RIGCHECK_PERF_DB");
    let dir = tempfile::TempDir::new().unwrap();
    let mut paths = Paths::under(dir.path());
    paths.home = std::env::var("HOME").unwrap().into();
    std::fs::create_dir_all(&paths.data).unwrap();
    std::fs::copy(db, paths.data.join("games.db")).unwrap();

    // What runs before the first frame, plus the two background jobs.
    let start = Instant::now();
    let (mut app, _) = App::new(paths.clone(), Language::English);
    let new = start.elapsed();
    let hardware = rigcheck_hardware::detect(std::path::Path::new("/"), &rigcheck_hardware::System);
    let games = rigcheck_steam::installed_games(&paths.home);
    let _ = app.update(Message::HardwareDetected(hardware));
    let _ = app.update(Message::GamesFound(games));
    let _ = app.view();
    let ready = start.elapsed();
    println!("App::new {new:?}; with hardware, Steam scan and first view {ready:?}");
    assert!(ready < Duration::from_secs(1), "cold start {ready:?}");

    // Broad queries are the slow ones: "s" matches ~50k names.
    for query in ["s", "a", "the", "co", "portal", "elden ri"] {
        let start = Instant::now();
        let _ = app.update(Message::CatalogSearch(query.to_owned()));
        let took = start.elapsed();
        println!("search {query:?}: {took:?}");
        assert!(
            took < Duration::from_millis(50),
            "search {query:?} took {took:?}"
        );
    }
}
