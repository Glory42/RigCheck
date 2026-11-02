//! Finding the signed-in Steam account (its SteamID64) from Steam's
//! `config/loginusers.vdf`, for the owned-games lookup.

use std::path::Path;

use rigcheck_steam::account_id;
use tempfile::TempDir;

/// A home whose native Steam has `loginusers.vdf` with `users`.
fn home_with(users: &str) -> TempDir {
    let home = TempDir::new().unwrap();
    let config = home.path().join(".local/share/Steam/config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("loginusers.vdf"), users).unwrap();
    home
}

#[test]
fn picks_the_account_steam_last_signed_in_with() {
    let home = home_with(
        r#""users"
{
	"76561198000000001"
	{
		"AccountName"		"old_account"
		"PersonaName"		"Old"
		"RememberPassword"		"1"
		"MostRecent"		"0"
		"Timestamp"		"1700000000"
	}
	"76561198000000002"
	{
		"AccountName"		"current"
		"PersonaName"		"Current"
		"RememberPassword"		"1"
		"WantsOfflineMode"		"0"
		"MostRecent"		"1"
		"Timestamp"		"1600000000"
	}
}
"#,
    );

    assert_eq!(account_id(home.path()), Some(76561198000000002));
}

#[test]
fn without_a_most_recent_flag_the_latest_sign_in_counts() {
    let home = home_with(
        r#""users"
{
	"76561198000000001"
	{
		"AccountName"		"a"
		"Timestamp"		"1700000000"
	}
	"76561198000000002"
	{
		"AccountName"		"b"
		"Timestamp"		"1600000000"
	}
}
"#,
    );

    assert_eq!(account_id(home.path()), Some(76561198000000001));
}

#[test]
fn no_steam_no_account() {
    assert_eq!(account_id(Path::new("/nonexistent")), None);
}
