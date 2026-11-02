//! Reading Steam's `GetOwnedGames` answer: every game an account owns,
//! installed or not.

use rigcheck_steam::{Owned, OwnedGame, owned_games};

fn fixture() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/fixtures/steam/owned_games.json"
    );
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn reads_every_owned_game() {
    let Owned::Games(games) = owned_games(&fixture()).unwrap() else {
        panic!("a public profile lists its games");
    };

    assert_eq!(
        games,
        [
            OwnedGame {
                app_id: 1091500,
                name: "Cyberpunk 2077".to_owned(),
                minutes_played: 3120
            },
            OwnedGame {
                app_id: 1150690,
                name: "OMORI".to_owned(),
                minutes_played: 845
            },
            OwnedGame {
                app_id: 1086940,
                name: "Baldur's Gate 3".to_owned(),
                minutes_played: 0
            },
        ]
    );
}

/// Steam answers an empty object when the profile's game details are
/// private, rather than an error: the app should say why the list is empty.
#[test]
fn a_private_profile_hides_its_games() {
    assert_eq!(owned_games(r#"{"response":{}}"#).unwrap(), Owned::Hidden);
}

#[test]
fn an_answer_that_isnt_json_is_an_error() {
    assert!(owned_games("<html>Forbidden</html>").is_err());
}
