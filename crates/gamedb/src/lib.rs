//! `games.db`: games, their requirements and the hardware table (ROADMAP §7.2).
//!
//! The app downloads the pipeline's build ([`install`]). [`GameDb::create_seed`]
//! builds a small one from `data/seed/`, the hardware table and the corpus,
//! for tests.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use rigcheck_core::{
    AntiCheat, Catalog, CatalogEntry, GameRequirements, Level, LinuxInfo, ParsedTier, ProtonTier,
    Requirement, Specs, Vendor, Verdict, evaluate, parse,
};
use rigcheck_hardware::{Hardware, Vram};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::Deserialize;

mod update;

pub use update::{Manifest, SCHEMA_VERSION, install};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

const SCHEMA: &str = include_str!("schema.sql");
const SEED_GAMES: &str = include_str!("../../../data/seed/games.toml");
const HARDWARE: &str = include_str!("../../../data/hardware.toml");
const CORPUS: &str = include_str!("../../../data/fixtures/requirements/corpus.json");

pub struct GameDb {
    conn: Connection,
    /// The specs `temp.runnable` was filled for, if it has been (see
    /// [`Filters::runs_on`]).
    runnable_for: RefCell<Option<Specs>>,
}

/// A game and what it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub app_id: u32,
    pub name: String,
    pub minimum: Option<Tier>,
    pub recommended: Option<Tier>,
    pub linux: LinuxInfo,
    /// verified | playable | unsupported | unknown
    pub deck: Option<String>,
    /// e.g. "BattlEye"
    pub anticheat_name: Option<String>,
    /// Whether the pipeline has read the store page yet. Most games in
    /// games.db are only known by name until it does.
    pub crawled: bool,
}

/// One requirement tier: Steam's text plus what was parsed and matched from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tier {
    pub cpu_text: Option<String>,
    pub gpu_text: Option<String>,
    /// Known alternatives, in the order the page lists them.
    pub cpus: Vec<CatalogEntry>,
    pub gpus: Vec<CatalogEntry>,
    pub ram_mb: Option<u32>,
    pub vram_mb: Option<u32>,
    pub storage_mb: Option<u32>,
    pub directx: Option<u8>,
}

/// One requirement tier's CPU and GPU text, as the store page wrote them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementText {
    pub app_id: u32,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
}

/// A game to write: what the pipeline (or the seed) knows about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameRecord {
    pub app_id: u32,
    pub name: String,
    /// As the store page writes it, e.g. "Feb 26, 2016".
    pub release_date: Option<String>,
    /// Has a Linux build.
    pub native: bool,
    /// platinum | gold | silver | bronze | borked
    pub protondb: Option<String>,
    /// verified | playable | unsupported | unknown
    pub deck: Option<String>,
    /// supported | running | planned | broken | denied
    pub anticheat: Option<String>,
    pub anticheat_name: Option<String>,
    pub minimum: Option<ParsedTier>,
    pub recommended: Option<ParsedTier>,
    /// When this was fetched, as a Unix timestamp.
    pub updated_at: i64,
}

impl Game {
    /// How well this game should run on a machine with `specs`.
    pub fn verdict(&self, specs: &Specs) -> Verdict {
        let requirements = GameRequirements {
            minimum: self.minimum.as_ref().map(Tier::requirement),
            recommended: self.recommended.as_ref().map(Tier::requirement),
        };
        evaluate(&requirements, specs, &self.linux)
    }
}

/// What the Catalog lists besides a search. The default lists everything.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filters {
    /// Only games with a Linux build.
    pub native_linux: bool,
    /// Only games Valve verified for the Steam Deck.
    pub deck_verified: bool,
    /// Only games ProtonDB rates this or better: Gold keeps Gold and Platinum.
    pub protondb_at_least: Option<ProtonTier>,
    /// Hides games whose anti-cheat doesn't work on Linux today: denied,
    /// broken, or support only planned.
    pub no_anticheat_issues: bool,
    /// Only games that reach at least their minimum on these specs.
    pub runs_on: Option<Specs>,
}

/// A game's Linux facts from its games.db columns.
fn linux_info(native: bool, protondb: Option<&str>, anticheat: Option<&str>) -> LinuxInfo {
    LinuxInfo {
        anticheat: anticheat.and_then(anticheat_status),
        protondb: protondb.and_then(proton_tier),
        native,
    }
}

/// The SQL conditions for `filters`' plain columns. Their values are ours,
/// not the user's, so they're written inline.
fn filter_conditions(filters: &Filters) -> Vec<String> {
    let mut conditions = Vec::new();
    if filters.native_linux {
        conditions.push("g.linux_native = 1".to_owned());
    }
    if filters.deck_verified {
        conditions.push("g.deck_status = 'verified'".to_owned());
    }
    if let Some(lowest) = filters.protondb_at_least {
        let best_first = [
            ProtonTier::Platinum,
            ProtonTier::Gold,
            ProtonTier::Silver,
            ProtonTier::Bronze,
            ProtonTier::Borked,
        ];
        let end = best_first
            .iter()
            .position(|&tier| tier == lowest)
            .unwrap_or(4);
        let names: Vec<String> = best_first[..=end]
            .iter()
            .map(|&tier| format!("'{}'", proton_tier_name(tier)))
            .collect();
        conditions.push(format!("g.protondb_tier IN ({})", names.join(", ")));
    }
    if filters.no_anticheat_issues {
        conditions.push(
            "(g.anticheat_status IS NULL OR g.anticheat_status NOT IN ('denied', 'broken', 'planned'))"
                .to_owned(),
        );
    }
    conditions
}

/// `query` as an FTS5 match: each word quoted (so no FTS syntax leaks in)
/// and matched as a prefix. `None` for a blank query.
fn fts_query(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|word| format!("\"{}\"*", word.replace('"', "")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// Turns detected hardware into what the matcher compares, using the
/// hardware table for CPU and GPU scores.
pub fn specs(catalog: &Catalog, hardware: &Hardware) -> Specs {
    let gpu = hardware.gaming_gpu();
    let vulkan = gpu.and_then(|gpu| gpu.vulkan.as_ref());
    let gpu_entry = gpu.and_then(|gpu| {
        catalog
            .gpu_by_pci(gpu.vendor_id, gpu.device_id)
            .or_else(|| catalog.match_gpus(&vulkan?.name).into_iter().next())
    });
    Specs {
        ram_mb: hardware.ram_bytes.map(|bytes| (bytes >> 20) as u32),
        vram_mb: gpu.and_then(|gpu| match gpu.vram {
            Vram::Dedicated(bytes) => Some((bytes >> 20) as u32),
            Vram::Shared | Vram::Unknown => None,
        }),
        // Installed games are already on disk.
        free_storage_mb: None,
        vulkan: vulkan.and_then(|vulkan| {
            let mut parts = vulkan.api_version.split('.').map(str::parse::<u32>);
            Some((parts.next()?.ok()?, parts.next()?.ok()?))
        }),
        nouveau: gpu.is_some_and(|gpu| gpu.kernel_driver.as_deref() == Some("nouveau")),
        cpu: hardware
            .cpu
            .as_deref()
            .and_then(|model| catalog.match_cpus(model).into_iter().next())
            .map(CatalogEntry::to_ref),
        gpu: gpu_entry.map(CatalogEntry::to_ref),
    }
}

impl Tier {
    /// What the matcher compares.
    pub fn requirement(&self) -> Requirement {
        Requirement {
            ram_mb: self.ram_mb,
            vram_mb: self.vram_mb,
            storage_mb: self.storage_mb,
            directx: self.directx,
            cpus: self.cpus.iter().map(CatalogEntry::to_ref).collect(),
            gpus: self.gpus.iter().map(CatalogEntry::to_ref).collect(),
        }
    }
}

impl GameDb {
    pub fn open(path: &Path) -> Result<GameDb, Error> {
        Ok(GameDb::with(Connection::open(path)?))
    }

    /// An empty games.db at `path` with the schema, replacing any file there.
    pub fn create(path: &Path) -> Result<GameDb, Error> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)",
            [SCHEMA_VERSION],
        )?;
        Ok(GameDb::with(conn))
    }

    fn with(conn: Connection) -> GameDb {
        GameDb {
            conn,
            runnable_for: RefCell::new(None),
        }
    }

    /// Builds the seed database at `path`, replacing any file there.
    pub fn create_seed(path: &Path) -> Result<GameDb, Error> {
        let db = GameDb::create(path)?;
        let catalog = Catalog::from_toml(HARDWARE)?;
        db.store_catalog(&catalog)?;
        for record in seed_records()? {
            db.store(&record, &catalog)?;
        }
        db.conn.execute(
            "INSERT INTO meta (key, value) VALUES ('source', 'seed')",
            [],
        )?;
        Ok(db)
    }

    /// Writes the hardware table, which ships inside games.db.
    pub fn store_catalog(&self, catalog: &Catalog) -> Result<(), Error> {
        let tx = self.conn.unchecked_transaction()?;
        insert_hardware(&tx, catalog, "manual")?;
        tx.commit()?;
        Ok(())
    }

    /// Adds hardware scored elsewhere (e.g. `source` "blender") to the table.
    pub fn store_extra_hardware(&self, extra: &Catalog, source: &str) -> Result<(), Error> {
        let tx = self.conn.unchecked_transaction()?;
        insert_hardware(&tx, extra, source)?;
        tx.commit()?;
        Ok(())
    }

    /// Matches every stored requirement's CPU and GPU text against the
    /// current hardware table again, so table growth reaches every game.
    pub fn rematch(&self) -> Result<(), Error> {
        let catalog = self.catalog()?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM requirement_hardware", [])?;
        let mut statement =
            tx.prepare("SELECT app_id, tier, cpu_raw, gpu_raw FROM requirements")?;
        let rows: Vec<(u32, String, Option<String>, Option<String>)> = statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<_, _>>()?;
        drop(statement);
        for (app_id, tier, cpu, gpu) in rows {
            insert_matches(&tx, &catalog, app_id, &tier, cpu.as_deref(), gpu.as_deref())?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Writes a game, replacing what was stored for it. CPU and GPU text are
    /// matched against `catalog`, which must already be stored.
    pub fn store(&self, record: &GameRecord, catalog: &Catalog) -> Result<(), Error> {
        let tx = self.conn.unchecked_transaction()?;
        let status = match &record.minimum {
            None => "missing",
            Some(m) if m.cpu.is_some() && m.gpu.is_some() && m.ram_mb.is_some() => "parsed",
            Some(_) => "partial",
        };
        for table in ["requirement_hardware", "requirements"] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE app_id = ?1"),
                [record.app_id],
            )?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO games (app_id, name, release_date, linux_native, protondb_tier,
                 deck_status, anticheat_status, anticheat_name, requirements_status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.app_id,
                record.name,
                record.release_date,
                record.native,
                record.protondb,
                record.deck,
                record.anticheat,
                record.anticheat_name,
                status,
                record.updated_at
            ],
        )?;
        let tiers = [
            ("minimum", &record.minimum),
            ("recommended", &record.recommended),
        ];
        for (tier, parsed) in tiers {
            let Some(parsed) = parsed else { continue };
            tx.execute(
                "INSERT INTO requirements (app_id, tier, ram_mb, vram_mb, storage_mb, directx, cpu_raw, gpu_raw)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    record.app_id,
                    tier,
                    parsed.ram_mb,
                    parsed.vram_mb,
                    parsed.storage_mb,
                    parsed.directx,
                    parsed.cpu,
                    parsed.gpu
                ],
            )?;
            insert_matches(
                &tx,
                catalog,
                record.app_id,
                tier,
                parsed.cpu.as_deref(),
                parsed.gpu.as_deref(),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Remembers that `app_id` was fetched at `fetched_at` (Unix time),
    /// whether or not it turned out to be a game.
    pub fn record_fetch(&self, app_id: u32, fetched_at: i64) -> Result<(), Error> {
        self.conn.execute(
            "INSERT OR REPLACE INTO crawl_log (app_id, fetched_at) VALUES (?1, ?2)",
            params![app_id, fetched_at],
        )?;
        Ok(())
    }

    /// Adds games known only by name (from the app list) as `not_crawled`,
    /// so search finds them. Games already stored are left as they are.
    pub fn store_names(&self, games: &[(u32, &str)]) -> Result<(), Error> {
        let tx = self.conn.unchecked_transaction()?;
        for (app_id, name) in games {
            tx.execute(
                "INSERT OR IGNORE INTO games (app_id, name, requirements_status, updated_at)
                 VALUES (?1, ?2, 'not_crawled', 0)",
                params![app_id, name],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Adds PCI IDs to hardware-table GPUs, by entry name. IDs already
    /// known (e.g. from data/hardware.toml) are kept.
    pub fn add_pci_ids(&self, ids: &[(u16, u16, &str)]) -> Result<(), Error> {
        let tx = self.conn.unchecked_transaction()?;
        for (vendor_id, device_id, name) in ids {
            tx.execute(
                "INSERT OR IGNORE INTO gpu_pci_id (vendor_id, device_id, hardware_id)
                 SELECT ?1, ?2, id FROM hardware WHERE kind = 'gpu' AND name = ?3",
                params![vendor_id, device_id, name],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Writes ProtonDB's tier and report count for games already stored.
    /// The report count is also the game's popularity.
    pub fn set_protondb(&self, facts: &[(u32, Option<ProtonTier>, u32)]) -> Result<(), Error> {
        let tx = self.conn.unchecked_transaction()?;
        for (app_id, tier, reports) in facts {
            tx.execute(
                "UPDATE games SET protondb_tier = ?2, protondb_reports = ?3, popularity = ?3
                 WHERE app_id = ?1",
                params![app_id, tier.map(proton_tier_name), reports],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Writes anti-cheat status and name for games already stored.
    pub fn set_anticheat(&self, facts: &[(u32, AntiCheat, String)]) -> Result<(), Error> {
        let tx = self.conn.unchecked_transaction()?;
        for (app_id, status, name) in facts {
            tx.execute(
                "UPDATE games SET anticheat_status = ?2, anticheat_name = ?3 WHERE app_id = ?1",
                params![app_id, anticheat_status_name(*status), name],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The `n` most popular games and whether their requirements are
    /// "parsed", "partial" or "missing".
    pub fn most_popular(&self, n: usize) -> Result<Vec<(u32, String)>, Error> {
        let mut statement = self.conn.prepare(
            "SELECT app_id, requirements_status FROM games ORDER BY popularity DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([n as i64], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every requirement tier's CPU and GPU text.
    pub fn requirement_text(&self) -> Result<Vec<RequirementText>, Error> {
        let mut statement = self
            .conn
            .prepare("SELECT app_id, cpu_raw, gpu_raw FROM requirements")?;
        let rows = statement.query_map([], |row| {
            Ok(RequirementText {
                app_id: row.get(0)?,
                cpu: row.get(1)?,
                gpu: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A value from the `meta` table (schema version, license, sources…).
    pub fn meta(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), Error> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Rebuilds the name index after games were written.
    pub fn rebuild_search(&self) -> Result<(), Error> {
        self.conn
            .execute("INSERT INTO games_fts(games_fts) VALUES ('rebuild')", [])?;
        Ok(())
    }

    /// Games `offset..offset + limit` of the Catalog, most popular first.
    /// With a `query`, only games whose name has words starting with each of
    /// its words: "witcher" → The Witcher 3, The Witcher…
    pub fn games_page(
        &self,
        query: &str,
        filters: &Filters,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<(u32, String)>, Error> {
        let (from, values) = self.catalog_from(query, filters)?;
        let sql = format!(
            "SELECT g.app_id, g.name {from} ORDER BY g.popularity DESC, g.name LIMIT {limit} OFFSET {offset}"
        );
        let page = self
            .conn
            .prepare(&sql)?
            .query_map(params_from_iter(values), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<Result<_, _>>()?;
        Ok(page)
    }

    /// How many games the Catalog lists for `query` and `filters`.
    pub fn games_count(&self, query: &str, filters: &Filters) -> Result<usize, Error> {
        let (from, values) = self.catalog_from(query, filters)?;
        let count: i64 = self.conn.query_row(
            &format!("SELECT count(*) {from}"),
            params_from_iter(values),
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// The `FROM … WHERE …` of a Catalog query, and the values for its `?`s.
    fn catalog_from(&self, query: &str, filters: &Filters) -> Result<(String, Vec<String>), Error> {
        let mut from = "FROM games g".to_owned();
        if let Some(specs) = &filters.runs_on {
            self.fill_runnable(specs)?;
            from.push_str(" JOIN temp.runnable USING (app_id)");
        }
        let mut conditions = Vec::new();
        let mut values = Vec::new();
        if let Some(terms) = fts_query(query) {
            from.push_str(" JOIN games_fts ON games_fts.rowid = g.app_id");
            conditions.push("games_fts MATCH ?".to_owned());
            values.push(terms);
        }
        conditions.extend(filter_conditions(filters));
        if !conditions.is_empty() {
            from = format!("{from} WHERE {}", conditions.join(" AND "));
        }
        Ok((from, values))
    }

    /// Fills `temp.runnable` (on this connection only) with the games that
    /// reach at least their minimum on `specs`, unless it already holds them.
    fn fill_runnable(&self, specs: &Specs) -> Result<(), Error> {
        if self.runnable_for.borrow().as_ref() == Some(specs) {
            return Ok(());
        }
        let checked = self.checked_games()?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS runnable (app_id INTEGER PRIMARY KEY);
             DELETE FROM temp.runnable;",
        )?;
        {
            let mut insert = tx.prepare("INSERT INTO temp.runnable (app_id) VALUES (?1)")?;
            for game in checked {
                if matches!(
                    game.verdict(specs).overall,
                    Level::Minimum | Level::Recommended
                ) {
                    insert.execute([game.app_id])?;
                }
            }
        }
        tx.commit()?;
        *self.runnable_for.borrow_mut() = Some(specs.clone());
        Ok(())
    }

    /// Every game whose store page has been read, as [`GameDb::game`] loads
    /// it, but in three queries instead of seven per game (~70k games after
    /// a month of crawling).
    fn checked_games(&self) -> Result<Vec<Game>, Error> {
        let mut games: HashMap<u32, Game> = HashMap::new();
        let mut statement = self.conn.prepare(
            "SELECT app_id, name, linux_native, protondb_tier, deck_status, anticheat_status,
                    anticheat_name
             FROM games WHERE requirements_status != 'not_crawled'",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let app_id: u32 = row.get(0)?;
            let protondb: Option<String> = row.get(3)?;
            let anticheat: Option<String> = row.get(5)?;
            let game = Game {
                app_id,
                name: row.get(1)?,
                minimum: None,
                recommended: None,
                linux: linux_info(row.get(2)?, protondb.as_deref(), anticheat.as_deref()),
                deck: row.get(4)?,
                anticheat_name: row.get(6)?,
                crawled: true,
            };
            games.insert(app_id, game);
        }

        let mut statement = self.conn.prepare(
            "SELECT app_id, tier, cpu_raw, gpu_raw, ram_mb, vram_mb, storage_mb, directx
             FROM requirements",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let Some(game) = games.get_mut(&row.get(0)?) else {
                continue;
            };
            let tier = Tier {
                cpu_text: row.get(2)?,
                gpu_text: row.get(3)?,
                ram_mb: row.get(4)?,
                vram_mb: row.get(5)?,
                storage_mb: row.get(6)?,
                directx: row.get(7)?,
                ..Tier::default()
            };
            match row.get_ref(1)?.as_str()? {
                "minimum" => game.minimum = Some(tier),
                _ => game.recommended = Some(tier),
            }
        }

        let mut statement = self.conn.prepare(
            "SELECT r.app_id, r.tier, r.kind, h.name, h.vendor, h.score, h.vram_mb
             FROM requirement_hardware r JOIN hardware h ON h.id = r.hardware_id
             ORDER BY r.app_id, r.tier, r.kind, r.position",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let Some(game) = games.get_mut(&row.get(0)?) else {
                continue;
            };
            let tier = match row.get_ref(1)?.as_str()? {
                "minimum" => game.minimum.as_mut(),
                _ => game.recommended.as_mut(),
            };
            let Some(tier) = tier else {
                continue;
            };
            let entry = CatalogEntry {
                name: row.get(3)?,
                vendor: vendor_from(&row.get::<_, String>(4)?)?,
                score: row.get(5)?,
                vram_mb: row.get(6)?,
                aliases: Vec::new(),
                pci: Vec::new(),
            };
            match row.get_ref(2)?.as_str()? {
                "cpu" => tier.cpus.push(entry),
                _ => tier.gpus.push(entry),
            }
        }
        Ok(games.into_values().collect())
    }

    /// When each app was last fetched.
    pub fn fetched(&self) -> Result<std::collections::HashMap<u32, i64>, Error> {
        let mut statement = self
            .conn
            .prepare("SELECT app_id, fetched_at FROM crawl_log")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The game with `app_id`, or `None` if the database doesn't have it.
    pub fn game(&self, app_id: u32) -> Result<Option<Game>, Error> {
        let row = self
            .conn
            .query_row(
                "SELECT name, linux_native, protondb_tier, deck_status, anticheat_status, anticheat_name,
                        requirements_status != 'not_crawled'
                 FROM games WHERE app_id = ?1",
                [app_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, bool>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, bool>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((name, native, protondb, deck, anticheat, anticheat_name, crawled)) = row else {
            return Ok(None);
        };
        Ok(Some(Game {
            app_id,
            name,
            minimum: self.tier(app_id, "minimum")?,
            recommended: self.tier(app_id, "recommended")?,
            linux: linux_info(native, protondb.as_deref(), anticheat.as_deref()),
            deck,
            anticheat_name,
            crawled,
        }))
    }

    /// How well the game with `app_id` should run, or `None` if it's not here.
    pub fn verdict(&self, app_id: u32, specs: &Specs) -> Result<Option<Verdict>, Error> {
        Ok(self.game(app_id)?.map(|game| game.verdict(specs)))
    }

    /// The hardware table shipped in the database.
    pub fn catalog(&self) -> Result<Catalog, Error> {
        let mut statement = self
            .conn
            .prepare("SELECT id, kind, vendor, name, score, vram_mb FROM hardware ORDER BY id")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, Option<u32>>(5)?,
            ))
        })?;
        let (mut cpus, mut gpus) = (Vec::new(), Vec::new());
        for row in rows {
            let (id, kind, vendor, name, score, vram_mb) = row?;
            let entry = CatalogEntry {
                name,
                vendor: vendor_from(&vendor)?,
                score,
                vram_mb,
                aliases: self.strings("SELECT alias FROM hardware_alias WHERE hardware_id = ?1", id)?,
                pci: self.strings(
                    "SELECT printf('%04x:%04x', vendor_id, device_id) FROM gpu_pci_id WHERE hardware_id = ?1",
                    id,
                )?,
            };
            if kind == "cpu" {
                cpus.push(entry);
            } else {
                gpus.push(entry);
            }
        }
        Ok(Catalog::new(cpus, gpus))
    }

    fn tier(&self, app_id: u32, tier: &str) -> Result<Option<Tier>, Error> {
        let row = self
            .conn
            .query_row(
                "SELECT cpu_raw, gpu_raw, ram_mb, vram_mb, storage_mb, directx
                 FROM requirements WHERE app_id = ?1 AND tier = ?2",
                params![app_id, tier],
                |row| {
                    Ok(Tier {
                        cpu_text: row.get(0)?,
                        gpu_text: row.get(1)?,
                        ram_mb: row.get(2)?,
                        vram_mb: row.get(3)?,
                        storage_mb: row.get(4)?,
                        directx: row.get(5)?,
                        ..Tier::default()
                    })
                },
            )
            .optional()?;
        let Some(mut found) = row else {
            return Ok(None);
        };
        found.cpus = self.tier_hardware(app_id, tier, "cpu")?;
        found.gpus = self.tier_hardware(app_id, tier, "gpu")?;
        Ok(Some(found))
    }

    fn tier_hardware(
        &self,
        app_id: u32,
        tier: &str,
        kind: &str,
    ) -> Result<Vec<CatalogEntry>, Error> {
        let mut statement = self.conn.prepare(
            "SELECT h.name, h.vendor, h.score, h.vram_mb FROM requirement_hardware r
             JOIN hardware h ON h.id = r.hardware_id
             WHERE r.app_id = ?1 AND r.tier = ?2 AND r.kind = ?3 ORDER BY r.position",
        )?;
        let rows = statement.query_map(params![app_id, tier, kind], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, Option<u32>>(3)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (name, vendor, score, vram_mb) = row?;
            entries.push(CatalogEntry {
                name,
                vendor: vendor_from(&vendor)?,
                score,
                vram_mb,
                aliases: Vec::new(),
                pci: Vec::new(),
            });
        }
        Ok(entries)
    }

    fn strings(&self, sql: &str, id: i64) -> Result<Vec<String>, Error> {
        let mut statement = self.conn.prepare(sql)?;
        let rows = statement.query_map([id], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// Links a requirement tier to the table entries its CPU and GPU text name.
fn insert_matches(
    tx: &rusqlite::Transaction,
    catalog: &Catalog,
    app_id: u32,
    tier: &str,
    cpu: Option<&str>,
    gpu: Option<&str>,
) -> Result<(), Error> {
    let matched = [
        ("cpu", cpu.map(|text| catalog.match_cpus(text))),
        ("gpu", gpu.map(|text| catalog.match_gpus(text))),
    ];
    for (kind, entries) in matched {
        for (position, entry) in entries.unwrap_or_default().into_iter().enumerate() {
            tx.execute(
                "INSERT INTO requirement_hardware (app_id, tier, kind, position, hardware_id)
                 SELECT ?1, ?2, ?3, ?4, id FROM hardware WHERE kind = ?3 AND name = ?5",
                params![app_id, tier, kind, position as i64, entry.name],
            )?;
        }
    }
    Ok(())
}

/// Writes the hardware table, with `source` as where the scores came from.
fn insert_hardware(
    tx: &rusqlite::Transaction,
    catalog: &Catalog,
    source: &str,
) -> Result<(), Error> {
    let entries = catalog
        .cpus()
        .iter()
        .map(|entry| ("cpu", entry))
        .chain(catalog.gpus().iter().map(|entry| ("gpu", entry)));
    for (kind, entry) in entries {
        tx.execute(
            "INSERT INTO hardware (kind, vendor, name, score, score_source, vram_mb)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                kind,
                vendor_name(entry.vendor),
                entry.name,
                entry.score,
                source,
                entry.vram_mb
            ],
        )?;
        let id = tx.last_insert_rowid();
        for alias in &entry.aliases {
            tx.execute(
                "INSERT INTO hardware_alias (alias, hardware_id) VALUES (?1, ?2)",
                params![alias, id],
            )?;
        }
        for pci in &entry.pci {
            let (vendor, device) = pci.split_once(':').ok_or("PCI IDs look like 10de:1c03")?;
            tx.execute(
                "INSERT INTO gpu_pci_id (vendor_id, device_id, hardware_id) VALUES (?1, ?2, ?3)",
                params![
                    u16::from_str_radix(vendor, 16)?,
                    u16::from_str_radix(device, 16)?,
                    id
                ],
            )?;
        }
    }
    Ok(())
}

/// The seed games: hand-written Linux facts from data/seed/games.toml, with
/// requirement text from the corpus.
fn seed_records() -> Result<Vec<GameRecord>, Error> {
    #[derive(Deserialize)]
    struct Seed {
        game: Vec<SeedGame>,
    }
    #[derive(Deserialize)]
    struct SeedGame {
        app_id: u32,
        name: String,
        #[serde(default)]
        native: bool,
        protondb: Option<String>,
        deck: Option<String>,
        anticheat: Option<String>,
        anticheat_name: Option<String>,
    }
    #[derive(Deserialize)]
    struct CorpusGame {
        app_id: u32,
        minimum: Option<String>,
        recommended: Option<String>,
    }

    let seed: Seed = toml::from_str(SEED_GAMES)?;
    let corpus: Vec<CorpusGame> = serde_json::from_str(CORPUS)?;
    Ok(seed
        .game
        .into_iter()
        .map(|game| {
            let pages = corpus.iter().find(|c| c.app_id == game.app_id);
            GameRecord {
                app_id: game.app_id,
                name: game.name,
                release_date: None,
                native: game.native,
                protondb: game.protondb,
                deck: game.deck,
                anticheat: game.anticheat,
                anticheat_name: game.anticheat_name,
                minimum: pages.and_then(|p| p.minimum.as_deref()).map(parse),
                recommended: pages.and_then(|p| p.recommended.as_deref()).map(parse),
                updated_at: 0,
            }
        })
        .collect())
}

fn vendor_name(vendor: Vendor) -> &'static str {
    match vendor {
        Vendor::Amd => "amd",
        Vendor::Intel => "intel",
        Vendor::Nvidia => "nvidia",
    }
}

fn vendor_from(name: &str) -> Result<Vendor, Error> {
    Ok(match name {
        "amd" => Vendor::Amd,
        "intel" => Vendor::Intel,
        "nvidia" => Vendor::Nvidia,
        other => return Err(format!("unknown vendor {other:?}").into()),
    })
}

fn proton_tier_name(tier: ProtonTier) -> &'static str {
    match tier {
        ProtonTier::Platinum => "platinum",
        ProtonTier::Gold => "gold",
        ProtonTier::Silver => "silver",
        ProtonTier::Bronze => "bronze",
        ProtonTier::Borked => "borked",
    }
}

fn anticheat_status_name(status: AntiCheat) -> &'static str {
    match status {
        AntiCheat::Supported => "supported",
        AntiCheat::Running => "running",
        AntiCheat::Planned => "planned",
        AntiCheat::Broken => "broken",
        AntiCheat::Denied => "denied",
    }
}

fn proton_tier(name: &str) -> Option<ProtonTier> {
    Some(match name {
        "platinum" => ProtonTier::Platinum,
        "gold" => ProtonTier::Gold,
        "silver" => ProtonTier::Silver,
        "bronze" => ProtonTier::Bronze,
        "borked" => ProtonTier::Borked,
        _ => return None,
    })
}

fn anticheat_status(name: &str) -> Option<AntiCheat> {
    Some(match name {
        "supported" => AntiCheat::Supported,
        "running" => AntiCheat::Running,
        "planned" => AntiCheat::Planned,
        "broken" => AntiCheat::Broken,
        "denied" => AntiCheat::Denied,
        _ => return None,
    })
}
