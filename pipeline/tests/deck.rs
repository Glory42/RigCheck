//! Tests for reading Steam Deck compatibility from Steam's store endpoint.

use rigcheck_pipeline::deck::status;

fn fixture(app_id: u32) -> String {
    let path = format!(
        "{}/../data/fixtures/deck/{app_id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn reads_the_deck_rating() {
    let cases = [
        (1086940, "verified"),    // Baldur's Gate 3
        (22380, "playable"),      // Fallout: New Vegas
        (1085660, "unsupported"), // Destiny 2
        (654600, "unknown"),      // PowBall Renaissance, not reviewed
    ];
    for (app_id, expected) in cases {
        assert_eq!(status(&fixture(app_id)).unwrap(), expected, "{app_id}");
    }
}
