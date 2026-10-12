// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's network git uses only your own network settings, never a repository's.
//!
//! A call that crosses a network ([`super::run_network`]) reads the repository's own config, and
//! a chat can write that file. The settings that decide where git connects, what it trusts there
//! and what it sends besides the credential are yours to choose, in your global or system config,
//! or purlis's: the `http.*` keys (a proxy, TLS and certificate settings, extra headers, the
//! rest), a remote's own proxy (`remote.<name>.proxy`, `.proxyAuthMethod`), and an address
//! rewrite (`url.<base>.insteadOf`, `.pushInsteadOf`) that takes a call to another host.
//!
//! **Held where a command line can hold it, refused where it cannot.** Each such key the
//! repository sets is given again on git's command line, which beats every config file at the
//! same URL: with the value your own config gives the same key, or with git's own default where
//! your config says nothing about that setting at all ([`DEFAULTS`]). A key that can be held
//! neither way, and any address rewrite to another host (no command line removes a rewrite),
//! refuses the call with a sentence naming the kind of setting, never its value.
//!
//! **A few keys stay the repository's** ([`ITS_OWN`]): buffer sizes, timeouts, the number of
//! requests and the HTTP version. They change how fast a transfer goes, not where it goes or
//! what it carries.
//!
//! **`clone` and `init` are not checked**: they make a repository whose config nobody has
//! written yet, and read no config of the repository they are started in.

/// The `http.*` variables a repository may keep: how a transfer is paced, never where it goes,
/// what it trusts or what it sends. As git lists them, lowercased.
const ITS_OWN: [&str; 7] = [
    "postbuffer",
    "lowspeedlimit",
    "lowspeedtime",
    "maxrequests",
    "minsessions",
    "version",
    "noepsv",
];

/// git's own default for an `http.*` variable, spelled so that a command line giving it is the
/// same as nothing set: `extraheader`'s empty value resets the list to none. `proxy` is handled
/// apart ([`held`]): an empty one also turns off a proxy your environment names.
const DEFAULTS: [(&str, &str); 3] = [
    ("sslverify", "true"),
    ("extraheader", ""),
    ("followredirects", "initial"),
];

/// The `http.*` variables git keeps a list of rather than one value: your own value cannot be
/// given back by one more entry, only the empty list.
const LISTS: [&str; 2] = ["extraheader", "curloptresolve"];

/// The kind of network setting a refusal names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Proxy,
    Tls,
    Header,
    Rewrite,
    Http,
}

impl Kind {
    fn of(variable: &str) -> Kind {
        if variable.starts_with("proxy") {
            Kind::Proxy
        } else if variable.starts_with("ssl")
            || variable.starts_with("schannel")
            || variable == "pinnedpubkey"
        {
            Kind::Tls
        } else if variable == "extraheader" {
            Kind::Header
        } else {
            Kind::Http
        }
    }

    /// The sentence that refuses the call.
    pub(super) fn refusal(self) -> &'static str {
        match self {
            Kind::Proxy => concat!(
                "this repository's own git config sets a proxy for the network, which only your ",
                "own git config may set for the git purlis runs, so git was not run there. Move ",
                "that setting to your global git config, or run git yourself"
            ),
            Kind::Tls => concat!(
                "this repository's own git config sets a TLS or certificate setting, which only ",
                "your own git config may set for the git purlis runs, so git was not run there. ",
                "Move that setting to your global git config, or run git yourself"
            ),
            Kind::Header => concat!(
                "this repository's own git config sets an extra HTTP header, which only your own ",
                "git config may set for the git purlis runs, so git was not run there. Move that ",
                "setting to your global git config, or run git yourself"
            ),
            Kind::Rewrite => concat!(
                "this repository's own git config rewrites a network address to another host or ",
                "repository, which only your own git config may do for the git purlis runs, so ",
                "git was not run there. Move that setting to your global git config, or run git ",
                "yourself"
            ),
            Kind::Http => concat!(
                "this repository's own git config sets an HTTP setting, which only your own git ",
                "config may set for the git purlis runs, so git was not run there. Move that ",
                "setting to your global git config, or run git yourself"
            ),
        }
    }
}

/// Whether a scope is yours or purlis's: global, system, the command line, or what git cannot
/// place (a file named on the command line).
fn yours(scope: &str) -> bool {
    matches!(scope, "global" | "system" | "unknown" | "command")
}

/// One entry of `git config --list --show-scope -z`: `(scope, section, subsection, variable,
/// value)`, the section and the variable lowercased as git lists them.
struct Entry<'a> {
    scope: &'a str,
    section: &'a str,
    sub: Option<&'a str>,
    variable: &'a str,
    value: &'a str,
}

fn entries(listing: &str) -> Vec<Entry<'_>> {
    super::listed_entries(listing)
        .filter_map(|(scope, key, value)| {
            let (section, rest) = key.split_once('.')?;
            let (sub, variable) = match rest.rsplit_once('.') {
                Some((sub, variable)) => (Some(sub), variable),
                None => (None, rest),
            };
            Some(Entry {
                scope,
                section,
                sub,
                variable,
                value,
            })
        })
        .collect()
}

/// The `-c` settings that give the window's network git your own network settings where the
/// repository's config sets one, or the [`Kind`] of the first it sets that cannot be held.
/// `listing` is `git config --list --show-scope -z` as the call will read it; `env_proxy` is
/// whether the environment git is given names a proxy.
pub(super) fn held(listing: &str, env_proxy: bool) -> Result<Vec<String>, Kind> {
    let all = entries(listing);
    let mut settings: Vec<String> = Vec::new();
    for entry in all.iter().filter(|entry| !yours(entry.scope)) {
        match (entry.section, entry.sub, entry.variable) {
            ("url", Some(base), "insteadof" | "pushinsteadof") => {
                if !same_place(base, entry.value) {
                    return Err(Kind::Rewrite);
                }
            }
            ("remote", Some(name), variable @ ("proxy" | "proxyauthmethod")) => {
                let replace =
                    your_value(&all, "remote", Some(name), variable).ok_or(Kind::Proxy)?;
                let held = setting("remote", Some(name), variable, replace, Kind::Proxy)?;
                if !settings.contains(&held) {
                    settings.push(held);
                }
            }
            ("http", sub, variable) if !ITS_OWN.contains(&variable) => {
                let kind = Kind::of(variable);
                let replace = http_replacement(&all, sub, variable, env_proxy).ok_or(kind)?;
                let held = setting("http", sub, variable, replace, kind)?;
                if !settings.contains(&held) {
                    settings.push(held);
                }
            }
            _ => {}
        }
    }
    Ok(settings)
}

/// What a repository's `http.<sub>.<variable>` is given back as: the same key's value in your own
/// config, or git's default where your config says nothing about the variable at all. `None`
/// where neither gives the value you would have without the repository's key.
fn http_replacement<'a>(
    all: &'a [Entry<'a>],
    sub: Option<&str>,
    variable: &str,
    env_proxy: bool,
) -> Option<&'a str> {
    let your_any = all.iter().any(|entry| {
        yours(entry.scope)
            && entry.scope != "command"
            && entry.section == "http"
            && entry.variable == variable
    });
    if LISTS.contains(&variable) {
        // One more entry cannot give back a list of yours; only the empty list is exact.
        return if your_any {
            None
        } else {
            DEFAULTS
                .iter()
                .find(|(name, _)| *name == variable)
                .map(|(_, value)| *value)
        };
    }
    if let Some(value) = your_value(all, "http", sub, variable) {
        return Some(value);
    }
    if your_any && sub.is_some() {
        // Yours sets the variable for another address, which may be the one this key replaced
        // at a lower or a higher precision: no one value is exact.
        return None;
    }
    if variable == "proxy" {
        // An empty proxy also turns off the one your environment names.
        return (!env_proxy).then_some("");
    }
    DEFAULTS
        .iter()
        .find(|(name, _)| *name == variable)
        .map(|(_, value)| *value)
}

/// The last value your own config (global or system) gives exactly `section.sub.variable`.
fn your_value<'a>(
    all: &'a [Entry<'a>],
    section: &str,
    sub: Option<&str>,
    variable: &str,
) -> Option<&'a str> {
    all.iter()
        .rev()
        .find(|entry| {
            matches!(entry.scope, "global" | "system" | "unknown")
                && entry.section == section
                && entry.sub == sub
                && entry.variable == variable
        })
        .map(|entry| entry.value)
}

/// `section.sub.variable=value` for git's command line, or `kind` where the key or the value
/// cannot be carried there.
fn setting(
    section: &str,
    sub: Option<&str>,
    variable: &str,
    value: &str,
    kind: Kind,
) -> Result<String, Kind> {
    let key = match sub {
        Some(sub) => format!("{section}.{sub}.{variable}"),
        None => format!("{section}.{variable}"),
    };
    if key.contains(['=', '\n', '\0']) || value.contains(['\n', '\0']) {
        return Err(kind);
    }
    Ok(format!("{key}={value}"))
}

/// Whether a rewrite of `prefix` to `base` keeps the call at the host and the repository it
/// named, or takes it off the network: `base` is a `file://` repository on this machine, which
/// asks for no credential; or both name a whole host, the same one, and the same path after it.
/// Over HTTP(S) on both sides the port must match too. An SSH form rewritten to HTTPS on the same
/// host (what `purlis git-policy --apply` writes) is the same place.
fn same_place(base: &str, prefix: &str) -> bool {
    if base.to_ascii_lowercase().starts_with("file://") {
        return true;
    }
    match (host_of(base), host_of(prefix)) {
        (Some(base), Some(prefix)) => {
            let web = |host: &Host| matches!(host.scheme.as_str(), "http" | "https");
            let port = !(web(&base) && web(&prefix)) || base.port == prefix.port;
            base.name == prefix.name && base.path == prefix.path && port
        }
        _ => false,
    }
}

/// The host a URL prefix names whole.
struct Host {
    scheme: String,
    name: String,
    port: Option<String>,
    /// What follows the host, without its leading `/`.
    path: String,
}

/// The host of `scheme://[user@]host[:port]/…`, whose authority ends at a `/`, or of an SSH
/// `[user@]host:path` that names its path's start; `None` where the prefix stops before the host
/// is whole.
fn host_of(url: &str) -> Option<Host> {
    let url = url.to_ascii_lowercase();
    let (scheme, authority, path) = match url.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_once('/')?;
            (scheme.to_string(), authority.to_string(), path.to_string())
        }
        None => {
            let (authority, path) = url.split_once(':')?;
            if authority.contains('/') {
                return None;
            }
            (
                "ssh".to_string(),
                authority.to_string(),
                path.trim_start_matches('/').to_string(),
            )
        }
    };
    let host = authority
        .rsplit_once('@')
        .map_or(authority.as_str(), |(_, host)| host);
    let (name, port) = match host.rsplit_once(':') {
        Some((name, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            (name, Some(port.to_string()))
        }
        _ => (host, None),
    };
    if scheme.is_empty() || name.is_empty() {
        return None;
    }
    Some(Host {
        scheme,
        name: name.to_string(),
        port,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A listing as `git config --list --show-scope -z` writes it.
    fn listing(entries: &[(&str, &str, &str)]) -> String {
        entries
            .iter()
            .map(|(scope, key, value)| format!("{scope}\0{key}\n{value}\0"))
            .collect()
    }

    #[test]
    fn a_repos_tls_verification_is_given_back_as_yours() {
        let planted = listing(&[("local", "http.sslverify", "false")]);
        assert_eq!(
            held(&planted, false),
            Ok(vec!["http.sslverify=true".into()])
        );

        let yours_off = listing(&[
            ("global", "http.sslverify", "false"),
            ("local", "http.sslverify", "false"),
        ]);
        assert_eq!(
            held(&yours_off, false),
            Ok(vec!["http.sslverify=false".into()])
        );
    }

    #[test]
    fn a_repos_proxy_is_turned_off_unless_your_environment_names_one() {
        let planted = listing(&[("local", "http.proxy", "http://127.0.0.1:9")]);
        assert_eq!(held(&planted, false), Ok(vec!["http.proxy=".into()]));
        assert_eq!(held(&planted, true), Err(Kind::Proxy));

        let yours = listing(&[
            ("global", "http.proxy", "http://proxy.corp:3128"),
            ("local", "http.proxy", "http://127.0.0.1:9"),
        ]);
        assert_eq!(
            held(&yours, true),
            Ok(vec!["http.proxy=http://proxy.corp:3128".into()])
        );
    }

    #[test]
    fn a_repos_ca_bundle_is_refused_unless_yours_names_the_same_key() {
        let planted = listing(&[("local", "http.sslcainfo", "/tmp/planted.pem")]);
        assert_eq!(held(&planted, false), Err(Kind::Tls));

        let yours = listing(&[
            ("global", "http.sslcainfo", "/etc/corp.pem"),
            ("local", "http.sslcainfo", "/tmp/planted.pem"),
        ]);
        assert_eq!(
            held(&yours, false),
            Ok(vec!["http.sslcainfo=/etc/corp.pem".into()])
        );
    }

    #[test]
    fn a_repos_header_for_one_address_is_reset_at_that_address() {
        let planted = listing(&[(
            "local",
            "http.https://forge.example/.extraheader",
            "X-Planted: 1",
        )]);
        assert_eq!(
            held(&planted, false),
            Ok(vec!["http.https://forge.example/.extraheader=".into()])
        );
        // A list of yours cannot be given back by one more entry.
        let yours = listing(&[
            ("global", "http.extraheader", "X-Yours: 1"),
            (
                "local",
                "http.https://forge.example/.extraheader",
                "X-Planted: 1",
            ),
        ]);
        assert_eq!(held(&yours, false), Err(Kind::Header));
    }

    #[test]
    fn a_repos_key_for_one_address_is_refused_where_yours_sets_it_for_another() {
        let mixed = listing(&[
            ("global", "http.sslverify", "false"),
            ("worktree", "http.https://forge.example/.sslverify", "false"),
        ]);
        assert_eq!(held(&mixed, false), Err(Kind::Tls));
    }

    #[test]
    fn a_rewrite_to_another_place_is_refused_and_one_to_the_same_place_kept() {
        let elsewhere = listing(&[(
            "local",
            "url.https://elsewhere.example/.insteadof",
            "https://forge.example/",
        )]);
        assert_eq!(held(&elsewhere, false), Err(Kind::Rewrite));
        let push = listing(&[(
            "local",
            "url.https://elsewhere.example/.pushinsteadof",
            "https://forge.example/",
        )]);
        assert_eq!(held(&push, false), Err(Kind::Rewrite));
        // A prefix that does not name a whole host is another host as far as purlis can tell.
        for partial in ["https://forge", "gh:"] {
            let planted = listing(&[("local", "url.https://forge.example/.insteadof", partial)]);
            assert_eq!(held(&planted, false), Err(Kind::Rewrite), "{partial}");
        }
        // Another port on the same name is another server.
        let port = listing(&[(
            "local",
            "url.https://forge.example:8443/.insteadof",
            "https://forge.example/",
        )]);
        assert_eq!(held(&port, false), Err(Kind::Rewrite));
        // What `purlis git-policy --apply` writes: an SSH form to HTTPS on the same host.
        let policy = listing(&[
            (
                "local",
                "url.https://forge.example/.insteadof",
                "git@forge.example:",
            ),
            (
                "local",
                "url.https://forge.example/.insteadof",
                "ssh://git@forge.example/",
            ),
        ]);
        assert_eq!(held(&policy, false), Ok(vec![]));
        // A repository on this machine asks for no credential and crosses no network.
        let local = listing(&[(
            "local",
            "url.file:///srv/forge/acme/.insteadof",
            "https://forge.example/acme/",
        )]);
        assert_eq!(held(&local, false), Ok(vec![]));
        // Another repository on the same host is another place.
        let moved = listing(&[(
            "local",
            "url.https://forge.example/other/.insteadof",
            "https://forge.example/acme/",
        )]);
        assert_eq!(held(&moved, false), Err(Kind::Rewrite));
        let same = listing(&[(
            "local",
            "url.https://forge.example/acme/.insteadof",
            "https://Forge.example/acme/",
        )]);
        assert_eq!(held(&same, false), Ok(vec![]));
        // Your own rewrite is yours.
        let yours = listing(&[(
            "global",
            "url.git@forge.example:.insteadof",
            "https://forge.example/",
        )]);
        assert_eq!(held(&yours, false), Ok(vec![]));
    }

    #[test]
    fn a_remotes_own_proxy_is_refused_unless_yours_names_it() {
        let planted = listing(&[("local", "remote.origin.proxy", "http://127.0.0.1:9")]);
        assert_eq!(held(&planted, false), Err(Kind::Proxy));
    }

    #[test]
    fn pacing_keys_stay_the_repos_and_other_sections_are_not_read() {
        let pacing = listing(&[
            ("local", "http.postbuffer", "524288000"),
            ("local", "http.version", "HTTP/1.1"),
            ("local", "core.autocrlf", "input"),
        ]);
        assert_eq!(held(&pacing, false), Ok(vec![]));
    }

    #[test]
    fn a_key_or_value_a_command_line_cannot_carry_refuses() {
        let planted = listing(&[("local", "http.https://a=b/.sslverify", "false")]);
        assert_eq!(held(&planted, false), Err(Kind::Tls));
    }

    #[test]
    fn a_refusal_names_the_kind_of_setting_and_no_value() {
        for kind in [
            Kind::Proxy,
            Kind::Tls,
            Kind::Header,
            Kind::Rewrite,
            Kind::Http,
        ] {
            let said = kind.refusal();
            assert!(
                said.starts_with("this repository's own git config"),
                "{said}"
            );
            assert!(!said.contains("127.0.0.1"), "{said}");
        }
    }
}
