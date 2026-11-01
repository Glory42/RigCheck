//! The rigcheck desktop app: an iced UI on top of the workspace crates.

mod my_hardware;

use std::path::Path;

use iced::widget::{center, text};
use iced::{Element, Size, Task, clipboard, window};
use rigcheck_hardware::Hardware;

/// Reverse-DNS app ID. Wayland uses it to match the window with its .desktop file
/// (and so its icon); Flatpak uses it as the app ID.
const APP_ID: &str = "dev.gorkemkaryol.rigcheck";
const ICON_PNG: &[u8] = include_bytes!("../../assets/brand/icon-256.png");

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title("rigcheck")
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
    /// `None` until detection finishes.
    hardware: Option<Hardware>,
}

#[derive(Debug, Clone)]
enum Message {
    HardwareDetected(Hardware),
    CopyHardwareReport,
}

impl App {
    /// Starting state plus work to run right away. Detection reads files and
    /// runs `vulkaninfo`, so it runs as a `Task` on a background thread
    /// instead of freezing the window.
    fn boot() -> (Self, Task<Message>) {
        let detect = Task::perform(
            async { rigcheck_hardware::detect(Path::new("/"), &rigcheck_hardware::System) },
            Message::HardwareDetected,
        );
        (App { hardware: None }, detect)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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
        match &self.hardware {
            Some(hardware) => my_hardware::view(hardware),
            None => center(text("Detecting your hardware…")).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_it_is_detecting_hardware_at_startup() {
        let (app, _) = App::boot();
        let mut ui = iced_test::simulator(app.view());
        assert!(ui.find("Detecting your hardware…").is_ok());
    }

    #[test]
    fn window_has_the_brand_icon() {
        assert!(window_settings().icon.is_some());
    }
}
