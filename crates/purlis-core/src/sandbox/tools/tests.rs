// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A persona's tools (#1407): what is read, what is refused, where the real program may come
//! from, and the shim that runs it through `purlis secret exec`.

use std::path::{Path, PathBuf};

use super::super::policy::Locks;
use super::{
    FIRST_ON_PATH, Handed, Ready, Tool, caution, first_on_path, general_purpose, keyed, loads_code,
    refused_name, refused_program, resolve_beside, shim,
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
    Tool::one(
        "kubectl",
        "devops",
        "KUBECONFIG",
        Handed::File,
        "kubeconfig",
    )
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
            Tool::one("psql", "devops", "PGPASSWORD", Handed::Env, "db/password"),
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
            program: real.canonicalize().expect("real"),
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
                browser: None,
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
            None,
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
            program: real.canonicalize().expect("real"),
        }]
    );
    // A teammate hands it another key: no shim until it is allowed again.
    let changed = grants(Tool::one(
        "kubectl",
        "devops",
        "KUBECONFIG",
        Handed::File,
        "admin",
    ));
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
            tool: Tool::one("kubectl", "devops", "KUBECONFIG", Handed::File, "it's"),
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
    // A tool that needs more than one value is told the form that hands it several (T-1).
    assert!(
        why.contains(
            "(for several values, values = { <VARIABLE> = { key = \"<key>\", as = \"file\" }, \
             … } in place of as and key)"
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

/// D-PE-ruling-2: what makes a program load or run code it names is taken out of a brokered
/// run's environment, and nothing else is.
#[test]
fn the_loaders_and_runtimes_start_up_variables_load_code_and_others_do_not() {
    for var in [
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
        "NODE_OPTIONS",
        "PYTHONPATH",
        "PERL5OPT",
        "RUBYOPT",
        "BASH_ENV",
        "JAVA_TOOL_OPTIONS",
        "GCONV_PATH",
        "LOCPATH",
        "GIO_EXTRA_MODULES",
        "GTK_MODULES",
    ] {
        assert!(loads_code(var), "{var}");
    }
    for var in [
        "PATH",
        "HOME",
        "KUBECONFIG",
        "LANG",
        "NODE_ENV",
        "PYTHONUNBUFFERED",
    ] {
        assert!(!loads_code(var), "{var}");
    }
}

// ---- a tool handed several values (T-1, the operator's ruling of 2026-10-14) ----------------

const KIBANA: &str = r#"tools = { curl = { vault = "logs", values = {
    KIBANA_USER = { key = "kibana-username", as = "env" },
    KIBANA_PASS = { key = "kibana-password", as = "file" } } } }"#;

fn value(var: &str, handed: Handed, key: &str) -> super::Value {
    super::Value {
        var: var.to_owned(),
        handed,
        key: key.to_owned(),
    }
}

fn kibana() -> Tool {
    Tool {
        name: "curl".to_owned(),
        vault: "logs".to_owned(),
        values: vec![
            value("KIBANA_PASS", Handed::File, "kibana-password"),
            value("KIBANA_USER", Handed::Env, "kibana-username"),
        ],
        login: None,
        args: None,
        mint: None,
        host: None,
    }
}

#[test]
fn a_tool_is_read_with_each_of_its_values_in_the_order_of_their_variables() {
    let (tools, refused) = read(KIBANA);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(tools, [kibana()]);
}

#[test]
fn a_tool_written_both_ways_or_with_a_faulty_value_is_left_out_whole() {
    let several = |values: &str| {
        format!(r#"tools = {{ curl = {{ vault = "logs", values = {{ {values} }} }} }}"#)
    };
    for (text, says) in [
        (
            r#"tools = { curl = { vault = "logs", key = "k", as = "A env", values = { B = { key = "k", as = "env" } } } }"#.to_owned(),
            "both values and key",
        ),
        (
            r#"tools = { curl = { vault = "logs", as = "A env", values = { B = { key = "k", as = "env" } } } }"#.to_owned(),
            "both values and as",
        ),
        (
            r#"tools = { curl = { vault = "logs", browser = "https://a.example.test/", values = { B = { key = "k", as = "file" } } } }"#.to_owned(),
            "both values and browser",
        ),
        (
            several(r#"KIBANA_USER = { key = "u", as = "env" }, kibana_user = { key = "p", as = "file" }"#),
            "one variable twice",
        ),
        (several(r#"PATH = { key = "k", as = "env" }"#), "PATH decides"),
        (several(r#"LD_PRELOAD = { key = "k", as = "file" }"#), "LD_PRELOAD decides"),
        (several(r#"DYLD_LIBRARY_PATH = { key = "k", as = "file" }"#), "DYLD_LIBRARY_PATH decides"),
        (several(r#"PURLIS_X = { key = "k", as = "env" }"#), "PURLIS_X decides"),
        (several(r#"CHARTER_X = { key = "k", as = "env" }"#), "CHARTER_X decides"),
        (several(r#"HOME = { key = "k", as = "env" }"#), "HOME decides"),
        (several(r#""1ST" = { key = "k", as = "env" }"#), "not a variable's name"),
        (several(r#""A-B" = { key = "k", as = "env" }"#), "not a variable's name"),
        (
            several(r#"A = { key = "k", as = "env", run = "x" }"#),
            "values.A.run is not a key purlis reads",
        ),
        (several(r#"A = { key = "k", as = "A env" }"#), "neither \"file\" nor \"env\""),
        (several(r#"A = { key = "k" }"#), "does not say as what"),
        (several(r#"A = { as = "env" }"#), "names no key"),
        (several(r#"A = { key = " ", as = "env" }"#), "names no key"),
        (several(r#"A = "k""#), "not a table of key and as"),
        (several(""), "names no value"),
        (
            r#"tools = { curl = { vault = "logs", values = ["A"] } }"#.to_owned(),
            "values is not a table",
        ),
    ] {
        let (tools, refused) = read(&text);
        assert_eq!(tools, Vec::<Tool>::new(), "{text}");
        assert_eq!(refused.len(), 1, "{text}: {refused:?}");
        assert!(refused[0].contains(says), "{text}: {}", refused[0]);
        assert!(refused[0].contains("runs nothing"), "{}", refused[0]);
    }
    // The same variable written twice is not TOML at all: the file is refused before purlis
    // reads a tool from it.
    assert!(
        toml::from_str::<toml::Value>(
            r#"tools = { curl = { vault = "logs", values = { A = { key = "u", as = "env" }, A = { key = "p", as = "file" } } } }"#
        )
        .is_err()
    );
}

#[test]
fn a_tool_is_handed_at_most_sixteen_values() {
    let values = |n: usize| {
        (0..n)
            .map(|i| format!(r#"V{i} = {{ key = "k{i}", as = "file" }}"#))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let text = |n: usize| {
        format!(
            r#"tools = {{ curl = {{ vault = "logs", values = {{ {} }} }} }}"#,
            values(n)
        )
    };
    assert_eq!(super::MOST_VALUES, 16);
    let (tools, refused) = read(&text(16));
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(tools[0].values.len(), 16);
    let (tools, refused) = read(&text(17));
    assert_eq!(tools, Vec::<Tool>::new());
    assert!(
        refused[0].contains("names 17 values, and a tool is handed at most 16"),
        "{refused:?}"
    );
}

#[test]
fn a_tool_s_line_names_every_value_with_its_key_quoted() {
    assert_eq!(
        kibana().whole(),
        "curl with \"kibana-password\" as KIBANA_PASS file and \"kibana-username\" as \
         KIBANA_USER env from vault logs"
    );
    let three = Tool {
        values: vec![
            value("A", Handed::Env, "a"),
            value("B", Handed::File, "b"),
            value("C", Handed::Env, "c"),
        ],
        ..kibana()
    };
    assert_eq!(
        three.whole(),
        "curl with \"a\" as A env, \"b\" as B file and \"c\" as C env from vault logs"
    );
    // One value is the one-value form, whichever way it is written: the same line and Allow.
    let (written_once, _) = read(
        r#"tools = { kubectl = { vault = "devops", values = { KUBECONFIG = { key = "kubeconfig", as = "file" } } } }"#,
    );
    assert_eq!(written_once, [kubectl()]);
    assert_eq!(
        kubectl().whole(),
        "kubectl with kubeconfig from vault devops as KUBECONFIG file"
    );
}

/// T-1: the Allow covers the whole declaration. Any change to any value's variable, key or
/// hand-over, a value added or taken away, or the vault, asks again; the order the values are
/// written in does not.
#[test]
fn the_allow_is_bound_to_every_value_and_not_to_the_order_they_are_written_in() {
    let digest =
        |tool: &Tool| super::super::persona::digest_of(&[], std::slice::from_ref(tool), false);
    let (reordered, refused) = read(
        r#"tools = { curl = { vault = "logs", values = {
            KIBANA_PASS = { as = "file", key = "kibana-password" },
            KIBANA_USER = { as = "env", key = "kibana-username" } } } }"#,
    );
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(digest(&reordered[0]), digest(&kibana()));
    let changed = |change: &dyn Fn(&mut Tool)| {
        let mut tool = kibana();
        change(&mut tool);
        tool
    };
    let others = [
        changed(&|tool| tool.values[0].key = "admin-password".to_owned()),
        changed(&|tool| tool.values[1].key = "admin-username".to_owned()),
        changed(&|tool| tool.values[0].var = "ES_PASS".to_owned()),
        changed(&|tool| tool.values[1].var = "ES_USER".to_owned()),
        changed(&|tool| tool.values[0].handed = Handed::Env),
        changed(&|tool| tool.values[1].handed = Handed::File),
        changed(&|tool| {
            tool.values.swap(0, 1);
            let (a, b) = (tool.values[0].key.clone(), tool.values[1].key.clone());
            tool.values[0].key = b;
            tool.values[1].key = a;
            tool.values.swap(0, 1);
        }),
        changed(&|tool| {
            tool.values.pop();
        }),
        changed(&|tool| {
            tool.values
                .push(value("KIBANA_SPACE", Handed::Env, "space"))
        }),
        changed(&|tool| tool.vault = "other".to_owned()),
    ];
    for other in &others {
        assert_ne!(digest(other), digest(&kibana()), "{}", other.whole());
    }
}

#[test]
fn the_shim_hands_every_value_the_way_it_is_declared() {
    let text = shim(
        Path::new("/Applications/Purlis.app/Contents/MacOS/purlis"),
        "logs",
        &Ready {
            tool: kibana(),
            program: PathBuf::from("/usr/bin/curl"),
        },
    );
    assert!(
        text.ends_with(
            "exec '/Applications/Purlis.app/Contents/MacOS/purlis' 'secret' 'exec' 'logs' \
             '--file' 'KIBANA_PASS=kibana-password' '--env' 'KIBANA_USER=kibana-username' \
             '--' '/usr/bin/curl' \"$@\"\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("hands it KIBANA_PASS and KIBANA_USER from the logs vault"),
        "{text}"
    );
}

/// T-1, from the hostile side: a chat's `secret exec` of a tool handed several values runs only
/// with exactly the declared set. Any other vault, a subset, a superset, the same pair twice,
/// another pairing of variable and key, a file where an env was declared or the other way, or a
/// `--dotenv`, is refused with a sentence naming the declared line, and never names a value.
#[test]
fn a_tool_handed_several_values_runs_only_with_exactly_its_declared_set() {
    let armed = vec![Ready {
        tool: kibana(),
        program: PathBuf::from("/usr/bin/curl"),
    }];
    let strings = |words: &[&str]| words.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
    let ask = |vault: &str, file: &[&str], env: &[&str], dotenv: &[&str]| {
        let (file, env, dotenv) = (strings(file), strings(env), strings(dotenv));
        super::declared(
            &armed,
            &[kibana()],
            "logs",
            &super::Asked {
                vault,
                env: &env,
                file: &file,
                dotenv: &dotenv,
                program: "curl",
            },
        )
        .map(|ready| ready.tool.clone())
    };
    let pass = "KIBANA_PASS=kibana-password";
    let user = "KIBANA_USER=kibana-username";
    assert_eq!(ask("logs", &[pass], &[user], &[]), Ok(kibana()));
    /// A vault, then the `--file`, `--env` and `--dotenv` hand-overs asked for.
    type Asking<'a> = (&'a str, &'a [&'a str], &'a [&'a str], &'a [&'a str]);
    let hostile: [Asking<'_>; 13] = [
        ("other", &[pass], &[user], &[]),
        ("logs", &[pass], &[], &[]),
        ("logs", &[], &[user], &[]),
        ("logs", &[], &[], &[]),
        ("logs", &[pass], &[user, "KIBANA_SPACE=space"], &[]),
        ("logs", &[pass, "EXTRA=kibana-username"], &[user], &[]),
        ("logs", &[pass, pass], &[user], &[]),
        ("logs", &[pass], &[user, user], &[]),
        (
            "logs",
            &["KIBANA_PASS=kibana-username"],
            &["KIBANA_USER=kibana-password"],
            &[],
        ),
        ("logs", &[pass, user], &[], &[]),
        ("logs", &[], &[pass, user], &[]),
        ("logs", &[user], &[pass], &[]),
        ("logs", &[pass], &[user], &["F=A:kibana-password"]),
    ];
    for (vault, file, env, dotenv) in hostile {
        let why = ask(vault, file, env, dotenv).expect_err("refused");
        assert!(
            why.contains("is logs's tool curl with \"kibana-password\" as KIBANA_PASS file and"),
            "{vault} {file:?} {env:?} {dotenv:?}: {why}"
        );
        assert!(
            why.contains(
                "`purlis secret exec logs --file KIBANA_PASS=kibana-password --env \
                 KIBANA_USER=kibana-username -- curl`"
            ),
            "{why}"
        );
    }
}

/// A tool named `name` handed one token, as a declaration writes it.
fn token_tool(name: &str) -> Tool {
    Tool::one(name, "api", "API_TOKEN", Handed::Env, "api-token")
}

#[test]
fn a_general_purpose_program_is_known_by_its_file_name_whatever_its_folder_or_case() {
    for program in [
        "curl",
        "/usr/bin/curl",
        "/opt/homebrew/bin/wget",
        "CURL",
        "/usr/local/bin/Http",
        "xh",
        "socat",
        "rsync",
        "ssh",
        "curl.exe",
        // Programs that copy, encode or rewrite what they are given.
        "base64",
        "/usr/bin/xxd",
        "Tee",
        "/usr/bin/openssl",
        // Forge command lines, which make whatever request their arguments name.
        "gh",
        "/opt/homebrew/bin/gh",
        "/usr/local/bin/GLAB",
        // So is a name no tool may take.
        "/usr/bin/python3.12",
        "/bin/bash",
    ] {
        assert!(general_purpose(Path::new(program)), "{program}");
    }
    for program in [
        "kubectl",
        "/usr/local/bin/aws",
        "psql",
        "ghq",
        "curlish",
        "/usr/bin/curl/kubectl",
        "",
    ] {
        assert!(!general_purpose(Path::new(program)), "{program}");
    }
}

#[test]
fn a_general_purpose_tool_says_so_in_one_sentence_and_an_unlisted_one_says_nothing() {
    let said = caution(&token_tool("curl"), &[]).expect("curl is general-purpose");
    assert_eq!(
        said,
        "curl can show the values it is handed in the chat's output; allow it only for a \
         persona you trust with them."
    );
    assert_eq!(
        caution(&token_tool("Wget"), &[])
            .as_deref()
            .map(|s| s.starts_with("Wget ")),
        Some(true)
    );
    assert_eq!(caution(&token_tool("aws"), &[]), None);
    assert!(caution(&token_tool("base64"), &[]).is_some());
    assert_eq!(caution(&kubectl(), &[]), None);
    for forge in ["gh", "glab"] {
        assert_eq!(
            caution(&token_tool(forge), &[]),
            Some(format!(
                "{forge} can show the values it is handed in the chat's output; allow it only \
                 for a persona you trust with them."
            )),
            "{forge}"
        );
    }
}

#[test]
fn a_tool_whose_name_finds_a_general_purpose_program_says_so_too() {
    let base = outside_temp("general");
    let system = base.join("system");
    let real = program(&base.join("cellar"), "curl");
    std::fs::create_dir_all(&system).expect("dir");
    std::os::unix::fs::symlink(&real, system.join("fetch")).expect("link");
    program(&base.join("other"), "kubectl");
    let search = vec![base.join("other"), system.clone()];
    let said = caution(&token_tool("fetch"), &search).expect("fetch runs curl");
    assert!(said.starts_with("fetch can show"), "{said}");
    assert_eq!(caution(&kubectl(), &search), None);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_sentence_is_not_part_of_what_an_allow_is_bound_to() {
    // An Allow made before the sentence existed still stands: the digest is of the tool's whole
    // line, as it was.
    assert_eq!(
        super::super::persona::digest_of(&[], &[token_tool("curl")], false),
        "2a4a76e02cadfb54839e461f5a7fa833809de99504ec817e9bdc382a318ff7c7"
    );
    assert_eq!(
        token_tool("curl").whole(),
        "curl with api-token from vault api as API_TOKEN env"
    );
}

#[test]
fn a_tool_whose_program_runs_what_it_is_told_is_refused_as_found_and_as_its_links_resolve() {
    for target in ["bash", "Python3.12", "env"] {
        let base = outside_temp("judged");
        let system = base.join("system");
        let real = program(&base.join("elsewhere"), target);
        std::fs::create_dir_all(&system).expect("dir");
        std::os::unix::fs::symlink(&real, system.join("fetch")).expect("link");
        let resolved = resolve(
            &base.join("project"),
            "devops",
            &[token_tool("fetch")],
            &Locks::none(),
            std::slice::from_ref(&system),
            &[],
        )
        .expect("tools");
        assert_eq!(resolved.ready, Vec::<Ready>::new(), "{target}");
        assert_eq!(resolved.left.len(), 1, "{target}: {:?}", resolved.left);
        assert!(
            resolved.left[0].starts_with("devops's tool fetch runs nothing: fetch runs whatever"),
            "{target}: {:?}",
            resolved.left
        );
        let _ = std::fs::remove_dir_all(&base);
    }
    // A program that runs only itself, under a link of another name, is the tool's.
    let base = outside_temp("judged-fine");
    let system = base.join("system");
    let real = program(&base.join("elsewhere"), "kubectl-1.31");
    std::fs::create_dir_all(&system).expect("dir");
    std::os::unix::fs::symlink(&real, system.join("kubectl")).expect("link");
    let resolved = resolve(
        &base.join("project"),
        "devops",
        &[kubectl()],
        &Locks::none(),
        std::slice::from_ref(&system),
        &[],
    )
    .expect("tools");
    assert_eq!(resolved.ready.len(), 1, "{:?}", resolved.left);
    assert_eq!(refused_program(&kubectl(), &system.join("kubectl")), None);
    assert!(refused_program(&token_tool("fetch"), Path::new("/bin/SH")).is_some());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_tool_runs_the_program_its_start_judged_by_its_resolved_path() {
    let base = outside_temp("bound");
    let system = base.join("system");
    let real = program(&base.join("elsewhere"), "kubectl-1.31");
    std::fs::create_dir_all(&system).expect("dir");
    std::os::unix::fs::symlink(&real, system.join("kubectl")).expect("link");
    let resolved = resolve(
        &base.join("project"),
        "devops",
        &[kubectl()],
        &Locks::none(),
        std::slice::from_ref(&system),
        &[],
    )
    .expect("tools");
    assert_eq!(
        resolved.ready,
        [Ready {
            tool: kubectl(),
            program: real.canonicalize().expect("real"),
        }]
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn every_link_on_the_way_to_a_tool_s_program_is_where_no_chat_writes() {
    let base = outside_temp("chain");
    let system = base.join("system");
    let granted = base.join("granted");
    let real = program(&base.join("elsewhere"), "kubectl");
    std::fs::create_dir_all(&system).expect("dir");
    std::fs::create_dir_all(&granted).expect("dir");
    // A link in a folder no chat writes, to a link in one a chat writes, to the program.
    std::os::unix::fs::symlink(&real, granted.join("middle")).expect("link");
    std::os::unix::fs::symlink(granted.join("middle"), system.join("kubectl")).expect("link");
    let resolved = resolve(
        &base.join("project"),
        "devops",
        &[kubectl()],
        &Locks::none(),
        std::slice::from_ref(&system),
        std::slice::from_ref(&granted),
    )
    .expect("tools");
    assert_eq!(resolved.ready, Vec::<Ready>::new());
    assert!(
        resolved.left[0].contains("no kubectl was found"),
        "{:?}",
        resolved.left
    );
    // A folder on the way that is a link through one a chat writes is the same.
    let via = base.join("via");
    std::fs::create_dir_all(granted.join("bin")).expect("dir");
    std::os::unix::fs::symlink(&real, granted.join("bin/kubectl")).expect("link");
    std::os::unix::fs::symlink(&granted, via.with_extension("d")).expect("link");
    std::fs::create_dir_all(&via).expect("dir");
    std::os::unix::fs::symlink(
        via.with_extension("d").join("bin/kubectl"),
        via.join("kubectl"),
    )
    .expect("link");
    let resolved = resolve(
        &base.join("project"),
        "devops",
        &[kubectl()],
        &Locks::none(),
        std::slice::from_ref(&via),
        std::slice::from_ref(&granted),
    )
    .expect("tools");
    assert_eq!(resolved.ready, Vec::<Ready>::new(), "{:?}", resolved.left);
    // With nothing on the way a chat writes, the same chain runs the program itself.
    let resolved = resolve(
        &base.join("project"),
        "devops",
        &[kubectl()],
        &Locks::none(),
        std::slice::from_ref(&system),
        &[],
    )
    .expect("tools");
    assert_eq!(resolved.ready.len(), 1, "{:?}", resolved.left);
    assert_eq!(
        resolved.ready[0].program,
        real.canonicalize().expect("real")
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// `unxz`, declared as a tool, found as a link to xz: what it runs is xz, by its own path.
fn unxz_at(xz: &Path) -> Ready {
    Ready {
        tool: Tool::one("unxz", "devops", "T", Handed::Env, "TOKEN"),
        program: xz.to_path_buf(),
    }
}

#[test]
fn a_tool_whose_program_has_another_name_hands_secret_exec_the_declared_one() {
    let text = shim(
        Path::new("/Applications/Purlis.app/Contents/MacOS/purlis"),
        "devops",
        &unxz_at(Path::new("/opt/homebrew/Cellar/xz/5.8.4/bin/xz")),
    );
    assert!(
        text.ends_with(
            "exec '/Applications/Purlis.app/Contents/MacOS/purlis' 'secret' 'exec' 'devops' \
             '--env' 'T=TOKEN' '--argv0' 'unxz' '--' '/opt/homebrew/Cellar/xz/5.8.4/bin/xz' \
             \"$@\"\n"
        ),
        "{text}"
    );
}

#[test]
fn a_tool_named_as_its_program_is_handed_no_other_name() {
    let text = shim(
        Path::new("/purlis"),
        "devops",
        &Ready {
            tool: Tool::one("kubectl", "devops", "T", Handed::Env, "TOKEN"),
            program: PathBuf::from("/opt/homebrew/bin/kubectl"),
        },
    );
    assert!(!text.contains("argv0"), "{text}");
    assert!(!text.contains("exec -a"), "{text}");
}

/// The shim with no app to ask, run as a chat runs it (through its own `#!/bin/sh`), runs the
/// program itself: under the declared name where that shell has `exec -a` (macOS's `sh` is
/// bash), and by its own path where it has not (dash, Debian's and Ubuntu's `sh`), which still
/// runs and fails nothing.
#[test]
fn a_shim_with_no_app_names_its_program_where_its_own_shell_can_and_runs_it_where_it_cannot() {
    use std::os::unix::fs::PermissionsExt;
    let Some(xz) = crate::testmulticall::xz() else {
        let _ =
            std::io::Write::write_all(&mut std::io::stderr(), b"skipped: no xz on this machine\n");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let file = crate::testmulticall::packed(&xz, tmp.path(), "unpacked\n");
    let script = tmp.path().join("unxz");
    std::fs::write(&script, shim(Path::new("/purlis"), "devops", &unxz_at(&xz))).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let out = crate::forklock::output(
        std::process::Command::new(&script)
            .arg("-c")
            .arg(&file)
            .env_remove("PURLIS_HOOK_SOCKET")
            .env_remove("CHARTER_HOOK_SOCKET"),
    )
    .unwrap();
    assert!(out.status.success(), "{out:?}");
    let names = crate::forklock::status(
        std::process::Command::new("/bin/sh").args(["-c", "(exec -a probe true) 2>/dev/null"]),
    )
    .unwrap()
    .success();
    if names {
        // Started as `unxz`: unpacked. `xz -c` would pack it again.
        assert_eq!(String::from_utf8_lossy(&out.stdout), "unpacked\n");
    } else {
        // Started by its own name: xz packed the file again, as `xz -c` does.
        assert!(out.stdout.starts_with(b"\xfd7zXZ\0"), "{out:?}");
    }
}
