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
//!
//! **A native crash** (#5691) is sent with the same fixed fields, what the system called the
//! crash for its message, and its crashing thread's stack, taken out of the minidump on the
//! device ([`super::native::stack_of`]): each frame as a module and an offset, each module by its
//! file name and build ids, for the intake to symbolicate with the build's own symbols. Nothing
//! else of the minidump leaves (the operator's ruling (b) of 2026-11-10, on #5691): no memory, no
//! other thread, no environment. Its fingerprint is where it crashed, by those frames, in which
//! purlis version.

use regex::Regex;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::native::CrashStack;
use super::{Draft, Known, NativeCrash, Panic, RanAsSaid, short_place};

/// How many of a native crash's first frames say where it crashed, for its fingerprint.
const FRAMES_TELL_IT: usize = 8;

/// An error the window did not catch, as its JavaScript saw it (#5690): thrown and not caught, or
/// a promise rejected with nobody to handle it. What of it is sent is the core's to say
/// (this module): the name, the scrubbed message, and the script's file name with its line and
/// column; never a stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowError {
    /// The error's name (`TypeError`), or what the window calls a rejection's reason that is no
    /// error.
    pub name: String,
    pub message: String,
    /// The address of the script it was thrown in (`tauri://localhost/assets/index-3f2a9c.js`),
    /// when the window knows it: never for a rejection.
    pub source: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// A promise rejected with nobody to handle it, rather than an error thrown.
    pub rejection: bool,
}

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
            stack: None,
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
            stack: None,
        }
        .envelope(None, dsn)
    }

    /// The Crash report of a native crash (#5691): its kind, scrubbed as a panic's message is, and
    /// its crashing thread's stack, each module named by its file alone and scrubbed too. Where
    /// it crashed is its first frames, so the same frames again in the same version are counted.
    /// One whose stack could not be read has a fingerprint of its own alone: nothing tells it
    /// from another, so it is never counted away, and it is still a crash to the sent list.
    pub fn of_native(crash: &NativeCrash, known: &Known, dsn: &str) -> CrashReport {
        let mut stack = crash.stack.clone();
        for module in &mut stack.modules {
            module.name = known.scrub(&without_paths(&module.name)).0;
        }
        let reason = stack
            .reason
            .take()
            // minidump's own words for the crash (`EXC_BAD_ACCESS / KERN_INVALID_ADDRESS`): no path
            // is in them, and the paths' scrub would take their ` / `.
            .map(|reason| known.scrub(&reason).0)
            .unwrap_or_default();
        let kind = reason
            .split(" / ")
            .next()
            .filter(|kind| !kind.is_empty())
            .unwrap_or("native crash")
            .to_string();
        let place = stack
            .frames
            .first()
            .map(|frame| where_in(&stack, frame))
            .unwrap_or_default();
        let version = crate::adopt::release_version();
        let mut parts = vec!["native".to_string(), version.to_string()];
        if stack.frames.is_empty() {
            parts.push(uuid::Uuid::new_v4().simple().to_string());
        }
        parts.extend(
            stack
                .frames
                .iter()
                .take(FRAMES_TELL_IT)
                .map(|frame| where_in(&stack, frame)),
        );
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        let at = crash
            .at
            .and_then(|at| chrono::DateTime::from_timestamp(i64::try_from(at).ok()?, 0));
        Fields {
            platform: "native",
            kind,
            mechanism: "minidump",
            fingerprint: fingerprint(&parts),
            title: if place.is_empty() {
                "purlis crashed".to_string()
            } else {
                format!("purlis crashed at {place}")
            },
            place,
            message: reason,
            stack: Some(stack),
        }
        .envelope(at, dsn)
    }
}

/// Where `frame` is: its module's name and its offset there (`purlis+0x1a2b3c`).
fn where_in(stack: &CrashStack, frame: &super::native::Frame) -> String {
    let module = stack
        .modules
        .get(frame.module)
        .map_or("?", |module| module.name.as_str());
    format!("{module}+{:#x}", frame.offset)
}

/// A native crash's frames as Sentry reads them: the outermost first, each an offset into its
/// module (`addr_mode` `rel:<module>`), so no address of the process leaves.
fn stacktrace(stack: &CrashStack) -> serde_json::Value {
    let frames: Vec<serde_json::Value> = stack
        .frames
        .iter()
        .rev()
        .map(|frame| {
            json!({
                "instruction_addr": format!("{:#x}", frame.offset),
                "addr_mode": format!("rel:{}", frame.module),
                "package": stack.modules.get(frame.module).map(|module| module.name.clone()),
                "trust": frame.trust,
            })
        })
        .collect();
    json!({ "frames": frames })
}

/// The modules a native crash's frames are in, as Sentry finds their symbols: by debug id.
fn debug_meta(stack: &CrashStack) -> serde_json::Value {
    let kind = match std::env::consts::OS {
        "macos" | "ios" => "macho",
        "windows" => "pe",
        _ => "elf",
    };
    let images: Vec<serde_json::Value> = stack
        .modules
        .iter()
        .map(|module| {
            json!({
                "type": kind,
                "code_file": module.name,
                "debug_id": module.debug_id,
                "code_id": module.code_id,
                "image_addr": "0x0",
                "image_size": module.size,
                "arch": stack.arch,
            })
        })
        .collect();
    json!({ "images": images })
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
    /// A native crash's stack, already scrubbed.
    stack: Option<CrashStack>,
}

impl Fields {
    /// The envelope of these fields, made `at` (now when not known), for the intake at `dsn`.
    fn envelope(self, at: Option<chrono::DateTime<chrono::Utc>>, dsn: &str) -> CrashReport {
        let event_id = uuid::Uuid::new_v4().simple().to_string();
        let mut event = fixed_fields(&event_id, self.platform, at);
        event["exception"] = json!({"values": [{
            "type": self.kind,
            "value": self.message,
            "mechanism": {"type": self.mechanism, "handled": false},
        }]});
        if let Some(stack) = &self.stack {
            event["exception"]["values"][0]["stacktrace"] = stacktrace(stack);
            event["debug_meta"] = debug_meta(stack);
        }
        event["culprit"] = json!(self.place);
        // Sentry groups by it, so the counts of repeats name the group they belong to.
        event["fingerprint"] = json!([self.fingerprint]);
        // And a tag of it, the one place Sentry's API gives it back: the steward's routine
        // matches a report to an issue already filed by it (#5701).
        event["tags"]["fingerprint"] = json!(self.fingerprint);
        CrashReport {
            fingerprint: Some(self.fingerprint),
            title: self.title,
            bytes: format!("{}{}", header(&event_id, dsn), event_item(&event)),
            event_id,
        }
    }
}

/// The fields every Crash report carries, and nothing else: when (`at`, or now), which purlis on
/// which channel, and which operating system.
fn fixed_fields(
    event_id: &str,
    platform: &str,
    at: Option<chrono::DateTime<chrono::Utc>>,
) -> serde_json::Value {
    let timestamp = at
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let channel = channel();
    json!({
        "event_id": event_id,
        "timestamp": timestamp,
        "platform": platform,
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

/// The fingerprint of `parts`: the same crash again in the same version has the same one.
pub(super) fn fingerprint(parts: &[&str]) -> String {
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
    /// A report draft for the intake at `dsn`: its title and body, marked as an agent's, whether
    /// the person changed its words, and whether they `reviewed` it: read and filed it from the
    /// Inbox (#5694), or let Always send send it unread (#5695). Its body was scrubbed when it
    /// was drafted. What its chat ran under (`ran_as`, Q5) rides in its tags, as its body's
    /// footer says it; a failure of purlis's own carries its `fingerprint` (#5697), so the
    /// intake groups the same failure from every chat and its counts name that group.
    ///
    /// It is a message event tagged `written_by=agent`, never a user-feedback item: an envelope
    /// carries one event, and the steward's routine reads events (ADR 0118).
    pub fn of_draft(
        draft: &Draft,
        edited: bool,
        reviewed: bool,
        ran_as: Option<&RanAsSaid>,
        fingerprint: Option<&str>,
        dsn: &str,
    ) -> CrashReport {
        let mut event = message_event(&format!("{}\n\n{}", draft.title, draft.body));
        event["tags"]["kind"] = json!(draft.kind.verb());
        event["tags"]["edited_by_the_person"] = json!(edited);
        // Sent unread by Always send (#5695), or read and filed by the person.
        event["tags"]["reviewed_by_the_person"] = json!(reviewed);
        if let Some(ran_as) = ran_as {
            event["tags"]["harness"] = json!(ran_as.harness);
            event["tags"]["harness_version"] = json!(ran_as.harness_version);
            event["tags"]["persona_kind"] = json!(ran_as.persona);
        }
        if let Some(fingerprint) = fingerprint {
            event["fingerprint"] = json!([fingerprint]);
            // Read back by the steward's routine from the tags Sentry's API answers (#5701).
            event["tags"]["fingerprint"] = json!(fingerprint);
        }
        event["culprit"] = json!(draft.title);
        message_envelope(event, draft.title.clone(), dsn)
    }

    /// **Words a chat added to a draft Always send already sent** (#5697, #5695): a message event
    /// of their own, tagged as the agent's and unreviewed, that names the event it follows
    /// (`follows`, its id) so the intake reads the two as one report.
    pub fn follow_up(follows: &str, title: &str, words: &str, dsn: &str) -> CrashReport {
        let mut event = message_event(&format!(
            "{title}\n\n**What the agent was doing:**\n\n{words}"
        ));
        event["tags"]["edited_by_the_person"] = json!(false);
        event["tags"]["reviewed_by_the_person"] = json!(false);
        event["tags"]["follows"] = json!(follows);
        event["culprit"] = json!(title);
        message_envelope(event, format!("Added to: {title}"), dsn)
    }
}

/// A message event of `text`, written by an agent, with the fixed fields every report carries.
fn message_event(text: &str) -> serde_json::Value {
    let event_id = uuid::Uuid::new_v4().simple().to_string();
    let mut event = fixed_fields(&event_id, "other", None);
    event["level"] = json!("info");
    event["tags"]["written_by"] = json!("agent");
    event["message"] = json!({"formatted": text});
    event
}

/// The envelope of a message `event`, listed as `title`, for the intake at `dsn`.
fn message_envelope(event: serde_json::Value, title: String, dsn: &str) -> CrashReport {
    let event_id = event["event_id"].as_str().unwrap_or_default().to_owned();
    CrashReport {
        // Counted by the held draft it was, never by the envelope (#5697).
        fingerprint: None,
        title,
        bytes: format!("{}{}", header(&event_id, dsn), event_item(&event)),
        event_id,
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
pub(super) fn without_paths(text: &str) -> String {
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
