// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser-login tool** (the operator's approval of 2026-10-12, D-PE-browser-1): a persona's
//! tool that logs a browser in to one declared page with a login from a vault, so a sandboxed
//! chat, which runs only declared tools (D-PE-ruling-2), can still reach a site behind a login.
//!
//! ```toml
//! [sandbox.personas.qa]
//! tools = { shop-login = { vault = "qa", browser = "https://shop.example.test/login",
//!                          user = "username", key = "password" } }
//! ```
//!
//! `browser` is the login page, and makes the tool this kind; `key` is the password's key and
//! `user` the user name's, which a page that asks for a password alone leaves out. A chat runs
//! `shop-login` (or `shop-login -s=<session>`), and purlis does the rest itself, with no shell
//! between: the app runs `purlis browser _login` ([`words`]) by its absolute path, which runs
//! Playwright's `playwright-cli` ([`PROGRAM`]), found as any tool's program is, where no chat
//! writes. It opens the declared page in a browser of its own and fills the login only while
//! that page is on the declared origin ([`crate::browser::login`]). The chat gets the logged-in
//! session's cookies to load into its own browser, never the login.
//!
//! **What the chat chooses: the session's name, and nothing else** ([`session_of`]). Not the
//! page, the host, the program or its arguments: those are the committed tool's, allowed on
//! this machine with the persona's hosts (D-PE-ruling-1).

use std::path::Path;

/// The key that makes a tool a browser login: the login page's URL.
pub const KEY: &str = "browser";

/// The key of a browser login's user name, beside `key`, the password's.
pub const USER_KEY: &str = "user";

/// The program a browser login runs, found where no chat writes: Playwright's own command line
/// (`npm install -g @playwright/cli@<version>`).
pub const PROGRAM: &str = "playwright-cli";

/// The variable a browser login's password file is handed in (`secret exec --file`).
pub const PASS_VAR: &str = "BROWSER_LOGIN_PASS";

/// The variable a browser login's user-name file is handed in (`secret exec --file`).
pub const USER_VAR: &str = "BROWSER_LOGIN_USER";

/// The purlis words that run a login: `purlis browser _login`.
pub const SUBCOMMAND: [&str; 2] = ["browser", "_login"];

/// The session a login is loaded for when the chat names none: `playwright-cli`'s own default.
pub const DEFAULT_SESSION: &str = "default";

/// The longest session name taken.
const MOST_SESSION: usize = 64;

/// **A browser login**: the page it opens, the origin it fills only on, and the key of the user
/// name, where the page asks for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    pub url: String,
    pub origin: String,
    pub user: Option<String>,
}

/// **The origin a login page's `url` is on** (`https://host[:port]`, the port left out where it
/// is the scheme's own, as a browser's `URL.origin` spells it), or why it may not be one: not
/// `https` (plain `http` is taken for this machine's own loopback alone), a user or password in
/// it, or a host that is not a plain lower-case name or address.
pub fn origin_of(url: &str) -> Result<String, String> {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        ("http", rest)
    } else {
        return Err(format!(
            "browser = {url:?} is not an https:// address of the login page"
        ));
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    if authority.contains('@') {
        return Err(format!(
            "browser = {url:?} carries a user or password; the login comes from the vault"
        ));
    }
    if path
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return Err(format!(
            "browser = {url:?} has a space, a backslash or a control character in it"
        ));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port))
            if !host.ends_with(':') && (!host.starts_with('[') || host.ends_with(']')) =>
        {
            (host, Some(port))
        }
        _ => (authority, None),
    };
    let named = host
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
        && !host.is_empty()
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && !host.contains("..");
    let v6 = host.len() > 2
        && host.starts_with('[')
        && host.ends_with(']')
        && host[1..host.len() - 1]
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f' | b':' | b'.'));
    if !(named || v6) {
        return Err(format!(
            "browser = {url:?} names no host purlis can bind the login to: a lower-case name or \
             address, with no other characters"
        ));
    }
    let port = match port {
        None => None,
        Some(port) => match port.parse::<u16>() {
            Ok(n) if n > 0 && !port.starts_with('0') => Some(n),
            _ => return Err(format!("browser = {url:?} has no port purlis can read")),
        },
    };
    let loopback = matches!(host, "localhost" | "127.0.0.1" | "[::1]");
    if scheme == "http" && !loopback {
        return Err(format!(
            "browser = {url:?} is plain http, which would send the login unencrypted; write \
             https:// (http:// is taken for this machine's own localhost alone)"
        ));
    }
    let default = if scheme == "https" { 443 } else { 80 };
    Ok(match port.filter(|port| *port != default) {
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    })
}

/// **The session a chat's arguments to a login name**: none, `-s=<name>`, `-s <name>` or
/// `--session=<name>`, a name of letters, digits, `.`, `_` and `-`. Anything else is refused:
/// the page, the host and the program are the declared tool's, never the chat's.
pub fn session_of(args: &[String], tool: &str, url: &str) -> Result<String, String> {
    let named = match args {
        [] => Some(DEFAULT_SESSION),
        [one] => one
            .strip_prefix("-s=")
            .or_else(|| one.strip_prefix("--session=")),
        [flag, name] if flag == "-s" || flag == "--session" => Some(name.as_str()),
        _ => None,
    };
    let ok = named.filter(|name| {
        !name.is_empty()
            && name.len() <= MOST_SESSION
            && name.starts_with(|c: char| c.is_ascii_alphanumeric())
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    });
    ok.map(str::to_owned).ok_or_else(|| {
        format!(
            "{tool} logs in to {url}, the page its declaration names, and takes only the browser \
             session to log in: `{tool}` or `{tool} -s=<session>`, a name of letters, digits, \
             '.', '_' and '-'. No page, host or program is the chat's to choose, so nothing ran"
        )
    })
}

/// The hand-overs a login asks `secret exec` for, in order: the password's file, then the user
/// name's, where it has one.
pub fn hand_over(key: &str, login: &Login) -> Vec<String> {
    std::iter::once(format!("{PASS_VAR}={key}"))
        .chain(login.user.iter().map(|user| format!("{USER_VAR}={user}")))
        .collect()
}

/// **The run of a login**, after the vault's files are handed: `purlis browser _login`, by
/// `purlis`'s absolute path, with Playwright's program `cli` (found where no chat writes), the
/// declared page and the chat's session.
pub fn words(purlis: &Path, cli: &Path, login: &Login, session: &str) -> Vec<String> {
    let mut words = vec![purlis.display().to_string()];
    words.extend(SUBCOMMAND.iter().map(|word| (*word).to_owned()));
    words.extend([
        "--cli".to_owned(),
        cli.display().to_string(),
        "--url".to_owned(),
        login.url.clone(),
        "--session".to_owned(),
        session.to_owned(),
    ]);
    words
}
