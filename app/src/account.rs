//! The user's own Steam Web API key, for listing every game their account
//! owns, not just the installed ones. Opt-in: without a key the app never
//! talks to the Steam Web API (ROADMAP §6).

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use iced::futures::channel::oneshot;
use iced::widget::{button, column, row, text, text_input};
use iced::{Element, Task};
use rigcheck_net::{Allowlist, Client, RateLimiter, Retry};
use rigcheck_steam::{Owned, owned_games};

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors};

/// The key box's widget ID.
pub const KEY_INPUT: &str = "steam-key";

/// In the config dir, readable by the user only.
const KEY_FILE: &str = "steam-api-key";
/// The last answer, in the cache dir, so ALL MY GAMES works offline.
const OWNED_FILE: &str = "owned-games.json";

/// Where keys come from.
const KEY_PAGE: &str = "https://steamcommunity.com/dev/apikey";

pub fn load_key(config: &Path) -> Option<String> {
    let key = fs::read_to_string(config.join(KEY_FILE)).ok()?;
    let key = key.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

/// Saves `key` where only the user can read it (permissions 600).
pub fn save_key(config: &Path, key: &str) -> io::Result<()> {
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

pub fn remove_key(config: &Path, cache: &Path) {
    let _ = fs::remove_file(config.join(KEY_FILE));
    let _ = fs::remove_file(cache.join(OWNED_FILE));
}

/// The owned games Steam last listed, if any.
pub fn load_owned(cache: &Path) -> Option<Owned> {
    owned_games(&fs::read_to_string(cache.join(OWNED_FILE)).ok()?).ok()
}

pub fn save_owned(cache: &Path, json: &str) {
    if fs::create_dir_all(cache).is_ok() {
        let _ = fs::write(cache.join(OWNED_FILE), json);
    }
}

/// Asks Steam for every game `steam_id` owns; the answer comes back as
/// [`Message::OwnedGames`]. Nothing is sent until iced runs the task.
pub fn fetch(key: String, steam_id: u64) -> Task<Message> {
    Task::perform(
        async move {
            let (sender, receiver) = oneshot::channel();
            std::thread::spawn(move || {
                let _ = sender.send(get(&key, steam_id));
            });
            receiver
                .await
                .unwrap_or_else(|_| Err("the request stopped".to_owned()))
        },
        Message::OwnedGames,
    )
}

/// The `GetOwnedGames` JSON. The net client keeps URLs out of its errors,
/// so the key can't end up in a message or log.
fn get(key: &str, steam_id: u64) -> Result<String, String> {
    let limiter = RateLimiter::new(5, Duration::from_secs(1));
    let mut client = Client::new(
        Allowlist::app(),
        limiter,
        Retry::new(1, Duration::from_secs(5)),
    )
    .map_err(|error| error.to_string())?;
    let url = format!(
        "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?key={key}&steamid={steam_id}&include_appinfo=1&include_played_free_games=1&format=json"
    );
    client.get_text(&url).map_err(|error| error.to_string())
}

/// Settings > Steam account.
pub fn view(
    input: &str,
    has_key: bool,
    steam_id: Option<u64>,
    i18n: &I18n,
    colors: Colors,
) -> Element<'static, Message> {
    let action = |id: &str, message: Message| {
        button(text(i18n.t(id)).font(BOLD).size(12))
            .padding([6, 10])
            .style(colors.button(false))
            .on_press(message)
    };
    let key_box = text_input(&i18n.t("account-key-placeholder"), input)
        .id(KEY_INPUT)
        .secure(true)
        .on_input(Message::SteamKeyInput)
        .on_submit(Message::SaveSteamKey)
        .padding(8)
        .font(BOLD);
    let account = match steam_id {
        Some(id) => i18n.t_with("account-signed-in", &[("id", &id.to_string())]),
        None => i18n.t("account-no-sign-in"),
    };
    let mut section = column![
        row![
            key_box,
            action("account-save", Message::SaveSteamKey),
            action("account-get-key", Message::OpenUrl(KEY_PAGE.to_owned())),
        ]
        .spacing(8)
        .align_y(iced::Center),
        text(account).color(colors.muted),
    ]
    .spacing(10);
    if has_key {
        section = section.push(
            row![
                text(i18n.t("account-key-saved")).font(BOLD),
                action("account-refresh", Message::RefreshOwned),
                action("account-remove", Message::RemoveSteamKey),
            ]
            .spacing(12)
            .align_y(iced::Center),
        );
    }
    section.into()
}
