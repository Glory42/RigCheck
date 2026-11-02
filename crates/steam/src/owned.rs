//! Every game a Steam account owns, from the Steam Web API's
//! `IPlayerService/GetOwnedGames`. This only reads the answer; the app makes
//! the request (with the user's own key) through `crates/net`.

use serde::Deserialize;

/// One owned game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedGame {
    pub app_id: u32,
    pub name: String,
    pub minutes_played: u32,
}

/// What `GetOwnedGames` said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owned {
    Games(Vec<OwnedGame>),
    /// An empty answer: the profile's game details are private.
    Hidden,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Reads a `GetOwnedGames` answer (`include_appinfo=1`, so names are in it).
pub fn owned_games(json: &str) -> Result<Owned, Error> {
    #[derive(Deserialize)]
    struct Answer {
        response: Response,
    }
    #[derive(Deserialize)]
    struct Response {
        games: Option<Vec<Game>>,
    }
    #[derive(Deserialize)]
    struct Game {
        appid: u32,
        #[serde(default)]
        name: String,
        #[serde(default)]
        playtime_forever: u32,
    }

    let answer: Answer = serde_json::from_str(json)?;
    Ok(match answer.response.games {
        None => Owned::Hidden,
        Some(games) => Owned::Games(
            games
                .into_iter()
                .map(|game| OwnedGame {
                    app_id: game.appid,
                    name: game.name,
                    minutes_played: game.playtime_forever,
                })
                .collect(),
        ),
    })
}
