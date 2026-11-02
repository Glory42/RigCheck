//! Asking Steam about a game games.db hasn't checked yet (ROADMAP §6, §7.1):
//! one `appdetails` request for the app ID whose page was opened. The answer
//! is kept in fetched.db by the caller.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::futures::channel::oneshot;
use iced::widget::{button, row, space, text};
use iced::{Element, Task};
use rigcheck_net::{Allowlist, Client, RateLimiter, Retry};

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors};

/// Where a lookup is, for the game page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Checking(u32),
    /// Offline, or Steam didn't answer.
    Failed(u32),
    /// Steam has no store page with requirements for it (DLC, delisted…).
    NoRequirements(u32),
}

/// One client for every lookup, so its rate limiter sees them all. Built on
/// first use.
#[derive(Clone, Default)]
pub struct Steam(Arc<Mutex<Option<Client>>>);

impl Steam {
    /// Asks Steam about `app_id` on a background thread; the answer comes
    /// back as [`Message::SteamLookup`].
    pub fn ask(&self, app_id: u32) -> Task<Message> {
        let steam = self.clone();
        let (sender, receiver) = oneshot::channel();
        std::thread::spawn(move || {
            let _ = sender.send(steam.get(app_id));
        });
        Task::perform(
            async move {
                receiver
                    .await
                    .unwrap_or_else(|_| Err("the lookup stopped".to_owned()))
            },
            move |result| Message::SteamLookup(app_id, result),
        )
    }

    /// The `appdetails` JSON for `app_id`. Blocks, waiting its turn under
    /// the rate limit.
    fn get(&self, app_id: u32) -> Result<String, String> {
        let mut client = self.0.lock().map_err(|_| "the client broke".to_owned())?;
        if client.is_none() {
            // Well under Steam's ~200 requests per 5 minutes, even for
            // someone opening game after game.
            let limiter = RateLimiter::new(20, Duration::from_secs(60));
            let retry = Retry::new(1, Duration::from_secs(5));
            *client = Some(
                Client::new(Allowlist::app(), limiter, retry).map_err(|error| error.to_string())?,
            );
        }
        let client = client.as_mut().expect("just built");
        let url = format!(
            "https://store.steampowered.com/api/appdetails?appids={app_id}&l=english&cc=us"
        );
        client.get_text(&url).map_err(|error| error.to_string())
    }
}

/// A line for the page of `app_id` about its lookup, if there's one to show.
pub fn note(
    state: State,
    app_id: u32,
    i18n: &I18n,
    colors: Colors,
) -> Option<Element<'static, Message>> {
    let line = |id: &str| text(i18n.t(id)).font(BOLD);
    match state {
        State::Checking(id) if id == app_id => Some(line("lookup-checking").into()),
        State::NoRequirements(id) if id == app_id => Some(line("lookup-no-requirements").into()),
        State::Failed(id) if id == app_id => Some(
            row![
                line("lookup-failed"),
                space::horizontal(),
                button(text(i18n.t("download-retry")).font(BOLD).size(13))
                    .padding([7, 12])
                    .style(colors.button(false))
                    .on_press(Message::RetryLookup(app_id)),
            ]
            .align_y(iced::Center)
            .into(),
        ),
        _ => None,
    }
}
