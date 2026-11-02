//! rigcheck-pipeline: builds games.db from Steam and its data sources
//! (ROADMAP Phase 4). The logic lives in the library; this is the command line.
//!
//!   rigcheck-pipeline applist <apps.json>
//!       Every game on Steam, from GetAppList. Needs $STEAM_API_KEY.
//!   rigcheck-pipeline crawl <apps.json> <games.db> <budget> [reports.json]
//!       Fetches up to <budget> store pages into games.db (created if
//!       missing): changed or never-fetched ones, most ProtonDB reports first
//!       when ProtonDB's reports_piiremoved.json is given. Run again to continue.
//!   rigcheck-pipeline build <games.db> <apps.json> <reports.json> <awacy-games.json> <pci.ids> <blender.jsonl>
//!       Adds parts missing from the hardware table (Blender Open Data), GPU
//!       PCI IDs, uncrawled games by name, ProtonDB tiers and anti-cheat
//!       status; re-matches hardware; rebuilds name search.
//!   rigcheck-pipeline publish <games.db> <out-dir> <db_version> <built_at> <url> <min_app_version>
//!       Writes <out-dir>/games.db.zst and manifest.json (ROADMAP §7.3).
//!   rigcheck-pipeline validate <games.db>
//!       Fails (exit 1) if games.db shouldn't be published.
//!   rigcheck-pipeline report <games.db>
//!       Hardware names data/hardware.toml doesn't know, most used first.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rigcheck_core::Catalog;
use rigcheck_gamedb::GameDb;
use rigcheck_gamedb::ingest::{Ingested, ingest};
use rigcheck_net::{Allowlist, Client, RateLimiter, Retry};
use rigcheck_pipeline::Error;
use rigcheck_pipeline::applist::{self, App};
use rigcheck_pipeline::crawl::plan;
use rigcheck_pipeline::validate::{Rules, validate};
use rigcheck_pipeline::{awacy, blender, build, deck, pci, protondb, publish, report};

const HARDWARE: &str = include_str!("../../data/hardware.toml");

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["applist", out] => fetch_applist(Path::new(out)),
        ["crawl", apps, db, budget] => crawl(Path::new(apps), Path::new(db), budget.parse()?, None),
        ["crawl", apps, db, budget, reports] => crawl(
            Path::new(apps),
            Path::new(db),
            budget.parse()?,
            Some(Path::new(reports)),
        ),
        ["build", db, apps, reports, awacy, pci_ids, blender] => build_db(
            Path::new(db),
            Inputs {
                apps: Path::new(apps),
                reports: Path::new(reports),
                awacy: Path::new(awacy),
                pci_ids: Path::new(pci_ids),
                blender: Path::new(blender),
            },
        ),
        [
            "publish",
            db,
            out,
            db_version,
            built_at,
            url,
            min_app_version,
        ] => {
            let release = publish::Release {
                db_version: db_version.to_string(),
                built_at: built_at.to_string(),
                url: url.to_string(),
                min_app_version: min_app_version.to_string(),
            };
            let manifest = publish::package(Path::new(db), Path::new(out), &release)?;
            println!("{} bytes, sha256 {}", manifest.size_bytes, manifest.sha256);
            Ok(())
        }
        ["validate", db] => validate_db(Path::new(db)),
        ["report", db] => report_db(Path::new(db)),
        _ => Err("usage: see the comment at the top of pipeline/src/main.rs".into()),
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

fn now() -> Result<i64, Error> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}

/// ProtonDB tiers and report counts for every game it has reports for.
fn protondb_summaries(reports: &Path) -> Result<HashMap<u32, protondb::Summary>, Error> {
    let reports = protondb::read_reports(BufReader::new(File::open(reports)?))?;
    Ok(protondb::summarize(&reports, now()?))
}

/// The data files `build` reads.
struct Inputs<'a> {
    apps: &'a Path,
    reports: &'a Path,
    awacy: &'a Path,
    pci_ids: &'a Path,
    blender: &'a Path,
}

fn build_db(db: &Path, inputs: Inputs) -> Result<(), Error> {
    let db = GameDb::open(db)?;
    let table = db.catalog()?;
    let gaps = blender::gap_scores(BufReader::new(File::open(inputs.blender)?), &table)?;
    // PCI IDs are looked up in the table as it will be, Blender parts included.
    let grown = Catalog::new(
        table.cpus().iter().chain(&gaps.cpus).cloned().collect(),
        table.gpus().iter().chain(&gaps.gpus).cloned().collect(),
    );
    let sources = build::Sources {
        apps: serde_json::from_str(&std::fs::read_to_string(inputs.apps)?)?,
        protondb: protondb_summaries(inputs.reports)?,
        anticheat: awacy::statuses(&std::fs::read_to_string(inputs.awacy)?)?,
        pci_ids: pci::ids(&grown, &std::fs::read_to_string(inputs.pci_ids)?),
        gaps,
    };
    build::build(&db, &sources)?;
    println!(
        "built: {} apps, ProtonDB facts for {}, anti-cheat for {}, {} PCI IDs, \
         Blender scores for {} CPUs and {} GPUs",
        sources.apps.len(),
        sources.protondb.len(),
        sources.anticheat.len(),
        sources.pci_ids.len(),
        sources.gaps.cpus.len(),
        sources.gaps.gpus.len()
    );
    Ok(())
}

fn validate_db(path: &Path) -> Result<(), Error> {
    let rules = Rules {
        top: 100,
        min_parsed_share: 0.9,
        max_bytes: 200 * 1024 * 1024, // uncompressed; ROADMAP §7.4 caps the .zst at 50 MB
    };
    let problems = validate(&GameDb::open(path)?, path, &rules)?;
    for problem in &problems {
        eprintln!("problem: {problem:?}");
    }
    if problems.is_empty() {
        println!("valid");
        Ok(())
    } else {
        Err(format!("{} problem(s); not publishable", problems.len()).into())
    }
}

fn report_db(path: &Path) -> Result<(), Error> {
    use std::io::Write;
    let db = GameDb::open(path)?;
    let mut out = std::io::stdout().lock();
    for line in report::unmatched(&db, &db.catalog()?)?.iter().take(100) {
        match writeln!(out, "{}\t{}\t{}", line.games, line.kind, line.text) {
            // `report | head` closes the pipe early; that's not an error.
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => return Ok(()),
            result => result?,
        }
    }
    Ok(())
}

fn crawl(apps: &Path, db_path: &Path, budget: usize, reports: Option<&Path>) -> Result<(), Error> {
    let apps: Vec<App> = serde_json::from_str(&std::fs::read_to_string(apps)?)?;
    let db = if db_path.exists() {
        GameDb::open(db_path)?
    } else {
        let db = GameDb::create(db_path)?;
        db.store_catalog(&Catalog::from_toml(HARDWARE)?)?;
        db
    };
    let catalog = db.catalog()?;
    let popularity: HashMap<u32, u32> = match reports {
        Some(reports) => protondb_summaries(reports)?
            .into_iter()
            .map(|(app_id, summary)| (app_id, summary.reports))
            .collect(),
        None => HashMap::new(),
    };
    let batch = plan(&apps, &db, &popularity, budget)?;
    let mut client = client()?;
    let (mut games, mut other) = (0, 0);
    for app_id in &batch {
        let url = format!(
            "https://store.steampowered.com/api/appdetails?appids={app_id}&l=english&cc=us"
        );
        let fetched_at = now()?;
        match client
            .get_text(&url)
            .and_then(|json| ingest(*app_id, &json))
        {
            Ok(Ingested::Game(mut record)) => {
                record.updated_at = fetched_at;
                // A failed Deck lookup only leaves the status empty.
                let deck_url = format!(
                    "https://store.steampowered.com/saleaction/ajaxgetdeckappcompatibilityreport?nAppID={app_id}&l=english"
                );
                record.deck = client
                    .get_text(&deck_url)
                    .and_then(|json| deck::status(&json))
                    .ok()
                    .map(str::to_owned);
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
