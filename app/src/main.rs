//! The rigcheck desktop app: an iced UI on top of the workspace crates.

mod catalog;
mod download;
mod game_page;
mod i18n;
mod library;
mod lookup;
mod my_hardware;
mod overrides;
mod settings;
mod settings_page;
mod style;
#[cfg(test)]
mod test_support;

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
    search: String,
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
}

#[derive(Debug, Clone)]
enum Message {
    Open(Page),
    HardwareDetected(Hardware),
    GamesFound(Vec<InstalledGame>),
    Search(String),
    /// A hardware-table name, or `None` to go back to what was detected.
    OverrideGpu(Option<String>),
    OverrideCpu(Option<String>),
    CopyHardwareReport,
    AppearanceSelected(Appearance),
    /// `None` picks "System default".
    LanguageSelected(Option<Language>),
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
    /// Steam's `appdetails` answer for an app ID, or why there's none.
    SteamLookup(u32, Result<String, String>),
    RetryLookup(u32),
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
        let fetch = match db {
            Some(_) => Task::none(),
            None => download_db(&paths.data),
        };
        let settings = Settings::load(&paths.config);
        let fetched = open_fetched(&paths.cache);
        let mut app = App {
            override_choices: overrides::Choices::default(),
            i18n: I18n::new(settings.language.unwrap_or(system_language)),
            settings,
            system_language,
            paths,
            page: Page::Library,
            hardware: None,
            db: None,
            download: download::Status::Running { done: 0, total: 0 },
            catalog: Catalog::default(),
            games: None,
            records: HashMap::new(),
            covers: HashMap::new(),
            search: String::new(),
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
        (app, Task::batch([detect, scan, fetch]))
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

    fn theme(&self) -> Theme {
        self.settings.appearance.theme()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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
            Message::SteamLookup(app_id, answer) => {
                self.looked_up(app_id, answer);
                Task::none()
            }
            Message::RetryLookup(app_id) => {
                self.lookup = lookup::State::Checking(app_id);
                self.steam.ask(app_id)
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
                for game in &games {
                    if let Some(cover) = rigcheck_steam::cover_art(&self.paths.home, game.app_id) {
                        self.covers.insert(game.app_id, cover);
                    }
                }
                self.games = Some(games);
                self.load_records();
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
                self.download = download::Status::Running { done, total };
                Task::none()
            }
            Message::Download(download::Event::Finished(result)) => {
                let opened = result.and_then(|()| {
                    GameDb::open(&self.paths.data.join("games.db"))
                        .map_err(|error| download::Failure::Failed(error.to_string()))
                });
                match opened {
                    Ok(db) => self.use_db(db),
                    Err(failure) => {
                        eprintln!("couldn't download games.db: {failure:?}");
                        self.download = download::Status::Failed(failure);
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
                download_db(&self.paths.data)
            }
            Message::CopyHardwareReport => match &self.hardware {
                Some(hardware) => clipboard::write(hardware.report()),
                None => Task::none(),
            },
        }
    }

    /// Loads a game for its page: games.db, then a recent lookup in
    /// fetched.db, else asks Steam (ROADMAP §7.1).
    fn open_game(&mut self, app_id: u32) -> Task<Message> {
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
        let colors = self.settings.appearance.colors();
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
                library::view(entries, &self.search, i18n, colors)
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
                        gpu_in_use: rig.gpu,
                        cpu_in_use: rig.cpu,
                    };
                    // Room on the right and bottom so hard shadows aren't clipped.
                    scrollable(
                        column![
                            my_hardware::view(hardware, i18n, colors),
                            overrides::view(&self.override_choices, picked, i18n, colors),
                        ]
                        .spacing(18)
                        .padding([0, 6]),
                    )
                    .into()
                }
                None => center(text(i18n.t("detecting-hardware"))).into(),
            },
            Page::Settings => settings_page::view(&self.settings, i18n, colors),
        };
        let mut body = column![].spacing(18);
        if let Some(banner) = download::view(&self.download, i18n, colors) {
            body = body.push(banner);
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
        let logo = image(Handle::from_bytes(ICON_PNG))
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

/// Scrolls the Catalog list back to its first row.
fn catalog_to_top() -> Task<Message> {
    iced::widget::operation::snap_to(
        catalog::LIST,
        iced::widget::operation::RelativeOffset::START,
    )
}

/// Downloads games.db into `data`, reporting progress as messages.
fn download_db(data: &Path) -> Task<Message> {
    Task::run(download::games_db(data.join("games.db")), Message::Download)
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
        let _ = app.update(Message::Download(download::Event::Finished(Ok(()))));

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
    fn window_has_the_brand_icon() {
        assert!(window_settings().icon.is_some());
    }
}
