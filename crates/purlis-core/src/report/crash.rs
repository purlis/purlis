// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Crash report: fixed fields, as a Sentry envelope** (#5689, spec #5683, Q5).
//!
//! A panic is sent with where it panicked, its message, purlis's version and channel, and the
//! operating system and its version: nothing else. No thread, no backtrace, no host name, no
//! user. The two free-text fields are scrubbed harder than a draft is, because nobody reads a
//! Crash report before it leaves: every absolute path goes whole, as `[path]`, before
//! [`Known::scrub`] takes out what it can identify (environment values, and the names of the
//! person's projects, workspaces, repos, personas and vaults).
//!
//! **A native crash** (#5691) is sent with the same fixed fields, no message and no place (the
//! process could say neither), and its minidump as the envelope's attachment, which Sentry reads
//! for the crashing thread's stack. Ruling O9 sends the minidump to the Report intake as it was
//! written; nothing else ever sees it. The sent list names it by its size and its SHA-256 in
//! place of its bytes: they are not text, and a list kept 90 days holds no memory of the app's.

use regex::Regex;
use serde_json::json;

use sha2::{Digest, Sha256};

use super::{Known, NativeCrash, Panic, short_place};

/// A Crash report, ready to send.
pub(super) struct CrashReport {
    pub event_id: String,
    /// What the sent list calls it.
    pub title: String,
    /// The envelope, exactly as it is sent.
    pub envelope: Vec<u8>,
    /// What the sent list shows: the envelope, with a minidump named in place of its bytes.
    pub shown: String,
}

impl CrashReport {
    /// The Crash report of `record`, scrubbed by what `known` names, for the intake at `dsn`.
    pub fn of_panic(record: &Panic, known: &Known, dsn: &str) -> CrashReport {
        let (place, _) = known.scrub(&without_paths(&short_place(&record.place)));
        let (message, _) = known.scrub(&without_paths(&record.message));
        let event_id = uuid::Uuid::new_v4().simple().to_string();
        let mut event = fixed_fields(&event_id, record.at);
        event["exception"] = json!({"values": [{
            "type": "panic",
            "value": message,
            "mechanism": {"type": "panic", "handled": false},
        }]});
        event["culprit"] = json!(place);
        let text = format!("{}{}", header(&event_id, dsn), event_item(&event));
        CrashReport {
            title: format!("purlis panicked at {place}"),
            envelope: text.clone().into_bytes(),
            shown: text,
            event_id,
        }
    }

    /// The Crash report of a native crash, with its minidump, for the intake at `dsn`.
    pub fn of_native_crash(crash: &NativeCrash, dsn: &str) -> CrashReport {
        let event_id = uuid::Uuid::new_v4().simple().to_string();
        let event = fixed_fields(&event_id, crash.at);
        let text = format!("{}{}", header(&event_id, dsn), event_item(&event));
        let attachment = json!({
            "type": "attachment",
            "length": crash.minidump.len(),
            "filename": "purlis.dmp",
            "attachment_type": "event.minidump",
        });
        let mut envelope = format!("{text}{attachment}\n").into_bytes();
        envelope.extend_from_slice(&crash.minidump);
        envelope.push(b'\n');
        let digest = Sha256::digest(&crash.minidump);
        let digest: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        CrashReport {
            title: "purlis crashed".to_string(),
            envelope,
            shown: format!(
                "{text}{attachment}\n[a minidump of {} bytes, sha256 {digest}]\n",
                crash.minidump.len()
            ),
            event_id,
        }
    }
}

/// The fields every Crash report carries, and nothing else: when, which purlis on which channel,
/// and which operating system. `at` is in seconds since 1970; none is now.
fn fixed_fields(event_id: &str, at: Option<u64>) -> serde_json::Value {
    let timestamp = at
        .and_then(|at| chrono::DateTime::from_timestamp(i64::try_from(at).ok()?, 0))
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let channel = channel();
    json!({
        "event_id": event_id,
        "timestamp": timestamp,
        "platform": "native",
        "level": "fatal",
        "release": format!("purlis@{}", crate::adopt::release_version()),
        "environment": channel,
        "tags": {"channel": channel},
        "contexts": {"os": {"name": std::env::consts::OS, "version": os_version()}},
    })
}

/// The envelope's header line.
fn header(event_id: &str, dsn: &str) -> String {
    let header = json!({
        "event_id": event_id,
        "sent_at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "dsn": dsn,
    });
    format!("{header}\n")
}

/// The event's item: its header line and the event.
fn event_item(event: &serde_json::Value) -> String {
    let event = event.to_string();
    let item = json!({"type": "event", "length": event.len(), "content_type": "application/json"});
    format!("{item}\n{event}\n")
}

/// The channel this build is for: `stable` for a release build, `dev` for a dev channel build,
/// `local` for one built by hand (ADR 0092's profiles).
fn channel() -> &'static str {
    match crate::adopt::build_profile() {
        "release" => "stable",
        "dev-release" => "dev",
        _ => "local",
    }
}

/// The kernel's release, as [`crate::adopt::system`] names it, or nothing where it names none.
fn os_version() -> Option<String> {
    let system = crate::adopt::system();
    let named = system.split(", ").next()?;
    named
        .split_once(' ')
        .map(|(_, version)| version.to_string())
}

/// `text` with every absolute path, home-relative path and Windows path replaced, whole, by
/// `[path]`: a path names the person's folders, and a Crash report is read by nobody first.
fn without_paths(text: &str) -> String {
    static ONCE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let paths = ONCE.get_or_init(|| {
        Regex::new(r#"(^|[\s'"`(=:,\[<])(?:/|~/|[A-Za-z]:\\)[^\s'"`()<>\[\],]*"#)
            .expect("a pattern this module wrote")
    });
    paths.replace_all(text, "${1}[path]").into_owned()
}

#[cfg(test)]
mod tests {
    use super::without_paths;

    #[test]
    fn every_absolute_path_goes_whole_and_a_relative_one_stays() {
        assert_eq!(
            without_paths("open /a/b c: ~/x/y, \"C:\\Users\\me\\f\" (/tmp/z) src/plane.rs:3"),
            "open [path] c: [path], \"[path]\" ([path]) src/plane.rs:3"
        );
    }
}
