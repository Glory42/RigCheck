//! Cover art: Steam's own cache first, then art rigcheck downloaded from
//! Steam's image CDN into `~/.cache/rigcheck/images/` (ROADMAP Phase 5).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::Task;
use iced::futures::channel::oneshot;
use rigcheck_net::{Allowlist, Client, RateLimiter, Retry};

use crate::Message;

pub struct Covers {
    /// Where Steam installs are looked for.
    home: PathBuf,
    /// `~/.cache/rigcheck/images`
    dir: PathBuf,
    /// Asked for in this session, found or not.
    asked: HashSet<u32>,
    /// One client for every download, so its rate limiter sees them all.
    cdn: Arc<Mutex<Option<Client>>>,
}

impl Covers {
    pub fn new(home: PathBuf, dir: PathBuf) -> Covers {
        Covers {
            home,
            dir,
            asked: HashSet::new(),
            cdn: Arc::default(),
        }
    }

    /// The cover for `app_id`, if Steam or rigcheck has one on disk.
    pub fn path(&self, app_id: u32) -> Option<PathBuf> {
        rigcheck_steam::cover_art(&self.home, app_id).or_else(|| {
            let downloaded = self.file(app_id);
            downloaded.is_file().then_some(downloaded)
        })
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
        let cdn = self.cdn.clone();
        Task::perform(
            async move {
                let (sender, receiver) = oneshot::channel();
                std::thread::spawn(move || {
                    let _ = sender.send(download(&cdn, app_id));
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("the download stopped".to_owned()))
            },
            move |result| Message::CoverFetched(app_id, result),
        )
    }

    fn file(&self, app_id: u32) -> PathBuf {
        self.dir.join(format!("{app_id}.jpg"))
    }
}

/// Steam's 600×900 library art for `app_id`, the same image Steam caches.
fn download(cdn: &Mutex<Option<Client>>, app_id: u32) -> Result<Vec<u8>, String> {
    let mut client = cdn.lock().map_err(|_| "the client broke".to_owned())?;
    if client.is_none() {
        // A CDN, so faster than the store API; still paced, since a big
        // library asks for hundreds at once.
        let limiter = RateLimiter::new(10, Duration::from_secs(1));
        let retry = Retry::new(1, Duration::from_secs(2));
        *client = Some(Client::new(Allowlist::app(), limiter, retry).map_err(|e| e.to_string())?);
    }
    let client = client.as_mut().expect("just built");
    let url = format!(
        "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/library_600x900.jpg"
    );
    let mut jpeg = Vec::new();
    client
        .download(&url, &mut jpeg, |_| {})
        .map_err(|e| e.to_string())?;
    Ok(jpeg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const BG3: u32 = 1086940;

    fn covers(dir: &TempDir) -> Covers {
        Covers::new(dir.path().join("home"), dir.path().join("cache/images"))
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
}
