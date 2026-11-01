//! Tests for reading Steam's app list, which comes in pages.

use rigcheck_pipeline::applist;

const PAGES: [&str; 2] = [
    include_str!("../../data/fixtures/applist/page-1.json"),
    include_str!("../../data/fixtures/applist/page-2.json"),
];

#[test]
fn reads_every_page_until_steam_says_there_are_no_more() {
    let mut asked = Vec::new();

    let apps = applist::collect(|last_appid| {
        asked.push(last_appid);
        Ok(PAGES[asked.len() - 1].to_owned())
    })
    .unwrap();

    assert_eq!(asked, [0, 620], "each page continues after the last one");
    let ids: Vec<u32> = apps.iter().map(|app| app.app_id).collect();
    assert_eq!(ids, [10, 70, 620, 413150, 1086940]);
    assert_eq!(apps[4].name, "Baldur's Gate 3");
}
