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

/// What an argument is read as, for the host it names.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Read {
    /// A repository, as gh reads `[HOST/]OWNER/REPO`: a URL's host, or the first of three or
    /// more parts, whatever it holds (glab's groups nest, so for glab, as [`Read::Other`]).
    Repo,
    /// A host's name alone (`--hostname`).
    Host,
    /// Anything else: a URL, an scp-style remote, or a first of three parts that looks like a
    /// host's name; a sentence (with a space) names none.
    Other,
}

/// **Why a forge run with `args` never runs with a vault's value**, if it does not: `--`,
/// which hands what follows to git; or an argument naming a host other than `host`, read as
/// pflag reads options (`-R` alone, joined, or last in a run of short options; `--repo`,
/// `--source`, `--hostname`, alone or with `=`) and go-gh reads a repository: the value of
/// `-R`/`--repo`/`--source`, and an operand of `gh repo` or `gh issue transfer`, name a host by
/// their first of three parts.
pub fn named_elsewhere(forge: &str, glab: bool, host: &str, args: &[String]) -> Option<String> {
    let terminal = "run it in your own terminal, where purlis limits nothing";
    if args.iter().any(|arg| arg == "--") {
        return Some(format!(
            "{forge} runs with a vault's value only with its own arguments, and \"--\" hands \
             the rest to git, so nothing ran; {terminal}."
        ));
    }
    let word = |at: usize| args.get(at).map(String::as_str);
    let repo_operands = !glab
        && (word(0) == Some("repo") || (word(0) == Some("issue") && word(1) == Some("transfer")));
    let mut pending: Option<Read> = None;
    for arg in args {
        let mut judged: Vec<(&str, Read)> = Vec::new();
        if let Some(read) = pending.take() {
            judged.push((arg, read));
        } else if let Some(long) = arg.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            let read = match name {
                "repo" | "source" => Read::Repo,
                "hostname" => Read::Host,
                _ => Read::Other,
            };
            match value {
                Some(value) => judged.push((value, read)),
                None if read != Read::Other => pending = Some(read),
                None => {}
            }
        } else if let Some(short) = arg.strip_prefix('-').filter(|short| !short.is_empty()) {
            // pflag reads a run of short options up to one that takes a value, which takes
            // the rest: `-cRhost/o/r`.
            if let Some((_, rest)) = short.split_once('R') {
                let rest = rest.strip_prefix('=').unwrap_or(rest);
                if rest.is_empty() {
                    pending = Some(Read::Repo);
                } else {
                    judged.push((rest, Read::Repo));
                }
            }
        } else {
            judged.push((
                arg,
                if repo_operands {
                    Read::Repo
                } else {
                    Read::Other
                },
            ));
        }
        for (value, read) in judged {
            let read = if glab && read == Read::Repo {
                Read::Other
            } else {
                read
            };
            let Some(named) = host_named(value, read) else {
                continue;
            };
            if !same_host(&named, host) {
                return Some(format!(
                    "{forge} runs with a vault's value only against {host}, the forge its tool \
                     names, and {arg:?} names {named}, so nothing ran: name a repository on \
                     {host}, or {terminal}."
                ));
            }
        }
    }
    None
}

/// **The host `value` names, read as `read`, if it names one**, lower-cased: a URL's
/// (`https://host/…`, `ssh://git@host/…`) and an scp-style remote's (`git@host:path`, or
/// `host:path` where the host holds a dot) as any; for [`Read::Repo`], the first of three or
/// more parts, spaces about it aside; for [`Read::Host`], the value; for [`Read::Other`], a
/// first of three parts holding a dot, a port or brackets and not starting with a dot (a
/// hidden folder's), and nothing in a value with a space.
fn host_named(value: &str, read: Read) -> Option<String> {
    let value = if read == Read::Other {
        if value.chars().any(char::is_whitespace) {
            return None;
        }
        value
    } else {
        value.trim()
    };
    if value.is_empty() {
        return None;
    }
    if read == Read::Host {
        return Some(value.to_ascii_lowercase());
    }
    if let Some((_, rest)) = value.split_once("://") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let host = authority.rsplit('@').next().unwrap_or_default();
        return Some(host.to_ascii_lowercase());
    }
    let parts: Vec<&str> = value.split('/').collect();
    let first = parts[0];
    if let Some((user_host, _)) = first.split_once(':').filter(|_| first.contains('@')) {
        return user_host
            .rsplit_once('@')
            .map(|(_, host)| host.to_ascii_lowercase());
    }
    let looks_like_host =
        !first.starts_with('.') && first.contains(['.', ':', '[']) && !first.is_empty();
    if parts.len() >= 3
        && !first.is_empty()
        && !first.starts_with('.')
        && (read == Read::Repo || looks_like_host)
    {
        return Some(
            first
                .rsplit('@')
                .next()
                .unwrap_or(first)
                .to_ascii_lowercase(),
        );
    }
    if parts.len() == 1 || parts.len() == 2 {
        if let Some((before, after)) = first.split_once(':') {
            if !after.is_empty() && !before.starts_with('.') && before.contains('.') {
                return Some(before.to_ascii_lowercase());
            }
        }
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
