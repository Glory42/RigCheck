//! rigcheck-pipeline: builds games.db from Steam and its data sources
//! (ROADMAP Phase 4). The logic lives in the library; this is the command line.
//!
//!   rigcheck-pipeline applist <apps.json>
//!       Every game on Steam, from GetAppList. Needs $STEAM_API_KEY.
//!   rigcheck-pipeline crawl <apps.json> <games.db> <budget>
//!       Fetches up to <budget> store pages, popular and changed ones first,
//!       into games.db (created if missing). Run again to continue.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rigcheck_core::Catalog;
use rigcheck_gamedb::GameDb;
use rigcheck_net::{Allowlist, Client, RateLimiter, Retry};
use rigcheck_pipeline::Error;
use rigcheck_pipeline::applist::{self, App};
use rigcheck_pipeline::crawl::plan;
use rigcheck_pipeline::ingest::{Ingested, ingest};

const HARDWARE: &str = include_str!("../../data/hardware.toml");

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["applist", out] => fetch_applist(Path::new(out)),
        ["crawl", apps, db, budget] => crawl(Path::new(apps), Path::new(db), budget.parse()?),
        _ => Err(
            "usage: rigcheck-pipeline applist <apps.json> | crawl <apps.json> <games.db> <budget>"
                .into(),
        ),
    }
}

/// Steam documents ~200 store requests per 5 minutes; stay below it.
fn client() -> Result<Client, Error> {
    Client::new(
        Allowlist::pipeline(),
        RateLimiter::new(150, Duration::from_secs(300)),
        Retry::new(4, Duration::from_secs(30)),
    )
}

fn fetch_applist(out: &Path) -> Result<(), Error> {
    let key = std::env::var("STEAM_API_KEY").map_err(|_| "set STEAM_API_KEY")?;
    let mut client = client()?;
    let apps = applist::collect(|last_appid| {
        let url = format!(
            "https://api.steampowered.com/IStoreService/GetAppList/v1/?key={key}\
             &include_games=true&include_dlc=false&include_software=false\
             &include_videos=false&include_hardware=false&max_results=50000&last_appid={last_appid}"
        );
        client.get_text(&url)
    })?;
    std::fs::write(out, serde_json::to_string(&apps)?)?;
    println!("{} games → {}", apps.len(), out.display());
    Ok(())
}

fn crawl(apps: &Path, db_path: &Path, budget: usize) -> Result<(), Error> {
    let apps: Vec<App> = serde_json::from_str(&std::fs::read_to_string(apps)?)?;
    let db = if db_path.exists() {
        GameDb::open(db_path)?
    } else {
        let db = GameDb::create(db_path)?;
        db.store_catalog(&Catalog::from_toml(HARDWARE)?)?;
        db
    };
    let catalog = db.catalog()?;
    // Popularity comes from ProtonDB report counts once enrichment lands.
    let batch = plan(&apps, &db, &HashMap::new(), budget)?;
    let mut client = client()?;
    let (mut games, mut other) = (0, 0);
    for app_id in &batch {
        let url = format!(
            "https://store.steampowered.com/api/appdetails?appids={app_id}&l=english&cc=us"
        );
        let fetched_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        match client
            .get_text(&url)
            .and_then(|json| ingest(*app_id, &json))
        {
            Ok(Ingested::Game(mut record)) => {
                record.updated_at = fetched_at;
                db.store(&record, &catalog)?;
                games += 1;
            }
            Ok(Ingested::NotAGame { .. } | Ingested::Unavailable) => other += 1,
            Err(error) => {
                // Not recorded, so the next run tries again.
                eprintln!("{app_id}: {error}");
                continue;
            }
        }
        db.record_fetch(*app_id, fetched_at)?;
    }
    println!(
        "fetched {} of {} planned: {games} games, {other} other",
        games + other,
        batch.len()
    );
    Ok(())
}
