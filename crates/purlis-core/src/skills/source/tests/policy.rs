// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The org's controls over skill sources (SK-11, #2774): a project whose shared settings hold a
//! `[skill_policy]` table installs and updates only from the sources and skills it allows, never
//! one it blocks, never a commit younger than its minimum age, and audits every install, update
//! and removal.

use super::*;

/// The plane of [`plane`], its shared settings holding `policy` after the schema line.
fn org_plane(policy: &str) -> tempfile::TempDir {
    let tmp = plane();
    std::fs::write(
        tmp.path().join("charter.toml"),
        format!("schema = 1\n\n{policy}"),
    )
    .unwrap();
    tmp
}

#[test]
fn an_unlisted_source_is_refused_with_its_reason_and_nothing_is_installed() {
    let (remote, _) = good_remote();
    let tmp = org_plane(
        "[skill_policy]\nmin_age_days = 0\nallow = [{ source = \"https://github.com/acme/skills\" }]\n",
    );
    let root = tmp.path();
    let source = remote.source();

    let refused = review(root, &source, &["deploy".to_owned()], &Level::project()).unwrap_err();
    assert!(
        refused.contains("not on this project's allow list"),
        "{refused}"
    );
    assert!(!root.join("skills/deploy").exists());
}

/// A source like [`good_remote`] whose one commit was made `days` ago.
fn remote_made_days_ago(days: u64) -> (Upstream, String) {
    let remote = Upstream::new();
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/notify/SKILL.md", "100644", NOTIFY),
    ]);
    let commit = commit_made(&remote, &tree, None, days);
    (remote, commit)
}

/// Commits `tree` on `main` after `parent`, its committer time `days` ago, and answers it.
fn commit_made(remote: &Upstream, tree: &str, parent: Option<&str>, days: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let at = now - days * 24 * 60 * 60;
    let parent = parent.map(|p| format!("parent {p}\n")).unwrap_or_default();
    let object = format!(
        "tree {tree}\n{parent}author Fixture <fixture@example.invalid> {at} +0000\n\
         committer Fixture <fixture@example.invalid> {at} +0000\n\nskills\n"
    );
    let out = remote
        .git(
            &["hash-object", "-t", "commit", "-w", "--stdin"],
            object.as_bytes(),
        )
        .out;
    let commit = String::from_utf8(out).unwrap().trim().to_owned();
    remote.git(&["update-ref", "refs/heads/main", &commit], b"");
    commit
}

fn allowing(url: &str, rest: &str) -> String {
    format!("[skill_policy]\nallow = [{{ source = \"{url}\" }}]\n{rest}")
}

#[test]
fn a_listed_source_old_enough_installs_and_is_pinned() {
    let (remote, commit) = remote_made_days_ago(8);
    let tmp = org_plane(&allowing(&remote.url(), ""));
    let root = tmp.path();
    let source = remote.source();
    let review = review(root, &source, &["deploy".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        cx_audited(cx, |cx| {
            install(cx, &source, &Level::project(), &review.accepted(), None)
        })
    })
    .0
    .unwrap();
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    assert_eq!(lock.skills[0].pin.as_str(), commit);
}

#[test]
fn a_blocked_skill_of_an_allowed_source_is_refused_and_a_block_wins() {
    let (remote, _) = remote_made_days_ago(30);
    let url = remote.url();
    let tmp = org_plane(&allowing(
        &url,
        &format!("block = [{{ source = \"{url}\", skill = \"notify\" }}]\n"),
    ));
    let root = tmp.path();
    // deploy brings notify along, which is blocked.
    let refused = review(
        root,
        &remote.source(),
        &["deploy".to_owned()],
        &Level::project(),
    )
    .unwrap_err();
    assert!(
        refused.contains("notify from") && refused.contains("block list"),
        "{refused}"
    );

    let tmp = org_plane(&format!(
        "[skill_policy]\nblock = [{{ source = \"{url}/\" }}]\n"
    ));
    let refused = review(
        tmp.path(),
        &remote.source(),
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap_err();
    assert!(refused.contains("block list of skill sources"), "{refused}");
}

#[test]
fn only_the_skills_an_allow_entry_names_are_allowed_from_its_source() {
    let (remote, _) = remote_made_days_ago(30);
    let url = remote.url();
    let tmp = org_plane(&format!(
        "[skill_policy]\nallow = [{{ source = \"{url}\", skill = \"notify\" }}]\n"
    ));
    let root = tmp.path();
    assert!(
        review(
            root,
            &remote.source(),
            &["notify".to_owned()],
            &Level::project()
        )
        .is_ok()
    );
    let refused = review(
        root,
        &remote.source(),
        &["deploy".to_owned()],
        &Level::project(),
    )
    .unwrap_err();
    assert!(
        refused.contains("deploy is not on this project's allow list"),
        "{refused}"
    );
}

#[test]
fn a_commit_younger_than_the_minimum_age_is_refused_at_review_and_at_install() {
    let (remote, _) = remote_made_days_ago(2);
    let url = remote.url();
    // Seven days, by default.
    let tmp = org_plane(&allowing(&url, ""));
    let root = tmp.path();
    let refused = review(
        root,
        &remote.source(),
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap_err();
    assert!(
        refused.contains("was committed 2 days ago")
            && refused.contains("at least 7 days old")
            && refused.contains("in 5 days"),
        "{refused}"
    );

    // Reviewed before the project set an age, and installed after: refused all the same.
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let review = review(root, &source, &["notify".to_owned()], &Level::project()).unwrap();
    std::fs::write(
        root.join("charter.toml"),
        format!("schema = 1\n\n{}", allowing(&url, "min_age_days = 3\n")),
    )
    .unwrap();
    let refused = on(root, |cx| {
        cx_audited(cx, |cx| {
            install(cx, &source, &Level::project(), &review.accepted(), None)
        })
    })
    .0
    .unwrap_err();
    assert!(
        refused.contains("at least 3 days old") && refused.contains("nothing was installed"),
        "{refused}"
    );
    assert!(!root.join("skills/notify").exists());
}

#[test]
fn an_exempt_commit_installs_young_and_an_age_of_zero_turns_the_age_off() {
    let (remote, commit) = remote_made_days_ago(0);
    let url = remote.url();
    for rest in [
        format!("min_age_exempt = [\"{commit}\"]\n"),
        "min_age_days = 0\n".to_owned(),
    ] {
        let tmp = org_plane(&allowing(&url, &rest));
        assert!(
            review(
                tmp.path(),
                &remote.source(),
                &["notify".to_owned()],
                &Level::project()
            )
            .is_ok(),
            "{rest}"
        );
    }
}

#[test]
fn a_policy_that_cannot_be_read_refuses_and_one_in_this_machines_file_is_never_applied() {
    let (remote, _) = remote_made_days_ago(30);
    let tmp = org_plane("[skill_policy]\nallow = \"everything\"\n");
    let refused = review(
        tmp.path(),
        &remote.source(),
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap_err();
    assert!(refused.contains("could not be read"), "{refused}");

    let tmp = plane();
    std::fs::write(
        tmp.path().join("charter.local.toml"),
        "[skill_policy]\nmin_age_days = 0\n",
    )
    .unwrap();
    let refused = review(
        tmp.path(),
        &remote.source(),
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap_err();
    assert!(refused.contains("never applied"), "{refused}");
}

/// Runs `change` with `cx`'s plane and lines, audited into `into`, as the window audits.
fn audited_into<T>(
    cx: &mut Ctx<'_>,
    into: &mut Vec<crate::skills::source::policy::Audited>,
    change: impl FnOnce(&mut Ctx<'_>) -> T,
) -> T {
    let mut audit = |one: &crate::skills::source::policy::Audited| {
        into.push(one.clone());
        Ok(())
    };
    let say = &mut *cx.say;
    change(&mut Ctx {
        plane: cx.plane,
        built_in: cx.built_in,
        by: By::Window,
        say,
        audit: Some(&mut audit),
    })
}

/// [`audited_into`], what was audited let go.
fn cx_audited<T>(cx: &mut Ctx<'_>, change: impl FnOnce(&mut Ctx<'_>) -> T) -> T {
    audited_into(cx, &mut Vec::new(), change)
}

#[test]
fn install_update_and_removal_are_each_audited_on_an_org_project() {
    let (remote, first) = remote_made_days_ago(20);
    let url = remote.url();
    let tmp = org_plane(&allowing(&url, ""));
    let root = tmp.path();
    let source = remote.source();
    let mut audited = Vec::new();

    let review = review(root, &source, &["notify".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        audited_into(cx, &mut audited, |cx| {
            install(cx, &source, &Level::project(), &review.accepted(), None)
        })
    })
    .0
    .unwrap();

    let newer_notify: &[u8] =
        b"---\nname: notify\ndescription: Tells the team.\n---\n\nPost to https://chat.example.org.\n";
    let tree = remote.tree(&[("skills/notify/SKILL.md", "100644", newer_notify)]);
    let second = commit_made(&remote, &tree, Some(&first), 10);
    let offered = update_review(root, &Level::project(), "notify").unwrap();
    on(root, |cx| {
        audited_into(cx, &mut audited, |cx| {
            update(
                cx,
                &Level::project(),
                "notify",
                &offered.accepted(),
                offered.grant.as_ref(),
            )
        })
    })
    .0
    .unwrap();

    on(root, |cx| {
        audited_into(cx, &mut audited, |cx| {
            crate::skills::verbs::delete(cx, &Level::project(), "notify")
        })
    })
    .0
    .unwrap();

    let said: Vec<(&str, serde_json::Value)> = audited
        .iter()
        .map(|one| {
            (
                one.kind(),
                one.body(crate::skills::source::policy::MadeAt::Window),
            )
        })
        .collect();
    let local = format!("local:{}", Origin::Git(source.remote.clone()).label());
    assert_eq!(
        said,
        [
            (
                "skill.install",
                serde_json::json!({"actor_kind": "human", "actor": "operator", "scope": "local-ui", "level": "project", "skills": ["notify"], "source": local, "pin": first})
            ),
            (
                "skill.update",
                serde_json::json!({"actor_kind": "human", "actor": "operator", "scope": "local-ui", "level": "project", "skills": ["notify"], "source": local, "pin": second})
            ),
            (
                "skill.remove",
                serde_json::json!({"actor_kind": "human", "actor": "operator", "scope": "local-ui", "level": "project", "skills": ["notify"], "source": local, "pin": second})
            ),
        ]
    );
}

#[test]
fn where_nothing_writes_the_audit_an_org_project_changes_no_installed_skill() {
    let (remote, _) = remote_made_days_ago(20);
    let tmp = org_plane(&allowing(&remote.url(), ""));
    let root = tmp.path();
    let source = remote.source();
    let review = review(root, &source, &["notify".to_owned()], &Level::project()).unwrap();
    let refused = on(root, |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    })
    .0
    .unwrap_err();
    assert!(
        refused.contains("nothing here can write the audit's log")
            && refused.contains("nothing was installed"),
        "{refused}"
    );
    assert!(!root.join("skills/notify").exists());

    on(root, |cx| {
        cx_audited(cx, |cx| {
            install(cx, &source, &Level::project(), &review.accepted(), None)
        })
    })
    .0
    .unwrap();
    let refused = on(root, |cx| {
        crate::skills::verbs::delete(cx, &Level::project(), "notify")
    })
    .0
    .unwrap_err();
    assert!(refused.contains("was not deleted"), "{refused}");
    assert!(root.join("skills/notify/SKILL.md").is_file());

    // A project without controls audits nothing, and needs nothing to.
    let tmp = plane();
    let review = super::review(
        tmp.path(),
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap();
    on(tmp.path(), |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    })
    .0
    .unwrap();
}

#[test]
fn a_newer_commit_younger_than_the_minimum_age_is_not_updated_to() {
    let (remote, first) = remote_made_days_ago(20);
    let url = remote.url();
    let tmp = org_plane(&allowing(&url, ""));
    let root = tmp.path();
    let source = remote.source();
    let review = review(root, &source, &["notify".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        cx_audited(cx, |cx| {
            install(cx, &source, &Level::project(), &review.accepted(), None)
        })
    })
    .0
    .unwrap();
    let newer: &[u8] = b"---\nname: notify\ndescription: Tells the team.\n---\n\nPost.\n";
    let tree = remote.tree(&[("skills/notify/SKILL.md", "100644", newer)]);
    commit_made(&remote, &tree, Some(&first), 1);
    let refused = update_review(root, &Level::project(), "notify").unwrap_err();
    assert!(
        refused.contains("was committed 1 day ago") && refused.contains("in 6 days"),
        "{refused}"
    );
}

#[test]
fn a_settings_file_that_does_not_parse_and_names_the_table_refuses() {
    let (remote, _) = remote_made_days_ago(30);
    let tmp = org_plane("[skill_policy\nmin_age_days = 0\n");
    let refused = review(
        tmp.path(),
        &remote.source(),
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap_err();
    assert!(refused.contains("could not be read"), "{refused}");
}

#[test]
fn an_age_of_one_day_is_said_in_the_singular() {
    let (remote, _) = remote_made_days_ago(0);
    let tmp = org_plane(&allowing(&remote.url(), "min_age_days = 1\n"));
    let refused = review(
        tmp.path(),
        &remote.source(),
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap_err();
    assert!(
        refused.contains("at least 1 day old") && refused.contains("in 1 day"),
        "{refused}"
    );
}

#[test]
fn a_deleted_installed_skill_comes_back_only_where_allowed_and_is_audited_as_an_install() {
    let (remote, commit) = remote_made_days_ago(20);
    let url = remote.url();
    let tmp = org_plane(&allowing(&url, ""));
    let root = tmp.path();
    let source = remote.source();
    let review = review(root, &source, &["notify".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        cx_audited(cx, |cx| {
            install(cx, &source, &Level::project(), &review.accepted(), None)
        })
    })
    .0
    .unwrap();
    on(root, |cx| {
        cx_audited(cx, |cx| {
            crate::skills::verbs::delete(cx, &Level::project(), "notify")
        })
    })
    .0
    .unwrap();
    let deleted = crate::skills::verbs::deleted(root).pop().unwrap();

    // Blocked since: it stays deleted.
    std::fs::write(
        root.join("charter.toml"),
        format!(
            "schema = 1\n\n{}",
            allowing(
                &url,
                &format!("block = [{{ source = \"{url}\", skill = \"notify\" }}]\n")
            )
        ),
    )
    .unwrap();
    let refused = on(root, |cx| {
        cx_audited(cx, |cx| crate::skills::verbs::restore(cx, &deleted, None))
    })
    .0
    .unwrap_err();
    assert!(
        refused.contains("block list") && refused.contains("was not restored"),
        "{refused}"
    );

    std::fs::write(
        root.join("charter.toml"),
        format!("schema = 1\n\n{}", allowing(&url, "")),
    )
    .unwrap();
    let mut audited = Vec::new();
    on(root, |cx| {
        audited_into(cx, &mut audited, |cx| {
            crate::skills::verbs::restore(cx, &deleted, None)
        })
    })
    .0
    .unwrap();
    let said: Vec<(&str, String)> = audited
        .iter()
        .map(|one| (one.kind(), one.pin.clone()))
        .collect();
    assert_eq!(said, [("skill.install", commit)]);
}

/// #6031: the Updates list says, before a review, that a newer commit is too young, in the
/// sentence the review refuses it with, and the moment it is old enough to take.
#[test]
fn updates_say_before_a_review_that_a_newer_commit_is_too_young_and_when_it_can_be_taken() {
    let (remote, first) = remote_made_days_ago(20);
    let tmp = org_plane(&allowing(&remote.url(), ""));
    let root = tmp.path();
    let source = remote.source();
    let review = review(root, &source, &["notify".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        cx_audited(cx, |cx| {
            install(cx, &source, &Level::project(), &review.accepted(), None)
        })
    })
    .0
    .unwrap();
    let newer: &[u8] = b"---\nname: notify\ndescription: Tells the team.\n---\n\nPost.\n";
    let tree = remote.tree(&[("skills/notify/SKILL.md", "100644", newer)]);
    commit_made(&remote, &tree, Some(&first), 1);

    let found = updates(root, None);
    let offer = found
        .offers
        .iter()
        .find(|one| one.name == "notify")
        .unwrap();
    let refused = offer.refused.as_ref().expect("too young");
    assert!(
        refused.why.contains("was committed 1 day ago") && refused.why.contains("in 6 days"),
        "{}",
        refused.why
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let until = refused.until.expect("a commit says when it was made");
    let six_days = 6 * 24 * 60 * 60;
    assert!(
        until > now + six_days - 120 && until <= now + six_days + 120,
        "{until}"
    );
}

/// #6031: a newer commit old enough, from a source the controls allow, is offered as ever.
#[test]
fn an_update_the_controls_allow_is_offered_unmarked() {
    let (remote, first) = remote_made_days_ago(30);
    let tmp = org_plane(&allowing(&remote.url(), ""));
    let root = tmp.path();
    let source = remote.source();
    let review = review(root, &source, &["notify".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        cx_audited(cx, |cx| {
            install(cx, &source, &Level::project(), &review.accepted(), None)
        })
    })
    .0
    .unwrap();
    let newer: &[u8] = b"---\nname: notify\ndescription: Tells the team.\n---\n\nPost.\n";
    let tree = remote.tree(&[("skills/notify/SKILL.md", "100644", newer)]);
    commit_made(&remote, &tree, Some(&first), 10);
    let found = updates(root, None);
    assert_eq!(found.offers.len(), 1);
    assert_eq!(found.offers[0].refused, None);
}

/// #6031: Sources says, before a review, whether the controls allow every skill of a source,
/// only some of them, or none, in the core's own sentence.
#[test]
fn a_source_says_whether_the_controls_allow_all_some_or_none_of_it() {
    let (remote, _) = remote_made_days_ago(30);
    let url = remote.url();
    let origin = Origin::Git(remote.source().remote);
    let of = |policy: &str| {
        let tmp = org_plane(policy);
        crate::skills::source::policy::of(tmp.path())
            .unwrap()
            .unwrap()
    };
    assert_eq!(of(&allowing(&url, "")).admits_source(&origin), Ok(()));
    let some = of(&format!(
        "[skill_policy]\nallow = [{{ source = \"{url}\", skill = \"notify\" }}]\n"
    ))
    .admits_source(&origin)
    .unwrap_err();
    assert!(
        some.only == ["notify"]
            && some.why.contains("has only notify on")
            && some.why.contains("allow list"),
        "{some:?}"
    );
    let none = of("[skill_policy]\nallow = [{ source = \"https://github.com/acme/skills\" }]\n")
        .admits_source(&origin)
        .unwrap_err();
    assert!(none.only.is_empty(), "{none:?}");
    assert!(
        none.why
            .contains("is not on this project's allow list of skill sources"),
        "{none:?}"
    );
    let blocked = of(&format!(
        "[skill_policy]\nblock = [{{ source = \"{url}\" }}]\n"
    ))
    .admits_source(&origin)
    .unwrap_err();
    assert!(
        blocked
            .why
            .contains("is on this project's block list of skill sources"),
        "{blocked:?}"
    );
}

/// #6031: a source's page marks each skill the controls would refuse, before its review, in the
/// sentence the review refuses it with.
#[test]
fn browsing_a_source_marks_each_skill_the_controls_refuse() {
    let (remote, _) = remote_made_days_ago(30);
    let url = remote.url();
    let tmp = org_plane(&format!(
        "[skill_policy]\nallow = [{{ source = \"{url}\" }}]\nblock = [{{ source = \"{url}\", skill = \"notify\" }}]\n"
    ));
    let browse = browse(tmp.path(), &remote.source()).unwrap();
    let refused = |name: &str| {
        browse
            .skills
            .iter()
            .find(|one| one.skill.name == name)
            .unwrap()
            .refused
            .clone()
    };
    assert_eq!(refused("deploy"), None);
    let notify = refused("notify").expect("blocked");
    assert!(
        notify.why.contains("notify from") && notify.why.contains("block list"),
        "{notify:?}"
    );
}

/// #6031: Settings shows the table as the team set it, and Sources one line of what it does.
#[test]
fn the_controls_are_shown_as_set_and_said_in_one_line() {
    let tmp = org_plane(
        "[skill_policy]\nallow = [\n  { source = \"https://github.com/acme/skills\" },\n  \
         { source = \"https://github.com/anthropics/skills\", skill = \"pdf\" },\n  \
         { extension = \"acme.registry\" },\n]\nblock = [{ source = \"https://github.com/acme/skills\", skill = \"yolo\" }]\n\
         min_age_exempt = [\"9f3c1d27a0b4e5f6a7b8c9d0e1f2a3b4c5d6e7f8\"]\n",
    );
    let policy = crate::skills::source::policy::of(tmp.path())
        .unwrap()
        .unwrap();
    let shown = policy.shown();
    assert_eq!(
        shown.allow,
        Some(vec![
            "https://github.com/acme/skills".to_owned(),
            "pdf from https://github.com/anthropics/skills".to_owned(),
            "the extension acme.registry".to_owned(),
        ])
    );
    assert_eq!(shown.block, ["yolo from https://github.com/acme/skills"]);
    assert_eq!(shown.min_age_days, 7);
    assert_eq!(shown.exempt, ["9f3c1d27a0b4e5f6a7b8c9d0e1f2a3b4c5d6e7f8"]);
    assert_eq!(
        policy.said(),
        "This project's controls apply: 3 sources allowed, 1 skill blocked, commits at least 7 days old."
    );
    let open =
        crate::skills::source::policy::of(org_plane("[skill_policy]\nmin_age_days = 0\n").path())
            .unwrap()
            .unwrap();
    assert_eq!(open.shown().allow, None);
    assert_eq!(
        open.said(),
        "This project's controls apply: every source not blocked is allowed, nothing blocked, no minimum age."
    );
}
