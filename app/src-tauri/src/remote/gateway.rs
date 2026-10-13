//! The listener's lifecycle, and the commands the window calls (iOS design §3, §4.2, §6.4):
//! where the gateway listens, the pairing window, the paired devices, and the keep-awake
//! setting. The gateway itself is `purlis_session_protocol::gateway`, served over
//! [`super::adapters`] and one [`Board`] per link.
//!
//! The listener binds this machine's primary address and each configured one, never every
//! interface: a phone reaches the desktop over the LAN or a Tailscale address the person
//! named, and nothing else is listened on.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use purlis_core::machine;
use purlis_core::remote::linkkey::{self, LinkKey};
use purlis_core::remote::pairing::{self, Desk, Ticket};
use purlis_core::remote::peers;
use purlis_core::remote::{audit::Act, fingerprint};
use purlis_core::secrets::{Ctx, Env};
use purlis_session_protocol::gateway::{Gateway, Revoked};
use purlis_session_protocol::link::ProvenDevice;
use purlis_session_protocol::noise::Identity;

use super::adapters::{self, DeskAdapter, Pins};
use super::awake;
use super::board::{Board, DeviceOf, Opened};
use crate::hooks::Events;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The addresses a start binds, and the configured ones it passes over, each with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub bind: Vec<SocketAddr>,
    pub skipped: Vec<(String, String)>,
}

/// What to bind: the primary address on `port` where there is one, then each configured
/// address on `port` or on the port it carries. `0.0.0.0` (every interface) is refused, text
/// that is not an address is skipped, and an address named twice is bound once.
pub fn bind_addresses(primary: Option<Ipv4Addr>, configured: &[String], port: u16) -> Planned {
    let mut planned = Planned {
        bind: vec![],
        skipped: vec![],
    };
    if let Some(ip) = primary {
        planned.bind.push(SocketAddr::from((ip, port)));
    }
    for text in configured {
        let parsed = text
            .parse::<SocketAddr>()
            .or_else(|_| text.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, port)));
        match parsed {
            Ok(addr) if addr.ip().is_unspecified() => planned
                .skipped
                .push((text.clone(), "never every interface".into())),
            Ok(addr) if !planned.bind.contains(&addr) => planned.bind.push(addr),
            Ok(_) => {}
            Err(_) => planned
                .skipped
                .push((text.clone(), "not an address".into())),
        }
    }
    planned
}

/// The address this machine reaches the world from: a UDP socket "connected" to a public
/// address sends nothing and learns which local address the route uses. `None` off the
/// network.
pub fn primary_address() -> Option<Ipv4Addr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// A started listener: what it bound, and the tasks that serve it.
struct Running {
    bound: Vec<SocketAddr>,
    tasks: Vec<tauri::async_runtime::JoinHandle<()>>,
}

/// Phone access on this desktop, managed by the app: the desk, the listener while it is on,
/// and the link key once it is read.
pub struct Remote {
    config: Option<PathBuf>,
    events: Option<Events>,
    desk: Arc<Desk>,
    revoked: Revoked,
    /// This desktop's link key, once read from the Keyring (or handed over by a test).
    key: Mutex<Option<LinkKey>>,
    /// Whether a start binds the primary address beside the configured ones; a test binds
    /// loopback alone.
    primary: bool,
    running: Mutex<Option<Running>>,
    /// "Keep this Mac awake while chats are running", as the store has it, read once and
    /// kept here so every chat move does not read the store again.
    keep_awake: AtomicBool,
    awake: awake::Holder,
}

impl Remote {
    pub fn new(config: Option<PathBuf>, events: Option<Events>) -> Remote {
        let desk = Arc::new(Desk::new(config.clone().unwrap_or_default()));
        let revoked = adapters::revocations(&desk);
        let keep_awake = config
            .as_deref()
            .is_some_and(|config| machine::read(config).store.keep_awake);
        Remote {
            config,
            events,
            desk,
            revoked,
            key: Mutex::new(None),
            primary: true,
            running: Mutex::new(None),
            keep_awake: AtomicBool::new(keep_awake),
            awake: awake::Holder::new(awake::Program::real()),
        }
    }

    /// Hold the machine awake exactly while the setting is on and a chat in `opened` is
    /// working or waiting on the person (iOS design §6.4); called after every chat move.
    pub fn reconsider_awake(&self, opened: &dyn Opened) {
        let keep_awake = self.keep_awake.load(Ordering::Relaxed);
        let mut any_working = false;
        if keep_awake {
            opened.with_planes(&mut |planes| {
                any_working = planes
                    .open_now()
                    .into_iter()
                    .filter_map(|plane| planes.held(&plane).ok())
                    .any(|held| {
                        held.chats().open_now().into_iter().any(|open| {
                            let glance = held.board().glance(open.session);
                            glance.asking || glance.state == purlis_core::state::State::Running
                        })
                    });
            });
        }
        self.awake.set(awake::wanted(keep_awake, any_working));
    }

    /// Let the machine sleep: the app is quitting.
    pub fn release_awake(&self) {
        self.awake.set(false);
    }

    /// With the link key given, so no Keyring is read: for a test.
    #[cfg(test)]
    pub fn with_key(self, key: LinkKey) -> Remote {
        *lock(&self.key) = Some(key);
        self
    }

    /// Binding only the configured addresses: for a test, which names loopback.
    #[cfg(test)]
    pub fn only_configured_addresses(mut self) -> Remote {
        self.primary = false;
        self
    }

    /// Holding the machine awake through `program` rather than the platform's own: for a test.
    #[cfg(test)]
    pub fn holding_awake_with(mut self, program: awake::Program) -> Remote {
        self.awake = awake::Holder::new(program);
        self
    }

    /// Whether the keep-awake child is running now.
    #[cfg(test)]
    pub fn holds_awake(&self) -> bool {
        self.awake.held()
    }

    /// The setting as the window just set it, kept for every chat move after.
    pub fn remember_keep_awake(&self, on: bool) {
        self.keep_awake.store(on, Ordering::Relaxed);
    }

    fn config(&self) -> Result<&Path, String> {
        self.config
            .as_deref()
            .ok_or_else(|| "this machine keeps no store, so no phone can pair with it".to_owned())
    }

    /// This desktop's device id, minted on the first ask (ADR 0066).
    pub fn device(&self) -> Result<String, String> {
        machine::device_id(self.config()?).map_err(|why| why.to_string())
    }

    /// This desktop's link key: read once from the Keyring, minted there on the first ask.
    fn key(&self) -> Result<LinkKey, String> {
        if let Some(key) = lock(&self.key).clone() {
            return Ok(key);
        }
        let config = self.config()?;
        let device = self.device()?;
        let ctx = Ctx::new(config, Env::from_process());
        let key = linkkey::load_or_mint(&ctx, &device)?;
        *lock(&self.key) = Some(key.clone());
        Ok(key)
    }

    /// Listen as the store says, over the planes `opened` holds; the addresses bound. Already
    /// listening answers what is bound.
    pub fn start(&self, opened: Arc<dyn Opened>) -> Result<Vec<SocketAddr>, String> {
        if let Some(running) = &*lock(&self.running) {
            return Ok(running.bound.clone());
        }
        let config = self.config()?.to_path_buf();
        let store = machine::read(&config).store;
        let primary = self.primary.then(primary_address).flatten();
        let planned = bind_addresses(primary, &store.gateway.addresses, store.gateway.port);
        for (addr, why) in &planned.skipped {
            tracing::info!("purlis: phone access does not listen on {addr}: {why}");
        }
        if planned.bind.is_empty() {
            return Err(
                "nothing to listen on: this machine has no network address, and none is set".into(),
            );
        }
        let key = self.key()?;
        let identity = Identity::from_private(key.private).map_err(|why| why.to_string())?;
        let host_device = self.device()?;
        let host_for: Arc<dyn Fn(&ProvenDevice) -> Board + Send + Sync> = {
            let config = config.clone();
            let events = self.events.clone();
            Arc::new(move |proven| {
                let key = proven.key();
                let events = events.clone();
                Board::new(
                    Arc::clone(&opened),
                    DeviceOf {
                        key,
                        device: adapters::device_name(&config, &key),
                    },
                    Arc::new(move |act| adapters::audit::act(&events, act)),
                )
            })
        };
        let on_event = {
            let config = config.clone();
            let events = self.events.clone();
            Arc::new(move |heard| adapters::audit::heard(&config, &events, heard))
        };
        let gateway = Arc::new(Gateway {
            identity,
            host_device,
            pins: Arc::new(Pins::new(config.clone())),
            desk: Arc::new(DeskAdapter::new(Arc::clone(&self.desk))),
            revoked: self.revoked.clone(),
            host_for,
            on_event,
        });
        let mut bound = Vec::new();
        let mut tasks = Vec::new();
        let mut refused = Vec::new();
        for addr in planned.bind {
            let listener = match std::net::TcpListener::bind(addr)
                .and_then(|listener| listener.set_nonblocking(true).map(|()| listener))
            {
                Ok(listener) => listener,
                Err(why) => {
                    refused.push(format!("{addr}: {why}"));
                    continue;
                }
            };
            let local = listener.local_addr().unwrap_or(addr);
            let gateway = Arc::clone(&gateway);
            tasks.push(tauri::async_runtime::spawn(async move {
                match tokio::net::TcpListener::from_std(listener) {
                    Ok(listener) => gateway.serve_links(listener).await,
                    Err(why) => tracing::warn!("purlis: phone access lost {local}: {why}"),
                }
            }));
            bound.push(local);
        }
        if bound.is_empty() {
            return Err(format!(
                "no address could be listened on: {}",
                refused.join("; ")
            ));
        }
        for why in refused {
            tracing::warn!("purlis: phone access is not listening on {why}");
        }
        *lock(&self.running) = Some(Running {
            bound: bound.clone(),
            tasks,
        });
        Ok(bound)
    }

    /// Stop listening. Links already open end with their tasks; the pairing window closes.
    pub fn stop(&self) {
        if let Some(running) = lock(&self.running).take() {
            for task in running.tasks {
                task.abort();
            }
        }
        self.desk.close_window();
    }

    /// The addresses listened on now.
    pub fn bound(&self) -> Vec<SocketAddr> {
        lock(&self.running)
            .as_ref()
            .map(|running| running.bound.clone())
            .unwrap_or_default()
    }

    /// Open a pairing window and answer the QR's text (iOS design §4.2): the host's device
    /// id and key, where it listens, and the code, for five minutes.
    pub fn pair_start(&self) -> Result<String, String> {
        let bound = self.bound();
        if bound.is_empty() {
            return Err("turn on phone access first: nothing is listening".into());
        }
        let key = self.key()?;
        let device = self.device()?;
        let addresses = bound.iter().map(ToString::to_string).collect();
        Ok(self
            .desk
            .open_window(&key, &device, addresses, Instant::now())
            .to_qr_text())
    }

    /// Stop pairing: the window closes, a code shown on screen is spent, and a phone whose
    /// offer was still waiting on the person is told it was not confirmed (iOS design §4.2).
    pub fn pair_stop(&self) {
        self.desk.close_window();
        while let Some((ticket, _)) = self.desk.pending() {
            self.desk.cancel(ticket);
        }
    }

    /// The desk itself: for a test of what a stop does to its window.
    #[cfg(test)]
    pub fn desk_for_tests(&self) -> Arc<Desk> {
        Arc::clone(&self.desk)
    }

    /// The window's view of phone access now.
    pub fn status(&self) -> Result<RemoteStatus, String> {
        let config = self.config()?;
        let store = machine::read(config).store;
        let peers = peers::read(config)?
            .into_iter()
            .map(|peer| PeerRow {
                fingerprint: peer.fingerprint(),
                device: peer.device,
                name: peer.name,
                paired: u32::try_from(peer.paired).unwrap_or(u32::MAX),
            })
            .collect();
        let bound = self.bound();
        Ok(RemoteStatus {
            on: store.gateway.on,
            running: !bound.is_empty(),
            bound: bound.iter().map(ToString::to_string).collect(),
            port: store.gateway.port,
            addresses: store.gateway.addresses,
            device: machine::known_device_id(config),
            fingerprint: lock(&self.key).as_ref().map(LinkKey::fingerprint),
            peers,
            keep_awake: store.keep_awake,
            pending: self.pending(),
        })
    }

    fn pending(&self) -> Option<PendingOffer> {
        let (ticket, offer) = self.desk.pending()?;
        Some(PendingOffer {
            ticket: ticket.0.to_string(),
            device: offer.device,
            name: offer.name,
            fingerprint: offer.fingerprint,
        })
    }

    fn update(&self, change: impl FnOnce(&mut machine::Store)) -> Result<(), String> {
        machine::update(self.config()?, change)
            .map(|_| ())
            .map_err(|why| format!("the machine store was not written: {why}"))
    }
}

/// [`Remote::reconsider_awake`] over the app's planes: what every chat move calls, once the
/// window has been told. Nothing to do before the app manages phone access.
pub fn reconsider_awake(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(remote) = app.try_state::<Remote>() {
        remote.reconsider_awake(app);
    }
}

/// Start listening at launch where the store says phone access is on. Nothing stops the app
/// over it: a refusal is logged and the Devices page shows it off.
pub fn start_if_on(app: &tauri::AppHandle) {
    use tauri::Manager;
    let remote = app.state::<Remote>();
    let on = remote
        .config
        .as_deref()
        .is_some_and(|config| machine::read(config).store.gateway.on);
    if on && let Err(why) = remote.start(Arc::new(app.clone())) {
        tracing::warn!("purlis: phone access did not start ({why})");
    }
}

/// One paired phone, as the Devices page lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PeerRow {
    pub device: String,
    pub name: String,
    pub fingerprint: String,
    /// When it paired, seconds since the epoch: a `u32`, since the window's TypeScript takes
    /// no 64-bit number, and it holds until 2106.
    pub paired: u32,
}

/// A phone offering to pair, waiting on the person's Pair or Cancel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PendingOffer {
    pub ticket: String,
    pub device: String,
    pub name: String,
    pub fingerprint: String,
}

/// Phone access as the Devices page draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RemoteStatus {
    /// The store's setting.
    pub on: bool,
    /// Whether something is listening now.
    pub running: bool,
    pub bound: Vec<String>,
    pub port: u16,
    pub addresses: Vec<String>,
    pub device: Option<String>,
    /// This desktop's key fingerprint, once the key has been read.
    pub fingerprint: Option<String>,
    pub peers: Vec<PeerRow>,
    pub keep_awake: bool,
    pub pending: Option<PendingOffer>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

#[tauri::command]
#[specta::specta]
pub fn remote_status(remote: tauri::State<'_, Remote>) -> Result<RemoteStatus, String> {
    remote.status()
}

/// Turn phone access on or off: the setting is kept, and the listener starts or stops.
#[tauri::command]
#[specta::specta]
pub fn remote_set_on(
    app: tauri::AppHandle,
    remote: tauri::State<'_, Remote>,
    on: bool,
) -> Result<RemoteStatus, String> {
    remote.update(|store| store.gateway.on = on)?;
    if on {
        remote.start(Arc::new(app))?;
    } else {
        remote.stop();
    }
    remote.status()
}

/// The addresses to listen on beside the primary one (a Tailscale address, say). A running
/// listener is started again on the new set.
#[tauri::command]
#[specta::specta]
pub fn remote_set_addresses(
    app: tauri::AppHandle,
    remote: tauri::State<'_, Remote>,
    addresses: Vec<String>,
) -> Result<RemoteStatus, String> {
    remote.update(|store| store.gateway.addresses = addresses)?;
    if !remote.bound().is_empty() {
        remote.stop();
        remote.start(Arc::new(app))?;
    }
    remote.status()
}

/// Open a pairing window: the QR's text, which the Devices page draws.
#[tauri::command]
#[specta::specta]
pub fn remote_pair_start(remote: tauri::State<'_, Remote>) -> Result<String, String> {
    remote.pair_start()
}

/// The phone offering to pair now, if one is.
#[tauri::command]
#[specta::specta]
pub fn remote_pair_pending(remote: tauri::State<'_, Remote>) -> Option<PendingOffer> {
    remote.pending()
}

/// Pair: the person checked the name and the fingerprint. The phone is pinned and told.
#[tauri::command]
#[specta::specta]
pub fn remote_pair_confirm(remote: tauri::State<'_, Remote>, ticket: String) -> Result<(), String> {
    let ticket = Ticket(ticket.parse().map_err(|_| "not a ticket".to_owned())?);
    match remote.desk.confirm(ticket, now_secs())? {
        pairing::Verdict::Paired { .. } => Ok(()),
        pairing::Verdict::Refused { why } => Err(why),
    }
}

/// Cancel: the phone is told it was not confirmed, and nothing is pinned.
#[tauri::command]
#[specta::specta]
pub fn remote_pair_cancel(remote: tauri::State<'_, Remote>, ticket: String) -> Result<(), String> {
    let ticket = Ticket(ticket.parse().map_err(|_| "not a ticket".to_owned())?);
    remote.desk.cancel(ticket);
    Ok(())
}

/// Stop pairing: the window closes, and a code shown on screen is spent.
#[tauri::command]
#[specta::specta]
pub fn remote_pair_stop(remote: tauri::State<'_, Remote>) {
    remote.pair_stop();
}

/// Unpair a phone: its record goes, every link it holds closes, and the audit says so.
#[tauri::command]
#[specta::specta]
pub fn remote_unpair(
    remote: tauri::State<'_, Remote>,
    device: String,
) -> Result<RemoteStatus, String> {
    if let Some(peer) = remote.desk.unpair(&device)? {
        adapters::audit::act(
            &remote.events,
            Act::Unpaired {
                device: peer.device,
                key: peer.key,
            },
        );
        tracing::info!("purlis: unpaired a phone (key {})", fingerprint(&peer.key));
    }
    remote.status()
}

/// "Keep this Mac awake while chats are running" (iOS design §6.4): the setting is kept;
/// what holds the machine awake reads it.
#[tauri::command]
#[specta::specta]
pub fn remote_set_keep_awake(
    app: tauri::AppHandle,
    remote: tauri::State<'_, Remote>,
    on: bool,
) -> Result<RemoteStatus, String> {
    remote.update(|store| store.keep_awake = on)?;
    remote.remember_keep_awake(on);
    remote.reconsider_awake(&app);
    remote.status()
}
