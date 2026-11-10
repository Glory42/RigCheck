//! The rigcheck desktop app: an iced UI on top of the workspace crates.

mod about;
mod account;
mod catalog;
mod compare;
mod covers;
mod download;
mod game_page;
mod i18n;
mod keys;
mod library;
mod lookup;
mod motion;
mod my_hardware;
mod overrides;
#[cfg(test)]
mod perf;
mod release;
#[cfg(test)]
mod screenshots;
mod settings;
mod settings_page;
mod style;
#[cfg(test)]
mod test_support;
mod web;
mod whatif;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use i18n::{I18n, Language};
use iced::widget::image::{FilterMethod, Handle};
use iced::widget::{button, center, column, container, image, row, scrollable, space, text};
use iced::{Element, Fill, Size, Task, Theme, clipboard, window};
use rigcheck_core::{Catalog, Specs};
use rigcheck_gamedb::ingest::{Ingested, ingest};
use rigcheck_gamedb::{Fetched, Game, GameDb, Records};
use rigcheck_hardware::{Hardware, Rig, Swap};
use rigcheck_steam::InstalledGame;
use settings::{Appearance, Settings};
use style::{BOLD, Colors};
use web::Web;

/// Reverse-DNS app ID. Wayland uses it to match the window with its .desktop file
/// (and so its icon); Flatpak uses it as the app ID.
const APP_ID: &str = "dev.gorkemkaryol.rigcheck";
const ICON_PNG: &[u8] = include_bytes!("../../assets/brand/icon-256.png");

fn main() -> iced::Result {
    let paths = Paths::from_xdg();
    let system_language = Language::from_locale(system_locale().as_deref());
    iced::application(
        move || App::new(paths.clone(), system_language),
        App::update,
        App::view,
    )
    .title("rigcheck")
    .theme(App::theme)
    .subscription(App::subscription)
    .window(window_settings())
    .run()
}

/// Where the app reads and writes. Tests point these at temp dirs.
#[derive(Debug, Clone)]
struct Paths {
    /// `settings.json`: ~/.config/rigcheck
    config: PathBuf,
    /// `games.db`: ~/.local/share/rigcheck
    data: PathBuf,
    /// `fetched.db`, live lookups: ~/.cache/rigcheck
    cache: PathBuf,
    /// Where Steam installs are looked for.
    home: PathBuf,
}

impl Paths {
    /// The XDG directories (ROADMAP §7.1), honoring `$XDG_CONFIG_HOME` etc.
    fn from_xdg() -> Paths {
        let project = directories::ProjectDirs::from("dev", "gorkemkaryol", "rigcheck")
            .expect("a home directory");
        let base = directories::BaseDirs::new().expect("a home directory");
        Paths {
            config: project.config_dir().to_owned(),
            data: project.data_dir().to_owned(),
            cache: project.cache_dir().to_owned(),
            home: base.home_dir().to_owned(),
        }
    }

    #[cfg(test)]
    fn under(dir: &Path) -> Paths {
        Paths {
            config: dir.join("config"),
            data: dir.join("data"),
            cache: dir.join("cache"),
            home: dir.join("home"),
        }
    }
}

/// The locale for messages, the way POSIX picks it: `$LC_ALL`, then
/// `$LC_MESSAGES`, then `$LANG`.
fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
}

fn window_settings() -> window::Settings {
    window::Settings {
        size: Size::new(1100.0, 720.0),
        min_size: Some(Size::new(640.0, 480.0)),
        icon: Some(
            window::icon::from_file_data(ICON_PNG, None).expect("bundled icon is a valid PNG"),
        ),
        platform_specific: window::settings::PlatformSpecific {
            application_id: APP_ID.to_owned(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// All UI state. Iced follows the Elm architecture: `view` draws the state,
/// user input becomes a `Message`, and `update` is the only place state changes.
struct App {
    paths: Paths,
    settings: Settings,
    /// Used when the user leaves Language on "System default".
    system_language: Language,
    /// Light or dark, for the System appearance.
    system_mode: iced::theme::Mode,
    i18n: I18n,
    page: Page,
    /// `None` until detection finishes.
    hardware: Option<Hardware>,
    /// The detected hardware read against the hardware table, with any
    /// override. `None` until detection finishes.
    rig: Option<Rig>,
    /// `None` until games.db is downloaded; the Library then shows NO DATA.
    db: Option<GameDb>,
    download: download::Downloader,
    catalog: Catalog,
    /// `None` until Steam has been scanned.
    games: Option<Vec<InstalledGame>>,
    /// The records of the Library's games (installed and owned).
    records: HashMap<u32, Game>,
    /// Cover art: Steam's cache, or downloads.
    art: covers::Covers,
    /// The latest frame time.
    now: std::time::Instant,
    /// A newer rigcheck release's version, if one is out.
    newer_release: Option<String>,
    /// Sizes for Settings > Storage, refreshed when they can change.
    storage: settings_page::Storage,
    search: String,
    library_order: library::Order,
    library_scope: library::Scope,
    /// The user's Steam account, for ALL MY GAMES.
    account: account::Account,
    catalog_list: catalog::List,
    /// Live lookups of games games.db hasn't checked; in memory if the
    /// cache directory can't be written.
    fetched: Fetched,
    /// Every request goes through here.
    web: Web,
    lookup: lookup::State,
    /// The open game page's record, and the tab it was opened from.
    opened: Option<Game>,
    opened_from: Page,
    override_choices: overrides::Choices,
    whatif: whatif::Picks,
    /// The game picked to compare with another.
    compare_pick: Option<u32>,
    whatif_choices: overrides::Choices,
}

/// The tabs in the top bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Library,
    /// A game, by app ID: installed, or picked in the Catalog.
    Game(u32),
    Catalog,
    MyRig,
    Settings,
    About,
    /// Two games side by side, by app ID.
    Compare(u32, u32),
}

#[derive(Debug, Clone)]
enum Message {
    Open(Page),
    HardwareDetected(Hardware),
    GamesFound(Vec<InstalledGame>),
    Search(String),
    LibraryOrder(library::Order),
    LibraryScope(library::Scope),
    /// Settings > Steam account, and Steam's answers about it.
    Account(account::Message),
    /// Enter in the Library's search: open the first game listed.
    LibraryOpenFirst,
    Shortcut(keys::Shortcut),
    /// A hardware-table name, or `None` to go back to what was detected.
    OverrideGpu(Option<String>),
    OverrideCpu(Option<String>),
    CopyHardwareReport,
    /// My Rig > What if: hypothetical parts (`None` keeps your own).
    WhatIfGpu(Option<String>),
    WhatIfCpu(Option<String>),
    WhatIfRam(Option<u32>),
    /// Settings > Privacy: check for a new games.db only when asked.
    SetManualUpdates(bool),
    /// Settings > Storage: the image cache limit in MB.
    ImageCacheLimit(u32),
    CheckForUpdates,
    Redownload,
    ClearFetched,
    ClearImages,
    AppearanceSelected(Appearance),
    /// `None` picks "System default".
    LanguageSelected(Option<Language>),
    /// A frame is drawn while something animates.
    Frame(std::time::Instant),
    /// The system switched between light and dark (or said which it is).
    SystemTheme(iced::theme::Mode),
    Download(download::Event),
    CatalogSearch(String),
    CatalogFilter(catalog::Chip),
    /// Only games ProtonDB rates this or better; `None` for all.
    CatalogProtonDb(Option<rigcheck_core::ProtonTier>),
    /// The Catalog list's scroll position and height, in pixels.
    CatalogScrolled {
        offset: f32,
        height: f32,
        width: f32,
    },
    RetryDownload,
    /// Opens a web page in the user's browser.
    OpenUrl(String),
    /// The latest release's version (`version.txt`), or why there's none.
    LatestRelease(Result<String, web::Failure>),
    /// Steam's `appdetails` answer for an app ID, or why there's none.
    SteamLookup(u32, Result<String, web::Failure>),
    RetryLookup(u32),
    /// A game page's COMPARE: remember this game (again: forget it).
    PickToCompare(u32),
    /// A cover downloaded from Steam's CDN, or why it couldn't be.
    CoverFetched(u32, Result<Vec<u8>, web::Failure>),
}

impl App {
    /// Starting state plus work to run right away. Detection reads files and
    /// runs `vulkaninfo`, so it runs as a `Task` on a background thread
    /// instead of freezing the window.
    fn new(paths: Paths, system_language: Language) -> (Self, Task<Message>) {
        let detect = Task::perform(
            async { rigcheck_hardware::detect(Path::new("/"), &rigcheck_hardware::System) },
            Message::HardwareDetected,
        );
        let home = paths.home.clone();
        let scan = Task::perform(
            async move { rigcheck_steam::installed_games(&home) },
            Message::GamesFound,
        );
        let db_path = paths.data.join("games.db");
        let db = db_path
            .exists()
            .then(|| GameDb::open(&db_path).ok())
            .flatten();
        let settings = Settings::load(&paths.config);
        let web = Web::net();
        let mut download = download::Downloader::new(web.clone(), db_path);
        let fetch = match &db {
            // A background update check (ROADMAP §7.3), unless turned off.
            Some(db) if !settings.manual_updates => download.check(db.db_version()),
            Some(_) => Task::none(),
            None => download.start(None, false),
        };
        // Same setting: is there a newer rigcheck?
        let release_check = if settings.manual_updates {
            Task::none()
        } else {
            release::check(&web)
        };
        let fetched = open_fetched(&paths.cache);
        // Same setting as the other start-up checks.
        let (account, owned_check) = account::Account::new(
            paths.config.clone(),
            paths.cache.clone(),
            &paths.home,
            web.clone(),
            !settings.manual_updates,
        );
        let mut art =
            covers::Covers::new(paths.home.clone(), paths.cache.join("images"), web.clone());
        art.evict(settings.image_cache_bytes());
        let mut app = App {
            override_choices: overrides::Choices::default(),
            whatif: whatif::Picks::default(),
            compare_pick: None,
            whatif_choices: overrides::Choices::default(),
            i18n: I18n::new(settings.language.unwrap_or(system_language)),
            settings,
            system_language,
            system_mode: iced::theme::Mode::None,
            paths,
            page: Page::Library,
            hardware: None,
            rig: None,
            db: None,
            download,
            catalog: Catalog::default(),
            games: None,
            records: HashMap::new(),
            art,
            storage: settings_page::Storage::default(),
            newer_release: None,
            now: std::time::Instant::now(),
            search: String::new(),
            library_order: library::Order::default(),
            library_scope: library::Scope::default(),
            account,
            catalog_list: catalog::List::default(),
            fetched,
            web,
            lookup: lookup::State::Idle,
            opened: None,
            opened_from: Page::Library,
        };
        if let Some(db) = db {
            app.use_db(db);
        }
        let system_mode = iced::system::theme().map(Message::SystemTheme);
        (
            app,
            Task::batch([detect, scan, fetch, release_check, system_mode, owned_check]),
        )
    }

    /// Switches to `db`: its hardware table, and its records for the
    /// Library's games found so far.
    fn use_db(&mut self, db: GameDb) {
        self.catalog = db.catalog().unwrap_or_default();
        self.override_choices = overrides::Choices::of(&self.catalog);
        self.whatif_choices = overrides::Choices::of(&self.catalog);
        // Lookups this games.db has caught up with, or that went stale.
        if let Err(error) = self.fetched.prune(&db, now()) {
            eprintln!("couldn't prune fetched.db: {error}");
        }
        self.db = Some(db);
        self.records_changed();
        // A new hardware table can score the same parts differently.
        self.rig_changed();
    }

    /// Where every page reads a game's record from (ROADMAP §7.1). `None`
    /// until there's a games.db.
    fn records(&self) -> Option<Records<'_>> {
        Some(Records::new(self.db.as_ref()?, &self.fetched, now()))
    }

    /// Reads every page's records again, after games.db or fetched.db
    /// changed: the Catalog's rows in view, the open game page's and the
    /// Library's.
    fn records_changed(&mut self) {
        if let Some(db) = &self.db {
            let records = Records::new(db, &self.fetched, now());
            self.catalog_list.refresh(&records);
            if let Page::Game(app_id) = self.page {
                self.opened = records.get(app_id).ok().flatten();
            }
        }
        self.load_records();
    }

    /// Reads the record of each of the Library's games.
    fn load_records(&mut self) {
        let Some(records) = self.records() else {
            return;
        };
        let installed = self.games.iter().flatten().map(|game| game.app_id);
        let owned = self.account.owned_games().iter().map(|game| game.app_id);
        let loaded = installed
            .chain(owned)
            .filter_map(|app_id| Some((app_id, records.get(app_id).ok()??)))
            .collect();
        self.records = loaded;
    }

    /// The Library's games for the current scope, as (app ID, name).
    fn library_games(&self) -> Option<Vec<(u32, &str)>> {
        match self.library_scope {
            library::Scope::Installed => self.games.as_ref().map(|games| {
                games
                    .iter()
                    .map(|game| (game.app_id, game.name.as_str()))
                    .collect()
            }),
            library::Scope::Owned => Some(
                self.account
                    .owned_games()
                    .iter()
                    .map(|game| (game.app_id, game.name.as_str()))
                    .collect(),
            ),
        }
    }

    /// Why ALL MY GAMES has nothing to list, if it hasn't.
    fn library_note(&self) -> Option<String> {
        if self.library_scope != library::Scope::Owned {
            return None;
        }
        self.account.note().map(|id| self.i18n.t(id))
    }

    /// What verdicts compare against. `None` until detection finishes.
    fn specs(&self) -> Option<&Specs> {
        self.rig.as_ref().map(Rig::specs)
    }

    /// Your rig with the What if picks in place of your parts.
    fn what_if_rig(&self) -> Option<Rig> {
        let swap = Swap {
            gpu: self.whatif.gpu.as_deref(),
            cpu: self.whatif.cpu.as_deref(),
            ram_gb: self.whatif.ram_gb,
        };
        Some(self.rig.as_ref()?.swapped(&self.catalog, swap))
    }

    /// "Cyberpunk 2077: TOO WEAK → LOW" for each installed game the What
    /// if picks would move to another stamp, by name.
    fn what_if_changes(&self) -> Vec<String> {
        let (now, then) = (self.specs(), self.what_if_rig());
        let then = then.as_ref().map(Rig::specs);
        let mut games: Vec<&InstalledGame> = self.games.iter().flatten().collect();
        games.sort_by_key(|game| game.name.to_lowercase());
        games
            .into_iter()
            .filter_map(|game| {
                let record = self.records.get(&game.app_id);
                let (before, _) = library::stamp(&library::status(record, now), &self.i18n);
                let (after, _) = library::stamp(&library::status(record, then), &self.i18n);
                (before != after).then(|| format!("{}: {before} → {after}", game.name))
            })
            .collect()
    }

    /// Your PC as the game page shows it, named as verdicts score it.
    fn rig_view(&self) -> game_page::Rig {
        let unknown = || self.i18n.t("unknown");
        let (Some(hardware), Some(rig)) = (&self.hardware, &self.rig) else {
            return game_page::Rig {
                cpu: unknown(),
                gpu: unknown(),
                ram: unknown(),
                vram: unknown(),
                api: unknown(),
            };
        };
        let gpu = hardware.gaming_gpu();
        let vulkan = gpu.and_then(|gpu| gpu.vulkan.as_ref());
        game_page::Rig {
            cpu: rig.cpu_name().map_or_else(unknown, str::to_owned),
            gpu: rig.gpu_name().map_or_else(unknown, str::to_owned),
            ram: hardware
                .ram_bytes
                .map(rigcheck_hardware::gib)
                .unwrap_or_else(unknown),
            vram: match gpu.map(|gpu| gpu.vram) {
                Some(rigcheck_hardware::Vram::Shared) => self.i18n.t("vram-shared"),
                Some(rigcheck_hardware::Vram::Dedicated(bytes)) => rigcheck_hardware::gib(bytes),
                _ => self.i18n.t("vram-unknown"),
            },
            api: vulkan.map_or_else(unknown, |vulkan| format!("Vulkan {}", vulkan.api_version)),
        }
    }

    fn status(&self, app_id: u32) -> library::Status {
        library::status(self.records.get(&app_id), self.specs())
    }

    /// The appearance to draw with: the picked one, or the system's.
    fn appearance(&self) -> Appearance {
        self.settings.appearance.resolve(self.system_mode)
    }

    /// Follows the system switching between light and dark, and listens for
    /// shortcuts (keys no widget used).
    fn subscription(&self) -> iced::Subscription<Message> {
        let mut subscriptions = vec![
            iced::system::theme_changes().map(Message::SystemTheme),
            iced::keyboard::listen().filter_map(key_shortcut),
        ];
        // Frames only while something moves: idle, the app doesn't redraw.
        if self.animating() {
            subscriptions.push(iced::window::frames().map(Message::Frame));
        }
        iced::Subscription::batch(subscriptions)
    }

    /// Whether a cover is loading (pulsing) or has just arrived (fading in).
    fn animating(&self) -> bool {
        self.art.animating(std::time::Instant::now())
    }

    /// `app_id`'s cover as drawn this frame.
    fn cover_of(&self, app_id: u32) -> library::Cover<'_> {
        self.art.cover_of(app_id, self.now)
    }

    fn shortcut(&mut self, shortcut: keys::Shortcut) -> Task<Message> {
        use keys::Shortcut;
        match (shortcut, self.page) {
            (Shortcut::Tab(n), _) => {
                let tabs = [
                    Page::Library,
                    Page::Catalog,
                    Page::MyRig,
                    Page::Settings,
                    Page::About,
                ];
                match tabs.get(usize::from(n).saturating_sub(1)) {
                    Some(&page) => self.update(Message::Open(page)),
                    None => Task::none(),
                }
            }
            (Shortcut::Search, Page::Library) => iced::widget::operation::focus(library::SEARCH),
            (Shortcut::Search, Page::Catalog) => iced::widget::operation::focus(catalog::SEARCH),
            (Shortcut::Back, Page::Game(_)) => self.update(Message::Open(self.opened_from)),
            (Shortcut::Back, Page::Compare(_, b)) => self.update(Message::Open(Page::Game(b))),
            (Shortcut::Move(_) | Shortcut::Step(_) | Shortcut::Page(_), Page::Catalog) => {
                match &self.db {
                    Some(db) => {
                        let records = Records::new(db, &self.fetched, now());
                        let list = &self.catalog_list;
                        let (columns, rows) = (list.columns() as i64, list.rows_in_view() as i64);
                        let tiles = match shortcut {
                            Shortcut::Move(by) => i64::from(by) * columns,
                            Shortcut::Page(by) => i64::from(by) * rows * columns,
                            Shortcut::Step(by) => i64::from(by),
                            _ => 0,
                        };
                        let offset = self.catalog_list.move_selection(tiles, &records);
                        iced::widget::operation::scroll_to(
                            catalog::LIST,
                            iced::widget::operation::AbsoluteOffset {
                                x: None,
                                y: Some(offset),
                            },
                        )
                    }
                    None => Task::none(),
                }
            }
            (Shortcut::Open, Page::Catalog) => {
                match self
                    .records()
                    .and_then(|records| self.catalog_list.selected_game(&records))
                {
                    Some(app_id) => self.update(Message::Open(Page::Game(app_id))),
                    None => Task::none(),
                }
            }
            _ => Task::none(),
        }
    }

    fn theme(&self) -> Theme {
        self.appearance().theme()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Open(Page::Settings) => {
                self.page = Page::Settings;
                self.refresh_storage();
                Task::none()
            }
            Message::SetManualUpdates(manual) => {
                self.settings.manual_updates = manual;
                self.save_settings();
                Task::none()
            }
            Message::ImageCacheLimit(mb) => {
                self.settings.image_cache_mb = Some(mb);
                self.save_settings();
                self.art.evict(self.settings.image_cache_bytes());
                self.refresh_storage();
                Task::none()
            }
            Message::CheckForUpdates | Message::Redownload => {
                let installed = match message {
                    Message::CheckForUpdates => self.db.as_ref().and_then(GameDb::db_version),
                    _ => None,
                };
                self.download.start(installed, self.db.is_some())
            }
            Message::ClearFetched => {
                if let Err(error) = self.fetched.clear() {
                    eprintln!("couldn't clear fetched.db: {error}");
                }
                self.lookup = lookup::State::Idle;
                self.records_changed();
                self.refresh_storage();
                Task::none()
            }
            Message::ClearImages => {
                self.art.clear();
                self.refresh_storage();
                Task::none()
            }
            Message::Open(page) => {
                if let Page::Game(app_id) = page {
                    if !matches!(self.page, Page::Game(_)) {
                        self.opened_from = self.page;
                    }
                    self.page = page;
                    return self.open_game(app_id);
                }
                self.page = page;
                // The tiles in view need their covers, again if the cache
                // limit deleted some.
                match page {
                    Page::Catalog => self.catalog_covers(),
                    Page::Library => self.library_covers(),
                    _ => Task::none(),
                }
            }
            Message::LatestRelease(answer) => {
                // Offline, or no release yet: nothing to say.
                self.newer_release = answer
                    .ok()
                    .filter(|latest| release::is_newer(latest, env!("CARGO_PKG_VERSION")))
                    .map(|latest| latest.trim().to_owned());
                Task::none()
            }
            Message::SteamLookup(app_id, answer) => {
                self.looked_up(app_id, answer);
                Task::none()
            }
            Message::CoverFetched(app_id, jpeg) => {
                self.art
                    .fetched(app_id, jpeg, self.settings.image_cache_bytes());
                Task::none()
            }
            Message::PickToCompare(app_id) => {
                self.compare_pick = (self.compare_pick != Some(app_id)).then_some(app_id);
                Task::none()
            }
            Message::RetryLookup(app_id) => {
                self.lookup = lookup::State::Checking(app_id);
                lookup::ask(&self.web, app_id)
            }
            Message::Frame(now) => {
                self.now = now;
                Task::none()
            }
            Message::SystemTheme(mode) => {
                self.system_mode = mode;
                Task::none()
            }
            Message::AppearanceSelected(appearance) => {
                self.settings.appearance = appearance;
                self.save_settings();
                Task::none()
            }
            Message::LanguageSelected(language) => {
                self.settings.language = language;
                self.i18n = I18n::new(language.unwrap_or(self.system_language));
                self.save_settings();
                Task::none()
            }
            Message::HardwareDetected(hardware) => {
                self.hardware = Some(hardware);
                self.rig_changed();
                Task::none()
            }
            Message::GamesFound(games) => {
                let downloads: Vec<_> = games.iter().map(|game| self.cover(game.app_id)).collect();
                self.games = Some(games);
                self.load_records();
                Task::batch(downloads)
            }
            Message::LibraryOpenFirst => {
                let query = self.search.to_lowercase();
                let first = self
                    .library_games()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|(_, name)| name.to_lowercase().contains(&query))
                    .min_by_key(|(_, name)| name.to_lowercase())
                    .map(|(app_id, _)| app_id);
                match first {
                    Some(app_id) => self.update(Message::Open(Page::Game(app_id))),
                    None => Task::none(),
                }
            }
            Message::Shortcut(shortcut) => self.shortcut(shortcut),
            Message::LibraryScope(scope) => {
                self.library_scope = scope;
                // The owned games' covers: Steam's cache, else downloaded.
                self.library_covers()
            }
            Message::Account(message) => {
                let answer = matches!(message, account::Message::Owned(_));
                let task = self.account.update(message);
                if answer {
                    self.load_records();
                }
                task
            }
            Message::LibraryOrder(order) => {
                self.library_order = order;
                Task::none()
            }
            Message::Search(query) => {
                self.search = query;
                Task::none()
            }
            Message::OverrideGpu(name) => {
                self.settings.gpu_override = name;
                self.save_settings();
                self.rig_changed();
                Task::none()
            }
            Message::OverrideCpu(name) => {
                self.settings.cpu_override = name;
                self.save_settings();
                self.rig_changed();
                Task::none()
            }
            Message::Download(event) => {
                if let Some(db) = self.download.event(event, self.db.is_some()) {
                    self.use_db(db);
                }
                Task::none()
            }
            Message::CatalogSearch(query) => {
                self.relist_catalog(|list, records| list.search(query, records))
            }
            Message::CatalogProtonDb(lowest) => {
                self.relist_catalog(|list, records| list.protondb(lowest, records))
            }
            Message::CatalogFilter(chip) => {
                let specs = self.specs().cloned();
                self.relist_catalog(|list, records| list.toggle(chip, specs, records))
            }
            Message::CatalogScrolled {
                offset,
                height,
                width,
            } => {
                if let Some(db) = &self.db {
                    let records = Records::new(db, &self.fetched, now());
                    self.catalog_list.scrolled(offset, height, width, &records);
                }
                self.catalog_covers()
            }
            Message::OpenUrl(url) => {
                // The desktop's own "open this link": the app sends nothing.
                std::thread::spawn(move || {
                    if let Err(error) = std::process::Command::new("xdg-open").arg(&url).status() {
                        eprintln!("couldn't open {url}: {error}");
                    }
                });
                Task::none()
            }
            Message::RetryDownload => self.download.start(None, self.db.is_some()),
            Message::WhatIfGpu(name) => {
                self.whatif.gpu = name;
                Task::none()
            }
            Message::WhatIfCpu(name) => {
                self.whatif.cpu = name;
                Task::none()
            }
            Message::WhatIfRam(gb) => {
                self.whatif.ram_gb = gb;
                Task::none()
            }
            Message::CopyHardwareReport => match &self.hardware {
                Some(hardware) => clipboard::write(hardware.report()),
                None => Task::none(),
            },
        }
    }

    /// Changes what the Catalog lists and shows it from the top.
    fn relist_catalog(
        &mut self,
        change: impl FnOnce(&mut catalog::List, &Records),
    ) -> Task<Message> {
        let Some(db) = &self.db else {
            return Task::none();
        };
        change(
            &mut self.catalog_list,
            &Records::new(db, &self.fetched, now()),
        );
        Task::batch([catalog_to_top(), self.catalog_covers()])
    }

    /// Opens a game's page: its cover and its record.
    fn open_game(&mut self, app_id: u32) -> Task<Message> {
        let cover = self.cover(app_id);
        Task::batch([cover, self.look_up(app_id)])
    }

    /// Covers for the Catalog tiles in view: on disk, or downloaded now.
    fn catalog_covers(&mut self) -> Task<Message> {
        let ids = self.catalog_list.visible_ids();
        Task::batch(ids.into_iter().map(|app_id| self.cover(app_id)))
    }

    /// Covers for the Library's games in its current scope.
    fn library_covers(&mut self) -> Task<Message> {
        let ids: Vec<u32> = self
            .library_games()
            .unwrap_or_default()
            .into_iter()
            .map(|(app_id, _)| app_id)
            .collect();
        Task::batch(ids.into_iter().map(|app_id| self.cover(app_id)))
    }

    /// Shows `app_id`'s cover if one is on disk, else downloads it.
    fn cover(&mut self, app_id: u32) -> Task<Message> {
        self.art.want(app_id)
    }

    /// The record for the game page, else asks Steam (ROADMAP §7.1).
    fn look_up(&mut self, app_id: u32) -> Task<Message> {
        let Some(records) = self.records() else {
            self.opened = None;
            return Task::none();
        };
        let (opened, ask) = (
            records.get(app_id).ok().flatten(),
            records.needs_lookup(app_id),
        );
        self.opened = opened;
        if !ask {
            return Task::none();
        }
        self.lookup = lookup::State::Checking(app_id);
        lookup::ask(&self.web, app_id)
    }

    /// Keeps Steam's answer about `app_id` and shows it on every page.
    fn looked_up(&mut self, app_id: u32, answer: Result<String, web::Failure>) {
        let Some(db) = &self.db else {
            return;
        };
        let records = Records::new(db, &self.fetched, now());
        let stored = answer
            .map_err(|failure| failure.to_string())
            .and_then(
                |json| match ingest(app_id, &json).map_err(|error| error.to_string())? {
                    Ingested::Game(record) => records
                        .store_lookup(*record)
                        .map(|()| true)
                        .map_err(|error| error.to_string()),
                    Ingested::NotAGame { .. } | Ingested::Unavailable => Ok(false),
                },
            );
        self.lookup = match stored {
            Ok(true) => lookup::State::Idle,
            Ok(false) => lookup::State::NoRequirements(app_id),
            Err(error) => {
                eprintln!("couldn't look up {app_id} on Steam: {error}");
                lookup::State::Failed(app_id)
            }
        };
        if self.lookup == lookup::State::Idle {
            self.records_changed();
        }
    }

    /// A game's record by the lookup order of every page (ROADMAP §7.1).
    fn record(&self, app_id: u32) -> Option<Game> {
        self.records()?.get(app_id).ok().flatten()
    }

    /// A game's name: its record's, else Steam's for an installed game.
    fn game_name(&self, app_id: u32) -> String {
        self.record(app_id)
            .map(|game| game.name)
            .or_else(|| {
                let installed = self
                    .games
                    .iter()
                    .flatten()
                    .find(|game| game.app_id == app_id);
                installed.map(|game| game.name.clone())
            })
            .unwrap_or_else(|| app_id.to_string())
    }

    /// Measures what Settings > Storage shows.
    fn refresh_storage(&mut self) {
        let size = |path: PathBuf| std::fs::metadata(path).map_or(0, |meta| meta.len());
        self.storage = settings_page::Storage {
            games_db: size(self.paths.data.join("games.db")),
            fetched_db: size(self.paths.cache.join("fetched.db")),
            images: self.art.size(),
        };
    }

    /// Reads the hardware against the hardware table again, with any
    /// override, and lets the Catalog's CAN RUN use the result.
    fn rig_changed(&mut self) {
        let overrides = Swap {
            gpu: self.settings.gpu_override.as_deref(),
            cpu: self.settings.cpu_override.as_deref(),
            ram_gb: None,
        };
        self.rig = self
            .hardware
            .as_ref()
            .map(|hardware| Rig::new(&self.catalog, hardware).swapped(&self.catalog, overrides));
        let specs = self.specs().cloned();
        if let Some(db) = &self.db {
            let records = Records::new(db, &self.fetched, now());
            self.catalog_list.use_specs(specs, &records);
        }
    }

    fn save_settings(&self) {
        if let Err(error) = self.settings.save(&self.paths.config) {
            eprintln!("couldn't save settings: {error}");
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let colors = self.appearance().colors();
        let i18n = &self.i18n;
        let page = match self.page {
            Page::Library => {
                let entries = self.library_games().map(|games| {
                    games
                        .into_iter()
                        .map(|(app_id, name)| library::Entry {
                            app_id,
                            name,
                            status: self.status(app_id),
                            cover: self.cover_of(app_id),
                        })
                        .collect()
                });
                library::view(
                    entries,
                    &self.search,
                    self.library_order,
                    self.library_scope,
                    self.library_note(),
                    i18n,
                    colors,
                )
            }
            Page::Game(app_id) => {
                // Installed games that aren't in games.db have only Steam's name.
                let installed = self
                    .games
                    .iter()
                    .flatten()
                    .find(|game| game.app_id == app_id);
                let name = self
                    .opened
                    .as_ref()
                    .map(|game| game.name.clone())
                    .or_else(|| installed.map(|game| game.name.clone()));
                match name {
                    Some(name) => game_page::view(
                        app_id,
                        name,
                        lookup::note(self.lookup, app_id, i18n, colors),
                        self.opened.as_ref(),
                        &library::status(self.opened.as_ref(), self.specs()),
                        &self.rig_view(),
                        self.cover_of(app_id),
                        self.opened_from,
                        self.compare_pick
                            .map(|other| (other, self.game_name(other))),
                        i18n,
                        colors,
                    ),
                    None => coming_soon(i18n.t("tab-library"), i18n, colors),
                }
            }
            Page::Catalog => catalog::view(
                &self.catalog_list,
                self.specs(),
                &|app_id| self.cover_of(app_id),
                i18n,
                colors,
            ),
            Page::MyRig => match &self.hardware {
                Some(hardware) => {
                    let rig = self.rig_view();
                    let picked = overrides::Picked {
                        gpu: self.settings.gpu_override.as_ref(),
                        cpu: self.settings.cpu_override.as_ref(),
                        gpu_in_use: rig.gpu.clone(),
                        cpu_in_use: rig.cpu.clone(),
                    };
                    // Room on the right and bottom so hard shadows aren't clipped.
                    scrollable(
                        column![
                            my_hardware::view(hardware, i18n, colors),
                            overrides::view(&self.override_choices, picked, i18n, colors),
                            whatif::view(
                                &self.whatif_choices,
                                &self.whatif,
                                (rig.gpu, rig.cpu),
                                self.what_if_changes(),
                                i18n,
                                colors,
                            ),
                        ]
                        .spacing(18)
                        .padding([0, 6]),
                    )
                    .into()
                }
                None => center(text(i18n.t("detecting-hardware"))).into(),
            },
            Page::About => about::view(i18n, colors),
            Page::Compare(a, b) => {
                let side = |app_id| {
                    let record = self.record(app_id);
                    compare::Side {
                        app_id,
                        name: self.game_name(app_id),
                        status: library::status(record.as_ref(), self.specs()),
                        record,
                    }
                };
                compare::view(side(a), side(b), i18n, colors)
            }
            Page::Settings => settings_page::view(
                &self.settings,
                self.storage,
                self.download.running(),
                self.account.view(i18n, colors),
                i18n,
                colors,
            ),
        };
        let mut body = column![].spacing(18);
        if let Some(banner) = download::view(self.download.status(), i18n, colors) {
            body = body.push(banner);
        }
        if let Some(version) = &self.newer_release {
            body = body.push(release::notice(version, i18n, colors));
        }
        column![
            self.top_bar(colors),
            container(body.push(page))
                .padding(18)
                .width(Fill)
                .height(Fill),
        ]
        .into()
    }

    /// Design D's top bar: pixel logo on the left, tabs on the right.
    fn top_bar(&self, colors: Colors) -> Element<'_, Message> {
        let tab = |id: &str, page: Page| {
            button(text(self.i18n.t(id)).font(BOLD).size(13))
                .padding([7, 12])
                .style(colors.button(
                    self.page == page
                        || (page == self.opened_from && matches!(self.page, Page::Game(_))),
                ))
                .on_press(Message::Open(page))
        };
        // One handle for the app's life: `from_bytes` gives every handle a
        // new ID, so a fresh one each redraw is a new image to decode.
        static LOGO: std::sync::LazyLock<Handle> =
            std::sync::LazyLock::new(|| Handle::from_bytes(ICON_PNG));
        let logo = image(LOGO.clone())
            .width(36)
            .height(36)
            .filter_method(FilterMethod::Nearest); // keep the pixel art crisp
        let bar = row![
            logo,
            text("RIGCHECK").font(BOLD).size(18),
            space::horizontal(),
            tab("tab-library", Page::Library),
            tab("tab-catalog", Page::Catalog),
            tab("tab-my-rig", Page::MyRig),
            tab("tab-settings", Page::Settings),
            tab("tab-about", Page::About),
        ]
        .spacing(10)
        .align_y(iced::Center);
        let bar = container(bar)
            .padding([12, 18])
            .width(Fill)
            .style(move |_| container::Style {
                text_color: Some(colors.text),
                ..container::Style::default().background(colors.hud)
            });
        let rule = container(space::vertical())
            .width(Fill)
            .height(4)
            .style(move |_| container::Style::default().background(colors.rule));
        column![bar, rule].into()
    }
}

/// fetched.db in `dir`, created on first use; in memory if it can't be.
fn open_fetched(dir: &Path) -> Fetched {
    Fetched::open(&dir.join("fetched.db")).unwrap_or_else(|error| {
        eprintln!("couldn't open fetched.db, so lookups last this session: {error}");
        Fetched::in_memory()
    })
}

/// Seconds since 1970, the way games.db and fetched.db store times.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// A key press no widget used, as a shortcut message.
fn key_shortcut(event: iced::keyboard::Event) -> Option<Message> {
    match event {
        iced::keyboard::Event::KeyPressed { key, modifiers, .. } => {
            keys::shortcut(&key, modifiers).map(Message::Shortcut)
        }
        _ => None,
    }
}

/// Scrolls the Catalog list back to its first row.
fn catalog_to_top() -> Task<Message> {
    iced::widget::operation::snap_to(
        catalog::LIST,
        iced::widget::operation::RelativeOffset::START,
    )
}

/// Placeholder for pages that later phases build.
fn coming_soon(title: String, i18n: &I18n, colors: Colors) -> Element<'static, Message> {
    center(
        container(column![text(title).font(BOLD).size(20), text(i18n.t("coming-soon"))].spacing(6))
            .padding(18)
            .style(move |_| colors.boxed(colors.panel)),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use i18n::Language;
    use settings::Appearance;
    use tempfile::TempDir;

    #[test]
    fn a_fresh_install_opens_in_paper() {
        let config = TempDir::new().unwrap();

        let (app, _) = App::new(Paths::under(config.path()), Language::English);

        assert_eq!(app.theme().to_string(), "Paper");
    }

    #[test]
    fn picking_an_appearance_applies_and_saves_it() {
        let config = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(config.path()), Language::English);

        click(&mut app, "SETTINGS");
        click(&mut app, "INK");

        assert_eq!(app.theme().to_string(), "Ink");
        assert_eq!(
            Settings::load(&Paths::under(config.path()).config).appearance,
            Appearance::Ink
        );
    }

    #[test]
    fn the_app_reopens_in_the_saved_appearance() {
        let config = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(config.path()), Language::English);
        click(&mut app, "SETTINGS");
        click(&mut app, "INK");

        let (reopened, _) = App::new(Paths::under(config.path()), Language::English);

        assert_eq!(reopened.theme().to_string(), "Ink");
    }

    #[test]
    fn picking_a_language_translates_the_app_and_is_remembered() {
        let config = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(config.path()), Language::English);

        click(&mut app, "SETTINGS");
        click(&mut app, "DEUTSCH");

        assert!(shows(&app, "EINSTELLUNGEN"));
        let (reopened, _) = App::new(Paths::under(config.path()), Language::English);
        assert!(shows(&reopened, "EINSTELLUNGEN"));
    }

    fn shows(app: &App, text: &str) -> bool {
        iced_test::simulator(app.view()).find(text).is_ok()
    }

    /// Clicks the widget showing `label` and runs the messages it produces.
    fn click(app: &mut App, label: &str) {
        let mut ui = iced_test::simulator(app.view());
        if let Err(error) = ui.click(label) {
            panic!("couldn't click {label:?}: {error}");
        }
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
    }

    #[test]
    fn says_it_is_detecting_hardware_at_startup() {
        let config = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(config.path()), Language::English);

        click(&mut app, "MY RIG");

        assert!(shows(&app, "Detecting your hardware…"));
    }

    /// An app on the AMD laptop with Cyberpunk 2077 and OMORI installed.
    fn library_app(dir: &TempDir) -> App {
        let paths = Paths::under(dir.path());
        std::fs::create_dir_all(&paths.data).unwrap();
        GameDb::create_seed(&paths.data.join("games.db")).unwrap();
        let (mut app, _) = App::new(Paths::under(dir.path()), Language::English);
        let _ = app.update(Message::HardwareDetected(test_support::amd_laptop()));
        let _ = app.update(Message::GamesFound(vec![
            test_support::installed(1091500, "Cyberpunk 2077"),
            test_support::installed(1150690, "OMORI"),
        ]));
        app
    }

    #[test]
    fn the_library_shows_each_installed_game_with_its_verdict() {
        let dir = TempDir::new().unwrap();
        let app = library_app(&dir);

        for text in ["Cyberpunk 2077", "TOO WEAK", "OMORI", "READY"] {
            assert!(shows(&app, text), "{text} not shown");
        }
    }

    #[test]
    fn clicking_a_game_opens_its_page() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "Cyberpunk 2077");

        for text in [
            "Your GPU is below the minimum.",
            "GeForce GTX 1060 6GB or Radeon RX 580 8GB or Arc A380",
            "Radeon Vega 7 (Ryzen 5000/7000U)",
            "PLATINUM",
        ] {
            assert!(shows(&app, text), "{text} not shown");
        }
    }

    #[test]
    fn overriding_the_gpu_is_saved_shown_and_used_for_verdicts() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        assert!(shows(&app, "TOO WEAK")); // Cyberpunk on the iGPU

        click(&mut app, "MY RIG");
        let mut ui = iced_test::simulator(app.view());
        ui.click(iced::widget::Id::new(overrides::GPU)).unwrap();
        ui.typewrite("RTX 3060");
        // Enter picks the highlighted match, as it does for keyboard users.
        ui.tap_key(iced::keyboard::key::Named::Enter);
        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        let saved = Settings::load(&Paths::under(dir.path()).config);
        assert_eq!(saved.gpu_override.as_deref(), Some("GeForce RTX 3060"));
        assert!(shows(&app, "GeForce RTX 3060"));
        click(&mut app, "LIBRARY");
        assert!(!shows(&app, "TOO WEAK"));
    }

    #[test]
    fn searching_filters_the_library() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        let mut ui = iced_test::simulator(app.view());
        ui.click(iced::widget::Id::new(library::SEARCH)).unwrap();
        ui.typewrite("omo");
        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        assert!(shows(&app, "OMORI"));
        assert!(!shows(&app, "Cyberpunk 2077"));
    }

    #[test]
    fn a_fresh_install_downloads_the_game_database_with_progress() {
        let dir = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(dir.path()), Language::English);

        assert!(shows(&app, "Downloading the game database…"));
        let _ = app.update(Message::Download(download::Event::Progress {
            done: 3_200_000,
            total: 6_400_000,
        }));

        assert!(shows(&app, "3.2 of 6.4 MB"));
    }

    #[test]
    fn verdicts_appear_once_the_download_is_installed() {
        let dir = TempDir::new().unwrap();
        let paths = Paths::under(dir.path());
        let (mut app, _) = App::new(paths.clone(), Language::English);
        let _ = app.update(Message::HardwareDetected(test_support::amd_laptop()));
        let _ = app.update(Message::GamesFound(vec![test_support::installed(
            1091500,
            "Cyberpunk 2077",
        )]));
        assert!(shows(&app, "NO DATA"));

        // What a finished download leaves behind: games.db in the data dir.
        std::fs::create_dir_all(&paths.data).unwrap();
        GameDb::create_seed(&paths.data.join("games.db")).unwrap();
        let _ = app.update(Message::Download(download::Event::Finished(Ok(
            download::Fetched::Installed,
        ))));

        assert!(shows(&app, "TOO WEAK"));
        assert!(!shows(&app, "Downloading the game database…"));
    }

    #[test]
    fn a_failed_download_can_be_retried() {
        let dir = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(dir.path()), Language::English);

        let _ = app.update(Message::Download(download::Event::Finished(Err(
            download::Failure::Failed("offline".to_owned()),
        ))));

        assert!(shows(
            &app,
            "Couldn't download the game database. Check your connection."
        ));
        click(&mut app, "RETRY");
        assert!(shows(&app, "Downloading the game database…"));
    }

    #[test]
    fn a_database_too_new_for_this_build_asks_for_an_update() {
        let dir = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(dir.path()), Language::English);

        let _ = app.update(Message::Download(download::Event::Finished(Err(
            download::Failure::NeedsNewerApp,
        ))));

        assert!(shows(
            &app,
            "The game database needs a newer rigcheck. Update the app to get it."
        ));
        assert!(!shows(&app, "RETRY"));
    }

    /// A Catalog one tile wide: one game per row, so rows read like a list.
    const ONE_COLUMN: f32 = library::TILE_WIDTH;

    /// The seed with popularities (Cyberpunk 2077 most popular, then OMORI),
    /// plus a game the pipeline only knows the name of.
    fn catalog_db_app(dir: &TempDir) -> App {
        let paths = Paths::under(dir.path());
        std::fs::create_dir_all(&paths.data).unwrap();
        let db = GameDb::create_seed(&paths.data.join("games.db")).unwrap();
        db.store_names(&[(30, "Day of Defeat")]).unwrap();
        db.set_protondb(&[
            (1091500, None, 3000),
            (1150690, None, 2000),
            (30, None, 1000),
        ])
        .unwrap();
        let (mut app, _) = App::new(paths, Language::English);
        let _ = app.update(Message::HardwareDetected(test_support::amd_laptop()));
        app
    }

    /// [`catalog_db_app`] on the Catalog page.
    fn catalog_app(dir: &TempDir) -> App {
        let mut app = catalog_db_app(dir);
        click(&mut app, "CATALOG");
        app
    }

    #[test]
    fn the_catalog_lists_games_most_popular_first_drawing_only_rows_in_view() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        let row = catalog::ROW_HEIGHT;

        let _ = app.update(Message::CatalogScrolled {
            offset: 0.0,
            height: row,
            width: ONE_COLUMN,
        });
        assert!(shows(&app, "Cyberpunk 2077"));
        assert!(shows(&app, "TOO WEAK"));
        assert!(!shows(&app, "OMORI"));

        let _ = app.update(Message::CatalogScrolled {
            offset: row,
            height: row,
            width: ONE_COLUMN,
        });
        assert!(shows(&app, "OMORI"));
        assert!(shows(&app, "READY"));
        assert!(!shows(&app, "Cyberpunk 2077"));
    }

    #[test]
    fn a_game_known_only_by_name_is_marked_not_checked_yet() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        let row = catalog::ROW_HEIGHT;

        let _ = app.update(Message::CatalogScrolled {
            offset: 2.0 * row,
            height: row,
            width: ONE_COLUMN,
        });

        assert!(shows(&app, "Day of Defeat"));
        assert!(shows(&app, "NOT CHECKED YET"));
    }

    #[test]
    fn searching_the_catalog_starts_its_matches_from_the_top() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        let row = catalog::ROW_HEIGHT;
        let _ = app.update(Message::CatalogScrolled {
            offset: row,
            height: row,
            width: ONE_COLUMN,
        });

        let mut ui = iced_test::simulator(app.view());
        ui.click(iced::widget::Id::new(catalog::SEARCH)).unwrap();
        ui.typewrite("cyber");
        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        assert!(shows(&app, "Cyberpunk 2077"));
        assert!(!shows(&app, "OMORI"));
    }

    #[test]
    fn a_catalog_game_opens_its_page_and_leads_back_to_the_catalog() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        // Day of Defeat is the third tile of the first row.
        let _ = app.update(Message::CatalogScrolled {
            offset: 0.0,
            height: catalog::ROW_HEIGHT,
            width: THREE_COLUMNS,
        });

        click(&mut app, "Day of Defeat");
        assert!(shows(
            &app,
            "rigcheck hasn't read this game's store page yet."
        ));

        click(&mut app, "◀ CATALOG");
        assert!(shows(&app, "Search every Steam game"));
    }

    #[test]
    fn a_filter_chip_narrows_the_catalog_from_the_top() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        let row = catalog::ROW_HEIGHT;
        let _ = app.update(Message::CatalogScrolled {
            offset: row,
            height: row,
            width: ONE_COLUMN,
        });

        click(&mut app, "NATIVE LINUX");

        // Native seed games have no popularity, so they list by name.
        assert!(shows(&app, "Counter-Strike 2"));
        assert!(!shows(&app, "OMORI"));
    }

    #[test]
    fn can_run_follows_a_hardware_override() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        let row = catalog::ROW_HEIGHT;

        click(&mut app, "CAN RUN");
        let _ = app.update(Message::CatalogScrolled {
            offset: 0.0,
            height: row,
            width: ONE_COLUMN,
        });
        // Cyberpunk is the most popular, but too much for the iGPU.
        assert!(shows(&app, "OMORI"));
        assert!(!shows(&app, "Cyberpunk 2077"));

        let _ = app.update(Message::OverrideGpu(Some("GeForce RTX 3060".to_owned())));

        assert!(shows(&app, "Cyberpunk 2077"));
    }

    #[test]
    fn the_protondb_filter_keeps_games_rated_at_least_that() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        let row = catalog::ROW_HEIGHT;

        click(&mut app, "GOLD+");
        let _ = app.update(Message::CatalogScrolled {
            offset: 0.0,
            height: row,
            width: ONE_COLUMN,
        });

        // catalog_app's popularity rows have no tier, so the seed's Gold and
        // Platinum games are left, by name: Balatro (Platinum) first.
        assert!(shows(&app, "Balatro"));
        assert!(!shows(&app, "Cyberpunk 2077"));
    }

    #[test]
    fn each_requirement_row_shows_how_this_pc_compares() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        // tests/verdict.rs in gamedb: OMORI's RAM meets the recommended 8 GB;
        // its CPU and GPU aren't in the hardware table.
        click(&mut app, "OMORI");
        assert!(shows(&app, "MEETS RECOMMENDED"));
        assert!(shows(&app, "NOT COMPARED"));

        click(&mut app, "◀ LIBRARY");
        click(&mut app, "Cyberpunk 2077");
        assert!(shows(&app, "BELOW MINIMUM"));
    }

    #[test]
    fn the_game_page_shows_steams_requirement_text() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "OMORI");

        // From OMORI's store page in the requirement corpus.
        for line in [
            "Memory: 4 GB RAM",
            "Memory: 8 GB RAM",
            "Additional Notes: 1280x720 display is recommended at x2 resolution but is not required",
        ] {
            assert!(shows(&app, line), "{line} not shown");
        }
    }

    #[test]
    fn the_store_button_opens_the_games_steam_page() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        click(&mut app, "OMORI");

        // Clicked by hand: running the message would open a browser.
        let mut ui = iced_test::simulator(app.view());
        ui.click("STEAM STORE").unwrap();
        let messages: Vec<Message> = ui.into_messages().collect();

        assert!(
            matches!(
                messages.as_slice(),
                [Message::OpenUrl(url)] if url == "https://store.steampowered.com/app/1150690/"
            ),
            "{messages:?}"
        );
    }

    const BG3: u32 = 1086940;

    /// A games.db that knows Baldur's Gate 3 only by name, on the AMD laptop.
    fn lookup_app(dir: &TempDir) -> App {
        lookup_app_knowing(
            dir,
            &[
                (BG3, "Baldur's Gate 3"),
                (2138330, "Cyberpunk 2077: Phantom Liberty"),
            ],
        )
    }

    /// A games.db that knows `names` only by name, on the AMD laptop.
    fn lookup_app_knowing(dir: &TempDir, names: &[(u32, &str)]) -> App {
        let paths = Paths::under(dir.path());
        std::fs::create_dir_all(&paths.data).unwrap();
        let db = GameDb::create(&paths.data.join("games.db")).unwrap();
        let table = include_str!("../../data/hardware.toml");
        db.store_catalog(&Catalog::from_toml(table).unwrap())
            .unwrap();
        db.store_names(names).unwrap();
        let (mut app, _) = App::new(paths, Language::English);
        let _ = app.update(Message::HardwareDetected(test_support::amd_laptop()));
        app
    }

    /// Steam's saved `appdetails` response for `app_id`.
    fn steam_says(app_id: u32) -> Message {
        let path = format!("../data/fixtures/appdetails/{app_id}.json");
        Message::SteamLookup(app_id, Ok(std::fs::read_to_string(path).unwrap()))
    }

    #[test]
    fn a_game_not_checked_yet_is_looked_up_on_steam() {
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app(&dir);

        let _ = app.update(Message::Open(Page::Game(BG3)));
        assert!(shows(&app, "Checking Steam…"));

        let _ = app.update(steam_says(BG3));

        // Radeon Vega 7 (32) against the RX 480 (95) in data/hardware.toml.
        assert!(shows(&app, "TOO WEAK"));
        assert!(shows(&app, "Memory: 8 GB RAM"));
        assert!(!shows(&app, "Checking Steam…"));
    }

    #[test]
    fn a_looked_up_game_is_remembered_across_restarts() {
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app(&dir);
        let _ = app.update(Message::Open(Page::Game(BG3)));
        let _ = app.update(steam_says(BG3));

        let (mut reopened, _) = App::new(Paths::under(dir.path()), Language::English);
        let _ = reopened.update(Message::HardwareDetected(test_support::amd_laptop()));
        let _ = reopened.update(Message::Open(Page::Game(BG3)));

        assert!(shows(&reopened, "TOO WEAK"));
        assert!(!shows(&reopened, "Checking Steam…"));
    }

    #[test]
    fn a_looked_up_game_shows_its_verdict_in_the_library_and_the_catalog() {
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app(&dir);
        let bg3 = test_support::installed(BG3, "Baldur's Gate 3");
        let _ = app.update(Message::GamesFound(vec![bg3]));
        click(&mut app, "CATALOG");
        assert!(!shows(&app, "TOO WEAK"));

        let _ = app.update(Message::Open(Page::Game(BG3)));
        let _ = app.update(steam_says(BG3));

        click(&mut app, "LIBRARY");
        assert!(shows(&app, "TOO WEAK"));
        click(&mut app, "CATALOG");
        assert!(shows(&app, "TOO WEAK"));
    }

    #[test]
    fn a_failed_lookup_says_so_and_can_be_retried() {
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app(&dir);
        let _ = app.update(Message::Open(Page::Game(BG3)));

        let offline = web::Failure::Failed("offline".to_owned());
        let _ = app.update(Message::SteamLookup(BG3, Err(offline)));
        assert!(shows(&app, "Couldn't reach Steam."));

        click(&mut app, "RETRY");
        assert!(shows(&app, "Checking Steam…"));
    }

    #[test]
    fn an_app_that_isnt_a_game_has_no_requirements_to_check() {
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app(&dir);
        let phantom_liberty = 2138330; // a DLC
        let _ = app.update(Message::Open(Page::Game(phantom_liberty)));

        let _ = app.update(steam_says(phantom_liberty));

        assert!(shows(&app, "Steam lists no system requirements for this."));
        assert!(!shows(&app, "Checking Steam…"));
    }

    #[test]
    fn an_update_is_swapped_in_without_a_restart() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        assert!(shows(&app, "READY")); // OMORI
        assert!(!shows(&app, "Downloading the game database…"));

        let _ = app.update(Message::Download(download::Event::Progress {
            done: 1_000_000,
            total: 6_400_000,
        }));
        assert!(shows(&app, "Updating the game database…"));

        // The new build rates OMORI Borked: Proton can't run it.
        let paths = Paths::under(dir.path());
        let new = GameDb::create_seed(&paths.data.join("games.db")).unwrap();
        new.set_protondb(&[(1150690, Some(rigcheck_core::ProtonTier::Borked), 10)])
            .unwrap();
        let _ = app.update(Message::Download(download::Event::Finished(Ok(
            download::Fetched::Installed,
        ))));

        assert!(shows(&app, "BLOCKED"));
        assert!(!shows(&app, "Updating the game database…"));
    }

    #[test]
    fn a_failed_update_keeps_the_current_database_quietly() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        let _ = app.update(Message::Download(download::Event::Progress {
            done: 1_000_000,
            total: 6_400_000,
        }));

        let _ = app.update(Message::Download(download::Event::Finished(Err(
            download::Failure::Failed("offline".to_owned()),
        ))));

        assert!(shows(&app, "READY"));
        assert!(!shows(
            &app,
            "Couldn't download the game database. Check your connection."
        ));
        assert!(!shows(&app, "Updating the game database…"));
    }

    #[test]
    fn an_update_needing_a_newer_app_keeps_the_current_database() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        let _ = app.update(Message::Download(download::Event::Finished(Err(
            download::Failure::NeedsNewerApp,
        ))));

        assert!(shows(
            &app,
            "The game database needs a newer rigcheck. Update the app to get it."
        ));
        assert!(shows(&app, "READY"));
    }

    #[test]
    fn updates_can_be_left_for_when_asked() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        let paths = Paths::under(dir.path());
        let automatic = App::new(paths.clone(), Language::English).1.units();

        click(&mut app, "SETTINGS");
        // Privacy is at the bottom: a window tall enough to show it.
        let mut ui = iced_test::Simulator::with_size(
            iced_test::core::Settings::default(),
            (1100.0, 1400.0),
            app.view(),
        );
        ui.click("ONLY WHEN I ASK").unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        assert!(Settings::load(&paths.config).manual_updates);
        // The next start skips the games.db and app version checks.
        assert_eq!(App::new(paths, Language::English).1.units(), automatic - 2);
    }

    #[test]
    fn clearing_fetched_games_means_asking_steam_again() {
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app(&dir);
        let _ = app.update(Message::Open(Page::Game(BG3)));
        let _ = app.update(steam_says(BG3));

        click(&mut app, "SETTINGS");
        // Settings is long: a window tall enough to reach Storage.
        on_tall_window(&mut app, |ui| {
            ui.click("CLEAR FETCHED GAMES").unwrap();
        });
        let _ = app.update(Message::Open(Page::Game(BG3)));

        assert!(shows(&app, "Checking Steam…"));
    }

    #[test]
    fn clearing_fetched_games_takes_their_verdicts_off_the_catalog() {
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app(&dir);
        let _ = app.update(Message::Open(Page::Game(BG3)));
        let _ = app.update(steam_says(BG3));
        click(&mut app, "CATALOG");
        assert!(shows(&app, "TOO WEAK"));

        click(&mut app, "SETTINGS");
        on_tall_window(&mut app, |ui| {
            ui.click("CLEAR FETCHED GAMES").unwrap();
        });
        click(&mut app, "CATALOG");

        assert!(!shows(&app, "TOO WEAK"));
        assert!(shows(&app, "NOT CHECKED YET"));
    }

    #[test]
    fn the_catalogs_filters_list_a_looked_up_game_like_its_tile() {
        const STARDEW: u32 = 413150;
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app_knowing(&dir, &[(STARDEW, "Stardew Valley")]);
        let _ = app.update(Message::Open(Page::Game(STARDEW)));
        let _ = app.update(steam_says(STARDEW));
        click(&mut app, "CATALOG");
        assert!(shows(&app, "READY"));

        click(&mut app, "CAN RUN");
        assert!(shows(&app, "Stardew Valley"), "CAN RUN hides it");
        click(&mut app, "CAN RUN");
        click(&mut app, "NATIVE LINUX");
        assert!(shows(&app, "Stardew Valley"), "NATIVE LINUX hides it");
    }

    #[test]
    fn clearing_images_frees_their_space() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        let _ = app.update(Message::CoverFetched(1150690, Ok(vec![0; 1_500_000])));

        click(&mut app, "SETTINGS");
        assert!(shows(&app, "1.5 MB"));
        // Settings is long: a window tall enough to reach Storage.
        on_tall_window(&mut app, |ui| {
            ui.click("CLEAR IMAGES").unwrap();
        });

        assert!(shows(&app, "0.0 MB"));
        let images = Paths::under(dir.path()).cache.join("images");
        assert_eq!(
            covers::Covers::new(dir.path().join("home"), images, Web::net()).size(),
            0
        );
    }

    #[test]
    fn covers_come_back_after_clearing_images() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        let _ = app.update(Message::CoverFetched(1150690, Ok(vec![0; 1000])));

        let _ = app.update(Message::ClearImages);

        assert!(matches!(app.cover_of(1150690), library::Cover::Missing));
        assert_eq!(
            app.cover(1150690).units(),
            1,
            "OMORI's cover is asked for again"
        );
    }

    #[test]
    fn a_cover_the_cache_limit_deletes_is_downloaded_again() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        // Below what Settings offers, to keep the covers small.
        let _ = app.update(Message::ImageCacheLimit(1));
        let _ = app.update(Message::CoverFetched(1150690, Ok(vec![0; 800_000])));
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Cyberpunk 2077's cover is newer: OMORI's goes to make room.
        let _ = app.update(Message::CoverFetched(1091500, Ok(vec![0; 800_000])));

        assert!(matches!(app.cover_of(1150690), library::Cover::Missing));
        assert!(matches!(
            app.cover_of(1091500),
            library::Cover::Ready { .. }
        ));
        assert_eq!(
            app.cover(1150690).units(),
            1,
            "OMORI's cover is asked for again"
        );
    }

    #[test]
    fn a_second_database_download_waits_for_the_first() {
        let dir = TempDir::new().unwrap();
        // Checking for a newer games.db, as at every start.
        let mut app = library_app(&dir);

        assert_eq!(app.update(Message::CheckForUpdates).units(), 0);
        assert_eq!(app.update(Message::Redownload).units(), 0);

        let _ = app.update(Message::Download(download::Event::Finished(Ok(
            download::Fetched::UpToDate,
        ))));
        assert_eq!(app.update(Message::CheckForUpdates).units(), 1);
        assert_eq!(app.update(Message::CheckForUpdates).units(), 0);
    }

    #[test]
    fn the_image_cache_limit_is_saved() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "SETTINGS");
        // Settings is long: a window tall enough to reach Storage.
        on_tall_window(&mut app, |ui| {
            ui.click("250 MB").unwrap();
        });

        let saved = Settings::load(&Paths::under(dir.path()).config);
        assert_eq!(saved.image_cache_mb, Some(250));
    }

    #[test]
    fn re_downloading_the_database_starts_right_away() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        // The update check at start-up found nothing newer.
        let _ = app.update(Message::Download(download::Event::Finished(Ok(
            download::Fetched::UpToDate,
        ))));

        click(&mut app, "SETTINGS");
        // Settings is long: a window tall enough to reach Storage.
        on_tall_window(&mut app, |ui| {
            ui.click("RE-DOWNLOAD").unwrap();
        });

        assert!(shows(&app, "Updating the game database…"));
    }

    #[test]
    fn the_privacy_section_lists_every_place_the_app_talks_to() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "SETTINGS");

        // ROADMAP §6.
        for destination in [
            "github.com: the game database",
            "store.steampowered.com: games the database hasn't checked",
            "shared.akamai.steamstatic.com: cover art",
        ] {
            assert!(shows(&app, destination), "{destination} not listed");
        }
    }

    #[test]
    fn the_about_page_credits_every_data_source_with_its_license() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "ABOUT");

        // ROADMAP §9.
        for line in [
            format!("Version {}", env!("CARGO_PKG_VERSION")),
            "rigcheck: MIT License".to_owned(),
            "The game database: Open Database License (ODbL 1.0)".to_owned(),
            "Steam store data: © Valve Corporation".to_owned(),
            "ProtonDB reports: ODbL 1.0, via bdefore/protondb-data".to_owned(),
            "AreWeAntiCheatYet: MIT, © AreWeAntiCheatYet contributors".to_owned(),
            "Blender Open Data: CC0".to_owned(),
            "pci.ids: BSD-3-Clause or GPL-2.0".to_owned(),
        ] {
            assert!(shows(&app, &line), "{line} not shown");
        }
    }

    #[test]
    fn the_about_page_links_to_the_code_and_the_bug_tracker() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        click(&mut app, "ABOUT");

        for (button, url) in [
            ("SOURCE CODE", "https://github.com/Glory42/rigcheck"),
            ("REPORT A BUG", "https://github.com/Glory42/rigcheck/issues"),
        ] {
            let mut ui = iced_test::simulator(app.view());
            ui.click(button).unwrap();
            let messages: Vec<Message> = ui.into_messages().collect();
            assert!(
                matches!(messages.as_slice(), [Message::OpenUrl(opened)] if opened == url),
                "{button}: {messages:?}"
            );
        }
    }

    /// Wayland finds the window's icon through the .desktop file named
    /// after the app ID.
    #[test]
    fn the_desktop_file_is_named_after_the_app_id() {
        let path = format!("../packaging/{APP_ID}.desktop");
        let desktop = std::fs::read_to_string(&path).expect("packaging/<app id>.desktop");

        assert!(desktop.lines().any(|line| line == format!("Icon={APP_ID}")));
        assert!(desktop.lines().any(|line| line == "Exec=rigcheck"));
    }

    #[test]
    fn a_newer_release_is_announced_with_a_link() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        let _ = app.update(Message::LatestRelease(Ok("999.0.0\n".to_owned())));

        assert!(shows(&app, "rigcheck 999.0.0 is out."));
        let mut ui = iced_test::simulator(app.view());
        ui.click("GET IT").unwrap();
        let messages: Vec<Message> = ui.into_messages().collect();
        assert!(
            matches!(
                messages.as_slice(),
                [Message::OpenUrl(url)] if url == "https://github.com/Glory42/rigcheck/releases/latest"
            ),
            "{messages:?}"
        );
    }

    #[test]
    fn no_release_notice_without_a_newer_version() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        for answer in [
            Ok(env!("CARGO_PKG_VERSION").to_owned()),
            Ok("0.0.1".to_owned()),
            Err(web::Failure::Failed("offline".to_owned())),
        ] {
            let _ = app.update(Message::LatestRelease(answer));
            assert!(!shows(&app, "GET IT"));
        }
    }

    #[test]
    fn the_library_says_how_many_games_meet_the_recommended_specs() {
        let dir = TempDir::new().unwrap();
        let app = library_app(&dir);

        // OMORI is READY, Cyberpunk 2077 TOO WEAK on the AMD laptop.
        assert!(shows(
            &app,
            "1 of 2 installed games meet the recommended specs."
        ));
    }

    /// Where `text` is drawn.
    fn bounds(app: &App, text: &str) -> iced::Rectangle {
        iced_test::simulator(app.view())
            .find(text)
            .unwrap()
            .bounds()
    }

    #[test]
    fn the_library_can_be_grouped_by_verdict() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        // A–Z by default: Cyberpunk 2077 first, on the same row as OMORI.
        assert!(bounds(&app, "Cyberpunk 2077").x < bounds(&app, "OMORI").x);

        click(&mut app, "BY VERDICT");

        // READY comes before TOO WEAK.
        assert!(bounds(&app, "OMORI").y < bounds(&app, "Cyberpunk 2077").y);
    }

    /// Runs `act` on a window tall enough for all of My Rig, then the
    /// messages it produced.
    fn on_tall_window(app: &mut App, act: impl FnOnce(&mut iced_test::Simulator<'_, Message>)) {
        let mut ui = iced_test::Simulator::with_size(
            iced_test::core::Settings::default(),
            (1100.0, 1800.0),
            app.view(),
        );
        act(&mut ui);
        let messages: Vec<Message> = ui.into_messages().collect();
        for message in messages {
            let _ = app.update(message);
        }
    }

    #[test]
    fn what_if_lists_the_games_an_upgrade_changes() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        click(&mut app, "MY RIG");

        on_tall_window(&mut app, |ui| {
            ui.click(iced::widget::Id::new(whatif::GPU)).unwrap();
            ui.typewrite("RTX 3060");
            ui.tap_key(iced::keyboard::key::Named::Enter);
        });

        // By hand from data/hardware.toml: the RTX 3060 (175) meets the
        // recommended RTX 2060 Super (160); the Ryzen 5 7530U (190) only the
        // minimum Ryzen 5 1600 (150), not the 7800X3D (380). OMORI was READY.
        assert!(shows(&app, "Cyberpunk 2077: TOO WEAK → LOW"));
        assert!(!shows(&app, "OMORI: READY → READY"));
        // Nothing is saved: the rest of the app keeps the detected GPU.
        assert_eq!(
            Settings::load(&Paths::under(dir.path()).config).gpu_override,
            None
        );
        click(&mut app, "LIBRARY");
        assert!(shows(&app, "TOO WEAK"));
    }

    #[test]
    fn what_if_says_when_nothing_changes() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        click(&mut app, "MY RIG");

        on_tall_window(&mut app, |ui| {
            ui.click("32 GB").unwrap();
        });

        assert!(shows(&app, "No installed game would change verdict."));
    }

    #[test]
    fn two_games_can_be_compared_side_by_side() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "Cyberpunk 2077");
        click(&mut app, "COMPARE");
        click(&mut app, "◀ LIBRARY");
        click(&mut app, "OMORI");
        click(&mut app, "COMPARE WITH Cyberpunk 2077");

        // Both games, each with its verdict and its own results (gamedb's
        // tests/verdict.rs: Cyberpunk's GPU is below the minimum on the AMD
        // laptop; OMORI's RAM meets the recommended 8 GB).
        for text in [
            "Cyberpunk 2077",
            "OMORI",
            "TOO WEAK",
            "READY",
            "BELOW MINIMUM",
            "MEETS RECOMMENDED",
            "GeForce GTX 1060 6GB or Radeon RX 580 8GB or Arc A380",
        ] {
            assert!(shows(&app, text), "{text} not shown");
        }
    }

    #[test]
    fn a_game_isnt_offered_to_compare_with_itself() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "OMORI");
        click(&mut app, "COMPARE");

        assert!(!shows(&app, "COMPARE WITH OMORI"));
    }

    #[test]
    fn a_fresh_install_follows_the_systems_light_or_dark_mode() {
        let dir = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(dir.path()), Language::English);

        let _ = app.update(Message::SystemTheme(iced::theme::Mode::Dark));
        assert_eq!(app.theme().to_string(), "Ink");

        let _ = app.update(Message::SystemTheme(iced::theme::Mode::Light));
        assert_eq!(app.theme().to_string(), "Paper");
    }

    #[test]
    fn a_picked_appearance_ignores_the_system() {
        let dir = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(dir.path()), Language::English);
        click(&mut app, "SETTINGS");
        click(&mut app, "PAPER");

        let _ = app.update(Message::SystemTheme(iced::theme::Mode::Dark));

        assert_eq!(app.theme().to_string(), "Paper");
    }

    #[test]
    fn settings_offer_following_the_system_and_start_with_it() {
        let dir = TempDir::new().unwrap();
        let (mut app, _) = App::new(Paths::under(dir.path()), Language::English);

        click(&mut app, "SETTINGS");

        assert!(shows(&app, "SYSTEM"));
        assert_eq!(
            Settings::load(&Paths::under(dir.path()).config).appearance,
            Appearance::System
        );
    }

    #[test]
    fn keys_switch_tabs_and_escape_goes_back() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        let _ = app.update(Message::Shortcut(keys::Shortcut::Tab(2)));
        assert!(shows(&app, "Search every Steam game"));

        let _ = app.update(Message::Shortcut(keys::Shortcut::Tab(1)));
        click(&mut app, "OMORI");
        let _ = app.update(Message::Shortcut(keys::Shortcut::Back));
        assert!(shows(&app, "Search your games"));
    }

    #[test]
    fn the_catalog_can_be_walked_with_the_keyboard() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        let row = catalog::ROW_HEIGHT;
        let _ = app.update(Message::CatalogScrolled {
            offset: 0.0,
            height: row,
            width: ONE_COLUMN,
        });

        // Cyberpunk 2077, OMORI, then Day of Defeat (catalog_app's popularity).
        let _ = app.update(Message::Shortcut(keys::Shortcut::Move(1)));
        let _ = app.update(Message::Shortcut(keys::Shortcut::Move(1)));
        let _ = app.update(Message::Shortcut(keys::Shortcut::Open));

        assert!(shows(
            &app,
            "rigcheck hasn't read this game's store page yet."
        ));
    }

    #[test]
    fn enter_in_the_library_search_opens_the_first_match() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        let mut ui = iced_test::simulator(app.view());
        ui.click(iced::widget::Id::new(library::SEARCH)).unwrap();
        ui.typewrite("omo");
        ui.tap_key(iced::keyboard::key::Named::Enter);
        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        assert!(shows(&app, "Meets the recommended specs."));
    }

    /// Exactly three tiles wide.
    const THREE_COLUMNS: f32 = 3.0 * library::TILE_WIDTH + 2.0 * catalog::GAP;

    #[test]
    fn the_catalog_is_a_grid_of_covers_downloaded_as_they_come_into_view() {
        let dir = TempDir::new().unwrap();
        // Not on the Catalog yet, so no tile has asked for its cover.
        let mut app = catalog_db_app(&dir);
        let scroll_to_top = Message::CatalogScrolled {
            offset: 0.0,
            height: catalog::ROW_HEIGHT,
            width: THREE_COLUMNS,
        };

        let downloads = app.update(scroll_to_top.clone());

        // No art on disk for the three games in view (the test's home has no
        // Steam): one cover download each, and only once.
        assert_eq!(downloads.units(), 3);
        assert_eq!(app.update(scroll_to_top).units(), 0);
        // One row of three tiles, most popular first.
        let _ = app.update(Message::Open(Page::Catalog));
        for name in ["Cyberpunk 2077", "OMORI", "Day of Defeat"] {
            assert!(shows(&app, name), "{name} not shown");
        }
    }

    #[test]
    fn the_catalog_grid_is_walked_across_and_down() {
        let dir = TempDir::new().unwrap();
        let mut app = catalog_app(&dir);
        let _ = app.update(Message::CatalogScrolled {
            offset: 0.0,
            height: 2.0 * catalog::ROW_HEIGHT,
            width: THREE_COLUMNS,
        });

        // Right twice: the third tile of the first row, Day of Defeat.
        let _ = app.update(Message::Shortcut(keys::Shortcut::Step(1)));
        let _ = app.update(Message::Shortcut(keys::Shortcut::Step(1)));
        let _ = app.update(Message::Shortcut(keys::Shortcut::Open));
        assert!(shows(
            &app,
            "rigcheck hasn't read this game's store page yet."
        ));
    }

    const KEY: &str = "0123456789ABCDEF0123456789ABCDEF";

    /// Steam's (saved) GetOwnedGames answer: Cyberpunk 2077, OMORI and
    /// Baldur's Gate 3, which isn't installed.
    fn steam_owns() -> Result<String, web::Failure> {
        Ok(std::fs::read_to_string("../data/fixtures/steam/owned_games.json").unwrap())
    }

    /// Saves a Steam key, then gets `answer` from Steam.
    fn steam_answers(app: &mut App, answer: Result<String, web::Failure>) {
        let _ = app.update(Message::Account(account::Message::Input(KEY.to_owned())));
        let _ = app.update(Message::Account(account::Message::Save));
        let _ = app.update(Message::Account(account::Message::Owned(answer)));
    }

    #[test]
    fn a_saved_steam_key_stays_private() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        click(&mut app, "SETTINGS");

        on_tall_window(&mut app, |ui| {
            ui.click(iced::widget::Id::new(account::KEY_INPUT)).unwrap();
            ui.typewrite(KEY);
        });
        on_tall_window(&mut app, |ui| {
            ui.click("SAVE KEY").unwrap();
        });

        let paths = Paths::under(dir.path());
        let file = paths.config.join("steam-api-key");
        assert_eq!(std::fs::read_to_string(&file).unwrap().trim(), KEY);
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "only the user may read it");
        let settings =
            std::fs::read_to_string(paths.config.join("settings.json")).unwrap_or_default();
        assert!(!settings.contains(KEY));
        assert!(!shows(&app, KEY));
        assert!(shows(&app, "A key is saved."));
    }

    #[test]
    fn all_my_games_lists_every_game_the_account_owns() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        steam_answers(&mut app, steam_owns());

        click(&mut app, "ALL MY GAMES");

        // Baldur's Gate 3 is owned, not installed: TOO WEAK on the iGPU like
        // Cyberpunk 2077 (RX 480 95 vs Vega 7 32); OMORI is READY.
        assert!(shows(&app, "Baldur's Gate 3"));
        assert!(shows(
            &app,
            "1 of 3 games you own meet the recommended specs."
        ));
        click(&mut app, "INSTALLED");
        assert!(!shows(&app, "Baldur's Gate 3"));
    }

    #[test]
    fn the_owned_games_are_remembered_for_offline_starts() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        steam_answers(&mut app, steam_owns());

        let (mut reopened, _) = App::new(Paths::under(dir.path()), Language::English);
        let _ = reopened.update(Message::HardwareDetected(test_support::amd_laptop()));
        click(&mut reopened, "ALL MY GAMES");

        assert!(shows(&reopened, "Baldur's Gate 3"));
    }

    #[test]
    fn a_private_profile_is_explained() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        steam_answers(&mut app, Ok(r#"{"response":{}}"#.to_owned()));

        click(&mut app, "ALL MY GAMES");

        assert!(shows(
            &app,
            "Steam isn't sharing this account's games. In Steam: your profile → Edit Profile → Privacy Settings → Game details: Public."
        ));
    }

    #[test]
    fn without_a_key_all_my_games_points_to_settings() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "ALL MY GAMES");

        assert!(shows(
            &app,
            "Add your Steam Web API key in Settings to see every game you own."
        ));
    }

    #[test]
    fn a_steam_id_pasted_as_the_key_is_refused_with_a_hint() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        click(&mut app, "SETTINGS");

        let steam_id = "76561198388197563".to_owned();
        let _ = app.update(Message::Account(account::Message::Input(steam_id)));
        let _ = app.update(Message::Account(account::Message::Save));

        assert!(
            !Paths::under(dir.path())
                .config
                .join("steam-api-key")
                .exists()
        );
        assert!(shows(
            &app,
            "That's a SteamID, not an API key. A Steam Web API key is 32 letters and digits, from GET A KEY. Your SteamID is found automatically."
        ));
    }

    #[test]
    fn a_rejected_key_is_named_as_the_problem() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        steam_answers(&mut app, Err(web::Failure::Rejected));

        click(&mut app, "ALL MY GAMES");

        assert!(shows(
            &app,
            "Steam didn't accept the key. Check it in Settings, or get a new one."
        ));
    }

    fn not_found() -> web::Failure {
        web::Failure::Failed("HTTP 404 Not Found".to_owned())
    }

    #[test]
    fn the_app_redraws_for_animations_only_while_covers_load_or_fade_in() {
        let dir = TempDir::new().unwrap();
        // Cyberpunk 2077 and OMORI have no art on disk: both download.
        let mut app = library_app(&dir);
        let loading = app.subscription().units();

        let _ = app.update(Message::CoverFetched(1091500, Err(not_found())));
        let _ = app.update(Message::CoverFetched(1150690, Err(not_found())));
        let idle = app.subscription().units();
        assert_eq!(idle, loading - 1, "no frames once nothing is loading");

        // A cover that arrives fades in, then everything is still again.
        let _ = app.update(Message::CoverFetched(1086940, Ok(b"jpeg".to_vec())));
        assert_eq!(app.subscription().units(), loading);
        std::thread::sleep(motion::FADE_IN + std::time::Duration::from_millis(50));
        assert_eq!(app.subscription().units(), idle);
    }

    #[test]
    fn window_has_the_brand_icon() {
        assert!(window_settings().icon.is_some());
    }
}
