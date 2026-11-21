// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The second press is of the first** (ADR 0128 D8.5): the first press is answered with a nonce,
//! the second names it, a double click of the first performs nothing, and a double click of the
//! second performs once. What a release shows is what is made, and a tag made with a release that
//! then failed is said exactly.

use purlis_core::heldact::{Asked, HeldActs, NotDone, Pressed, Requested};

use crate::support::held_acts::{Fake, NoTrees, Rec, Second, repo, sha, tag, the_person, world};

fn tag_v1() -> Requested {
    Requested::Tag {
        repo: repo("acme/app"),
        name: tag("v1"),
        rev: "main".into(),
    }
}

fn forge() -> Fake {
    let forge = Fake::github();
    forge.with(|s| {
        s.revs.insert("main".into(), sha('1'));
    });
    forge
}

#[test]
fn a_double_click_of_the_first_press_performs_nothing() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &tag_v1(), 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    let Ok(Pressed::AsksOnceMore { nonce: first, .. }) =
        held.press(&world, id, &digest, None, the_person())
    else {
        panic!()
    };
    // The same click, again, with no nonce: the same answer, and still nothing made.
    let Ok(Pressed::AsksOnceMore { nonce: again, .. }) =
        held.press(&world, id, &digest, None, the_person())
    else {
        panic!("a repeat of the first press is the first press")
    };
    assert_eq!(first, again);
    assert!(forge.log().is_empty());
    // A second press naming another nonce is no second press of this first.
    assert_eq!(
        held.press(&world, id, &digest, Some("not-the-nonce"), the_person())
            .unwrap_err(),
        NotDone::NotWhatYouSaw
    );
    assert!(forge.log().is_empty());
}

#[test]
fn a_double_click_of_the_second_press_performs_once() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &tag_v1(), 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    let Ok(Pressed::AsksOnceMore { nonce, .. }) =
        held.press(&world, id, &digest, None, the_person())
    else {
        panic!()
    };
    // Two threads press the second press at once.
    let results: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| scope.spawn(|| held.press(&world, id, &digest, Some(&nonce), the_person())))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Ok(Pressed::Done(_))))
            .count(),
        1,
        "{results:?}"
    );
    assert_eq!(forge.log().len(), 1, "made once: {:?}", forge.log());
}

#[test]
fn a_nonce_does_not_outlive_a_change_of_what_is_shown() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &tag_v1(), 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    let Ok(Pressed::AsksOnceMore { nonce, .. }) =
        held.press(&world, id, &digest, None, the_person())
    else {
        panic!()
    };
    forge.with(|s| {
        s.merged.insert(sha('1'));
    });
    assert_eq!(
        held.press(&world, id, &digest, Some(&nonce), the_person())
            .unwrap_err(),
        NotDone::ShowsOtherwise
    );
    let (_, fresh) = held.shown(id).unwrap();
    // The old nonce is no second press of the new row: it is a first press again.
    assert!(matches!(
        held.press(&world, id, &fresh, Some(&nonce), the_person()),
        Ok(Pressed::AsksOnceMore { .. })
    ));
    assert!(forge.log().is_empty());
}

fn release(title: &str, notes: &str) -> Requested {
    Requested::Release {
        repo: repo("acme/app"),
        tag: tag("v9"),
        rev: Some("main".into()),
        title: title.into(),
        notes: notes.into(),
        draft: false,
        prerelease: false,
    }
}

#[test]
fn a_release_shows_and_makes_the_same_text_with_nothing_in_it_that_does_not_show() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    // A title with a right-to-left override and a line break, notes with a zero-width space and a
    // bell, and a CRLF.
    let asked = held.ask(
        &world,
        1,
        true,
        &release(
            "Safe\u{202E}gnp.exe\nnext",
            "line one\r\nline\u{200B} two\u{7}",
        ),
        10,
    );
    let Asked::Held { id, .. } = asked else {
        panic!("{asked:?}")
    };
    let (shown, digest) = held.shown(id).unwrap();
    let title = shown.rows.iter().find(|r| r.label == "Title").unwrap();
    assert_eq!(title.value, "Safegnp.exe next");
    assert_eq!(shown.body.as_ref().unwrap().text, "line one\nline two");
    crate::support::held_acts::press(&held, &world, id, &digest).unwrap();
    crate::support::held_acts::press(&held, &world, id, &digest).unwrap();
    // What the forge was given is what the row said, byte for byte.
    assert!(
        forge.log()[1].contains("title=\"Safegnp.exe next\""),
        "{:?}",
        forge.log()
    );
    assert!(
        forge.log()[1].contains("notes=\"line one\\nline two\""),
        "{:?}",
        forge.log()
    );
}

#[test]
fn a_title_or_notes_longer_than_purlis_shows_are_not_offered() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    for requested in [
        release(&"t".repeat(201), ""),
        release("t", &"n".repeat(60_001)),
    ] {
        assert!(matches!(
            held.ask(&world, 1, true, &requested, 10),
            Asked::Refused(purlis_core::heldact::Refused::Text(_))
        ));
    }
}

#[test]
fn a_tag_made_and_a_release_that_failed_is_said_exactly_and_asked_again_against_the_tag() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &release("Nine", "notes"), 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    crate::support::held_acts::press(&held, &world, id, &digest).unwrap();
    // The tag is made; the release call then fails: no tag is made twice.
    forge.with(|s| s.fail_release = Some("the notes were refused".into()));
    let refused = crate::support::held_acts::press(&held, &world, id, &digest).unwrap_err();
    let NotDone::TagMadeReleaseFailed { tag, at, why } = &refused else {
        panic!("{refused:?}")
    };
    assert_eq!(
        (tag.as_str(), at.as_str(), why.as_str()),
        ("v9", sha('1').as_str(), "the notes were refused")
    );
    let said = refused.to_string();
    assert!(
        said.contains("the tag v9 was made") && said.contains("the release was not"),
        "{said}"
    );
    assert_eq!(forge.log().len(), 1, "only the tag");
    // The ask shows what is true now: the release against the tag that exists.
    let (shown, digest) = held.shown(id).unwrap();
    assert!(
        shown
            .rows
            .iter()
            .any(|r| r.label == "Makes the tag" && r.value.starts_with("No"))
    );
    forge.with(|s| s.fail_release = None);
    crate::support::held_acts::press(&held, &world, id, &digest).unwrap();
    assert!(matches!(
        crate::support::held_acts::press(&held, &world, id, &digest),
        Ok(Pressed::Done(_))
    ));
    assert_eq!(forge.log().len(), 2);
    assert!(forge.log()[1].starts_with("create_release acme/app v9"));
}
