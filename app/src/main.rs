//! The rigcheck desktop app: an iced UI on top of the workspace crates.

mod about;
mod catalog;
mod compare;
mod covers;
mod download;
mod game_page;
mod i18n;
mod keys;
mod library;
mod lookup;
mod my_hardware;
mod overrides;
mod release;
#[cfg(test)]
mod screenshots;
mod settings;
mod settings_page;
mod style;
#[cfg(test)]
mod test_support;
mod whatif;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use i18n::{I18n, Language};
use iced::widget::image::{FilterMethod, Handle};
use iced::widget::{
    button, center, column, combo_box, container, image, row, scrollable, space, text,
};
use iced::{Element, Fill, Size, Task, Theme, clipboard, window};
use rigcheck_core::{Catalog, Specs};
use rigcheck_gamedb::ingest::{Ingested, ingest};
use rigcheck_gamedb::{Game, GameDb};
use rigcheck_hardware::Hardware;
use rigcheck_steam::InstalledGame;
use settings::{Appearance, Settings};
use style::{BOLD, Colors};

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
    /// `None` until games.db is downloaded; the Library then shows NO DATA.
    db: Option<GameDb>,
    download: download::Status,
    catalog: Catalog,
    /// `None` until Steam has been scanned.
    games: Option<Vec<InstalledGame>>,
    /// games.db records and cover art for the installed games.
    records: HashMap<u32, Game>,
    covers: HashMap<u32, PathBuf>,
    /// Where covers come from: Steam's cache, or downloads.
    art: covers::Covers,
    /// A newer rigcheck release's version, if one is out.
    newer_release: Option<String>,
    /// Sizes for Settings > Storage, refreshed when they can change.
    storage: settings_page::Storage,
    search: String,
    library_order: library::Order,
    catalog_list: catalog::List,
    /// Live lookups of games games.db hasn't checked; `None` if the cache
    /// directory can't be written.
    fetched: Option<GameDb>,
    steam: lookup::Steam,
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
    },
    RetryDownload,
    /// Opens a web page in the user's browser.
    OpenUrl(String),
    /// The latest release's version (`version.txt`), or why there's none.
    LatestRelease(Result<String, String>),
    /// Steam's `appdetails` answer for an app ID, or why there's none.
    SteamLookup(u32, Result<String, String>),
    RetryLookup(u32),
    /// A game page's COMPARE: remember this game (again: forget it).
    PickToCompare(u32),
    /// A cover downloaded from Steam's CDN, or why it couldn't be.
    CoverFetched(u32, Result<Vec<u8>, String>),
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
        let fetch = match &db {
            // A background update check (ROADMAP §7.3), unless turned off.
            Some(db) if !settings.manual_updates => {
                download_db(&paths.data, db.meta("db_version").ok().flatten())
            }
            Some(_) => Task::none(),
            None => download_db(&paths.data, None),
        };
        // Same setting: is there a newer rigcheck?
        let release_check = if settings.manual_updates {
            Task::none()
        } else {
            release::check()
        };
        let fetched = open_fetched(&paths.cache);
        let art = covers::Covers::new(paths.home.clone(), paths.cache.join("images"));
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
            db: None,
            download: download::Status::Running { done: 0, total: 0 },
            catalog: Catalog::default(),
            games: None,
            records: HashMap::new(),
            covers: HashMap::new(),
            art,
            storage: settings_page::Storage::default(),
            newer_release: None,
            search: String::new(),
            library_order: library::Order::default(),
            catalog_list: catalog::List::default(),
            fetched,
            steam: lookup::Steam::default(),
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
            Task::batch([detect, scan, fetch, release_check, system_mode]),
        )
    }

    /// Switches to `db`: its hardware table, and its records for the
    /// installed games found so far.
    fn use_db(&mut self, db: GameDb) {
        self.catalog = db.catalog().unwrap_or_default();
        let names = |entries: &[rigcheck_core::CatalogEntry]| {
            entries.iter().map(|entry| entry.name.clone()).collect()
        };
        self.override_choices = overrides::Choices {
            gpus: combo_box::State::new(names(self.catalog.gpus())),
            cpus: combo_box::State::new(names(self.catalog.cpus())),
        };
        self.whatif_choices = overrides::Choices {
            gpus: combo_box::State::new(names(self.catalog.gpus())),
            cpus: combo_box::State::new(names(self.catalog.cpus())),
        };
        self.catalog_list.refresh(&db);
        self.db = Some(db);
        self.download = download::Status::Done;
        self.load_records();
    }

    /// Looks up each installed game in games.db.
    fn load_records(&mut self) {
        let (Some(db), Some(games)) = (&self.db, &self.games) else {
            return;
        };
        self.records = games
            .iter()
            .filter_map(|game| Some((game.app_id, db.game(game.app_id).ok()??)))
            .collect();
    }

    /// What verdicts compare against: detected hardware scored with the
    /// hardware table. `None` until detection finishes.
    fn specs(&self) -> Option<Specs> {
        let mut specs = rigcheck_gamedb::specs(&self.catalog, self.hardware.as_ref()?);
        if let Some(gpu) = self.override_gpu() {
            specs.gpu = Some(gpu.to_ref());
            specs.vram_mb = gpu.vram_mb;
        }
        if let Some(cpu) = self.override_cpu() {
            specs.cpu = Some(cpu.to_ref());
        }
        Some(specs)
    }

    fn override_gpu(&self) -> Option<&rigcheck_core::CatalogEntry> {
        let name = self.settings.gpu_override.as_ref()?;
        self.catalog.gpus().iter().find(|entry| &entry.name == name)
    }

    fn override_cpu(&self) -> Option<&rigcheck_core::CatalogEntry> {
        let name = self.settings.cpu_override.as_ref()?;
        self.catalog.cpus().iter().find(|entry| &entry.name == name)
    }

    /// [`App::specs`] with the What if picks in place of your parts.
    fn what_if_specs(&self) -> Option<Specs> {
        let mut specs = self.specs()?;
        let find = |entries: &[rigcheck_core::CatalogEntry], name: &Option<String>| {
            let name = name.as_ref()?;
            entries.iter().find(|entry| &entry.name == name).cloned()
        };
        if let Some(gpu) = find(self.catalog.gpus(), &self.whatif.gpu) {
            specs.gpu = Some(gpu.to_ref());
            specs.vram_mb = gpu.vram_mb;
        }
        if let Some(cpu) = find(self.catalog.cpus(), &self.whatif.cpu) {
            specs.cpu = Some(cpu.to_ref());
        }
        if let Some(gb) = self.whatif.ram_gb {
            specs.ram_mb = Some(gb * 1024);
        }
        Some(specs)
    }

    /// "Cyberpunk 2077: TOO WEAK → LOW" for each installed game the What
    /// if picks would move to another stamp, by name.
    fn what_if_changes(&self) -> Vec<String> {
        let (now, then) = (self.specs(), self.what_if_specs());
        let mut games: Vec<&InstalledGame> = self.games.iter().flatten().collect();
        games.sort_by_key(|game| game.name.to_lowercase());
        games
            .into_iter()
            .filter_map(|game| {
                let record = self.records.get(&game.app_id);
                let (before, _) =
                    library::stamp(&library::status(record, now.as_ref()), &self.i18n);
                let (after, _) =
                    library::stamp(&library::status(record, then.as_ref()), &self.i18n);
                (before != after).then(|| format!("{}: {before} → {after}", game.name))
            })
            .collect()
    }

    /// Your PC as the game page shows it, named by the hardware table when possible.
    fn rig(&self) -> game_page::Rig {
        let unknown = || self.i18n.t("unknown");
        let Some(hardware) = &self.hardware else {
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
        let cpu = hardware.cpu.as_deref().map(|model| {
            self.catalog
                .match_cpus(model)
                .first()
                .map_or_else(|| model.to_owned(), |entry| entry.name.clone())
        });
        let gpu_name = gpu.and_then(|gpu| {
            self.catalog
                .gpu_by_pci(gpu.vendor_id, gpu.device_id)
                .map(|entry| entry.name.clone())
                .or_else(|| vulkan.map(|vulkan| vulkan.name.clone()))
        });
        let cpu = self.override_cpu().map(|entry| entry.name.clone()).or(cpu);
        let gpu_name = self
            .override_gpu()
            .map(|entry| entry.name.clone())
            .or(gpu_name);
        game_page::Rig {
            cpu: cpu.unwrap_or_else(unknown),
            gpu: gpu_name.unwrap_or_else(unknown),
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
        library::status(self.records.get(&app_id), self.specs().as_ref())
    }

    /// The appearance to draw with: the picked one, or the system's.
    fn appearance(&self) -> Appearance {
        self.settings.appearance.resolve(self.system_mode)
    }

    /// Follows the system switching between light and dark, and listens for
    /// shortcuts (keys no widget used).
    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::system::theme_changes().map(Message::SystemTheme),
            iced::keyboard::listen().filter_map(key_shortcut),
        ])
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
            (Shortcut::Move(by), Page::Catalog) => match &self.db {
                Some(db) => {
                    let offset = self.catalog_list.move_selection(by, db);
                    iced::widget::operation::scroll_to(
                        catalog::LIST,
                        iced::widget::operation::AbsoluteOffset {
                            x: None,
                            y: Some(offset),
                        },
                    )
                }
                None => Task::none(),
            },
            (Shortcut::Open, Page::Catalog) => {
                match self
                    .db
                    .as_ref()
                    .and_then(|db| self.catalog_list.selected_game(db))
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
                    Message::CheckForUpdates => self
                        .db
                        .as_ref()
                        .and_then(|db| db.meta("db_version").ok().flatten()),
                    _ => None,
                };
                self.download = match self.db {
                    Some(_) => download::Status::Updating { done: 0, total: 0 },
                    None => download::Status::Running { done: 0, total: 0 },
                };
                download_db(&self.paths.data, installed)
            }
            Message::ClearFetched => {
                // Closed first: the file is replaced by an empty one.
                self.fetched = None;
                let path = self.paths.cache.join("fetched.db");
                let _ = std::fs::remove_file(&path);
                self.fetched = open_fetched(&self.paths.cache);
                self.lookup = lookup::State::Idle;
                self.refresh_storage();
                Task::none()
            }
            Message::ClearImages => {
                self.art.clear();
                let images = self.paths.cache.join("images");
                self.covers.retain(|_, path| !path.starts_with(&images));
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
                Task::none()
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
                // A game without art is normal; only a failed save is worth a line.
                if let Ok(jpeg) = jpeg {
                    match self.art.save(app_id, &jpeg) {
                        Ok(path) => {
                            self.covers.insert(app_id, path);
                            self.art.evict(self.settings.image_cache_bytes());
                        }
                        Err(error) => eprintln!("couldn't keep the cover of {app_id}: {error}"),
                    }
                }
                Task::none()
            }
            Message::PickToCompare(app_id) => {
                self.compare_pick = (self.compare_pick != Some(app_id)).then_some(app_id);
                Task::none()
            }
            Message::RetryLookup(app_id) => {
                self.lookup = lookup::State::Checking(app_id);
                self.steam.ask(app_id)
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
                self.specs_changed();
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
                    .games
                    .iter()
                    .flatten()
                    .filter(|game| game.name.to_lowercase().contains(&query))
                    .min_by_key(|game| game.name.to_lowercase());
                match first {
                    Some(game) => self.update(Message::Open(Page::Game(game.app_id))),
                    None => Task::none(),
                }
            }
            Message::Shortcut(shortcut) => self.shortcut(shortcut),
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
                self.specs_changed();
                Task::none()
            }
            Message::OverrideCpu(name) => {
                self.settings.cpu_override = name;
                self.save_settings();
                self.specs_changed();
                Task::none()
            }
            Message::Download(download::Event::Progress { done, total }) => {
                self.download = match self.db {
                    Some(_) => download::Status::Updating { done, total },
                    None => download::Status::Running { done, total },
                };
                Task::none()
            }
            Message::Download(download::Event::Finished(result)) => {
                let opened = result.and_then(|fetched| match fetched {
                    download::Fetched::UpToDate => Ok(None),
                    download::Fetched::Installed => GameDb::open(&self.paths.data.join("games.db"))
                        .map(Some)
                        .map_err(|error| download::Failure::Failed(error.to_string())),
                });
                self.download = download::Status::Done;
                match opened {
                    Ok(Some(db)) => self.use_db(db),
                    Ok(None) => {}
                    Err(failure) => {
                        eprintln!("couldn't download games.db: {failure:?}");
                        // A failed update keeps the current games.db in use;
                        // only say so if there's none, or the app is too old.
                        if self.db.is_none() || failure == download::Failure::NeedsNewerApp {
                            self.download = download::Status::Failed(failure);
                        }
                    }
                }
                Task::none()
            }
            Message::CatalogSearch(query) => match &self.db {
                Some(db) => {
                    self.catalog_list.search(query, db);
                    catalog_to_top()
                }
                None => Task::none(),
            },
            Message::CatalogProtonDb(lowest) => match &self.db {
                Some(db) => {
                    self.catalog_list.protondb(lowest, db);
                    catalog_to_top()
                }
                None => Task::none(),
            },
            Message::CatalogFilter(chip) => match &self.db {
                Some(db) => {
                    self.catalog_list.toggle(chip, self.specs(), db);
                    catalog_to_top()
                }
                None => Task::none(),
            },
            Message::CatalogScrolled { offset, height } => {
                if let Some(db) = &self.db {
                    self.catalog_list.scrolled(offset, height, db);
                }
                Task::none()
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
            Message::RetryDownload => {
                self.download = download::Status::Running { done: 0, total: 0 };
                download_db(&self.paths.data, None)
            }
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

    /// Opens a game's page: its cover and its record.
    fn open_game(&mut self, app_id: u32) -> Task<Message> {
        let cover = self.cover(app_id);
        Task::batch([cover, self.look_up(app_id)])
    }

    /// Shows `app_id`'s cover if one is on disk, else downloads it.
    fn cover(&mut self, app_id: u32) -> Task<Message> {
        if self.covers.contains_key(&app_id) {
            return Task::none();
        }
        match self.art.path(app_id) {
            Some(path) => {
                self.covers.insert(app_id, path);
                Task::none()
            }
            None => self.art.fetch(app_id),
        }
    }

    /// The record for the game page: games.db, then a recent lookup in
    /// fetched.db, else asks Steam (ROADMAP §7.1).
    fn look_up(&mut self, app_id: u32) -> Task<Message> {
        self.opened = self.db.as_ref().and_then(|db| db.game(app_id).ok()?);
        if self.opened.as_ref().is_some_and(|game| game.crawled) {
            return Task::none();
        }
        let (Some(_), Some(fetched)) = (&self.db, &self.fetched) else {
            return Task::none();
        };
        if let Some(game) = fetched.lookup(app_id, now()).ok().flatten() {
            self.opened = Some(game);
            return Task::none();
        }
        self.lookup = lookup::State::Checking(app_id);
        self.steam.ask(app_id)
    }

    /// Keeps Steam's answer about `app_id` in fetched.db and shows it.
    fn looked_up(&mut self, app_id: u32, answer: Result<String, String>) {
        let (Some(db), Some(fetched)) = (&self.db, &self.fetched) else {
            return;
        };
        let stored = answer.and_then(|json| {
            match ingest(app_id, &json).map_err(|error| error.to_string())? {
                Ingested::Game(record) => fetched
                    .store_lookup(*record, db, now())
                    .map(|()| true)
                    .map_err(|error| error.to_string()),
                Ingested::NotAGame { .. } | Ingested::Unavailable => Ok(false),
            }
        });
        self.lookup = match stored {
            Ok(true) => lookup::State::Idle,
            Ok(false) => lookup::State::NoRequirements(app_id),
            Err(error) => {
                eprintln!("couldn't look up {app_id} on Steam: {error}");
                lookup::State::Failed(app_id)
            }
        };
        if self.page == Page::Game(app_id) && self.lookup == lookup::State::Idle {
            self.opened = fetched.lookup(app_id, now()).ok().flatten();
        }
    }

    /// A game's record by the lookup order of its page (ROADMAP §7.1):
    /// games.db if checked, else a recent live lookup, else games.db's name.
    fn record(&self, app_id: u32) -> Option<Game> {
        let from_db = self.db.as_ref().and_then(|db| db.game(app_id).ok()?);
        if from_db.as_ref().is_some_and(|game| game.crawled) {
            return from_db;
        }
        self.fetched
            .as_ref()
            .and_then(|fetched| fetched.lookup(app_id, now()).ok()?)
            .or(from_db)
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

    /// Lets the Catalog's CAN RUN use the current specs.
    fn specs_changed(&mut self) {
        let specs = self.specs();
        if let Some(db) = &self.db {
            self.catalog_list.use_specs(specs, db);
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
                let entries = self.games.as_ref().map(|games| {
                    games
                        .iter()
                        .map(|game| library::Entry {
                            game,
                            status: self.status(game.app_id),
                            cover: self.covers.get(&game.app_id),
                        })
                        .collect()
                });
                library::view(entries, &self.search, self.library_order, i18n, colors)
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
                        &library::status(self.opened.as_ref(), self.specs().as_ref()),
                        &self.rig(),
                        self.covers.get(&app_id),
                        self.opened_from,
                        self.compare_pick
                            .map(|other| (other, self.game_name(other))),
                        i18n,
                        colors,
                    ),
                    None => coming_soon(i18n.t("tab-library"), i18n, colors),
                }
            }
            Page::Catalog => catalog::view(&self.catalog_list, self.specs().as_ref(), i18n, colors),
            Page::MyRig => match &self.hardware {
                Some(hardware) => {
                    let rig = self.rig();
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
                        status: library::status(record.as_ref(), self.specs().as_ref()),
                        record,
                    }
                };
                compare::view(side(a), side(b), i18n, colors)
            }
            Page::Settings => settings_page::view(&self.settings, self.storage, i18n, colors),
        };
        let mut body = column![].spacing(18);
        if let Some(banner) = download::view(&self.download, i18n, colors) {
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

/// fetched.db in `dir`, created on first use.
fn open_fetched(dir: &Path) -> Option<GameDb> {
    let path = dir.join("fetched.db");
    let opened = if path.exists() {
        GameDb::open(&path)
    } else {
        std::fs::create_dir_all(dir)
            .map_err(Into::into)
            .and_then(|()| GameDb::create(&path))
    };
    opened
        .inspect_err(|error| eprintln!("couldn't open fetched.db: {error}"))
        .ok()
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

/// Downloads games.db into `data`, reporting progress as messages.
/// Downloads games.db into `data` unless `installed` is the published
/// build, reporting progress as messages.
fn download_db(data: &Path, installed: Option<String>) -> Task<Message> {
    Task::run(
        download::games_db(data.join("games.db"), installed),
        Message::Download,
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

    /// The seed with popularities (Cyberpunk 2077 most popular, then OMORI),
    /// plus a game the pipeline only knows the name of.
    fn catalog_app(dir: &TempDir) -> App {
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
        db.rebuild_search().unwrap();
        let (mut app, _) = App::new(paths, Language::English);
        let _ = app.update(Message::HardwareDetected(test_support::amd_laptop()));
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
        });
        assert!(shows(&app, "Cyberpunk 2077"));
        assert!(shows(&app, "TOO WEAK"));
        assert!(!shows(&app, "OMORI"));

        let _ = app.update(Message::CatalogScrolled {
            offset: row,
            height: row,
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
        let row = catalog::ROW_HEIGHT;
        let _ = app.update(Message::CatalogScrolled {
            offset: 2.0 * row,
            height: row,
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
        let paths = Paths::under(dir.path());
        std::fs::create_dir_all(&paths.data).unwrap();
        let db = GameDb::create(&paths.data.join("games.db")).unwrap();
        let table = include_str!("../../data/hardware.toml");
        db.store_catalog(&Catalog::from_toml(table).unwrap())
            .unwrap();
        db.store_names(&[
            (BG3, "Baldur's Gate 3"),
            (2138330, "Cyberpunk 2077: Phantom Liberty"),
        ])
        .unwrap();
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
    fn a_failed_lookup_says_so_and_can_be_retried() {
        let dir = TempDir::new().unwrap();
        let mut app = lookup_app(&dir);
        let _ = app.update(Message::Open(Page::Game(BG3)));

        let _ = app.update(Message::SteamLookup(BG3, Err("offline".to_owned())));
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
        click(&mut app, "CLEAR FETCHED GAMES");
        let _ = app.update(Message::Open(Page::Game(BG3)));

        assert!(shows(&app, "Checking Steam…"));
    }

    #[test]
    fn clearing_images_frees_their_space() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);
        let _ = app.update(Message::CoverFetched(1150690, Ok(vec![0; 1_500_000])));

        click(&mut app, "SETTINGS");
        assert!(shows(&app, "1.5 MB"));
        click(&mut app, "CLEAR IMAGES");

        assert!(shows(&app, "0.0 MB"));
        let images = Paths::under(dir.path()).cache.join("images");
        assert_eq!(
            covers::Covers::new(dir.path().join("home"), images).size(),
            0
        );
    }

    #[test]
    fn the_image_cache_limit_is_saved() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "SETTINGS");
        click(&mut app, "250 MB");

        let saved = Settings::load(&Paths::under(dir.path()).config);
        assert_eq!(saved.image_cache_mb, Some(250));
    }

    #[test]
    fn re_downloading_the_database_starts_right_away() {
        let dir = TempDir::new().unwrap();
        let mut app = library_app(&dir);

        click(&mut app, "SETTINGS");
        click(&mut app, "RE-DOWNLOAD");

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
            Err("offline".to_owned()),
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

    #[test]
    fn window_has_the_brand_icon() {
        assert!(window_settings().icon.is_some());
    }
}
