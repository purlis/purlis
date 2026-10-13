//! The `remote.*` events (iOS design §4.4): what a paired device did, as metadata. Until ADR
//! 0075's audit store exists they go to the event log, and move to the audit when it does.
//! There is no field here for a prompt's, a message's or an answer's text, by construction
//! (D9).

use serde_json::json;

use crate::eventlog::{Event, Recorder};

/// Every kind this module writes; `docs/plane-format.md`'s events row names them.
pub const KINDS: &[&str] = &[
    "remote.paired",
    "remote.unpaired",
    "remote.linked",
    "remote.refused",
    "remote.answered",
    "remote.prompted",
];

/// The scope word every paired device of a desktop is admitted as.
const SCOPE: &str = "remote-device";

/// One act. Ids and keys only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Paired {
        device: String,
        key: [u8; 32],
    },
    Unpaired {
        device: String,
        key: [u8; 32],
    },
    /// A link opened; `device` is `None` when the host has not mapped the key to its record.
    Linked {
        device: Option<String>,
        key: [u8; 32],
    },
    /// A key this host has not pinned was dropped.
    Refused {
        key: [u8; 32],
    },
    Answered {
        device: String,
        key: [u8; 32],
        chat: String,
        ask: String,
        choice: String,
    },
    Prompted {
        device: String,
        key: [u8; 32],
        chat: String,
        bytes: usize,
    },
}

pub fn record(recorder: &mut Recorder, act: Act) -> std::io::Result<Event> {
    let (kind, chat, body) = match act {
        Act::Paired { device, key } => (
            "remote.paired",
            None,
            json!({"device": device, "fingerprint": super::fingerprint(&key), "scope": SCOPE}),
        ),
        Act::Unpaired { device, key } => (
            "remote.unpaired",
            None,
            json!({"device": device, "fingerprint": super::fingerprint(&key), "scope": SCOPE}),
        ),
        Act::Linked { device, key } => (
            "remote.linked",
            None,
            json!({"device": device, "fingerprint": super::fingerprint(&key), "scope": SCOPE}),
        ),
        Act::Refused { key } => (
            "remote.refused",
            None,
            json!({"device": null, "fingerprint": super::fingerprint(&key), "scope": SCOPE}),
        ),
        Act::Answered {
            device,
            key,
            chat,
            ask,
            choice,
        } => (
            "remote.answered",
            Some(chat),
            json!({
                "device": device,
                "fingerprint": super::fingerprint(&key),
                "scope": SCOPE,
                "ask": ask,
                "choice": choice,
            }),
        ),
        Act::Prompted {
            device,
            key,
            chat,
            bytes,
        } => (
            "remote.prompted",
            Some(chat),
            json!({
                "device": device,
                "fingerprint": super::fingerprint(&key),
                "scope": SCOPE,
                "bytes": bytes,
            }),
        ),
    };
    recorder.remote(kind, chat.as_deref(), body)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_kind_is_named_once() {
        let mut kinds = super::KINDS.to_vec();
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(kinds.len(), super::KINDS.len());
    }
}
