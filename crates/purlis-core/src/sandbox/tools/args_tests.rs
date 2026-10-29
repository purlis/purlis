// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A declared tool runs only with the options the person controls (#3421): its `args`, read,
//! spelled on its line, and held to on each run; and the forge command lines' built-in refusals.

use std::path::PathBuf;

use super::{Handed, Ready, Tool, keyed, run_words};

fn read(text: &str) -> (Vec<Tool>, Vec<String>) {
    let value: toml::Value = toml::from_str(text).expect("TOML");
    let (tools, refused) = keyed(
        value.get("tools"),
        "sandbox.personas.devops",
        "charter.toml",
        &["sandbox", "personas", "devops"],
    );
    (tools, refused.into_iter().map(|one| one.why).collect())
}

fn tool_with(name: &str, args: Option<&[&str]>) -> Tool {
    let mut tool = Tool::one(name, "devops", "KUBECONFIG", Handed::File, "kubeconfig");
    tool.args = args.map(|args| args.iter().map(|one| (*one).to_owned()).collect());
    tool
}

fn ready(tool: Tool, program: &str) -> Ready {
    Ready {
        tool,
        program: PathBuf::from(program),
    }
}

fn said(words: &[&str]) -> Vec<String> {
    words.iter().map(|one| (*one).to_owned()).collect()
}

fn runs(ready: &Ready, words: &[&str]) -> Result<Vec<String>, String> {
    run_words(ready, &said(words))
}

// ---- reading `args` ---------------------------------------------------------------------------

#[test]
fn a_tool_s_args_are_read_in_the_order_written_and_absent_means_none() {
    let (tools, refused) = read(
        r#"tools = { kubectl = { vault = "devops", as = "KUBECONFIG file", key = "kubeconfig", args = ["get", "-n", "*"] },
                     helm = { vault = "devops", as = "KUBECONFIG file", key = "kubeconfig" } }"#,
    );
    assert_eq!(refused, Vec::<String>::new());
    let helm = tools.iter().find(|tool| tool.name == "helm").expect("helm");
    let kubectl = tools
        .iter()
        .find(|tool| tool.name == "kubectl")
        .expect("kubectl");
    assert_eq!(helm.args, None);
    assert_eq!(kubectl.args, Some(said(&["get", "-n", "*"])));
}

#[test]
fn args_go_with_a_tool_handed_several_values_too() {
    let (tools, refused) = read(
        r#"tools = { curl = { vault = "logs", args = [], values = {
            A = { key = "a", as = "env" }, B = { key = "b", as = "file" } } } }"#,
    );
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(tools[0].args, Some(Vec::new()));
}

#[test]
fn faulty_args_leave_the_tool_out_whole_with_a_sentence() {
    let one = |args: &str| {
        format!(
            r#"tools = {{ kubectl = {{ vault = "devops", as = "KUBECONFIG file", key = "k", args = {args} }} }}"#
        )
    };
    let many = format!(
        "[{}]",
        (0..65)
            .map(|n| format!("\"a{n}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let long = format!("[\"{}\"]", "a".repeat(257));
    for (text, says) in [
        (one("\"get\""), "args is not an array"),
        (one("[1]"), "args holds 1, which is not a string"),
        (one("[\"\"]"), "an empty pattern"),
        (one("[\"a\\tb\"]"), "control character"),
        (one(&many), "at most 64"),
        (one(&long), "longer than 256"),
        (
            r#"tools = { shop = { vault = "web", browser = "https://shop.example.test/login", key = "pw", args = [] } }"#
                .to_owned(),
            "a browser login runs no program",
        ),
    ] {
        let (tools, refused) = read(&text);
        assert!(tools.is_empty(), "{text}");
        assert!(
            refused.iter().any(|why| why.contains(says)),
            "{text}: {refused:?} should say {says:?}"
        );
    }
}

// ---- the line an Allow is bound to ------------------------------------------------------------

#[test]
fn a_tool_s_line_names_its_args_and_one_without_keeps_its_line() {
    assert_eq!(
        tool_with("kubectl", None).whole(),
        "kubectl with kubeconfig from vault devops as KUBECONFIG file"
    );
    assert_eq!(
        tool_with("kubectl", Some(&["get", "--namespace=*"])).whole(),
        "kubectl with kubeconfig from vault devops as KUBECONFIG file, only with arguments \
         matching \"get\", \"--namespace=*\""
    );
    assert_eq!(
        tool_with("kubectl", Some(&[])).whole(),
        "kubectl with kubeconfig from vault devops as KUBECONFIG file, only with no arguments"
    );
}

#[test]
fn any_change_to_args_is_a_change_to_the_digest() {
    let digest = |tool: Tool| crate::sandbox::persona::digest_of(&[], &[tool], false);
    let none = digest(tool_with("kubectl", None));
    let get = digest(tool_with("kubectl", Some(&["get"])));
    let get_n = digest(tool_with("kubectl", Some(&["get", "-n"])));
    let empty = digest(tool_with("kubectl", Some(&[])));
    for (a, b) in [
        (&none, &get),
        (&get, &get_n),
        (&none, &empty),
        (&empty, &get),
    ] {
        assert_ne!(a, b);
    }
}

// ---- a run held to them ------------------------------------------------------------------------

#[test]
fn a_run_goes_ahead_only_where_every_argument_matches_a_pattern() {
    let kubectl = ready(
        tool_with(
            "kubectl",
            Some(&["get", "describe", "-n", "--namespace=*", "*"]),
        ),
        "/usr/bin/kubectl",
    );
    assert_eq!(
        runs(&kubectl, &["get", "pods", "-n", "web", "--namespace=web"]).expect("runs"),
        said(&[
            "/usr/bin/kubectl",
            "get",
            "pods",
            "-n",
            "web",
            "--namespace=web"
        ])
    );
    let why = runs(&kubectl, &["get", "pods", "--kubeconfig=/tmp/other"]).expect_err("refused");
    assert!(why.contains("--kubeconfig=/tmp/other"), "{why}");
    assert!(why.contains("tools.kubectl.args"), "{why}");
}

#[test]
fn an_option_is_let_in_only_by_a_pattern_that_spells_one() {
    let operands = ready(tool_with("kubectl", Some(&["*"])), "/usr/bin/kubectl");
    assert!(runs(&operands, &["get", "pods"]).is_ok());
    for option in ["-v", "--raw", "--", "-"] {
        assert!(runs(&operands, &["get", option]).is_err(), "{option}");
    }
    let options = ready(
        tool_with("kubectl", Some(&["-*", "get"])),
        "/usr/bin/kubectl",
    );
    assert!(runs(&options, &["get", "--raw", "-o"]).is_ok());
    assert!(runs(&options, &["get", "pods"]).is_err());
}

#[test]
fn a_star_stands_for_any_run_of_characters_and_nothing_else_is_special() {
    let tool = ready(
        tool_with("kubectl", Some(&["--output=*json", "pod?", "a*b*c"])),
        "/usr/bin/kubectl",
    );
    for fine in [
        "--output=json",
        "--output=jsonjson",
        "pod?",
        "abc",
        "a-b-c",
        "axxbyyc",
    ] {
        assert!(runs(&tool, &[fine]).is_ok(), "{fine}");
    }
    for refused in [
        "--output=yaml",
        "pods",
        "pod1",
        "ab",
        "abcd",
        "--output=json ",
    ] {
        assert!(runs(&tool, &[refused]).is_err(), "{refused}");
    }
}

#[test]
fn empty_args_runs_the_tool_with_no_argument_and_absent_args_with_any() {
    let none = ready(tool_with("kubectl", Some(&[])), "/usr/bin/kubectl");
    assert!(runs(&none, &[]).is_ok());
    assert!(runs(&none, &["version"]).is_err());
    let any = ready(tool_with("kubectl", None), "/usr/bin/kubectl");
    assert!(runs(&any, &["config", "--kubeconfig", "x", "-v=9"]).is_ok());
}

// ---- the forge command lines' built-in refusals -------------------------------------------------

#[test]
fn credential_and_settings_subcommands_of_gh_never_run_whatever_args_say() {
    let gh = ready(tool_with("gh", Some(&["*", "-*"])), "/opt/homebrew/bin/gh");
    for words in [
        &["auth", "token"][..],
        &["auth", "status", "--show-token"],
        &["config", "set", "editor", "vim"],
        &["alias", "set", "x", "y"],
        &["extension", "install", "a/b"],
        &["extensions", "list"],
        &["ext", "exec", "x"],
        &["ssh-key", "add", "k.pub"],
        &["gpg-key", "list"],
        &["repo", "deploy-key", "add", "k.pub"],
        &["AUTH", "token"],
    ] {
        let why = runs(&gh, words).expect_err("refused");
        assert!(why.contains("own terminal"), "{words:?}: {why}");
    }
    let open = ready(tool_with("gh", None), "/opt/homebrew/bin/gh");
    assert!(runs(&open, &["auth", "token"]).is_err());
    assert!(runs(&open, &["pr", "view", "12"]).is_ok());
    assert!(runs(&open, &["repo", "view"]).is_ok());
}

#[test]
fn glab_has_its_own_and_both_refuse_api_against_another_host() {
    let glab = ready(tool_with("glab", None), "/usr/local/bin/glab");
    for words in [
        &["token", "create"][..],
        &["deploy-key", "add"],
        &["auth", "status"],
        &["config", "get", "token"],
        &["api", "--hostname", "evil.example", "user"],
        &["api", "--hostname=evil.example", "user"],
        &["api", "https://evil.example/api/v4/user"],
    ] {
        assert!(runs(&glab, words).is_err(), "{words:?}");
    }
    assert!(runs(&glab, &["api", "projects/1"]).is_ok());
    assert!(runs(&glab, &["mr", "list"]).is_ok());
    let gh = ready(tool_with("gh", None), "/opt/homebrew/bin/gh");
    assert!(runs(&gh, &["api", "--hostname", "evil.example", "user"]).is_err());
    assert!(runs(&gh, &["api", "repos/a/b/pulls"]).is_ok());
}

#[test]
fn a_forge_run_starts_with_its_command_not_an_option() {
    let gh = ready(tool_with("gh", None), "/opt/homebrew/bin/gh");
    assert!(runs(&gh, &["--repo", "x", "auth", "token"]).is_err());
    assert!(runs(&gh, &["-R", "a/b", "pr", "list"]).is_err());
    assert!(runs(&gh, &["--version"]).is_ok());
    assert!(runs(&gh, &["--help"]).is_ok());
    assert!(runs(&gh, &[]).is_ok());
}

#[test]
fn the_refusals_follow_the_program_not_only_the_tool_s_name() {
    // A tool named otherwise whose program is gh, and one named gh whose program is gh.exe.
    let renamed = ready(tool_with("github", None), "/opt/homebrew/bin/gh");
    assert!(runs(&renamed, &["auth", "token"]).is_err());
    let exe = ready(tool_with("tool", None), "C:/bin/GH.EXE");
    assert!(runs(&exe, &["auth", "token"]).is_err());
    let other = ready(tool_with("kubectl", None), "/usr/bin/kubectl");
    assert!(runs(&other, &["auth", "can-i", "get", "pods"]).is_ok());
}
