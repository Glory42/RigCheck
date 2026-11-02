//! The rigcheck desktop app: an iced UI on top of the workspace crates.

mod download;
mod game_page;
mod i18n;
mod library;
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
            home: base.home_dir().to_owned(),
        }
    }

    #[cfg(test)]
    fn under(dir: &Path) -> Paths {
        Paths {
            config: dir.join("config"),
            data: dir.join("data"),
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
    override_choices: overrides::Choices,
}

/// The tabs in the top bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Library,
    /// An installed game, by app ID.
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
    RetryDownload,
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
        match (self.records.get(&app_id), self.specs()) {
            (None, _) => library::Status::NoData,
            (Some(_), None) => library::Status::Checking,
            (Some(game), Some(specs)) => library::Status::Verdict(game.verdict(&specs)),
        }
    }

    fn theme(&self) -> Theme {
        self.settings.appearance.theme()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Open(page) => {
                self.page = page;
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
                Task::none()
            }
            Message::OverrideCpu(name) => {
                self.settings.cpu_override = name;
                self.save_settings();
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
            Page::Game(app_id) => match self
                .games
                .iter()
                .flatten()
                .find(|game| game.app_id == app_id)
            {
                Some(game) => game_page::view(
                    game,
                    self.records.get(&app_id),
                    &self.status(app_id),
                    &self.rig(),
                    self.covers.get(&app_id),
                    i18n,
                    colors,
                ),
                None => coming_soon(i18n.t("tab-library"), i18n, colors),
            },
            Page::Catalog => coming_soon(i18n.t("tab-catalog"), i18n, colors),
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
                        || (page == Page::Library && matches!(self.page, Page::Game(_))),
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

    #[test]
    fn window_has_the_brand_icon() {
        assert!(window_settings().icon.is_some());
    }
}
