//! Downloading games.db when there isn't one yet (ROADMAP §7.3): read the
//! published manifest, stream `games.db.zst` to a temp file, then let
//! `rigcheck_gamedb::install` verify it and swap it in.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use iced::futures::{SinkExt, Stream, executor::block_on};
use iced::widget::{button, column, container, progress_bar, row, space, text};
use iced::{Element, Fill};
use rigcheck_gamedb::{Manifest, install};

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors};
use crate::web::Web;

#[derive(Debug, Clone)]
pub enum Event {
    /// Bytes of `games.db.zst` downloaded so far, out of `total`.
    Progress {
        done: u64,
        total: u64,
    },
    Finished(Result<Fetched, Failure>),
}

/// How a download ended well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fetched {
    /// A new games.db is in place.
    Installed,
    /// The published games.db is the one already installed.
    UpToDate,
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
    /// The first download: there's no games.db yet.
    Running {
        done: u64,
        total: u64,
    },
    /// A newer games.db is downloading; the current one stays in use.
    Updating {
        done: u64,
        total: u64,
    },
    Failed(Failure),
}

/// Downloads games.db to `dest`, unless `installed` (its `db_version`) is
/// already the published one. The requests block, so they run on their own
/// thread and send their progress back through the stream.
pub fn games_db(web: Web, dest: PathBuf, installed: Option<String>) -> impl Stream<Item = Event> {
    iced::stream::channel(8, async move |events| {
        std::thread::spawn(move || {
            let mut events = events;
            let result = fetch(&web, &dest, installed.as_deref(), |done, total| {
                let _ = block_on(events.send(Event::Progress { done, total }));
            });
            let _ = block_on(events.send(Event::Finished(result)));
        });
    })
}

fn fetch(
    web: &Web,
    dest: &Path,
    installed: Option<&str>,
    mut progress: impl FnMut(u64, u64),
) -> Result<Fetched, Failure> {
    let failed = |error: rigcheck_gamedb::Error| Failure::Failed(error.to_string());
    let manifest = web
        .manifest()
        .map_err(|failure| Failure::Failed(failure.to_string()))?;
    let manifest = Manifest::from_json(&manifest).map_err(failed)?;
    if manifest.needs_newer_app() {
        return Err(Failure::NeedsNewerApp);
    }
    if !manifest.is_update_for(installed) {
        return Ok(Fetched::UpToDate);
    }
    let part = dest.with_extension("db.zst.part");
    let result = (|| {
        if let Some(dir) = dest.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut file = File::create(&part)?;
        web.download(&manifest.url, &mut file, &mut |done| {
            progress(done, manifest.size_bytes)
        })
        .map_err(|failure| failure.to_string())?;
        install(&part, &manifest, dest)
    })();
    let _ = fs::remove_file(&part);
    result.map(|_| Fetched::Installed).map_err(failed)
}

/// The banner shown while games.db is missing, or `None` once it's there.
pub fn view(status: &Status, i18n: &I18n, colors: Colors) -> Option<Element<'static, Message>> {
    let content = match *status {
        Status::Done => return None,
        Status::Running { done, total } | Status::Updating { done, total } => column![
            text(i18n.t(if matches!(status, Status::Running { .. }) {
                "download-title"
            } else {
                "download-updating"
            }))
            .font(BOLD),
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
