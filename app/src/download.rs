//! Downloading games.db when there isn't one yet (ROADMAP §7.3): read the
//! published manifest, stream `games.db.zst` to a temp file, then let
//! `rigcheck_gamedb::install` verify it and swap it in.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use iced::futures::{SinkExt, Stream, executor::block_on};
use iced::widget::{button, column, container, progress_bar, row, space, text};
use iced::{Element, Fill, Task};
use rigcheck_gamedb::{GameDb, Manifest, install};

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

/// games.db's downloads, one at a time, and where they are for the banner.
pub struct Downloader {
    web: Web,
    /// Where games.db goes.
    dest: PathBuf,
    status: Status,
    /// Whether a download is on its way, shown or not: two at once would
    /// write the same temp file.
    running: bool,
}

impl Downloader {
    pub fn new(web: Web, dest: PathBuf) -> Downloader {
        Downloader {
            web,
            dest,
            status: Status::Done,
            running: false,
        }
    }

    /// Downloads games.db unless `installed` (its `db_version`) is the
    /// published build, showing the banner right away. `has_db`: whether
    /// there's a games.db to keep using meanwhile. Does nothing while a
    /// download runs.
    pub fn start(&mut self, installed: Option<String>, has_db: bool) -> Task<Message> {
        if self.running {
            return Task::none();
        }
        self.status = progress(0, 0, has_db);
        self.run(installed)
    }

    /// [`Downloader::start`] without the banner until a newer games.db
    /// downloads: the update check at start-up.
    pub fn check(&mut self, installed: Option<String>) -> Task<Message> {
        if self.running {
            return Task::none();
        }
        self.run(installed)
    }

    fn run(&mut self, installed: Option<String>) -> Task<Message> {
        self.running = true;
        Task::run(
            games_db(self.web.clone(), self.dest.clone(), installed),
            Message::Download,
        )
    }

    /// Follows the download. Returns the games.db it installed, opened.
    /// A failed update leaves the current games.db in use, and only says
    /// so if there's none (`has_db`) or the app is too old for the new one.
    pub fn event(&mut self, event: Event, has_db: bool) -> Option<GameDb> {
        let result = match event {
            Event::Progress { done, total } => {
                self.status = progress(done, total, has_db);
                return None;
            }
            Event::Finished(result) => result,
        };
        self.running = false;
        self.status = Status::Done;
        let opened = result.and_then(|fetched| match fetched {
            Fetched::UpToDate => Ok(None),
            Fetched::Installed => GameDb::open(&self.dest)
                .map(Some)
                .map_err(|error| Failure::Failed(error.to_string())),
        });
        match opened {
            Ok(db) => db,
            Err(failure) => {
                eprintln!("couldn't download games.db: {failure:?}");
                if !has_db || failure == Failure::NeedsNewerApp {
                    self.status = Status::Failed(failure);
                }
                None
            }
        }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Whether a download is on its way, so Settings can't start another.
    pub fn running(&self) -> bool {
        self.running
    }
}

/// The banner's progress: the first download, or an update while
/// games.db stays in use.
fn progress(done: u64, total: u64, has_db: bool) -> Status {
    if has_db {
        Status::Updating { done, total }
    } else {
        Status::Running { done, total }
    }
}

/// Downloads games.db to `dest`, unless `installed` (its `db_version`) is
/// already the published one. The requests block, so they run on their own
/// thread and send their progress back through the stream.
fn games_db(web: Web, dest: PathBuf, installed: Option<String>) -> impl Stream<Item = Event> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn downloader(dir: &TempDir) -> Downloader {
        let web = Web::with(crate::web::fake::Fake::replying(Ok("{}")));
        Downloader::new(web, dir.path().join("games.db"))
    }

    fn finished(result: Result<Fetched, Failure>) -> Event {
        Event::Finished(result)
    }

    #[test]
    fn one_download_runs_at_a_time() {
        let dir = TempDir::new().unwrap();
        let mut downloader = downloader(&dir);

        assert_eq!(downloader.check(None).units(), 1);
        assert_eq!(downloader.start(None, true).units(), 0);
        assert_eq!(downloader.check(None).units(), 0);
        assert!(downloader.running());

        downloader.event(finished(Ok(Fetched::UpToDate)), true);
        assert!(!downloader.running());
        assert_eq!(downloader.start(None, true).units(), 1);
    }

    #[test]
    fn the_start_up_check_shows_nothing_until_an_update_downloads() {
        let dir = TempDir::new().unwrap();
        let mut downloader = downloader(&dir);

        let _ = downloader.check(Some("2026-11-01.1".to_owned()));
        assert_eq!(*downloader.status(), Status::Done);

        downloader.event(Event::Progress { done: 1, total: 2 }, true);
        assert_eq!(*downloader.status(), Status::Updating { done: 1, total: 2 });
    }

    #[test]
    fn a_failed_update_is_quiet_unless_the_app_is_too_old() {
        let dir = TempDir::new().unwrap();
        let mut downloader = downloader(&dir);
        let offline = || Failure::Failed("offline".to_owned());

        let _ = downloader.start(None, true);
        assert!(downloader.event(finished(Err(offline())), true).is_none());
        assert_eq!(*downloader.status(), Status::Done);

        let _ = downloader.start(None, true);
        downloader.event(finished(Err(Failure::NeedsNewerApp)), true);
        assert_eq!(*downloader.status(), Status::Failed(Failure::NeedsNewerApp));

        let _ = downloader.start(None, false);
        downloader.event(finished(Err(offline())), false);
        assert_eq!(*downloader.status(), Status::Failed(offline()));
    }

    #[test]
    fn an_installed_download_comes_back_opened() {
        let dir = TempDir::new().unwrap();
        let mut downloader = downloader(&dir);
        let _ = downloader.start(None, false);
        // What a finished download leaves behind.
        GameDb::create_seed(&dir.path().join("games.db")).unwrap();

        let db = downloader.event(finished(Ok(Fetched::Installed)), false);

        assert!(db.is_some());
        assert_eq!(*downloader.status(), Status::Done);
    }
}
