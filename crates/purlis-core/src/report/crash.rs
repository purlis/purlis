// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Crash report: fixed fields, as a Sentry envelope** (#5689, #5690, spec #5683, Q5).
//!
//! A panic is sent with where it panicked, its message, purlis's version and channel, and the
//! operating system and its version: nothing else. No thread, no backtrace, no host name, no
//! user. The two free-text fields are scrubbed harder than a draft is, because nobody reads a
//! Crash report before it leaves: every absolute path goes whole, as `[path]`, before
//! [`Known::scrub`] takes out what it can identify (environment values, and the names of the
//! person's projects, workspaces, repos, personas and vaults). An error the window did not catch
//! is sent the same way, with its name for the panic's type and its script's file name for the
//! place (#5690).

use regex::Regex;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{Draft, Known, Panic, WindowError, short_place};

/// A Crash report, ready to send.
pub(super) struct CrashReport {
    pub event_id: String,
    /// What the sent list calls it.
    pub title: String,
    /// The envelope, exactly as it is sent.
    pub bytes: String,
    /// Where it crashed in which purlis version (#5693, Q15): the same crash again in the same
    /// version has the same one, and is counted instead of sent. A draft the person filed has
    /// none.
    pub fingerprint: Option<String>,
}

impl CrashReport {
    /// The Crash report of `record`, scrubbed by what `known` names, for the intake at `dsn`.
    pub fn of_panic(record: &Panic, known: &Known, dsn: &str) -> CrashReport {
        let (place, _) = known.scrub(&without_paths(&short_place(&record.place)));
        let (message, _) = known.scrub(&without_paths(&record.message));
        let version = record
            .version
            .as_deref()
            .unwrap_or(crate::adopt::release_version());
        let at = record
            .at
            .and_then(|at| chrono::DateTime::from_timestamp(i64::try_from(at).ok()?, 0));
        Fields {
            platform: "native",
            kind: "panic".to_string(),
            mechanism: "panic",
            fingerprint: fingerprint(&["panic", version, &place]),
            title: format!("purlis panicked at {place}"),
            place,
            message,
        }
        .envelope(at, dsn)
    }

    /// The Crash report of an error the window did not catch (#5690), scrubbed as a panic is:
    /// its script is named by its file alone, since its address may be a folder of the person's.
    /// One with no script (a rejection) is told apart from another by its name and message.
    pub fn of_window_error(error: &WindowError, known: &Known, dsn: &str) -> CrashReport {
        let (kind, _) = known.scrub(&without_paths(&error.name));
        let (message, _) = known.scrub(&without_paths(&error.message));
        let place = error.source.as_deref().map(|source| {
            let file = source
                .rsplit(['/', '\\'])
                .find(|part| !part.is_empty())
                .unwrap_or(source);
            let (file, _) = known.scrub(&without_paths(file));
            match (error.line, error.column) {
                (Some(line), Some(column)) => format!("{file}:{line}:{column}"),
                (Some(line), None) => format!("{file}:{line}"),
                _ => file,
            }
        });
        let version = crate::adopt::release_version();
        let (fingerprint, title) = match &place {
            Some(place) => (
                fingerprint(&["window", version, place]),
                format!("purlis's window failed at {place}"),
            ),
            None => (
                fingerprint(&["window", version, &kind, &message]),
                format!("purlis's window failed: {kind}"),
            ),
        };
        Fields {
            platform: "javascript",
            kind,
            mechanism: if error.rejection {
                "onunhandledrejection"
            } else {
                "onerror"
            },
            fingerprint,
            title,
            place: place.unwrap_or_default(),
            message,
        }
        .envelope(None, dsn)
    }
}

/// The fixed fields of a Crash report, already scrubbed.
struct Fields {
    platform: &'static str,
    /// The exception's type: `panic`, or a window error's name.
    kind: String,
    /// How it was caught, as Sentry names it.
    mechanism: &'static str,
    fingerprint: String,
    title: String,
    place: String,
    message: String,
}

impl Fields {
    /// The envelope of these fields, made `at` (now when not known), for the intake at `dsn`.
    fn envelope(self, at: Option<chrono::DateTime<chrono::Utc>>, dsn: &str) -> CrashReport {
        let event_id = uuid::Uuid::new_v4().simple().to_string();
        let now = chrono::Utc::now();
        let timestamp = at.unwrap_or(now).format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let channel = channel();
        let event = json!({
            "event_id": event_id,
            "timestamp": timestamp,
            "platform": self.platform,
            "level": "fatal",
            "release": format!("purlis@{}", crate::adopt::release_version()),
            "environment": channel,
            "tags": {"channel": channel},
            "contexts": {"os": {"name": std::env::consts::OS, "version": os_version()}},
            "exception": {"values": [{
                "type": self.kind,
                "value": self.message,
                "mechanism": {"type": self.mechanism, "handled": false},
            }]},
            "culprit": self.place,
            // Sentry groups by it, so the counts of repeats name the group they belong to.
            "fingerprint": [self.fingerprint],
        })
        .to_string();
        let header = json!({
            "event_id": event_id,
            "sent_at": now.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "dsn": dsn,
        });
        let item =
            json!({"type": "event", "length": event.len(), "content_type": "application/json"});
        CrashReport {
            fingerprint: Some(self.fingerprint),
            title: self.title,
            bytes: format!("{header}\n{item}\n{event}\n"),
            event_id,
        }
    }
}

/// The fingerprint of `parts`: the same crash again in the same version has the same one.
fn fingerprint(parts: &[&str]) -> String {
    Sha256::digest(parts.join("\0"))
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `envelope` carrying the counts of `repeats` (#5693, Q15): each fingerprint and how many times
/// its report was made again since it was sent, in the event's `extra`. A repeat is never sent on
/// its own: its count rides on the next report that leaves. It is the event's own field rather
/// than an envelope item of its own because an envelope holds one event, and Sentry drops an item
/// type it does not know. An envelope that is not three lines of JSON, or no repeats, is left as
/// it is.
pub(super) fn with_repeats(envelope: &str, repeats: &[(String, u32)]) -> String {
    let lines: Vec<&str> = envelope.lines().collect();
    let [header, _, event] = lines[..] else {
        return envelope.to_string();
    };
    let Ok(mut event) = serde_json::from_str::<serde_json::Value>(event) else {
        return envelope.to_string();
    };
    if repeats.is_empty() || !event.is_object() {
        return envelope.to_string();
    }
    event["extra"] = json!({"repeats": repeats
        .iter()
        .map(|(fingerprint, count)| json!({"fingerprint": fingerprint, "count": count}))
        .collect::<Vec<_>>()});
    let event = event.to_string();
    let item = json!({"type": "event", "length": event.len(), "content_type": "application/json"});
    format!("{header}\n{item}\n{event}\n")
}

/// The event id `envelope`'s header names, or none.
pub(super) fn event_id_of(envelope: &str) -> String {
    envelope
        .lines()
        .next()
        .and_then(|header| serde_json::from_str::<serde_json::Value>(header).ok())
        .and_then(|header| header["event_id"].as_str().map(str::to_string))
        .unwrap_or_default()
}

impl CrashReport {
    /// A report draft the person filed from the Inbox, for the intake at `dsn` (#5694): its
    /// title and body as they were shown, marked as an agent's, and whether the person changed
    /// its words. Its body was scrubbed when it was drafted and read before it was filed.
    pub fn of_draft(draft: &Draft, edited: bool, dsn: &str) -> CrashReport {
        let event_id = uuid::Uuid::new_v4().simple().to_string();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let channel = channel();
        let event = json!({
            "event_id": event_id,
            "timestamp": now,
            "platform": "other",
            "level": "info",
            "release": format!("purlis@{}", crate::adopt::release_version()),
            "environment": channel,
            "tags": {
                "channel": channel,
                "kind": draft.kind.verb(),
                "written_by": "agent",
                "edited_by_the_person": edited,
            },
            "contexts": {"os": {"name": std::env::consts::OS, "version": os_version()}},
            "message": {"formatted": format!("{}\n\n{}", draft.title, draft.body)},
            "culprit": draft.title,
        })
        .to_string();
        let header = json!({"event_id": event_id, "sent_at": now, "dsn": dsn});
        let item =
            json!({"type": "event", "length": event.len(), "content_type": "application/json"});
        CrashReport {
            // A draft the person read and filed is never counted away: filing it is their act.
            fingerprint: None,
            title: draft.title.clone(),
            bytes: format!("{header}\n{item}\n{event}\n"),
            event_id,
        }
    }
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
