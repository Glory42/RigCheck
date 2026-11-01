# ROADMAP

> **Working name:** `rigcheck` (placeholder, rename freely)
>
> A Linux desktop app that tells you whether your PC can run a game before you buy it. It checks your installed Steam library and can search every game on Steam. Beyond raw hardware, it covers the Linux-specific questions: Proton compatibility, anti-cheat, drivers and Vulkan.

---

## 1. Principles

These are the rules every decision below follows. If a feature breaks one of them, the feature changes, not the principle.

1. **Local-first.** All game data, hardware info and results live on the user's machine. The app works offline once the database is downloaded.
2. **Zero user data.** No accounts, no telemetry, no analytics, no crash reporting servers. The app never sends the user's hardware, library or searches anywhere.
3. **No servers.** The project hosts no database and no API. The only thing published is a static database file, on GitHub Releases.
4. **Zero cost.** Everything runs on free tiers: a public GitHub repo, GitHub Actions, GitHub Releases, Flathub and AUR.
5. **Honest results.** If the app can't parse a requirement or identify a piece of hardware, it says **"unknown"** and shows the raw text. It never invents confidence.
6. **Open source (MIT).** The code is MIT-licensed. The repo stays **private during early development** and goes public before the data pipeline starts publishing (see §1.1). Being public gives free GitHub Actions and public release downloads, satisfies the data licenses (see §9) and earns trust from Linux users.

### 1.1 Repository visibility plan

| Stage | Visibility | Why |
|---|---|---|
| Phases 0–3 | **Private** | Early, messy work. CI runs fit in the free private quota (2,000 Actions minutes/month on GitHub Free) |
| Before Phase 4 | **Public** | Required for zero cost: release assets in a private repo need authentication, so the app couldn't download `games.db` or app updates. A daily crawl would also use up the private Actions quota within days |

**While private:**
- Run CI only on pushes to `master` and on pull requests.
- Cache Rust builds (`Swatinem/rust-cache`) to save minutes.
- Develop against the hand-written seed database instead of a published one.

**Before going public:** check that no secrets are in the git history, and make sure the README, LICENSE and credits are in place.

### 1.2 Licenses

| What | License |
|---|---|
| Code (app, crates, pipeline) | **MIT** |
| Published `games.db` | **ODbL**: required because it includes ProtonDB data, which is ODbL (share-alike) |

### Non-goals
- No Windows/macOS support. Linux only.
- No user-submitted reports or community database. ProtonDB already does this, and it would need a server and user data.
- No FPS predictions. Results are "meets minimum / recommended", not "62 fps on High".
- No game stores other than Steam in v1. GOG and Epic come later, if ever (see §11).

---

## 2. What the app does

| Feature | Description |
|---|---|
| **My Library** | Lists every installed Steam game with cover art and a verdict badge (✅ / ⚠️ / ❌ / ❓) |
| **Catalog** | Instant search across every game on Steam, owned or not |
| **Game page** | Per-component verdict (CPU, GPU, RAM, VRAM, storage, graphics API), the bottleneck component, ProtonDB rating, anti-cheat status, Steam Deck status and a native Linux flag |
| **My Hardware** | Shows detected CPU, GPU(s), RAM, VRAM, driver, Vulkan version, distro, kernel and free disk space |
| **Appearance** | Five color themes for the pixel-art look (Paper, Ink, Midnight, Plum, Forest). Paper is the default; the choice is saved in `settings.json` |
| **Language** | English, Turkish and German (Fluent `.ftl` files in `app/i18n/`). Follows the system language unless the user picks one. Game names and Steam's requirement text stay in English |
| **Storage settings** | Database size and update status, a clear button for the cache, an image cache limit |

### Verdict levels
| Badge | Meaning |
|---|---|
| ✅ **Recommended** | Every component meets the recommended specs |
| 🟢 **Minimum** | Every component meets the minimum specs, at least one falls short of recommended |
| ❌ **Below minimum** | At least one component is below minimum. The app names it |
| ⛔ **Blocked** | The game can't run on Linux regardless of hardware, e.g. anti-cheat that denies Linux or a ProtonDB "Borked" rating |
| ❓ **Unknown** | Not enough data. The app shows the raw requirement text |

The overall verdict is the **worst component**. A ⛔ Linux blocker overrides every hardware verdict.

---

## 3. Architecture

```
┌──────────────────── Desktop app (one Rust binary, iced) ───────────────┐
│                                                                        │
│  app/  UI: Library │ Catalog │ Game page │ My Hardware │ Settings      │
│        Elm architecture: state → view → Message → update               │
│        calls the crates below directly (no IPC, no webview)            │
│  ────────────────────────────────────────────────────────────────────  │
│  crates/                                                               │
│  ├── hardware   detect CPU/GPU/RAM/VRAM/Vulkan/driver/disk             │
│  ├── steam      find Steam installs, list installed games              │
│  ├── gamedb     games.db (read-only) + fetched.db (cache), updater     │
│  ├── net        the ONLY crate with an HTTP client: live Steam         │
│  │              lookups, cover art download, games.db download         │
│  └── core       requirement parser + matcher (shared with pipeline)    │
└────────────────────────────────────────────────────────────────────────┘
          │ only 3 network destinations, all read-only, all via net (§6)
          ▼
   GitHub Releases (games.db)   Steam store API (live lookups)   Steam CDN (images)

┌──────────────────── Data pipeline (GitHub Actions, scheduled) ─────────┐
│ 1. Download the previous games.db release                              │
│ 2. Fetch Steam app list → find new/changed games                       │
│ 3. Crawl requirements (rate-limited, resumable)                        │
│ 4. Parse with `core` → structured requirements                         │
│ 5. Join ProtonDB dump, AreWeAntiCheatYet, hardware scores              │
│ 6. Validate → compress → publish games.db + manifest.json              │
└────────────────────────────────────────────────────────────────────────┘
```

**Key idea:** the requirement parser and matcher live in a single Rust crate (`core`). The pipeline uses it to pre-parse popular games, and the app uses the **same code** to parse live-fetched games. One parser, one source of truth.

---

## 4. Tech stack

| Layer | Choice | Why |
|---|---|---|
| Language | **Rust, everywhere** | One language for UI, logic and pipeline. A single native binary, no webview |
| UI | **iced** (0.14) | Native Rust GUI in the Elm style, used by System76's COSMIC desktop. GPU rendering (wgpu) with a software fallback (tiny-skia) |
| Lists | `scrollable` + render only the visible rows | Smooth scrolling through 100k+ rows |
| Async work (UI) | iced `Task` on tokio | Loading states for disk and network work without blocking the UI |
| App directories | `directories` (XDG) | `~/.local/share`, `~/.cache`, `~/.config`; Flatpak redirects these into its sandbox |
| Database | **SQLite** via `rusqlite` (bundled), **FTS5** for search | Local, millisecond search, zero cost |
| Steam files | `steamlocate`, `keyvalues-parser` | Finds Steam (native + Flatpak) and parses VDF/ACF |
| System info | `sysinfo` + reading `/proc`, `/sys`, `vulkaninfo` | CPU, RAM, disks, GPU details |
| HTTP | `reqwest` (rustls), only inside `crates/net` | Live lookups, images, database download |
| Serialization | `serde`, `serde_json` | Everything |
| Compression | `zstd` | Smaller database downloads |
| Tests (Rust) | built-in tests + `insta` snapshots | Parser and matcher correctness |
| Tests (UI) | `iced_test` (headless simulator) + unit tests on `update` | Screens and state changes, no display needed |
| Dependency rules | `cargo-deny` | Enforces that only `crates/net` has an HTTP client (§6) |
| Packaging | `cargo-packager` (deb, AppImage, pacman), `cargo-generate-rpm` (rpm) | Linux packages from the same binary |
| Pipeline | Rust binary in the same workspace | Shares `core` with the app |
| CI/CD | GitHub Actions | Free for public repos |

---

## 5. Repository layout

```
rigcheck/
├── Cargo.toml                 # Rust workspace
├── deny.toml                  # dependency rules (only crates/net may do HTTP)
├── app/                       # the desktop app (iced), binary `rigcheck`
│   └── src/                   # main.rs + one module per screen
├── pipeline/                  # data pipeline binary (crawl, parse, build games.db)
├── crates/                    # libraries shared by app/ and pipeline/
│   ├── core/                  # requirement parser, matcher, shared types (no I/O)
│   ├── hardware/              # Linux hardware detection
│   ├── steam/                 # Steam install + library discovery
│   ├── gamedb/                # games.db schema, queries, migrations
│   └── net/                   # HTTP client with the host allowlist (§6)
├── packaging/                 # .desktop file, AppStream metainfo, Flatpak manifest (Phase 7)
├── website/                   # Astro showcase site (Phase 9, after v1.0)
├── assets/brand/              # pixel-art icon and the scripts that draw it
├── data/                      # hand-maintained, version-controlled inputs
│   ├── hardware.toml          # known CPUs/GPUs: scores, aliases ("GTX 1060" → GTX 1060 6GB), PCI IDs
│   ├── seed/games.toml        # the seed games.db's games and Linux facts (until Phase 4)
│   └── fixtures/              # real requirement strings + fake /proc, /sys trees for tests
├── .github/workflows/
│   ├── ci.yml                 # lint + test on every PR
│   ├── release-app.yml        # build + publish app on tag
│   └── build-db.yml           # scheduled data pipeline
├── ROADMAP.md
└── README.md
```

**Rule of thumb:** things you run live at the top (`app/`, `pipeline/`), and logic lives in `crates/`. `app/` stays thin: screens, state and wiring. Anything worth unit-testing belongs in a crate.

Crate packages are prefixed (`rigcheck-core`, …) because a package called `core` would clash with Rust's built-in `core` library. Directory names stay short.

**Testability rule:** `hardware` and `steam` take a root path (default `/`, or `$HOME` for Steam). Tests point them at fake directory trees in `data/fixtures/`, so detection gets tested without real hardware.

---

## 6. Privacy & network

The app makes **only** these requests. This list goes in the README and on an in-app "Privacy" page.

| Destination | When | What is sent |
|---|---|---|
| `github.com` (Releases) | On startup (configurable), to fetch `manifest.json` and, if newer, `games.db.zst` | A plain file download. No identifiers |
| `store.steampowered.com/api/appdetails` | When the user opens a game missing from `games.db` | The app ID being looked up |
| Steam CDN | When showing cover art | The app ID of the image |

As with any download, GitHub and Valve see the user's IP address. The project itself receives **nothing**.

**Enforced in code, not just promised:**
- Only `crates/net` may depend on an HTTP client. `deny.toml` bans `reqwest` everywhere else (and bans other HTTP clients outright), and `cargo deny check bans` fails CI if that ever changes.
- The client in `crates/net` only accepts the three hosts above. Any other host is a bug.
- A settings toggle lets users disable automatic update checks and update manually.

---

## 7. Data design

### 7.1 Files on the user's machine (XDG)

| File | Path | Clearable? |
|---|---|---|
| `games.db` | `~/.local/share/rigcheck/` | No. The UI offers **Re-download** instead |
| `fetched.db` | `~/.cache/rigcheck/` | Yes. Holds live lookups for games not in `games.db` |
| Cover images | `~/.cache/rigcheck/images/` | Yes. LRU-evicted at a user-set limit (default 500 MB) |
| `settings.json` | `~/.config/rigcheck/` | Reset button |

Use the `directories` crate (`ProjectDirs`), which follows the XDG variables. Flatpak points those variables into the sandbox, so the same code works there.

**Lookup order:** `games.db` → `fetched.db` → live fetch (stored in `fetched.db`). Updating `games.db` never touches `fetched.db`. Entries in `fetched.db` older than 30 days, or that later appear in `games.db`, are ignored and pruned.

### 7.2 `games.db` schema (draft)

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
-- schema_version, built_at, source versions, attribution

CREATE TABLE games (
  app_id              INTEGER PRIMARY KEY,
  name                TEXT NOT NULL,
  release_date        TEXT,
  popularity          INTEGER,          -- used to rank search results
  linux_native        INTEGER NOT NULL DEFAULT 0,
  protondb_tier       TEXT,             -- platinum|gold|silver|bronze|borked|NULL
  protondb_reports    INTEGER,
  deck_status         TEXT,             -- verified|playable|unsupported|unknown
  anticheat_status    TEXT,             -- none|supported|running|planned|broken|denied|NULL
  anticheat_name      TEXT,             -- EAC, BattlEye, ...
  requirements_status TEXT NOT NULL,    -- parsed|partial|missing|not_crawled
  updated_at          INTEGER NOT NULL
);

CREATE VIRTUAL TABLE games_fts USING fts5(
  name, content='games', content_rowid='app_id', tokenize='unicode61 remove_diacritics 2'
);

CREATE TABLE requirements (
  app_id      INTEGER NOT NULL REFERENCES games(app_id),
  tier        TEXT NOT NULL CHECK (tier IN ('minimum', 'recommended')),
  ram_mb      INTEGER,
  vram_mb     INTEGER,
  storage_mb  INTEGER,
  directx     TEXT,                     -- "11", "12"
  vulkan      TEXT,
  opengl      TEXT,
  cpu_raw     TEXT,                     -- short cleaned text, shown when matching fails
  gpu_raw     TEXT,
  PRIMARY KEY (app_id, tier)
);

-- "Intel i5-4460 or AMD FX-6300" → two rows (OR alternatives)
CREATE TABLE requirement_hardware (
  app_id      INTEGER NOT NULL,
  tier        TEXT NOT NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('cpu', 'gpu')),
  position    INTEGER NOT NULL,         -- order on the store page
  hardware_id INTEGER NOT NULL REFERENCES hardware(id)
);

CREATE TABLE hardware (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL,           -- cpu|gpu
  vendor       TEXT NOT NULL,           -- intel|amd|nvidia
  name         TEXT NOT NULL,           -- canonical display name
  score        INTEGER,                 -- normalized performance score
  score_source TEXT,                    -- blender-open-data|manual
  vram_mb      INTEGER,
  vulkan       TEXT,                    -- max supported Vulkan version (GPUs)
  release_year INTEGER
);

CREATE TABLE hardware_alias (alias TEXT PRIMARY KEY, hardware_id INTEGER NOT NULL);
CREATE TABLE gpu_pci_id (
  vendor_id INTEGER, device_id INTEGER, hardware_id INTEGER,
  PRIMARY KEY (vendor_id, device_id)
);
```

`hardware`, `hardware_alias` and `gpu_pci_id` ship inside `games.db`, so the app's live-fetch parser has the same knowledge as the pipeline.

### 7.3 `manifest.json`

```json
{
  "schema_version": 1,
  "db_version": "2026-11-01.1",
  "built_at": "2026-11-01T04:00:00Z",
  "url": "https://github.com/Glory42/rigcheck/releases/download/db-latest/games.db.zst",
  "sha256": "…",
  "size_bytes": 15000000,
  "min_app_version": "0.4.0"
}
```

- If the app's supported `schema_version` is older than the manifest's, the app keeps its current database and asks the user to update the app.
- Update flow: download to a temp file → verify sha256 → decompress → open and run a sanity query → atomically rename over the old file. A failed update never leaves a broken database.

### 7.4 Size budget

| Part | Estimate |
|---|---|
| All games (search fields + FTS index) | ~10–20 MB |
| Parsed requirements | ~20 MB |
| Hardware tables, ProtonDB, anti-cheat | < 2 MB |
| **On disk** | **~30–60 MB** |
| **Download (zstd)** | **~10–20 MB** |

**Rule:** don't ship raw requirement HTML. It would add hundreds of MB. Store only the short cleaned `cpu_raw` / `gpu_raw` text, and fetch full text live if a game page needs it. A CI check fails the build if `games.db.zst` grows past a set budget (start at 50 MB).

---

## 8. Matching logic

### 8.1 Detecting the user's hardware
| Component | Source | Notes |
|---|---|---|
| CPU | `/proc/cpuinfo` model name → `hardware_alias` | Normalize ("Intel(R) Core(TM) i5-8400 CPU @ 2.80GHz" → "i5-8400") |
| GPU | `/sys/class/drm/card*/device/{vendor,device}` → `gpu_pci_id` | PCI IDs are exact, which beats matching name strings |
| VRAM | `mem_info_vram_total` (AMD), `nvidia-smi` (NVIDIA), unknown for Intel iGPUs | iGPUs share system RAM. Treat VRAM as "shared" |
| RAM | `/proc/meminfo` | Compare with 10% tolerance: the kernel and an iGPU keep some RAM, so a 16 GB laptop reports ~15.0 GiB |
| Driver | `/sys/module/nvidia/version`, Mesa version via `vulkaninfo` | Detect `nouveau` and warn about it |
| Vulkan | `vulkaninfo --summary` | If it's missing, show a "Install vulkan-tools" hint |
| Disk | Free space on the Steam library folders | |
| Distro / kernel | `/etc/os-release`, `uname` | Shown for information only |

**Hybrid laptops** (Intel/AMD iGPU + NVIDIA/AMD dGPU): detect all GPUs, use the **strongest** one for verdicts, and mention PRIME offload if relevant.

**Manual override:** users can correct detected hardware or pick "what if I had GPU X?". This also powers the "should I upgrade?" feature (§10 Phase 8).

### 8.2 Parsing requirements
1. Clean the HTML → split into fields (OS, Processor, Memory, Graphics, DirectX, Storage).
2. Split alternatives on `/`, `or`, `,`, `|`.
3. Normalize each token: lowercase, strip marketing words ("NVIDIA GeForce", "or better", "equivalent", "series"), and keep the VRAM hint ("2GB").
4. Look up in `hardware_alias`. Exact match first, then fuzzy matching with a conservative threshold.
5. Anything unmatched → record it as unknown and keep the raw text. In the pipeline, log unmatched tokens to a report so the aliases in `data/hardware.toml` can grow over time.

### 8.3 Comparing
- **CPU/GPU:** the requirement is met if `user.score >= required.score`. When several alternatives are listed ("i5-4460 or FX-6300"), compare against the **same-vendor** alternative if there is one, otherwise the lowest-scored one. *(This is a decision to revisit with real data.)*
- **RAM:** 10% tolerance (see §8.1). **VRAM and storage:** exact. A GPU that shares system RAM skips the VRAM check.
- **Graphics API:** DirectX through Proton needs Vulkan 1.3 (DXVK for DX9–11, VKD3D-Proton for DX12). Native Linux games skip this check. *(Later: driver-specific DX12 features, native OpenGL/Vulkan versions.)*
- **Confidence:** High when CPU, GPU and RAM could all be compared, Partial when some could, Low when none.
- **Proton margin:** optionally require ~10% more score for non-native games. This is a setting, off by default until it's proven useful.
- **Linux layer:** anti-cheat `denied`/`broken` → ⛔. ProtonDB `borked` → ⛔. `nouveau` driver → warning on every 3D game.

### 8.4 Output (shared Rust type, used directly by the UI)
```rust
struct Verdict {
    overall: Level,                 // Recommended | Minimum | Below | Blocked | Unknown
    components: Vec<ComponentResult>, // per CPU/GPU/RAM/VRAM/Storage/API
    bottleneck: Option<Component>,
    linux: LinuxStatus,             // proton tier, anticheat, deck, native, driver warnings
    confidence: Confidence,         // High | Partial | Low, based on how much was parsed
}
```

---

## 9. Data sources & licenses

| Data | Source | Access | License (verify before release) |
|---|---|---|---|
| Steam app list | `IStoreService/GetAppList` | Free Steam Web API key, stored as a GitHub Actions secret | Steam API Terms |
| Requirements | Steam `appdetails` (unofficial) | No key. ~200 requests / 5 min | Steam API Terms |
| Requirements (backup) | PCGamingWiki API | Free | CC BY-NC-SA. Check compatibility |
| Proton compatibility | ProtonDB data dumps (`bdefore/protondb-data`) | GitHub | ODbL (attribution + share-alike) |
| Anti-cheat | AreWeAntiCheatYet (GitHub JSON) | GitHub | MIT ✅ (keep the copyright notice in Credits) |
| Deck status | Steam `appdetails` / Deck compatibility endpoint | Unofficial | Steam API Terms |
| CPU/GPU scores | Blender Open Data | Public dataset | Check the license |
| PCI IDs | `pci.ids` | Public | BSD-3 / GPL-2 |
| Popularity | ProtonDB report counts, Steam's most-played charts | Free | n/a |
| App icon / logo | Original pixel art in `assets/brand/` (drawn by scripts in `assets/brand/src/`) | n/a | MIT, our own work: no attribution needed, and it can be trademarked later |

**Rules:**
- An in-app **Credits** page lists every source with its license.
- `games.db` is published under ODbL (see §1.2), and the code under MIT.
- Be polite: honor rate limits, send a descriptive `User-Agent` with the repo URL, cache everything and never re-crawl unchanged games.

---

## 10. Phases

Each phase ends with a usable, tested result. Check items off as you go.

### Phase 0: Foundation (v0.0)
- [x] Install the toolchain: `rustup` (stable, with clippy and rustfmt) and `cargo-deny`. iced loads Wayland/X11/Vulkan at runtime, so there are no extra system packages to build
- [ ] Learn Rust basics: *The Rust Book*, chapters 1–10. Keep going in parallel; you don't have to finish first
- [x] Create a **private** GitHub repo (`Glory42/rigcheck`) with the MIT license (see §1.1)
- [x] Set up the Cargo workspace and the crate skeletons from §5
- [x] iced app skeleton in `app/` that opens an empty window with the brand icon, plus a headless `iced_test` test
- [x] CI (`ci.yml`): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check bans`. Run on `master` + PRs only, with Rust caching, to stay inside the private-repo minutes
- [x] Network lock: `crates/net` skeleton, and a `deny.toml` that bans HTTP clients from every other crate (§6)

**Done when:** an empty app window opens, and CI passes on a pull request.

### Phase 1: My Library (v0.1)
- [x] `steam` crate: find Steam installs (native `~/.steam`, `~/.local/share/Steam`, Flatpak `~/.var/app/com.valvesoftware.Steam`)
- [x] Parse `libraryfolders.vdf` → all library folders (including other drives)
- [x] Parse `appmanifest_*.acf` → app ID, name, install size, folder
- [x] Filter out non-games (Proton versions, Steam Linux Runtime, Steamworks redistributables)
- [x] Load installed games into the app with an iced `Task`, so the scan never blocks the UI
- [x] Cover art from Steam's own cache on disk (`steam::cover_art`), shown with iced's `image` widget. Nothing is downloaded; CDN downloads for uncached games move to Phase 5
- [x] Library page: grid of cover tiles with name, verdict stamp and search box (a plain grid is fast enough for an installed library; the Catalog's 100k games get virtualization in Phase 5)
- [x] Tests with fixture Steam directories

**Done when:** the app shows your installed Steam games with cover art, including games on secondary drives.

### Phase 2: My Hardware (v0.2)
- [x] `hardware` crate: CPU, RAM, GPU(s) via PCI IDs, VRAM, driver, Vulkan, disk, distro/kernel (§8.1)
- [x] Handle hybrid GPUs, iGPU-only systems, `nouveau`, and a missing `vulkaninfo`
- [x] Fixture tests: fake `/proc` + `/sys` trees for an AMD desktop, an NVIDIA desktop, an Intel laptop, a hybrid laptop and a Steam Deck
- [x] My Hardware page with a "Copy as text" button (useful for asking for help; copying stays local)
- [x] Manual hardware override UI: type to pick a GPU/CPU from the hardware table on My Rig; saved, and verdicts use it

**Done when:** the hardware page is correct on your machine and every fixture test passes.

### Phase 3: Matcher core (v0.3)
Build this test-first. It's pure logic with no I/O, so it's the easiest part to get right with tests.
- [x] `core` types: `Requirement`, `HardwareRef`, `Verdict`, `Level`, `Confidence`
- [x] Requirement parser (§8.2 step 1): fields, RAM/VRAM/storage/DirectX numbers, with a fixture corpus of **100+ real requirement strings** (145 games, 268 tiers, 92.9% fully parsed) and snapshot tests (`insta`)
- [x] Match CPU/GPU text to the hardware table (§8.2 steps 2–5): split alternatives, normalize, alias lookup (fuzzy matching later)
- [x] Matcher (§8.3), with tests covering every verdict level and edge case
- [x] Hand-write a **seed `games.db`** with ~20 games covering the range: native Linux, Proton Platinum, anti-cheat denied, DX12-heavy, ancient, brand new, missing requirements (`data/seed/games.toml`, 22 games)
- [x] Seed hardware table with ~50 common CPUs/GPUs, using manual scores for now (`data/hardware.toml`, ~70 entries)
- [x] Verdict badges on the Library page, and a first version of the game page

**Done when:** your installed games that appear in the seed DB show correct verdicts, and the parser corpus is ≥ 90% parsed.

### Phase 4: Data pipeline v1 (v0.4)
- [x] **Make the repo public** (checklist in §1.1). This is required for free scheduled Actions and public `games.db` downloads
- [ ] Add the `games.db` ODbL license notice and the Credits data (§9)
- [ ] `pipeline` binary with subcommands: `applist`, `crawl`, `enrich`, `build`, `validate`, `report`
- [x] App list: fetch every app → keep games and drop DLC, soundtracks, tools and demos
- [x] Crawler: rate-limited (stay under the limit), retries with backoff, **resumable**. State lives in the previous `games.db` (`crawl_log`: when each app was fetched), so each run continues where the last one stopped, and unchanged store pages aren't fetched again
- [ ] Crawl priority: popular first (by ProtonDB report count / most-played), then new releases, then everything else
- [ ] Enrich: ProtonDB dump, AreWeAntiCheatYet, Deck status, hardware scores (Blender Open Data, with `data/hardware.toml` for parts it lacks), PCI IDs
- [ ] `validate`: schema check, size budget, sanity checks (e.g. top 100 games must have parsed requirements)
- [ ] `report`: unmatched hardware tokens sorted by frequency, used to grow the aliases in `data/hardware.toml`
- [ ] `build-db.yml`: daily cron → download the previous release → run the pipeline (stop well under the 6-hour job limit) → publish `games.db.zst` + `manifest.json` to a rolling `db-latest` release
- [ ] Keep scheduled workflows alive: the run commits an updated `data/db-version.json`, since GitHub disables cron in repos inactive for 60 days
- [ ] Optional: run the first full crawl locally to get past the multi-day backlog faster

**Done when:** a scheduled run publishes a valid `games.db` with the top ~5k games fully parsed and every game searchable by name.

### Phase 5: Catalog & live lookups (v0.5)
- [ ] App downloads `games.db` on first launch, with a progress bar
- [ ] Catalog page: FTS5 search, ranked by popularity, with a virtualized list
- [ ] Filters: "Only games I can run", ProtonDB tier, native Linux, Deck verified, no anti-cheat issues
- [ ] Full game page: per-component table, bottleneck, Linux status, raw requirement text and a link to the Steam store page
- [ ] `fetch` module: live `appdetails` lookup for games missing from `games.db` or with `not_crawled` status → parse with `core` → store in `fetched.db`
- [ ] Client-side rate limiting and a clear "couldn't reach Steam" state
- [ ] Cover art from the Steam CDN (via `crates/net`, cached in `~/.cache/rigcheck/images/`) for games Steam hasn't cached locally

**Done when:** you can search any Steam game, open it and get a verdict, including games the pipeline hasn't crawled yet.

### Phase 6: Updates & storage (v0.6)
- [ ] Updater: manifest check → download → verify → atomic swap (§7.3)
- [ ] Schema version check and a "please update the app" message
- [ ] Settings → Storage: sizes, Check for updates, Re-download database, Clear fetched games, Clear images, image cache limit
- [ ] Settings → Privacy: auto-update toggle and the list of network destinations (§6)
- [ ] LRU image cache eviction

**Done when:** the database updates in the background with no broken states, even if you kill the app mid-download (test that).

### Phase 7: Packaging & release (v0.7)
- [ ] `release-app.yml`: on tag → build AppImage and `.deb` (`cargo-packager`) and `.rpm` (`cargo-generate-rpm`) → attach to a GitHub Release
- [ ] Update notice: compare the running version with the latest GitHub Release and link to it. Flathub and AUR users get updates from their package manager, so the app never replaces its own binary
- [ ] Test on at least Arch, Ubuntu/Debian, Fedora and SteamOS (Deck desktop mode)
- [ ] Test on NVIDIA proprietary drivers and on a machine without working Vulkan. iced should fall back to its software renderer (tiny-skia); check that it does
- [ ] AUR package (`rigcheck-bin`)
- [ ] Flathub submission (manifest, permissions: read-only access to Steam folders and network).
- [ ] Desktop integration in `packaging/`: a `.desktop` file named after the app ID (`dev.gorkemkaryol.rigcheck`), AppStream metainfo, and hicolor icons (`128`, `256`, `512` from `assets/brand/`). On Wayland, the window icon comes from this `.desktop` file
- [ ] About page in the app: version, license, links, data-source credits (§9)
- [ ] README with screenshots, privacy section and credits

**Done when:** a stranger can install the app from Flathub or AUR and it works.

### Phase 8: Polish → v1.0
- [ ] "What if I upgrade?": pick a hypothetical GPU/CPU/RAM and see which games change verdict
- [ ] Library summary: "38 of 42 installed games meet recommended specs"
- [ ] Sort and group the library by verdict
- [ ] Compare two games side by side
- [ ] Keyboard navigation and accessibility pass
- [ ] Light/dark theme following the system
- [ ] Performance pass: cold start under 1 s, search under 50 ms on 100k games
- [ ] Tune the parser against the `report` output until ≥ 95% of the top 5k games parse fully

**Done when:** v1.0 is tagged, and a post on r/linux_gaming doesn't surface show-stopping bugs.

### Phase 9: Showcase website (after v1.0)
A static showcase page. It describes the app and sends people to GitHub, Flathub or AUR. It has no backend, collects no data and runs no analytics.
- [ ] Astro site in `apps/website/`
- [ ] Host on a subdomain of `gorkemkaryol.dev` (e.g. `rigcheck.gorkemkaryol.dev`)
- [ ] Free hosting: GitHub Pages (custom domain via a `CNAME` DNS record) or Cloudflare Pages
- [ ] Content: what it does, screenshots or a short video, privacy promise (§6), install buttons (Flathub, AUR, AppImage/deb/rpm from GitHub Releases), credits
- [ ] Favicon from `assets/brand/` (`icon-16.png`, `icon-32.png`, `icon-64.png`)
- [ ] Optional: a "latest version" badge, read at build time from the GitHub Releases API

**Done when:** the subdomain serves the page over HTTPS and every install link works.

---

## 11. Future ideas (post-1.0, only if they keep the principles)

- **Owned games:** an optional Steam Web API key entered by the user, stored locally and only ever sent to Steam.
- **Non-Steam launchers:** Heroic (GOG/Epic) and Lutris libraries, read from local config files. Requirements would come from PCGamingWiki.
- **Wishlist import:** paste a Steam wishlist URL to see which wishlisted games you can run.
- **Translations** (i18n).
- **Diff-based database updates** if `games.db` ever grows past ~100 MB.
- **CLI companion** (`rigcheck check "elden ring"`), which is easy since all the logic is in Rust crates.

---

## 12. Risks & mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Steam changes or blocks the unofficial `appdetails` endpoint | Pipeline and live lookups break | Cache aggressively, use PCGamingWiki as a backup, and keep the pipeline's sources modular |
| Requirement parsing is inaccurate | Wrong verdicts erode trust | Show ❓ instead of guessing, use a large fixture corpus, track parse-rate metrics in `report`, show raw text on every game page |
| Hardware scores missing for old or obscure parts | Unknown verdicts | Manual scores in `data/hardware.toml`, fall back to a release-year/tier estimate marked "low confidence" |
| iced is pre-1.0 | Breaking changes between releases, weaker screen-reader support than GTK or the web | Pin the version, keep `app/` thin so upgrades touch little code, upgrade on purpose rather than automatically |
| GPU driver problems (no Vulkan, old drivers) | Blank or crashing window for some users | iced falls back to software rendering (tiny-skia). Document `ICED_BACKEND=tiny-skia` and test on NVIDIA every release |
| Scheduled workflow auto-disabled | Database goes stale | The pipeline commits a version file each run, and a "DB older than 7 days" warning appears in the app |
| Steam installed via Flatpak or Snap | Library not found | Check all known paths, plus a manual "add Steam folder" option |
| First crawl takes days | Slow start | Crawl popular games first, make it resumable, and optionally run the first crawl locally |
| Data license conflicts | Legal risk | Verify every license in §9 before v0.4. Keep attribution in the app and the database `meta` table |
| Private-repo Actions minutes run out (2,000/month) | CI blocked during Phases 0–3 | CI only on `master` + PRs with Rust caching, no scheduled jobs while private, go public before Phase 4 |
| Learning Rust while building | Slow progress | Phases 1–3 need only basic Rust. Keep the `core` crate pure and test-driven, and don't optimize early. For the UI, start each screen from the closest iced example |

---

## 13. Zero-cost checklist

| Need | Free solution |
|---|---|
| Code hosting | Public GitHub repo |
| CI/CD and pipeline compute | GitHub Actions (free for public repos) |
| Database hosting | GitHub Releases asset (up to 2 GB per file) |
| App distribution | GitHub Releases, Flathub, AUR |
| App updates | Flathub and AUR, plus an in-app "new version" notice from GitHub Releases |
| Code signing | Not required on Linux. The updater key is self-generated |
| Images | Steam CDN directly |
| Steam Web API key | Free |
| Website (after v1.0) | GitHub Pages or Cloudflare Pages on a subdomain of `gorkemkaryol.dev` (domain already owned) |
| Servers, databases, analytics | **None. That's the point** |

---

## 14. Learning resources

- *The Rust Book*: https://doc.rust-lang.org/book/
- *Rust by Example*: https://doc.rust-lang.org/rust-by-example/
- iced book and API docs: https://book.iced.rs/, https://docs.rs/iced
- iced examples (most features have one): https://github.com/iced-rs/iced/tree/master/examples
- cargo-deny: https://embarkstudios.github.io/cargo-deny/
- `rusqlite` docs + the SQLite FTS5 docs: https://www.sqlite.org/fts5.html
- Steam VDF/ACF format: Valve Developer Community wiki ("KeyValues")
- Linux sysfs for DRM devices: kernel documentation, `/sys/class/drm`
