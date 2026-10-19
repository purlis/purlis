// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a project");
    std::fs::create_dir_all(crate::names::state(dir.path()).join("app")).expect("its state");
    dir
}

#[test]
fn the_addresses_kept_at_quit_are_offered_once_at_the_next_start() {
    let dir = project();

    keep(
        dir.path(),
        &[
            "https://staging.example.test/cart".to_owned(),
            "http://127.0.0.1:5173/".to_owned(),
        ],
    )
    .expect("kept");

    assert_eq!(
        take(dir.path()),
        [
            "https://staging.example.test/cart",
            "http://127.0.0.1:5173/"
        ]
    );
    assert!(take(dir.path()).is_empty(), "offered once");
    assert!(!path(dir.path()).exists(), "taken away once read");
}

#[test]
fn the_file_holds_addresses_only() {
    let dir = project();

    keep(dir.path(), &["https://shop.example.test/".to_owned()]).expect("kept");

    let text = std::fs::read_to_string(path(dir.path())).expect("the file");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let keys: Vec<&String> = doc.as_object().expect("an object").keys().collect();
    assert_eq!(keys, ["addresses"]);
}

#[test]
fn only_web_addresses_are_kept_each_once_and_none_writes_nothing() {
    let dir = project();

    keep(
        dir.path(),
        &[
            "file:///etc/passwd".to_owned(),
            "about:blank".to_owned(),
            "https://a.example.test/".to_owned(),
            "https://a.example.test/".to_owned(),
        ],
    )
    .expect("kept");
    assert_eq!(take(dir.path()), ["https://a.example.test/"]);

    keep(dir.path(), &["chrome://settings".to_owned()]).expect("nothing to keep");
    assert!(!path(dir.path()).exists());
}

#[test]
fn a_file_that_does_not_read_offers_nothing_and_is_taken_away() {
    let dir = project();
    std::fs::write(path(dir.path()), "{not json").expect("a broken file");

    assert!(take(dir.path()).is_empty());
    assert!(!path(dir.path()).exists());
}

#[test]
fn a_link_in_its_place_is_never_read_or_followed() {
    let dir = project();
    let elsewhere = dir.path().join("elsewhere.json");
    std::fs::write(&elsewhere, r#"{"addresses":["https://x.example.test/"]}"#).expect("a file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&elsewhere, path(dir.path())).expect("a link");

    assert!(take(dir.path()).is_empty());
    assert!(elsewhere.exists(), "what the link named is left as it is");
}

#[test]
fn an_address_is_kept_without_its_user_password_query_or_fragment() {
    assert_eq!(
        cut("https://ann:secret@shop.example.test:8443/reset?token=abc#access_token=xyz")
            .as_deref(),
        Some("https://shop.example.test:8443/reset")
    );
    assert_eq!(
        cut("HTTP://Example.test").as_deref(),
        Some("http://Example.test/")
    );
    assert_eq!(
        cut("https://x.example.test?code=1").as_deref(),
        Some("https://x.example.test/")
    );
    assert_eq!(cut("https://@/"), None);
    assert_eq!(cut("mailto:ann@example.test"), None);

    let dir = project();
    keep(
        dir.path(),
        &["https://ann:pw@a.example.test/p?q=1".to_owned()],
    )
    .expect("kept");
    let text = std::fs::read_to_string(path(dir.path())).expect("the file");
    assert!(!text.contains("pw") && !text.contains("q=1"), "{text}");
    assert_eq!(take(dir.path()), ["https://a.example.test/p"]);
}
