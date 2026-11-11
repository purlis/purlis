// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What Always allow writes, and what it allows of a persona's list.

use super::*;
use crate::sandbox::tools;

fn bound(env: &[&str], file: &[&str]) -> Bound {
    Bound {
        chat: 1,
        name: "curl".into(),
        program: "/usr/bin/curl".into(),
        vault: "team".into(),
        env: env.iter().map(|one| (*one).to_owned()).collect(),
        file: file.iter().map(|one| (*one).to_owned()).collect(),
        cwd: "/work".into(),
        args: vec!["--these-never-appear".into()],
    }
}

/// A project with `manifest`, and a program search path of the system's.
fn project(manifest: &str) -> tempfile::TempDir {
    tools::SEARCH_HERE
        .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), manifest).unwrap();
    std::fs::create_dir_all(tmp.path().join("personas/devops")).unwrap();
    std::fs::write(
        tmp.path().join("personas/devops/persona.md"),
        "---\nrole: Ops\n---\n",
    )
    .unwrap();
    tmp
}

fn manifest(root: &Path) -> String {
    std::fs::read_to_string(root.join("charter.toml")).unwrap()
}

/// The person allows all of `persona`'s list as it stands.
fn allow_all(root: &Path, persona: &str) {
    let shown = persona::unjudged_in(root, &manifest(root))
        .into_iter()
        .find(|one| one.persona == persona)
        .expect("listed");
    crate::sandbox::local::allow_persona_hosts(root, persona, &shown.digest).unwrap();
}

fn allowed_digest(root: &Path, persona: &str) -> Option<String> {
    tools::allowed_digest_here(root, persona)
}

const ON: &str = "[sandbox]\nmode = \"on\"\n";

#[test]
fn the_line_holds_the_name_vault_key_and_hand_over_and_no_args() {
    let line = line_of(&bound(&[], &["KUBECONFIG=config"])).unwrap();
    assert_eq!(line.name, "curl");
    assert_eq!(
        line.text,
        r#"curl = { vault = "team", as = "KUBECONFIG file", key = "config" }"#
    );
    assert!(!line.by_env);
    assert!(!line.text.contains("args"));
    assert!(!line.text.contains("never-appear"));
}

#[test]
fn env_is_kept_where_the_command_used_it_and_says_so() {
    let line = line_of(&bound(&["T=TOKEN"], &[])).unwrap();
    assert_eq!(
        line.text,
        r#"curl = { vault = "team", as = "T env", key = "TOKEN" }"#
    );
    assert!(line.by_env);
}

#[test]
fn several_values_are_written_as_values_each_as_the_command_handed_it() {
    let line = line_of(&bound(&["A=KEY_A"], &["B=KEY_B"])).unwrap();
    assert_eq!(
        line.text,
        r#"curl = { vault = "team", values = { A = { key = "KEY_A", as = "env" }, B = { key = "KEY_B", as = "file" } } }"#
    );
}

#[test]
fn a_hostile_name_stays_a_string_and_cannot_add_a_key() {
    let mut one = bound(&[], &["V=x\", args = [\"y"]);
    one.vault = "te\"am".into();
    let line = line_of(&one).unwrap();
    let parsed: toml::Table = line.text.parse().unwrap();
    let tool = parsed["curl"].as_table().unwrap();
    assert_eq!(tool["vault"].as_str(), Some("te\"am"));
    assert!(!tool.contains_key("args"));
}

#[test]
fn it_is_written_exactly_as_shown_and_reads_back_as_the_tool() {
    let tmp = project(ON);
    let one = bound(&[], &["V=TOKEN"]);
    write(tmp.path(), "devops", &one).unwrap();
    let text = manifest(tmp.path());
    assert!(text.contains(&line_of(&one).unwrap().text), "{text}");
    let (declared, _) = tools::declared_and_allowed_here(tmp.path(), "devops");
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].name, "curl");
}

#[test]
fn everything_else_in_the_file_stays_as_it_was_written() {
    let before = "# a comment\n[sandbox]\nmode = \"on\"   # beside it\nhosts = [\"a.example\"]\n\n[other]\nkept = 1\n";
    let tmp = project(before);
    write(tmp.path(), "devops", &bound(&[], &["V=TOKEN"])).unwrap();
    let text = manifest(tmp.path());
    assert!(
        text.starts_with(before.trim_end()) || text.contains("# beside it"),
        "{text}"
    );
    assert!(text.contains("# a comment"));
    assert!(text.contains("kept = 1"));
}

#[test]
fn it_joins_the_tools_a_persona_writes_inline_or_as_a_table() {
    for file in [
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\ntools = { other = { vault = \"team\", as = \"O file\", key = \"K\" } }\n",
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops.tools]\nother = { vault = \"team\", as = \"O file\", key = \"K\" }\n",
        "[sandbox]\nmode = \"on\"\n[sandbox.personas]\ndevops = { hosts = [\"a.example\"] }\n",
    ] {
        let tmp = project(file);
        let one = bound(&[], &["V=TOKEN"]);
        write(tmp.path(), "devops", &one).unwrap();
        let (declared, _) = tools::declared_and_allowed_here(tmp.path(), "devops");
        assert!(
            declared.iter().any(|tool| tool.name == "curl"),
            "{}",
            manifest(tmp.path())
        );
        assert!(
            manifest(tmp.path()).contains(&line_of(&one).unwrap().text),
            "{}",
            manifest(tmp.path())
        );
    }
}

#[test]
fn where_the_persona_was_fully_allowed_the_digest_moves_for_that_line_alone() {
    let tmp =
        project("[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"a.example\"]\n");
    allow_all(tmp.path(), "devops");
    assert!(allowed_digest(tmp.path(), "devops").is_some());
    let written = write(tmp.path(), "devops", &bound(&[], &["V=TOKEN"])).unwrap();
    let allow = written
        .allow
        .expect("all of it was allowed, so this is too");
    assert_eq!(allow.hosts.len(), 1);
    assert_eq!(allow.tools.len(), 1);
    assert_eq!(allow.tools[0].name, "curl");
    // Until kept, the file's new line stands unallowed: nothing moved by itself.
    assert_eq!(allowed_digest(tmp.path(), "devops"), None);
    crate::sandbox::local::allow_persona_hosts(tmp.path(), "devops", &allow.digest).unwrap();
    assert_eq!(allowed_digest(tmp.path(), "devops"), Some(allow.digest));
}

#[test]
fn where_a_teammates_host_is_pending_nothing_else_is_approved_and_the_notice_stays() {
    let tmp =
        project("[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"a.example\"]\n");
    allow_all(tmp.path(), "devops");
    // A teammate's change arrives, unread.
    std::fs::write(
        tmp.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"a.example\", \"teammate.example\"]\n",
    )
    .unwrap();
    let written = write(tmp.path(), "devops", &bound(&[], &["V=TOKEN"])).unwrap();
    assert_eq!(written.allow, None, "nothing else is approved");
    assert_eq!(allowed_digest(tmp.path(), "devops"), None);
    let shown = persona::shown(tmp.path());
    let devops = shown.iter().find(|one| one.persona == "devops").unwrap();
    assert!(
        devops.asks() && !devops.allowed(),
        "the usual Notice stays up"
    );
    assert!(manifest(tmp.path()).contains("teammate.example"));
}

#[test]
fn a_persona_with_nothing_listed_has_nothing_waiting() {
    let tmp = project(ON);
    let written = write(tmp.path(), "devops", &bound(&[], &["V=TOKEN"])).unwrap();
    assert_eq!(written.allow.expect("nothing waited").tools.len(), 1);
}

#[test]
fn a_machine_that_did_not_allow_it_has_none_of_the_tools_in_force() {
    let tmp = project(ON);
    write(tmp.path(), "devops", &bound(&[], &["V=TOKEN"])).unwrap();
    let (declared, allowed) = tools::declared_and_allowed_here(tmp.path(), "devops");
    assert_eq!(declared.len(), 1);
    assert!(
        allowed.is_empty(),
        "a teammate's machine runs nothing until its own Allow"
    );
}

#[test]
fn a_write_that_fails_says_why_and_leaves_the_file_as_it_was() {
    let broken = "[sandbox\nnot toml";
    let tmp = project(broken);
    let why = write(tmp.path(), "devops", &bound(&[], &["V=TOKEN"])).unwrap_err();
    assert!(why.contains("not valid TOML"), "{why}");
    assert_eq!(manifest(tmp.path()), broken);
}

#[test]
fn a_tool_of_that_name_already_there_is_refused_not_overwritten() {
    let file = "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\ntools = { curl = { vault = \"other\", as = \"O file\", key = \"K\" } }\n";
    let tmp = project(file);
    let why = write(tmp.path(), "devops", &bound(&[], &["V=TOKEN"])).unwrap_err();
    assert!(why.contains("already"), "{why}");
    assert_eq!(manifest(tmp.path()), file);
}

#[test]
fn a_chat_with_no_persona_writes_nothing() {
    let tmp = project(ON);
    assert!(write(tmp.path(), "", &bound(&[], &["V=TOKEN"])).is_err());
    assert_eq!(manifest(tmp.path()), ON);
}

#[test]
fn always_allow_alone_names_the_vault_to_give_and_gives_nothing_itself() {
    let tmp = project(ON);
    std::fs::write(
        tmp.path().join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "t.json"}, "persona": "devops"},
            "other": {"provider": "plain-file", "config": {"file": "o.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    let mut elsewhere = bound(&[], &["V=TOKEN"]);
    elsewhere.vault = "other".into();
    let written = write(tmp.path(), "devops", &elsewhere).unwrap();
    assert_eq!(written.vault.expect("not its vault").vault, "other");
    assert!(
        crate::sandbox::local::granted_vaults(tmp.path()).is_empty(),
        "the caller keeps it, audited"
    );
}

#[test]
fn a_vault_the_persona_may_use_is_not_given_again() {
    let tmp = project(ON);
    std::fs::write(
        tmp.path().join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "t.json"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .unwrap();
    let written = write(tmp.path(), "devops", &bound(&[], &["V=TOKEN"])).unwrap();
    assert_eq!(written.vault, None);
}

#[test]
fn a_program_not_reviewed_is_never_written_and_the_file_stays_as_it_was() {
    let tmp = project(ON);
    let mut one = bound(&[], &["V=TOKEN"]);
    one.name = "kubectl".into();
    one.program = "/usr/bin/kubectl".into();
    let why = write(tmp.path(), "devops", &one).unwrap_err();
    assert!(why.contains("kubectl"), "{why}");
    assert_eq!(manifest(tmp.path()), ON);
}

/// #5080: the notice says, before Always allow is pressed, whether something else of the
/// persona's waits on the person: then the press approves its own line alone.
#[test]
fn the_notice_knows_beforehand_when_something_else_of_the_persona_waits() {
    let tmp =
        project("[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"a.example\"]\n");
    assert!(
        others_wait(tmp.path(), "devops"),
        "never allowed: its list waits"
    );
    allow_all(tmp.path(), "devops");
    assert!(!others_wait(tmp.path(), "devops"), "all of it allowed");
    std::fs::write(
        tmp.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"a.example\", \"teammate.example\"]\n",
    )
    .unwrap();
    assert!(others_wait(tmp.path(), "devops"), "a teammate's host waits");
    assert!(!others_wait(project(ON).path(), "devops"), "nothing listed");
}

#[test]
fn undo_takes_out_the_line_it_wrote_and_leaves_the_rest_of_the_file() {
    let before = "# a comment\n[sandbox]\nmode = \"on\"\n\n[other]\nkept = 1\n";
    let tmp = project(before);
    let one = bound(&[], &["V=TOKEN"]);
    write(tmp.path(), "devops", &one).unwrap();
    let line = line_of(&one).unwrap().text;
    assert!(holds(tmp.path(), "devops", &line));
    assert_eq!(
        remove(tmp.path(), "devops", &line).unwrap(),
        Removed::Taken { allow: None }
    );
    let text = manifest(tmp.path());
    assert!(!text.contains("curl"), "{text}");
    assert!(
        text.contains("# a comment") && text.contains("kept = 1"),
        "{text}"
    );
    assert!(!holds(tmp.path(), "devops", &line));
    let (declared, _) = tools::declared_and_allowed_here(tmp.path(), "devops");
    assert!(declared.is_empty());
}

#[test]
fn undo_finds_the_line_whether_the_persona_holds_its_tools_inline_or_as_a_table() {
    for file in [
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\ntools = { other = { vault = \"team\", as = \"O file\", key = \"K\" } }\n",
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops.tools]\nother = { vault = \"team\", as = \"O file\", key = \"K\" }\n",
    ] {
        let tmp = project(file);
        let one = bound(&[], &["V=TOKEN"]);
        write(tmp.path(), "devops", &one).unwrap();
        let line = line_of(&one).unwrap().text;
        assert!(matches!(
            remove(tmp.path(), "devops", &line).unwrap(),
            Removed::Taken { .. }
        ));
        let (declared, _) = tools::declared_and_allowed_here(tmp.path(), "devops");
        assert_eq!(
            declared
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            ["other"],
            "{}",
            manifest(tmp.path())
        );
    }
}

#[test]
fn a_line_gone_already_or_changed_since_is_gone_and_nothing_is_written() {
    let tmp = project(ON);
    let one = bound(&[], &["V=TOKEN"]);
    let line = line_of(&one).unwrap().text;
    assert_eq!(remove(tmp.path(), "devops", &line).unwrap(), Removed::Gone);
    write(tmp.path(), "devops", &one).unwrap();
    // A teammate's edit makes it theirs: Undo never takes it out.
    let edited = manifest(tmp.path()).replace("\"TOKEN\"", "\"OTHER\"");
    std::fs::write(tmp.path().join("charter.toml"), &edited).unwrap();
    assert!(!holds(tmp.path(), "devops", &line));
    assert_eq!(remove(tmp.path(), "devops", &line).unwrap(), Removed::Gone);
    assert_eq!(manifest(tmp.path()), edited);
}

#[test]
fn where_the_persona_was_fully_allowed_the_allow_moves_with_the_undo() {
    let tmp =
        project("[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\nhosts = [\"a.example\"]\n");
    allow_all(tmp.path(), "devops");
    let one = bound(&[], &["V=TOKEN"]);
    let written = write(tmp.path(), "devops", &one).unwrap();
    crate::sandbox::local::allow_persona_hosts(
        tmp.path(),
        "devops",
        &written.allow.unwrap().digest,
    )
    .unwrap();
    let Removed::Taken { allow: Some(allow) } =
        remove(tmp.path(), "devops", &line_of(&one).unwrap().text).unwrap()
    else {
        panic!("all of it was allowed, so what is left is to allow too");
    };
    assert_eq!(allow.hosts.len(), 1);
    assert!(allow.tools.is_empty());
}

#[test]
fn a_line_that_reads_as_no_tool_is_refused() {
    let tmp = project(ON);
    assert!(remove(tmp.path(), "devops", "not a line").is_err());
    assert!(!holds(tmp.path(), "devops", "not a line"));
}
