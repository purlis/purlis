// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An administrator's policy (#1343): what each lock does to a compiled sandbox, the file it is
//! read from, and when that file is refused.

use std::path::{Path, PathBuf};

use super::super::grant::{self, Grants, What};
use super::super::hosts::Host;
use super::super::{Compiled, Machine, Os, Plane, Policy, Preset};
use super::{Locks, Seen, TEST_LOCKS, holding_no_kept_grants, judge};

const FILE: &str = "/etc/purlis/policy.json";

fn locks(json: &str) -> Locks {
    Locks::parse(json, Path::new(FILE))
}

/// The project's sandbox, with its own hosts.
const PROJECT: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\", \
                       \"toolchains\"]\nhosts = [\"api.example.com\", \"build.corp.example\"]\n\n\
                       [sandbox.personas.devops]\nhosts = [\"10.0.0.5:6443\"]\n";

fn machine() -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(PathBuf::from("/home/op")),
        os: Os::MacOs,
    }
}

fn policy() -> Policy {
    Plane::of(Some(PROJECT)).said().policy.expect("on")
}

/// The project compiled for a chat running as `persona` with `chat`'s grants, under `under`.
fn compiled(under: &Locks, persona: Option<&str>, chat: &Grants) -> Compiled {
    let root = tempfile::tempdir().expect("a project");
    // A machine where the person allowed devops's hosts as listed (D-1362-7).
    super::super::persona::allow_every_as_listed(root.path(), &policy());
    TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
    let compiled = Compiled::granted(
        &policy(),
        &Plane::of(Some(PROJECT)),
        root.path(),
        &machine(),
        persona,
        chat,
    );
    TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
    compiled
}

fn has(compiled: &Compiled, host: &str) -> bool {
    compiled.hosts.iter().any(|one| one == host)
}

fn chat_host(host: &str) -> Grants {
    Grants {
        hosts: vec![Host::parse(host).expect("a host")],
        writes: Vec::new(),
        ..Grants::default()
    }
}

// ---- the acceptance: a policy that forbids a project host ---------------------------------------

#[test]
fn a_project_host_the_policy_does_not_allow_is_refused_when_the_sandbox_compiles() {
    let under = locks(r#"{"sandbox": {"hosts": ["*.corp.example"]}}"#);
    let compiled = compiled(&under, None, &Grants::default());
    assert!(has(&compiled, "build.corp.example"), "{:?}", compiled.hosts);
    assert!(!has(&compiled, "api.example.com"), "{:?}", compiled.hosts);
    // The presets' own hosts are the presets' lock, not the hosts'.
    assert!(has(&compiled, "github.com"));
}

#[test]
fn without_a_policy_the_project_s_hosts_compile_as_before() {
    let compiled = compiled(&Locks::none(), None, &Grants::default());
    assert!(has(&compiled, "api.example.com"));
    assert!(has(&compiled, "build.corp.example"));
}

// ---- one compiled test per lock ---------------------------------------------------------------

#[test]
fn fixed_presets_turn_every_other_preset_off_whatever_the_project_says() {
    let under = locks(r#"{"sandbox": {"presets": ["model-providers"]}}"#);
    let compiled = compiled(&under, None, &Grants::default());
    assert!(has(&compiled, "api.anthropic.com"));
    assert!(!has(&compiled, "github.com"), "{:?}", compiled.hosts);
    assert!(!has(&compiled, "registry.npmjs.org"));
    // Package registries off: no package caches widen the sandbox either.
    assert_eq!(compiled.widened.caches, None);
}

#[test]
fn a_fixed_preset_list_never_turns_on_a_preset_the_project_left_off() {
    let under = locks(r#"{"sandbox": {"presets": ["model-providers", "forge"]}}"#);
    assert_eq!(
        under.presets(&[Preset::Forge]),
        [Preset::Forge],
        "the strictest wins: the project's choice, within the policy's"
    );
}

#[test]
fn forbidden_personal_hosts_reach_no_chat_and_the_project_s_still_do() {
    let under = locks(r#"{"sandbox": {"personal-hosts": false}}"#);
    let compiled = compiled(&under, None, &chat_host("tools.example.org"));
    assert!(!has(&compiled, "tools.example.org"), "{:?}", compiled.hosts);
    assert!(has(&compiled, "api.example.com"));
    // …and without the lock, the same grant reaches the chat.
    assert!(has(
        &self::compiled(&Locks::none(), None, &chat_host("tools.example.org")),
        "tools.example.org"
    ));
}

#[test]
fn forbidden_persona_hosts_reach_no_chat_of_the_persona() {
    let under = locks(r#"{"sandbox": {"persona-hosts": false}}"#);
    assert!(!has(
        &compiled(&under, Some("devops"), &Grants::default()),
        "10.0.0.5:6443"
    ));
    assert!(has(
        &compiled(&Locks::none(), Some("devops"), &Grants::default()),
        "10.0.0.5:6443"
    ));
}

#[test]
fn forbidden_write_grants_compile_no_granted_folder() {
    let root = tempfile::tempdir().expect("a project");
    let folder = std::fs::canonicalize(root.path())
        .expect("real")
        .join("out");
    std::fs::create_dir(&folder).expect("a folder");
    let grants = Grants {
        hosts: Vec::new(),
        writes: vec![folder.clone()],
        ..Grants::default()
    };
    let compile = |under: &Locks| {
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
        let compiled = Compiled::granted(
            &policy(),
            &Plane::of(Some(PROJECT)),
            root.path(),
            &machine(),
            None,
            &grants,
        );
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
        compiled
    };
    grant::TEST_HARNESS_TEMP.with(|temp| temp.set(&[]));
    assert_eq!(
        compile(&Locks::none()).writable,
        std::slice::from_ref(&folder)
    );
    assert!(
        compile(&locks(r#"{"sandbox": {"write-grants": false}}"#))
            .writable
            .is_empty()
    );
    grant::TEST_HARNESS_TEMP.with(|temp| temp.set(&grant::HARNESS_TEMP));
}

/// #1927: a folder and a host a person allowed before a policy fixed the scopes are judged
/// against it at every start too. A host kept for the person or for the project is a line of
/// their own hosts, held by `personal-hosts`, `hosts` and `never-hosts`, never by the scopes.
#[test]
fn a_folder_or_a_host_kept_at_a_scope_policy_leaves_out_reaches_no_start() {
    let root = tempfile::tempdir().expect("a project");
    let real = std::fs::canonicalize(root.path()).expect("real");
    let (held, mine) = (real.join("held"), real.join("mine"));
    std::fs::create_dir(&held).expect("a folder");
    std::fs::create_dir(&mine).expect("a folder");
    let home = tempfile::tempdir().expect("a config home");
    super::super::local::TEST_CONFIG_HOME
        .with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    super::super::local::grant_write(&real, &mine).expect("kept");
    let grants = Grants {
        hosts: vec![Host::parse("chat.example").expect("a host")],
        writes: vec![held.clone()],
        ..Grants::default()
    };
    let compile = |under: &Locks| {
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
        let compiled = Compiled::granted(
            &policy(),
            &Plane::of(Some(PROJECT)),
            &real,
            &machine(),
            None,
            &grants,
        );
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
        compiled
    };
    grant::TEST_HARNESS_TEMP.with(|temp| temp.set(&[]));
    let open = compile(&Locks::none());
    assert_eq!(open.writable, [held.clone(), mine.clone()]);
    assert!(has(&open, "chat.example"));
    // This chat's alone: the folder kept for the project on this machine reaches nothing.
    let chat_only = compile(&locks(r#"{"sandbox": {"allow-scopes": ["chat"]}}"#));
    assert_eq!(chat_only.writable, std::slice::from_ref(&held));
    assert!(has(&chat_only, "chat.example"));
    // This project's alone: what was allowed for the one chat reaches nothing.
    let you_only = compile(&locks(r#"{"sandbox": {"allow-scopes": ["you"]}}"#));
    assert_eq!(you_only.writable, std::slice::from_ref(&mine));
    assert!(!has(&you_only, "chat.example"));
    // A host nobody's Allow kept is a line someone wrote in the project's file: the file's,
    // whatever the scopes say.
    assert!(has(&you_only, "build.corp.example"));
    assert!(has(&chat_only, "build.corp.example"));
    // Still kept, for the day the policy lets it back.
    assert_eq!(
        super::super::local::granted_writes(&real),
        std::slice::from_ref(&mine)
    );
    grant::TEST_HARNESS_TEMP.with(|temp| temp.set(&grant::HARNESS_TEMP));
    super::super::local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// #1927 (review): a host an Allow kept, for the person or for the project, is judged against
/// the scopes at every start like anything else an Allow kept. This machine's record of what
/// an Allow kept tells it from a line someone wrote in the file, which stays the file's.
#[test]
fn a_host_an_allow_kept_at_a_scope_policy_leaves_out_reaches_no_start() {
    let host = |name: &str| Host::parse(name).expect("a host");
    let made = |name: &str, level: &str| super::super::local::Made {
        what: "host".to_owned(),
        target: name.to_owned(),
        level: level.to_owned(),
        at: 7,
        chat: None,
    };
    // The rule, on its own: only an Allow's host, only at the scope left out.
    let record = [
        made("kept.example", "you"),
        made("api.example.com", "project"),
    ];
    let mine = [host("kept.example"), host("typed.example")];
    let in_scope = |listed: &[Host], level, under: &Locks| {
        super::super::hosts_in_scope(listed, level, under, &record)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };
    let no_you = locks(r#"{"sandbox": {"allow-scopes": ["chat", "project"]}}"#);
    let no_project = locks(r#"{"sandbox": {"allow-scopes": ["chat", "you"]}}"#);
    for open in [
        Locks::none(),
        locks(r#"{"sandbox": {}}"#),
        no_project.clone(),
    ] {
        assert_eq!(
            in_scope(&mine, grant::Level::You, &open),
            ["kept.example", "typed.example"]
        );
    }
    assert_eq!(
        in_scope(&mine, grant::Level::You, &no_you),
        ["typed.example"]
    );
    // The record is by level: kept for the person is not kept for the project.
    assert_eq!(
        in_scope(&mine, grant::Level::Project, &no_project),
        ["kept.example", "typed.example"]
    );
    let theirs = [host("api.example.com"), host("build.corp.example")];
    assert_eq!(
        in_scope(&theirs, grant::Level::Project, &no_project),
        ["build.corp.example"]
    );
    assert_eq!(
        in_scope(&theirs, grant::Level::Project, &no_you),
        ["api.example.com", "build.corp.example"]
    );
    // A file purlis refuses keeps no scope: nothing an Allow kept.
    let refused = locks(r#"{"sandbox": {"allow-scopes": ["everywhere"]}}"#);
    assert_eq!(
        in_scope(&mine, grant::Level::You, &refused),
        ["typed.example"]
    );

    // And through a start: the project's file lists `api.example.com`, which an Allow on this
    // machine kept for everyone in the project.
    let home = tempfile::tempdir().expect("a config home");
    super::super::local::TEST_CONFIG_HOME
        .with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let root = tempfile::tempdir().expect("a project");
    super::super::local::record_made(root.path(), made("api.example.com", "project"))
        .expect("recorded");
    let start = |under: &Locks| {
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
        let compiled = Compiled::granted(
            &policy(),
            &Plane::of(Some(PROJECT)),
            root.path(),
            &machine(),
            None,
            &Grants::default(),
        );
        let held = [host("api.example.com"), host("build.corp.example")]
            .iter()
            .filter_map(|one| {
                super::super::host_held(root.path(), one, grant::Level::Project, under)
                    .map(|why| (one.to_string(), why))
            })
            .collect::<Vec<_>>();
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
        (compiled, held)
    };
    let (open, held) = start(&Locks::none());
    assert!(has(&open, "api.example.com") && has(&open, "build.corp.example"));
    assert!(held.is_empty());
    let (scoped, held) = start(&locks(
        r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat", "you"]}}"#,
    ));
    assert!(!has(&scoped, "api.example.com"), "{:?}", scoped.hosts);
    assert!(has(&scoped, "build.corp.example"));
    // Said as held, with who set the policy; the line someone wrote is not.
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].0, "api.example.com");
    assert!(
        held[0].1.starts_with("Held by policy") && held[0].1.contains("set by IT"),
        "{}",
        held[0].1
    );
    super::super::local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

#[test]
fn a_forbidden_opt_out_is_refused_with_the_policy_and_its_owner() {
    let under = locks(r#"{"owner": "Platform team", "sandbox": {"opt-out": false}}"#);
    let why = under.opt_out_refused().expect("forbidden");
    assert!(why.contains("Locked by policy"), "{why}");
    assert!(why.contains("Platform team"), "{why}");
    assert!(why.contains(FILE), "{why}");
    assert_eq!(Locks::none().opt_out_refused(), None);
}

#[test]
fn a_grant_policy_forbids_is_refused_with_why_at_every_level_it_forbids() {
    let under = locks(
        r#"{"owner": "IT", "sandbox": {"personal-hosts": false, "write-grants": false,
            "hosts": ["*.corp.example"]}}"#,
    );
    let host = What::Host(Host::parse("a.corp.example").expect("a host"));
    assert!(under.refuses_grant(&host, grant::Level::Chat).is_some());
    assert!(under.refuses_grant(&host, grant::Level::You).is_some());
    assert_eq!(under.refuses_grant(&host, grant::Level::Project), None);
    let elsewhere = What::Host(Host::parse("pastebin.example").expect("a host"));
    let why = under
        .refuses_grant(&elsewhere, grant::Level::Project)
        .expect("not allowed");
    assert!(why.contains("not a host policy allows"), "{why}");
    assert!(why.contains("set by IT"), "{why}");
    let write = What::Write(PathBuf::from("/opt/cache"));
    assert!(under.refuses_grant(&write, grant::Level::Chat).is_some());
}

// ---- reading the file -------------------------------------------------------------------------

#[test]
fn an_allowed_host_covers_names_under_a_wildcard_and_any_port_where_it_names_none() {
    let one = |typed: &str| Host::parse(typed).expect("a host");
    assert!(one("*.corp.example").covers(&one("a.corp.example")));
    assert!(one("*.corp.example").covers(&one("a.b.corp.example:8443")));
    assert!(one("*.corp.example").covers(&one("*.b.corp.example")));
    assert!(one("*.corp.example").covers(&one("*.corp.example")));
    assert!(!one("*.corp.example").covers(&one("corp.example")));
    assert!(!one("*.corp.example").covers(&one("evilcorp.example")));
    assert!(one("api.example.com").covers(&one("api.example.com:8443")));
    assert!(!one("api.example.com:443").covers(&one("api.example.com:8443")));
    assert!(!one("api.example.com").covers(&one("*.api.example.com")));
    assert!(one("10.0.0.5").covers(&one("10.0.0.5:6443")));
}

#[test]
fn a_policy_naming_no_owner_is_set_by_this_machine_s_administrator() {
    let under = locks(r#"{"sandbox": {"opt-out": false}}"#);
    assert_eq!(
        under.locked_by(),
        "Locked by policy, set by this machine's administrator in /etc/purlis/policy.json."
    );
}

#[test]
fn true_and_absent_lock_nothing() {
    let under = locks(
        r#"{"sandbox": {"personal-hosts": true, "persona-hosts": true, "opt-out": true,
            "write-grants": true}}"#,
    );
    assert!(under.any());
    assert!(!under.forbids_personal_hosts());
    assert!(!under.forbids_persona_hosts());
    assert!(!under.forbids_opt_out());
    assert!(!under.forbids_write_grants());
    assert!(!under.forbids_vault_grants());
    assert_eq!(under.vault_grants_refused(), None);
    assert!(!under.fixes_presets());
    assert_eq!(under.allowed_hosts(), None);
}

#[test]
fn a_policy_this_version_cannot_read_is_refused_and_locks_everything() {
    for text in [
        "not json",
        "[]",
        r#"{"sandbox": {"networks": []}}"#,
        r#"{"sandbox": {"presets": ["browser"]}}"#,
        r#"{"sandbox": {"hosts": ["https://a.example/"]}}"#,
        r#"{"sandbox": {"opt-out": "no"}}"#,
        r#"{"owner": 7}"#,
        r#"{"admins": []}"#,
    ] {
        let under = locks(text);
        assert!(under.refused_because().is_some(), "{text}");
        assert!(under.forbids_opt_out(), "{text}");
        assert!(under.forbids_write_grants(), "{text}");
        assert!(under.forbids_vault_grants(), "{text}");
        assert!(under.forbids_personal_hosts(), "{text}");
        assert!(under.forbids_persona_hosts(), "{text}");
        assert_eq!(under.allowed_hosts(), Some(&[][..]), "{text}");
        assert!(under.locked_by().contains("is refused"), "{text}");
    }
}

fn seen(uid: u32, mode: u32) -> Seen {
    Seen {
        link: false,
        expected_type: true,
        uid,
        mode,
    }
}

#[test]
fn only_a_file_and_folder_root_owns_and_no_one_else_writes_is_trusted() {
    assert_eq!(judge(seen(0, 0o100_644), seen(0, 0o40_755)), Ok(()));
    // Owned by the person, or writable by the group or anyone: a chat running as the person
    // could have written it.
    assert!(judge(seen(501, 0o100_644), seen(0, 0o40_755)).is_err());
    assert!(judge(seen(0, 0o100_664), seen(0, 0o40_755)).is_err());
    assert!(judge(seen(0, 0o100_646), seen(0, 0o40_755)).is_err());
    assert!(judge(seen(0, 0o100_644), seen(501, 0o40_755)).is_err());
    assert!(judge(seen(0, 0o100_644), seen(0, 0o40_777)).is_err());
    // A link, the file or its folder: it could be pointed anywhere.
    let link = Seen {
        link: true,
        ..seen(0, 0o120_755)
    };
    assert_eq!(
        judge(link, seen(0, 0o40_755)),
        Err("it is a link".to_owned())
    );
    assert!(judge(seen(0, 0o100_644), link).is_err());
}

#[cfg(unix)]
#[test]
fn a_policy_file_the_person_owns_is_refused_and_locks_everything() {
    let dir = tempfile::tempdir().expect("a folder");
    let file = dir.path().join("policy.json");
    std::fs::write(&file, r#"{"sandbox": {}}"#).expect("written");
    if rustix::process::geteuid().is_root() {
        return;
    }
    let under = Locks::read(&file);
    assert!(
        under
            .refused_because()
            .is_some_and(|why| why.contains("not owned")),
        "{under:?}"
    );
    assert!(under.forbids_opt_out());
}

#[cfg(unix)]
#[test]
fn a_policy_file_that_is_a_link_is_refused() {
    let dir = tempfile::tempdir().expect("a folder");
    let real = dir.path().join("real.json");
    std::fs::write(&real, "{}").expect("written");
    let link = dir.path().join("policy.json");
    std::os::unix::fs::symlink(&real, &link).expect("linked");
    let under = Locks::read(&link);
    assert!(under.refused_because().is_some(), "{under:?}");
}

#[test]
fn no_policy_folder_is_no_policy() {
    let dir = tempfile::tempdir().expect("a folder");
    assert_eq!(
        Locks::read(&dir.path().join("absent").join("policy.json")),
        Locks::none()
    );
}

#[cfg(unix)]
#[test]
fn a_policy_folder_the_person_owns_is_refused_even_with_no_file_in_it() {
    // Whoever owns the folder could have taken the file out of it: never read as no policy.
    let dir = tempfile::tempdir().expect("a folder");
    if rustix::process::geteuid().is_root() {
        return;
    }
    let under = Locks::read(&dir.path().join("policy.json"));
    assert!(
        under
            .refused_because()
            .is_some_and(|why| why.contains("its folder is not owned")),
        "{under:?}"
    );
    assert!(under.forbids_opt_out());
    assert_eq!(under.allowed_hosts(), Some(&[][..]));
}

#[test]
fn every_chat_is_denied_writing_the_policy_folder() {
    let root = tempfile::tempdir().expect("a project");
    let denied = super::super::Denied::of(root.path(), &machine());
    let folder = super::super::real(&super::machine_folder());
    assert!(
        denied.paths.iter().any(|denial| denial.path == folder
            && denial.class == super::super::Class::HumanPowers),
        "{:?}",
        denied.paths
    );
}

#[test]
fn a_person_s_opt_out_is_refused_where_policy_forbids_it_and_the_refusal_names_who() {
    // A project whose manifest is gone reaches the opt-out (D-1410e): no file to write here.
    let gone = tempfile::tempdir().expect("a project");
    let start = |under: &Locks| {
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
        let decided = super::super::decide(
            crate::harness::Harness::ClaudeCode,
            gone.path(),
            &machine(),
            &|_| true,
            Some(&super::super::OptOut { reason: None }),
            None,
        );
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
        decided
    };
    assert!(matches!(
        start(&Locks::none()),
        Ok(Some(super::super::Decided::Unsandboxed(_)))
    ));
    let under = locks(r#"{"owner": "Platform team", "sandbox": {"opt-out": false}}"#);
    let refused = start(&under).expect_err("refused");
    assert_eq!(
        refused,
        super::super::NotStarted::OptOutLocked(under.opt_out_refused().expect("locked"))
    );
    let said = refused.to_string();
    assert!(said.contains("set by Platform team"), "{said}");
    assert!(said.ends_with("Nothing was started."), "{said}");
}

// ---- a forge's host is the project's choice, and the hosts lock holds it (D-1343-10) ----------

const FORGES: &str = "[sandbox]\nmode = \"on\"\negress = [\"forge\"]\n\n[[forge]]\nkind = \"gitlab\"\n\
                      host = \"pastebin.example\"\n\n[[forge]]\nkind = \"gitlab\"\n\
                      host = \"git.corp.example\"\n";

/// What a chat in a project saying [`FORGES`] reaches, under `under`.
fn forged(under: &Locks) -> Vec<String> {
    let root = tempfile::tempdir().expect("a project");
    let plane = Plane::of(Some(FORGES));
    TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
    let compiled = Compiled::granted(
        &plane.said().policy.expect("on"),
        &plane,
        root.path(),
        &machine(),
        None,
        &Grants::default(),
    );
    TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
    compiled.hosts
}

#[test]
fn a_forge_host_policy_does_not_allow_is_not_reached() {
    let reached = forged(&locks(r#"{"sandbox": {"hosts": []}}"#));
    assert!(
        !reached.iter().any(|one| one == "pastebin.example"),
        "{reached:?}"
    );
    assert!(
        !reached.iter().any(|one| one == "git.corp.example"),
        "{reached:?}"
    );
    // The preset's own fixed list is the presets lock's, not the hosts'.
    assert!(reached.iter().any(|one| one == "github.com"), "{reached:?}");

    let reached = forged(&locks(r#"{"sandbox": {"hosts": ["*.corp.example"]}}"#));
    assert!(
        reached.iter().any(|one| one == "git.corp.example"),
        "{reached:?}"
    );
    assert!(
        !reached.iter().any(|one| one == "pastebin.example"),
        "{reached:?}"
    );

    let reached = forged(&Locks::none());
    assert!(
        reached.iter().any(|one| one == "pastebin.example"),
        "{reached:?}"
    );
}

#[test]
fn under_a_refused_policy_file_the_presets_stay_and_no_forge_host_is_reached() {
    let refused = locks("not json");
    assert!(refused.refused_because().is_some());
    let reached = forged(&refused);
    assert!(reached.iter().any(|one| one == "github.com"), "{reached:?}");
    assert!(
        !reached.iter().any(|one| one == "pastebin.example"),
        "{reached:?}"
    );
    assert!(
        !reached.iter().any(|one| one == "git.corp.example"),
        "{reached:?}"
    );
}

#[test]
fn vault_grants_is_a_lock_of_its_own() {
    let under = locks(r#"{"owner": "IT", "sandbox": {"vault-grants": false}}"#);
    assert!(under.forbids_vault_grants());
    assert!(!under.forbids_write_grants(), "a folder is another lock's");
    assert!(!under.forbids_persona_hosts(), "and so is a persona's host");
    assert_eq!(
        under.vault_grants_refused().as_deref(),
        Some(
            "Policy forbids allowing a persona a vault it is not tagged for. Locked by policy, \
             set by IT in /etc/purlis/policy.json."
        )
    );
    assert!(!locks(r#"{"sandbox": {"write-grants": false}}"#).forbids_vault_grants());
}

// ---- a policy that forbids the opt-out requires the sandbox (D-1423-1) -------------------------

const REQUIRED: &str = r#"{"owner": "Platform team", "sandbox": {"opt-out": false}}"#;

/// What a chat in a project whose manifest says `text` reaches under `under`: `None` where no
/// sandbox is in force there.
fn reached(text: &str, under: &Locks) -> Option<Vec<String>> {
    let root = tempfile::tempdir().expect("a project");
    let plane = Plane::of(Some(text));
    let policy = plane.in_force(under)?;
    TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
    let compiled = Compiled::granted(
        &policy,
        &plane,
        root.path(),
        &machine(),
        None,
        &Grants::default(),
    );
    TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
    Some(compiled.hosts)
}

#[test]
fn a_project_with_no_sandbox_runs_sandboxed_with_the_default_presets_where_policy_requires_it() {
    let none = "schema = 1\n";
    assert_eq!(reached(none, &Locks::none()), None, "its own choice");
    // A policy that locks something else requires nothing.
    assert_eq!(
        reached(none, &locks(r#"{"sandbox": {"write-grants": false}}"#)),
        None
    );
    let hosts = reached(none, &locks(REQUIRED)).expect("sandboxed");
    for preset in Preset::DEFAULT {
        for host in preset.own_hosts() {
            assert!(hosts.iter().any(|one| one == host), "{host}: {hosts:?}");
        }
    }
}

#[test]
fn a_project_whose_sandbox_is_off_runs_sandboxed_as_if_it_had_turned_it_on() {
    // `[sandbox]` with no mode has not turned it on: what it says applies once policy does.
    let off = "[sandbox]\negress = [\"model-providers\"]\nhosts = [\"api.example.com\"]\n";
    assert_eq!(reached(off, &Locks::none()), None);
    let hosts = reached(off, &locks(REQUIRED)).expect("sandboxed");
    assert!(
        hosts.iter().any(|one| one == "api.anthropic.com"),
        "{hosts:?}"
    );
    assert!(
        hosts.iter().any(|one| one == "api.example.com"),
        "{hosts:?}"
    );
    assert!(!hosts.iter().any(|one| one == "github.com"), "{hosts:?}");
}

#[test]
fn the_policy_s_own_presets_and_hosts_hold_the_sandbox_it_requires() {
    let under =
        locks(r#"{"sandbox": {"opt-out": false, "presets": ["model-providers"], "hosts": []}}"#);
    let hosts = reached("schema = 1\n", &under).expect("sandboxed");
    assert!(
        hosts.iter().any(|one| one == "api.anthropic.com"),
        "{hosts:?}"
    );
    assert!(!hosts.iter().any(|one| one == "github.com"), "{hosts:?}");
    assert!(!hosts.iter().any(|one| one == "crates.io"), "{hosts:?}");
    let off = "[sandbox]\nhosts = [\"api.example.com\"]\n";
    let hosts = reached(off, &under).expect("sandboxed");
    assert!(
        !hosts.iter().any(|one| one == "api.example.com"),
        "{hosts:?}"
    );
}

#[test]
fn a_refused_policy_file_requires_the_sandbox_too() {
    let hosts = reached("schema = 1\n", &locks("not json")).expect("sandboxed");
    assert!(
        hosts.iter().any(|one| one == "api.anthropic.com"),
        "{hosts:?}"
    );
}

#[test]
fn settings_is_told_the_sandbox_is_on_and_required_and_by_whom() {
    assert_eq!(
        locks(REQUIRED).required_by().as_deref(),
        Some("On, required by policy, set by Platform team in /etc/purlis/policy.json.")
    );
    assert_eq!(Locks::none().required_by(), None);
    assert_eq!(locks(r#"{"sandbox": {"hosts": []}}"#).required_by(), None);
    let refused = locks("not json").required_by().expect("required");
    assert!(refused.starts_with("On, required by policy: "), "{refused}");
    assert!(refused.contains(FILE), "{refused}");
}

/// What a chat of Claude Code starts under in the project at `root` on `os`, under `under`.
fn decided(
    root: &Path,
    os: Os,
    under: &Locks,
) -> Result<Option<super::super::Decided>, super::super::NotStarted> {
    TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
    let decided = super::super::decide(
        crate::harness::Harness::ClaudeCode,
        root,
        &Machine { os, ..machine() },
        &|_| true,
        None,
        None,
    );
    TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
    decided
}

#[test]
fn a_system_with_no_backend_refuses_the_chat_where_policy_requires_the_sandbox() {
    use super::super::{By, Decided, Lifted, NotStarted};
    // A project whose manifest is gone reaches the decision (D-1410e): no file to write here.
    let project = tempfile::tempdir().expect("a project");
    // With no policy purlis starts it without the sandbox, as itself (ruling V21 3).
    assert_eq!(
        decided(project.path(), Os::Windows, &Locks::none()),
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::NoBackend(Os::Windows),
            reason: None,
        })))
    );
    // Under a policy that requires it, never unconfined.
    let under = locks(REQUIRED);
    let refused = decided(project.path(), Os::Windows, &under).expect_err("refused");
    assert_eq!(refused, NotStarted::RequiredWithoutBackend(Os::Windows));
    assert_eq!(
        refused.said(&under),
        "purlis has no sandbox backend on Windows yet, and policy requires the sandbox for \
         every chat on this machine, so nothing was started. Policy forbids starting a chat \
         without the sandbox. Locked by policy, set by Platform team in /etc/purlis/policy.json."
    );
}

#[test]
fn a_project_with_no_sandbox_starts_its_chats_sandboxed_where_policy_requires_it() {
    use super::super::Decided;
    let project = tempfile::tempdir().expect("a project");
    let project = project.path().canonicalize().expect("a real folder");
    std::fs::write(project.join(crate::plane::MANIFEST), "schema = 1\n").expect("a manifest");
    // #2071: the floor, where no policy requires the full sandbox.
    assert!(matches!(
        decided(&project, Os::MacOs, &Locks::none()),
        Ok(Some(Decided::Sandboxed(ref applied))) if applied.level() == super::super::Level::Floor
    ));
    assert!(matches!(
        decided(&project, Os::MacOs, &locks(REQUIRED)),
        Ok(Some(Decided::Sandboxed(_)))
    ));
    // And on a system with no backend it is refused, whatever its file says.
    assert_eq!(
        decided(&project, Os::Windows, &locks(REQUIRED)),
        Err(super::super::NotStarted::RequiredWithoutBackend(
            Os::Windows
        ))
    );
    assert!(matches!(
        decided(&project, Os::Windows, &Locks::none()),
        Ok(Some(Decided::Floorless(_)))
    ));
}

// ---- no refusal sends the person to an opt-out policy forbids (#1423) --------------------------

/// Every refusal whose way out names the opt-out.
fn refusals_that_name_the_opt_out() -> Vec<super::super::NotStarted> {
    use super::super::{Class, Named, NotStarted, Service, Uncompilable, Unheld};
    let named = Named {
        file: PathBuf::from("/p/.claude/settings.json"),
        word: "./run.sh".to_owned(),
    };
    vec![
        NotStarted::HeldBack(crate::harness::Harness::Codex, 1150),
        NotStarted::CoversItsGround {
            path: PathBuf::from("/p"),
            class: Class::LaterCode,
            named: Some(named.clone()),
            within: None,
        },
        NotStarted::CoversItsGround {
            path: PathBuf::from("/p"),
            class: Class::LaterCode,
            named: None,
            within: None,
        },
        NotStarted::Unread(named),
        NotStarted::Uncompilable(Uncompilable {
            harness: crate::harness::Harness::ClaudeCode,
            unheld: Unheld::Service(Service::CredentialStore, Os::Linux),
        }),
    ]
}

#[test]
fn a_refusal_names_the_policy_and_its_owner_instead_of_an_opt_out_policy_forbids() {
    let under = locks(REQUIRED);
    for refusal in refusals_that_name_the_opt_out() {
        let open = refusal.said(&Locks::none());
        assert_eq!(open, refusal.to_string());
        assert!(open.contains("without the sandbox"), "{open}");
        let locked = refusal.said(&under);
        assert!(
            !locked.contains("new-chat picker") && !locked.contains("tart this chat without"),
            "{locked}"
        );
        // It is policy, not the project, that requires the sandbox here (#1431).
        if open.starts_with(super::super::LEAD) {
            assert!(locked.starts_with(super::super::POLICY_LEAD), "{locked}");
        }
        assert!(!locked.contains(super::super::LEAD), "{locked}");
        assert!(
            locked.ends_with(
                "Policy forbids starting a chat without the sandbox. Locked by policy, set by \
                 Platform team in /etc/purlis/policy.json."
            ),
            "{locked}"
        );
    }
    // The fix that is still the person's stays.
    let keyring = refusals_that_name_the_opt_out()
        .pop()
        .expect("the keyring refusal");
    assert!(
        keyring
            .said(&under)
            .contains("Move those secrets to a plain-file or 1Password vault"),
        "{}",
        keyring.said(&under)
    );
}

#[test]
fn the_picker_s_refusal_names_no_opt_out_policy_forbids() {
    use super::super::Ahead;
    // A project whose manifest is gone reaches the decision (D-1410e): no file to write here.
    let project = tempfile::tempdir().expect("a project");
    let ahead = |under: &Locks| {
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = under.clone());
        let ahead = super::super::ahead(
            crate::harness::Harness::ClaudeCode,
            project.path(),
            &Machine {
                os: Os::Windows,
                ..machine()
            },
            &|_| true,
            "",
            &|_| Ok(()),
        );
        TEST_LOCKS.with(|locks| *locks.borrow_mut() = Locks::none());
        ahead
    };
    assert!(matches!(ahead(&Locks::none()), Ahead::Unsandboxed(_)));
    let Ahead::Refused { why, install } = ahead(&locks(REQUIRED)) else {
        panic!("a chat that cannot be sandboxed is refused under the policy");
    };
    assert_eq!(install, None);
    // The picker says what locks it on a line of its own, so the policy is not said twice.
    assert_eq!(
        why,
        "purlis has no sandbox backend on Windows yet, and policy requires the sandbox for \
         every chat on this machine, so nothing was started."
    );
}

#[test]
fn a_folder_refusal_names_the_policy_instead_of_an_opt_out_policy_forbids() {
    use super::super::FolderRefusal;
    let under = locks(REQUIRED);
    for why in [
        FolderRefusal::Missing,
        FolderRefusal::Linked,
        FolderRefusal::Outside,
    ] {
        let open = why.said(&Locks::none());
        assert!(
            open.ends_with("; Start without the sandbox is yours to pick when you start it."),
            "{open}"
        );
        assert!(open.starts_with(super::super::LEAD), "{open}");
        let locked = why.said(&under);
        assert!(locked.starts_with(super::super::POLICY_LEAD), "{locked}");
        assert!(!locked.contains("yours to pick"), "{locked}");
        assert!(!locked.contains("without the sandbox is"), "{locked}");
        assert!(locked.contains("nothing was started"), "{locked}");
        assert!(
            locked.ends_with(&format!(". {POLICY_SENTENCE}")),
            "{locked}"
        );
    }
}

// ---- every refusal a chat's line gives is said under the lock (#1423, review M1) ---------------

/// How every refusal ends under [`REQUIRED`].
const POLICY_SENTENCE: &str = "Policy forbids starting a chat without the sandbox. Locked by \
                               policy, set by Platform team in /etc/purlis/policy.json.";

/// This thread's locks until it is dropped, a panic included.
struct Under;

impl Under {
    fn these(locks: &Locks) -> Self {
        TEST_LOCKS.with(|held| *held.borrow_mut() = locks.clone());
        Self
    }
}

impl Drop for Under {
    fn drop(&mut self) {
        TEST_LOCKS.with(|held| *held.borrow_mut() = Locks::none());
    }
}

/// Why a Claude Code chat's line is refused in `cwd` of the project at `root`, under `under`.
fn line_refused(root: &Path, cwd: &Path, under: &Locks) -> String {
    let _under = Under::these(under);
    let applied = super::super::applied_for(
        crate::harness::Harness::ClaudeCode,
        &policy(),
        &Plane::of(Some(PROJECT)),
        root,
        &machine(),
    )
    .expect("compiles");
    applied
        .line(
            super::super::Words {
                program: "claude".to_owned(),
                command: Vec::new(),
                armed: Vec::new(),
                charters: Vec::new(),
            },
            &super::super::At {
                cwd: Some(cwd),
                ..super::super::At::default()
            },
        )
        .expect_err("refused")
}

/// A refusal under [`REQUIRED`] says it is policy that requires the sandbox (#1431), names no
/// opt-out, and ends with the policy and who set it.
fn names_the_policy_and_no_opt_out(said: &str) {
    assert!(said.starts_with(super::super::POLICY_LEAD), "{said}");
    assert!(!said.contains(super::super::LEAD), "{said}");
    assert!(said.ends_with(POLICY_SENTENCE), "{said}");
    let before = said.strip_suffix(POLICY_SENTENCE).unwrap_or(said);
    assert!(!before.contains("without the sandbox"), "{said}");
    assert!(!said.contains("new-chat picker"), "{said}");
    assert!(!said.contains("yours to pick"), "{said}");
}

#[cfg(unix)]
#[test]
fn a_profile_chat_in_a_linked_folder_is_refused_naming_the_policy_and_no_opt_out() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let elsewhere = tempfile::tempdir().expect("somewhere outside");
    let linked = root.join("swapped");
    std::os::unix::fs::symlink(elsewhere.path(), &linked).expect("a link");

    let open = line_refused(&root, &linked, &Locks::none());
    assert_eq!(
        open,
        super::super::FolderRefusal::Linked.said(&Locks::none())
    );
    assert!(open.contains("Start without the sandbox"), "{open}");

    let locked = line_refused(&root, &linked, &locks(REQUIRED));
    assert!(
        locked.contains("is a link or not a real folder"),
        "{locked}"
    );
    names_the_policy_and_no_opt_out(&locked);
}

#[cfg(unix)]
#[test]
fn a_manifest_covering_the_chat_s_ground_is_refused_naming_the_policy_and_no_opt_out() {
    // #1336: a manifest in the chat's folder that is a link to that folder would leave the
    // chat unable to write where it works.
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).expect("a folder");
    std::os::unix::fs::symlink(&sub, sub.join("purlis.local.toml")).expect("a link");

    let open = line_refused(&root, &sub, &Locks::none());
    assert!(open.contains("purlis.local.toml"), "{open}");
    assert!(
        open.ends_with("Start this chat without the sandbox from the new-chat picker."),
        "{open}"
    );

    let locked = line_refused(&root, &sub, &locks(REQUIRED));
    assert!(
        locked.contains("purlis.local.toml") && locked.contains("nothing was started"),
        "{locked}"
    );
    names_the_policy_and_no_opt_out(&locked);
}

#[test]
fn a_wrap_that_cannot_be_written_names_no_opt_out_policy_forbids() {
    use super::super::seatbelt;
    let lead = "this project runs every chat sandboxed, and";
    let open = seatbelt::not_started(lead, seatbelt::TOO_LARGE, true);
    assert!(
        open.ends_with("or start this chat without the sandbox from the new-chat picker."),
        "{open}"
    );
    for why in [seatbelt::TOO_LARGE, seatbelt::CONTROL] {
        let locked = seatbelt::not_started(lead, why, false);
        assert!(locked.contains("so nothing was started."), "{locked}");
        assert!(!locked.contains("without the sandbox"), "{locked}");
        assert!(!locked.contains("new-chat picker"), "{locked}");
    }
    // What is still the person's to change stays.
    assert!(
        seatbelt::not_started(lead, seatbelt::TOO_LARGE, false)
            .ends_with("so nothing was started. Have the configs name fewer scripts."),
    );
}

/// A wrapped harness's own refusal goes through the line, which tells the adapter the opt-out
/// is locked and ends what it says with the policy: here a path the profile cannot state
/// (`seatbelt::not_started`), for Codex and opencode both.
#[test]
fn a_seatbelt_refusal_at_the_line_ends_with_the_policy_and_names_no_opt_out() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    // A folder whose name holds a control character: no rule can state it exactly.
    let cwd = root.join("a\nb");
    std::fs::create_dir_all(&cwd).expect("a folder");
    let under = locks(REQUIRED);
    for harness in [
        crate::harness::Harness::Codex,
        crate::harness::Harness::Opencode,
    ] {
        let refused = |locks: &Locks| {
            let _under = Under::these(locks);
            let applied = super::super::applied_for(
                harness,
                &policy(),
                &Plane::of(Some(PROJECT)),
                &root,
                &machine(),
            )
            .expect("compiles");
            let confinement = applied.confine().expect("confined");
            applied
                .line(
                    super::super::Words {
                        program: "harness".to_owned(),
                        command: Vec::new(),
                        armed: Vec::new(),
                        charters: Vec::new(),
                    },
                    &super::super::At {
                        cwd: Some(&cwd),
                        confinement: confinement.as_ref(),
                        ..super::super::At::default()
                    },
                )
                .expect_err("refused")
        };
        let open = refused(&Locks::none());
        assert!(open.contains(super::super::seatbelt::CONTROL), "{open}");
        let locked = refused(&under);
        assert!(
            locked.contains(super::super::seatbelt::CONTROL),
            "{harness:?}: {locked}"
        );
        names_the_policy_and_no_opt_out(&locked);
    }
}

// ---- the hosts a teammate is told of are the hosts a chat reaches (#1423) -----------------------

#[test]
fn the_hosts_changed_notice_names_no_host_policy_locks_out() {
    let plane = Plane::of(Some(
        "[sandbox]\nmode = \"on\"\nhosts = [\"api.example.com\", \"build.corp.example\"]\n\n\
         [sandbox.personas.devops]\nhosts = [\"10.0.0.5:6443\"]\n\n\
         [[forge]]\nkind = \"gitlab\"\nhost = \"pastebin.example\"\n\n\
         [[forge]]\nkind = \"gitlab\"\nhost = \"git.corp.example\"\n",
    ));
    assert_eq!(
        plane.granted_hosts(&Locks::none()),
        [
            "api.example.com",
            "build.corp.example",
            "pastebin.example",
            "git.corp.example",
        ]
    );
    // A hosts list holds the project's own and its forges'. A persona's are never told here:
    // they are asked for apart (D-1362-7).
    assert_eq!(
        plane.granted_hosts(&locks(
            r#"{"sandbox": {"hosts": ["*.corp.example"], "persona-hosts": false}}"#
        )),
        ["build.corp.example", "git.corp.example"]
    );
    // A forge's host reaches nothing while policy turns the forge preset off.
    assert_eq!(
        plane.granted_hosts(&locks(r#"{"sandbox": {"presets": ["model-providers"]}}"#)),
        ["api.example.com", "build.corp.example"]
    );
    // In a project that has not turned the sandbox on, nothing is told until policy requires it.
    let off = Plane::of(Some("[sandbox]\nhosts = [\"api.example.com\"]\n"));
    assert_eq!(off.granted_hosts(&Locks::none()), Vec::<String>::new());
    assert_eq!(off.granted_hosts(&locks(REQUIRED)), ["api.example.com"]);
}

// ---- the lead says who requires the sandbox (#1431) ---------------------------------------------

#[test]
fn a_refusal_s_lead_says_policy_where_policy_requires_the_sandbox() {
    use super::super::{LEAD, POLICY_LEAD, under_policy};
    let why = format!("{LEAD}, and the egress proxy was not started, so nothing was started.");
    assert_eq!(under_policy(&Locks::none(), why.clone()), why);
    // A lock that does not require the sandbox leaves the project's lead.
    assert_eq!(
        under_policy(
            &locks(r#"{"sandbox": {"write-grants": false}}"#),
            why.clone()
        ),
        why
    );
    assert_eq!(
        under_policy(&locks(REQUIRED), why),
        format!(
            "{POLICY_LEAD}, and the egress proxy was not started, so nothing was started. \
             {POLICY_SENTENCE}"
        )
    );
    // A sentence led otherwise keeps its own words, and still ends with the policy.
    assert_eq!(
        under_policy(&locks(REQUIRED), "purlis could not start it.".to_owned()),
        format!("purlis could not start it. {POLICY_SENTENCE}")
    );
}

// ---- the live ask (#1666): asks off, scopes removed, hosts pinned never allowed ---------------

#[test]
fn policy_can_turn_live_asks_off_and_says_so() {
    assert!(!Locks::none().forbids_live_asks());
    assert_eq!(Locks::none().live_asks_refused(), None);
    let under = locks(r#"{"owner": "IT", "sandbox": {"live-asks": false}}"#);
    assert!(under.forbids_live_asks());
    let why = under.live_asks_refused().expect("off");
    assert!(why.contains("Policy turns off asking"), "{why}");
    assert!(why.contains("set by IT"), "{why}");
    assert!(!locks(r#"{"sandbox": {"live-asks": true}}"#).forbids_live_asks());
}

#[test]
fn policy_can_remove_an_allow_scope_and_says_which() {
    let under = locks(r#"{"owner": "IT", "sandbox": {"allow-scopes": ["you"]}}"#);
    let host = What::Host(Host::parse("api.example.com").expect("a host"));
    assert_eq!(under.refuses_grant(&host, grant::Level::You), None);
    let chat = under
        .refuses_grant(&host, grant::Level::Chat)
        .expect("removed");
    assert!(
        chat.contains("Policy removed Allow for this chat"),
        "{chat}"
    );
    assert!(chat.contains("set by IT"), "{chat}");
    let project = under
        .refuses_grant(&host, grant::Level::Project)
        .expect("removed");
    assert!(
        project.contains("Policy removed Allow for everyone in this project"),
        "{project}"
    );
    // Absent, every scope stays.
    assert_eq!(
        locks(r#"{"sandbox": {}}"#).refuses_grant(&host, grant::Level::Chat),
        None
    );
}

/// The operator's ruling C3 of 2026-10-13: `machine` is a policy's word for "on this machine,
/// for every project". Named, an Allow may keep a socket or a service there; a policy that
/// fixes the scopes and leaves it out removes it, as it removes `you` where it leaves that
/// out, and neither word stands in for the other.
#[test]
fn a_policy_names_the_every_project_scope_by_a_word_of_its_own() {
    let socket = What::Socket(PathBuf::from("/var/run/docker.sock"));
    let service = What::Service("com.example.helper".to_owned());
    let named =
        locks(r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat", "you", "machine"]}}"#);
    assert_eq!(named.refused_because(), None, "a word purlis knows");
    assert_eq!(named.refuses_grant(&socket, grant::Level::Machine), None);
    assert_eq!(named.refuses_grant(&service, grant::Level::Machine), None);
    assert!(named.keeps_scope(grant::Level::Machine));

    let left_out = locks(r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat", "you"]}}"#);
    assert!(!left_out.keeps_scope(grant::Level::Machine));
    assert!(left_out.keeps_scope(grant::Level::You));
    let why = left_out
        .refuses_grant(&socket, grant::Level::Machine)
        .expect("left out");
    assert!(
        why.contains("Policy removed Allow on this machine, for every project"),
        "{why}"
    );
    assert!(why.contains("set by IT"), "{why}");
    assert_eq!(
        left_out.scope_removed(grant::Level::Machine).as_deref(),
        Some(why.as_str()),
        "one sentence, with or without a target to name"
    );

    // Neither word stands in for the other.
    let alone = locks(r#"{"sandbox": {"allow-scopes": ["machine"]}}"#);
    assert_eq!(alone.refuses_grant(&socket, grant::Level::Machine), None);
    assert!(alone.refuses_grant(&socket, grant::Level::You).is_some());
    assert!(alone.refuses_grant(&socket, grant::Level::Chat).is_some());

    // A policy that fixes no scopes leaves every scope, this one too; so does no policy.
    for open in [locks(r#"{"sandbox": {}}"#), Locks::none()] {
        assert_eq!(open.refuses_grant(&socket, grant::Level::Machine), None);
        assert!(open.keeps_scope(grant::Level::Machine));
        assert_eq!(open.scope_removed(grant::Level::Machine), None);
    }

    // The word widens nothing else: a host and a folder are never every project's, and the
    // lock on sockets and services holds whatever the scopes say.
    let host = What::Host(Host::parse("api.example.com").expect("a host"));
    assert!(named.refuses_grant(&host, grant::Level::Machine).is_some());
    assert!(
        named
            .refuses_grant(
                &What::Write(PathBuf::from("/opt/cache")),
                grant::Level::Machine
            )
            .is_some()
    );
    let locked = locks(
        r#"{"sandbox": {"socket-grants": false, "allow-scopes": ["chat", "you", "machine"]}}"#,
    );
    assert!(
        locked
            .refuses_grant(&socket, grant::Level::Machine)
            .is_some()
    );

    // A file purlis refuses keeps no scope at all, this one included.
    let refused = locks(r#"{"sandbox": {"allow-scopes": ["everywhere"]}}"#);
    assert!(refused.refused_because().is_some());
    assert!(!refused.keeps_scope(grant::Level::Machine));
    assert!(
        refused
            .refuses_grant(&socket, grant::Level::Machine)
            .is_some()
    );
}

/// #1927 (review): `once` is a word of `allow-scopes` like the others. A list that leaves it
/// out removes "Allow once", and a Notice says which scopes the policy removed and who set it,
/// each named once, so an answer is never only missing.
#[test]
fn the_scopes_a_policy_removed_are_said_each_by_name() {
    use grant::Level::{Chat, Machine, Once, You};
    let offered = [Once, Machine, Chat, You];
    assert_eq!(Locks::none().scopes_removed(&offered), None);
    assert_eq!(locks(r#"{"sandbox": {}}"#).scopes_removed(&offered), None);
    let all = locks(r#"{"sandbox": {"allow-scopes": ["once", "chat", "you", "machine"]}}"#);
    assert_eq!(all.refused_because(), None, "once is a word purlis knows");
    assert_eq!(all.scopes_removed(&offered), None);
    let under = locks(r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat", "you"]}}"#);
    assert_eq!(
        under.scopes_removed(&offered).as_deref(),
        Some(
            "Policy removed Allow once, for this chat's next run; Allow on this machine, for \
             every project. Locked by policy, set by IT in /etc/purlis/policy.json."
        )
    );
    assert_eq!(under.scopes_removed(&[Chat, You]), None);
}

#[test]
fn a_host_policy_pins_as_never_allowed_is_refused_at_every_scope() {
    let under = locks(r#"{"owner": "IT", "sandbox": {"never-hosts": ["*.paste.example"]}}"#);
    let pinned = What::Host(Host::parse("drop.paste.example:443").expect("a host"));
    for level in [grant::Level::Chat, grant::Level::You, grant::Level::Project] {
        let why = under.refuses_grant(&pinned, level).expect("pinned");
        assert!(why.contains("Policy never allows"), "{why}");
    }
    let host = Host::parse("drop.paste.example").expect("a host");
    assert!(under.never_allows(&host).is_some());
    assert_eq!(
        under.never_allows(&Host::parse("api.example.com").expect("a host")),
        None
    );
    let other = What::Host(Host::parse("api.example.com").expect("a host"));
    assert_eq!(under.refuses_grant(&other, grant::Level::You), None);
}

#[test]
fn a_project_host_policy_pins_never_allowed_does_not_compile() {
    let under = locks(r#"{"sandbox": {"never-hosts": ["api.example.com"]}}"#);
    let compiled = compiled(&under, None, &Grants::default());
    assert!(!has(&compiled, "api.example.com"), "{:?}", compiled.hosts);
    assert!(has(&compiled, "build.corp.example"));
}

#[test]
fn a_policy_s_new_keys_that_cannot_be_read_refuse_the_file_and_turn_asks_off() {
    for text in [
        r#"{"sandbox": {"live-asks": "no"}}"#,
        r#"{"sandbox": {"allow-scopes": ["everyone"]}}"#,
        r#"{"sandbox": {"never-hosts": ["https://a.example/"]}}"#,
    ] {
        let under = locks(text);
        assert!(under.refused_because().is_some(), "{text}");
        assert!(under.forbids_live_asks(), "{text}");
    }
}

// ---- persona tools (#1407, #1362 acceptance 5) ----------------------------------------------

#[test]
fn policy_can_forbid_persona_tools_and_names_itself_and_the_tool() {
    let under = locks(r#"{"owner": "IT", "sandbox": {"persona-tools": false}}"#);
    assert!(under.forbids_persona_tools());
    let why = under.refuses_tool("devops", "kubectl").expect("forbidden");
    assert!(why.contains("kubectl"), "{why}");
    assert!(why.contains("devops"), "{why}");
    assert!(why.contains("Locked by policy, set by IT"), "{why}");
}

#[test]
fn persona_tools_are_locked_by_nothing_where_the_key_is_absent_or_true() {
    for text in [
        r#"{"sandbox": {}}"#,
        r#"{"sandbox": {"persona-tools": true}}"#,
        r#"{"sandbox": {"persona-hosts": false}}"#,
    ] {
        let under = locks(text);
        assert!(!under.forbids_persona_tools(), "{text}");
        assert_eq!(under.refuses_tool("devops", "kubectl"), None, "{text}");
    }
    assert_eq!(Locks::none().refuses_tool("devops", "kubectl"), None);
}

#[test]
fn a_refused_policy_file_forbids_persona_tools_too() {
    for text in ["not json", r#"{"sandbox": {"persona-tools": "no"}}"#] {
        let under = locks(text);
        assert!(under.refused_because().is_some(), "{text}");
        assert!(under.forbids_persona_tools(), "{text}");
        assert!(under.refuses_tool("devops", "kubectl").is_some(), "{text}");
    }
}

/// #1975, PC-14: `"off"` turns proposed changes off; `"on"` and absent leave them on; any
/// other word refuses the file, closed, which turns them off too.
#[test]
fn proposed_changes_are_turned_off_by_the_word_off_and_by_a_refused_file() {
    assert!(
        locks(r#"{"sandbox": {}}"#)
            .proposed_changes_refused()
            .is_none()
    );
    assert!(
        locks(r#"{"sandbox": {"proposed-changes": "on"}}"#)
            .proposed_changes_refused()
            .is_none()
    );
    let off = locks(r#"{"owner": "Platform", "sandbox": {"proposed-changes": "off"}}"#);
    let said = off.proposed_changes_refused().expect("off");
    assert!(
        said.starts_with("Policy turns proposed changes off"),
        "{said}"
    );
    for refused in [
        r#"{"sandbox": {"proposed-changes": false}}"#,
        r#"{"sandbox": {"proposed-changes": "never"}}"#,
    ] {
        assert!(
            locks(refused).proposed_changes_refused().is_some(),
            "{refused}"
        );
    }
    assert!(Locks::none().proposed_changes_refused().is_none());
}

/// The internal services a runner's chats planned on another machine reach (#2445, ruled
/// 2026-10-18): a list of their own, which `hosts` neither gives nor takes.
#[test]
fn relayed_internal_lists_hosts_of_its_own_and_leaves_hosts_alone() {
    let file = Path::new("/etc/purlis/policy.json");
    let locks = Locks::parse(
        r#"{"sandbox": {"relayed-internal": ["10.96.0.1:443", "*.svc.cluster.local"]}}"#,
        file,
    );

    assert_eq!(
        locks
            .relayed_internal()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["10.96.0.1:443", "*.svc.cluster.local"]
    );
    assert!(locks.allowed_hosts().is_none());
    assert!(
        Locks::parse(r#"{"sandbox": {"hosts": ["10.96.0.1"]}}"#, file)
            .relayed_internal()
            .is_empty()
    );
}

/// A range is an entry of its own (#2530).
#[test]
fn relayed_internal_takes_a_range() {
    let locks = Locks::parse(
        r#"{"sandbox": {"relayed-internal": ["10.96.0.0/12", "[fd00:10:96::/112]:443"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );

    assert_eq!(locks.refused_because(), None);
    assert_eq!(
        locks
            .relayed_internal()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["10.96.0.0/12", "[fd00:10:96::/112]:443"]
    );
}

/// One bad entry is named in a sentence of its own and carries nothing (#2530): the rest of the
/// list, and every other lock, stand as written, and nothing else is locked for it.
#[test]
fn a_bad_relayed_internal_entry_is_refused_alone_and_locks_nothing_else() {
    let locks = Locks::parse(
        r#"{"sandbox": {"relayed-internal": ["10.0.0.5/8", "10.96.0.1:443", 7, "0.0.0.0/0"],
                        "never-hosts": ["*.paste.example"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );

    assert_eq!(locks.refused_because(), None);
    assert_eq!(
        locks
            .relayed_internal()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["10.96.0.1:443"]
    );
    let refused = locks.relayed_internal_refused();
    assert_eq!(refused.len(), 3, "{refused:?}");
    assert!(refused[0].contains("\"10.0.0.5/8\"") && refused[0].contains("10.0.0.0/8"));
    assert!(refused[1].contains("7") && refused[1].contains("text"));
    assert!(refused[2].contains("\"0.0.0.0/0\""));
    let control = Locks::parse(
        "{\"sandbox\": {\"relayed-internal\": [\"10.0.0.5/8\\nx\"]}}",
        Path::new("/etc/purlis/policy.json"),
    );
    let said = control.relayed_internal_refused();
    assert!(
        said.len() == 1 && !said[0].chars().any(char::is_control),
        "{said:?}"
    );
    assert!(!locks.forbids_personal_hosts());
    assert!(!locks.forbids_write_grants());
    assert!(!locks.forbids_opt_out());
    assert!(locks.allowed_hosts().is_none());
    assert_eq!(locks.never_hosts().len(), 1);
}

#[test]
fn a_relayed_internal_that_is_not_a_list_refuses_the_file() {
    let locks = Locks::parse(
        r#"{"sandbox": {"relayed-internal": "10.0.0.0/8"}}"#,
        Path::new("/etc/purlis/policy.json"),
    );

    assert!(locks.refused_because().is_some());
    assert!(locks.relayed_internal().is_empty());
}

/// SK-16 (#2779, the operator's ruling of 2026-10-26): a grade's chat keeps no Allow at any
/// scope while its sandbox is decided, and every other decision keeps what policy leaves open.
#[test]
fn a_grades_chat_keeps_no_allow_of_any_scope_and_only_while_it_is_decided() {
    let plane = tempfile::tempdir().unwrap();
    let levels = [
        crate::sandbox::grant::Level::Chat,
        crate::sandbox::grant::Level::You,
        crate::sandbox::grant::Level::Machine,
    ];
    assert!(
        levels
            .iter()
            .all(|level| Locks::of(plane.path()).keeps_scope(*level))
    );
    let held = holding_no_kept_grants(|| {
        levels
            .iter()
            .any(|level| Locks::of(plane.path()).keeps_scope(*level))
    });
    assert!(!held, "no scope is kept for a grade's chat");
    assert!(
        levels
            .iter()
            .all(|level| Locks::of(plane.path()).keeps_scope(*level))
    );
}
