//! fetched.db: live lookups of games games.db hasn't checked, and
//! [`Records`], which reads the two in ROADMAP §7.1's order: games.db, then
//! a recent lookup, then a live fetch (made by the app, kept here).

use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};

use crate::{Error, Game, GameDb, GameRecord, vendor_name};

/// A lookup older than this is worth asking Steam again (ROADMAP §7.1).
const STALE_AFTER: i64 = 30 * 24 * 60 * 60;

/// fetched.db. Same schema as games.db, holding only looked-up games and
/// the hardware rows their requirements point at.
pub struct Fetched {
    db: GameDb,
    /// `None` in memory.
    path: Option<PathBuf>,
}

impl Fetched {
    /// fetched.db at `path`, created (with its directory) on first use.
    pub fn open(path: &Path) -> Result<Fetched, Error> {
        let db = if path.exists() {
            GameDb::open(path)?
        } else {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            GameDb::create(path)?
        };
        Ok(Fetched {
            db,
            path: Some(path.to_owned()),
        })
    }

    /// A fetched.db in memory, for when the cache can't be written: lookups
    /// then last until the app closes.
    pub fn in_memory() -> Fetched {
        Fetched {
            db: GameDb::create_in_memory().expect("an empty in-memory database"),
            path: None,
        }
    }

    /// Forgets every lookup. The file is replaced by an empty one.
    pub fn clear(&mut self) -> Result<(), Error> {
        let Some(path) = self.path.clone() else {
            *self = Fetched::in_memory();
            return Ok(());
        };
        // Closed first, so the file isn't in use when it goes.
        self.db = GameDb::create_in_memory()?;
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
        *self = Fetched::open(&path)?;
        Ok(())
    }

    /// Deletes lookups that are stale at `now` or that `games` has since
    /// checked itself (ROADMAP §7.1). Returns how many went.
    pub fn prune(&self, games: &GameDb, now: i64) -> Result<usize, Error> {
        let mut statement = self
            .db
            .conn
            .prepare("SELECT app_id, updated_at FROM games")?;
        let stored: Vec<(u32, i64)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        drop(statement);
        let gone: Vec<u32> = stored
            .into_iter()
            .filter(|&(app_id, at)| now - at > STALE_AFTER || checked(games, app_id))
            .map(|(app_id, _)| app_id)
            .collect();
        let tx = self.db.conn.unchecked_transaction()?;
        for app_id in &gone {
            for table in ["requirement_hardware", "requirements", "games"] {
                tx.execute(&format!("DELETE FROM {table} WHERE app_id = ?1"), [app_id])?;
            }
        }
        tx.commit()?;
        Ok(gone.len())
    }

    /// Saves a live lookup of a store page, made at `now` (Unix seconds). A
    /// store page doesn't say what ProtonDB, the Deck or AreWeAntiCheatYet
    /// think, so whatever `games` knows about that is kept, and its hardware
    /// table matches the CPU and GPU text.
    fn store(&self, mut record: GameRecord, games: &GameDb, now: i64) -> Result<(), Error> {
        type Known = (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let known: Option<Known> = games
            .conn
            .query_row(
                "SELECT protondb_tier, deck_status, anticheat_status, anticheat_name
                 FROM games WHERE app_id = ?1",
                [record.app_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        if let Some((protondb, deck, anticheat, anticheat_name)) = known {
            record.protondb = record.protondb.or(protondb);
            record.deck = record.deck.or(deck);
            record.anticheat = record.anticheat.or(anticheat);
            record.anticheat_name = record.anticheat_name.or(anticheat_name);
        }
        record.updated_at = now;
        let catalog = games.catalog()?;
        // Requirement rows point at this file's own hardware rows: add the
        // ones it lacks, never renumbering those earlier lookups point at.
        let tx = self.db.conn.unchecked_transaction()?;
        let entries = catalog
            .cpus()
            .iter()
            .map(|entry| ("cpu", entry))
            .chain(catalog.gpus().iter().map(|entry| ("gpu", entry)));
        for (kind, entry) in entries {
            tx.execute(
                "INSERT INTO hardware (kind, vendor, name, score, score_source, vram_mb)
                 SELECT ?1, ?2, ?3, ?4, 'games.db', ?5
                 WHERE NOT EXISTS (SELECT 1 FROM hardware WHERE kind = ?1 AND name = ?3)",
                params![
                    kind,
                    vendor_name(entry.vendor),
                    entry.name,
                    entry.score,
                    entry.vram_mb
                ],
            )?;
        }
        tx.commit()?;
        self.db.store(&record, &catalog)
    }

    /// The game with `app_id`, unless it was looked up more than 30 days
    /// before `now`.
    fn fresh(&self, app_id: u32, now: i64) -> Result<Option<Game>, Error> {
        let looked_up_at: Option<i64> = self
            .db
            .conn
            .query_row(
                "SELECT updated_at FROM games WHERE app_id = ?1",
                [app_id],
                |row| row.get(0),
            )
            .optional()?;
        match looked_up_at {
            Some(at) if now - at <= STALE_AFTER => self.db.game(app_id),
            _ => Ok(None),
        }
    }
}

/// Whether games.db has read `app_id`'s store page.
fn checked(games: &GameDb, app_id: u32) -> bool {
    games
        .conn
        .query_row(
            "SELECT 1 FROM games WHERE app_id = ?1 AND requirements_status != 'not_crawled'",
            [app_id],
            |_| Ok(()),
        )
        .is_ok()
}

/// Where every page reads a game from: games.db, with fetched.db's
/// lookups over the games games.db hasn't checked (ROADMAP §7.1).
pub struct Records<'a> {
    games: &'a GameDb,
    fetched: &'a Fetched,
    /// Unix seconds, for what counts as a recent lookup.
    now: i64,
}

impl<'a> Records<'a> {
    pub fn new(games: &'a GameDb, fetched: &'a Fetched, now: i64) -> Records<'a> {
        Records {
            games,
            fetched,
            now,
        }
    }

    /// games.db, for what only it has: the Catalog's pages and counts, the
    /// hardware table.
    pub fn games(&self) -> &'a GameDb {
        self.games
    }

    /// The record for `app_id`: games.db's if it has checked the game, else
    /// a recent lookup's, else games.db's name-only one. `None` if neither
    /// file has it.
    pub fn get(&self, app_id: u32) -> Result<Option<Game>, Error> {
        let from_games = self.games.game(app_id)?;
        if from_games.as_ref().is_some_and(|game| game.crawled) {
            return Ok(from_games);
        }
        Ok(self.fetched.fresh(app_id, self.now)?.or(from_games))
    }

    /// Whether `app_id` is worth asking Steam about: neither games.db nor a
    /// recent lookup has its requirements.
    pub fn needs_lookup(&self, app_id: u32) -> bool {
        !checked(self.games, app_id) && !matches!(self.fetched.fresh(app_id, self.now), Ok(Some(_)))
    }

    /// Keeps a live lookup of `app_id`'s store page, so [`Records::get`]
    /// answers with it from now on.
    pub fn store_lookup(&self, record: GameRecord) -> Result<(), Error> {
        self.fetched.store(record, self.games, self.now)
    }
}
