// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A tag a chat asks for is made by the app, on the person's two presses, at the commit the ask
//! showed** (ADR 0128 D8.2). The ask says whether that commit is on the default branch, in words;
//! a tag that exists is never moved; and nothing is done before the second press.

use purlis_core::heldact::{Act, Asked, HeldActs, NotDone, Pressed, Refused, Requested};

use crate::support::held_acts::{
    Fake, NoTrees, Rec, Second, base, repo, sha, tag, the_person, world,
};

fn tag_request(name: &str, rev: &str) -> Requested {
    Requested::Tag {
        repo: repo("acme/app"),
        name: tag(name),
        rev: rev.to_owned(),
    }
}

fn forge() -> Fake {
    let forge = Fake::github();
    forge.with(|s| {
        s.revs.insert("main".into(), sha('1'));
        s.revs.insert("feature".into(), sha('2'));
        s.merged.insert(sha('1'));
    });
    forge
}

#[test]
fn the_ask_shows_the_commit_and_says_in_words_that_nobody_merged_it() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);

    let Asked::Held { id, new: true } =
        held.ask(&world, 1, true, &tag_request("v1", "feature"), 10)
    else {
        panic!("the person is asked");
    };
    let (shown, _) = held.shown(id).unwrap();
    let rows: Vec<(String, String)> = shown
        .rows
        .iter()
        .map(|r| (r.label.clone(), r.value.clone()))
        .collect();
    assert!(rows.contains(&("Tag".into(), "v1".into())), "{rows:?}");
    assert!(
        rows.contains(&("Commit".into(), sha('2').as_str().into())),
        "{rows:?}"
    );
    let on = rows
        .iter()
        .find(|(l, _)| l == "On the default branch")
        .unwrap();
    assert!(
        on.1.starts_with("No") && on.1.contains("Nobody merged it there"),
        "{on:?}"
    );

    // A commit on the default branch says so.
    let Asked::Held { id: merged, .. } = held.ask(&world, 1, true, &tag_request("v0", "main"), 11)
    else {
        panic!("asked");
    };
    let (shown, _) = held.shown(merged).unwrap();
    let on = shown
        .rows
        .iter()
        .find(|r| r.label == "On the default branch")
        .unwrap();
    assert_eq!(on.value, "Yes, it is on main");
}

#[test]
fn nothing_is_made_by_an_ask_nor_by_the_first_press_and_the_second_makes_what_was_shown() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &tag_request("v1", "feature"), 10)
    else {
        panic!();
    };
    assert_eq!(forge.log(), Vec::<String>::new(), "an ask performs nothing");

    let (shown, digest) = held.shown(id).unwrap();
    let Ok(Pressed::AsksOnceMore(again)) = held.press(&world, id, &digest, the_person()) else {
        panic!("the first press asks once more");
    };
    assert_eq!(*again, shown, "it asks once more of exactly what was shown");
    assert_eq!(
        forge.log(),
        Vec::<String>::new(),
        "one press performs nothing"
    );

    let Ok(Pressed::Done(done)) = held.press(&world, id, &digest, the_person()) else {
        panic!("the second press performs");
    };
    assert_eq!(
        done.act,
        Act::Tag {
            repo: repo("acme/app"),
            name: tag("v1"),
            target: sha('2')
        }
    );
    // What was done is what the row said: this tag, at this commit, in this repo.
    assert_eq!(
        forge.log(),
        vec![format!("create_tag acme/app v1 {}", sha('2').as_str())]
    );
    // A third press finds nothing: it is done once.
    assert_eq!(
        held.press(&world, id, &digest, the_person()).unwrap_err(),
        NotDone::NotHeld
    );
    assert_eq!(forge.log().len(), 1);
}

#[test]
fn a_press_of_a_row_that_is_not_what_the_ask_shows_does_nothing() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &tag_request("v1", "feature"), 10)
    else {
        panic!();
    };
    assert_eq!(
        held.press(&world, id, "not-the-digest", the_person())
            .unwrap_err(),
        NotDone::NotWhatYouSaw
    );
    assert!(forge.log().is_empty());
}

#[test]
fn an_existing_tag_is_never_moved_neither_asked_nor_pressed() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    forge.with(|s| {
        s.tags.insert("v1".into(), sha('1'));
    });
    let asked = held.ask(&world, 1, true, &tag_request("v1", "feature"), 10);
    assert_eq!(
        asked,
        Asked::Refused(Refused::TagExists {
            name: tag("v1"),
            at: sha('1')
        })
    );
    assert!(held.every().is_empty(), "nobody is asked to move a tag");
}

#[test]
fn a_tag_made_after_the_ask_refuses_the_press_and_nothing_is_made() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &tag_request("v1", "feature"), 10)
    else {
        panic!();
    };
    let (_, digest) = held.shown(id).unwrap();
    assert!(matches!(
        held.press(&world, id, &digest, the_person()),
        Ok(Pressed::AsksOnceMore(_))
    ));
    // Someone tags v1 on the forge between the presses.
    forge.with(|s| {
        s.tags.insert("v1".into(), sha('3'));
    });
    let refused = held.press(&world, id, &digest, the_person()).unwrap_err();
    assert_eq!(
        refused,
        NotDone::Refused(Refused::TagExists {
            name: tag("v1"),
            at: sha('3')
        })
    );
    assert!(
        forge.log().is_empty(),
        "the other tag was not touched: {:?}",
        forge.log()
    );
    assert!(held.every().is_empty(), "the ask is gone");
}

#[test]
fn a_fact_that_changes_between_the_presses_asks_again_instead_of_performing() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &tag_request("v1", "feature"), 10)
    else {
        panic!();
    };
    let (_, digest) = held.shown(id).unwrap();
    assert!(matches!(
        held.press(&world, id, &digest, the_person()),
        Ok(Pressed::AsksOnceMore(_))
    ));
    // The commit gets merged onto the default branch: the row would now read otherwise.
    forge.with(|s| {
        s.merged.insert(sha('2'));
    });
    assert_eq!(
        held.press(&world, id, &digest, the_person()).unwrap_err(),
        NotDone::ShowsOtherwise
    );
    assert!(forge.log().is_empty());
    // The ask shows the new fact, and takes its own two presses.
    let (shown, fresh) = held.shown(id).unwrap();
    assert!(shown.rows.iter().any(|r| r.value == "Yes, it is on main"));
    assert!(matches!(
        held.press(&world, id, &fresh, the_person()),
        Ok(Pressed::AsksOnceMore(_))
    ));
    assert!(forge.log().is_empty());
    assert!(matches!(
        held.press(&world, id, &fresh, the_person()),
        Ok(Pressed::Done(_))
    ));
    assert_eq!(forge.log().len(), 1);
}

#[test]
fn a_forge_that_refuses_leaves_the_ask_waiting() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &tag_request("v1", "feature"), 10)
    else {
        panic!();
    };
    let (_, digest) = held.shown(id).unwrap();
    held.press(&world, id, &digest, the_person()).unwrap();
    forge.with(|s| s.refuses = Some("the forge said no".into()));
    assert_eq!(
        held.press(&world, id, &digest, the_person()).unwrap_err(),
        NotDone::Forge("the forge said no".into())
    );
    assert_eq!(held.every().len(), 1, "the ask waits again, as it was");
}

#[test]
fn a_tag_of_a_repo_the_person_did_not_record_is_not_asked() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let other = Requested::Tag {
        repo: repo("evil/other"),
        name: tag("v1"),
        rev: "main".into(),
    };
    assert_eq!(
        held.ask(&world, 1, true, &other, 10),
        Asked::Refused(Refused::NotRecorded(repo("evil/other")))
    );
}

#[test]
fn a_chat_is_asked_for_a_tag_once_however_often_it_tries() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let first = held.ask(&world, 1, true, &tag_request("v1", "feature"), 10);
    let again = held.ask(&world, 1, true, &tag_request("v1", "feature"), 20);
    let (Asked::Held { id: a, new: true }, Asked::Held { id: b, new: false }) = (first, again)
    else {
        panic!();
    };
    assert_eq!(a, b);
    assert_eq!(held.every().len(), 1);
    assert_eq!(held.every()[0].since, 10);
    let _ = base("main");
}
