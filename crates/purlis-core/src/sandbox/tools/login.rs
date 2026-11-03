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
//! `user` the user name's, which a page that asks for a password alone leaves out.
//!
//! **A site that signs in on another host** (#4995, the person's ruling of 2026-11-03) also
//! names `form`, the origin its login form is on: `browser = "https://app.example.com/"`,
//! `form = "https://auth.example.com"`. The browser opens the page, and the password is typed on
//! the form's origin alone. Without `form` it is typed on the page's own origin, as before.
//!
//! **purlis performs it, in the chat's own browser** (#2016, spec #2008). The chat calls the
//! `browser_login` tool with the login's name; the app checks that the chat's start armed it
//! (declared, and allowed on this machine with the persona's hosts, D-PE-ruling-1), reads its
//! values from the vault itself, and hands them to the session host, whose browser goes to the
//! declared page, fills them only while that page is on the declared origin, submits, and hands
//! the browser back. The chat gets the logged-in browser, never the login, and no file of the
//! session is written anywhere it reads. Run as a command, the tool runs nothing and says so.
//!
//! **What the chat chooses: which declared login, and nothing else.** Not the page, the host or
//! the values: those are the committed tool's and the vault's.

/// The key that makes a tool a browser login: the login page's URL.
pub const KEY: &str = "browser";

/// The key of the origin a browser login's form is on, where it isn't the page's own (#4995).
pub const FORM_KEY: &str = "form";

/// The key of a browser login's user name, beside `key`, the password's.
pub const USER_KEY: &str = "user";

/// The variable a browser login's password file is handed in (`secret exec --file`).
pub const PASS_VAR: &str = "BROWSER_LOGIN_PASS";

/// The variable a browser login's user-name file is handed in (`secret exec --file`).
pub const USER_VAR: &str = "BROWSER_LOGIN_USER";

/// **A browser login**: the page it opens and the origin it fills only on. Its keys are the
/// tool's values ([`super::Tool::values`]): the password's file in [`PASS_VAR`], and the user
/// name's in [`USER_VAR`] where the page asks for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    pub url: String,
    pub origin: String,
    /// **The origin its form is on, where that isn't [`Self::origin`]** (#4995): the one place
    /// the password is typed. None types it on the page's own origin.
    pub form: Option<String>,
}

impl Login {
    /// **The one origin the password is typed on**: the form's, else the page's own.
    pub fn fills_on(&self) -> &str {
        self.form.as_deref().unwrap_or(&self.origin)
    }
}

/// **A login's `form`** (#4995): an origin as [`origin_of`] reads one, written as an origin alone,
/// with no page, query or fragment, or why it is not one.
pub fn form_of(form: &str) -> Result<String, String> {
    let origin = origin_of(form).map_err(|why| why.replace("browser = ", "form = "))?;
    if form.trim_end_matches('/') != origin {
        return Err(format!(
            "form = {form:?} is not an origin alone: write the scheme and host of the login form, \
             as {origin:?}"
        ));
    }
    Ok(origin)
}

/// **The origin a login page's `url` is on** (`https://host[:port]`, the port left out where it
/// is the scheme's own, and a name in its ASCII spelling, as a browser's `URL.origin` spells
/// it), or why it may not be one: not
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
    // A name written in another script is the one a browser goes to by its ASCII spelling
    // (IDNA), which is the spelling its page's address carries.
    let ascii;
    let host = if host.is_ascii() {
        host
    } else {
        ascii = idna::domain_to_ascii(host).map_err(|_| {
            format!("browser = {url:?} names a host purlis cannot spell as a browser does")
        })?;
        ascii.as_str()
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
