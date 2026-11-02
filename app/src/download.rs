//! Downloading games.db when there isn't one yet (ROADMAP §7.3): read the
//! published manifest, stream `games.db.zst` to a temp file, then let
//! `rigcheck_gamedb::install` verify it and swap it in.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::futures::{SinkExt, Stream, executor::block_on};
use iced::widget::{button, column, container, progress_bar, row, space, text};
use iced::{Element, Fill};
use rigcheck_gamedb::{Manifest, install};
use rigcheck_net::{Allowlist, Client, Error, RateLimiter, Retry};

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors};

const MANIFEST_URL: &str =
    "https://github.com/Glory42/RigCheck/releases/download/db-latest/manifest.json";

#[derive(Debug, Clone)]
pub enum Event {
    /// Bytes of `games.db.zst` downloaded so far, out of `total`.
    Progress {
        done: u64,
        total: u64,
    },
    Finished(Result<(), Failure>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The published games.db uses a schema this build can't read.
    NeedsNewerApp,
    /// Offline, GitHub unreachable or a bad download. The text is for the log.
    Failed(String),
}

/// Where the download is, for the banner under the top bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// games.db is there; nothing to show.
    Done,
    Running {
        done: u64,
        total: u64,
    },
    Failed(Failure),
}

/// Downloads games.db to `dest`. The requests block, so they run on their
/// own thread and send their progress back through the stream.
pub fn games_db(dest: PathBuf) -> impl Stream<Item = Event> {
    iced::stream::channel(8, async move |events| {
        std::thread::spawn(move || {
            let mut events = events;
            let result = fetch(&dest, |done, total| {
                let _ = block_on(events.send(Event::Progress { done, total }));
            });
            let _ = block_on(events.send(Event::Finished(result)));
        });
    })
}

fn fetch(dest: &Path, mut progress: impl FnMut(u64, u64)) -> Result<(), Failure> {
    let failed = |error: Error| Failure::Failed(error.to_string());
    // Two requests in all, so pacing barely matters; retries cover a
    // GitHub hiccup.
    let limiter = RateLimiter::new(10, Duration::from_secs(1));
    let retry = Retry::new(2, Duration::from_secs(2));
    let mut client = Client::new(Allowlist::app(), limiter, retry).map_err(failed)?;
    let manifest =
        Manifest::from_json(&client.get_text(MANIFEST_URL).map_err(failed)?).map_err(failed)?;
    if manifest.needs_newer_app() {
        return Err(Failure::NeedsNewerApp);
    }
    let part = dest.with_extension("db.zst.part");
    let result = (|| {
        if let Some(dir) = dest.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut file = File::create(&part)?;
        client.download(&manifest.url, &mut file, |done| {
            progress(done, manifest.size_bytes)
        })?;
        install(&part, &manifest, dest)
    })();
    let _ = fs::remove_file(&part);
    result.map(drop).map_err(failed)
}

/// The banner shown while games.db is missing, or `None` once it's there.
pub fn view(status: &Status, i18n: &I18n, colors: Colors) -> Option<Element<'static, Message>> {
    let content = match *status {
        Status::Done => return None,
        Status::Running { done, total } => column![
            text(i18n.t("download-title")).font(BOLD),
            progress_bar(0.0..=total.max(1) as f32, done as f32).girth(10),
            text(i18n.t_with(
                "download-progress",
                &[("done", &megabytes(done)), ("total", &megabytes(total))],
            ))
            .size(13),
        ],
        Status::Failed(Failure::Failed(_)) => column![
            row![
                text(i18n.t("download-failed")).font(BOLD),
                space::horizontal(),
                button(text(i18n.t("download-retry")).font(BOLD).size(13))
                    .padding([7, 12])
                    .style(colors.button(false))
                    .on_press(Message::RetryDownload),
            ]
            .align_y(iced::Center)
        ],
        Status::Failed(Failure::NeedsNewerApp) => {
            column![text(i18n.t("download-needs-newer-app")).font(BOLD)]
        }
    };
    Some(
        container(content.spacing(8))
            .padding(14)
            .width(Fill)
            .style(move |_| colors.boxed(colors.panel))
            .into(),
    )
}

/// `bytes` in megabytes with one decimal, e.g. "6.4".
fn megabytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000.0)
}
