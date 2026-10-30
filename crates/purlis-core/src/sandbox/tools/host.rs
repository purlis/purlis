// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The forge host a forge tool's run reaches** (#3869): the one its declaration names.
//!
//! ```toml
//! [sandbox.personas.devops]
//! tools = { gh = { vault = "forge", as = "GH_ENTERPRISE_TOKEN env", key = "token",
//!                  host = "github.example.com" } }
//! ```
//!
//! A tool whose program is `gh` or `glab` reaches `host`, or `github.com` and `gitlab.com`
//! where it names none. The run is told it ([`env`]: `GH_HOST`, `GITLAB_HOST`), so a command
//! that names no host, and the remotes the program picks, are that host's; and a run whose
//! arguments name another host (a repository as `HOST/OWNER/REPO`, a URL, `git@host:…`) never
//! runs ([`named_elsewhere`]). `host` on a tool of any other program never runs.

/// The key of a tool's table that names its forge host.
pub const KEY: &str = "host";

/// **A tool's `host`**, lower-cased, or why it is not taken: a host's name (letters, digits,
/// dots and dashes, each label between dots non-empty), with a port after a colon where it has
/// one; never a URL, a path, a user or a space.
pub fn of(value: &toml::Value) -> Result<String, String> {
    let example = "host = \"github.example.com\"";
    let Some(host) = value.as_str() else {
        return Err(format!("host is not a host's name — write {example}"));
    };
    let host = host.trim().to_ascii_lowercase();
    let (name, port) = match host.split_once(':') {
        Some((name, port)) => (name, Some(port)),
        None => (host.as_str(), None),
    };
    let name_ok = !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    let port_ok = port.is_none_or(|port| port.parse::<u16>().is_ok_and(|port| port > 0));
    if !name_ok || !port_ok {
        return Err(format!(
            "host {host:?} is not a host's name, with a port where it has one — write {example}"
        ));
    }
    Ok(host)
}

/// How `host` reads at the end of its tool's line.
pub fn spelled(host: &str) -> String {
    format!(", on {host}")
}

/// The host a forge's run reaches where its tool names none.
pub fn default_of(glab: bool) -> &'static str {
    if glab { "gitlab.com" } else { "github.com" }
}

/// **The variable that tells a forge's program its host**: `GITLAB_HOST` for glab, `GH_HOST`
/// for gh.
pub fn env(glab: bool, host: &str) -> (String, String) {
    let var = if glab { "GITLAB_HOST" } else { "GH_HOST" };
    (var.to_owned(), host.to_owned())
}

/// **Why a forge run with `args` never runs with a vault's value**, if it does not: one of
/// them names a host other than `host` ([`host_named`]). A repository's own option (`-R`,
/// `--repo`) names a host by its first of three parts for gh, and for glab where that part
/// holds a dot (glab's groups nest).
pub fn named_elsewhere(forge: &str, glab: bool, host: &str, args: &[String]) -> Option<String> {
    let mut repo_next = false;
    for arg in args {
        let (value, repo) = if repo_next {
            (arg.as_str(), true)
        } else if let Some(value) = arg.strip_prefix("--repo=") {
            (value, true)
        } else if let Some(value) = arg.strip_prefix("-R").filter(|value| !value.is_empty()) {
            (value, true)
        } else if arg.starts_with('-') {
            (arg.split_once('=').map_or("", |(_, value)| value), false)
        } else {
            (arg.as_str(), false)
        };
        repo_next = !repo_next && (arg == "-R" || arg == "--repo");
        let Some(named) = host_named(value, repo && !glab) else {
            continue;
        };
        if !same_host(&named, host) {
            return Some(format!(
                "{forge} runs with a vault's value only against {host}, the forge its tool \
                 names, and {arg:?} names {named}, so nothing ran: name a repository on {host}, \
                 or run it in your own terminal, where purlis limits nothing."
            ));
        }
    }
    None
}

/// **The host `value` names, if it names one**: a URL's (`https://host/…`, `ssh://git@host/…`),
/// an scp-style remote's (`git@host:path`, or `host:path` where the host holds a dot), or a
/// repository's first part where it is one of three or more (`host/owner/repo`), always where
/// `three_parts` and otherwise where that part holds a dot. Lower-cased.
fn host_named(value: &str, three_parts: bool) -> Option<String> {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return None;
    }
    if let Some((_, rest)) = value.split_once("://") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let host = authority.rsplit('@').next().unwrap_or_default();
        return Some(host.to_ascii_lowercase());
    }
    if let Some((before, after)) = value.split_once(':') {
        if !before.contains('/') && !after.is_empty() {
            if let Some((_, host)) = before.rsplit_once('@') {
                return Some(host.to_ascii_lowercase());
            }
            if before.contains('.') {
                return Some(before.to_ascii_lowercase());
            }
        }
        return None;
    }
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() >= 3 && !parts[0].is_empty() && (three_parts || parts[0].contains('.')) {
        return Some(parts[0].to_ascii_lowercase());
    }
    None
}

/// Whether `named` is `host`: the same name, and the same port where `host` has one.
fn same_host(named: &str, host: &str) -> bool {
    if host.contains(':') {
        return named == host;
    }
    named.split(':').next() == Some(host)
}
