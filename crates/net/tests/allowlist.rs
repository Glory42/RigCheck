//! Tests for the host allowlists (ROADMAP §6): the only places rigcheck may
//! send requests to.

use rigcheck_net::Allowlist;

#[test]
fn the_app_may_only_reach_the_three_privacy_destinations() {
    let app = Allowlist::app();

    for url in [
        "https://github.com/Glory42/rigcheck/releases/download/db-latest/manifest.json",
        // Where github.com redirects release downloads.
        "https://release-assets.githubusercontent.com/github-production-release-asset/1441615821/eec53fcc-5b7c-4082-aa08-4b61f6291d26",
        "https://store.steampowered.com/api/appdetails?appids=1086940",
        "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1086940/library_600x900.jpg",
        // Only with the user's own key: every game the account owns.
        "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/",
    ] {
        assert!(app.allows(url), "{url} should be allowed");
    }
    for url in [
        "https://example.com/",
        "http://store.steampowered.com/api/appdetails",
        "https://store.steampowered.com.example.com/api/appdetails",
        "not a url",
    ] {
        assert!(!app.allows(url), "{url} should be refused");
    }
}

#[test]
fn the_pipeline_may_also_reach_its_data_sources() {
    let pipeline = Allowlist::pipeline();

    for url in [
        "https://api.steampowered.com/IStoreService/GetAppList/v1/",
        "https://store.steampowered.com/api/appdetails?appids=1086940",
        "https://github.com/bdefore/protondb-data/raw/master/reports/reports_oct1_2026.tar.gz",
        "https://raw.githubusercontent.com/AreWeAntiCheatYet/AreWeAntiCheatYet/master/games.json",
    ] {
        assert!(pipeline.allows(url), "{url} should be allowed");
    }
    assert!(!pipeline.allows("https://example.com/"));
}
