// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The browsers open when purlis last quit** (#2022, ruled 2026-10-18):
//! `app/browsers-at-quit.json` in the project's state folder.
//!
//! Quit ends every browser. Before it does, the app keeps where each of the project's chats'
//! browsers was ([`keep`]), and the next start offers them once ([`take`]): each address as a
//! button, never loaded by itself.
//!
//! - **Addresses only**: `{"addresses": ["https://…"]}`, each cut to its scheme, host, port and
//!   path ([`cut`]): no user, password, query or fragment, where a sign-in or a reset link puts
//!   its token. No cookie, login, title or page content is kept, and only `http` and `https`
//!   addresses, each once, at most [`MOST`]. The app keeps none of a browser that held a login
//!   or text the person typed.
//! - **This machine's, never committed**: under `app/`, beside the project's other records of
//!   this machine, which no sandboxed chat writes.
//! - **Offered once**: [`take`] reads it and removes it, so a second start offers nothing. A file
//!   that does not read, or a link in its place, offers nothing and is taken away; a link is
//!   never followed.

use std::io;
use std::path::{Path, PathBuf};

/// The file, relative to the project's state folder ([`path`]).
pub const IN_STATE: &str = "app/browsers-at-quit.json";

/// The most addresses kept.
pub const MOST: usize = 32;

/// The longest address kept, in bytes: a longer one is not kept.
const LONGEST: usize = 2048;

/// The file in the project at `root`.
pub fn path(root: &Path) -> PathBuf {
    crate::names::state(root).join(IN_STATE)
}

/// Whether `address` is one kept: `http` or `https`, and not too long.
fn kept(address: &str) -> bool {
    address.len() <= LONGEST
        && ["http://", "https://"].iter().any(|scheme| {
            address
                .get(..scheme.len())
                .is_some_and(|s| s.eq_ignore_ascii_case(scheme))
        })
}

/// **`address` cut to its scheme, host, port and path**: none of the user, password, query or
/// fragment. `None` for what is no `http` or `https` address.
pub fn cut(address: &str) -> Option<String> {
    if !kept(address) {
        return None;
    }
    let (scheme, rest) = address.split_once("://")?;
    // Where the authority ends: the path, the query or the fragment, whichever comes first.
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, after) = rest.split_at(end);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if host.is_empty() {
        return None;
    }
    let path_end = after.find(['?', '#']).unwrap_or(after.len());
    let path = &after[..path_end];
    let path = if path.is_empty() { "/" } else { path };
    Some(format!("{}://{host}{path}", scheme.to_ascii_lowercase()))
}

/// **Keeps `addresses`** for the next start, in their order, each once and each [`cut`]: the web
/// ones alone. With none to keep, nothing is written and any file left is taken away.
pub fn keep(root: &Path, addresses: &[String]) -> io::Result<()> {
    let mut kept_now: Vec<String> = Vec::new();
    for address in addresses.iter().filter_map(|one| cut(one)) {
        if !kept_now.contains(&address) && kept_now.len() < MOST {
            kept_now.push(address);
        }
    }
    let path = path(root);
    if kept_now.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(none) if none.kind() == io::ErrorKind::NotFound => Ok(()),
            done => done,
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&serde_json::json!({ "addresses": kept_now }))
        .map_err(io::Error::other)?;
    crate::rewrite::replace(
        root,
        &path,
        format!("{text}\n").as_bytes(),
        crate::rewrite::Mode::Private,
    )
}

/// **The addresses kept at the last quit**, taken away as they are read: offered once.
pub fn take(root: &Path) -> Vec<String> {
    let path = path(root);
    let read = match std::fs::symlink_metadata(&path) {
        Err(_) => return Vec::new(),
        // Never through a link, and never a folder or a device.
        Ok(there) if there.is_file() => std::fs::read_to_string(&path).ok(),
        Ok(_) => None,
    };
    if let Err(why) = std::fs::remove_file(&path) {
        tracing::warn!("purlis: the browsers kept at the last quit stay on disk: {why}");
    }
    let Some(doc) = read.and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    else {
        return Vec::new();
    };
    let mut taken: Vec<String> = Vec::new();
    for address in doc
        .get("addresses")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
    {
        if let Some(address) = cut(address)
            && !taken.contains(&address)
            && taken.len() < MOST
        {
            taken.push(address);
        }
    }
    taken
}

#[cfg(test)]
mod tests;
