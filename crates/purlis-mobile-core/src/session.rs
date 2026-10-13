//! A desktop across reconnects (iOS design §6.2, §6.3): the link while there is one, what
//! the app has open to attach again after the next one, the `subscribe(since)` cursor kept
//! per desktop, and a standing the app draws — Connected, Reconnecting…, Desktop unreachable.
//! Nothing is queued to be sent later: a call with no link fails now.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use purlis_session_protocol::noise::Identity;
use purlis_session_protocol::session::{Event, Events, Since};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::watch;

use crate::Error;
use crate::hosts::{self, Host};
use crate::link::Link;
use crate::store::Store;

/// Where the session stands, as the Notice reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    Connected,
    Reconnecting,
    Unreachable(String),
}

/// A connection a session can run a link over.
pub trait Io: AsyncRead + AsyncWrite + Unpin + Send + 'static {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> Io for T {}

/// What a dial answers: a connection, or why none could be made.
pub type Dialed<'a> = Pin<Box<dyn Future<Output = Result<Box<dyn Io>, Error>> + Send + 'a>>;

/// How a session reaches its desktop: TCP to the desktop's addresses, or whatever a test
/// hands it.
pub trait Dial: Send + Sync {
    fn dial<'a>(&'a self, addresses: &'a [String]) -> Dialed<'a>;
}

/// TCP: each address in order, the first that connects within the timeout.
pub struct Tcp {
    pub timeout: Duration,
}

impl Dial for Tcp {
    fn dial<'a>(&'a self, addresses: &'a [String]) -> Dialed<'a> {
        Box::pin(async move {
            let mut tried = Vec::new();
            for address in addresses {
                match tokio::time::timeout(self.timeout, tokio::net::TcpStream::connect(address))
                    .await
                {
                    Ok(Ok(tcp)) => {
                        let _ = tcp.set_nodelay(true);
                        return Ok(Box::new(tcp) as Box<dyn Io>);
                    }
                    Ok(Err(err)) => tried.push(format!("{address}: {err}")),
                    Err(_) => tried.push(format!("{address}: no answer in time")),
                }
            }
            Err(Error::Unreachable(if tried.is_empty() {
                "no address is known for this desktop".into()
            } else {
                tried.join("; ")
            }))
        })
    }
}

/// What the app has open on a chat, to attach again after a reconnect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Open {
    View { chat: String },
    Timeline { chat: String },
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The cursor's record for a desktop: its device id, lowercased, under `cursor-`.
fn cursor_key(device: &str) -> String {
    format!("cursor-{}", device.to_ascii_lowercase())
}

pub struct Session {
    store: Arc<dyn Store>,
    me: Identity,
    /// The desktop as last read from the store: a rename or new addresses made while a
    /// session lives reach its next connect.
    host: Mutex<Host>,
    dial: Arc<dyn Dial>,
    link: Mutex<Option<Arc<Link>>>,
    standing: Arc<watch::Sender<Standing>>,
    /// One connect at a time: a foreground reconnect and a Retry pressed together are one
    /// dial, not two racing for the link.
    connecting: tokio::sync::Mutex<()>,
    /// Which connect made the link in hand: an older link ending moves nothing.
    generation: Arc<AtomicU64>,
    open: Mutex<Vec<Open>>,
}

impl Session {
    pub fn new(store: Arc<dyn Store>, me: Identity, host: Host, dial: Arc<dyn Dial>) -> Session {
        let (standing, _) = watch::channel(Standing::Reconnecting);
        Session {
            store,
            me,
            host: Mutex::new(host),
            dial,
            link: Mutex::new(None),
            standing: Arc::new(standing),
            connecting: tokio::sync::Mutex::new(()),
            generation: Arc::new(AtomicU64::new(0)),
            open: Mutex::new(Vec::new()),
        }
    }

    /// The desktop as this session knows it.
    pub fn host(&self) -> Host {
        lock(&self.host).clone()
    }

    /// The standing as it changes: what the Notice draws.
    pub fn standing(&self) -> watch::Receiver<Standing> {
        self.standing.subscribe()
    }

    /// Which connect made the link in hand: a screen attached under an older number is
    /// attached to a link that is gone, whatever the standing says now.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Connect, or connect again: Reconnecting while it tries, then Connected, or Unreachable
    /// with why. A desktop whose key is not the pinned one is unreachable, never re-paired;
    /// one that no longer knows this phone says so. The desktop's record is read again first,
    /// so a rename or an address added since reaches this connect.
    pub async fn connect(&self) -> Result<(), Error> {
        let _one_at_a_time = self.connecting.lock().await;
        self.standing.send_replace(Standing::Reconnecting);
        *lock(&self.link) = None;
        let device = lock(&self.host).device.clone();
        if let Ok(Some(fresh)) = hosts::find(&*self.store, &device) {
            *lock(&self.host) = fresh;
        }
        let host = self.host();
        let io = match self.dial.dial(&host.addresses).await {
            Ok(io) => io,
            Err(err) => {
                self.standing
                    .send_replace(Standing::Unreachable(err.to_string()));
                return Err(err);
            }
        };
        match Link::open(io, &self.me, &host).await {
            Ok(link) => {
                let link = Arc::new(link);
                let mine = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
                *lock(&self.link) = Some(Arc::clone(&link));
                self.standing.send_replace(Standing::Connected);
                // The desktop going away is noticed here, not only at the next call: the
                // Notice reads Reconnecting… the moment the link ends.
                let mut ended = link.ended();
                let standing = Arc::clone(&self.standing);
                let generation = Arc::clone(&self.generation);
                tokio::spawn(async move {
                    while !*ended.borrow_and_update() {
                        if ended.changed().await.is_err() {
                            break;
                        }
                    }
                    // Only while this is still the session's link: a later connect's standing
                    // is its own.
                    if generation.load(Ordering::SeqCst) == mine {
                        standing.send_if_modified(|now| {
                            if *now == Standing::Connected {
                                *now = Standing::Reconnecting;
                                true
                            } else {
                                false
                            }
                        });
                    }
                });
                Ok(())
            }
            Err(err) => {
                self.standing
                    .send_replace(Standing::Unreachable(err.to_string()));
                Err(err)
            }
        }
    }

    /// The link, while there is one; [`Error::NotConnected`] otherwise, now, with nothing
    /// queued (iOS design §6.3).
    pub fn link(&self) -> Result<Arc<Link>, Error> {
        let link = lock(&self.link).clone().ok_or(Error::NotConnected)?;
        if link.is_closed() {
            *lock(&self.link) = None;
            self.standing.send_replace(Standing::Reconnecting);
            return Err(Error::NotConnected);
        }
        Ok(link)
    }

    /// Subscribe from where this desktop's cursor stands.
    pub async fn subscribe(&self) -> Result<Events, Error> {
        self.link()?.subscribe(self.cursor()).await
    }

    /// The last `seq` held of each device this desktop's log carries.
    pub fn cursor(&self) -> Since {
        self.store
            .get(&cursor_key(&self.host().device))
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<BTreeMap<String, u64>>(&json).ok())
            .unwrap_or_default()
    }

    /// An event was taken: the cursor moves past it, and is kept.
    pub fn saw(&self, event: &Event) -> Result<(), Error> {
        let mut cursor = self.cursor();
        let last = cursor.entry(event.device_id.clone()).or_insert(0);
        if event.seq > *last {
            *last = event.seq;
        }
        let json = serde_json::to_string(&cursor).map_err(|err| Error::Store(err.to_string()))?;
        self.store.set(&cursor_key(&self.host().device), &json)
    }

    /// The app opened a view or a timeline: attached again after the next reconnect.
    pub fn opened(&self, open: Open) {
        let mut opens = lock(&self.open);
        if !opens.contains(&open) {
            opens.push(open);
        }
    }

    pub fn closed(&self, open: &Open) {
        lock(&self.open).retain(|one| one != open);
    }

    pub fn to_reattach(&self) -> Vec<Open> {
        lock(&self.open).clone()
    }
}
