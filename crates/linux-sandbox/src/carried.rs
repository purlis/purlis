// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The environment the wrap hands its command, carried past the wrap itself** (#2460).
//!
//! The wrap is purlis's own program, started outside any sandbox, so what it starts with is
//! fixed by purlis and never the chat's: the variables a dynamic loader or a library reads are
//! not taken out by name, since a list of names can miss one. The command's environment is
//! carried instead, each variable under a name of the wrap's own ([`EACH`] and its place, with
//! [`COUNT`] saying how many), and read back whole by the wrap ([`read`]), which gives the
//! command exactly that and nothing of its own. Each variable stays one variable, so whatever
//! fitted in the command's environment fits in the wrap's.
//!
//! A wrap whose own environment is anything else (a variable not carried, a piece missing)
//! reads none and starts nothing. The one exception is [`SET_BY_THE_TERMINAL`], which the
//! terminal a chat runs in sets on whatever it starts; it is let be and not handed on, as the
//! command's own is carried with the rest.

use std::ffi::{OsStr, OsString};

/// The variable of the wrap's own that says how many it carries.
pub const COUNT: &str = "PURLIS_SANDBOX_ENV";

/// The start of each carried variable's name in the wrap's own environment; its place follows.
pub const EACH: &str = "PURLIS_SANDBOX_ENV_";

/// The one variable a wrap may start with beside what it carries: the terminal's type, which
/// the terminal sets on every program it starts.
pub const SET_BY_THE_TERMINAL: &str = "TERM";

/// Why a wrap reads no environment from its own: purlis did not build it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotBuilt;

impl std::fmt::Display for NotBuilt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the sandbox was started with an environment purlis did not build")
    }
}

/// **The wrap's whole environment, carrying `env`** for the command it starts: start the wrap
/// with exactly this, from an empty environment.
pub fn carried<K: AsRef<OsStr>, V: AsRef<OsStr>>(
    env: impl IntoIterator<Item = (K, V)>,
) -> Vec<(OsString, OsString)> {
    let mut out = vec![(OsString::from(COUNT), OsString::new())];
    for (at, (name, value)) in env.into_iter().enumerate() {
        let mut each = name.as_ref().to_owned();
        each.push("=");
        each.push(value);
        out.push((format!("{EACH}{at}").into(), each));
    }
    out[0].1 = (out.len() - 1).to_string().into();
    out
}

/// **What [`carried`] carried, read back from the wrap's own environment `own`**, in its
/// order; or [`NotBuilt`] where `own` is not what [`carried`] makes.
#[cfg(unix)]
pub fn read(
    own: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<Vec<(OsString, OsString)>, NotBuilt> {
    use std::os::unix::ffi::OsStringExt;
    let mut count = None;
    let mut each = std::collections::BTreeMap::new();
    for (name, value) in own {
        let name = name.to_str().ok_or(NotBuilt)?;
        if name == COUNT {
            let n = value
                .to_str()
                .and_then(|n| n.parse::<usize>().ok())
                .ok_or(NotBuilt)?;
            if count.replace(n).is_some() {
                return Err(NotBuilt);
            }
        } else if let Some(at) = name.strip_prefix(EACH) {
            let n = at.parse::<usize>().map_err(|_| NotBuilt)?;
            if n.to_string() != at || each.insert(n, value).is_some() {
                return Err(NotBuilt);
            }
        } else if name != SET_BY_THE_TERMINAL {
            return Err(NotBuilt);
        }
    }
    // Every place below the count, each once, and none above it.
    let count = count.ok_or(NotBuilt)?;
    if each.len() != count || each.keys().next_back().is_some_and(|last| *last >= count) {
        return Err(NotBuilt);
    }
    each.into_values()
        .map(|pair| {
            let mut name = pair.into_vec();
            let at = name
                .iter()
                .position(|byte| *byte == b'=')
                .filter(|at| *at > 0)
                .ok_or(NotBuilt)?;
            let value = name.split_off(at + 1);
            name.pop();
            Ok((OsString::from_vec(name), OsString::from_vec(value)))
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests;
