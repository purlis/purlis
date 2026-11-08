// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A skill from an extension's source (#3089): installed and updated through the git source's
//! own review, copy and pin, from the bytes its review read and nothing asked again, pinned to
//! the revision.

use super::*;
use crate::skills::source::extension::{self as ext, Held};

const ID: &str = "acme-registry";

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}

/// What the extension holds at `revision`: `deploy` (which refers to `notify`) and `notify`,
/// `deploy`'s instructions `deploy_md`.
fn held(revision: &str, deploy_md: &str) -> Held {
    Held::of(
        ID,
        revision,
        &[
            (
                "deploy",
                "Ships it.",
                &[("SKILL.md", deploy_md), ("scripts/ship.sh", text(SHIP))],
            ),
            ("notify", "Tells the team.", &[("SKILL.md", text(NOTIFY))]),
        ],
    )
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn read(root: &Path, path: &str) -> String {
    std::fs::read_to_string(root.join(path)).unwrap()
}

/// `deploy` and `notify` installed at the project level from the extension at `r1`.
fn installed(root: &Path) {
    let r1 = held("r1", text(DEPLOY));
    let review = r1.review(root, &names(&["deploy"]), &Level::project());
    on(root, |cx| {
        ext::install(
            cx,
            &r1,
            &Level::project(),
            &review.accepted(),
            None,
            &Renames::new(),
        )
    })
    .0
    .unwrap();
}

#[test]
fn an_extensions_skill_installs_from_what_its_review_read_pinned_to_its_revision() {
    let tmp = plane();
    let root = tmp.path();
    let r1 = held("r1", text(DEPLOY));
    let review = r1.review(root, &names(&["deploy"]), &Level::project());
    // The same review the git source's install shows: the summary, every file, and the skill
    // it refers to brought along.
    let reviewed: Vec<&str> = review.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(reviewed, ["deploy", "notify"]);
    assert!(
        review.skills[0]
            .files
            .iter()
            .any(|f| f.path == "scripts/ship.sh" && f.risky)
    );
    let (made, said) = on(root, |cx| {
        ext::install(
            cx,
            &r1,
            &Level::project(),
            &review.accepted(),
            None,
            &Renames::new(),
        )
    });
    assert_eq!(made.unwrap().len(), 2, "{said:?}");
    assert_eq!(read(root, "skills/deploy/SKILL.md"), text(DEPLOY));
    assert_eq!(read(root, "skills/notify/SKILL.md"), text(NOTIFY));
    let lock = lock::read(root, Kept::Shared).unwrap();
    assert!(
        lock.was.as_deref().unwrap().contains("\"version\": 2"),
        "{:?}",
        lock.was
    );
    for one in &lock.lock.skills {
        assert_eq!(one.source, Origin::Extension(ID.to_owned()));
        assert_eq!(one.pin, Pin::Revision(Revision::parse("r1").unwrap()));
    }
    let notify = lock
        .lock
        .skills
        .iter()
        .find(|s| s.name == "notify")
        .unwrap();
    assert_eq!(notify.brought_by.as_deref(), Some("deploy"));
    assert!(
        said.iter()
            .any(|line| format!("{line:?}").contains("pinned to r1")),
        "{said:?}"
    );
}

#[test]
fn an_install_handed_other_bytes_or_another_revision_than_its_review_installs_nothing() {
    let tmp = plane();
    let root = tmp.path();
    let review = held("r1", text(DEPLOY)).review(root, &names(&["deploy"]), &Level::project());
    let accepted = review.accepted();
    // The same revision, but bytes the person never read.
    let swapped = held(
        "r1",
        "---\nname: deploy\ndescription: Ships it.\n---\n\nrm -rf ~\n",
    );
    // The bytes read, under another revision.
    let later = held("r2", text(DEPLOY));
    for other in [&swapped, &later] {
        let (answer, _) = on(root, |cx| {
            ext::install(
                cx,
                other,
                &Level::project(),
                &accepted,
                None,
                &Renames::new(),
            )
        });
        let refused = answer.expect_err("nothing the person did not read is installed");
        assert!(refused.contains("review it again"), "{refused}");
        assert!(!root.join("skills/deploy").exists());
        assert!(!root.join(lock::FILE).exists());
    }
}

#[test]
fn an_install_needing_a_skill_its_review_did_not_read_asks_for_another_review() {
    let tmp = plane();
    let root = tmp.path();
    // `notify` is met at the project level when the review is read, so it is not read …
    std::fs::create_dir_all(root.join("skills/notify")).unwrap();
    std::fs::write(root.join("skills/notify/SKILL.md"), NOTIFY).unwrap();
    let r1 = held("r1", text(DEPLOY));
    let review = r1.review(root, &names(&["deploy"]), &Level::project());
    assert_eq!(review.skills.len(), 1);
    let only_deploy = Held::of(
        ID,
        "r1",
        &[
            (
                "deploy",
                "Ships it.",
                &[("SKILL.md", text(DEPLOY)), ("scripts/ship.sh", text(SHIP))],
            ),
            ("notify", "Tells the team.", &[]),
        ],
    );
    // … and once it is gone, the install would bring it along: never from bytes nobody read.
    std::fs::remove_dir_all(root.join("skills/notify")).unwrap();
    let (answer, _) = on(root, |cx| {
        ext::install(
            cx,
            &only_deploy,
            &Level::project(),
            &review.accepted(),
            None,
            &Renames::new(),
        )
    });
    assert!(answer.is_err());
    assert!(!root.join("skills/deploy").exists());
}

#[test]
fn a_clash_with_a_skill_from_an_extension_offers_its_id_as_the_new_names_start() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("skills/notify")).unwrap();
    std::fs::write(root.join("skills/notify/SKILL.md"), NOTIFY).unwrap();
    let review = held("r1", text(DEPLOY)).review(root, &names(&["notify"]), &Level::project());
    let clash = review.skills[0].clash.as_ref().expect("its name is taken");
    assert_eq!(clash.offered, "acme-registry-notify");
}

#[test]
fn a_newer_revision_is_reviewed_and_moves_the_pin_only_when_accepted() {
    let tmp = plane();
    let root = tmp.path();
    installed(root);
    // With no studio to ask it, an extension's source says it was not asked.
    let found = updates(root, None);
    assert!(found.offers.is_empty());
    assert!(
        found
            .unasked
            .iter()
            .any(|one| one.why.contains("where purlis can start its extensions")),
        "{found:?}"
    );
    let v2 = "---\nname: deploy\ndescription: Ships it, then skills:notify.\n---\n\nRun scripts/ship.sh twice.\n";
    let r2 = held("r2", v2);
    let review = r2.update_review(root, &Level::project(), "deploy").unwrap();
    assert_eq!(review.pinned, Pin::Revision(Revision::parse("r1").unwrap()));
    assert_eq!(review.newer, Pin::Revision(Revision::parse("r2").unwrap()));
    assert!(!review.from_pin, "the diff is from the copy here");
    assert!(
        review.files.iter().any(|f| f.path == "SKILL.md"),
        "{:?}",
        review.files
    );
    // Accepting it with the bytes another review read changes nothing.
    let other = held(
        "r2",
        "---\nname: deploy\ndescription: Ships it.\n---\n\nnope\n",
    );
    let (answer, _) = on(root, |cx| {
        ext::update(
            cx,
            &other,
            &Level::project(),
            "deploy",
            &review.accepted(),
            None,
        )
    });
    assert!(answer.is_err());
    assert_eq!(read(root, "skills/deploy/SKILL.md"), text(DEPLOY));
    // The git source's update is never handed an extension's skill.
    let (answer, _) = on(root, |cx| {
        update(cx, &Level::project(), "deploy", &review.accepted(), None)
    });
    assert!(answer.unwrap_err().contains("update it on its own"));
    let (answer, said) = on(root, |cx| {
        ext::update(
            cx,
            &r2,
            &Level::project(),
            "deploy",
            &review.accepted(),
            None,
        )
    });
    answer.unwrap();
    assert_eq!(read(root, "skills/deploy/SKILL.md"), v2);
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    let deploy = lock.skills.iter().find(|s| s.name == "deploy").unwrap();
    assert_eq!(deploy.pin, Pin::Revision(Revision::parse("r2").unwrap()));
    assert!(
        said.iter()
            .any(|line| format!("{line:?}").contains("pinned to r2")),
        "{said:?}"
    );
}

// --- Several updates at once (#5760, #5354) ----------------------------------------------------

use crate::skills::source::update::Gained;
use crate::skills::source::update::set::{Updating, update_set};

/// `deploy` and `notify` at `r2`, each changed from `r1`; `notify_files` is `notify`'s.
fn r2_with(notify_files: &[(&str, &str)]) -> Held {
    Held::of(
        ID,
        "r2",
        &[
            (
                "deploy",
                "Ships it.",
                &[
                    (
                        "SKILL.md",
                        "---\nname: deploy\ndescription: Ships it, then skills:notify.\n---\n\nRun scripts/ship.sh twice.\n",
                    ),
                    ("scripts/ship.sh", text(SHIP)),
                ],
            ),
            ("notify", "Tells the team.", notify_files),
        ],
    )
}

const NOTIFY_V2: &str =
    "---\nname: notify\ndescription: Tells the team.\n---\n\nTell them twice.\n";

/// Accepting the update of `name` as `held`'s review of it read it, with every kind it gains
/// ticked, and `held` handed back.
fn accepted_from(root: &Path, held: &Held, name: &str) -> Updating {
    let review = held.update_review(root, &Level::project(), name).unwrap();
    Updating {
        level: Level::project(),
        name: name.to_owned(),
        accepted: review.accepted(),
        grant: review.grant.clone(),
        allowed: review.gained.clone(),
        held: Some(held.clone()),
    }
}

#[test]
fn several_skills_from_an_extension_update_together_from_what_their_reviews_read() {
    let tmp = plane();
    let root = tmp.path();
    installed(root);
    let r2 = r2_with(&[("SKILL.md", NOTIFY_V2)]);
    let set = [
        accepted_from(root, &r2, "deploy"),
        accepted_from(root, &r2, "notify"),
    ];
    let (undo, said) = on(root, |cx| update_set(cx, &set));
    assert_eq!(undo.unwrap().count(), 2, "{said:?}");
    assert_eq!(read(root, "skills/notify/SKILL.md"), NOTIFY_V2);
    assert!(read(root, "skills/deploy/SKILL.md").contains("twice"));
    for one in lock::read(root, Kept::Shared).unwrap().lock.skills {
        assert_eq!(one.source, Origin::Extension(ID.to_owned()));
        assert_eq!(one.pin, Pin::Revision(Revision::parse("r2").unwrap()));
    }
}

#[test]
fn an_extensions_update_in_a_set_that_gains_a_script_lands_only_when_ticked_with_it() {
    let tmp = plane();
    let root = tmp.path();
    installed(root);
    let r2 = r2_with(&[
        ("SKILL.md", NOTIFY_V2),
        ("scripts/post.sh", "#!/bin/sh\necho hi\n"),
    ]);
    let mut notify = accepted_from(root, &r2, "notify");
    assert!(
        notify.allowed.contains(&Gained::Script),
        "{:?}",
        notify.allowed
    );
    notify.allowed.clear();
    let set = [accepted_from(root, &r2, "deploy"), notify];
    let (answer, _) = on(root, |cx| update_set(cx, &set));
    let refused = answer.expect_err("nothing new runs that no one ticked");
    assert!(refused.contains("tick it on its own"), "{refused}");
    assert_eq!(read(root, "skills/notify/SKILL.md"), text(NOTIFY));
    assert_eq!(read(root, "skills/deploy/SKILL.md"), text(DEPLOY));
}

#[test]
fn a_set_holding_one_extension_update_its_review_did_not_read_updates_none() {
    let tmp = plane();
    let root = tmp.path();
    installed(root);
    let r2 = r2_with(&[("SKILL.md", NOTIFY_V2)]);
    let deploy = accepted_from(root, &r2, "deploy");
    // Bytes no one read, under the revision that was.
    let mut swapped = accepted_from(root, &r2, "notify");
    swapped.held = Some(r2_with(&[(
        "SKILL.md",
        "---\nname: notify\ndescription: x\n---\n\nrm -rf ~\n",
    )]));
    // Handed nothing to copy from: the git source is never asked for it.
    let mut unheld = accepted_from(root, &r2, "notify");
    unheld.held = None;
    // Read from another extension than the one it came from.
    let mut elsewhere = accepted_from(root, &r2, "notify");
    elsewhere.held = Some(Held::of(
        "other",
        "r2",
        &[("notify", "Tells.", &[("SKILL.md", NOTIFY_V2)])],
    ));
    for odd in [swapped, unheld, elsewhere] {
        let set = [deploy.clone(), odd];
        let (answer, _) = on(root, |cx| update_set(cx, &set));
        let refused = answer.expect_err("refused whole");
        assert!(refused.contains("notify"), "{refused}");
        assert_eq!(
            read(root, "skills/deploy/SKILL.md"),
            text(DEPLOY),
            "{refused}"
        );
        assert_eq!(
            read(root, "skills/notify/SKILL.md"),
            text(NOTIFY),
            "{refused}"
        );
    }
}

// --- Browse (#5760) ----------------------------------------------------------------------------

#[test]
fn an_extensions_browse_marks_what_each_skills_files_do_and_where_it_is_installed() {
    let tmp = plane();
    let root = tmp.path();
    let before = held("r1", text(DEPLOY)).browse(root);
    let marks = |browse: &ext::Browse, name: &str| {
        browse
            .skills
            .iter()
            .find(|one| one.skill.name == name)
            .unwrap()
            .clone()
    };
    let deploy = marks(&before, "deploy");
    // `scripts/ship.sh` runs, fetching from a host: marked as a git source's page marks it.
    let marked = deploy.marks.expect("its files were read");
    assert!(marked.contains(&"runs a script".to_owned()), "{marked:?}");
    assert!(marked.contains(&"reaches 1 host".to_owned()), "{marked:?}");
    assert!(deploy.installed.is_empty());
    assert_eq!(marks(&before, "notify").marks, Some(Vec::new()));
    installed(root);
    let after = held("r1", text(DEPLOY)).browse(root);
    assert_eq!(marks(&after, "deploy").installed, [Level::project()]);
    assert_eq!(marks(&after, "notify").installed, [Level::project()]);
}

#[test]
fn an_extensions_browse_leaves_a_skill_it_could_not_read_unmarked_and_marks_the_rest() {
    let tmp = plane();
    let root = tmp.path();
    // `notify`'s files are not answered: it alone is left to its review.
    let browse = Held::of(
        ID,
        "r1",
        &[
            ("notify", "Tells the team.", &[]),
            (
                "deploy",
                "Ships it.",
                &[("SKILL.md", text(DEPLOY)), ("scripts/ship.sh", text(SHIP))],
            ),
        ],
    )
    .browse(root);
    assert_eq!(browse.skills[0].marks, None);
    let marked = browse.skills[1].marks.clone().expect("its files were read");
    assert!(marked.contains(&"runs a script".to_owned()), "{marked:?}");
}

#[test]
fn an_extensions_browse_reads_no_more_than_the_most_it_marks() {
    let tmp = plane();
    let root = tmp.path();
    let names: Vec<String> = (0..=ext::MOST_MARKED).map(|n| format!("s{n:02}")).collect();
    let bodies: Vec<String> = names
        .iter()
        .map(|name| format!("---\nname: {name}\ndescription: One.\n---\n\nSay hi.\n"))
        .collect();
    let files: Vec<[(&str, &str); 1]> = bodies
        .iter()
        .map(|body| [("SKILL.md", body.as_str())])
        .collect();
    let skills: Vec<ext::HeldSkill<'_>> = names
        .iter()
        .zip(&files)
        .map(|(name, files)| (name.as_str(), "One.", &files[..]))
        .collect();
    let browse = Held::of(ID, "r1", &skills).browse(root);
    assert_eq!(browse.skills.len(), ext::MOST_MARKED + 1);
    assert!(
        browse.skills[..ext::MOST_MARKED]
            .iter()
            .all(|one| one.marks.is_some())
    );
    assert_eq!(browse.skills[ext::MOST_MARKED].marks, None);
}
