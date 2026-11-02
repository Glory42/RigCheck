//! "A newer rigcheck is out": the version of the latest GitHub Release,
//! compared with the running one (ROADMAP Phase 7). The app never replaces
//! itself; Flathub and AUR users update through their package manager.

use std::time::Duration;

use iced::futures::channel::oneshot;
use iced::widget::{button, container, row, space, text};
use iced::{Element, Fill, Task};
use rigcheck_net::{Allowlist, Client, RateLimiter, Retry};

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors};

/// Uploaded with every release (packaging/package.sh). `latest` skips
/// prereleases, so the daily `db-latest` games.db release never counts.
/// github.com redirects to its asset host, both in ROADMAP §6.
const VERSION_URL: &str =
    "https://github.com/Glory42/rigcheck/releases/latest/download/version.txt";
/// Where the notice links to.
pub const PAGE: &str = "https://github.com/Glory42/rigcheck/releases/latest";

/// Asks GitHub for the latest release's version; the answer comes back as
/// [`Message::LatestRelease`]. Nothing is sent until iced runs the task.
pub fn check() -> Task<Message> {
    Task::perform(
        async {
            let (sender, receiver) = oneshot::channel();
            std::thread::spawn(move || {
                let _ = sender.send(fetch());
            });
            receiver
                .await
                .unwrap_or_else(|_| Err("the check stopped".to_owned()))
        },
        Message::LatestRelease,
    )
}

fn fetch() -> Result<String, String> {
    let limiter = RateLimiter::new(5, Duration::from_secs(1));
    let mut client = Client::new(
        Allowlist::app(),
        limiter,
        Retry::new(1, Duration::from_secs(2)),
    )
    .map_err(|error| error.to_string())?;
    client
        .get_text(VERSION_URL)
        .map_err(|error| error.to_string())
}

/// The notice for a newer `version`, under the top bar.
pub fn notice(version: &str, i18n: &I18n, colors: Colors) -> Element<'static, Message> {
    container(
        row![
            text(i18n.t_with("release-out", &[("version", version)])).font(BOLD),
            space::horizontal(),
            button(text(i18n.t("release-get")).font(BOLD).size(13))
                .padding([7, 12])
                .style(colors.button(false))
                .on_press(Message::OpenUrl(PAGE.to_owned())),
        ]
        .align_y(iced::Center),
    )
    .padding(14)
    .width(Fill)
    .style(move |_| colors.boxed(colors.panel))
    .into()
}

/// Whether `latest` (e.g. "0.8.0") is a newer version than `running`.
/// Anything that isn't `major.minor.patch` counts as not newer.
pub fn is_newer(latest: &str, running: &str) -> bool {
    match (parse(latest), parse(running)) {
        (Some(latest), Some(running)) => latest > running,
        _ => false,
    }
}

/// "0.10.2" → (0, 10, 2). Tuples compare number by number.
fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let mut numbers = version.trim().split('.').map(|part| part.parse().ok());
    let version = (numbers.next()??, numbers.next()??, numbers.next()??);
    numbers.next().is_none().then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_number_by_number() {
        assert!(is_newer("0.8.0", "0.7.0"));
        assert!(is_newer("0.10.0", "0.9.3")); // not as text: "0.10" < "0.9"
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.7.0", "0.7.0"));
        assert!(!is_newer("0.6.9", "0.7.0"));
    }

    #[test]
    fn a_version_file_with_a_newline_or_junk_is_read_carefully() {
        assert!(is_newer("0.8.0\n", "0.7.0"));
        assert!(!is_newer("<html>Not Found</html>", "0.7.0"));
        assert!(!is_newer("", "0.7.0"));
    }
}
