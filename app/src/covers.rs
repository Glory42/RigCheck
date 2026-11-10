//! Cover art: Steam's own cache first, then art rigcheck downloaded from
//! Steam's image CDN into `~/.cache/rigcheck/images/` (ROADMAP Phase 5).
//! [`Covers`] knows which tiles have a cover, which are waiting for one, and
//! which just got one, so a tile never points at a file that's gone.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use iced::Task;

use crate::Message;
use crate::library::Cover;
use crate::motion;
use crate::web::{Failure, Web, background};

pub struct Covers {
    /// Where Steam installs are looked for.
    home: PathBuf,
    /// `~/.cache/rigcheck/images`
    dir: PathBuf,
    /// Asked for in this session, found or not, while their file is there.
    asked: HashSet<u32>,
    /// Asked for and not answered yet: their tiles pulse. Once answered
    /// without art, a tile gets a still placeholder (and `asked` keeps it
    /// from being asked again).
    loading: HashSet<u32>,
    /// The covers on disk that tiles draw, by game.
    shown: HashMap<u32, PathBuf>,
    /// When each downloaded cover arrived, for its fade-in.
    arrived: HashMap<u32, Instant>,
    /// The pulse's phase.
    started: Instant,
    web: Web,
}

impl Covers {
    pub fn new(home: PathBuf, dir: PathBuf, web: Web) -> Covers {
        Covers {
            home,
            dir,
            asked: HashSet::new(),
            loading: HashSet::new(),
            shown: HashMap::new(),
            arrived: HashMap::new(),
            started: Instant::now(),
            web,
        }
    }

    /// Shows `app_id`'s cover if one is on disk, else downloads it, unless
    /// it was already asked for in this session (a game without art isn't
    /// asked again). The answer comes back as [`Message::CoverFetched`];
    /// nothing is sent until iced runs the task.
    pub fn want(&mut self, app_id: u32) -> Task<Message> {
        if self.shown.contains_key(&app_id) {
            return Task::none();
        }
        if let Some(path) = self.path(app_id) {
            self.shown.insert(app_id, path);
            return Task::none();
        }
        if !self.asked.insert(app_id) {
            return Task::none();
        }
        self.loading.insert(app_id);
        let web = self.web.clone();
        background(move || web.cover(app_id)).map(move |jpeg| Message::CoverFetched(app_id, jpeg))
    }

    /// The download for `app_id` ended, with a cover or without. A cover is
    /// kept and fades in; then the least recently used covers go until
    /// they take at most `limit` bytes.
    pub fn fetched(&mut self, app_id: u32, jpeg: Result<Vec<u8>, Failure>, limit: u64) {
        self.loading.remove(&app_id);
        // A game without art is normal; only a failed save is worth a line.
        let Ok(jpeg) = jpeg else {
            return;
        };
        match self.save(app_id, &jpeg) {
            Ok(path) => {
                self.shown.insert(app_id, path);
                self.arrived.insert(app_id, Instant::now());
                self.evict(limit);
            }
            Err(error) => eprintln!("couldn't keep the cover of {app_id}: {error}"),
        }
    }

    /// `app_id`'s cover as drawn at `now`.
    pub fn cover_of(&self, app_id: u32, now: Instant) -> Cover<'_> {
        if let Some(path) = self.shown.get(&app_id) {
            let opacity = self.arrived.get(&app_id).map_or(1.0, |at| {
                motion::fade_in(now.saturating_duration_since(*at))
            });
            Cover::Ready { path, opacity }
        } else if self.loading.contains(&app_id) {
            let pulse = motion::pulse(now.saturating_duration_since(self.started));
            Cover::Loading { pulse }
        } else {
            Cover::Missing
        }
    }

    /// Whether a cover is loading (pulsing) or has just arrived (fading
    /// in) at `now`, so tiles need frames.
    pub fn animating(&self, now: Instant) -> bool {
        !self.loading.is_empty()
            || self
                .arrived
                .values()
                .any(|at| now.saturating_duration_since(*at) < motion::FADE_IN)
    }

    /// Bytes the downloaded covers take.
    pub fn size(&self) -> u64 {
        self.on_disk().iter().map(|(_, bytes, _)| bytes).sum()
    }

    /// Deletes the least recently used downloaded covers until they take at
    /// most `limit` bytes (ROADMAP §7.1). Steam's own cache isn't touched.
    /// A tile whose cover went asks for it again.
    pub fn evict(&mut self, limit: u64) {
        let mut covers = self.on_disk();
        covers.sort_by_key(|(_, _, used)| *used);
        let mut size: u64 = covers.iter().map(|(_, bytes, _)| bytes).sum();
        for (path, bytes, _) in covers {
            if size <= limit {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                size -= bytes;
                self.forget(&path);
            }
        }
    }

    /// Deletes every downloaded cover.
    pub fn clear(&mut self) {
        self.evict(0);
    }

    /// Downloads and keeps `app_id`'s cover now, blocking: for the README
    /// screenshots, which don't run iced's tasks.
    #[cfg(test)]
    pub fn download_now(&mut self, app_id: u32) {
        let jpeg = self.web.cover(app_id);
        self.fetched(app_id, jpeg, u64::MAX);
    }

    /// The cover for `app_id`, if Steam or rigcheck has one on disk.
    /// Finding a downloaded cover marks it used, for [`Covers::evict`].
    fn path(&self, app_id: u32) -> Option<PathBuf> {
        rigcheck_steam::cover_art(&self.home, app_id).or_else(|| {
            let downloaded = self.file(app_id);
            let file = std::fs::File::options()
                .write(true)
                .open(&downloaded)
                .ok()?;
            let _ = file.set_modified(SystemTime::now());
            Some(downloaded)
        })
    }

    /// Stops drawing the deleted cover at `path`, and lets its game ask
    /// for it again.
    fn forget(&mut self, path: &PathBuf) {
        let gone: Vec<u32> = self
            .shown
            .iter()
            .filter(|(_, shown)| *shown == path)
            .map(|(&app_id, _)| app_id)
            .collect();
        for app_id in gone {
            self.shown.remove(&app_id);
            self.arrived.remove(&app_id);
            self.asked.remove(&app_id);
        }
    }

    /// Each downloaded cover: its path, size and when it was last used.
    fn on_disk(&self) -> Vec<(PathBuf, u64, SystemTime)> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jpg"))
            .filter_map(|entry| {
                let meta = entry.metadata().ok()?;
                Some((entry.path(), meta.len(), meta.modified().ok()?))
            })
            .collect()
    }

    /// Keeps a downloaded cover. Written to a temp file and renamed, so a
    /// half-written image is never shown.
    fn save(&self, app_id: u32, jpeg: &[u8]) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let file = self.file(app_id);
        let part = file.with_extension("jpg.part");
        std::fs::write(&part, jpeg)?;
        std::fs::rename(&part, &file)?;
        Ok(file)
    }

    fn file(&self, app_id: u32) -> PathBuf {
        self.dir.join(format!("{app_id}.jpg"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const BG3: u32 = 1086940;

    fn covers(dir: &TempDir) -> Covers {
        let web = crate::web::Web::with(crate::web::fake::Fake::replying(Ok("jpeg")));
        Covers::new(
            dir.path().join("home"),
            dir.path().join("cache/images"),
            web,
        )
    }

    #[test]
    fn a_downloaded_cover_is_found_again_after_a_restart() {
        let dir = TempDir::new().unwrap();
        assert_eq!(covers(&dir).path(BG3), None);

        covers(&dir).save(BG3, b"jpeg").unwrap();

        let found = covers(&dir).path(BG3).expect("saved");
        assert_eq!(std::fs::read(found).unwrap(), b"jpeg");
    }

    #[test]
    fn steams_own_cover_comes_first() {
        let dir = TempDir::new().unwrap();
        let steams = dir
            .path()
            .join("home/.local/share/Steam/appcache/librarycache/1086940/library_600x900.jpg");
        std::fs::create_dir_all(steams.parent().unwrap()).unwrap();
        std::fs::write(&steams, b"steam's").unwrap();
        covers(&dir).save(BG3, b"ours").unwrap();

        assert_eq!(covers(&dir).path(BG3), Some(steams));
    }

    #[test]
    fn each_cover_is_asked_for_once() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);

        assert_eq!(covers.want(BG3).units(), 1);
        assert_eq!(covers.want(BG3).units(), 0);

        covers.save(620, b"jpeg").unwrap();
        assert_eq!(covers.want(620).units(), 0, "already on disk");
        assert!(matches!(
            covers.cover_of(620, Instant::now()),
            Cover::Ready { .. }
        ));
    }

    fn not_found() -> Failure {
        Failure::Failed("HTTP 404 Not Found".to_owned())
    }

    /// Saves a 10-byte cover for each app, oldest use first: `n` seconds ago.
    fn used_at(covers: &Covers, uses: &[(u32, u64)]) {
        let now = std::time::SystemTime::now();
        for &(app_id, seconds_ago) in uses {
            let path = covers.save(app_id, b"0123456789").unwrap();
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(now - std::time::Duration::from_secs(seconds_ago))
                .unwrap();
        }
    }

    #[test]
    fn eviction_removes_the_least_recently_used_covers_first() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        used_at(&covers, &[(1, 300), (2, 100), (3, 200)]);

        covers.evict(20);

        assert_eq!(covers.size(), 20);
        assert!(covers.path(1).is_none(), "used longest ago");
        assert!(covers.path(2).is_some() && covers.path(3).is_some());
    }

    #[test]
    fn showing_a_cover_counts_as_using_it() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        used_at(&covers, &[(1, 300), (2, 100)]);

        assert!(covers.path(1).is_some()); // shown just now
        covers.evict(10);

        assert!(covers.path(1).is_some());
        assert!(covers.path(2).is_none());
    }

    #[test]
    fn clearing_removes_every_downloaded_cover() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        used_at(&covers, &[(1, 0), (2, 0)]);

        covers.clear();

        assert_eq!(covers.size(), 0);
        assert!(covers.path(1).is_none());
    }

    #[test]
    fn a_cover_pulses_from_its_request_then_fades_in() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        let now = Instant::now();
        assert!(matches!(covers.cover_of(BG3, now), Cover::Missing));
        assert!(!covers.animating(now));

        let _ = covers.want(BG3);
        assert!(matches!(covers.cover_of(BG3, now), Cover::Loading { .. }));
        assert!(covers.animating(now));

        covers.fetched(BG3, Ok(b"jpeg".to_vec()), u64::MAX);
        let now = Instant::now();
        let Cover::Ready { opacity, .. } = covers.cover_of(BG3, now) else {
            panic!("arrived");
        };
        assert!(opacity < 1.0, "fading in");
        assert!(covers.animating(now));
        assert!(!covers.animating(now + motion::FADE_IN));
    }

    #[test]
    fn a_game_without_art_is_missing_and_not_asked_again() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        let _ = covers.want(BG3);

        covers.fetched(BG3, Err(not_found()), u64::MAX);

        assert!(matches!(
            covers.cover_of(BG3, Instant::now()),
            Cover::Missing
        ));
        assert_eq!(covers.want(BG3).units(), 0);
        assert!(!covers.animating(Instant::now()), "no pulsing forever");
    }

    #[test]
    fn a_cover_deleted_to_make_room_is_asked_for_again() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        let _ = covers.want(1);
        covers.fetched(1, Ok(b"0123456789".to_vec()), 10);
        used_at(&covers, &[(1, 300)]);
        let _ = covers.want(2);

        covers.fetched(2, Ok(b"0123456789".to_vec()), 10);

        assert!(matches!(covers.cover_of(1, Instant::now()), Cover::Missing));
        assert!(matches!(
            covers.cover_of(2, Instant::now()),
            Cover::Ready { .. }
        ));
        assert_eq!(covers.want(1).units(), 1);
    }

    #[test]
    fn covers_are_asked_for_again_after_clearing() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        let _ = covers.want(BG3);
        covers.fetched(BG3, Ok(b"jpeg".to_vec()), u64::MAX);

        covers.clear();

        assert!(matches!(
            covers.cover_of(BG3, Instant::now()),
            Cover::Missing
        ));
        assert_eq!(covers.want(BG3).units(), 1);
    }
}
