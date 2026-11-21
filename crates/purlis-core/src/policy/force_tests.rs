// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The policy in force and the accepted floor** (M75-14, #7023): each decision is the stricter of
//! the project as committed now and what the person accepted, every version of the accepted floor
//! included. The accepted copy is handed in as a standing built by hand; the floor is sealed for
//! real, in a machine of the test's own, as the app does at every decision.

use std::path::{Path, PathBuf};

use super::*;
use crate::enforcement::Resolved;
use crate::policy::decide::Outcome;
use crate::policy::levels::Chat;
use crate::sandbox::local::OwnConfigHome;
use crate::secrets::answers::{Deny, Entry};
use crate::secrets::seal;

const WORKSPACES: [&str; 1] = ["web"];
const PERSONAS: [&str; 1] = ["builder"];

pub(in crate::policy) const STRICT: &str = "[policy]\ntemplate = \"strict\"\n";
pub(in crate::policy) const HEADER: &str = "// purlis-policy 1\n";

/// A machine of the test's own: its config home, a keyring that knows nothing, a key.
pub(in crate::policy) struct Machine {
    _home: OwnConfigHome,
    pub(in crate::policy) dir: tempfile::TempDir,
}

pub(in crate::policy) fn machine() -> Machine {
    let home = OwnConfigHome::new();
    seal::forget_kept_for_this_test();
    seal::set_for_this_test(Some(vec![7; 32]));
    seal::set_foreign_for_this_test(false);
    seal::set_locked_for_this_test(false);
    seal::set_no_keyring_for_this_test(false);
    Machine {
        _home: home,
        dir: tempfile::tempdir().expect("a folder for planes"),
    }
}

pub(in crate::policy) fn git(dir: &Path, args: &[&str]) -> String {
    let ran = crate::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=nobody"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
    String::from_utf8_lossy(&ran.stdout).trim().to_owned()
}

/// A plane on `machine`, its root committing `files` (and a `.gitignore` for the local files).
pub(in crate::policy) fn plane(machine: &Machine, files: &[(&str, &str)]) -> PathBuf {
    let root = machine.dir.path().join("plane");
    std::fs::create_dir_all(&root).expect("a plane");
    let root = std::fs::canonicalize(&root).expect("a real path");
    git(&root, &["init", "-q"]);
    std::fs::write(
        root.join(".gitignore"),
        "*.local.toml\n.charter/\n.purlis/\n",
    )
    .expect("a .gitignore");
    commit(&root, files);
    root
}

/// Commits `files` as they are now: written, or removed where the text is `-`.
pub(in crate::policy) fn commit(root: &Path, files: &[(&str, &str)]) {
    for (name, text) in files {
        let path = root.join(name);
        if *text == "-" {
            std::fs::remove_file(&path).expect("a file to remove");
            continue;
        }
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("a folder");
        std::fs::write(path, text).expect("a file");
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "--allow-empty", "-m", "c"]);
}

pub(in crate::policy) fn sources(root: &Path) -> Sources<'_> {
    Sources {
        root,
        workspaces: &WORKSPACES,
        personas: &PERSONAS,
        organisation: None,
        accepted: None,
    }
}

pub(in crate::policy) fn holding(entries: Vec<Entry>) -> Standing {
    Standing {
        state: State::Holds,
        entries,
        ..Standing::fresh()
    }
}

pub(in crate::policy) fn accepted(version: &Version) -> Entry {
    Entry::new(answers::ACCEPTED, &serde_json::to_string(version).unwrap())
}

pub(in crate::policy) fn floor(version: &Version) -> Entry {
    Entry::new(answers::FLOOR, &serde_json::to_string(version).unwrap())
}

pub(in crate::policy) fn version(settings: &str, rules: Option<&str>) -> Version {
    Version {
        settings_file: Some("purlis.toml".to_owned()),
        settings: Some(settings.to_owned()),
        rules: rules.map(str::to_owned),
        ..Version::default()
    }
}

fn base(attended: bool) -> serde_json::Value {
    serde_json::json!({
        "sandbox": "floor", "attended": attended, "harness_mode": "default", "depth": 0
    })
}

fn request<'a>(
    action: &'a str,
    resource: (&'a str, &'a str),
    context: serde_json::Value,
) -> Request<'a> {
    Request {
        chat: "1",
        of: Chat {
            workspace: Some("web"),
            persona: Some("builder"),
        },
        action,
        resource,
        context,
    }
}

pub(in crate::policy) fn merge(attended: bool) -> Request<'static> {
    let mut context = base(attended);
    context["via"] = "hook".into();
    request("merge", ("Branch", "acme/web#main"), context)
}

fn connect(host: &'static str) -> Request<'static> {
    let mut context = base(true);
    context["via"] = "hook".into();
    context["port"] = 443.into();
    request("connect", ("Host", host), context)
}

fn store(vault: &'static str) -> Request<'static> {
    let mut context = base(true);
    context["program"] = "op".into();
    context["declared"] = true.into();
    request("store", ("Vault", vault), context)
}

fn read(path: &'static str) -> Request<'static> {
    request("read", ("Path", path), base(true))
}

/// What the host does for one request: the policy in force over the store as it stands, then what
/// it saw sealed.
pub(in crate::policy) fn host(root: &Path, request: &Request<'_>) -> Decision {
    in_force(
        root,
        &sources(root),
        &answers::standing(root),
        request,
        None,
    )
}

/// One decision with a standing built by hand, and nothing sealed.
fn by(root: &Path, standing: &Standing, request: &Request<'_>) -> Decision {
    judge(root, &sources(root), standing, request, None).0
}

pub(in crate::policy) fn forbid_merge(id: &str, when: &str) -> String {
    format!(
        "{HEADER}@id(\"{id}\")\nforbid (principal, action == Action::\"merge\", resource){when};\n"
    )
}

const WHEN_ATTENDED: &str = " when { context.attended }";
const WHEN_UNATTENDED: &str = " when { !context.attended }";

#[test]
fn a_committed_tightening_applies_at_once() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let none = Standing::fresh();
    assert_eq!(by(&root, &none, &merge(true)).after_mode, Outcome::Ask);
    commit(
        &root,
        &[("policy.cedar", &forbid_merge("guards-merge-ban", ""))],
    );
    let said = by(&root, &none, &merge(true));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
}

#[test]
fn a_committed_loosening_waits_for_the_press() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&ban)))]);
    assert_eq!(by(&root, &standing, &merge(true)).after_mode, Outcome::Deny);
    // The commit drops the ban: nothing is accepted of that yet.
    commit(&root, &[("policy.cedar", "-")]);
    let said = by(&root, &standing, &merge(true));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
    // The press, which slice 15 makes, accepts the loosened copy: then it stands.
    let pressed = holding(vec![accepted(&version(STRICT, None))]);
    assert_eq!(by(&root, &pressed, &merge(true)).after_mode, Outcome::Ask);
}

#[test]
fn moving_head_back_keeps_a_sealed_tightening() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let before = git(&root, &["rev-parse", "HEAD"]);
    commit(
        &root,
        &[("policy.cedar", &forbid_merge("guards-merge-ban", ""))],
    );
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Deny);
    git(&root, &["update-ref", "HEAD", &before]);
    git(&root, &["reset", "-q", "--hard", "HEAD"]);
    assert!(!root.join("policy.cedar").exists(), "HEAD is back");
    let said = host(&root, &merge(true));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
}

#[test]
fn a_committed_broken_file_keeps_the_sealed_forbids() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    commit(
        &root,
        &[("policy.cedar", &forbid_merge("guards-merge-ban", ""))],
    );
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Deny);
    commit(&root, &[("policy.cedar", "this is not a policy {")]);
    let said = host(&root, &merge(true));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
}

#[test]
fn deleting_an_entity_a_forbid_names_keeps_the_forbid() {
    let m = machine();
    let rule = format!(
        "{HEADER}@id(\"sandbox-no-web-read\")\nforbid (principal, action == Action::\"read\", resource) \
         when {{ resource has workspace && resource.workspace == Workspace::\"web\" }};\n"
    );
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &rule),
            ("workspaces/web/README.md", "web\n"),
        ],
    );
    let path = "workspaces/web/README.md";
    assert_eq!(host(&root, &read(path)).after_mode, Outcome::Deny);
    // The workspace the rule names is deleted in a commit: the path has no owning workspace now.
    commit(&root, &[("workspaces/web/README.md", "-")]);
    let said = host(&root, &read(path));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["sandbox-no-web-read"], "{said:?}");
}

#[test]
fn a_widened_keep_waits_for_the_press() {
    let m = machine();
    let hold = |keep: &str| {
        format!(
            "{HEADER}@id(\"guards-merge-hold\") @level(\"project\") @ask(\"you\") @keep(\"{keep}\")\n\
             forbid (principal, action in [Action::\"merge\", Action::\"land\"], resource);\n"
        )
    };
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &hold("everywhere")),
        ],
    );
    let standing = holding(vec![accepted(&version(STRICT, Some(&hold("this chat"))))]);
    let said = by(&root, &standing, &merge(true));
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    assert_eq!(said.keep.as_deref(), Some("this chat"), "{said:?}");
    // The press accepts the widened copy.
    let pressed = holding(vec![accepted(&version(STRICT, Some(&hold("everywhere"))))]);
    assert_eq!(
        by(&root, &pressed, &merge(true)).keep.as_deref(),
        Some("everywhere")
    );
}

#[test]
fn each_evaluation_uses_its_own_sides_named_sets() {
    let m = machine();
    let known = format!("{STRICT}\n[policy.sets]\nknown-hosts = [\"api.acme.com\"]\n");
    let root = plane(&m, &[("purlis.toml", &known)]);
    // Committed knows the host; what was accepted does not, and its sets are its own.
    let standing = holding(vec![accepted(&version(STRICT, None))]);
    let said = by(&root, &standing, &connect("api.acme.com"));
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    assert_eq!(said.ids(), ["network-unknown-host"], "{said:?}");
    // Where both know it, it goes ahead.
    let both = holding(vec![accepted(&version(&known, None))]);
    assert_eq!(
        by(&root, &both, &connect("api.acme.com")).after_mode,
        Outcome::Allow
    );
}

#[test]
fn a_changed_rule_keeps_its_tightening_half_until_the_press() {
    let m = machine();
    let was = forbid_merge("guards-merge-rule", WHEN_ATTENDED);
    let now = forbid_merge("guards-merge-rule", WHEN_UNATTENDED);
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &now)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&was)))]);
    // The rule now bans the unattended and no longer the attended; each half is still held.
    for attended in [true, false] {
        let said = by(&root, &standing, &merge(attended));
        assert_eq!(
            said.after_mode,
            Outcome::Deny,
            "attended {attended}: {said:?}"
        );
        assert_eq!(said.ids(), ["guards-merge-rule"]);
    }
}

#[test]
fn two_successive_commits_keep_every_tightening_seen_since_the_last_press() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    commit(
        &root,
        &[(
            "policy.cedar",
            &forbid_merge("guards-merge-first", WHEN_ATTENDED),
        )],
    );
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Deny);
    commit(
        &root,
        &[(
            "policy.cedar",
            &forbid_merge("guards-merge-second", WHEN_UNATTENDED),
        )],
    );
    assert_eq!(host(&root, &merge(false)).after_mode, Outcome::Deny);
    // Both are dropped from HEAD, and both are still held.
    commit(&root, &[("policy.cedar", "-")]);
    for attended in [true, false] {
        let said = host(&root, &merge(attended));
        assert_eq!(
            said.after_mode,
            Outcome::Deny,
            "attended {attended}: {said:?}"
        );
    }
}

#[test]
fn each_accepted_floor_version_decides_with_its_own_named_sets() {
    let m = machine();
    let production = format!(
        "{STRICT}\n[policy.sets]\nknown-hosts = [\"api.acme.com\"]\nproduction-hosts = [\"api.acme.com\"]\n"
    );
    let known = format!("{STRICT}\n[policy.sets]\nknown-hosts = [\"api.acme.com\"]\n");
    let root = plane(&m, &[("purlis.toml", &known)]);
    // The sealed version holds the host as production; HEAD holds it as known, nothing more.
    let standing = holding(vec![floor(&version(&production, None))]);
    let said = by(&root, &standing, &connect("api.acme.com"));
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
    assert_eq!(said.ids(), ["network-production-host"], "{said:?}");
}

/// `n` more commits to `policy.cedar`, each a rule of its own, and the host deciding after each.
fn commits_deciding(root: &Path, from: usize, n: usize) {
    for i in from..from + n {
        commit(
            root,
            &[(
                "policy.cedar",
                &forbid_merge(&format!("guards-merge-v{i}"), WHEN_ATTENDED),
            )],
        );
        host(root, &merge(false));
    }
}

fn past_the_cap(root: &Path) -> bool {
    let standing = answers::standing(root);
    standing
        .causes()
        .iter()
        .any(|cause| matches!(cause, Deny::PastTheFloorCap))
}

#[test]
fn a_commit_past_the_accepted_floor_cap_records_the_deny_state() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    // The plane's first commit is version one; the cap holds 32 of them.
    host(&root, &merge(false));
    commits_deciding(&root, 2, 31);
    assert!(!past_the_cap(&root), "32 versions are within the cap");
    assert_eq!(
        answers::standing(root.as_path()).state,
        State::Holds,
        "the store holds"
    );
    commits_deciding(&root, 33, 1);
    assert!(past_the_cap(&root), "the 33rd is past it");
}

#[test]
fn the_accepted_floor_cap_is_the_shipped_value_before_limits_land() {
    assert_eq!(cap(), 32);
}

#[test]
fn local_tightenings_do_not_count_toward_the_accepted_floor_cap() {
    let m = machine();
    let hold = "[policy]\ntemplate = \"hold\"\n";
    let root = plane(&m, &[("purlis.toml", hold)]);
    host(&root, &merge(false));
    commits_deciding(&root, 2, 31);
    // Thirty-two committed versions are held. The local file tightens three more times.
    let local = root.join(crate::settings::Which::Local.file_at(&root));
    for i in 0..3 {
        std::fs::write(
            &local,
            format!("[policy]\ntemplate = \"strict\"\n# tightening {i}\n"),
        )
        .expect("the local file");
        host(&root, &merge(false));
    }
    let sealed = answers::standing(&root)
        .entries
        .iter()
        .filter(|e| e.kind == answers::FLOOR)
        .count();
    assert!(sealed >= 34, "each tightening is sealed: {sealed}");
    assert!(
        !past_the_cap(&root),
        "the local file's versions are not counted"
    );
}

#[test]
fn an_uncommitted_vault_tag_edit_changes_no_decision() {
    let m = machine();
    let rule = prod_vault_rule();
    let dev = r#"{"vaults": {"db": {"provider": "file", "persona": "builder", "env": "dev"}}}"#;
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &rule),
            ("vaults.json", dev),
        ],
    );
    assert_eq!(host(&root, &store("db")).after_mode, Outcome::Allow);
    // The working file says prod; HEAD still says dev.
    std::fs::write(root.join("vaults.json"), dev.replace("dev", "prod")).expect("an edit");
    assert_eq!(host(&root, &store("db")).after_mode, Outcome::Allow);
}

fn prod_vault_rule() -> String {
    format!(
        "{HEADER}@id(\"vaults-no-prod-store\")\nforbid (principal, action == Action::\"store\", resource is Vault) \
         when {{ resource.hasTag(\"env\") && resource.getTag(\"env\") == \"prod\" }};\n"
    )
}

#[test]
fn a_committed_loosening_of_an_entity_attribute_waits_for_the_press() {
    let m = machine();
    let rule = prod_vault_rule();
    let vaults = |env: &str| {
        format!(
            r#"{{"vaults": {{"db": {{"provider": "file", "persona": "builder", "env": "{env}"}}}}}}"#
        )
    };
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &rule),
            ("vaults.json", &vaults("dev")),
        ],
    );
    let mut was = version(STRICT, Some(&rule));
    was.vaults = Some(vaults("prod"));
    let standing = holding(vec![accepted(&was)]);
    // HEAD says dev, which the person has not accepted: the vault is still prod.
    let said = by(&root, &standing, &store("db"));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["vaults-no-prod-store"], "{said:?}");
}

#[test]
fn an_unsealed_local_vault_tag_is_ignored_and_its_vault_belongs_to_no_persona() {
    let m = machine();
    let rule = format!(
        "{HEADER}@id(\"vaults-builder-store\")\nforbid (principal, action == Action::\"store\", resource is Vault) \
         when {{ resource in Persona::\"builder\" }};\n"
    );
    let shared = r#"{"vaults": {"db": {"provider": "file"}}}"#;
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &rule),
            ("vaults.json", shared),
        ],
    );
    // A local tag, which nothing sealed at registration, says the vault is the builder's.
    for local in [".charter/vaults.json", ".purlis/vaults.json"] {
        std::fs::create_dir_all(root.join(local).parent().unwrap()).unwrap();
        std::fs::write(
            root.join(local),
            r#"{"vaults": {"db": {"persona": "builder"}}}"#,
        )
        .unwrap();
    }
    let said = host(&root, &store("db"));
    assert_eq!(
        said.after_mode,
        Outcome::Allow,
        "belongs to no persona: {said:?}"
    );
    // The same tag, committed in the shared half, is the vault's persona.
    commit(
        &root,
        &[(
            "vaults.json",
            r#"{"vaults": {"db": {"provider": "file", "persona": "builder"}}}"#,
        )],
    );
    let said = host(&root, &store("db"));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["vaults-builder-store"], "{said:?}");
}

#[test]
fn a_new_local_tightening_applies_at_once() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let standing = holding(vec![accepted(&version(open, None))]);
    assert_eq!(
        by(&root, &standing, &merge(true)).after_mode,
        Outcome::Allow
    );
    let local = root.join(crate::settings::Which::Local.file_at(&root));
    std::fs::write(&local, STRICT).expect("the local file");
    assert_eq!(by(&root, &standing, &merge(true)).after_mode, Outcome::Ask);
}

#[test]
fn removing_a_local_tightening_waits_for_the_press() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let local = root.join(crate::settings::Which::Local.file_at(&root));
    std::fs::write(&local, STRICT).expect("the local file");
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Ask);
    // The person takes the line out: the sealed tightening stays in force.
    std::fs::remove_file(&local).expect("removed");
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Ask);
}

// ---- review round 2: start, broken sides, one read of HEAD ------------------------------------

fn start() -> Request<'static> {
    let mut context = base(true);
    context["profile"] = "p".into();
    context["gaps"] = serde_json::json!([]);
    request("start", ("Harness", "claude"), context)
}

fn forbid_start() -> String {
    format!(
        "{HEADER}@id(\"trust-no-start\")\nforbid (principal, action == Action::\"start\", resource);\n"
    )
}

#[test]
fn a_start_is_held_to_the_accepted_copy_and_the_floor() {
    let m = machine();
    let ban = forbid_start();
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&ban)))]);
    assert_eq!(by(&root, &standing, &start()).after_mode, Outcome::Deny);
    // A commit drops the forbid on `start`: the accepted copy still holds it.
    commit(&root, &[("policy.cedar", "-")]);
    let said = by(&root, &standing, &start());
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["trust-no-start"], "{said:?}");
    // And a start judged the way the host judges it reads the store, not a fresh one.
    assert_eq!(
        crate::policy::decide::Sealed::for_start(&root).standing,
        answers::standing(&root)
    );
}

/// A standing in the deny state where a commit went past the cap: the store verified, and its
/// entries are set aside in `kept`.
fn denied_past_the_cap(kept: Vec<Entry>) -> Standing {
    Standing {
        state: State::Deny(vec![Deny::PastTheFloorCap]),
        kept,
        ..Standing::fresh()
    }
}

/// A verified store whose old nevers are not all imported: its entries are set aside in `kept`.
fn denied_by_a_late_never(kept: Vec<Entry>) -> Standing {
    Standing {
        state: State::Deny(vec![Deny::NeversNotImported]),
        kept,
        store_verified: true,
        ..Standing::fresh()
    }
}

#[test]
fn a_start_with_a_never_not_imported_is_held_to_the_accepted_copy() {
    let m = machine();
    let ban = forbid_start();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let standing = denied_by_a_late_never(vec![accepted(&version(STRICT, Some(&ban)))]);
    assert!(!standing.is_unverified_by_a_reachable_cause());
    let said = by(&root, &standing, &start());
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["trust-no-start"], "{said:?}");
}

#[test]
fn a_never_not_imported_beside_a_verified_empty_store_is_not_a_reachable_cause() {
    // A genuine first-run upgrade: the store verified and holds nothing yet.
    assert!(!denied_by_a_late_never(Vec::new()).is_unverified_by_a_reachable_cause());
    // No verified store behind it: a chat could have written the file, so a start is downgraded.
    let unbacked = Standing {
        state: State::Deny(vec![Deny::NeversNotImported]),
        ..Standing::fresh()
    };
    assert!(unbacked.is_unverified_by_a_reachable_cause());
}

#[test]
fn a_start_past_the_cap_is_held_to_the_forbid_only_the_accepted_copy_holds() {
    let m = machine();
    let ban = forbid_start();
    let open = "[policy]\ntemplate = \"open\"\n";
    // The commit dropped the forbid; the accepted copy still holds it.
    let root = plane(&m, &[("purlis.toml", open)]);
    let standing = denied_past_the_cap(vec![accepted(&version(STRICT, Some(&ban)))]);
    let said = by(&root, &standing, &start());
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["trust-no-start"], "{said:?}");
}

#[test]
fn a_start_past_the_cap_is_held_to_a_forbid_only_a_floor_version_holds() {
    let m = machine();
    let ban = forbid_start();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let standing = denied_past_the_cap(vec![floor(&version(STRICT, Some(&ban)))]);
    let said = by(&root, &standing, &start());
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}

#[test]
fn a_start_with_an_entry_that_does_not_read_is_refused() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    for entry in [
        Entry::new(answers::ACCEPTED, "not json"),
        Entry::new(answers::FLOOR, "{\"settings\": 3}"),
        // A version sealed before the format stamp: its personas are not known.
        Entry::new(answers::FLOOR, "{\"settings_file\":\"purlis.toml\"}"),
    ] {
        for standing in [
            holding(vec![entry.clone()]),
            denied_past_the_cap(vec![entry.clone()]),
        ] {
            let said = by(&root, &standing, &start());
            assert_eq!(said.after_mode, Outcome::Deny, "{entry:?} {said:?}");
        }
    }
}

#[test]
fn a_start_with_known_gaps_is_refused_where_nothing_was_verified() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let denied = Standing {
        state: State::Deny(vec![Deny::CloneKeySwapped]),
        ..Standing::fresh()
    };
    let said = by(&root, &denied, &start());
    assert_eq!(
        said.after_mode,
        strict_of_start(),
        "no gaps: the shipped strict policy"
    );
    let mut gappy = start();
    gappy.context["gaps"] = serde_json::json!(["processes"]);
    let said = by(&root, &denied, &gappy);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}

fn strict_of_start() -> Outcome {
    undecidable_strict(&start()).after_mode
}

/// The plane's folders as the disk holds them now: what the app lists for every decision.
fn disk_sources_host(root: &Path, request: &Request<'_>) -> Decision {
    let list = |parent: &str| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(root.join(parent))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| !n.starts_with('.'))
            .collect();
        names.sort();
        names
    };
    let (workspaces, personas) = (list("workspaces"), list("personas"));
    let workspaces: Vec<&str> = workspaces.iter().map(String::as_str).collect();
    let personas: Vec<&str> = personas.iter().map(String::as_str).collect();
    let sources = Sources {
        root,
        workspaces: &workspaces,
        personas: &personas,
        organisation: None,
        accepted: None,
    };
    in_force(root, &sources, &answers::standing(root), request, None)
}

const TABLES: &str =
    "[policy]\ntemplate = \"strict\"\n[policy.persona.reviewer]\n[policy.workspace.web]\n";

fn plane_with_tables(m: &Machine) -> PathBuf {
    let ban = forbid_merge("guards-merge-ban", "");
    plane(
        m,
        &[
            ("purlis.toml", TABLES),
            ("policy.cedar", &ban),
            ("personas/reviewer/persona.md", "r\n"),
            ("workspaces/web/README.md", "web\n"),
        ],
    )
}

#[test]
fn an_uncommitted_persona_rename_drops_no_forbid() {
    let m = machine();
    let root = plane_with_tables(&m);
    assert_eq!(
        disk_sources_host(&root, &merge(true)).after_mode,
        Outcome::Deny
    );
    std::fs::rename(
        root.join("personas/reviewer"),
        root.join("personas/_reviewer"),
    )
    .expect("a rename, uncommitted");
    let said = disk_sources_host(&root, &merge(true));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
}

#[test]
fn an_uncommitted_workspace_removal_drops_no_forbid() {
    let m = machine();
    let root = plane_with_tables(&m);
    assert_eq!(
        disk_sources_host(&root, &merge(true)).after_mode,
        Outcome::Deny
    );
    std::fs::remove_dir_all(root.join("workspaces/web")).expect("removed, uncommitted");
    let said = disk_sources_host(&root, &merge(true));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
}

#[test]
fn a_head_that_moves_between_reads_never_skips_a_sealed_version() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    commit(
        &root,
        &[("policy.cedar", &forbid_merge("guards-merge-ban", ""))],
    );
    let strict_commit = git(&root, &["rev-parse", "HEAD"]);
    assert_eq!(host(&root, &merge(true)).after_mode, Outcome::Deny);
    commit(&root, &[("policy.cedar", "-")]);
    assert_eq!(
        host(&root, &merge(true)).after_mode,
        Outcome::Deny,
        "sealed"
    );
    // HEAD is on the loose version, and moves to the strict one once, between the reads.
    let (at, moved) = (root.clone(), std::rc::Rc::new(std::cell::Cell::new(false)));
    seam::set(Some(Box::new(move || {
        if !moved.replace(true) {
            git(&at, &["update-ref", "HEAD", &strict_commit]);
        }
    })));
    let said = host(&root, &merge(true));
    seam::set(None);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
}

#[test]
fn a_head_that_never_holds_still_has_no_version_of_two_commits() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let first = git(&root, &["rev-parse", "HEAD"]);
    commit(
        &root,
        &[("policy.cedar", &forbid_merge("guards-merge-ban", ""))],
    );
    let second = git(&root, &["rev-parse", "HEAD"]);
    let (at, turn) = (root.clone(), std::rc::Rc::new(std::cell::Cell::new(false)));
    seam::set(Some(Box::new(move || {
        let to = if turn.replace(!turn.get()) {
            &second
        } else {
            &first
        };
        git(&at, &["update-ref", "HEAD", to]);
    })));
    let read = Version::at_head(&root);
    assert!(
        matches!(read, Head::Unread),
        "a HEAD that keeps moving is no read: {read:?}"
    );
    // The current side is then undecidable, never a fresh read over a world with no workspaces.
    let said = host(&root, &merge(true));
    seam::set(None);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}

#[test]
fn each_side_is_held_to_the_floor_before_the_stricter_wins() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let watch = "[policy]\ntemplate = \"watch\"\n";
    let root = plane(&m, &[("purlis.toml", watch), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(STRICT, None))]);
    let floor = Resolved::strict();
    let said = judge(
        &root,
        &sources(&root),
        &standing,
        &merge(true),
        Some(&floor),
    )
    .0;
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["guards-merge-ban"], "{said:?}");
}

#[test]
fn an_unreadable_accepted_copy_is_the_deny_state() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let standing = holding(vec![Entry::new(answers::ACCEPTED, "not json")]);
    let said = by(&root, &standing, &merge(true));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert!(said.deny_state.is_some(), "{said:?}");
    let floored = holding(vec![Entry::new(answers::FLOOR, "not json")]);
    let said = by(&root, &floored, &merge(true));
    assert!(said.deny_state.is_some(), "{said:?}");
}

#[test]
fn the_local_files_sealed_versions_are_capped_too() {
    let m = machine();
    let hold = "[policy]\ntemplate = \"hold\"\n";
    let root = plane(&m, &[("purlis.toml", hold)]);
    let local = root.join(crate::settings::Which::Local.file_at(&root));
    for i in 0..cap() + 2 {
        std::fs::write(
            &local,
            format!("[policy]\ntemplate = \"strict\"\n# tightening {i}\n"),
        )
        .expect("the local file");
        host(&root, &merge(false));
    }
    assert!(
        past_the_cap(&root),
        "local versions past the cap are recorded"
    );
}

// ---- review round 4: a start in a deny state where nothing was verified -----------------------

fn clone_key_swapped() -> Standing {
    Standing {
        state: State::Deny(vec![Deny::CloneKeySwapped]),
        ..Standing::fresh()
    }
}

#[test]
fn a_start_with_the_sandbox_off_is_refused_where_nothing_was_verified() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    // The person accepted a forbid on a start with the sandbox off; a swapped `.git` verified
    // nothing, so the accepted copy is not read, and the refusal is the deny state's own.
    let ban = format!(
        "{HEADER}@id(\"no-unsandboxed-start\")\nforbid (principal, action == Action::\"start\", resource) when {{ context.sandbox == \"off\" }};\n"
    );
    let mut standing = clone_key_swapped();
    standing.entries = vec![accepted(&version(open, Some(&ban)))];
    let mut unsandboxed = start();
    unsandboxed.context["sandbox"] = "off".into();
    let said = by(&root, &standing, &unsandboxed);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(by(&root, &standing, &start()).after_mode, strict_of_start());
}

#[test]
fn a_start_naming_a_gap_family_at_log_is_refused_where_nothing_was_verified() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    // The caller counts every family a gap affects, so `context.gaps` is not empty even where
    // the family is at log.
    let mut gappy = start();
    gappy.context["gaps"] = serde_json::json!(["trust"]);
    let said = by(&root, &clone_key_swapped(), &gappy);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}

#[test]
fn a_start_that_does_not_say_its_gaps_is_refused_where_nothing_was_verified() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let mut silent = start();
    silent
        .context
        .as_object_mut()
        .expect("object")
        .remove("gaps");
    assert!(has_gaps(&silent), "a missing `gaps` counts as a gap");
    let said = by(&root, &clone_key_swapped(), &silent);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}

// ---- the deny state judges a start with every level (M75-07b part 2) -------------------------------

const FORBID_START: &str = "// purlis-policy 1\n@id(\"trust-no-start\")\nforbid (principal, action == Action::\"start\", resource);\n";
/// The project commits a forbid on `start` and switches it off; the You level's file turns it on.
const START_OFF: &str =
    "[policy]\ntemplate = \"strict\"\n\n[policy.rules]\n\"trust-no-start\" = \"off\"\n";
const START_ON: &str = "[policy.rules]\n\"trust-no-start\" = \"enforce\"\n";

fn in_the_deny_state(entries: Vec<Entry>) -> Standing {
    Standing {
        state: State::Deny(vec![Deny::CloneKeySwapped]),
        entries,
        ..Standing::fresh()
    }
}

/// A `start`, as the app asks it, in the plane at `root` with `standing`.
fn start_in(root: &Path, standing: Standing) -> Decision {
    let gaps: Vec<String> = Vec::new();
    let asker = crate::policy::decide::Asker {
        chat: 1,
        of: Chat {
            workspace: None,
            persona: None,
        },
        sandbox: "full",
        attended: true,
        harness_mode: "default",
        depth: 0,
        via: "start",
        start: Some(crate::policy::decide::StartFacts {
            profile: "default",
            gaps: &gaps,
        }),
    };
    crate::policy::decide::in_plane(
        root,
        &asker,
        "start",
        ("Harness", "claude-code"),
        &crate::policy::decide::Sealed {
            standing,
            machine: None,
        },
    )
}

fn local_file(root: &Path) -> PathBuf {
    root.join(crate::settings::Which::Local.file_at(root))
}

#[test]
fn in_the_deny_state_a_you_level_local_forbid_on_start_still_refuses_the_chat() {
    let m = machine();
    let root = plane(
        &m,
        &[("purlis.toml", START_OFF), ("policy.cedar", FORBID_START)],
    );
    // Nothing forbids it while the rule is off.
    let said = start_in(&root, in_the_deny_state(Vec::new()));
    assert_ne!(said.after_mode, Outcome::Deny, "{said:?}");
    // The person's own file turns it on: the chat is refused, with the deny state beside it.
    std::fs::write(local_file(&root), START_ON).expect("the local file");
    let said = start_in(&root, in_the_deny_state(Vec::new()));
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["trust-no-start"], "{said:?}");
}

#[test]
fn in_the_deny_state_a_sealed_local_forbid_on_start_removed_from_the_file_still_refuses_the_chat() {
    let m = machine();
    let root = plane(
        &m,
        &[("purlis.toml", START_OFF), ("policy.cedar", FORBID_START)],
    );
    // The accepted floor holds the project as it was with the local file's tightening in it.
    let held = Version {
        local: Some(START_ON.to_owned()),
        ..version(START_OFF, Some(FORBID_START))
    };
    // Past the cap the store verified, so what it held still judges a start (the entries a deny
    // state sets aside are `kept`).
    let standing = denied_past_the_cap(vec![floor(&held)]);
    // The live local file has none of it: not there at all.
    assert!(!local_file(&root).exists());
    let said = start_in(&root, standing);
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["trust-no-start"], "{said:?}");
    // Without the sealed floor, the same live files let the chat start.
    let said = start_in(&root, in_the_deny_state(Vec::new()));
    assert_ne!(said.after_mode, Outcome::Deny, "{said:?}");
}

#[test]
fn a_raised_cap_stays_held_by_the_floor_until_the_press() {
    let m = machine();
    let raised = "[policy.limits]\n\"dispatch.running-per-chat\" = 12\n";
    let root = plane(&m, &[("purlis.toml", raised)]);
    let key = "dispatch.running-per-chat";
    // Nothing accepted and nothing floored: the shipped 6 stands for the accepted side.
    assert_eq!(limit(&sources(&root), &holding(vec![]), key), Some(6));
    // An earlier committed version, now on the floor, said 2: moving the cap up again drops
    // nothing.
    let tight = version("[policy.limits]\n\"dispatch.running-per-chat\" = 2\n", None);
    assert_eq!(
        limit(&sources(&root), &holding(vec![floor(&tight)]), key),
        Some(2)
    );
    // The accepted copy's own number is what a raise waits on.
    let ten = version(
        "[policy.limits]\n\"dispatch.running-per-chat\" = 10\n",
        None,
    );
    assert_eq!(
        limit(&sources(&root), &holding(vec![accepted(&ten)]), key),
        Some(10)
    );
}

#[test]
fn a_floor_version_that_does_not_read_holds_the_strictest_limit() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let broken = Entry::new(answers::FLOOR, "not json");
    assert_eq!(
        limit(&sources(&root), &holding(vec![broken]), "trust.unread-max"),
        Some(0)
    );
}

// ---- review round 5: a start no chat can have brought to a refusal --------------------------

fn gappy_start() -> Request<'static> {
    let mut gappy = start();
    gappy.context["gaps"] = serde_json::json!(["sandbox", "network"]);
    gappy
}

#[test]
fn a_plane_with_no_store_or_a_keyring_at_fault_starts_with_gaps_as_it_always_did() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let standing_of = |state: State| Standing {
        state,
        ..Standing::fresh()
    };
    let fresh = by(&root, &Standing::fresh(), &gappy_start());
    assert_ne!(fresh.after_mode, Outcome::Deny, "{fresh:?}");
    for state in [
        State::NoKeyring,
        State::Unverifiable,
        State::Deny(vec![Deny::KeyringDidNotStart]),
        State::Deny(vec![Deny::SealKeyLocked]),
    ] {
        let said = by(&root, &standing_of(state.clone()), &gappy_start());
        assert_eq!(said.after_mode, fresh.after_mode, "{state:?}: {said:?}");
        assert!(said.deny_state.is_none(), "{state:?}: {said:?}");
    }
}

#[test]
fn a_keyring_cause_beside_a_cause_a_chat_reaches_still_refuses_a_start_with_gaps() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&m, &[("purlis.toml", open)]);
    let both = Standing {
        state: State::Deny(vec![Deny::KeyringDidNotStart, Deny::CloneKeySwapped]),
        ..Standing::fresh()
    };
    let said = by(&root, &both, &gappy_start());
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}

#[test]
fn a_folder_that_is_no_repository_starts_with_gaps_where_a_broken_git_does_not() {
    let m = machine();
    let open = "[policy]\ntemplate = \"open\"\n";
    let bare = m.dir.path().join("not-a-repository");
    std::fs::create_dir_all(&bare).expect("a folder");
    let bare = std::fs::canonicalize(&bare).expect("a real path");
    std::fs::write(bare.join("purlis.toml"), open).expect("a file");
    let said = by(&bare, &Standing::fresh(), &gappy_start());
    assert_ne!(
        said.after_mode,
        Outcome::Deny,
        "no commit is no refusal: {said:?}"
    );
    // A `.git` that git cannot read is a repository that failed, never one that is not there.
    let broken = m.dir.path().join("broken");
    std::fs::create_dir_all(broken.join(".git")).expect("a broken .git");
    let said = by(&broken, &Standing::fresh(), &gappy_start());
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
}

// ---- #7307 round 2: limits are held across every side, and so is their mode --------------------

use crate::policy::limits::Verdict;

const ANYONE: Chat<'static> = Chat {
    workspace: None,
    persona: None,
};

const CAP: &str = "dispatch.running-per-chat";
const NO_CAP: &str = "dispatch.may-dispatch";

fn limits(text: &str) -> Version {
    version(text, None)
}

fn limit_in(root: &Path, standing: &Standing, key: &str) -> Option<i64> {
    limit(&sources(root), standing, key)
}

fn verdict_in(root: &Path, standing: &Standing, used: i64) -> Verdict {
    limit_verdict(&sources(root), standing, ANYONE, CAP, used)
}

#[test]
fn a_cap_the_accepted_copy_holds_still_holds_past_the_floor_cap() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let accepted_at_3 = accepted(&limits(
        "[policy.limits]\n\"dispatch.running-per-chat\" = 3\n",
    ));
    let kept = denied_past_the_cap(vec![accepted_at_3.clone()]);
    assert_eq!(
        limit_in(&root, &kept, CAP),
        Some(3),
        "from the kept entries"
    );
    assert_eq!(limit_in(&root, &holding(vec![accepted_at_3]), CAP), Some(3));
    // A floor version's number is kept the same way.
    let floor_at_2 = floor(&limits(
        "[policy.limits]\n\"dispatch.running-per-chat\" = 2\n",
    ));
    assert_eq!(
        limit_in(&root, &denied_past_the_cap(vec![floor_at_2]), CAP),
        Some(2)
    );
}

#[test]
fn a_key_with_no_shipped_cap_is_capped_by_a_kept_version_past_the_floor_cap() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    assert_eq!(limit_in(&root, &Standing::fresh(), NO_CAP), None);
    let kept = denied_past_the_cap(vec![accepted(&limits(
        "[policy.limits]\n\"dispatch.may-dispatch\" = 4\n",
    ))]);
    assert_eq!(limit_in(&root, &kept, NO_CAP), Some(4));
}

#[test]
fn an_entry_that_does_not_read_counts_as_the_strictest_limit_never_as_silent() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    for entry in [
        Entry::new(answers::ACCEPTED, "not json"),
        Entry::new(answers::FLOOR, "{\"settings\": 3}"),
    ] {
        for standing in [
            holding(vec![entry.clone()]),
            denied_past_the_cap(vec![entry.clone()]),
        ] {
            assert_eq!(limit_in(&root, &standing, CAP), Some(0), "{entry:?}");
            assert_eq!(limit_in(&root, &standing, NO_CAP), Some(0), "{entry:?}");
            assert_eq!(
                limit_in(&root, &standing, "audit.record-days"),
                Some(i64::MAX),
                "a retention's strictest is the longest"
            );
            assert_eq!(
                verdict_in(&root, &standing, 0),
                Verdict::Refuse,
                "{entry:?}"
            );
        }
    }
}

#[test]
fn a_committed_limits_log_loosens_nothing_while_the_accepted_copy_enforces() {
    let m = machine();
    for mode in ["log", "off"] {
        let root = plane(
            &m,
            &[("purlis.toml", &format!("[policy]\nlimits = \"{mode}\"\n"))],
        );
        let standing = holding(vec![accepted(&version(STRICT, None))]);
        assert_eq!(verdict_in(&root, &standing, 7), Verdict::Refuse, "{mode}");
        assert_eq!(verdict_in(&root, &standing, 6), Verdict::Within, "{mode}");
    }
}

#[test]
fn a_floor_version_that_enforces_holds_a_cap_the_committed_file_logs() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", "[policy]\nlimits = \"log\"\n")]);
    let log = limits("[policy]\nlimits = \"log\"\n");
    let enforcing = floor(&limits("[policy]\ntemplate = \"strict\"\n"));
    let standing = holding(vec![accepted(&log), enforcing]);
    assert_eq!(verdict_in(&root, &standing, 7), Verdict::Refuse);
    // Every side logging counts the cap and refuses nothing.
    let all_log = holding(vec![accepted(&log), floor(&log)]);
    assert_eq!(verdict_in(&root, &all_log, 7), Verdict::Counted);
    assert_eq!(verdict_in(&root, &all_log, 6), Verdict::Within);
}

#[test]
fn a_cap_the_accepted_copy_raised_and_the_committed_file_lowered_is_the_lower() {
    let m = machine();
    let root = plane(
        &m,
        &[(
            "purlis.toml",
            "[policy]\ntemplate = \"strict\"\n[policy.limits]\n\"dispatch.running-per-chat\" = 2\n",
        )],
    );
    let standing = holding(vec![accepted(&limits(
        "[policy]\ntemplate = \"strict\"\n[policy.limits]\n\"dispatch.running-per-chat\" = 10\n",
    ))]);
    assert_eq!(limit_in(&root, &standing, CAP), Some(2));
    assert_eq!(verdict_in(&root, &standing, 3), Verdict::Refuse);
}

#[test]
fn a_key_the_table_does_not_name_is_never_met() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let none = Standing::fresh();
    assert_eq!(limit_in(&root, &none, "no.such-key"), None);
    assert_eq!(
        limit_verdict(&sources(&root), &none, ANYONE, "no.such-key", i64::MAX),
        Verdict::Within
    );
}

#[test]
fn your_own_file_can_tighten_a_limit_and_never_loosen_one() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let local = root.join(crate::settings::Which::Local.file_at(&root));
    std::fs::write(
        &local,
        "[policy.limits]\n\"dispatch.running-per-chat\" = 2\n",
    )
    .expect("the local file");
    assert_eq!(limit_in(&root, &Standing::fresh(), CAP), Some(2));
    std::fs::write(
        &local,
        "[policy.limits]\n\"dispatch.running-per-chat\" = 12\n",
    )
    .expect("the local file");
    assert_eq!(limit_in(&root, &Standing::fresh(), CAP), Some(6));
}
