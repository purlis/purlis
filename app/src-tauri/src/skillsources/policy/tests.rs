// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::skillsources::SourceRemote;

fn git(url: &str) -> SourceOrigin {
    SourceOrigin::Git {
        remote: SourceRemote {
            url: url.to_owned(),
            branch: None,
            label: url.to_owned(),
        },
        label: url.to_owned(),
    }
}

fn project(policy: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("charter.toml"),
        format!("schema = 1\n\n{policy}"),
    )
    .unwrap();
    tmp
}

#[test]
fn settings_shows_the_controls_as_set_and_sources_what_they_do_to_each_source() {
    // #6031: the table read only, where it is set, and each source the window lists with the
    // core's sentence.
    let tmp = project(
        "[skill_policy]\nallow = [\n  { source = \"https://github.com/acme/skills\" },\n  \
         { source = \"https://github.com/anthropics/skills\", skill = \"pdf\" },\n]\n\
         min_age_days = 3\n",
    );
    let shown = policy_shown(
        tmp.path(),
        vec![
            git("https://github.com/acme/skills"),
            git("https://github.com/anthropics/skills"),
            git("https://github.com/obra/superpowers"),
            SourceOrigin::Extension {
                id: "acme.registry".to_owned(),
                label: "acme.registry".to_owned(),
            },
        ],
    );
    assert_eq!(shown.file, "charter.toml");
    let SkillPolicyState::Set {
        allow,
        block,
        min_age_days,
        said,
        ..
    } = &shown.state
    else {
        panic!("{:?}", shown.state);
    };
    assert_eq!(
        allow.as_deref(),
        Some(
            &[
                "https://github.com/acme/skills".to_owned(),
                "pdf from https://github.com/anthropics/skills".to_owned()
            ][..]
        )
    );
    assert!(block.is_empty());
    assert_eq!(*min_age_days, 3);
    assert!(said.contains("2 sources allowed"), "{said}");
    let verdicts: Vec<(Option<&str>, &[String])> = shown
        .sources
        .iter()
        .map(|one| (one.refused.as_deref(), one.only.as_slice()))
        .collect();
    assert_eq!(verdicts[0], (None, &[][..]));
    assert_eq!(verdicts[1].1, ["pdf"]);
    assert!(verdicts[1].0.unwrap().contains("has only pdf on"));
    assert!(
        verdicts[2]
            .0
            .unwrap()
            .contains("is not on this project's allow list")
    );
    assert!(
        verdicts[3]
            .0
            .unwrap()
            .contains("is not on this project's allow list")
    );
}

#[test]
fn a_project_without_controls_refuses_nothing_and_one_that_cannot_be_read_refuses_every_source() {
    let none = project("");
    let shown = policy_shown(
        none.path(),
        vec![git("https://github.com/obra/superpowers")],
    );
    assert_eq!(shown.state, SkillPolicyState::None);
    assert_eq!(shown.sources[0].refused, None);

    let broken = project("[skill_policy]\nallow = [{ skill = \"pdf\" }]\n");
    let shown = policy_shown(
        broken.path(),
        vec![git("https://github.com/obra/superpowers")],
    );
    let SkillPolicyState::Refused { why } = &shown.state else {
        panic!("{:?}", shown.state);
    };
    assert!(why.contains("could not be read"), "{why}");
    assert_eq!(shown.sources[0].refused.as_deref(), Some(why.as_str()));
}

fn event(at_ms: u64, kind: &str, body: serde_json::Value) -> Event {
    Event {
        v: 1,
        device_id: "device".to_owned(),
        seq: at_ms,
        ulid: ulid::Ulid::from_parts(at_ms, 7).to_string(),
        chat: None,
        run: None,
        parent_run: None,
        kind: kind.to_owned(),
        body,
    }
}

fn audited(skills: &[&str], scope: &str) -> serde_json::Value {
    serde_json::json!({
        "actor_kind": "human",
        "actor": "operator",
        "scope": scope,
        "level": "project",
        "skills": skills,
        "source": "https://github.com/acme/skills",
        "pin": "4be9c21",
    })
}

#[test]
fn settings_lists_the_recorded_skill_acts_newest_first_and_nothing_else() {
    // #6031, SK-11: each install, update and removal the audit wrote, where it was made, and no
    // other event of the log.
    let tmp = tempfile::tempdir().unwrap();
    let events = [
        event(
            1_800_000_000_000,
            policy::INSTALL_KIND,
            audited(&["release-train", "notify"], "local-ui"),
        ),
        event(
            1_800_000_100_000,
            "trust.grant",
            serde_json::json!({ "what": "a grant" }),
        ),
        event(
            1_800_000_200_000,
            policy::REMOVE_KIND,
            audited(&["lint-fix"], "terminal"),
        ),
    ];
    let text: String = events
        .iter()
        .map(|one| format!("{}\n", serde_json::to_string(one).unwrap()))
        .collect();
    std::fs::write(tmp.path().join(eventlog::FILE), text).unwrap();

    let shown = acts_in(tmp.path()).unwrap();
    assert!(!shown.more);
    let seen: Vec<(SkillActKind, &[String], SkillActBy)> = shown
        .acts
        .iter()
        .map(|one| (one.act, one.skills.as_slice(), one.by))
        .collect();
    assert_eq!(
        seen,
        [
            (
                SkillActKind::Remove,
                &["lint-fix".to_owned()][..],
                SkillActBy::Terminal
            ),
            (
                SkillActKind::Install,
                &["release-train".to_owned(), "notify".to_owned()][..],
                SkillActBy::Window
            ),
        ]
    );
    assert_eq!(shown.acts[1].source, "https://github.com/acme/skills");
    assert_eq!(shown.acts[1].pin, "4be9c21");
    assert_eq!(shown.acts[1].level, "project");
    assert!((shown.acts[1].at - 1_800_000_000.0).abs() < 1.0);
}

#[test]
fn at_most_the_newest_fifty_acts_are_listed_and_more_says_there_are_older() {
    let tmp = tempfile::tempdir().unwrap();
    let text: String = (0..u64::try_from(MOST_ACTS + 3).unwrap())
        .map(|at| {
            let one = event(
                1_800_000_000_000 + at * 1000,
                policy::UPDATE_KIND,
                audited(&["notify"], "local-ui"),
            );
            format!("{}\n", serde_json::to_string(&one).unwrap())
        })
        .collect();
    std::fs::write(tmp.path().join(eventlog::FILE), text).unwrap();
    let shown = acts_in(tmp.path()).unwrap();
    assert_eq!(shown.acts.len(), MOST_ACTS);
    assert!(shown.more);
}
