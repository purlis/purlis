// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_session_protocol::browsing::{Act, Browsed, PageSaid, Typed};

use super::*;

const CHAT: &str = "01JB0000000000000000000000";

fn project() -> tempfile::TempDir {
    tempfile::tempdir().expect("a project")
}

fn on(address: &str) -> Browsed {
    Browsed {
        page: Some(PageSaid {
            page: "p1".into(),
            address: address.into(),
            title: "Cart".into(),
            current: true,
        }),
        settled: true,
        ..Browsed::default()
    }
}

fn shot(bytes: &[u8]) -> Browsed {
    Browsed {
        png: Some(bytes.to_vec()),
        ..on("https://shop.example.test/cart")
    }
}

fn screenshot() -> Act {
    Act::Screenshot { page: None }
}

fn click(target: &str) -> Act {
    Act::Click {
        target: target.into(),
    }
}

fn record_act(root: &Path, at: u64, act: &Act, browsed: &Browsed) {
    let happened = acted("b1", act, browsed, at, None).expect("a moment");
    record(root, CHAT, happened).expect("recorded");
}

#[test]
fn the_timeline_lists_the_agents_acts_with_the_screenshots_its_tools_returned() {
    let dir = project();

    record_act(
        dir.path(),
        1,
        &click("e12"),
        &on("https://shop.example.test/cart"),
    );
    record_act(dir.path(), 2, &screenshot(), &shot(b"\x89PNG one"));

    let moments = read(dir.path(), CHAT);
    assert_eq!(moments.len(), 2);
    assert_eq!(moments[0].did, "Clicked e12");
    assert_eq!(moments[0].by, By::Agent);
    assert_eq!(moments[0].browser, "b1");
    assert_eq!(moments[0].shot, None);
    assert_eq!(moments[1].did, "Took a screenshot");
    let n = moments[1].shot.expect("its screenshot kept");
    assert_eq!(
        shot_of(dir.path(), CHAT, n).as_deref(),
        Some(&b"\x89PNG one"[..])
    );
}

#[test]
fn a_page_is_kept_without_its_query_or_fragment() {
    let dir = project();

    record_act(
        dir.path(),
        1,
        &click("e1"),
        &on("https://user:pw@shop.example.test/reset?token=abc#top"),
    );

    assert_eq!(
        read(dir.path(), CHAT)[0].page.as_deref(),
        Some("https://shop.example.test/reset")
    );
}

#[test]
fn what_the_agent_typed_is_not_kept() {
    let dir = project();
    let typed = Act::Type {
        page: None,
        fields: vec![
            Typed {
                into: Some("e4".into()),
                text: "hunter2".into(),
                replace: false,
            },
            Typed {
                into: None,
                text: "more".into(),
                replace: true,
            },
        ],
    };

    record_act(dir.path(), 1, &typed, &on("https://shop.example.test/"));

    let moments = read(dir.path(), CHAT);
    assert_eq!(moments[0].did, "Typed into 2 fields");
    let file = std::fs::read_to_string(folder(dir.path(), CHAT).join(FILE)).expect("its file");
    assert!(!file.contains("hunter2"), "{file}");
}

#[test]
fn a_retry_and_a_read_of_the_page_are_no_moment() {
    let repeated = Browsed {
        repeated: true,
        ..on("https://shop.example.test/")
    };
    assert!(acted("b1", &click("e1"), &repeated, 1, None).is_none());
    let snapshot = Act::Snapshot {
        page: None,
        within: None,
        from: None,
        lines: None,
    };
    assert!(acted("b1", &snapshot, &on("https://shop.example.test/"), 1, None).is_none());
}

#[test]
fn no_screenshot_is_kept_for_a_moment_the_person_or_purlis_drove() {
    let dir = project();

    for (at, driver) in [(1, By::Person), (2, By::Purlis)] {
        let happened = acted(
            "b1",
            &screenshot(),
            &shot(b"\x89PNG typed"),
            at,
            Some(driver),
        )
        .expect("a moment");
        record(dir.path(), CHAT, happened).expect("recorded");
    }

    let moments = read(dir.path(), CHAT);
    assert_eq!(moments.len(), 2);
    for moment in &moments {
        assert_eq!(moment.shot, None);
        assert_eq!(moment.unkept, Some(Unkept::Driven));
    }
    let pngs = std::fs::read_dir(folder(dir.path(), CHAT))
        .expect("its folder")
        .filter(|one| {
            one.as_ref()
                .is_ok_and(|one| one.path().extension().is_some_and(|it| it == "png"))
        })
        .count();
    assert_eq!(pngs, 0, "no screenshot on disk");
}

#[test]
fn the_person_and_purlis_driving_are_moments_without_a_screenshot() {
    let dir = project();

    record(dir.path(), CHAT, took_over("b1", 1)).expect("recorded");
    record(dir.path(), CHAT, handed_back("b1", 2)).expect("recorded");
    record(
        dir.path(),
        CHAT,
        signed_in(
            "b1",
            "shop",
            &on("https://shop.example.test/account?x=1"),
            3,
        ),
    )
    .expect("recorded");

    let moments = read(dir.path(), CHAT);
    let said: Vec<(&str, By)> = moments
        .iter()
        .map(|one| (one.did.as_str(), one.by))
        .collect();
    assert_eq!(
        said,
        [
            ("You took the browser over", By::Person),
            ("You handed the browser back", By::Person),
            ("purlis signed in with shop", By::Purlis),
        ]
    );
    assert!(moments.iter().all(|one| one.shot.is_none()));
    assert_eq!(
        moments[2].page.as_deref(),
        Some("https://shop.example.test/account")
    );
}

#[test]
fn the_timeline_keeps_its_newest_moments_within_its_bound() {
    let dir = project();

    for at in 0..(MOST_MOMENTS as u64 + 5) {
        record_act(
            dir.path(),
            at,
            &click("e1"),
            &on("https://shop.example.test/"),
        );
    }

    let moments = read(dir.path(), CHAT);
    assert_eq!(moments.len(), MOST_MOMENTS);
    assert_eq!(moments[0].at, 5, "the oldest went first");
}

#[test]
fn the_timeline_keeps_its_newest_screenshots_within_its_bound() {
    let dir = project();

    for at in 0..(MOST_SHOTS as u64 + 3) {
        record_act(
            dir.path(),
            at,
            &screenshot(),
            &shot(format!("png {at}").as_bytes()),
        );
    }

    let moments = read(dir.path(), CHAT);
    let kept: Vec<u64> = moments.iter().filter_map(|one| one.shot).collect();
    assert_eq!(kept.len(), MOST_SHOTS);
    for old in &moments[..3] {
        assert_eq!(old.shot, None);
        assert_eq!(old.unkept, Some(Unkept::Bound));
    }
    let on_disk = std::fs::read_dir(folder(dir.path(), CHAT))
        .expect("its folder")
        .filter(|one| {
            one.as_ref()
                .is_ok_and(|one| one.path().extension().is_some_and(|it| it == "png"))
        })
        .count();
    assert_eq!(on_disk, MOST_SHOTS, "the ones let go are gone from disk");
    let newest = moments.last().and_then(|one| one.shot).expect("the newest");
    assert_eq!(
        shot_of(dir.path(), CHAT, newest).as_deref(),
        Some(format!("png {}", MOST_SHOTS + 2).as_bytes())
    );
}

#[test]
fn a_screenshot_past_the_largest_kept_is_not_kept() {
    let dir = project();

    record_act(
        dir.path(),
        1,
        &screenshot(),
        &shot(&vec![0; MOST_SHOT_BYTES + 1]),
    );

    let moments = read(dir.path(), CHAT);
    assert_eq!(moments[0].shot, None);
    assert_eq!(moments[0].unkept, Some(Unkept::TooLarge));
}

#[test]
fn only_the_newest_chats_keep_a_timeline() {
    let dir = project();
    let chats: Vec<String> = (0..=MOST_CHATS)
        .map(|n| format!("01JB00000000000000000000{n:02}"))
        .collect();

    for (at, chat) in chats.iter().enumerate() {
        let happened = took_over("b1", at as u64);
        record(dir.path(), chat, happened).expect("recorded");
        // Apart in time, as a file system's clock tells them.
        std::thread::sleep(std::time::Duration::from_millis(15));
    }

    assert!(
        read(dir.path(), &chats[0]).is_empty(),
        "the oldest chat's is gone"
    );
    for chat in &chats[1..] {
        assert_eq!(read(dir.path(), chat).len(), 1, "{chat}");
    }
}

#[test]
fn a_chat_that_is_no_ulid_has_no_timeline() {
    let dir = project();

    assert!(record(dir.path(), "../outside", took_over("b1", 1)).is_err());
    assert!(read(dir.path(), "../outside").is_empty());
    assert!(!dir.path().join("outside").exists());
}

#[test]
fn a_removed_timeline_is_gone_with_its_screenshots() {
    let dir = project();
    record_act(dir.path(), 1, &screenshot(), &shot(b"png"));

    remove(dir.path(), CHAT).expect("removed");

    assert!(read(dir.path(), CHAT).is_empty());
    assert!(!folder(dir.path(), CHAT).exists());
    assert!(remove(dir.path(), CHAT).is_ok(), "removing none is fine");
}

#[test]
fn a_screenshot_the_timeline_does_not_list_is_not_read() {
    let dir = project();
    record_act(
        dir.path(),
        1,
        &click("e1"),
        &on("https://shop.example.test/"),
    );
    std::fs::write(folder(dir.path(), CHAT).join("7.png"), b"planted").expect("planted");

    assert_eq!(shot_of(dir.path(), CHAT, 7), None);
}
