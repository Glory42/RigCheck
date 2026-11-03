//! The user's own Steam Web API key, for listing every game their account
//! owns, not just the installed ones. Opt-in: without a key the app never
//! talks to the Steam Web API (ROADMAP §6).
//!
//! [`Account`] holds it all: the saved key, the signed-in account, what
//! Steam last listed and how it last answered. Settings > Steam account
//! changes it; ALL MY GAMES reads it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use iced::widget::{button, column, row, text, text_input};
use iced::{Element, Task};
use rigcheck_steam::{Owned, OwnedGame, owned_games};

use crate::i18n::I18n;
use crate::style::{BOLD, Colors};
use crate::web::{Failure, Web, background};

/// The key box's widget ID.
pub const KEY_INPUT: &str = "steam-key";

/// In the config dir, readable by the user only.
const KEY_FILE: &str = "steam-api-key";
/// The last answer, in the cache dir, so ALL MY GAMES works offline.
const OWNED_FILE: &str = "owned-games.json";

/// Where keys come from.
const KEY_PAGE: &str = "https://steamcommunity.com/dev/apikey";

/// What Settings > Steam account and Steam's answers do to the account.
#[derive(Debug, Clone)]
pub enum Message {
    /// What's typed in the key box.
    Input(String),
    Save,
    Remove,
    Refresh,
    /// Steam's `GetOwnedGames` answer, or why there's none.
    Owned(Result<String, Failure>),
}

/// The Steam account, as far as ALL MY GAMES is concerned.
pub struct Account {
    config: PathBuf,
    cache: PathBuf,
    web: Web,
    /// The signed-in Steam account, from Steam's own files.
    steam_id: Option<u64>,
    key: Key,
    /// Every game the account owns, as Steam last listed them. Kept for
    /// offline starts, and through a refresh that fails.
    owned: Option<Owned>,
    /// What's typed in the key box; never shown (a secure input) and only
    /// written to disk by SAVE KEY.
    input: String,
    /// Why the typed key wasn't saved, if it wasn't.
    bad_key: Option<BadKey>,
}

/// Whether a key is saved, and if so how Steam last answered to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    None,
    Saved(Answer),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// Not asked yet, or waiting for Steam.
    Pending,
    Listed,
    /// Offline, or Steam failed.
    Failed,
    /// Steam answered 401/403: it doesn't accept the key.
    Rejected,
}

impl Account {
    /// The account as saved on disk. `home` is where Steam's files are
    /// looked for. Unless `ask` is false (updates only when asked), Steam is
    /// asked for the owned games right away.
    pub fn new(
        config: PathBuf,
        cache: PathBuf,
        home: &Path,
        web: Web,
        ask: bool,
    ) -> (Account, Task<crate::Message>) {
        let account = Account {
            key: match load_key(&config) {
                Some(_) => Key::Saved(Answer::Pending),
                None => Key::None,
            },
            steam_id: rigcheck_steam::account_id(home),
            owned: load_owned(&cache),
            config,
            cache,
            web,
            input: String::new(),
            bad_key: None,
        };
        let task = if ask { account.ask() } else { Task::none() };
        (account, task)
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Input(key) => {
                self.input = key;
                Task::none()
            }
            Message::Save => {
                let key = self.input.trim().to_owned();
                if let Err(bad) = check_key(&key) {
                    self.bad_key = Some(bad);
                    return Task::none();
                }
                self.input.clear();
                self.bad_key = None;
                if let Err(error) = save_key(&self.config, &key) {
                    eprintln!("couldn't save the Steam key: {error}");
                    return Task::none();
                }
                self.key = Key::Saved(Answer::Pending);
                self.ask()
            }
            Message::Remove => {
                remove_key(&self.config, &self.cache);
                self.key = Key::None;
                self.owned = None;
                Task::none()
            }
            Message::Refresh => self.ask(),
            Message::Owned(answer) => {
                // An answer to a key that has since been removed.
                if self.key == Key::None {
                    return Task::none();
                }
                let listed = answer.and_then(|json| {
                    let owned =
                        owned_games(&json).map_err(|error| Failure::Failed(error.to_string()))?;
                    save_owned(&self.cache, &json);
                    Ok(owned)
                });
                self.key = Key::Saved(match listed {
                    Ok(owned) => {
                        self.owned = Some(owned);
                        Answer::Listed
                    }
                    Err(Failure::Rejected) => Answer::Rejected,
                    Err(Failure::Failed(error)) => {
                        // Never the URL (it holds the key): the net client keeps it out.
                        eprintln!("couldn't list the owned games: {error}");
                        Answer::Failed
                    }
                });
                Task::none()
            }
        }
    }

    /// Every game the account owns, if Steam has listed them.
    pub fn owned_games(&self) -> &[OwnedGame] {
        match &self.owned {
            Some(Owned::Games(games)) => games,
            _ => &[],
        }
    }

    /// Why ALL MY GAMES has nothing to list, as a message ID; `None` if it
    /// has a list.
    pub fn note(&self) -> Option<&'static str> {
        Some(match (&self.owned, self.key) {
            // A saved list still shows, even if a later refresh failed.
            (Some(Owned::Games(_)), _) => return None,
            (_, Key::Saved(Answer::Rejected)) => "library-owned-key-rejected",
            (Some(Owned::Hidden), _) => "library-owned-private",
            (None, Key::None) => "library-owned-no-key",
            (None, Key::Saved(_)) if self.steam_id.is_none() => "library-owned-no-account",
            (None, Key::Saved(Answer::Failed)) => "library-owned-failed",
            (None, Key::Saved(_)) => "library-owned-loading",
        })
    }

    /// Asks Steam for every game the account owns; the answer comes back as
    /// [`Message::Owned`]. Nothing without a saved key and a signed-in
    /// account.
    fn ask(&self) -> Task<crate::Message> {
        // Read from disk each time: the key isn't kept in memory.
        let (Some(key), Some(steam_id)) = (load_key(&self.config), self.steam_id) else {
            return Task::none();
        };
        let web = self.web.clone();
        background(move || web.owned_games(&key, steam_id))
            .map(|answer| crate::Message::Account(Message::Owned(answer)))
    }

    /// Settings > Steam account.
    pub fn view(&self, i18n: &I18n, colors: Colors) -> Element<'static, crate::Message> {
        let account = |message| crate::Message::Account(message);
        let action = |id: &str, message: crate::Message| {
            button(text(i18n.t(id)).font(BOLD).size(12))
                .padding([6, 10])
                .style(colors.button(false))
                .on_press(message)
        };
        let key_box = text_input(&i18n.t("account-key-placeholder"), &self.input)
            .id(KEY_INPUT)
            .secure(true)
            .on_input(move |key| account(Message::Input(key)))
            .on_submit(account(Message::Save))
            .padding(8)
            .font(BOLD);
        let signed_in = match self.steam_id {
            Some(id) => i18n.t_with("account-signed-in", &[("id", &id.to_string())]),
            None => i18n.t("account-no-sign-in"),
        };
        let mut section = column![
            row![
                key_box,
                action("account-save", account(Message::Save)),
                action(
                    "account-get-key",
                    crate::Message::OpenUrl(KEY_PAGE.to_owned())
                ),
            ]
            .spacing(8)
            .align_y(iced::Center),
            text(signed_in).color(colors.muted),
        ]
        .spacing(10);
        if let Some(bad) = self.bad_key {
            let id = match bad {
                BadKey::SteamId => "account-key-is-steam-id",
                BadKey::Shape => "account-key-shape",
            };
            section = section.push(text(i18n.t(id)).font(BOLD));
        }
        if self.key != Key::None {
            section = section.push(
                row![
                    text(i18n.t("account-key-saved")).font(BOLD),
                    action("account-refresh", account(Message::Refresh)),
                    action("account-remove", account(Message::Remove)),
                ]
                .spacing(12)
                .align_y(iced::Center),
            );
        }
        section.into()
    }
}

/// Why a typed key wasn't saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadKey {
    /// 17 digits: the account's SteamID, which is found automatically.
    SteamId,
    /// Not 32 letters and digits (A–F, 0–9).
    Shape,
}

/// Checks that `key` looks like a Steam Web API key: 32 hexadecimal
/// characters.
fn check_key(key: &str) -> Result<(), BadKey> {
    if key.len() == 17 && key.bytes().all(|b| b.is_ascii_digit()) {
        Err(BadKey::SteamId)
    } else if key.len() == 32 && key.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(BadKey::Shape)
    }
}

fn load_key(config: &Path) -> Option<String> {
    let key = fs::read_to_string(config.join(KEY_FILE)).ok()?;
    let key = key.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

/// Saves `key` where only the user can read it (permissions 600).
fn save_key(config: &Path, key: &str) -> io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    fs::create_dir_all(config)?;
    let path = config.join(KEY_FILE);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    // `mode` only applies to a new file: tighten an old one too.
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    io::Write::write_all(&mut file, format!("{key}\n").as_bytes())
}

fn remove_key(config: &Path, cache: &Path) {
    let _ = fs::remove_file(config.join(KEY_FILE));
    let _ = fs::remove_file(cache.join(OWNED_FILE));
}

/// The owned games Steam last listed, if any.
fn load_owned(cache: &Path) -> Option<Owned> {
    owned_games(&fs::read_to_string(cache.join(OWNED_FILE)).ok()?).ok()
}

fn save_owned(cache: &Path, json: &str) {
    if fs::create_dir_all(cache).is_ok() {
        let _ = fs::write(cache.join(OWNED_FILE), json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::fake::Fake;
    use tempfile::TempDir;

    const KEY: &str = "0123456789ABCDEF0123456789ABCDEF";

    fn account(dir: &TempDir) -> Account {
        let web = Web::with(Fake::replying(Ok("{}")));
        let (account, _) = Account::new(
            dir.path().join("config"),
            dir.path().join("cache"),
            &dir.path().join("home"),
            web,
            false,
        );
        account
    }

    fn saved(dir: &TempDir) -> Account {
        let mut account = account(dir);
        let _ = account.update(Message::Input(KEY.to_owned()));
        let _ = account.update(Message::Save);
        account
    }

    fn owns_one_game() -> Message {
        let json = r#"{"response":{"game_count":1,"games":[{"appid":620,"name":"Portal 2"}]}}"#;
        Message::Owned(Ok(json.to_owned()))
    }

    #[test]
    fn a_key_is_checked_before_it_is_saved() {
        assert_eq!(check_key(KEY), Ok(()));
        assert_eq!(check_key("76561198388197563"), Err(BadKey::SteamId));
        assert_eq!(check_key("not a key"), Err(BadKey::Shape));
    }

    #[test]
    fn without_a_key_or_a_signed_in_account_the_note_says_which() {
        let dir = TempDir::new().unwrap();
        assert_eq!(account(&dir).note(), Some("library-owned-no-key"));
        // No Steam files under `home`, so no account.
        assert_eq!(saved(&dir).note(), Some("library-owned-no-account"));
    }

    #[test]
    fn removing_a_rejected_key_starts_over() {
        let dir = TempDir::new().unwrap();
        let mut account = saved(&dir);
        let _ = account.update(Message::Owned(Err(Failure::Rejected)));
        assert_eq!(account.note(), Some("library-owned-key-rejected"));

        let _ = account.update(Message::Remove);

        assert_eq!(account.note(), Some("library-owned-no-key"));
    }

    #[test]
    fn a_failed_refresh_keeps_the_list_steam_gave_before() {
        let dir = TempDir::new().unwrap();
        let mut account = saved(&dir);
        let _ = account.update(owns_one_game());

        let _ = account.update(Message::Owned(Err(Failure::Failed("offline".to_owned()))));

        assert_eq!(account.owned_games().len(), 1);
        assert_eq!(account.note(), None);
    }

    #[test]
    fn an_answer_after_the_key_was_removed_is_dropped() {
        let dir = TempDir::new().unwrap();
        let mut account = saved(&dir);
        let _ = account.update(Message::Remove);

        let _ = account.update(owns_one_game());

        assert!(account.owned_games().is_empty());
        assert!(!dir.path().join("cache").join(OWNED_FILE).exists());
    }
}
