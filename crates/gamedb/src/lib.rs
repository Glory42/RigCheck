//! `games.db`: games, their requirements and the hardware table (ROADMAP §7.2).
//!
//! Until Phase 4's pipeline publishes the real database, [`GameDb::create_seed`]
//! builds a small one from `data/seed/`, the hardware table and the corpus.

use std::path::Path;

use rigcheck_core::{
    AntiCheat, Catalog, CatalogEntry, GameRequirements, LinuxInfo, ParsedTier, ProtonTier,
    Requirement, Specs, Vendor, Verdict, evaluate, parse,
};
use rigcheck_hardware::{Hardware, Vram};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

const SCHEMA: &str = include_str!("schema.sql");
const SEED_GAMES: &str = include_str!("../../../data/seed/games.toml");
const HARDWARE: &str = include_str!("../../../data/hardware.toml");
const CORPUS: &str = include_str!("../../../data/fixtures/requirements/corpus.json");

pub struct GameDb {
    conn: Connection,
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
        Ok(GameDb {
            conn: Connection::open(path)?,
        })
    }

    /// An empty games.db at `path` with the schema, replacing any file there.
    pub fn create(path: &Path) -> Result<GameDb, Error> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', '1')",
            [],
        )?;
        Ok(GameDb { conn })
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
        insert_hardware(&tx, catalog)?;
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
            let matched = [
                ("cpu", parsed.cpu.as_deref().map(|t| catalog.match_cpus(t))),
                ("gpu", parsed.gpu.as_deref().map(|t| catalog.match_gpus(t))),
            ];
            for (kind, entries) in matched {
                for (position, entry) in entries.unwrap_or_default().into_iter().enumerate() {
                    tx.execute(
                        "INSERT INTO requirement_hardware (app_id, tier, kind, position, hardware_id)
                         SELECT ?1, ?2, ?3, ?4, id FROM hardware WHERE kind = ?3 AND name = ?5",
                        params![record.app_id, tier, kind, position as i64, entry.name],
                    )?;
                }
            }
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

    /// Every requirement tier's CPU and GPU text, by app.
    pub fn requirement_text(&self) -> Result<Vec<(u32, Option<String>, Option<String>)>, Error> {
        let mut statement = self
            .conn
            .prepare("SELECT app_id, cpu_raw, gpu_raw FROM requirements")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Rebuilds the name index after games were written.
    pub fn rebuild_search(&self) -> Result<(), Error> {
        self.conn
            .execute("INSERT INTO games_fts(games_fts) VALUES ('rebuild')", [])?;
        Ok(())
    }

    /// Games whose name has words starting with each word of `query`, most
    /// popular first: "witcher" → The Witcher 3, The Witcher…
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<(u32, String)>, Error> {
        // Each word quoted (so no FTS syntax leaks in) and matched as a prefix.
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|word| format!("\"{}\"*", word.replace('"', "")))
            .collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = self.conn.prepare(
            "SELECT g.app_id, g.name FROM games_fts f JOIN games g ON g.app_id = f.rowid
             WHERE games_fts MATCH ?1 ORDER BY g.popularity DESC, g.name LIMIT ?2",
        )?;
        let rows = statement.query_map(params![terms.join(" "), limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
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
                "SELECT name, linux_native, protondb_tier, deck_status, anticheat_status, anticheat_name
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
                    ))
                },
            )
            .optional()?;
        let Some((name, native, protondb, deck, anticheat, anticheat_name)) = row else {
            return Ok(None);
        };
        Ok(Some(Game {
            app_id,
            name,
            minimum: self.tier(app_id, "minimum")?,
            recommended: self.tier(app_id, "recommended")?,
            linux: LinuxInfo {
                anticheat: anticheat.as_deref().and_then(anticheat_status),
                protondb: protondb.as_deref().and_then(proton_tier),
                native,
            },
            deck,
            anticheat_name,
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

/// Writes the hardware table.
fn insert_hardware(tx: &rusqlite::Transaction, catalog: &Catalog) -> Result<(), Error> {
    let entries = catalog
        .cpus()
        .iter()
        .map(|entry| ("cpu", entry))
        .chain(catalog.gpus().iter().map(|entry| ("gpu", entry)));
    for (kind, entry) in entries {
        tx.execute(
            "INSERT INTO hardware (kind, vendor, name, score, score_source, vram_mb)
             VALUES (?1, ?2, ?3, ?4, 'manual', ?5)",
            params![
                kind,
                vendor_name(entry.vendor),
                entry.name,
                entry.score,
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
