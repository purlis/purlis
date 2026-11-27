// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Git LFS, download only (ADR 0128 D3f, D3i).
//!
//! The batch call's answer names **storage addresses on other hosts, with their own credentials**
//! (a `header` map of authorization for the object). The chat gets neither:
//!
//! - each `href` is checked against a closed list (the chat's recorded forge hosts, the
//!   download-only hosts and the forge's storage host), and one that is not on it ends the answer,
//!   with a sentence that says whose files they are;
//! - the address and its headers are **kept here**, in memory, under a random token, and the chat
//!   is given an address on the host it already reached (`/__purlis-lfs/<token>`), which this
//!   listener serves by fetching the kept address with the kept headers, once per request and
//!   with no credential of the person's;
//! - an action that is not a download (`upload`, `verify`) is never passed on.
//!
//! A chat that names a storage address itself, or a token that was never given, gets a refusal:
//! the listener dials a storage address only for one the forge handed it.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;
use url::Url;

use super::super::is_download_host;

/// Where a chat asks for an object the listener holds the address of.
pub(crate) const PATH_PREFIX: &str = "/__purlis-lfs/";

/// Hosts besides a chat's recorded ones that an LFS batch may name: GitHub's object storage.
const STORAGE_HOSTS: &[&str] = &["github-cloud.githubusercontent.com"];

/// How long an address is held.
const HOLD: Duration = Duration::from_secs(15 * 60);

/// The most addresses held at once, for all of a chat's batches.
const MOST: usize = 20_000;

/// The sentence a window shows when a batch names a host that is not on the list.
pub(crate) const UNKNOWN_HOST: &str = "This repo's LFS files live on a host purlis does not know.";

struct Entry {
    url: String,
    headers: Vec<(String, String)>,
    /// The host the chat was given the address on: only a request to it may ask for the object.
    chat_host: String,
    expires: Instant,
}

/// The storage addresses a chat's batches handed the listener.
#[derive(Default)]
pub struct LfsStorage {
    held: Mutex<HashMap<String, Entry>>,
}

impl LfsStorage {
    fn hold(&self, url: &Url, headers: Vec<(String, String)>, chat_host: &str) -> Option<String> {
        let mut raw = [0u8; 16];
        getrandom::fill(&mut raw).ok()?;
        let token = data_encoding::HEXLOWER.encode(&raw);
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        held.retain(|_, entry| entry.expires > now);
        if held.len() >= MOST {
            return None;
        }
        held.insert(
            token.clone(),
            Entry {
                url: url.to_string(),
                headers,
                chat_host: chat_host.to_string(),
                expires: now + HOLD,
            },
        );
        Some(token)
    }

    /// The address and headers held under `token`, for a request that came in on `chat_host`.
    pub(crate) fn lookup(
        &self,
        token: &str,
        chat_host: &str,
    ) -> Option<(String, Vec<(String, String)>)> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = held.get(token)?;
        (entry.expires > Instant::now() && entry.chat_host == chat_host)
            .then(|| (entry.url.clone(), entry.headers.clone()))
    }
}

/// Whether the batch's answer may name `host`.
fn on_the_list(host: &str, recorded: &[String]) -> bool {
    recorded.iter().any(|h| h == host) || is_download_host(host) || STORAGE_HOSTS.contains(&host)
}

/// Write a batch's answer again for the chat: every `href` becomes an address on `chat_host` that
/// this listener serves, and the headers that came with it are kept, never passed on. `Err` is an
/// answer that names a host off the list, or an action that is not a download, or one this cannot
/// read: it is not passed on.
pub(crate) fn rewrite_batch(
    body: &[u8],
    chat_host: &str,
    recorded: &[String],
    storage: &LfsStorage,
) -> Result<Vec<u8>, &'static str> {
    let mut answer: Value = serde_json::from_slice(body).map_err(|_| UNKNOWN_HOST)?;
    let objects = answer
        .get_mut("objects")
        .and_then(Value::as_array_mut)
        .ok_or(UNKNOWN_HOST)?;
    for object in objects {
        let Some(actions) = object.get_mut("actions").and_then(Value::as_object_mut) else {
            continue;
        };
        if actions.keys().any(|name| name != "download") {
            return Err(UNKNOWN_HOST);
        }
        let Some(download) = actions.get_mut("download").and_then(Value::as_object_mut) else {
            return Err(UNKNOWN_HOST);
        };
        let href = download
            .get("href")
            .and_then(Value::as_str)
            .ok_or(UNKNOWN_HOST)?;
        let url = Url::parse(href).map_err(|_| UNKNOWN_HOST)?;
        let host = url
            .host_str()
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let plain = url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none_or(|p| p == 443)
            && url.fragment().is_none();
        if !plain || !on_the_list(&host, recorded) {
            return Err(UNKNOWN_HOST);
        }
        let headers: Vec<(String, String)> = download
            .get("header")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        // Header names a client sends on its own are not the storage's to ask for.
        if headers
            .iter()
            .any(|(n, v)| n.contains(['\r', '\n', ':']) || v.contains(['\r', '\n']))
        {
            return Err(UNKNOWN_HOST);
        }
        let token = storage.hold(&url, headers, chat_host).ok_or(UNKNOWN_HOST)?;
        download.remove("header");
        download.insert(
            "href".to_string(),
            Value::String(format!("https://{chat_host}{PATH_PREFIX}{token}")),
        );
    }
    serde_json::to_vec(&answer).map_err(|_| UNKNOWN_HOST)
}
