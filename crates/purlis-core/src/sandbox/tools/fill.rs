// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A secret-fill line** (#5116, spec #5100, the person's rulings SF-4 and SF-6 on #5096): one
//! vault key allowed into fields on one exact origin, written by the secret fill notice's Always
//! allow beside a persona's browser logins, or by hand.
//!
//! ```toml
//! [sandbox.personas.qa]
//! tools = { dash-api-key = { vault = "qa", key = "dash-api-key",
//!                            fill = "https://dash.example.test" } }
//! ```
//!
//! `fill` is the origin alone (scheme, host and port, `https://` only, plain `http://` for
//! `localhost`, `127.0.0.1` and `[::1]` alone), and makes the line this kind. Once the person on
//! this machine allowed the persona's hosts and tools with it, a chat of that persona fills that
//! key into a one-line field whose own frame is on that origin without a notice. A multi-line
//! field or an editable area asks each time all the same (SF-5, #5120).
//!
//! **`card = true` marks the key as a card or payment value** (SF-4): every fill of that vault's
//! key, on any origin, is offered Allow once alone, and the line allows nothing without a notice.
//!
//! **It runs no program.** It is never armed as a chat's tool: no shim, no `secret exec`, and it
//! holds no value a run could be handed. Its line is part of what the persona's Allow covers.

/// The key that makes a tool a secret-fill line: the origin its key may be filled on.
pub const KEY: &str = "fill";

/// The key that marks a secret-fill line's key as a card or payment value (SF-4).
pub const CARD_KEY: &str = "card";

/// **A secret-fill line**: the key, the one origin it may be filled on, and whether the key is a
/// card or payment value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fill {
    pub key: String,
    pub origin: String,
    /// A card or payment value: Allow once only, on any origin (SF-4).
    pub card: bool,
}

/// **A line's `fill`**: an origin as a login page's is read, written as an origin alone, or why
/// it is not one.
pub fn origin_of(fill: &str) -> Result<String, String> {
    let origin = super::login::origin_of(fill).map_err(|why| {
        why.replace("browser = ", "fill = ")
            .replace("of the login page", "of the site")
    })?;
    if fill.trim_end_matches('/') != origin {
        return Err(format!(
            "fill = {fill:?} is not an origin alone: write the scheme and host of the site the key \
             goes to, as {origin:?}"
        ));
    }
    Ok(origin)
}

/// **A secret-fill line's table**, or why the tool is not one: `vault`, `key` and `fill`, and
/// `card = true` where the key is a card's.
pub(super) fn of(
    tool: &toml::map::Map<String, toml::Value>,
    key: String,
    text: &dyn Fn(&str) -> Option<String>,
) -> Result<Fill, String> {
    if let Some(other) = tool
        .keys()
        .find(|one| !["vault", "key", KEY, CARD_KEY].contains(&one.as_str()))
    {
        return Err(format!(
            "{other} is not a key of a secret-fill line — it holds vault, key, fill and, for a \
             card, card = true"
        ));
    }
    let Some(fill) = text(KEY) else {
        return Err("fill names no origin — fill = \"https://…\"".to_owned());
    };
    let origin = origin_of(&fill)?;
    let card = match tool.get(CARD_KEY) {
        None => false,
        Some(toml::Value::Boolean(card)) => *card,
        Some(_) => return Err("card is true or false".to_owned()),
    };
    Ok(Fill { key, origin, card })
}

/// Names other settings read as a program's tool by name alone (a forge's command line), which
/// a line Always writes never takes.
const READ_AS_A_PROGRAM: [&str; 3] = ["gh", "glab", "git"];

/// **The name Always gives a line of `key`**, where `taken` holds the persona's tools' names: the
/// key's own, made a tool's name, then with `-2`, `-3` and on until one is free, never a name
/// read as a program's ([`READ_AS_A_PROGRAM`]). None where the key makes no name.
pub fn name_for(key: &str, taken: &dyn Fn(&str) -> bool) -> Option<String> {
    let plain: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let plain = plain.trim_start_matches(['.', '-']).to_owned();
    let base: String = plain.chars().take(48).collect();
    if base.is_empty() {
        return None;
    }
    (1..100)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|name| {
            super::refused_name(name).is_none()
                && !taken(name)
                && !READ_AS_A_PROGRAM.contains(&name.as_str())
        })
}
