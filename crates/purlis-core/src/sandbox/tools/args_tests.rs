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
    assert!(runs(&glab, &["api", "projects/1/issues"]).is_ok());
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

// ---- the audit of each forge's whole command set (#3894) ----------------------------------------

#[test]
fn commands_that_show_or_change_stored_secrets_never_run() {
    let gh = ready(tool_with("gh", None), "/opt/homebrew/bin/gh");
    for words in [
        &["secret", "list"][..],
        &["secret", "set", "TOKEN", "--body", "x"],
        &["repo", "set-default", "a/b"],
    ] {
        assert!(runs(&gh, words).is_err(), "{words:?}");
    }
    assert!(runs(&gh, &["variable", "list"]).is_ok());
    let glab = ready(tool_with("glab", None), "/usr/local/bin/glab");
    for words in [
        &["variable", "get", "TOKEN"][..],
        &["variable", "export"],
        &["securefile", "download", "1"],
        &["duo", "cli"],
        &["opentofu", "init", "production"],
        &["cluster", "agent", "get-token", "--agent", "1"],
        &["cluster", "agent", "token", "create"],
        &["cluster", "agent", "token-cache", "list"],
        &["cluster", "agent", "bootstrap", "a"],
        &["cluster", "agent", "update-kubeconfig"],
        &["runner-controller", "token", "create"],
    ] {
        assert!(runs(&glab, words).is_err(), "{words:?}");
    }
    // glab's aliases, which no help page names, are refused as the commands they stand for.
    for words in [
        &["var", "get", "TOKEN"][..],
        &["conf", "get", "token"],
        &["tf", "init", "s"],
        &["terraform", "state", "download", "s"],
        &["rc", "token", "create", "1"],
        &["mcp", "serve"],
        &["ci", "get", "--with-variables"],
        &["pipe", "get", "--with-variables=true"],
        &["repo", "view", "-F", "json"],
        &["project", "view", "--output=json"],
        &["repo", "view", "-Fjson"],
    ] {
        assert!(runs(&glab, words).is_err(), "{words:?}");
    }
    assert!(runs(&glab, &["-v"]).is_ok());
    assert!(runs(&glab, &["repo", "view"]).is_ok());
    assert!(runs(&glab, &["ci", "get"]).is_ok());
    assert!(runs(&glab, &["cluster", "agent", "list"]).is_ok());
    assert!(runs(&glab, &["opentofu", "state", "list"]).is_ok());
    assert!(runs(&glab, &["runner", "list"]).is_ok());
}

#[test]
fn api_never_reaches_a_route_that_does_what_a_refused_command_does() {
    let gh = ready(tool_with("gh", None), "/opt/homebrew/bin/gh");
    for words in [
        &["api", "user/keys"][..],
        &["api", "/user/keys/12", "-X", "DELETE"],
        &[
            "api",
            "-X",
            "POST",
            "user/gpg_keys",
            "-f",
            "armored_public_key=x",
        ],
        &["api", "user/ssh_signing_keys"],
        &["api", "repos/{owner}/{repo}/keys"],
        &["api", "repos/a/b/actions/secrets/X"],
        &["api", "repos/a/b/environments/prod/secrets"],
        &["api", "orgs/a/dependabot/secrets"],
        &["api", "user/codespaces/secrets"],
        &[
            "api",
            "repos/a/b/actions/runners/registration-token",
            "-X",
            "POST",
        ],
        &["api", "orgs/a/actions/runners/generate-jitconfig"],
        &["api", "orgs/a/personal-access-tokens"],
        &["api", "repositories/12/keys"],
        &["api", "organizations/9/actions/secrets"],
        // Spelled otherwise: case, a query, doubled and trailing slashes, an escaped letter.
        &["api", "USER/Keys"],
        &["api", "user/keys?per_page=1"],
        &["api", "user//keys/"],
        &["api", "user/%6beys"],
        &["api", "api/v3/user/keys"],
        // Read as a server resolves its dot segments.
        &["api", "repos/a/b/../../../user/keys"],
        &["api", "orgs/a/credential-authorizations"],
        &["api", "installation/token", "-X", "DELETE"],
        &["api", "app-manifests/c/conversions"],
    ] {
        assert!(runs(&gh, words).is_err(), "{words:?}");
    }
    for words in [
        &["api", "repos/a/b/pulls"][..],
        &["api", "repos/keys/secrets/issues"],
        &["api", "user"],
        &["api", "search/issues?q=a%20b"],
        &["api", "repos/a/b/actions/variables"],
        &["api", "users/a/keys"],
        &["api", "repos/a/b/./pulls"],
        &["api", "user", "--jq", "."],
        &[
            "api",
            "repos/a/b/issues",
            "--input",
            "./body.json",
            "-X",
            "POST",
        ],
    ] {
        assert!(runs(&gh, words).is_ok(), "{words:?}");
    }
    let glab = ready(tool_with("glab", None), "/usr/local/bin/glab");
    for words in [
        &["api", "user/keys"][..],
        &["api", "user/gpg_keys"],
        &["api", "projects/:id/deploy_keys"],
        &["api", "projects/a%2Fb/deploy_tokens"],
        &["api", "groups/1/deploy_tokens"],
        &["api", "personal_access_tokens/self/rotate", "-X", "POST"],
        &["api", "users/2/impersonation_tokens"],
        &["api", "projects/1/access_tokens"],
        &["api", "groups/1/access_tokens"],
        &["api", "projects/1/variables/TOKEN"],
        &["api", "groups/1/variables"],
        &["api", "admin/ci/variables"],
        &["api", "projects/1/pipeline_schedules/3/variables"],
        &["api", "projects/1/secure_files/2/download"],
        &["api", "projects/1/triggers"],
        &["api", "projects/1/cluster_agents/2/tokens"],
        &["api", "runners/reset_registration_token"],
        &["api", "runners/5/reset_authentication_token"],
        &["api", "user/runners"],
        &["api", "applications"],
        &["api", "api/v4/projects/1/variables"],
        // Grape routes a format after a dot.
        &["api", "projects/1/variables.json"],
        &["api", "-X", "POST", "user/keys.json", "-f", "title=x"],
        &["api", "projects/1/pipelines/2/variables"],
        &["api", "projects/1/pipeline_schedules/3"],
        &["api", "projects/1/terraform/state/prod"],
        &["api", "runners/reset_authentication_token"],
        &["api", "application/settings"],
        // A project's and a group's own page carry a runner token for a maintainer.
        &["api", "projects/:id"],
        &["api", "groups/7.json"],
    ] {
        assert!(runs(&glab, words).is_err(), "{words:?}");
    }
    for words in [
        &["api", "projects/1/issues"][..],
        &["api", "projects/a%2Fb/merge_requests"],
    ] {
        assert!(runs(&glab, words).is_ok(), "{words:?}");
    }
}

#[test]
fn graphql_never_asks_for_a_credential_and_glab_s_never_runs() {
    let gh = ready(tool_with("gh", None), "/opt/homebrew/bin/gh");
    for words in [
        &[
            "api",
            "graphql",
            "-f",
            "query={repository(owner:\"a\",name:\"b\"){tempCloneToken}}",
        ][..],
        &[
            "api",
            "graphql",
            "-f",
            "query={enterprise(slug:\"a\"){ownerInfo{samlIdentityProvider{RecoveryCodes}}}}",
        ],
        &[
            "api",
            "graphql",
            "-f",
            "query=mutation{regenerateVerifiableDomainToken(input:{id:\"x\"}){verificationToken}}",
        ],
        // A query read from a file or stdin is never seen, so it never runs.
        &["api", "graphql", "-F", "query=@q.graphql"],
        &["api", "graphql", "--input", "q.json"],
        &["api", "graphql", "--input=-"],
    ] {
        assert!(runs(&gh, words).is_err(), "{words:?}");
    }
    assert!(runs(&gh, &["api", "graphql", "-f", "query={viewer{login}}"]).is_ok());
    let glab = ready(tool_with("glab", None), "/usr/local/bin/glab");
    assert!(
        runs(
            &glab,
            &["api", "graphql", "-f", "query={currentUser{username}}"]
        )
        .is_err()
    );
}

#[test]
fn what_follows_a_double_dash_goes_to_another_program_so_it_never_runs() {
    let gh = ready(tool_with("gh", None), "/opt/homebrew/bin/gh");
    assert!(runs(&gh, &["repo", "clone", "o/r", "--", "--template=./t"]).is_err());
    let glab = ready(tool_with("glab", None), "/usr/local/bin/glab");
    assert!(runs(&glab, &["repo", "clone", "g/p", "--", "--depth=1"]).is_err());
    assert!(runs(&gh, &["repo", "clone", "o/r"]).is_ok());
}

#[test]
fn the_re_review_s_forms_never_run_and_lists_still_do() {
    let glab = ready(tool_with("glab", None), "/usr/local/bin/glab");
    for words in [
        &["api", "../v4/projects/1/variables"][..],
        &["api", "../v4/user/keys"],
        &["api", "../../api/graphql", "-f", "query={x}"],
        &["api", "projects/:namespace/variables"],
        &["schedule", "create", "--variable", "K:v"],
        &["sched", "update", "1", "--variable=K:v"],
        &["orbit", "local", "x"],
        &["repo", "view", "-wFjson"],
    ] {
        assert!(runs(&glab, words).is_err(), "{words:?}");
    }
    for words in [
        &["repo", "list", "-F", "json"][..],
        &["project", "list", "--jq", ".[]"],
        &["schedule", "list"],
    ] {
        assert!(runs(&glab, words).is_ok(), "{words:?}");
    }
    let gh = ready(tool_with("gh", None), "/opt/homebrew/bin/gh");
    for words in [
        &["api", "{branch}"][..],
        &["api", "repos/{owner}/{repo}/git/refs/heads/{branch}"],
        &["api", "../v3/user/keys"],
    ] {
        assert!(runs(&gh, words).is_err(), "{words:?}");
    }
    assert!(runs(&gh, &["api", "repos/{owner}/{repo}/pulls"]).is_ok());
}
