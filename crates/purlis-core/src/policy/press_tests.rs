// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The loosening press** (M75-15, #7024): what the window shows at the press, against the policy
//! in force, and what Accept does. The accepted copy and the floor are handed in as a standing
//! built by hand, except where a test accepts: that one runs on a sealed store of its own.

use std::path::{Path, PathBuf};

use super::force::tests::{
    HEADER, Machine, STRICT, accepted, commit, floor, forbid_merge, git, holding, host, machine,
    merge, plane, sources, version,
};
use super::*;
use crate::policy::decide::Outcome;
use crate::secrets::answers::{Deny, Entry, State};

const WHEN_ATTENDED: &str = " when { context.attended }";
const WHEN_UNATTENDED: &str = " when { !context.attended }";
const OPEN: &str = "[policy]\ntemplate = \"open\"\n";

/// A test plays the person's press.
#[allow(clippy::disallowed_methods, reason = "a test plays the person's press")]
fn the_persons_press() -> answers::Press {
    answers::Press::of_the_persons_press()
}

/// The press the project at `root` raises with `standing`, `HEAD` read as the host reads it.
fn press_of(root: &Path, standing: &Standing) -> Option<Press> {
    of_head(root, &sources(root), standing, &Version::at_head(root))
}

fn rule<'a>(press: &'a Press, id: &str) -> &'a RuleChange {
    press
        .diff
        .rules
        .iter()
        .find(|one| one.id == id)
        .unwrap_or_else(|| panic!("no row for {id}: {:?}", press.diff.rules))
}

/// A permit of `id` for the merge action.
fn permit_merge(id: &str) -> String {
    format!("@id(\"{id}\")\npermit (principal, action == Action::\"merge\", resource);\n")
}

/// A plane whose accepted copy holds the ban on a merge and whose `HEAD` has dropped it: the
/// press the commit raises.
fn dropped_the_ban(m: &Machine) -> (PathBuf, Standing) {
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&ban)))]);
    commit(&root, &[("policy.cedar", "-")]);
    (root, standing)
}

/// Gives the plane's clone an upstream that holds every commit it has now.
fn push_to_an_upstream(m: &Machine, root: &Path) {
    let bare = m.dir.path().join("upstream.git");
    git(
        m.dir.path(),
        &["init", "-q", "--bare", bare.to_str().expect("a path")],
    );
    git(
        root,
        &["remote", "add", "origin", bare.to_str().expect("a path")],
    );
    git(root, &["push", "-q", "-u", "origin", "HEAD:main"]);
}

#[test]
fn the_press_shows_each_rule_as_loosens_tightens_or_changes() {
    let m = machine();
    let was = format!(
        "{HEADER}{}{}{}",
        // Dropped: a forbid is gone.
        "@id(\"guards-merge-ban\")\nforbid (principal, action == Action::\"merge\", resource);\n",
        // Edited in a way purlis cannot call narrower or wider.
        "@id(\"guards-merge-edit\")\nforbid (principal, action == Action::\"merge\", resource) \
         when { context.attended };\n",
        // Asks the person; will forbid outright.
        "@id(\"guards-merge-ask\") @ask(\"you\")\nforbid (principal, action == Action::\"merge\", \
         resource);\n",
    );
    let now = format!(
        "{HEADER}{}{}{}{}",
        "@id(\"guards-merge-edit\")\nforbid (principal, action == Action::\"merge\", resource) \
         when { !context.attended };\n",
        "@id(\"guards-merge-ask\")\nforbid (principal, action == Action::\"merge\", resource);\n",
        // A forbid added, and a permit added.
        "@id(\"guards-merge-new\")\nforbid (principal, action == Action::\"merge\", resource);\n",
        permit_merge("guards-merge-grant"),
    );
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &now)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&was)))]);
    let press = press_of(&root, &standing).expect("a press");
    let seen: Vec<(&str, Kind, &str)> = press
        .diff
        .rules
        .iter()
        .map(|one| (one.id.as_str(), one.kind, one.what))
        .collect();
    assert_eq!(
        seen,
        [
            ("guards-merge-ban", Kind::Loosens, "removed"),
            ("guards-merge-edit", Kind::Changes, "edited"),
            ("guards-merge-ask", Kind::Tightens, "no longer asks"),
            ("guards-merge-new", Kind::Tightens, "added"),
            ("guards-merge-grant", Kind::Loosens, "added"),
        ],
        "{press:?}"
    );
    assert_eq!(press.reason, Reason::Loosening);
}

#[test]
fn the_press_shows_mode_and_limit_changes() {
    let m = machine();
    let was = format!(
        "{STRICT}[policy.limits.dispatch]\nrunning-per-chat = 3\n[policy.limits.asks]\n\
         cool-off-within-minutes = 90\n"
    );
    let now = "[policy]\ntemplate = \"strict\"\nguards.merge = \"log\"\n\
               [policy.limits.dispatch]\nrunning-per-chat = 9\n[policy.limits.asks]\n\
               cool-off-within-minutes = 30\n";
    let root = plane(&m, &[("purlis.toml", now)]);
    let standing = holding(vec![accepted(&version(&was, None))]);
    let press = press_of(&root, &standing).expect("a press");
    let mode = press
        .diff
        .modes
        .iter()
        .find(|one| one.key == "guards.merge")
        .expect("a row for the merge guard");
    assert_eq!(
        (mode.before, mode.after, mode.kind),
        ("enforce", "log", Kind::Loosens)
    );
    let limit = |key: &str| {
        press
            .diff
            .limits
            .iter()
            .find(|one| one.key == key)
            .unwrap_or_else(|| panic!("no row for {key}: {:?}", press.diff.limits))
    };
    // A cap raised loosens; a wait lowered loosens, by each key's own direction.
    let cap = limit("dispatch.running-per-chat");
    assert_eq!(
        (cap.before, cap.after, cap.kind),
        (Some(3), Some(9), Kind::Loosens)
    );
    let wait = limit("asks.cool-off-within-minutes");
    assert_eq!(
        (wait.before, wait.after, wait.kind),
        (Some(90), Some(30), Kind::Loosens)
    );
}

#[test]
fn the_author_is_shown_as_the_commit_claims_it_and_never_vouched_for() {
    let m = machine();
    let (root, standing) = dropped_the_ban(&m);
    // The commit names its author, and purlis does not vouch for it: nothing it fetched, nothing
    // it signed, so the author is always the commit's own claim, pushed or not.
    let said = press_of(&root, &standing)
        .expect("a press")
        .commit
        .expect("a commit");
    assert_eq!(said.author, "t <nobody>", "{said:?}");
    assert_eq!(said.subject, "c");
    assert_eq!(said.sha.len(), 40);
    push_to_an_upstream(&m, &root);
    let pushed = press_of(&root, &standing)
        .expect("a press")
        .commit
        .expect("a commit");
    assert_eq!(
        pushed.author, said.author,
        "pushing proves nothing about who wrote it"
    );
}

#[test]
fn a_replace_ref_in_the_repo_changes_nothing_about_the_author_shown() {
    let m = machine();
    let (root, standing) = dropped_the_ban(&m);
    let first = git(&root, &["rev-list", "--max-parents=0", "HEAD"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    // A replace ref anywhere in the repo changes nothing the press shows: history is read as it
    // was written, so the author is the commit's own claim, never another object's.
    git(&root, &["replace", &first, &head]);
    let said = press_of(&root, &standing)
        .expect("a press")
        .commit
        .expect("a commit");
    assert_eq!(said.author, "t <nobody>", "{said:?}");
    assert_eq!(said.sha, head);
}

#[test]
fn the_press_diffs_against_the_policy_in_force_not_only_the_accepted_copy() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    // `HEAD` holds what the accepted copy holds (no ban), but the floor holds a commit that did.
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let standing = holding(vec![
        accepted(&version(STRICT, None)),
        floor(&version(STRICT, Some(&ban))),
    ]);
    let press = press_of(&root, &standing).expect("a press");
    assert_eq!(
        rule(&press, "guards-merge-ban").kind,
        Kind::Loosens,
        "{press:?}"
    );
    assert_eq!(press.reason, Reason::Loosening);
    // Against the accepted copy alone there is nothing to accept.
    let alone = holding(vec![accepted(&version(STRICT, None))]);
    assert_eq!(press_of(&root, &alone), None);
}

#[test]
fn a_commit_past_the_accepted_floor_cap_raises_the_press() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    // A commit that only tightens raises nothing…
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(STRICT, None))]);
    assert_eq!(press_of(&root, &standing), None);
    // …until it went past the cap, which is the deny state until the press.
    let past = Standing {
        state: State::Deny(vec![Deny::PastTheFloorCap]),
        kept: vec![accepted(&version(STRICT, None))],
        ..Standing::fresh()
    };
    let press = press_of(&root, &past).expect("a press");
    assert_eq!(press.reason, Reason::PastTheCap);
    assert_eq!(rule(&press, "guards-merge-ban").kind, Kind::Tightens);
    assert!(press.commit.is_some());
}

#[test]
fn removing_a_local_tightening_raises_the_press_and_is_shown_as_a_loosening() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", OPEN)]);
    // The project is looser than the shipped default, which stands in until the first press.
    let first = of(&root).expect("a press against the shipped default");
    accept(&root, &the_persons_press(), &first.digest).expect("the first press");
    let local = root.join(crate::settings::Which::Local.file_at(&root));
    std::fs::write(&local, STRICT).expect("the local file");
    // The app sees the tightening and seals it into the floor.
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Ask);
    assert_eq!(
        of(&root),
        None,
        "nothing waits while the tightening is there"
    );
    // The person takes the line out.
    std::fs::remove_file(&local).expect("removed");
    let press = of(&root).expect("a press for the removal");
    assert_eq!(press.reason, Reason::LocalRemoval);
    assert_eq!(press.commit, None, "a removal has no commit and no author");
    assert!(
        press
            .diff
            .local
            .iter()
            .any(|one| one.key == "template" && one.was == "strict" && one.now == "open"),
        "{press:?}"
    );
    assert!(
        press
            .diff
            .modes
            .iter()
            .any(|one| one.key == "guards.merge" && one.kind == Kind::Loosens),
        "{press:?}"
    );
    // Until the press the sealed tightening stays in force…
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Ask);
    // …and Accept applies it.
    accept(&root, &the_persons_press(), &press.digest).expect("the press");
    assert_eq!(of(&root), None);
    assert_ne!(host(&root, &merge(true)).after_mode, Outcome::Ask);
    let held = answers::standing(&root);
    assert!(!held.entries.iter().any(|e| e.kind == answers::FLOOR));
    assert!(held.entries.iter().any(|e| e.kind == answers::ACCEPTED));
}

#[test]
fn an_unrelated_press_lists_a_pending_local_file_loosening() {
    let m = machine();
    let was = forbid_merge("guards-merge-rule", WHEN_ATTENDED);
    let now = forbid_merge("guards-merge-rule", WHEN_UNATTENDED);
    let root = plane(&m, &[("purlis.toml", OPEN), ("policy.cedar", &now)]);
    // The floor holds a local tightening that the file no longer says, and an unrelated commit
    // changed a rule.
    let with_local = |rules: &str| Version {
        local: Some(STRICT.to_owned()),
        ..version(OPEN, Some(rules))
    };
    let standing = holding(vec![
        accepted(&version(OPEN, Some(&was))),
        floor(&with_local(&was)),
    ]);
    let press = press_of(&root, &standing).expect("a press");
    assert_eq!(press.reason, Reason::Loosening, "the commit's own press");
    assert_eq!(rule(&press, "guards-merge-rule").kind, Kind::Changes);
    assert!(press.commit.is_some());
    assert!(
        press.diff.local.iter().any(|one| one.key == "template"),
        "the local loosening is listed in the commit's press: {press:?}"
    );
}

fn other(press: &Press) -> Vec<&str> {
    press
        .diff
        .other
        .iter()
        .map(|one| one.part.as_str())
        .collect()
}

#[test]
fn a_commit_that_only_shrinks_a_set_raises_a_press_with_a_row() {
    let m = machine();
    let sets = |hosts: &str| format!("{STRICT}[policy.sets]\nknown-hosts = [{hosts}]\n");
    let root = plane(&m, &[("purlis.toml", &sets("\"a.com\""))]);
    let standing = holding(vec![accepted(&version(
        &sets("\"a.com\", \"b.com\""),
        None,
    ))]);
    let press = press_of(&root, &standing).expect("a press");
    assert_eq!(other(&press), ["policy.sets.known-hosts"], "{press:?}");
    assert_eq!(press.reason, Reason::Loosening);
}

#[test]
fn a_commit_that_only_adds_a_workspace_mode_raises_a_press_with_a_row() {
    let m = machine();
    let now = format!("{STRICT}[policy.workspace.web]\nguards.merge = \"log\"\n");
    let root = plane(&m, &[("purlis.toml", &now)]);
    let standing = holding(vec![accepted(&version(STRICT, None))]);
    let press = press_of(&root, &standing).expect("a press");
    assert_eq!(other(&press), ["policy.workspace.web"], "{press:?}");
}

#[test]
fn a_changed_vault_registry_or_an_unreadable_floor_version_is_a_row() {
    let m = machine();
    let root = plane(
        &m,
        &[("purlis.toml", STRICT), ("vaults.json", "{\"vaults\":{}}")],
    );
    let held = Version {
        vaults: Some("{\"vaults\":{\"v\":{\"env\":\"prod\"}}}".to_owned()),
        ..version(STRICT, None)
    };
    let standing = holding(vec![
        accepted(&held),
        Entry::new(answers::FLOOR, "not a version"),
    ]);
    let press = press_of(&root, &standing).expect("a press");
    let rows = other(&press);
    assert!(
        rows.contains(&"the shared vault registry (vaults.json)"),
        "{rows:?}"
    );
    assert!(
        rows.contains(&"a sealed version of the accepted floor does not read"),
        "{rows:?}"
    );
}

#[test]
fn an_absent_rule_mode_is_the_families_mode_not_enforce() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let was = "[policy]\ntemplate = \"strict\"\nguards.merge = \"log\"\n[policy.rules]\n\"guards-merge-ban\" = \"enforce\"\n";
    let now = "[policy]\ntemplate = \"strict\"\nguards.merge = \"log\"\n";
    let root = plane(&m, &[("purlis.toml", now), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(was, Some(&ban)))]);
    let press = press_of(&root, &standing).expect("a press");
    let row = press
        .diff
        .modes
        .iter()
        .find(|one| one.key == "rules.guards-merge-ban")
        .unwrap_or_else(|| panic!("{press:?}"));
    assert_eq!(
        (row.before, row.after, row.kind),
        ("enforce", "log", Kind::Loosens)
    );
}

#[test]
fn a_permit_going_off_to_enforce_loosens_and_a_dropped_ask_with_other_edits_only_changes() {
    let m = machine();
    let grant = permit_merge("guards-merge-grant");
    let off = format!("{STRICT}[policy.rules]\n\"guards-merge-grant\" = \"off\"\n");
    let on = format!("{STRICT}[policy.rules]\n\"guards-merge-grant\" = \"enforce\"\n");
    let rules = format!("{HEADER}{grant}");
    let root = plane(&m, &[("purlis.toml", &on), ("policy.cedar", &rules)]);
    let standing = holding(vec![accepted(&version(&off, Some(&rules)))]);
    let press = press_of(&root, &standing).expect("a press");
    let row = press
        .diff
        .modes
        .iter()
        .find(|one| one.key == "rules.guards-merge-grant")
        .unwrap_or_else(|| panic!("{press:?}"));
    assert_eq!(row.kind, Kind::Loosens, "{row:?}");
    // An `@ask` dropped together with another edit is not a tightening.
    let asking = format!(
        "{HEADER}@id(\"guards-merge-x\") @ask(\"you\")\nforbid (principal, action == Action::\"merge\", \
         resource) when {{ context.attended }};\n"
    );
    let plain = forbid_merge("guards-merge-x", WHEN_UNATTENDED);
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &plain)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&asking)))]);
    let press = press_of(&root, &standing).expect("a press");
    assert_eq!(
        rule(&press, "guards-merge-x").kind,
        Kind::Changes,
        "{press:?}"
    );
}

#[test]
fn the_digest_binds_the_bases_the_rows_and_the_commit() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let one = holding(vec![
        accepted(&version(STRICT, None)),
        floor(&version(STRICT, Some(&ban))),
    ]);
    let other_floor = holding(vec![
        accepted(&version(STRICT, None)),
        floor(&version(
            STRICT,
            Some(&format!("{ban}// the same rules, another version\n")),
        )),
    ]);
    let first = press_of(&root, &one).expect("a press").digest;
    assert_ne!(
        first,
        press_of(&root, &other_floor).expect("a press").digest
    );
    // A new commit with the same project text is a different press.
    commit(&root, &[]);
    assert_ne!(first, press_of(&root, &one).expect("a press").digest);
}

#[test]
fn a_rule_mode_entry_that_goes_is_a_row_whatever_family_it_reads_as() {
    let m = machine();
    // A forbid over two families' actions: how its entry reads as a mode depends on the request.
    let both = format!(
        "{HEADER}@id(\"trust-x\")\nforbid (principal, action in [Action::\"start\", Action::\"merge\"], \
         resource);\n"
    );
    let head = "[policy]\ntemplate = \"strict\"\nguards.merge = \"off\"\n";
    let was = format!("{head}[policy.rules]\n\"trust-x\" = \"enforce\"\n");
    let root = plane(&m, &[("purlis.toml", head), ("policy.cedar", &both)]);
    let standing = holding(vec![accepted(&version(&was, Some(&both)))]);
    let press = press_of(&root, &standing).expect("a press");
    assert!(other(&press).contains(&"policy.rules.trust-x"), "{press:?}");
}

#[test]
fn a_rule_mode_entry_that_appears_is_a_row_even_where_a_mode_row_shows_it_too() {
    let m = machine();
    let both = format!(
        "{HEADER}@id(\"trust-x\")\nforbid (principal, action in [Action::\"start\", Action::\"merge\"], \
         resource);\n"
    );
    let was = "[policy]\ntemplate = \"strict\"\nguards.merge = \"off\"\n";
    let now = format!("{was}[policy.rules]\n\"trust-x\" = \"log\"\n");
    let root = plane(&m, &[("purlis.toml", &now), ("policy.cedar", &both)]);
    let standing = holding(vec![accepted(&version(was, Some(&both)))]);
    let press = press_of(&root, &standing).expect("a press");
    assert!(
        press
            .diff
            .modes
            .iter()
            .any(|row| row.key == "rules.trust-x"),
        "{press:?}"
    );
    assert!(other(&press).contains(&"policy.rules.trust-x"), "{press:?}");
    assert!(press.diff.waits());
}

// ---- the shipped default is versioned (M75-17, #7026) --------------------------------------------

/// The shipped default of a later purlis that bans a merge outright.
fn shipped_with_a_ban() -> String {
    format!(
        "{}\n\n@id(\"guards-merge-shipped-ban\") @level(\"project\")\nforbid (principal, action == \
         Action::\"merge\", resource);\n",
        crate::policy::DEFAULT
    )
}

/// A plain strict project, accepted under the shipped default `text`.
fn accepted_under(text: String) -> Entry {
    accepted(&Version {
        shipped: Some(text),
        ..version(STRICT, None)
    })
}

#[test]
fn a_shipped_default_update_is_announced_once() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    // The update dropped a forbid the person accepted under: it waits, with its own reason, its
    // diff and no commit, since no commit loosened anything.
    let standing = holding(vec![accepted_under(shipped_with_a_ban())]);
    let press = press_of(&root, &standing).expect("a press");
    assert_eq!(press.reason, Reason::ShippedDefault, "{press:?}");
    assert_eq!(press.commit, None, "{press:?}");
    assert_eq!(rule(&press, "guards-merge-shipped-ban").kind, Kind::Loosens);
    assert!(
        press
            .diff
            .other
            .iter()
            .any(|one| one.part.contains("shipped default")),
        "{press:?}"
    );
    // The same press every time it is read, so the window has one line, not one per decision.
    assert_eq!(press_of(&root, &standing).as_ref(), Some(&press));
    // On the real store: the press, Accept, and then nothing is raised again.
    let real = accepted_under(shipped_with_a_ban());
    answers::commit(&root, |draft| {
        draft.add(&the_persons_press(), &real.kind, &real.text);
    })
    .expect("an accepted copy");
    let press = of(&root).expect("a press on the real store");
    assert_eq!(press.reason, Reason::ShippedDefault);
    assert_eq!(
        host(&root, &merge(true)).after_mode,
        Outcome::Deny,
        "held until Accept"
    );
    accept(&root, &the_persons_press(), &press.digest).expect("the press");
    assert_eq!(of(&root), None, "announced once");
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Ask);
    assert_eq!(
        of(&root),
        None,
        "and not again by the decision that sealed what it saw"
    );
}

#[test]
fn a_chats_commit_of_a_loosening_changes_nothing_until_the_press() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    // The person pressed Accept on the project as it stands: the ban is what they accepted.
    if let Some(first) = of(&root) {
        accept(&root, &the_persons_press(), &first.digest).expect("the first press");
    }
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Deny);
    // A chat commits the loosening: it drops the ban. Whoever the commit says wrote it, it is
    // only a commit.
    commit(&root, &[("policy.cedar", "-")]);
    let said = host(&root, &merge(true));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
    // The window shows the loosening, with its row, and waits.
    let press = of(&root).expect("a press for the commit");
    assert_eq!(rule(&press, "guards-merge-ban").kind, Kind::Loosens);
    assert!(press.commit.is_some());
    // Still nothing changed for any chat…
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Deny);
    // …until the person's press, which accepts exactly what it showed.
    accept(&root, &the_persons_press(), &press.digest).expect("the press");
    assert_eq!(of(&root), None);
    assert_ne!(host(&root, &merge(true)).after_mode, Outcome::Deny);
}
