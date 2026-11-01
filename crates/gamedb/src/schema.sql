-- games.db, ROADMAP §7.2. Full-text search (games_fts) arrives with the
-- Catalog in Phase 5.
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);

CREATE TABLE games (
  app_id              INTEGER PRIMARY KEY,
  name                TEXT NOT NULL,
  linux_native        INTEGER NOT NULL DEFAULT 0,
  protondb_tier       TEXT,
  deck_status         TEXT,
  anticheat_status    TEXT,
  anticheat_name      TEXT,
  requirements_status TEXT NOT NULL,   -- parsed|partial|missing
  updated_at          INTEGER NOT NULL
);

CREATE TABLE requirements (
  app_id     INTEGER NOT NULL REFERENCES games(app_id),
  tier       TEXT NOT NULL CHECK (tier IN ('minimum', 'recommended')),
  ram_mb     INTEGER,
  vram_mb    INTEGER,
  storage_mb INTEGER,
  directx    INTEGER,
  cpu_raw    TEXT,
  gpu_raw    TEXT,
  PRIMARY KEY (app_id, tier)
);

-- "Intel i5-4460 or AMD FX-6300" → two rows, in the order the page lists them.
CREATE TABLE requirement_hardware (
  app_id      INTEGER NOT NULL,
  tier        TEXT NOT NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('cpu', 'gpu')),
  position    INTEGER NOT NULL,
  hardware_id INTEGER NOT NULL REFERENCES hardware(id)
);

CREATE TABLE hardware (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('cpu', 'gpu')),
  vendor       TEXT NOT NULL,
  name         TEXT NOT NULL,
  score        INTEGER NOT NULL,
  score_source TEXT NOT NULL,
  vram_mb      INTEGER
);

CREATE TABLE hardware_alias (alias TEXT NOT NULL, hardware_id INTEGER NOT NULL);

CREATE TABLE gpu_pci_id (
  vendor_id   INTEGER NOT NULL,
  device_id   INTEGER NOT NULL,
  hardware_id INTEGER NOT NULL,
  PRIMARY KEY (vendor_id, device_id)
);
