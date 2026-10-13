//! A link to a desktop (iOS design §3, §4.3): Noise XX against the pinned key, then the
//! session protocol as `remote-device`. The protocol's client answers the commands; the
//! streams the desktop opens — a view's bytes, a timeline's items — are taken as they come
//! and handed to whoever attached that view or timeline.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self as protocol_link, Acceptor};
use purlis_session_protocol::noise::{self, Identity};
use purlis_session_protocol::session::{
    self, Attached, Chat, Client, Events, Since, TimelineAttached,
};
use purlis_session_protocol::stream::{self, Kind};
use purlis_session_protocol::timeline::{self, Item, Timelines};
use purlis_session_protocol::version::HANDSHAKE_TIMEOUT;
use purlis_session_protocol::view::{self, Chunk, Viewer};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, watch};

use crate::Error;
use crate::hosts::Host;
use crate::pair::handshake_failed;

/// How many chunks or items a reader may hold before the host is made to wait: the host's
/// window is what the phone has not acknowledged, so this bounds the phone's memory.
const QUEUE: usize = 64;

/// Where a view's acknowledgements come from: the app says how many bytes its terminal has
/// finished drawing, chunk by chunk, and only then does the host hear of them (the host's
/// watermark follows the screen, not the link). Dropping it ends the acknowledging, not the
/// view.
pub type Acks = mpsc::UnboundedSender<usize>;

/// A view's route: where its chunks go, and where its acknowledgements come from.
struct ViewRoute {
    chunks: mpsc::Sender<Chunk>,
    acks: Arc<Mutex<Option<mpsc::UnboundedReceiver<usize>>>>,
}

/// The streams the desktop opened, by the number the phone was told, and the ones that came
/// before their number was known.
#[derive(Default)]
struct Routes {
    views: HashMap<u32, ViewRoute>,
    timelines: HashMap<u32, mpsc::Sender<Item>>,
    parked_views: HashMap<u32, view::Reader>,
    parked_timelines: HashMap<u32, timeline::Reader>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How long one command may wait for the desktop's answer before the link is taken for dead:
/// a connection that is up but silent (a captive portal, a switch from Wi-Fi to cellular)
/// must fail now, not deliver a prompt an hour late (iOS design §6.3).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(15);

pub struct Link {
    client: Client,
    routes: Arc<Mutex<Routes>>,
    events: Mutex<Option<Events>>,
    closed: Arc<AtomicBool>,
    /// `true` once the desktop's side is gone: what a session watches.
    ended: watch::Receiver<bool>,
    router: tokio::task::JoinHandle<()>,
}

impl Drop for Link {
    fn drop(&mut self) {
        self.router.abort();
    }
}

impl Link {
    /// Open over `io`: the handshake against `host`'s pinned key, then the session protocol.
    /// A handshake that does not finish within the protocol's own deadline is
    /// [`Error::TimedOut`]; one that finishes and is then answered with nothing is
    /// [`Error::NotPaired`] — the desktop dropped a key it no longer pins.
    pub async fn open<S>(io: S, me: &Identity, host: &Host) -> Result<Link, Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let secured = tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            noise::connect(io, me, &host.device, Some(host.key)),
        )
        .await
        .map_err(|_| Error::TimedOut)?
        .map_err(handshake_failed)?;
        let plain = noise::pump(secured);
        let link = tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            protocol_link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice),
        )
        .await
        .map_err(|_| Error::TimedOut)?
        .map_err(|_| Error::NotPaired)?;
        let (client, acceptor, events) = Client::new(link);
        let routes = Arc::new(Mutex::new(Routes::default()));
        let closed = Arc::new(AtomicBool::new(false));
        let (ended_tx, ended) = watch::channel(false);
        let router = tokio::spawn(route(
            acceptor,
            Arc::clone(&routes),
            Arc::clone(&closed),
            ended_tx,
        ));
        Ok(Link {
            client,
            routes,
            events: Mutex::new(Some(events)),
            closed,
            ended,
            router,
        })
    }

    /// Whether the desktop's side is gone: every call fails with [`Error::NotConnected`].
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    /// Says `true` once the desktop's side is gone: what a session waits on to say
    /// Reconnecting… without being asked.
    pub fn ended(&self) -> watch::Receiver<bool> {
        self.ended.clone()
    }

    fn up(&self) -> Result<(), Error> {
        if self.is_closed() {
            Err(Error::NotConnected)
        } else {
            Ok(())
        }
    }

    /// One command's answer within [`CALL_TIMEOUT`]; a desktop that stays silent is taken
    /// for gone, and the link closed.
    async fn within<T>(
        &self,
        call: impl std::future::Future<Output = Result<T, session::CallError>>,
    ) -> Result<T, Error> {
        self.up()?;
        match tokio::time::timeout(CALL_TIMEOUT, call).await {
            Ok(answer) => Ok(answer?),
            Err(_) => {
                self.closed.store(true, Ordering::Relaxed);
                self.router.abort();
                Err(Error::TimedOut)
            }
        }
    }

    pub async fn list(&self) -> Result<Vec<Chat>, Error> {
        self.within(self.client.list()).await
    }

    /// Attach a view: the answer, the chunks as they come (`None` once the stream ends), and
    /// where the app acknowledges what its terminal has drawn ([`Acks`]).
    pub async fn attach(
        &self,
        chat: &str,
    ) -> Result<(Attached, mpsc::Receiver<Chunk>, Acks), Error> {
        let attached = self.within(self.client.attach(chat)).await?;
        let (tx, rx) = mpsc::channel(QUEUE);
        let (acks, acked) = mpsc::unbounded_channel();
        // The one sender goes to the pump: either now, for a stream that arrived first, or
        // through the route the router takes it from. Nothing else holds it, so the receiver
        // ends when the desktop ends the stream.
        let mut routes = lock(&self.routes);
        if let Some(reader) = routes.parked_views.remove(&attached.view) {
            drop(routes);
            tokio::spawn(pump_view(reader, tx, Some(acked)));
        } else {
            routes.views.insert(
                attached.view,
                ViewRoute {
                    chunks: tx,
                    acks: Arc::new(Mutex::new(Some(acked))),
                },
            );
        }
        Ok((attached, rx, acks))
    }

    /// Attach a timeline: the answer, and the items as they come; `None` once the stream ends.
    pub async fn attach_timeline(
        &self,
        chat: &str,
    ) -> Result<(TimelineAttached, mpsc::Receiver<Item>), Error> {
        let attached = self.within(self.client.attach_timeline(chat)).await?;
        let (tx, rx) = mpsc::channel(QUEUE);
        let mut routes = lock(&self.routes);
        if let Some(reader) = routes.parked_timelines.remove(&attached.timeline) {
            drop(routes);
            tokio::spawn(pump_timeline(reader, tx));
        } else {
            routes.timelines.insert(attached.timeline, tx);
        }
        Ok((attached, rx))
    }

    pub async fn detach(&self, chat: &str, view: u32) -> Result<(), Error> {
        lock(&self.routes).views.remove(&view);
        self.within(self.client.detach(chat, view)).await
    }

    pub async fn detach_timeline(&self, chat: &str, timeline: u32) -> Result<(), Error> {
        lock(&self.routes).timelines.remove(&timeline);
        self.within(self.client.detach_timeline(chat, timeline))
            .await
    }

    /// Bytes for the chat's terminal, as they are: the quick-key bar's keys.
    pub async fn write(&self, chat: &str, bytes: &[u8]) -> Result<(), Error> {
        self.within(self.client.write(chat, bytes)).await
    }

    /// A turn's worth of text for the chat's agent (iOS design §5.2).
    pub async fn prompt(&self, chat: &str, text: &str) -> Result<(), Error> {
        self.within(self.client.prompt(chat, text)).await
    }

    /// The answer to one of a chat's asks; a lasting choice is refused by the desktop (D8),
    /// and never offered by the UI.
    pub async fn answer(&self, chat: &str, ask: &str, answer: &str) -> Result<(), Error> {
        self.within(self.client.answer(chat, ask, answer)).await
    }

    /// End the chat and everything it started.
    pub async fn stop(&self, chat: &str) -> Result<(), Error> {
        self.within(self.client.stop(chat)).await
    }

    /// Every event after `since`, then each as it is written: the receiver, once per link,
    /// handed over only once the desktop took the subscription.
    pub async fn subscribe(&self, since: Since) -> Result<Events, Error> {
        self.within(self.client.subscribe(since)).await?;
        lock(&self.events)
            .take()
            .ok_or_else(|| Error::Link("the events were already taken".into()))
    }
}

/// Takes each stream the desktop opens and routes it by its kind and number. The route's
/// sender moves into the pump, so when the desktop ends the stream (a view that fell behind,
/// a chat that ended) the app's receiver ends with it and the app knows to attach again.
async fn route(
    mut acceptor: Acceptor,
    routes: Arc<Mutex<Routes>>,
    closed: Arc<AtomicBool>,
    ended: watch::Sender<bool>,
) {
    let mut viewer = Viewer::default();
    let mut timelines = Timelines::default();
    while let Ok(stream) = acceptor.accept().await {
        let Ok(opened) = stream::take(stream).await else {
            continue;
        };
        match opened.kind() {
            Kind::View => {
                let Ok(reader) = viewer.accept_opened(opened).await else {
                    continue;
                };
                let mut routes = lock(&routes);
                match routes.views.remove(&reader.view().0) {
                    Some(route) => {
                        let acked = lock(&route.acks).take();
                        tokio::spawn(pump_view(reader, route.chunks, acked));
                    }
                    None => {
                        routes.parked_views.insert(reader.view().0, reader);
                    }
                }
            }
            Kind::Timeline => {
                let Ok(reader) = timelines.accept_opened(opened).await else {
                    continue;
                };
                let mut routes = lock(&routes);
                match routes.timelines.remove(&reader.timeline().0) {
                    Some(tx) => {
                        tokio::spawn(pump_timeline(reader, tx));
                    }
                    None => {
                        routes.parked_timelines.insert(reader.timeline().0, reader);
                    }
                }
            }
        }
    }
    closed.store(true, Ordering::Relaxed);
    {
        let mut routes = lock(&routes);
        routes.views.clear();
        routes.timelines.clear();
        routes.parked_views.clear();
        routes.parked_timelines.clear();
    }
    let _ = ended.send(true);
}

/// The view's bytes to the app, and the app's acknowledgements back to the host as its
/// terminal draws them: the host's window follows the screen. An app that stopped
/// acknowledging (it dropped the sender) still gets the bytes; the host then waits on its
/// window, which is the bound the protocol means.
async fn pump_view(
    mut reader: view::Reader,
    tx: mpsc::Sender<Chunk>,
    acks: Option<mpsc::UnboundedReceiver<usize>>,
) {
    let mut acks = acks;
    loop {
        tokio::select! {
            chunk = reader.next() => {
                let Some(Ok(chunk)) = chunk else { return };
                if tx.send(chunk).await.is_err() {
                    return;
                }
            }
            acked = async {
                match acks.as_mut() {
                    Some(acks) => acks.recv().await,
                    None => std::future::pending().await,
                }
            } => match acked {
                Some(bytes) => {
                    if reader.ack(bytes).await.is_err() {
                        return;
                    }
                }
                None => acks = None,
            },
        }
    }
}

async fn pump_timeline(mut reader: timeline::Reader, tx: mpsc::Sender<Item>) {
    while let Some(Ok(item)) = reader.next().await {
        if tx.send(item).await.is_err() {
            return;
        }
    }
}
