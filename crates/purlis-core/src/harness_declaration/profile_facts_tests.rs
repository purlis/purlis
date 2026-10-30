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
    use SignInKind::*;
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
            config_env: Some("CLAUDE_CONFIG_DIR".to_owned()),
            data_env: None
        }
    );
    assert_eq!(
        folders(Harness::Codex),
        Folders {
            config_env: Some("CODEX_HOME".to_owned()),
            data_env: None
        }
    );
    assert_eq!(
        folders(Harness::Opencode),
        Folders {
            config_env: Some("XDG_CONFIG_HOME".to_owned()),
            data_env: Some("XDG_DATA_HOME".to_owned())
        }
    );
}

#[test]
fn each_built_in_says_whether_its_credentials_can_be_kept_from_its_commands() {
    // Read out of its sandbox as purlis compiles it (#2037).
    assert_eq!(
        of(Harness::ClaudeCode).sign_in.kept_from_commands,
        Answer::No(
            "on macOS and on a runner purlis cannot keep Claude Code's sign-in from the commands it runs"
                .to_owned()
        )
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
            file: "{config}/projects/{cwd_key}/{id}.jsonl".to_owned(),
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
config_env = "SHELLY_HOME"
data_env = "SHELLY_DATA"

[conversation]
export = ["sessions", "export", "--id={id}"]
import = ["sessions", "import"]
"#;

#[test]
fn a_project_declared_harness_declares_the_same_four_facts() {
    let dir = project(&[("shelly.toml", SHELLY)]);

    let read = read(dir.path());

    assert_eq!(read.refused, Vec::<Refused>::new());
    let shelly = read.get("shelly").expect("declared");
    assert_eq!(
        shelly.sign_in.kinds,
        [SignInKind::ApiKey, SignInKind::Local]
    );
    assert_eq!(
        shelly.sign_in.kept_from_commands,
        Answer::No("it runs its commands as itself".to_owned())
    );
    assert_eq!(
        shelly.folders,
        Folders {
            config_env: Some("SHELLY_HOME".to_owned()),
            data_env: Some("SHELLY_DATA".to_owned())
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
fn a_conversation_kept_as_a_file_is_read_from_the_file_it_names() {
    let text = "name = \"aider\"\nprogram = \"aider\"\nenv = [\"AIDER_*\"]\n\
                [folders]\nconfig_env = \"AIDER_HOME\"\n\
                [conversation]\nfile = \"{config}/chats/{cwd_key}/*/chat-*-{id}.md\"\n\
                with_forked_from = true\n";
    let dir = project(&[("aider.toml", text)]);

    let read = read(dir.path());

    assert_eq!(read.refused, Vec::<Refused>::new());
    assert_eq!(
        read.get("aider").unwrap().conversation,
        Some(Conversation::File {
            file: "{config}/chats/{cwd_key}/*/chat-*-{id}.md".to_owned(),
            with_forked_from: true
        })
    );
}

#[test]
fn a_working_directorys_folder_is_its_path_with_every_other_character_a_dash() {
    assert_eq!(cwd_key(Path::new("/Users/a/my.app_2")), "-Users-a-my-app-2");
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
    assert_eq!(aider.profile_sign_in(&[], &[]), None);
}

fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn passed(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|entry| (*entry).to_owned()).collect()
}

#[test]
fn a_built_in_profile_whose_sign_in_purlis_cannot_see_is_unknown() {
    // A subscription is the harness's own, and purlis never looks at it.
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        assert_eq!(of(harness).profile_sign_in(&[], &[]), None, "{harness:?}");
    }
}

#[test]
fn a_built_in_profile_passed_a_providers_key_signs_in_with_it() {
    let claude = of(Harness::ClaudeCode);
    assert_eq!(
        claude.profile_sign_in(&[], &passed(&["ANTHROPIC_API_KEY"])),
        Some(SignInKind::ApiKey)
    );
    assert_eq!(
        claude.profile_sign_in(&[], &passed(&["ANTHROPIC_*"])),
        Some(SignInKind::ApiKey)
    );
    assert_eq!(
        of(Harness::Codex).profile_sign_in(&[], &passed(&["OPENAI_API_KEY"])),
        Some(SignInKind::ApiKey)
    );
    // A variable that is no provider's key says nothing.
    assert_eq!(claude.profile_sign_in(&[], &passed(&["GOPATH"])), None);
}

#[test]
fn a_built_in_profile_switched_to_a_cloud_signs_in_there() {
    let claude = of(Harness::ClaudeCode);
    assert_eq!(
        claude.profile_sign_in(&env(&[("CLAUDE_CODE_USE_BEDROCK", "1")]), &[]),
        Some(SignInKind::Cloud)
    );
    // A switch set off is no switch.
    assert_eq!(
        claude.profile_sign_in(&env(&[("CLAUDE_CODE_USE_VERTEX", "0")]), &[]),
        None
    );
    // A key and a cloud both: purlis cannot tell which one the harness uses.
    assert_eq!(
        claude.profile_sign_in(
            &env(&[("CLAUDE_CODE_USE_BEDROCK", "1")]),
            &passed(&["ANTHROPIC_API_KEY"])
        ),
        None
    );
}

#[test]
fn a_harness_that_supports_one_kind_alone_gives_its_profiles_that_kind() {
    let with = |kinds: &str| {
        let text = format!("name = \"aider\"\nprogram = \"aider\"\n[sign_in]\nkinds = {kinds}\n");
        let dir = project(&[("aider.toml", &text)]);
        let read = read(dir.path());
        assert_eq!(read.refused, Vec::<Refused>::new());
        read.get("aider")
            .unwrap()
            .profile_sign_in(&[], &passed(&["ANTHROPIC_API_KEY"]))
    };

    assert_eq!(with(r#"["local"]"#), Some(SignInKind::Local));
    // Two kinds of a harness purlis does not ship: it knows none of its variables.
    assert_eq!(with(r#"["api-key", "subscription"]"#), None);
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
    let home = "[folders]\nconfig_env = \"AIDER_HOME\"\n[conversation]\n";
    let file = |path: &str| format!("{home}file = \"{path}\"\n");
    for (table, said) in [
        (
            "[sign_in]\nkinds = [\"oauth\"]\n".to_owned(),
            "a sign-in kind is one of: subscription, api-key, cloud, local",
        ),
        (
            "[sign_in]\nkinds = [\"local\", \"local\"]\n".to_owned(),
            "names local twice",
        ),
        (
            "[sign_in]\nkept_from_commands = \"maybe\"\n".to_owned(),
            "a capability is \"yes\", \"no: <the reason>\" or \"unknown\"",
        ),
        (
            "[folders]\nconfig_env = \"OTHER_HOME\"\n".to_owned(),
            "which is not in its env namespace",
        ),
        (
            "[folders]\nconfig_env = \"PATH\"\n".to_owned(),
            "changes what a process loads",
        ),
        (
            "[folders]\nconfig_env = \"AIDER_TOKEN\"\n".to_owned(),
            "named like a credential",
        ),
        (
            "[folders]\nconfig_env = \"AIDER_*\"\n".to_owned(),
            "one variable's whole name",
        ),
        (
            "[folders]\ndata_env = \"AIDER_DATA\"\n".to_owned(),
            "gives data_env and no config_env",
        ),
        (
            "[folders]\nconfig_env = \"AIDER_HOME\"\ndata_env = \"AIDER_HOME\"\n".to_owned(),
            "one variable for config_env and data_env",
        ),
        (
            "[conversation]\nmoved_by = \"file\"\n".to_owned(),
            "is not a harness declaration purlis reads",
        ),
        (
            "[conversation]\n".to_owned(),
            "gives neither a file nor both export and import",
        ),
        (file("{config}/../{id}.json"), "holds a part that is not plain"),
        (file("{config}/**/{id}.json"), "holds a part that is not plain"),
        (file("{config}/s?/{id}.json"), "holds a part that is not plain"),
        (file("{config}/x-{cwd_key}/{id}.json"), "holds a part that is not plain"),
        (file("/tmp/{id}.json"), "does not start at {config}/ or {data}/"),
        (
            file("{data}/{id}.json"),
            "starts at a folder its [folders] does not name",
        ),
        (file("{config}/sessions.json"), "names no {id}"),
        (
            "[conversation]\nexport = [\"export\"]\nimport = [\"import\"]\n".to_owned(),
            "names no {id}, so it could not export",
        ),
        (
            "[conversation]\nexport = [\"export\", \"{id}\"]\nimport = [\"import\"]\n".to_owned(),
            "is a bare {id}",
        ),
        (
            "[conversation]\nexport = [\"--session={id}\"]\nimport = [\"/bin/sh\"]\n".to_owned(),
            "is a path",
        ),
        (
            "[conversation]\nexport = [\"--session={id}\"]\n".to_owned(),
            "gives neither a file nor both export and import",
        ),
        (
            "[conversation]\nexport = [\"--session={id}\"]\nimport = [\"import\"]\nfile = \"{config}/{id}\"\n".to_owned(),
            "Keep one",
        ),
        (
            "[conversation]\nexport = [\"--session={id}\"]\nimport = [\"import\"]\nwith_forked_from = true\n".to_owned(),
            "gives with_forked_from and no file",
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
            "[harnesses.claude]\nprogram = \"claude\"\n[harnesses.claude.folders]\nconfig_env = \"CLAUDE_HOME\"\n",
            "Remove [folders]",
        ),
        (
            "[harnesses.opencode]\nprogram = \"opencode\"\n[harnesses.opencode.conversation]\nexport = [\"export\", \"{id}\"]\nimport = [\"import\", \"--force\"]\n",
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

// ----- the plans a subscription signs in to (`[sign_in] plans`, #2039) ---------------------

fn plan(provider: &str, title: &str) -> Plan {
    Plan {
        provider: provider.to_owned(),
        title: title.to_owned(),
    }
}

/// `[sign_in]` of a harness a project declares, `name`, as its one refusal or its plans.
fn plans_of(name: &str, sign_in: &str) -> Result<Vec<Plan>, String> {
    let text = format!("name = \"{name}\"\nprogram = \"{name}\"\n[sign_in]\n{sign_in}");
    parse(&text, Origin::Project, &format!("harnesses/{name}.toml")).map(|d| d.sign_in.plans)
}

#[test]
fn opencode_signs_a_subscription_in_to_the_plans_its_declaration_lists() {
    assert_eq!(
        of(Harness::Opencode).sign_in.plans,
        [
            plan("openai", "ChatGPT"),
            plan("github-copilot", "GitHub Copilot")
        ]
    );
}

#[test]
fn a_harness_declaring_a_subscription_with_no_plans_signs_in_to_its_own_vendors_plan_alone() {
    for harness in [Harness::ClaudeCode, Harness::Codex] {
        let declared = of(harness);
        assert!(declared.sign_in.kinds.contains(&SignInKind::Subscription));
        assert_eq!(declared.sign_in.plans, [], "{harness:?}");
    }
}

#[test]
fn a_project_declared_harness_lists_its_own_plans() {
    assert_eq!(
        plans_of(
            "shelly",
            "kinds = [\"subscription\"]\n\
             plans = [{ provider = \"openai\", title = \"ChatGPT\" }]\n"
        ),
        Ok(vec![plan("openai", "ChatGPT")])
    );
}

#[test]
fn anthropic_is_refused_in_the_plans_of_any_harness_but_claude_codes() {
    let why = plans_of(
        "shelly",
        "kinds = [\"subscription\"]\n\
         plans = [{ provider = \"anthropic\", title = \"Claude\" }]\n",
    )
    .expect_err("refused");
    assert_eq!(
        why,
        "harnesses/shelly.toml's [sign_in] plans names anthropic, and a Claude plan is signed \
         in only through Claude Code's own sign-in. Remove it."
    );
    // This machine's own table is held to it too.
    let local = parse(
        "name = \"shelly\"\nprogram = \"shelly\"\n[sign_in]\nkinds = [\"subscription\"]\n\
         plans = [{ provider = \"anthropic\", title = \"Claude\" }]\n",
        Origin::Local,
        "charter.local.toml",
    );
    assert!(local.unwrap_err().contains("names anthropic"));
}

#[test]
fn plans_are_a_subscriptions_and_each_is_a_providers_own_id_once() {
    assert_eq!(
        plans_of(
            "shelly",
            "kinds = [\"api-key\"]\nplans = [{ provider = \"openai\", title = \"ChatGPT\" }]\n"
        ),
        Err(
            "harnesses/shelly.toml's [sign_in] plans names the plans a subscription signs in \
             to, and [sign_in] kinds lists no subscription. List subscription in kinds, or \
             remove plans."
                .to_owned()
        )
    );
    for provider in ["", "Open AI", "openai;x", "op'enai", "-p"] {
        let why = plans_of(
            "shelly",
            &format!(
                "kinds = [\"subscription\"]\nplans = [{{ provider = \"{provider}\", title = \"A\" }}]\n"
            ),
        )
        .expect_err(provider);
        assert!(why.contains("is not a provider's id"), "{provider}: {why}");
    }
    let twice = plans_of(
        "shelly",
        "kinds = [\"subscription\"]\nplans = [{ provider = \"openai\", title = \"A\" }, \
         { provider = \"openai\", title = \"B\" }]\n",
    )
    .expect_err("twice");
    assert!(twice.contains("names openai twice"), "{twice}");
    let drawn = plans_of(
        "shelly",
        "kinds = [\"subscription\"]\nplans = [{ provider = \"openai\", title = \"A\\nB\" }]\n",
    )
    .expect_err("title");
    assert!(drawn.contains("[sign_in] plans' title"), "{drawn}");
}

#[test]
fn the_one_declaration_that_may_list_anthropic_is_claude_codes_own() {
    assert_eq!(of(Harness::ClaudeCode).name, CLAUDE_CODE);
}
