//! A timeline: one chat's story to one client, on a stream of its own (iOS design §5.1).
//!
//! What a view is for a terminal's bytes, a timeline is for what the host knows of a chat
//! without reading its output (AGENTS.md: nothing parses harness output): a turn began or ended,
//! a tool ran, the agent asked something and who answered, and, for a chat that speaks ACP, its
//! messages and plan. Every item carries the chat's `level` (ADR 0073), so a client knows how
//! much to expect.
//!
//! **On the wire,** the host opens the stream and writes a header ([`TIMELINE`], then the
//! timeline's number and its epoch, each a big-endian `u32`), then one length-delimited JSON
//! object per item: first the bounded snapshot of what it still holds, ended by
//! [`What::CaughtUp`], then each item as it happens. The client writes nothing back: Yamux's
//! per-stream credit is the flow control, so a slow phone throttles only its own stream, and a
//! client that stops reading past the host's queue bound is dropped and attaches again, as a
//! view is (V76).
//!
//! **Message text lives only here and in the host's memory** (D9): it is never written to the
//! event log or to disk for this feature, and this crate logs nothing.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tokio_util::sync::CancellationToken;

use crate::link::{LinkError, Opener, Stream};
use crate::stream::{Kind, Opened};

/// What opens a timeline's stream.
pub const TIMELINE: u8 = 0x02;

/// One item of a chat's timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// How much the host knows of the chat: 2 from its hooks, 3 from ACP (ADR 0073).
    pub level: u8,
    #[serde(flatten)]
    pub what: What,
}

/// Which kind of item, each with what it carries. Unknown fields are ignored, so a later minor
/// may add some; an unknown `item` word fails to read, and a client passes such an item over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum What {
    /// A turn began (`began`), or ended with `stop_reason` (`end_turn`, `cancelled`, …, the
    /// harness's own word).
    Turn {
        turn: String,
        began: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
    },
    /// A chunk of a message's text; `done` on its last chunk. Level 3 only.
    Message {
        turn: String,
        role: Role,
        text: String,
        done: bool,
    },
    /// A tool call: at level 3 with the agent's title, at level 2 its name, verdict and timing
    /// only (ruling O3: a digest, never the arguments).
    Tool {
        tool: String,
        title: String,
        kind: String,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    /// The agent's plan, whole, each time it changes. Level 3 only.
    Plan { steps: Vec<Step> },
    /// The agent asked something: the source's own summary and choices, in its order and
    /// words. A choice that lasts beyond the session is marked, so a phone can leave it out
    /// (D8); a secret's ask has no answerable choice from a phone (FD-27).
    Ask {
        ask: String,
        summary: String,
        choices: Vec<Choice>,
        elicits_secret: bool,
    },
    /// The ask was answered, by `by`: `desktop`, `device`, or the harness's own word when it
    /// closed the ask itself.
    AskClosed { ask: String, by: String },
    /// Tokens so far. Level 3 only.
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },
    /// The snapshot is over: everything after this happened after the attach.
    CaughtUp,
}

/// Who a message is from. A word from a later minor reads as [`Role::Unknown`], so a client
/// keeps the text and draws it plainly rather than losing the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Agent,
    #[serde(other)]
    Unknown,
}

/// Every `item` word this version knows: what [`Reader::next`] reads, and passes over
/// otherwise. A variant added to [`What`] fails `every_item_word_is_known` until it is here.
pub const ITEM_WORDS: &[&str] = &[
    "turn",
    "message",
    "tool",
    "plan",
    "ask",
    "ask_closed",
    "usage",
    "caught_up",
];

/// One step of the agent's plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub text: String,
    /// `pending`, `in_progress`, `completed`: ACP's words.
    pub status: String,
}

/// One of an ask's choices, in the source's own words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub id: String,
    pub label: String,
    /// Whether choosing it writes a rule that outlasts the session.
    pub lasts: bool,
}

/// The most one item may be on the wire. An ask's choices or a plan fit many times over; a
/// message chunk is cut by the host well below it.
pub const MOST_ITEM_BYTES: usize = 64 * 1024;

/// How much one timeline may hold queued before its client is dropped.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Bytes of items queued for the stream and not yet written. Past it, the client has fallen
    /// behind. Yamux's window is the in-flight bound; this is the host's memory bound.
    pub most_queued_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            most_queued_bytes: 256 << 10,
        }
    }
}

/// A timeline's number on a link: which chat's story it is, as the layer above says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimelineId(pub u32);

/// Which attachment of a timeline this is: every re-attach is the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Epoch(pub u32);

/// Why a push was refused, or why the host closed a timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Closed {
    /// The client could not keep up and was dropped ([`Feed::push`]).
    #[error("the timeline fell behind and was dropped; attach it again")]
    FellBehind,
    /// The stream could not be written: the link, or the client's end of it, is gone. The
    /// host learns on the push after it happened, not before.
    #[error("the timeline's stream is gone; attach it again on a live link")]
    Gone,
    /// This one item is over [`MOST_ITEM_BYTES`] on the wire and was not queued. The timeline
    /// is still open: the host cuts the item (a message chunk, a plan) and pushes again.
    #[error("an item over the wire's bound was not queued; the timeline is still open")]
    TooBig,
}

#[derive(Default)]
struct Queue {
    items: VecDeque<Bytes>,
    bytes: usize,
    /// The [`Feed`] was dropped: write what is queued, then end the stream.
    gone: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    more: Notify,
    /// Cancelled when the host closes the timeline: a writer blocked on the stream's credit
    /// stops where it is, as a view's does.
    dropped: CancellationToken,
    why: OnceLock<Closed>,
    most_queued: usize,
}

impl Shared {
    fn queue(&self) -> MutexGuard<'_, Queue> {
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Close the timeline: the first reason given is the one kept, the queue is let go, and
    /// a writer blocked on the stream stops where it is.
    fn close(&self, why: Closed) -> Closed {
        let why = *self.why.get_or_init(|| why);
        let mut queue = self.queue();
        queue.items.clear();
        queue.bytes = 0;
        drop(queue);
        self.dropped.cancel();
        why
    }
}

/// The host's end of a timeline: where a chat's items go.
pub struct Feed {
    shared: Arc<Shared>,
}

impl Feed {
    /// Queue `item` for the stream, without waiting. Past [`Limits::most_queued_bytes`] the
    /// client has fallen behind: the queue is let go, the stream ends, and this and every later
    /// push answer [`Closed::FellBehind`]. The host then attaches the timeline again with a
    /// fresh snapshot at the next epoch. An item over [`MOST_ITEM_BYTES`] is refused as
    /// [`Closed::TooBig`] and nothing else changes. A stream that could not be written answers
    /// [`Closed::Gone`] from the push after it failed.
    pub fn push(&self, item: &Item) -> Result<(), Closed> {
        if let Some(why) = self.shared.why.get() {
            return Err(*why);
        }
        let bytes = encode(item)?;
        let mut queue = self.shared.queue();
        if let Some(why) = self.shared.why.get() {
            return Err(*why);
        }
        if queue.bytes + bytes.len() > self.shared.most_queued {
            drop(queue);
            return Err(self.shared.close(Closed::FellBehind));
        }
        queue.bytes += bytes.len();
        queue.items.push_back(bytes);
        drop(queue);
        self.shared.more.notify_one();
        Ok(())
    }

    /// Bytes queued and not yet written.
    pub fn queued_bytes(&self) -> usize {
        self.shared.queue().bytes
    }

    /// Why the host closed the timeline, once it has.
    pub fn closed(&self) -> Option<Closed> {
        self.shared.why.get().copied()
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.shared.queue().gone = true;
        self.shared.more.notify_one();
    }
}

/// An item as one frame of the wire, or [`Closed::TooBig`] when it would not fit one.
fn encode(item: &Item) -> Result<Bytes, Closed> {
    let bytes = serde_json::to_vec(item).expect("an item is always JSON");
    if bytes.len() > MOST_ITEM_BYTES {
        return Err(Closed::TooBig);
    }
    Ok(Bytes::from(bytes))
}

fn framed(stream: Stream) -> Framed<Stream, LengthDelimitedCodec> {
    LengthDelimitedCodec::builder()
        .max_frame_length(MOST_ITEM_BYTES)
        .new_framed(stream)
}

/// Write the header, the snapshot, `caught_up`, then whatever is pushed, until the feed is
/// dropped or the client falls behind.
async fn write(
    mut stream: Stream,
    timeline: TimelineId,
    epoch: Epoch,
    snapshot: Vec<Bytes>,
    shared: Arc<Shared>,
) -> Result<(), LinkError> {
    let mut header = Vec::with_capacity(9);
    header.push(TIMELINE);
    header.extend_from_slice(&timeline.0.to_be_bytes());
    header.extend_from_slice(&epoch.0.to_be_bytes());
    stream.write_all(&header).await?;
    let mut out = framed(stream);
    let writing = async {
        for item in snapshot {
            out.send(item).await?;
        }
        loop {
            let next = {
                let mut queue = shared.queue();
                match queue.items.pop_front() {
                    Some(item) => {
                        queue.bytes -= item.len();
                        Some(item)
                    }
                    None if queue.gone => return Ok::<(), LinkError>(()),
                    None => None,
                }
            };
            match next {
                Some(item) => out.send(item).await?,
                None => shared.more.notified().await,
            }
        }
    };
    tokio::select! {
        done = writing => done,
        () = shared.dropped.cancelled() => Ok(()),
    }
}

/// The host's timelines on one link: it numbers each timeline's attachments.
pub struct Attacher {
    opener: Opener,
    limits: Limits,
    epochs: Mutex<HashMap<TimelineId, Epoch>>,
}

impl Attacher {
    pub fn new(opener: Opener, limits: Limits) -> Self {
        Attacher {
            opener,
            limits,
            epochs: Mutex::new(HashMap::new()),
        }
    }

    /// Attach `timeline`, new or again, with `snapshot`, the items the host still holds in
    /// order: open its stream at the next epoch. The snapshot's first item's level is
    /// `caught_up`'s; an empty snapshot says level 0, and the layer above tells the client the
    /// level in its answer ([`crate::session::TimelineAttached`]). A snapshot holding an item
    /// over [`MOST_ITEM_BYTES`] is refused before any stream opens: nobody could read it.
    pub async fn attach(
        &self,
        timeline: TimelineId,
        snapshot: Vec<Item>,
    ) -> Result<Feed, LinkError> {
        let level = snapshot.first().map(|item| item.level).unwrap_or(0);
        let caught_up = Item {
            level,
            what: What::CaughtUp,
        };
        let snapshot: Vec<Bytes> = snapshot
            .iter()
            .chain(std::iter::once(&caught_up))
            .map(|item| {
                encode(item).map_err(|too_big| {
                    LinkError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        too_big.to_string(),
                    ))
                })
            })
            .collect::<Result<_, _>>()?;
        let epoch = {
            let mut epochs = self
                .epochs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let next = match epochs.get(&timeline) {
                None => Epoch(1),
                Some(before) => before.0.checked_add(1).map(Epoch).ok_or_else(|| {
                    LinkError::Io(std::io::Error::other("a timeline attached 4 billion times"))
                })?,
            };
            epochs.insert(timeline, next);
            next
        };
        let stream = self.opener.open().await?;
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            more: Notify::new(),
            dropped: CancellationToken::new(),
            why: OnceLock::new(),
            most_queued: self.limits.most_queued_bytes,
        });
        let writer = Arc::clone(&shared);
        tokio::spawn(async move {
            if write(stream, timeline, epoch, snapshot, Arc::clone(&writer))
                .await
                .is_err()
            {
                // The link, or the client's end, is gone: the next push says so.
                writer.close(Closed::Gone);
            }
        });
        Ok(Feed { shared })
    }
}

/// The client's end of a timeline.
pub struct Reader {
    timeline: TimelineId,
    epoch: Epoch,
    input: Framed<Stream, LengthDelimitedCodec>,
}

impl Reader {
    pub fn timeline(&self) -> TimelineId {
        self.timeline
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// The next item, or `None` when the host has ended the timeline. An item whose word is
    /// one this version does not know ([`ITEM_WORDS`]) is from a later minor and is passed
    /// over, as the session protocol passes over a frame kind it does not know. An item of a
    /// known word that does not read is an error: the host is not speaking this protocol.
    pub async fn next(&mut self) -> Option<Result<Item, LinkError>> {
        loop {
            let frame = match self.input.next().await? {
                Ok(frame) => frame,
                Err(e) => return Some(Err(e.into())),
            };
            let value: serde_json::Value = match serde_json::from_slice(&frame) {
                Ok(value) => value,
                Err(e) => return Some(Err(not_an_item(&format!("not JSON: {e}")))),
            };
            let word = value
                .get("item")
                .and_then(|word| word.as_str())
                .map(str::to_owned);
            match word {
                Some(word) if ITEM_WORDS.contains(&word.as_str()) => {
                    return Some(
                        serde_json::from_value::<Item>(value)
                            .map_err(|e| not_an_item(&format!("a `{word}` item: {e}"))),
                    );
                }
                // A word from a later minor: passed over.
                Some(_) => continue,
                None => return Some(Err(not_an_item("an item that names no `item`"))),
            }
        }
    }
}

/// A frame on a timeline that is not an item this protocol says.
fn not_an_item(why: &str) -> LinkError {
    LinkError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("a timeline frame this client cannot read: {why}"),
    ))
}

/// The client's timelines on one link: it takes each the host opens, and refuses an attachment
/// that is not newer than the last it saw of that timeline.
#[derive(Default)]
pub struct Timelines {
    seen: HashMap<TimelineId, Epoch>,
}

impl Timelines {
    /// Take a timeline the host opened, reading its kind first ([`crate::stream::take`]).
    pub async fn accept(&mut self, stream: Stream) -> Result<Reader, LinkError> {
        self.accept_opened(crate::stream::take(stream).await?).await
    }

    /// Take a timeline whose kind the client already read.
    pub async fn accept_opened(&mut self, opened: Opened) -> Result<Reader, LinkError> {
        if opened.kind() != Kind::Timeline {
            return Err(LinkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("a {:?} stream is not a timeline", opened.kind()),
            )));
        }
        let mut stream = opened.into_stream();
        let timeline = TimelineId(stream.read_u32().await?);
        let epoch = Epoch(stream.read_u32().await?);
        if let Some(last) = self.seen.get(&timeline)
            && epoch <= *last
        {
            return Err(LinkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "timeline {} at epoch {}, which is not newer than {}",
                    timeline.0, epoch.0, last.0
                ),
            )));
        }
        self.seen.insert(timeline, epoch);
        Ok(Reader {
            timeline,
            epoch,
            input: framed(stream),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_item_is_one_flat_object_tagged_by_its_kind() {
        let item = Item {
            level: 2,
            what: What::Tool {
                tool: "t1".into(),
                title: "cargo test".into(),
                kind: "execute".into(),
                status: "completed".into(),
                duration_ms: Some(1200),
            },
        };
        assert_eq!(
            serde_json::to_value(&item).unwrap(),
            json!({"level": 2, "item": "tool", "tool": "t1", "title": "cargo test",
                   "kind": "execute", "status": "completed", "duration_ms": 1200})
        );
    }

    #[test]
    fn caught_up_carries_only_its_level() {
        let item = Item {
            level: 3,
            what: What::CaughtUp,
        };
        assert_eq!(
            serde_json::to_value(&item).unwrap(),
            json!({"level": 3, "item": "caught_up"})
        );
    }

    #[test]
    fn a_field_from_a_later_minor_is_ignored_and_an_unknown_item_does_not_read() {
        let read: Item = serde_json::from_value(
            json!({"level": 2, "item": "turn", "turn": "u1", "began": true, "mood": "calm"}),
        )
        .unwrap();
        assert_eq!(
            read.what,
            What::Turn {
                turn: "u1".into(),
                began: true,
                stop_reason: None
            }
        );
        assert!(serde_json::from_value::<Item>(json!({"level": 2, "item": "mood"})).is_err());
    }

    #[test]
    fn every_word_is_snake_case() {
        let items = [
            What::Turn {
                turn: "u".into(),
                began: false,
                stop_reason: Some("end_turn".into()),
            },
            What::Message {
                turn: "u".into(),
                role: Role::Agent,
                text: "hi".into(),
                done: true,
            },
            What::Plan {
                steps: vec![Step {
                    text: "a".into(),
                    status: "pending".into(),
                }],
            },
            What::Ask {
                ask: "a".into(),
                summary: "Allow cargo test?".into(),
                choices: vec![Choice {
                    id: "allow".into(),
                    label: "Allow".into(),
                    lasts: false,
                }],
                elicits_secret: false,
            },
            What::AskClosed {
                ask: "a".into(),
                by: "desktop".into(),
            },
            What::Usage {
                input_tokens: 1,
                output_tokens: 2,
            },
            What::CaughtUp,
        ];
        for what in items {
            let value = serde_json::to_value(Item { level: 3, what }).unwrap();
            let mut keys = vec![];
            collect_keys(&value, &mut keys);
            for key in keys {
                assert!(
                    key.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                    "{key} in {value}"
                );
            }
            let word = value["item"].as_str().unwrap();
            assert!(
                word.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{word}"
            );
        }
    }

    #[test]
    fn every_item_word_is_known_and_a_later_role_reads_as_unknown() {
        let items = [
            What::Turn {
                turn: "u".into(),
                began: true,
                stop_reason: None,
            },
            What::Message {
                turn: "u".into(),
                role: Role::User,
                text: "hi".into(),
                done: true,
            },
            What::Tool {
                tool: "t".into(),
                title: "t".into(),
                kind: "k".into(),
                status: "s".into(),
                duration_ms: None,
            },
            What::Plan { steps: vec![] },
            What::Ask {
                ask: "a".into(),
                summary: "s".into(),
                choices: vec![],
                elicits_secret: false,
            },
            What::AskClosed {
                ask: "a".into(),
                by: "desktop".into(),
            },
            What::Usage {
                input_tokens: 0,
                output_tokens: 0,
            },
            What::CaughtUp,
        ];
        let mut words: Vec<&str> = Vec::new();
        for what in items {
            let value = serde_json::to_value(Item { level: 2, what }).unwrap();
            let word = value["item"].as_str().unwrap().to_owned();
            assert!(
                ITEM_WORDS.contains(&word.as_str()),
                "{word} is not in ITEM_WORDS"
            );
            words.push(ITEM_WORDS.iter().find(|w| **w == word).unwrap());
        }
        words.sort_unstable();
        let mut known = ITEM_WORDS.to_vec();
        known.sort_unstable();
        assert_eq!(words, known, "ITEM_WORDS names a word no variant says");
        let read: Item = serde_json::from_value(json!({
            "level": 3, "item": "message", "turn": "u", "role": "system", "text": "hi", "done": true
        }))
        .unwrap();
        assert_eq!(
            read.what,
            What::Message {
                turn: "u".into(),
                role: Role::Unknown,
                text: "hi".into(),
                done: true
            }
        );
    }

    fn collect_keys(value: &serde_json::Value, into: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, inner) in fields {
                    into.push(key.clone());
                    collect_keys(inner, into);
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|i| collect_keys(i, into)),
            _ => {}
        }
    }
}
