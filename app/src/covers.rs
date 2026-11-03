//! Cover art: Steam's own cache first, then art rigcheck downloaded from
//! Steam's image CDN into `~/.cache/rigcheck/images/` (ROADMAP Phase 5).

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

use iced::Task;

use crate::Message;
use crate::web::{Web, background};

pub struct Covers {
    /// Where Steam installs are looked for.
    home: PathBuf,
    /// `~/.cache/rigcheck/images`
    dir: PathBuf,
    /// Asked for in this session, found or not.
    asked: HashSet<u32>,
    /// Asked for and not answered yet: their tiles pulse. Once answered
    /// without art, a tile gets a still placeholder (and `asked` keeps it
    /// from being asked again).
    loading: HashSet<u32>,
    web: Web,
}

impl Covers {
    pub fn new(home: PathBuf, dir: PathBuf, web: Web) -> Covers {
        Covers {
            home,
            dir,
            asked: HashSet::new(),
            loading: HashSet::new(),
            web,
        }
    }

    /// The cover for `app_id`, if Steam or rigcheck has one on disk.
    /// Showing a downloaded cover marks it used, for [`Covers::evict`].
    pub fn path(&self, app_id: u32) -> Option<PathBuf> {
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

    /// Bytes the downloaded covers take.
    pub fn size(&self) -> u64 {
        self.downloaded().iter().map(|(_, bytes, _)| bytes).sum()
    }

    /// Deletes the least recently used downloaded covers until they take at
    /// most `limit` bytes (ROADMAP §7.1). Steam's own cache isn't touched.
    pub fn evict(&self, limit: u64) {
        let mut covers = self.downloaded();
        covers.sort_by_key(|(_, _, used)| *used);
        let mut size: u64 = covers.iter().map(|(_, bytes, _)| bytes).sum();
        for (path, bytes, _) in covers {
            if size <= limit {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                size -= bytes;
            }
        }
    }

    /// Deletes every downloaded cover.
    pub fn clear(&self) {
        self.evict(0);
    }

    /// Each downloaded cover: its path, size and when it was last used.
    fn downloaded(&self) -> Vec<(PathBuf, u64, SystemTime)> {
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
    pub fn save(&self, app_id: u32, jpeg: &[u8]) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let file = self.file(app_id);
        let part = file.with_extension("jpg.part");
        std::fs::write(&part, jpeg)?;
        std::fs::rename(&part, &file)?;
        Ok(file)
    }

    /// Downloads the cover for `app_id` unless it's on disk or was already
    /// asked for in this session (a game without art isn't asked again).
    /// The answer comes back as [`Message::CoverFetched`]; nothing is sent
    /// until iced runs the task.
    pub fn fetch(&mut self, app_id: u32) -> Task<Message> {
        if self.path(app_id).is_some() || !self.asked.insert(app_id) {
            return Task::none();
        }
        self.loading.insert(app_id);
        let web = self.web.clone();
        background(move || web.cover(app_id)).map(move |jpeg| Message::CoverFetched(app_id, jpeg))
    }

    /// Downloads and keeps `app_id`'s cover now, blocking: for the README
    /// screenshots, which don't run iced's tasks.
    #[cfg(test)]
    pub fn download_now(&self, app_id: u32) -> Option<PathBuf> {
        let jpeg = self.web.cover(app_id).ok()?;
        self.save(app_id, &jpeg).ok()
    }

    /// The download for `app_id` ended, with a cover or without.
    pub fn settled(&mut self, app_id: u32) {
        self.loading.remove(&app_id);
    }

    pub fn is_loading(&self, app_id: u32) -> bool {
        self.loading.contains(&app_id)
    }

    /// Whether any cover is on its way, so the pulse needs frames.
    pub fn any_loading(&self) -> bool {
        !self.loading.is_empty()
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

        assert_eq!(covers.fetch(BG3).units(), 1);
        assert_eq!(covers.fetch(BG3).units(), 0);

        covers.save(620, b"jpeg").unwrap();
        assert_eq!(covers.fetch(620).units(), 0, "already on disk");
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
        let covers = covers(&dir);
        used_at(&covers, &[(1, 300), (2, 100), (3, 200)]);

        covers.evict(20);

        assert_eq!(covers.size(), 20);
        assert!(covers.path(1).is_none(), "used longest ago");
        assert!(covers.path(2).is_some() && covers.path(3).is_some());
    }

    #[test]
    fn showing_a_cover_counts_as_using_it() {
        let dir = TempDir::new().unwrap();
        let covers = covers(&dir);
        used_at(&covers, &[(1, 300), (2, 100)]);

        assert!(covers.path(1).is_some()); // shown just now
        covers.evict(10);

        assert!(covers.path(1).is_some());
        assert!(covers.path(2).is_none());
    }

    #[test]
    fn clearing_removes_every_downloaded_cover() {
        let dir = TempDir::new().unwrap();
        let covers = covers(&dir);
        used_at(&covers, &[(1, 0), (2, 0)]);

        covers.clear();

        assert_eq!(covers.size(), 0);
        assert!(covers.path(1).is_none());
    }

    #[test]
    fn a_cover_is_loading_from_its_request_until_it_arrives() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        assert!(!covers.is_loading(BG3) && !covers.any_loading());

        let _ = covers.fetch(BG3);
        assert!(covers.is_loading(BG3) && covers.any_loading());

        covers.settled(BG3);
        assert!(!covers.is_loading(BG3) && !covers.any_loading());
    }

    #[test]
    fn a_game_without_art_is_missing_and_not_asked_again() {
        let dir = TempDir::new().unwrap();
        let mut covers = covers(&dir);
        let _ = covers.fetch(BG3);

        covers.settled(BG3);

        assert!(!covers.is_loading(BG3));
        assert_eq!(covers.fetch(BG3).units(), 0);
        assert!(!covers.is_loading(BG3), "no pulsing forever");
    }
}
