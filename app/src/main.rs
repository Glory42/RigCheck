//! The rigcheck desktop app: an iced UI on top of the workspace crates.

mod my_hardware;
mod settings;
mod settings_page;
mod style;

use std::path::{Path, PathBuf};

use iced::widget::image::{FilterMethod, Handle};
use iced::widget::{button, center, column, container, image, row, space, text};
use iced::{Element, Fill, Size, Task, Theme, clipboard, window};
use rigcheck_hardware::Hardware;
use settings::{Appearance, Settings};
use style::{BOLD, Colors};

/// Reverse-DNS app ID. Wayland uses it to match the window with its .desktop file
/// (and so its icon); Flatpak uses it as the app ID.
const APP_ID: &str = "dev.gorkemkaryol.rigcheck";
const ICON_PNG: &[u8] = include_bytes!("../../assets/brand/icon-256.png");

fn main() -> iced::Result {
    // ~/.config/rigcheck, or wherever $XDG_CONFIG_HOME points.
    let config_dir = directories::ProjectDirs::from("dev", "gorkemkaryol", "rigcheck")
        .expect("a home directory")
        .config_dir()
        .to_owned();
    iced::application(move || App::new(config_dir.clone()), App::update, App::view)
        .title("rigcheck")
        .theme(App::theme)
        .window(window_settings())
        .run()
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
    /// Where `settings.json` lives.
    config_dir: PathBuf,
    settings: Settings,
    page: Page,
    /// `None` until detection finishes.
    hardware: Option<Hardware>,
}

/// The tabs in the top bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Library,
    Catalog,
    MyRig,
    Settings,
}

#[derive(Debug, Clone)]
enum Message {
    Open(Page),
    HardwareDetected(Hardware),
    CopyHardwareReport,
    AppearanceSelected(Appearance),
}

impl App {
    /// Starting state plus work to run right away. Detection reads files and
    /// runs `vulkaninfo`, so it runs as a `Task` on a background thread
    /// instead of freezing the window.
    fn new(config_dir: PathBuf) -> (Self, Task<Message>) {
        let detect = Task::perform(
            async { rigcheck_hardware::detect(Path::new("/"), &rigcheck_hardware::System) },
            Message::HardwareDetected,
        );
        let app = App {
            settings: Settings::load(&config_dir),
            config_dir,
            // Library and Catalog arrive in later phases; My Rig works today.
            page: Page::MyRig,
            hardware: None,
        };
        (app, detect)
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
                if let Err(error) = self.settings.save(&self.config_dir) {
                    eprintln!("couldn't save settings: {error}");
                }
                Task::none()
            }
            Message::HardwareDetected(hardware) => {
                self.hardware = Some(hardware);
                Task::none()
            }
            Message::CopyHardwareReport => match &self.hardware {
                Some(hardware) => clipboard::write(hardware.report()),
                None => Task::none(),
            },
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let colors = self.settings.appearance.colors();
        let page = match self.page {
            Page::Library => coming_soon("LIBRARY", colors),
            Page::Catalog => coming_soon("CATALOG", colors),
            Page::MyRig => match &self.hardware {
                Some(hardware) => my_hardware::view(hardware, colors),
                None => center(text("Detecting your hardware…")).into(),
            },
            Page::Settings => settings_page::view(self.settings.appearance, colors),
        };
        column![
            self.top_bar(colors),
            container(page).padding(18).width(Fill).height(Fill),
        ]
        .into()
    }

    /// Design D's top bar: pixel logo on the left, tabs on the right.
    fn top_bar(&self, colors: Colors) -> Element<'_, Message> {
        let tab = |label: &'static str, page: Page| {
            button(text(label).font(BOLD).size(13))
                .padding([7, 12])
                .style(colors.button(self.page == page))
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
            tab("LIBRARY", Page::Library),
            tab("CATALOG", Page::Catalog),
            tab("MY RIG", Page::MyRig),
            tab("SETTINGS", Page::Settings),
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

/// Placeholder for pages that later phases build.
fn coming_soon(title: &'static str, colors: Colors) -> Element<'static, Message> {
    center(
        container(column![text(title).font(BOLD).size(20), text("Coming soon.")].spacing(6))
            .padding(18)
            .style(move |_| colors.boxed(colors.panel)),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use settings::Appearance;
    use tempfile::TempDir;

    #[test]
    fn a_fresh_install_opens_in_paper() {
        let config = TempDir::new().unwrap();

        let (app, _) = App::new(config.path().to_owned());

        assert_eq!(app.theme().to_string(), "Paper");
    }

    #[test]
    fn picking_an_appearance_applies_and_saves_it() {
        let config = TempDir::new().unwrap();
        let (mut app, _) = App::new(config.path().to_owned());

        click(&mut app, "SETTINGS");
        click(&mut app, "INK");

        assert_eq!(app.theme().to_string(), "Ink");
        assert_eq!(Settings::load(config.path()).appearance, Appearance::Ink);
    }

    #[test]
    fn the_app_reopens_in_the_saved_appearance() {
        let config = TempDir::new().unwrap();
        let (mut app, _) = App::new(config.path().to_owned());
        click(&mut app, "SETTINGS");
        click(&mut app, "INK");

        let (reopened, _) = App::new(config.path().to_owned());

        assert_eq!(reopened.theme().to_string(), "Ink");
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
        let (app, _) = App::new(config.path().to_owned());
        let mut ui = iced_test::simulator(app.view());
        assert!(ui.find("Detecting your hardware…").is_ok());
    }

    #[test]
    fn window_has_the_brand_icon() {
        assert!(window_settings().icon.is_some());
    }
}
