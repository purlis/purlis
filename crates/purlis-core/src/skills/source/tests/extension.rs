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
    let notify = lock.lock.skills.iter().find(|s| s.name == "notify").unwrap();
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
    let swapped = held("r1", "---\nname: deploy\ndescription: Ships it.\n---\n\nrm -rf ~\n");
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
    let review = r2
        .update_review(root, &Level::project(), "deploy")
        .unwrap();
    assert_eq!(review.pinned, Pin::Revision(Revision::parse("r1").unwrap()));
    assert_eq!(review.newer, Pin::Revision(Revision::parse("r2").unwrap()));
    assert!(!review.from_pin, "the diff is from the copy here");
    assert!(
        review.files.iter().any(|f| f.path == "SKILL.md"),
        "{:?}",
        review.files
    );
    // Accepting it with the bytes another review read changes nothing.
    let other = held("r2", "---\nname: deploy\ndescription: Ships it.\n---\n\nnope\n");
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
