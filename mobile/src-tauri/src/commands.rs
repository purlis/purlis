//! The commands the screens call (iOS design §3): a few lines each over `purlis_mobile_core`,
//! and the events the desktop's streams become — `view` chunks, `timeline` items, `standing`
//! changes and the subscription's `event`s — each carrying the desktop's device id and the
//! chat, since several desktops may be paired.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use purlis_mobile_core::Error;
use purlis_mobile_core::hosts::{self, Host};
use purlis_mobile_core::pair::pair_tcp;
use purlis_mobile_core::payload::Payload;
use purlis_mobile_core::session::{Open, Session};
use purlis_session_protocol::noise::fingerprint;
use purlis_session_protocol::session::{Attached, Chat, Pushed, TimelineAttached};
use purlis_session_protocol::timeline::Item;
use purlis_session_protocol::view::Chunk;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::phone::{Phone, TimelineOpen, ViewOpen};

/// How long a pairing waits for the desktop's address to answer.
const PAIR_TIMEOUT: Duration = Duration::from_secs(10);

type Shared = Arc<Phone>;

fn words(err: Error) -> String {
    err.to_string()
}

#[derive(Serialize, Clone)]
pub struct PhoneInfo {
    pub id: String,
    pub name: String,
    /// This phone's key fingerprint, as the desktop's Pair question shows it.
    pub fingerprint: String,
}

#[derive(Serialize, Clone)]
pub struct DesktopRow {
    pub device: String,
    pub name: String,
    pub addresses: Vec<String>,
    /// `connected`, `reconnecting`, `unreachable` or `disconnected` (not yet dialled).
    pub standing: &'static str,
    pub why: Option<String>,
    /// Which link this standing is about: it changes with every connect, so a screen knows
    /// to attach again even when it never saw the link go.
    pub generation: u64,
}

#[derive(Serialize, Clone)]
struct ViewEvent {
    device: String,
    chat: String,
    view: u32,
    kind: &'static str,
    b64: String,
}

#[derive(Serialize, Clone)]
struct StreamEnded {
    device: String,
    chat: String,
    number: u32,
}

#[derive(Serialize, Clone)]
struct TimelineEvent {
    device: String,
    chat: String,
    timeline: u32,
    item: Item,
}

#[derive(Serialize, Clone)]
struct StandingEvent {
    device: String,
    standing: &'static str,
    why: Option<String>,
    generation: u64,
}

#[derive(Serialize, Clone)]
struct PushedEvent {
    device: String,
    pushed: Pushed,
}

fn row(phone: &Phone, host: &Host) -> DesktopRow {
    let (standing, why) = phone.standing_of(&host.device);
    DesktopRow {
        device: host.device.clone(),
        name: host.name.clone(),
        addresses: host.addresses.clone(),
        standing,
        why,
        generation: phone.generation_of(&host.device),
    }
}

fn row_of(phone: &Phone, device: &str) -> Result<DesktopRow, String> {
    let host = hosts::find(&**phone.records(), device)
        .map_err(words)?
        .ok_or_else(|| format!("no desktop {device} is paired"))?;
    Ok(row(phone, &host))
}

#[tauri::command]
pub fn phone(phone: State<'_, Shared>) -> PhoneInfo {
    let device = phone.device();
    PhoneInfo {
        id: device.id,
        name: device.name,
        fingerprint: fingerprint(&phone.me().public()),
    }
}

#[tauri::command]
pub fn rename_phone(phone: State<'_, Shared>, name: String) -> Result<PhoneInfo, String> {
    let device = phone.rename(&name).map_err(words)?;
    Ok(PhoneInfo {
        id: device.id,
        name: device.name,
        fingerprint: fingerprint(&phone.me().public()),
    })
}

/// The phone's own link key is forgotten and a new one minted: every pairing made with the
/// old one is void, and the next pairing is made with the new one from now, not from the
/// next launch.
#[tauri::command]
pub fn forget_link_key(phone: State<'_, Shared>) -> Result<PhoneInfo, String> {
    let paired = hosts::read(&**phone.records()).map_err(words)?;
    for host in paired {
        phone.forget_session(&host.device);
        hosts::remove(&**phone.records(), &host.device).map_err(words)?;
    }
    phone.forget_link_key().map_err(words)?;
    let device = phone.device();
    Ok(PhoneInfo {
        id: device.id,
        name: device.name,
        fingerprint: fingerprint(&phone.me().public()),
    })
}

#[tauri::command]
pub fn desktops(phone: State<'_, Shared>) -> Result<Vec<DesktopRow>, String> {
    Ok(hosts::read(&**phone.records())
        .map_err(words)?
        .iter()
        .map(|host| row(&phone, host))
        .collect())
}

/// Pair with the desktop whose code was scanned or pasted, then connect to it.
#[tauri::command]
pub async fn pair(
    app: AppHandle,
    phone: State<'_, Shared>,
    text: String,
) -> Result<DesktopRow, String> {
    let payload = Payload::from_qr_text(&text).map_err(words)?;
    let mut host = pair_tcp(&phone.me(), &phone.device(), &payload, PAIR_TIMEOUT)
        .await
        .map_err(words)?;
    if host.name.is_empty() {
        host.name = host
            .addresses
            .first()
            .cloned()
            .unwrap_or_else(|| "Desktop".into());
    }
    hosts::add(&**phone.records(), host.clone()).map_err(words)?;
    phone.forget_session(&host.device);
    // The pairing is made whatever the first connect does: the row says where it stands.
    let device = host.device.clone();
    let session = phone.session(&device).map_err(words)?;
    if session.connect().await.is_ok() {
        follow(&app, &phone, &device, &session).await;
    }
    emit_standing(&app, &phone, &device);
    row_of(&phone, &device)
}

#[tauri::command]
pub fn rename_desktop(
    phone: State<'_, Shared>,
    device: String,
    name: String,
) -> Result<DesktopRow, String> {
    hosts::rename(&**phone.records(), &device, &name).map_err(words)?;
    row_of(&phone, &device)
}

#[tauri::command]
pub fn set_addresses(
    phone: State<'_, Shared>,
    device: String,
    addresses: Vec<String>,
) -> Result<DesktopRow, String> {
    hosts::set_addresses(&**phone.records(), &device, addresses).map_err(words)?;
    row_of(&phone, &device)
}

/// Unpair on this phone: its record goes and its link ends. The desktop keeps its record
/// until it is unpaired there too (the protocol has no unpair command yet).
#[tauri::command]
pub fn unpair(phone: State<'_, Shared>, device: String) -> Result<(), String> {
    phone.forget_session(&device);
    hosts::remove(&**phone.records(), &device).map_err(words)?;
    Ok(())
}

fn emit_standing(app: &AppHandle, phone: &Phone, device: &str) {
    let (standing, why) = phone.standing_of(device);
    let _ = app.emit(
        "standing",
        StandingEvent {
            device: device.to_owned(),
            standing,
            why,
            generation: phone.generation_of(device),
        },
    );
}

/// The standing's word for the screen, and why when unreachable.
fn standing_words(
    standing: &purlis_mobile_core::session::Standing,
) -> (&'static str, Option<String>) {
    use purlis_mobile_core::session::Standing;
    match standing {
        Standing::Connected => ("connected", None),
        Standing::Reconnecting => ("reconnecting", None),
        Standing::Unreachable(why) => ("unreachable", Some(why.clone())),
    }
}

/// Watch a session's standing from now on, once per session: each change reaches the screen
/// as a `standing` event. Holds the lock for no await.
fn watch_standing(app: &AppHandle, phone: &Phone, device: &str, session: &Arc<Session>) {
    let mut following = phone.following();
    let entry = following.entry(device.to_owned()).or_default();
    if entry.standing.is_some() {
        return;
    }
    let mut standing = session.standing();
    let app = app.clone();
    let of = device.to_owned();
    let session = Arc::clone(session);
    entry.standing = Some(tokio::spawn(async move {
        while standing.changed().await.is_ok() {
            let (word, why) = standing_words(&standing.borrow_and_update());
            let _ = app.emit(
                "standing",
                StandingEvent {
                    device: of.clone(),
                    standing: word,
                    why,
                    generation: session.generation(),
                },
            );
        }
    }));
}

/// Follow a session from now on: its standing as it changes, and its events once subscribed.
async fn follow(app: &AppHandle, phone: &Phone, device: &str, session: &Arc<Session>) {
    watch_standing(app, phone, device, session);
    let previous = phone
        .following()
        .get_mut(device)
        .and_then(|entry| entry.events.take());
    if let Some(task) = previous {
        task.abort();
    }
    let subscribed = session.subscribe().await;
    if let Ok(mut events) = subscribed {
        let app = app.clone();
        let of = device.to_owned();
        let session = Arc::clone(session);
        let task = tokio::spawn(async move {
            while let Some(pushed) = events.recv().await {
                if let Pushed::Event(event) = &pushed {
                    let _ = session.saw(event);
                }
                let _ = app.emit(
                    "event",
                    PushedEvent {
                        device: of.clone(),
                        pushed,
                    },
                );
            }
        });
        phone
            .following()
            .entry(device.to_owned())
            .or_default()
            .events = Some(task);
    }
}

/// Connect to a paired desktop, or connect again: the row says where it stands.
#[tauri::command]
pub async fn connect(
    app: AppHandle,
    phone: State<'_, Shared>,
    device: String,
) -> Result<DesktopRow, String> {
    let session = phone.session(&device).map_err(words)?;
    let outcome = session.connect().await;
    emit_standing(&app, &phone, &device);
    if outcome.is_ok() {
        follow(&app, &phone, &device, &session).await;
    }
    outcome.map_err(words)?;
    row_of(&phone, &device)
}

/// Back in the foreground (iOS design §6.2): every paired desktop is connected again. A
/// desktop that cannot be reached says so in its row; the others carry on.
#[tauri::command]
pub async fn reconnect_all(
    app: AppHandle,
    phone: State<'_, Shared>,
) -> Result<Vec<DesktopRow>, String> {
    let paired = hosts::read(&**phone.records()).map_err(words)?;
    // Every desktop is dialled at once: one that is away must not hold the others back for
    // its timeouts.
    let dialling: Vec<_> = paired
        .into_iter()
        .map(|host| {
            let session = phone.session(&host.device).ok();
            let dial = session
                .clone()
                .map(|session| tokio::spawn(async move { session.connect().await.is_ok() }));
            (host, session, dial)
        })
        .collect();
    let mut rows = Vec::new();
    for (host, session, dial) in dialling {
        if let (Some(session), Some(dial)) = (session, dial) {
            let connected = dial.await.unwrap_or(false);
            emit_standing(&app, &phone, &host.device);
            if connected {
                follow(&app, &phone, &host.device, &session).await;
            }
        }
        rows.push(row(&phone, &host));
    }
    Ok(rows)
}

#[tauri::command]
pub fn standing(phone: State<'_, Shared>, device: String) -> Result<DesktopRow, String> {
    row_of(&phone, &device)
}

fn link_of(
    phone: &Phone,
    device: &str,
) -> Result<(Arc<Session>, Arc<purlis_mobile_core::link::Link>), String> {
    let session = phone.session(device).map_err(words)?;
    let link = session.link().map_err(words)?;
    Ok((session, link))
}

#[tauri::command]
pub async fn list(phone: State<'_, Shared>, device: String) -> Result<Vec<Chat>, String> {
    let (_, link) = link_of(&phone, &device)?;
    link.list().await.map_err(words)
}

/// Open a view of a chat's terminal: its bytes arrive as `view` events, base64, snapshot
/// then live; `ack` says what the terminal drew. A view already open on the chat is replaced.
#[tauri::command]
pub async fn attach(
    app: AppHandle,
    phone: State<'_, Shared>,
    device: String,
    chat: String,
) -> Result<Attached, String> {
    let (session, link) = link_of(&phone, &device)?;
    let (attached, mut chunks, acks) = link.attach(&chat).await.map_err(words)?;
    let key = (device.clone(), chat.clone());
    let replaced = phone.views().remove(&key);
    if let Some(old) = replaced {
        // The view this one replaces ends on the desktop too, so it stops feeding a window
        // nobody drains.
        old.task.abort();
        let _ = link.detach(&chat, old.view).await;
    }
    let view = attached.view;
    let feeding = app.clone();
    let (of, on) = (device.clone(), chat.clone());
    let task = tokio::spawn(async move {
        while let Some(chunk) = chunks.recv().await {
            let (kind, bytes) = match chunk {
                Chunk::Snapshot(bytes) => ("snapshot", bytes),
                Chunk::Live(bytes) => ("live", bytes),
            };
            let _ = feeding.emit(
                "view",
                ViewEvent {
                    device: of.clone(),
                    chat: on.clone(),
                    view,
                    kind,
                    b64: base64::engine::general_purpose::STANDARD.encode(&bytes),
                },
            );
        }
        let _ = feeding.emit(
            "view-ended",
            StreamEnded {
                device: of,
                chat: on,
                number: view,
            },
        );
    });
    phone.views().insert(key, ViewOpen { view, task, acks });
    session.opened(Open::View { chat });
    Ok(attached)
}

/// The terminal drew `bytes` more of the chat's view (xterm's write callback).
#[tauri::command]
pub fn ack(phone: State<'_, Shared>, device: String, chat: String, view: u32, bytes: usize) {
    // An ack names its view: one drawn late, for a view since replaced, moves nothing.
    if let Some(open) = phone.views().get(&(device, chat))
        && open.view == view
    {
        let _ = open.acks.send(bytes);
    }
}

#[tauri::command]
pub async fn detach(phone: State<'_, Shared>, device: String, chat: String) -> Result<(), String> {
    // The phone's own bookkeeping goes first: with no link there is nothing to tell.
    let open = phone.views().remove(&(device.clone(), chat.clone()));
    if let Some(open) = &open {
        open.task.abort();
    }
    if let Some(session) = phone.session_if_any(&device) {
        session.closed(&Open::View { chat: chat.clone() });
    }
    let (_, link) = link_of(&phone, &device)?;
    if let Some(open) = open {
        link.detach(&chat, open.view).await.map_err(words)?;
    }
    Ok(())
}

/// Open a chat's timeline: its items arrive as `timeline` events.
#[tauri::command]
pub async fn attach_timeline(
    app: AppHandle,
    phone: State<'_, Shared>,
    device: String,
    chat: String,
) -> Result<TimelineAttached, String> {
    let (session, link) = link_of(&phone, &device)?;
    let (attached, mut items) = link.attach_timeline(&chat).await.map_err(words)?;
    let key = (device.clone(), chat.clone());
    if let Some(old) = phone.timelines().remove(&key) {
        old.task.abort();
    }
    let timeline = attached.timeline;
    let feeding = app.clone();
    let (of, on) = (device.clone(), chat.clone());
    let task = tokio::spawn(async move {
        while let Some(item) = items.recv().await {
            let _ = feeding.emit(
                "timeline",
                TimelineEvent {
                    device: of.clone(),
                    chat: on.clone(),
                    timeline,
                    item,
                },
            );
        }
        let _ = feeding.emit(
            "timeline-ended",
            StreamEnded {
                device: of,
                chat: on,
                number: timeline,
            },
        );
    });
    phone
        .timelines()
        .insert(key, TimelineOpen { timeline, task });
    session.opened(Open::Timeline { chat });
    Ok(attached)
}

#[tauri::command]
pub async fn detach_timeline(
    phone: State<'_, Shared>,
    device: String,
    chat: String,
) -> Result<(), String> {
    let open = phone.timelines().remove(&(device.clone(), chat.clone()));
    if let Some(open) = &open {
        open.task.abort();
    }
    if let Some(session) = phone.session_if_any(&device) {
        session.closed(&Open::Timeline { chat: chat.clone() });
    }
    let (_, link) = link_of(&phone, &device)?;
    if let Some(open) = open {
        link.detach_timeline(&chat, open.timeline)
            .await
            .map_err(words)?;
    }
    Ok(())
}

/// Bytes for the chat's terminal: the quick-key bar's keys, as they are.
#[tauri::command]
pub async fn write(
    phone: State<'_, Shared>,
    device: String,
    chat: String,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let (_, link) = link_of(&phone, &device)?;
    link.write(&chat, &bytes).await.map_err(words)
}

/// A turn's worth of text for the chat's agent: the composer's Send (iOS design §5.2).
#[tauri::command]
pub async fn prompt(
    phone: State<'_, Shared>,
    device: String,
    chat: String,
    text: String,
) -> Result<(), String> {
    let (_, link) = link_of(&phone, &device)?;
    link.prompt(&chat, &text).await.map_err(words)
}

/// The answer to one of a chat's asks, by its choice's id.
#[tauri::command]
pub async fn answer(
    phone: State<'_, Shared>,
    device: String,
    chat: String,
    ask: String,
    choice: String,
) -> Result<(), String> {
    let (_, link) = link_of(&phone, &device)?;
    link.answer(&chat, &ask, &choice).await.map_err(words)
}

/// End the chat and everything it started.
#[tauri::command]
pub async fn stop(phone: State<'_, Shared>, device: String, chat: String) -> Result<(), String> {
    let (_, link) = link_of(&phone, &device)?;
    link.stop(&chat).await.map_err(words)
}
