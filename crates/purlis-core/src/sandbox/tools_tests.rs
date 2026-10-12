// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A persona's tools (#1407): what is read, what is refused, where the real program may come
//! from, and the shim that runs it through `purlis secret exec`.

use std::path::{Path, PathBuf};

use super::super::policy::Locks;
use super::{
    FIRST_ON_PATH, Handed, Ready, Tool, first_on_path, keyed, refused_name, resolve_beside, shim,
};

const HERE: &str = "sandbox.personas.devops";
const FILE: &str = "charter.toml";

fn read(text: &str) -> (Vec<Tool>, Vec<String>) {
    let value: toml::Value = toml::from_str(text).expect("TOML");
    let (tools, refused) = keyed(
        value.get("tools"),
        HERE,
        FILE,
        &["sandbox", "personas", "devops"],
    );
    (tools, refused.into_iter().map(|one| one.why).collect())
}

fn kubectl() -> Tool {
    Tool {
        name: "kubectl".to_owned(),
        vault: "devops".to_owned(),
        var: "KUBECONFIG".to_owned(),
        handed: Handed::File,
        key: "kubeconfig".to_owned(),
    }
}

#[test]
fn a_tool_is_read_with_its_vault_key_and_how_its_value_is_handed() {
    let (tools, refused) = read(
        r#"tools = { kubectl = { vault = "devops", as = "KUBECONFIG file", key = "kubeconfig" },
                     psql = { vault = "devops", as = "PGPASSWORD env", key = "db/password" } }"#,
    );
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(
        tools,
        [
            kubectl(),
            Tool {
                name: "psql".to_owned(),
                vault: "devops".to_owned(),
                var: "PGPASSWORD".to_owned(),
                handed: Handed::Env,
                key: "db/password".to_owned(),
            }
        ]
    );
}

#[test]
fn shells_interpreters_env_sudo_xargs_and_purlis_itself_are_never_tools() {
    for name in [
        "sh",
        "bash",
        "zsh",
        "fish",
        "env",
        "sudo",
        "xargs",
        "python3",
        "python3.12",
        "node",
        "perl",
        "ruby",
        "osascript",
        "purlis",
        "charter",
        "Bash",
    ] {
        assert!(refused_name(name).is_some(), "{name}");
        let (tools, refused) = read(&format!(
            "tools = {{ {name:?} = {{ vault = \"devops\", as = \"X file\", key = \"k\" }} }}"
        ));
        assert_eq!(tools, Vec::<Tool>::new(), "{name}");
        assert_eq!(refused.len(), 1, "{name}: {refused:?}");
        assert!(refused[0].contains("runs nothing"), "{}", refused[0]);
    }
    for name in ["kubectl", "psql", "gcloud", "helm", "aws"] {
        assert_eq!(refused_name(name), None, "{name}");
    }
}

#[test]
fn a_name_that_is_a_path_or_not_a_plain_word_is_refused() {
    for name in ["/usr/bin/kubectl", "../kubectl", "-x", ".hidden", "a b", ""] {
        assert!(refused_name(name).is_some(), "{name:?}");
    }
}

#[test]
fn a_tool_with_any_fault_is_left_out_whole_with_a_sentence_and_its_key() {
    for (text, says) in [
        (
            r#"tools = { kubectl = { as = "KUBECONFIG file", key = "k" } }"#,
            "no vault",
        ),
        (
            r#"tools = { kubectl = { vault = "devops", key = "k" } }"#,
            "as what",
        ),
        (
            r#"tools = { kubectl = { vault = "devops", as = "KUBECONFIG", key = "k" } }"#,
            "neither",
        ),
        (
            r#"tools = { kubectl = { vault = "devops", as = "KUBECONFIG file" } }"#,
            "no key",
        ),
        (
            r#"tools = { kubectl = { vault = "devops", as = "PATH file", key = "k" } }"#,
            "PATH",
        ),
        (
            r#"tools = { kubectl = { vault = "devops", as = "DYLD_INSERT_LIBRARIES file", key = "k" } }"#,
            "DYLD",
        ),
        (
            r#"tools = { kubectl = { vault = "devops", as = "KUBECONFIG file", key = "k", run = "x" } }"#,
            "run is not a key",
        ),
        (r#"tools = { kubectl = "devops" }"#, "not a table"),
        (r#"tools = ["kubectl"]"#, "not a table"),
    ] {
        let (tools, refused) = read(text);
        assert_eq!(tools, Vec::<Tool>::new(), "{text}");
        assert_eq!(refused.len(), 1, "{text}: {refused:?}");
        assert!(refused[0].contains(says), "{text}: {}", refused[0]);
        assert!(refused[0].contains(HERE), "{}", refused[0]);
    }
}

#[test]
fn a_tool_s_refusal_carries_the_key_it_is_about() {
    let value: toml::Value =
        toml::from_str(r#"tools = { sh = { vault = "devops", as = "X file", key = "k" } }"#)
            .expect("TOML");
    let (_, refused) = keyed(
        value.get("tools"),
        HERE,
        FILE,
        &["sandbox", "personas", "devops"],
    );
    assert_eq!(
        refused[0].key.as_deref(),
        Some(
            &[
                "sandbox".to_owned(),
                "personas".to_owned(),
                "devops".to_owned(),
                "tools".to_owned(),
                "sh".to_owned()
            ][..]
        )
    );
}

/// A runnable program called `name` in `dir`.
fn program(dir: &Path, name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).expect("dir");
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\n").expect("written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
    path
}

/// A folder a test writes in. The machine's temporary folders are not the ones the tests judge
/// by ([`resolve_beside`]): each test names its own.
fn outside_temp(name: &str) -> PathBuf {
    let dir = tempfile::tempdir().expect("a folder").keep().join(name);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// [`resolve_beside`], with `temps` the only temporary folders.
fn resolve(
    root: &Path,
    persona: &str,
    tools: &[Tool],
    locks: &Locks,
    search: &[PathBuf],
    writable: &[PathBuf],
) -> Option<super::Resolved> {
    resolve_beside(root, persona, tools, locks, search, writable, &[])
}

#[test]
fn the_real_program_is_found_only_where_no_chat_writes() {
    let base = outside_temp("found");
    let root = base.join("project");
    let granted = base.join("granted");
    let system = base.join("system");
    // A chat could plant one in its project, in a folder it was granted, or in a temp folder.
    program(&root.join("bin"), "kubectl");
    program(&granted, "kubectl");
    let temp = tempfile::tempdir().expect("temp");
    program(temp.path(), "kubectl");
    let real = program(&system, "kubectl");
    let search = vec![
        root.join("bin"),
        granted.clone(),
        temp.path().to_path_buf(),
        PathBuf::from("relative/bin"),
        system.clone(),
    ];
    let resolved = resolve_beside(
        &root,
        "devops",
        &[kubectl()],
        &Locks::none(),
        &search,
        std::slice::from_ref(&granted),
        &[temp.path().to_path_buf()],
    )
    .expect("tools");
    assert_eq!(
        resolved.ready,
        [Ready {
            tool: kubectl(),
            program: real,
        }]
    );
    assert_eq!(resolved.left, Vec::<String>::new());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_program_that_links_into_a_folder_a_chat_writes_is_not_the_real_one() {
    let base = outside_temp("linked");
    let root = base.join("project");
    let planted = program(&root.join("bin"), "kubectl");
    let system = base.join("system");
    std::fs::create_dir_all(&system).expect("dir");
    std::os::unix::fs::symlink(&planted, system.join("kubectl")).expect("link");
    let resolved = resolve(
        &root,
        "devops",
        &[kubectl()],
        &Locks::none(),
        std::slice::from_ref(&system),
        &[],
    )
    .expect("tools");
    assert_eq!(resolved.ready, Vec::<Ready>::new());
    assert!(
        resolved.left[0].contains("no kubectl was found"),
        "{:?}",
        resolved.left
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_tool_policy_forbids_gets_no_shim_and_says_why() {
    let base = outside_temp("locked");
    let system = base.join("system");
    program(&system, "kubectl");
    let locks = Locks::parse(
        r#"{"sandbox": {"persona-tools": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    let resolved = resolve(
        &base.join("project"),
        "devops",
        &[kubectl()],
        &locks,
        std::slice::from_ref(&system),
        &[],
    )
    .expect("tools");
    assert_eq!(resolved.ready, Vec::<Ready>::new());
    assert!(
        resolved.left[0].contains("Locked by policy"),
        "{:?}",
        resolved.left
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// D-PE-ruling-1: a chat's start arms a persona's tool only once this machine allowed it with
/// the persona's hosts; before that, and after any change to it, there is no shim.
#[test]
fn a_tool_is_armed_for_a_chat_only_once_this_machine_allows_it() {
    use super::super::hosts::Host;
    use super::super::persona::{Grants, digest, digest_of};
    let base = outside_temp("allow");
    let root = base.join("project");
    std::fs::create_dir_all(&root).expect("dir");
    let system = base.join("system");
    let real = program(&system, "kubectl");
    let host = Host::parse("10.0.0.5:6443").expect("a host");
    let grants = |tool: Tool| {
        std::collections::BTreeMap::from([(
            "devops".to_owned(),
            Grants {
                hosts: vec![host.clone()],
                tools: vec![tool],
            },
        )])
    };
    let armed = |personas: &std::collections::BTreeMap<String, Grants>| {
        super::for_chat_beside(
            &root,
            personas,
            "devops",
            &Locks::none(),
            std::slice::from_ref(&system),
            &[],
            &[],
        )
    };
    let listed = grants(kubectl());
    assert_eq!(armed(&listed), None, "before the Allow, no shim");
    // An Allow of its hosts alone is not one of its tools.
    super::super::local::allow_persona_hosts(
        &root,
        "devops",
        &digest(std::slice::from_ref(&host), false),
    )
    .expect("kept");
    assert_eq!(armed(&listed), None);
    super::super::local::allow_persona_hosts(
        &root,
        "devops",
        &digest_of(std::slice::from_ref(&host), &[kubectl()], false),
    )
    .expect("kept");
    assert_eq!(
        armed(&listed).expect("armed").ready,
        [Ready {
            tool: kubectl(),
            program: real,
        }]
    );
    // A teammate hands it another key: no shim until it is allowed again.
    let changed = grants(Tool {
        key: "admin".to_owned(),
        ..kubectl()
    });
    assert_eq!(armed(&changed), None);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_persona_without_tools_has_nothing_to_resolve() {
    assert_eq!(
        super::resolve(Path::new("/p"), "devops", &[], &Locks::none(), &[], &[]),
        None
    );
}

#[test]
fn the_shim_runs_secret_exec_with_the_absolute_program_and_every_argument() {
    let text = shim(
        Path::new("/Applications/Purlis.app/Contents/MacOS/purlis"),
        "devops",
        &Ready {
            tool: Tool {
                key: "it's".to_owned(),
                ..kubectl()
            },
            program: PathBuf::from("/opt/homebrew/bin/kubectl"),
        },
    );
    assert!(text.starts_with("#!/bin/sh\n"), "{text}");
    // A `secret exec` run's child has no hook channel: there the shim is the program itself, so
    // `purlis secret exec devops -- kubectl` never runs a second `secret exec` inside the first.
    assert!(
        text.contains("if [ -z \"${PURLIS_HOOK_SOCKET:-}${CHARTER_HOOK_SOCKET:-}\" ]; then\n"),
        "{text}"
    );
    assert!(
        text.contains("exec '/opt/homebrew/bin/kubectl' \"$@\"\nfi\n"),
        "{text}"
    );
    assert!(
        text.ends_with(
            "exec '/Applications/Purlis.app/Contents/MacOS/purlis' 'secret' 'exec' 'devops' \
             '--file' 'KUBECONFIG=it'\\''s' '--' '/opt/homebrew/bin/kubectl' \"$@\"\n"
        ),
        "{text}"
    );
}

#[test]
fn shims_are_written_and_an_earlier_start_s_are_taken_away() {
    let base = outside_temp("written");
    let root = base.join("project");
    let system = base.join("system");
    program(&system, "kubectl");
    let resolved = resolve(
        &root,
        "devops",
        &[kubectl()],
        &Locks::none(),
        std::slice::from_ref(&system),
        &[],
    )
    .expect("tools");
    let stale = resolved.folder.join("bin").join("helm");
    std::fs::create_dir_all(stale.parent().unwrap()).expect("dir");
    std::fs::write(&stale, "old").expect("old");
    let written = resolved
        .write(Path::new("/x/purlis"))
        .expect("written")
        .expect("one ready");
    assert!(written.bin.starts_with(&root), "{}", written.bin.display());
    assert!(written.bin.join("kubectl").is_file());
    assert!(!stale.exists(), "an earlier start's shim stays");
    let env = std::fs::read_to_string(&written.env_file).expect("env file");
    assert!(env.contains(&written.bin.display().to_string()), "{env}");
    // Under the app's state, which every chat is denied writing.
    assert!(
        written
            .bin
            .components()
            .any(|part| part.as_os_str() == "app"),
        "{}",
        written.bin.display()
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_claude_code_chat_also_gets_the_env_file_and_others_the_path_alone() {
    let written = super::Written {
        bin: PathBuf::from("/p/.purlis/app/tools/devops/bin"),
        env_file: PathBuf::from("/p/.purlis/app/tools/devops/claude-env.sh"),
    };
    let claude = written.env(crate::harness::Harness::ClaudeCode);
    assert!(claude.contains(&(
        "CLAUDE_ENV_FILE".to_owned(),
        "/p/.purlis/app/tools/devops/claude-env.sh".to_owned()
    )));
    let codex = written.env(crate::harness::Harness::Codex);
    assert_eq!(
        codex,
        [(
            FIRST_ON_PATH.to_owned(),
            "/p/.purlis/app/tools/devops/bin".to_owned()
        )]
    );
}

#[test]
fn the_tools_folder_goes_first_on_path_and_the_hand_over_is_taken_out() {
    let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    assert_eq!(
        first_on_path(
            vec![
                pair(FIRST_ON_PATH, "/t/bin"),
                pair("PATH", "/usr/bin:/t/bin:/bin")
            ],
            Some("/ignored".to_owned()),
        ),
        [pair("PATH", "/t/bin:/usr/bin:/bin")]
    );
    assert_eq!(
        first_on_path(
            vec![pair(FIRST_ON_PATH, "/t/bin")],
            Some("/app:/usr/bin".to_owned())
        ),
        [pair("PATH", "/t/bin:/app:/usr/bin")]
    );
    assert_eq!(
        first_on_path(vec![pair("PATH", "/usr/bin")], None),
        [pair("PATH", "/usr/bin")],
        "no tools, no change"
    );
    assert_eq!(
        first_on_path(vec![pair(FIRST_ON_PATH, "/t/bin"), pair("A", "b")], None),
        [pair("A", "b")],
        "no PATH to go in front of: the inherited one stands, never the tools alone"
    );
}

// ---- what a brokered `secret exec` from a chat may run (D-PE-ruling-2) -----------------------

fn armed_kubectl() -> Vec<Ready> {
    vec![Ready {
        tool: kubectl(),
        program: PathBuf::from("/opt/tools/bin/kubectl"),
    }]
}

fn asked<'a>(program: &'a str, file: &'a [String]) -> super::Asked<'a> {
    super::Asked {
        vault: "devops",
        env: &[],
        file,
        dotenv: &[],
        program,
    }
}

#[test]
fn a_declared_tool_named_bare_or_by_its_program_runs_as_that_program() {
    let file = vec!["KUBECONFIG=kubeconfig".to_owned()];
    let armed = armed_kubectl();
    for named in ["kubectl", "/opt/tools/bin/kubectl"] {
        let ready = super::declared(&armed, &[kubectl()], "devops", &asked(named, &file))
            .expect("declared");
        assert_eq!(ready.program, PathBuf::from("/opt/tools/bin/kubectl"));
    }
    // Another program of the same name, as a chat's PATH or a path might name it, is not.
    let other = super::declared(&armed, &[kubectl()], "devops", &asked("./kubectl", &file))
        .expect_err("not the tool's program");
    assert!(
        other.contains("is devops's tool, and it is not armed"),
        "{other}"
    );
    let elsewhere =
        super::declared(&armed, &[], "devops", &asked("/tmp/x/kubectl", &file)).expect_err("no");
    assert!(elsewhere.contains("kubectl is not one"), "{elsewhere}");
}

#[test]
fn a_tool_declared_and_not_armed_here_says_what_arms_it() {
    let file = vec!["KUBECONFIG=kubeconfig".to_owned()];
    let why = super::declared(&[], &[kubectl()], "devops", &asked("kubectl", &file))
        .expect_err("not armed");
    assert!(
        why.contains("kubectl is devops's tool, and it is not armed in this chat"),
        "{why}"
    );
    assert!(why.contains("allowed on this machine"), "{why}");
    assert!(why.contains("Start the chat again"), "{why}");
}

#[test]
fn an_undeclared_program_is_refused_naming_the_tool_to_declare_and_a_shell_never_is() {
    let env = vec!["TOKEN=api".to_owned()];
    let curl = super::Asked {
        vault: "team",
        env: &env,
        file: &[],
        dotenv: &[],
        program: "curl",
    };
    let why = super::declared(&armed_kubectl(), &[kubectl()], "devops", &curl).expect_err("no");
    assert!(
        why.contains(
            "[sandbox.personas.devops] tools = { curl = { vault = \"team\", as = \"<VARIABLE> \
             file\", key = \"<key>\" } }"
        ),
        "{why}"
    );
    let bash = super::Asked {
        program: "/bin/bash",
        ..curl
    };
    let why = super::declared(&armed_kubectl(), &[], "devops", &bash).expect_err("no");
    assert!(why.contains("bash cannot be one"), "{why}");
}

#[test]
fn a_tool_runs_with_its_own_vault_and_hand_over_and_nothing_more() {
    let armed = armed_kubectl();
    let own = vec!["KUBECONFIG=kubeconfig".to_owned()];
    let other_key = vec!["KUBECONFIG=admin".to_owned()];
    let two = vec![own[0].clone(), "OTHER=admin".to_owned()];
    assert!(super::declared(&armed, &[], "devops", &asked("kubectl", &own)).is_ok());
    for file in [&other_key, &two, &Vec::new()] {
        let why = super::declared(&armed, &[], "devops", &asked("kubectl", file))
            .expect_err("not its hand-over");
        assert!(why.contains("a chat runs it with exactly that"), "{why}");
        assert!(
            why.contains("`purlis secret exec devops --file KUBECONFIG=kubeconfig -- kubectl`"),
            "{why}"
        );
    }
    let as_env = super::Asked {
        env: &own,
        file: &[],
        ..asked("kubectl", &[])
    };
    assert!(super::declared(&armed, &[], "devops", &as_env).is_err());
    let another_vault = super::Asked {
        vault: "team",
        ..asked("kubectl", &own)
    };
    assert!(super::declared(&armed, &[], "devops", &another_vault).is_err());
}

#[test]
fn a_tool_run_starts_from_a_path_without_the_folders_a_chat_writes() {
    let root = PathBuf::from("/p/project");
    let granted = PathBuf::from("/p/granted");
    let path = "/p/project/node_modules/.bin:/usr/bin:relative:/p/granted/bin:/opt/tools/bin";
    let kept = super::path_outside(path, &root, &[granted]);
    assert_eq!(kept, "/usr/bin:/opt/tools/bin");
}
