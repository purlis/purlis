// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A forge tool's run reaches only the forge host the person declared (#3869).

use std::path::PathBuf;

use super::{Handed, Ready, Tool, forge_env, keyed, run_words};

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

fn ready(name: &str, host: Option<&str>, program: &str) -> Ready {
    let mut tool = Tool::one(name, "forge", "GH_TOKEN", Handed::Env, "token");
    tool.host = host.map(str::to_owned);
    Ready {
        tool,
        program: PathBuf::from(program),
    }
}

fn runs(ready: &Ready, words: &[&str]) -> Result<Vec<String>, String> {
    let said: Vec<String> = words.iter().map(|one| (*one).to_owned()).collect();
    run_words(ready, &said)
}

#[test]
fn a_tool_s_host_is_read_lower_cased_and_named_on_its_line() {
    let (tools, refused) = read(
        "[tools]\ngh = { vault = \"forge\", as = \"GH_ENTERPRISE_TOKEN env\", key = \"t\", host = \"GitHub.Example.com\" }\n",
    );
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(tools[0].host.as_deref(), Some("github.example.com"));
    assert!(
        tools[0].whole().ends_with(", on github.example.com"),
        "{}",
        tools[0].whole()
    );
    // A tool without one keeps the line, and the Allow, it had.
    let (plain, _) =
        read("[tools]\ngh = { vault = \"forge\", as = \"GH_TOKEN env\", key = \"t\" }\n");
    assert!(!plain[0].whole().contains(", on "), "{}", plain[0].whole());
}

#[test]
fn a_host_that_is_not_a_host_s_name_leaves_the_tool_out() {
    for host in [
        "\"https://github.com\"",
        "\"github.com/x\"",
        "\"\"",
        "\"a b.com\"",
        "\"me@host\"",
        "3",
        "\"github.com:x\"",
    ] {
        let (tools, refused) = read(&format!(
            "[tools]\ngh = {{ vault = \"forge\", as = \"GH_TOKEN env\", key = \"t\", host = {host} }}\n"
        ));
        assert!(tools.is_empty(), "{host}");
        assert_eq!(refused.len(), 1, "{host}");
    }
    let (tools, _) = read(
        "[tools]\ngh = { vault = \"forge\", as = \"GH_TOKEN env\", key = \"t\", host = \"ghe.example.com:8443\" }\n",
    );
    assert_eq!(tools[0].host.as_deref(), Some("ghe.example.com:8443"));
}

#[test]
fn a_host_on_a_tool_that_is_not_a_forge_s_never_runs() {
    let kubectl = ready("kubectl", Some("github.com"), "/usr/bin/kubectl");
    assert!(runs(&kubectl, &["get", "pods"]).is_err());
}

#[test]
fn a_run_naming_another_host_never_runs() {
    let gh = ready("gh", None, "/opt/homebrew/bin/gh");
    for words in [
        &["issue", "list", "-R", "evil.example/o/r"][..],
        &["issue", "list", "--repo", "evil.example/o/r"],
        &["issue", "list", "--repo=evil.example/o/r"],
        &["issue", "list", "-Revil.example/o/r"],
        &["issue", "list", "-R", "localhost/o/r"],
        &["repo", "clone", "evil.example/o/r"],
        &["pr", "view", "https://evil.example/o/r/pull/1"],
        &["repo", "clone", "git@evil.example:o/r.git"],
        &["repo", "clone", "ssh://git@evil.example/o/r"],
        &[
            "pr",
            "checkout",
            "https://github.com.evil.example/o/r/pull/1",
        ],
        // A host with a port, or bracketed, and gh's own repository operands.
        &["issue", "list", "-R", "localhost:8080/o/r"],
        &["repo", "clone", "intranet:443/o/r"],
        &["issue", "list", "-R", "[::1]/o/r"],
        &["repo", "clone", "intranet/o/r"],
        &["repo", "view", "localhost/o/r"],
        // As pflag reads options, and go-gh a repository.
        &["issue", "view", "-cRevil.example/o/r", "1"],
        &["pr", "list", "-wR", "evil.example/o/r"],
        &["issue", "list", "-R", "evil.example/o/r "],
        &["issue", "list", "-R", "[2001:db8::1]/o/r"],
        &["issue", "list", "-R", "evil:8443/o/r"],
        &["issue", "transfer", "1", "intranet/o/r"],
        &["repo", "sync", "--source", "intranet/o/r"],
        &["attestation", "verify", "x", "--hostname", "evil.example"],
        &["attestation", "download", "x", "--hostname=evil.example"],
    ] {
        let why = runs(&gh, words).expect_err("refused");
        assert!(why.contains("github.com"), "{words:?}: {why}");
    }
    for words in [
        &["issue", "list", "-R", "o/r"][..],
        &["issue", "list", "-R", "github.com/o/r"],
        &["pr", "view", "https://github.com/o/r/pull/1"],
        &["pr", "create", "--head", "owner:branch"],
        &["release", "upload", "v1", "dist/app.tar.gz"],
        &["pr", "list", "--json", "number,title"],
        &["release", "upload", "v1", "./dist/app.tar.gz"],
        &["gist", "create", "../notes/a/x.md"],
        &["issue", "create", "--body", "see https://docs.rs/x"],
        &["gist", "create", "src/a/b.rs"],
        &["workflow", "view", ".github/workflows/ci.yml"],
        &["repo", "create", "x", "--source", "./a/b/c"],
        &["attestation", "verify", "x", "--hostname", "github.com"],
    ] {
        assert!(runs(&gh, words).is_ok(), "{words:?}");
    }
}

#[test]
fn glab_s_groups_are_not_hosts_and_a_declared_host_is_the_one_reached() {
    let glab = ready("glab", Some("gitlab.example.com"), "/usr/local/bin/glab");
    assert!(runs(&glab, &["issue", "list", "-R", "group/sub/project"]).is_ok());
    assert!(runs(&glab, &["issue", "list", "-R", "gitlab.example.com/g/p"]).is_ok());
    assert!(
        runs(
            &glab,
            &[
                "mr",
                "view",
                "https://GitLab.Example.com/g/p/-/merge_requests/1"
            ]
        )
        .is_ok()
    );
    assert!(runs(&glab, &["issue", "list", "-R", "gitlab.com/g/p"]).is_err());
    assert!(runs(&glab, &["repo", "clone", "git@gitlab.com:g/p.git"]).is_err());
    let on_port = ready("gh", Some("ghe.example.com:8443"), "/opt/homebrew/bin/gh");
    assert!(
        runs(
            &on_port,
            &["pr", "view", "https://ghe.example.com:8443/o/r/pull/1"]
        )
        .is_ok()
    );
    assert!(
        runs(
            &on_port,
            &["pr", "view", "https://ghe.example.com/o/r/pull/1"]
        )
        .is_err()
    );
}

#[test]
fn a_forge_run_is_told_its_host_and_another_program_is_told_nothing() {
    assert_eq!(
        forge_env(&ready("gh", None, "/opt/homebrew/bin/gh")),
        vec![("GH_HOST".to_owned(), "github.com".to_owned())]
    );
    assert_eq!(
        forge_env(&ready(
            "forge",
            Some("gitlab.example.com"),
            "/usr/local/bin/glab"
        )),
        vec![
            ("GITLAB_HOST".to_owned(), "gitlab.example.com".to_owned()),
            ("GLAB_SEND_TELEMETRY".to_owned(), "false".to_owned()),
            ("GLAB_CHECK_UPDATE".to_owned(), "false".to_owned()),
        ]
    );
    assert!(forge_env(&ready("kubectl", None, "/usr/bin/kubectl")).is_empty());
}

#[test]
fn what_follows_a_double_dash_goes_to_git_so_it_never_runs() {
    let gh = ready("gh", None, "/opt/homebrew/bin/gh");
    assert!(runs(&gh, &["repo", "clone", "o/r", "--", "--template=./t"]).is_err());
    let glab = ready("glab", None, "/usr/local/bin/glab");
    assert!(runs(&glab, &["repo", "clone", "g/p", "--", "--depth=1"]).is_err());
}

/// #1349 part 2: a `glab` tool on the clone's self-hosted GitLab runs `glab api` and `glab mr` on
/// that host, and no run that names gitlab.com or any other host: the token a chat is handed is
/// the host's own and goes to no other.
#[test]
fn a_glab_tool_on_a_self_hosted_gitlab_reaches_that_host_and_no_other() {
    let glab = ready("glab", Some("git.acme.internal"), "/usr/local/bin/glab");
    for ok in [
        &["api", "projects/7/merge_requests"][..],
        &["mr", "list"],
        &["mr", "view", "3", "-R", "git.acme.internal/acme/svc"],
        &["mr", "list", "-R", "acme/sub/svc"],
    ] {
        assert!(runs(&glab, ok).is_ok(), "{ok:?}");
    }
    for refused in [
        &["api", "https://git.acme.internal/api/v4/user"][..],
        &["api", "https://gitlab.com/api/v4/user"],
        &["api", "--hostname", "gitlab.com", "user"],
        &["api", "--hostname=gitlab.com", "user"],
        &["mr", "list", "-R", "gitlab.com/acme/svc"],
        &[
            "mr",
            "view",
            "https://gitlab.com/acme/svc/-/merge_requests/3",
        ],
        // A lookalike, and a port the declared host does not name (none: the default port only).
        &[
            "api",
            "--hostname",
            "git.acme.internal.evil.example",
            "user",
        ],
        &["api", "--hostname", "evil-git.acme.internal", "user"],
        &["api", "--hostname", "git.acme.internal:9999", "user"],
        &["api", "https://git.acme.internal:9999/api/v4/user"],
        &["mr", "list", "-R", "git.acme.internal:9999/acme/svc"],
        // A lookalike that is not an `api` run.
        &[
            "mr",
            "list",
            "-R",
            "git.acme.internal.evil.example/acme/svc",
        ],
        &["mr", "list", "-R", "evil-git.acme.internal/acme/svc"],
        &[
            "repo",
            "clone",
            "git@git.acme.internal.evil.example:acme/svc.git",
        ],
        &["repo", "clone", "ssh://git@git.acme.internal:2222/acme/svc"],
    ] {
        let said = runs(&glab, refused).expect_err(&format!("{refused:?}"));
        // `glab api` refuses any other host by what it names; a repository elsewhere, by the
        // forge it reaches: a refusal says which, never only that it was refused.
        if refused[0] == "api" {
            assert!(
                said.contains("names another host") && said.contains("against git.acme.internal"),
                "{said}"
            );
        } else {
            assert!(
                said.contains("only against git.acme.internal") && said.contains("names "),
                "{said}"
            );
        }
    }
    // The run is told its host, so a command that names none reaches it.
    assert_eq!(
        forge_env(&glab),
        [
            ("GITLAB_HOST".to_owned(), "git.acme.internal".to_owned()),
            ("GLAB_SEND_TELEMETRY".to_owned(), "false".to_owned()),
            ("GLAB_CHECK_UPDATE".to_owned(), "false".to_owned()),
        ]
    );
}

/// An explicit default port is the default port: `:443` (https) and `:22` (ssh) against a
/// declaration with no port are allowed, any other port is not, and a declared port is exact.
#[test]
fn an_explicit_default_port_is_the_host_s_own_and_no_other_port_is() {
    let glab = ready("glab", Some("git.acme.internal"), "/usr/local/bin/glab");
    for ok in [
        &[
            "mr",
            "view",
            "https://git.acme.internal:443/acme/svc/-/merge_requests/3",
        ][..],
        &["repo", "clone", "ssh://git@git.acme.internal:22/acme/svc"],
        &["mr", "list", "-R", "git.acme.internal:443/acme/svc"],
    ] {
        assert!(runs(&glab, ok).is_ok(), "{ok:?}");
    }
    for refused in [
        &[
            "mr",
            "view",
            "https://git.acme.internal:80/acme/svc/-/merge_requests/3",
        ][..],
        &["repo", "clone", "ssh://git@git.acme.internal:2222/acme/svc"],
        &["mr", "list", "-R", "git.acme.internal:4430/acme/svc"],
    ] {
        assert!(runs(&glab, refused).is_err(), "{refused:?}");
    }
    let on_port = ready("gh", Some("ghe.example.com:8443"), "/opt/homebrew/bin/gh");
    assert!(
        runs(
            &on_port,
            &["pr", "view", "https://ghe.example.com:443/o/r/pull/1"]
        )
        .is_err()
    );
}
