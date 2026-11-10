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
