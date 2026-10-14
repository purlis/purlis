// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a harness declaration says about the profiles of that harness (#2031, spec #2030): the
//! sign-in kinds it supports, the folder variables it uses, whether its credentials can be kept
//! from the commands it runs, and how a conversation is exported and resumed. Every expected
//! answer is written out.

use super::*;

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(DIR)).unwrap();
    for (file, text) in files {
        std::fs::write(dir.path().join(DIR).join(file), text).unwrap();
    }
    dir
}

#[test]
fn each_built_in_says_which_sign_in_kinds_it_supports() {
    use SignIn::*;
    assert_eq!(
        of(Harness::ClaudeCode).sign_in.kinds,
        [Subscription, ApiKey, Cloud]
    );
    assert_eq!(
        of(Harness::Codex).sign_in.kinds,
        [Subscription, ApiKey, Cloud, Local]
    );
    assert_eq!(
        of(Harness::Opencode).sign_in.kinds,
        [Subscription, ApiKey, Cloud, Local]
    );
}

#[test]
fn each_built_in_says_its_folder_variables() {
    let folders = |harness: Harness| of(harness).folders.clone();
    assert_eq!(
        folders(Harness::ClaudeCode),
        Folders {
            config: Some("CLAUDE_CONFIG_DIR".to_owned()),
            data: None
        }
    );
    assert_eq!(
        folders(Harness::Codex),
        Folders {
            config: Some("CODEX_HOME".to_owned()),
            data: None
        }
    );
    assert_eq!(
        folders(Harness::Opencode),
        Folders {
            config: Some("OPENCODE_CONFIG_DIR".to_owned()),
            data: Some("XDG_DATA_HOME".to_owned())
        }
    );
}

#[test]
fn each_built_in_says_whether_its_credentials_can_be_kept_from_its_commands() {
    assert_eq!(
        of(Harness::ClaudeCode).sign_in.kept_from_commands,
        Answer::Yes
    );
    assert_eq!(
        of(Harness::Codex).sign_in.kept_from_commands,
        Answer::No("Codex and the commands it runs are wrapped in one sandbox".to_owned())
    );
    assert_eq!(
        of(Harness::Opencode).sign_in.kept_from_commands,
        Answer::No("opencode and the commands it runs are wrapped in one sandbox".to_owned())
    );
}

#[test]
fn each_built_in_says_how_a_conversation_is_exported_and_resumed() {
    assert_eq!(
        of(Harness::ClaudeCode).conversation,
        Some(Conversation::File {
            file: "{config}/projects/*/{id}.jsonl".to_owned(),
            with_forked_from: false
        })
    );
    assert_eq!(
        of(Harness::Codex).conversation,
        Some(Conversation::File {
            file: "{config}/sessions/*/*/*/rollout-*-{id}.jsonl".to_owned(),
            with_forked_from: true
        })
    );
    assert_eq!(
        of(Harness::Opencode).conversation,
        Some(Conversation::Export {
            export: vec!["export".to_owned(), "{id}".to_owned()],
            import: vec!["import".to_owned()]
        })
    );
    // Each resumes by its id, so a conversation that came back can be carried on.
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        assert!(of(harness).session.resume.is_some(), "{harness:?}");
    }
}

/// A harness a project declares, with all four facts.
const SHELLY: &str = r#"
name = "shelly"
program = "shelly"
env = ["SHELLY_*"]

[session]
resume = ["--resume={id}"]

[sign_in]
kinds = ["api-key", "local"]
kept_from_commands = "no: it runs its commands as itself"

[folders]
config = "SHELLY_HOME"
data = "SHELLY_DATA"

[conversation]
moved_by = "export"
export = ["sessions", "export", "--id={id}"]
import = ["sessions", "import"]
"#;

#[test]
fn a_project_declared_harness_declares_the_same_four_facts() {
    let dir = project(&[("shelly.toml", SHELLY)]);

    let read = read(dir.path());

    assert_eq!(read.refused, Vec::<Refused>::new());
    let shelly = read.get("shelly").expect("declared");
    assert_eq!(shelly.sign_in.kinds, [SignIn::ApiKey, SignIn::Local]);
    assert_eq!(
        shelly.sign_in.kept_from_commands,
        Answer::No("it runs its commands as itself".to_owned())
    );
    assert_eq!(
        shelly.folders,
        Folders {
            config: Some("SHELLY_HOME".to_owned()),
            data: Some("SHELLY_DATA".to_owned())
        }
    );
    assert_eq!(
        shelly.conversation,
        Some(Conversation::Export {
            export: vec![
                "sessions".to_owned(),
                "export".to_owned(),
                "--id={id}".to_owned()
            ],
            import: vec!["sessions".to_owned(), "import".to_owned()]
        })
    );
}

#[test]
fn a_harness_that_says_nothing_of_its_profiles_supports_no_sign_in_and_keeps_nothing_apart() {
    let dir = project(&[("aider.toml", "name = \"aider\"\nprogram = \"aider\"\n")]);

    let aider = read(dir.path()).get("aider").cloned().expect("declared");

    assert_eq!(aider.sign_in.kinds, []);
    // Silence never reads as yes.
    assert_eq!(aider.sign_in.kept_from_commands, Answer::Unknown);
    assert!(!aider.sign_in.kept_from_commands.holds());
    assert_eq!(aider.folders, Folders::default());
    assert_eq!(aider.conversation, None);
    assert_eq!(aider.profile_sign_in(), None);
}

#[test]
fn a_profile_signs_in_through_its_harnesss_own_flow_where_that_is_a_subscription() {
    let with = |kinds: &str| {
        let text = format!("name = \"aider\"\nprogram = \"aider\"\n[sign_in]\nkinds = {kinds}\n");
        let dir = project(&[("aider.toml", &text)]);
        let read = read(dir.path());
        assert_eq!(read.refused, Vec::<Refused>::new());
        read.get("aider").unwrap().profile_sign_in()
    };

    assert_eq!(
        with(r#"["api-key", "subscription"]"#),
        Some(SignIn::Subscription)
    );
    assert_eq!(with(r#"["local"]"#), Some(SignIn::Local));
    // Two kinds and no subscription: purlis cannot tell which one a profile has.
    assert_eq!(with(r#"["api-key", "cloud"]"#), None);
    assert_eq!(
        of(Harness::ClaudeCode).profile_sign_in(),
        Some(SignIn::Subscription)
    );
}

/// The one sentence `text`, as `harnesses/<name>.toml`, is refused with.
fn refused(name: &str, text: &str) -> String {
    let dir = project(&[(&format!("{name}.toml"), text)]);
    let read = read(dir.path());
    assert!(read.get(name).is_none(), "{text}");
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    read.refused[0].reason.clone()
}

#[test]
fn a_declarations_profile_facts_are_refused_where_they_are_not_ones_purlis_reads() {
    let head = "name = \"aider\"\nprogram = \"aider\"\nenv = [\"AIDER_*\"]\n";
    for (table, said) in [
        (
            "[sign_in]\nkinds = [\"oauth\"]\n",
            "a sign-in kind is one of: subscription, api-key, cloud, local",
        ),
        (
            "[sign_in]\nkinds = [\"local\", \"local\"]\n",
            "names local twice",
        ),
        (
            "[sign_in]\nkept_from_commands = \"maybe\"\n",
            "a capability is \"yes\", \"no: <the reason>\" or \"unknown\"",
        ),
        (
            "[folders]\nconfig = \"OTHER_HOME\"\n",
            "which is not in its env namespace",
        ),
        (
            "[folders]\nconfig = \"PATH\"\n",
            "changes what a process loads",
        ),
        (
            "[folders]\nconfig = \"AIDER_TOKEN\"\n",
            "named like a credential",
        ),
        (
            "[folders]\nconfig = \"AIDER_*\"\n",
            "one variable's whole name",
        ),
        (
            "[folders]\ndata = \"AIDER_DATA\"\n",
            "gives data and no config",
        ),
        (
            "[folders]\nconfig = \"AIDER_HOME\"\ndata = \"AIDER_HOME\"\n",
            "one variable for config and data",
        ),
        (
            "[conversation]\nmoved_by = \"copy\"\n",
            "it is \"file\" or \"export\"",
        ),
        ("[conversation]\nmoved_by = \"file\"\n", "names no file"),
        (
            "[folders]\nconfig = \"AIDER_HOME\"\n[conversation]\nmoved_by = \"file\"\nfile = \"{config}/../{id}.json\"\n",
            "holds a part that is not plain",
        ),
        (
            "[folders]\nconfig = \"AIDER_HOME\"\n[conversation]\nmoved_by = \"file\"\nfile = \"/tmp/{id}.json\"\n",
            "does not start at {config}/ or {data}/",
        ),
        (
            "[folders]\nconfig = \"AIDER_HOME\"\n[conversation]\nmoved_by = \"file\"\nfile = \"{data}/{id}.json\"\n",
            "starts at a folder its [folders] does not name",
        ),
        (
            "[folders]\nconfig = \"AIDER_HOME\"\n[conversation]\nmoved_by = \"file\"\nfile = \"{config}/sessions.json\"\n",
            "names no {id}",
        ),
        (
            "[conversation]\nmoved_by = \"export\"\nexport = [\"export\"]\nimport = [\"import\"]\n",
            "names no {id}, so it could not export",
        ),
        (
            "[conversation]\nmoved_by = \"export\"\nexport = [\"export\", \"{id}\"]\nimport = [\"import\"]\n",
            "is a bare {id}",
        ),
        (
            "[conversation]\nmoved_by = \"export\"\nexport = [\"--session={id}\"]\nimport = [\"/bin/sh\"]\n",
            "is a path",
        ),
        (
            "[conversation]\nmoved_by = \"export\"\nexport = [\"--session={id}\"]\n",
            "needs both export and import",
        ),
        (
            "[conversation]\nmoved_by = \"export\"\nexport = [\"--session={id}\"]\nimport = [\"import\"]\nfile = \"{config}/{id}\"\n",
            "copies no file",
        ),
    ] {
        let reason = refused("aider", &format!("{head}{table}"));
        assert!(reason.contains(said), "{table}: {reason}");
    }
}

#[test]
fn a_local_replacement_keeps_the_built_ins_profile_facts() {
    let restated = r#"
[harnesses.codex]
program = "codex"
tested = "0.150.0"

[harnesses.codex.sign_in]
kinds = ["subscription", "api-key", "cloud", "local"]
kept_from_commands = "no: Codex and the commands it runs are wrapped in one sandbox"
"#;
    let dir = project(&[]);
    let read = read_with(dir.path(), Ok(Some(restated.to_owned())));
    assert_eq!(read.refused, Vec::<Refused>::new());
    let codex = read.replacement(Harness::Codex).expect("replaced");
    assert_eq!(codex.sign_in, of(Harness::Codex).sign_in);
    assert_eq!(codex.folders, of(Harness::Codex).folders);
    assert_eq!(codex.conversation, of(Harness::Codex).conversation);

    for (text, said) in [
        (
            "[harnesses.claude]\nprogram = \"claude\"\n[harnesses.claude.sign_in]\nkinds = [\"subscription\"]\nkept_from_commands = \"yes\"\n",
            "Remove [sign_in]",
        ),
        (
            "[harnesses.codex]\nprogram = \"codex\"\n[harnesses.codex.sign_in]\nkinds = [\"subscription\", \"api-key\", \"cloud\", \"local\"]\nkept_from_commands = \"yes\"\n",
            "Remove [sign_in]",
        ),
        (
            "[harnesses.claude]\nprogram = \"claude\"\n[harnesses.claude.folders]\nconfig = \"CLAUDE_HOME\"\n",
            "Remove [folders]",
        ),
        (
            "[harnesses.opencode]\nprogram = \"opencode\"\n[harnesses.opencode.conversation]\nmoved_by = \"export\"\nexport = [\"export\", \"{id}\"]\nimport = [\"import\", \"--force\"]\n",
            "Remove [conversation]",
        ),
    ] {
        let read = read_with(dir.path(), Ok(Some(text.to_owned())));
        let name = text
            .trim_start_matches("[harnesses.")
            .split(']')
            .next()
            .unwrap();
        assert_eq!(
            read.get(name).map(|d| d.origin),
            Some(Origin::BuiltIn),
            "{text}: the built-in stands"
        );
        assert!(
            read.refused.iter().any(|one| one.reason.contains(said)),
            "{text}: {:?}",
            read.refused
        );
    }
}
