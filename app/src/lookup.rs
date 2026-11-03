//! Asking Steam about a game games.db hasn't checked yet (ROADMAP §6, §7.1):
//! one `appdetails` request for the app ID whose page was opened. The answer
//! is kept in fetched.db by the caller.

use iced::widget::{button, row, space, text};
use iced::{Element, Task};

use crate::Message;
use crate::i18n::I18n;
use crate::style::{BOLD, Colors};
use crate::web::{Web, background};

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

/// Asks Steam about `app_id`; the answer comes back as
/// [`Message::SteamLookup`].
pub fn ask(web: &Web, app_id: u32) -> Task<Message> {
    let web = web.clone();
    background(move || web.appdetails(app_id))
        .map(move |answer| Message::SteamLookup(app_id, answer))
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
