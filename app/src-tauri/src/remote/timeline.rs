//! A chat's level-2 story for a phone (iOS design §5.1): turns and tools from the event log,
//! asks from the hooks, nothing from the terminal. The event log is the named-cause record
//! (ADR 0076); it is read with `eventlog::subscribe` from the chat's first event the host
//! still holds, so the snapshot is what the log holds and the live items follow at the poll.
//! A tool item is a digest — name, verdict, timing — never the call's arguments (ruling O3).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use purlis_core::eventlog::{self, Delivery};
use purlis_core::harness::asks::Raised;
use purlis_core::harness::hooked::HookAsks;
use purlis_core::harness::model::ChoiceScope;
use purlis_session_protocol::link::Opener;
use purlis_session_protocol::session::{self, NO_SUCH_CHAT, Pushed, Refusal};
use purlis_session_protocol::timeline::{Attacher, Choice, Item, Limits, TimelineId, What};
use tokio::sync::mpsc;

use super::board::{Board, Found, refuse};

/// How often a timeline or a subscription looks for new events while attached.
const POLL: Duration = Duration::from_millis(250);

/// What a hooks harness tells of a chat (ADR 0073).
const LEVEL: u8 = 2;

/// The item an event of chat `chat_id` is, or `None` for another chat's event or a kind that
/// tells a phone nothing.
pub fn item_of(event: &eventlog::Event, chat_id: &str) -> Option<Item> {
    if event.chat.as_deref() != Some(chat_id) {
        return None;
    }
    let turn = event.run.clone().unwrap_or_default();
    let what = match event.kind.as_str() {
        "run.started" | "hook.userpromptsubmit" => What::Turn {
            turn,
            began: true,
            stop_reason: None,
        },
        "hook.stop" => What::Turn {
            turn,
            began: false,
            stop_reason: Some("end_turn".into()),
        },
        "hook.sessionend" => What::Turn {
            turn,
            began: false,
            stop_reason: Some("ended".into()),
        },
        kind if kind.starts_with("hook.") && event.body.get("tool").is_some() => What::Tool {
            tool: event.ulid.clone(),
            title: event.body["tool"].as_str().unwrap_or("tool").to_owned(),
            kind: "tool".into(),
            status: event.body["decision"].as_str().unwrap_or("none").to_owned(),
            duration_ms: event
                .body
                .get("tool_ms")
                .and_then(serde_json::Value::as_u64),
        },
        _ => return None,
    };
    Some(Item { level: LEVEL, what })
}

/// An open ask as a phone draws it: the source's summary and choices, the lasting one marked.
pub fn ask_item(raised: &Raised) -> Item {
    Item {
        level: LEVEL,
        what: What::Ask {
            ask: raised.id.to_string(),
            summary: raised.ask.summary.as_str().to_owned(),
            choices: raised
                .ask
                .options
                .iter()
                .map(|choice| Choice {
                    id: choice.id.clone(),
                    label: choice.label.clone(),
                    lasts: choice.scope == ChoiceScope::Always,
                })
                .collect(),
            elicits_secret: raised.ask.elicits_secret,
        },
    }
}

/// The ask `ask` is no longer open, closed by `by`.
pub fn closed_item(ask: &str, by: &str) -> Item {
    Item {
        level: LEVEL,
        what: What::AskClosed {
            ask: ask.into(),
            by: by.into(),
        },
    }
}

/// What a timeline of one chat reads: the chat's id on the wire, its session number as the
/// asks name it, the event log where the host has one, and the asks.
struct Source {
    chat: String,
    session: String,
    log: Option<PathBuf>,
    asks: Arc<HookAsks>,
}

impl Source {
    fn of(board: &Board, found: &Found) -> Result<Source, Refusal> {
        board
            .with_held(found, |held| {
                let chat = held.chats().chat_at(found.session).and_then(|at| at.id)?;
                Some(Source {
                    chat,
                    session: found.session.to_string(),
                    log: held.hooks().event_log().map(|(dir, _)| dir),
                    asks: held.hooks().asks_shared(),
                })
            })?
            .ok_or_else(|| refuse(NO_SUCH_CHAT, "the chat ended"))
    }

    /// The chat's open asks now, oldest first.
    fn pending(&self) -> Vec<Raised> {
        self.asks
            .pending(std::time::Instant::now())
            .into_iter()
            .filter(|raised| raised.chat == self.session)
            .collect()
    }
}

/// Open a timeline of `found` for the phone, numbered `number` on the wire: the snapshot from
/// the log and the open asks, then each new item at the poll, until the task is aborted (a
/// detach, or the link ending, which drops the board and aborts every timeline it held) or
/// the feed is closed (the phone fell behind). The task holds nothing of the board itself —
/// only the asks the phone answered — so the board's drop is never waiting on it.
pub(super) async fn attach(
    board: &Board,
    opener: Opener,
    found: &Found,
    number: u32,
) -> Result<tokio::task::JoinHandle<()>, Refusal> {
    let source = Source::of(board, found)?;
    let answered = board.answered_handle();
    let mut log = source.log.as_deref().map(|dir| eventlog::subscribe(dir, 0));
    let mut snapshot = Vec::new();
    if let Some(log) = log.as_mut() {
        for delivery in log.poll().unwrap_or_default() {
            if let Delivery::Event(event) = delivery {
                snapshot.extend(item_of(&event, &source.chat));
            }
        }
    }
    let mut open: Vec<String> = Vec::new();
    for raised in source.pending() {
        open.push(raised.id.to_string());
        snapshot.push(ask_item(&raised));
    }
    let feed = Attacher::new(opener, Limits::default())
        .attach(TimelineId(number), snapshot)
        .await
        .map_err(|e| refuse("link_failed", e.to_string()))?;
    Ok(tokio::spawn(async move {
        loop {
            tokio::time::sleep(POLL).await;
            if let Some(log) = log.as_mut() {
                for delivery in log.poll().unwrap_or_default() {
                    if let Delivery::Event(event) = delivery
                        && let Some(item) = item_of(&event, &source.chat)
                        && feed.push(&item).is_err()
                    {
                        return;
                    }
                }
            }
            let pending = source.pending();
            for raised in &pending {
                let id = raised.id.to_string();
                if !open.contains(&id) {
                    open.push(id);
                    if feed.push(&ask_item(raised)).is_err() {
                        return;
                    }
                }
            }
            let mut fell_behind = false;
            open.retain(|id| {
                let still = pending.iter().any(|raised| raised.id.to_string() == *id);
                if !still {
                    let by = if answered
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .contains(id)
                    {
                        "device"
                    } else {
                        "desktop"
                    };
                    fell_behind |= feed.push(&closed_item(id, by)).is_err();
                }
                still
            });
            if fell_behind {
                return;
            }
        }
    }))
}

/// `subscribe(since)` (ADR 0066): the event log's events as the protocol pushes them, from
/// after the last `seq` the client holds of this device, one task per link, until the
/// receiver is dropped.
pub(super) async fn subscribe(
    board: Board,
    since: session::Since,
) -> Result<mpsc::Receiver<Pushed>, Refusal> {
    let (dir, device) = board
        .event_log()
        .ok_or_else(|| refuse(session::UNKNOWN_COMMAND, "this host keeps no event log"))?;
    let from = since.get(&device).copied().unwrap_or(0);
    let (tx, rx) = mpsc::channel(256);
    tokio::spawn(async move {
        let mut subscription = eventlog::subscribe(&dir, from);
        loop {
            for delivery in subscription.poll().unwrap_or_default() {
                let pushed = match delivery {
                    Delivery::Event(e) => Pushed::Event(session::Event {
                        v: e.v,
                        device_id: e.device_id,
                        seq: e.seq,
                        ulid: e.ulid,
                        chat: e.chat,
                        run: e.run,
                        parent_run: e.parent_run,
                        kind: e.kind,
                        body: e.body,
                    }),
                    Delivery::Missed { after, resumes_at } => Pushed::Missed {
                        device: device.clone(),
                        after,
                        resumes_at,
                    },
                    // The client's cursor is past the log: it carries on from what follows.
                    Delivery::Ahead { .. } => continue,
                };
                if tx.send(pushed).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(POLL).await;
        }
    });
    Ok(rx)
}
