// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A view: one terminal's bytes to one client, on a stream of its own.
//!
//! **On the wire,** the host opens the stream and writes a header (`0x01`, then the view's
//! number, its epoch and the snapshot's length, each a big-endian `u32`), the snapshot, and
//! then every byte the terminal writes after it, raw. The snapshot is what
//! `Engine::snapshot` makes: the bytes that draw the scrollback, the screen, the cursor and
//! the modes into a blank terminal. The client writes back only acknowledgements, each a
//! big-endian `u32`: how many more bytes it has drawn.
//!
//! **The host never waits on a view.** [`Feed::push`] queues and returns. A task per view
//! writes the queue to the stream.
//!
//! **A view is of a kind, and what a view does is written once for every kind** (ADR 0068,
//! amended for #1994). A hosted program offers a [`Source`]: each attach hands its snapshot
//! apart from the live bytes that follow it, as an asynchronous stream. [`Views`] holds one
//! link's views of any kind: it numbers them, follows each source on a task (no thread per
//! view), drops a view that falls behind and opens it again from a fresh snapshot, and closes
//! every one at once when the link ends. A terminal's snapshot is its screen and scrollback; a
//! browser's (spec 2) is its latest frame.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use futures::stream::BoxStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{Notify, mpsc, watch};
use tokio_util::sync::CancellationToken;

use crate::link::{LinkError, Opener, Stream};

pub mod frames;
use crate::session::ViewEnd;

/// What opens a view's stream.
pub const VIEW: u8 = 0x01;

/// The most a [`Reader`] hands over in one [`Chunk`].
const MOST_CHUNK_BYTES: usize = 16 * 1024;

/// How much one view may hold, and when it pauses.
///
/// **The defaults were re-measured by SC-16 and kept** (`charter-session-bench`, over a
/// `charterd.sock`, a client drawing at the window's measured 28 MB/s, ADR 0026; macOS, M-series,
/// 2026-10-02, three runs each). On the local link:
///
/// | high / low | keystroke in a flooding pane | 2 MB burst | keystroke beside ten floods |
/// |---|---|---|---|
/// | 16 / 4 KiB | 9.4 ms | 75.8 ms | 0.1 ms |
/// | 32 / 8 KiB | 9.8 ms | 76.0 ms | 0.1 ms |
/// | **64 / 16 KiB** | **10.6 ms** | **76.0 ms** | **0.1 ms** |
/// | 128 / 32 KiB | 12.4 ms | 75.6 ms | 0.1 ms |
/// | 256 / 64 KiB | 15.5 ms | 75.4 ms | 0.1 ms |
///
/// The burst is the client's drawing speed at every size, so a higher watermark buys nothing
/// locally and costs a keystroke typed into a flooding pane what it adds in flight. A lower one
/// saves at most 1.2 ms. The queue is the rest of that keystroke's wait: a full 1 MiB queue
/// measured 36.7 ms, inside L1's 50 ms, where 2 MiB measured 74 ms and 512 KiB 17.4 ms.
///
/// **A long link wants more in flight.** At 20 ms round trip, a 2 MB burst took 820 ms at 64 KiB
/// and 350 ms at 256 KiB, and the flooding pane's keystroke 100 ms against 71 ms. These are the
/// local link's limits; a remote link sizes its own (#930).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Bytes queued for the view and not yet written. Past it, the view has fallen behind.
    pub most_queued_bytes: usize,
    /// Bytes written and not yet acknowledged, past which the host stops writing.
    pub high_watermark: usize,
    /// Bytes not yet acknowledged, below which the host writes again.
    pub low_watermark: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            most_queued_bytes: 1 << 20,
            high_watermark: 64 << 10,
            low_watermark: 16 << 10,
        }
    }
}

/// What a client reads from a view.
#[derive(Debug, PartialEq, Eq)]
pub enum Chunk {
    /// Part of the snapshot. Every snapshot chunk comes before the first live one.
    Snapshot(Bytes),
    /// Bytes the terminal wrote after the snapshot.
    Live(Bytes),
}

/// Why the host closed a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Closed {
    /// The view could not keep up and was dropped (see [`Feed::push`]).
    #[error("the view fell behind and was dropped; attach it again with a fresh snapshot")]
    FellBehind,
    /// The client acknowledged bytes it was never sent.
    #[error("the client acknowledged more than it was sent")]
    OverAcknowledged,
    /// The host closed it: the client detached it, or the host let go of it.
    #[error("the view was closed")]
    Closed,
    /// The link it was on ended, and every view on it with it.
    #[error("the link the view was on ended")]
    LinkEnded,
}

#[derive(Default)]
struct Queue {
    chunks: VecDeque<Bytes>,
    /// The bytes in `chunks`.
    bytes: usize,
    /// The [`Feed`] was dropped: write what is queued, then end the view.
    gone: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    more: Notify,
    /// Cancelled when the host closes the view: the writer stops where it is.
    dropped: CancellationToken,
    /// Cancelled once the writer has ended the view's stream, whatever ended it.
    finished: CancellationToken,
    /// Why, once it has.
    why: OnceLock<Closed>,
    /// Bytes handed to the stream so far, header aside: what the client may acknowledge.
    sent: AtomicU64,
    /// Bytes the client has acknowledged drawing.
    drawn: AtomicU64,
    /// Told each acknowledgement, and each end of the view.
    acked: Notify,
    most_queued: usize,
}

impl Shared {
    fn queue(&self) -> MutexGuard<'_, Queue> {
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Close the view: the first reason given is the one kept.
    fn close(&self, why: Closed) -> Closed {
        let why = *self.why.get_or_init(|| why);
        self.dropped.cancel();
        self.acked.notify_waiters();
        why
    }

    /// Whether nothing is queued and the client has drawn every byte it was sent.
    fn all_drawn(&self) -> bool {
        self.queue().chunks.is_empty()
            && self.sent.load(Ordering::Acquire) == self.drawn.load(Ordering::Acquire)
    }
}

/// The host's end of a view: where the terminal's bytes go.
pub struct Feed {
    shared: Arc<Shared>,
}

impl Feed {
    /// Queue `bytes` for the view, without waiting.
    ///
    /// When the queue would pass [`Limits::most_queued_bytes`], the view has fallen behind:
    /// its queue is let go, its stream ends, and this and every later push answer
    /// [`Closed::FellBehind`]. The host then attaches the view again with a fresh snapshot and the next
    /// epoch, exactly as it attaches a new one.
    pub fn push(&self, bytes: Bytes) -> Result<(), Closed> {
        let mut queue = self.shared.queue();
        if let Some(why) = self.shared.why.get() {
            return Err(*why);
        }
        if queue.bytes + bytes.len() > self.shared.most_queued {
            queue.chunks.clear();
            queue.bytes = 0;
            return Err(self.shared.close(Closed::FellBehind));
        }
        queue.bytes += bytes.len();
        queue.chunks.push_back(bytes);
        drop(queue);
        self.shared.more.notify_one();
        Ok(())
    }

    /// Bytes queued and not yet written: what this view holds of the host's memory, beside
    /// the at most [`Limits::high_watermark`] bytes it has written and the client not drawn.
    pub fn queued_bytes(&self) -> usize {
        self.shared.queue().bytes
    }

    /// Bytes written to the client and not yet acknowledged as drawn: at most the high
    /// watermark, plus the one piece being written.
    pub fn in_flight_bytes(&self) -> usize {
        let sent = self.shared.sent.load(Ordering::Acquire);
        let drawn = self.shared.drawn.load(Ordering::Acquire);
        usize::try_from(sent.saturating_sub(drawn)).unwrap_or(usize::MAX)
    }

    /// **Waits until the client has drawn everything pushed so far** (#2050): nothing is
    /// queued, and every byte written is acknowledged. Returns at once once the view is closed.
    pub async fn drawn(&self) {
        loop {
            let acked = self.shared.acked.notified();
            tokio::pin!(acked);
            acked.as_mut().enable();
            if self.shared.why.get().is_some() || self.shared.all_drawn() {
                return;
            }
            acked.await;
        }
    }

    /// Why the host closed the view, once it has.
    pub fn closed(&self) -> Option<Closed> {
        self.shared.why.get().copied()
    }

    /// Close the view now, for `why`: its stream ends where it is, and what was queued is let go.
    fn close(&self, why: Closed) {
        let mut queue = self.shared.queue();
        queue.chunks.clear();
        queue.bytes = 0;
        drop(queue);
        self.shared.close(why);
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.shared.queue().gone = true;
        self.shared.more.notify_one();
    }
}

/// Start a view on `stream`, a stream this end just opened: write the header and the
/// snapshot, then whatever is pushed to the [`Feed`] it returns, never more than
/// `limits.high_watermark` ahead of what the client has acknowledged.
fn start(
    stream: Stream,
    view: ViewId,
    epoch: Epoch,
    snapshot: Bytes,
    limits: Limits,
) -> (Feed, Arc<Shared>) {
    let shared = Arc::new(Shared {
        queue: Mutex::new(Queue::default()),
        more: Notify::new(),
        dropped: CancellationToken::new(),
        finished: CancellationToken::new(),
        why: OnceLock::new(),
        sent: AtomicU64::new(0),
        drawn: AtomicU64::new(0),
        acked: Notify::new(),
        most_queued: limits.most_queued_bytes,
    });
    let pump = Arc::clone(&shared);
    tokio::spawn(async move {
        let (acks, out) = tokio::io::split(stream);
        let (acked, drawn) = watch::channel(0u64);
        let counting = tokio::spawn(count_acks(acks, acked, Arc::clone(&pump)));
        let mut writer = Gated {
            out,
            sent: 0,
            drawn,
            limits,
            shared: Arc::clone(&pump),
        };
        tokio::select! {
            _ = writer.write(view, epoch, &snapshot, &pump) => {}
            () = pump.dropped.cancelled() => {}
        }
        let _ = writer.out.shutdown().await;
        counting.abort();
        pump.finished.cancel();
        pump.acked.notify_waiters();
    });
    (
        Feed {
            shared: Arc::clone(&shared),
        },
        shared,
    )
}

/// Read the client's acknowledgements, and keep the running total of bytes drawn. A client
/// that acknowledges more than it was sent is wrong about the view, and loses it.
async fn count_acks(mut acks: ReadHalf<Stream>, drawn: watch::Sender<u64>, shared: Arc<Shared>) {
    let mut total: u64 = 0;
    while let Ok(n) = acks.read_u32().await {
        match total.checked_add(u64::from(n)) {
            Some(more) if more <= shared.sent.load(Ordering::Acquire) => {
                total = more;
                shared.drawn.store(total, Ordering::Release);
                drawn.send_replace(total);
                shared.acked.notify_waiters();
            }
            _ => {
                shared.close(Closed::OverAcknowledged);
                return;
            }
        }
    }
}

/// The writing half of a view, held to the watermarks.
struct Gated {
    out: WriteHalf<Stream>,
    sent: u64,
    drawn: watch::Receiver<u64>,
    limits: Limits,
    shared: Arc<Shared>,
}

impl Gated {
    async fn write(
        &mut self,
        view: ViewId,
        epoch: Epoch,
        snapshot: &Bytes,
        shared: &Shared,
    ) -> std::io::Result<()> {
        let length = u32::try_from(snapshot.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "a snapshot over 4 GiB")
        })?;
        let header = Header {
            view,
            epoch,
            snapshot: length,
        };
        self.out.write_all(&header.to_bytes()).await?;
        self.send(snapshot).await?;
        loop {
            let next = {
                let mut queue = shared.queue();
                match queue.chunks.pop_front() {
                    Some(chunk) => {
                        queue.bytes -= chunk.len();
                        Some(chunk)
                    }
                    None if queue.gone => return Ok(()),
                    None => None,
                }
            };
            match next {
                Some(chunk) => self.send(&chunk).await?,
                None => shared.more.notified().await,
            }
        }
    }

    /// Write `bytes` in pieces, waiting before each one while the client is a high watermark
    /// behind, until it is back below the low one.
    async fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let high = self.limits.high_watermark as u64;
        let low = self.limits.low_watermark as u64;
        for piece in bytes.chunks(MOST_CHUNK_BYTES) {
            if self.sent.saturating_sub(*self.drawn.borrow()) >= high {
                let sent = self.sent;
                self.out.flush().await?;
                if self
                    .drawn
                    .wait_for(|drawn| sent.saturating_sub(*drawn) <= low)
                    .await
                    .is_err()
                {
                    return Err(std::io::ErrorKind::BrokenPipe.into());
                }
            }
            // Counted before it is written, so an acknowledgement of it can never arrive first.
            self.sent += piece.len() as u64;
            self.shared.sent.store(self.sent, Ordering::Release);
            self.out.write_all(piece).await?;
        }
        self.out.flush().await
    }
}

/// The client's end of a view.
pub struct Reader {
    view: ViewId,
    epoch: Epoch,
    snapshot_left: usize,
    input: ReadHalf<Stream>,
    acks: WriteHalf<Stream>,
}

impl Reader {
    /// The view's number, as the host gave it.
    pub fn view(&self) -> ViewId {
        self.view
    }

    /// Which snapshot of the view this is. A view attached again after it fell behind comes
    /// back with the same number and a higher epoch, and the client resets its terminal.
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Whether the snapshot is whole: every byte of it has been handed over (at once, for an
    /// empty one), so what comes next is live. A client tells it after each
    /// [`Chunk::Snapshot`], and knows where the screen as it is now ends without waiting for
    /// the program to write again.
    pub fn snapshot_complete(&self) -> bool {
        self.snapshot_left == 0
    }

    /// The next chunk, or `None` when the host has ended the view.
    pub async fn next(&mut self) -> Option<Result<Chunk, LinkError>> {
        let most = if self.snapshot_left > 0 {
            self.snapshot_left.min(MOST_CHUNK_BYTES)
        } else {
            MOST_CHUNK_BYTES
        };
        let mut buffer = vec![0u8; most];
        match self.input.read(&mut buffer).await {
            Ok(0) => None,
            Ok(n) => {
                buffer.truncate(n);
                let bytes = Bytes::from(buffer);
                if self.snapshot_left > 0 {
                    self.snapshot_left -= n;
                    Some(Ok(Chunk::Snapshot(bytes)))
                } else {
                    Some(Ok(Chunk::Live(bytes)))
                }
            }
            Err(e) => Some(Err(e.into())),
        }
    }

    /// Tell the host `bytes` more have been drawn. A client acks what its terminal has
    /// finished writing, xterm's write callback, not what it has merely read.
    pub async fn ack(&mut self, bytes: usize) -> Result<(), LinkError> {
        let mut left = bytes;
        while left > 0 {
            let n = left.min(u32::MAX as usize);
            self.acks.write_all(&(n as u32).to_be_bytes()).await?;
            left -= n;
        }
        self.acks.flush().await?;
        Ok(())
    }
}

/// Take a view the host opened: read its header.
async fn accept(stream: Stream) -> Result<Reader, LinkError> {
    let (mut input, acks) = tokio::io::split(stream);
    let header = Header::read_from(&mut input).await?;
    Ok(reading(header, input, acks))
}

fn reading(header: Header, input: ReadHalf<Stream>, acks: WriteHalf<Stream>) -> Reader {
    Reader {
        view: header.view,
        epoch: header.epoch,
        snapshot_left: header.snapshot as usize,
        input,
        acks,
    }
}

/// A stream the host opened, as its first byte says it is (1.34): a view's, or one that carries
/// a bundle of code ([`crate::code`]).
pub enum Opened {
    View(Reader),
    /// A code stream, after its header: the bundle's bytes follow in the direction its command
    /// says.
    Code(crate::code::Header, Stream),
}

/// What a view's stream opens with ([`VIEW`], then the view's number, its epoch and its
/// snapshot's length, each a big-endian `u32`): what a host writes, and what a host that joins
/// a view's stream from another host to a stream of its own reads and writes again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub view: ViewId,
    pub epoch: Epoch,
    /// How many bytes of the stream, after the header, are the snapshot.
    pub snapshot: u32,
}

impl Header {
    /// How long a header is on the wire.
    pub const LEN: usize = 13;

    /// The header as it is written.
    pub fn to_bytes(&self) -> [u8; Header::LEN] {
        let mut bytes = [0u8; Header::LEN];
        bytes[0] = VIEW;
        bytes[1..5].copy_from_slice(&self.view.0.to_be_bytes());
        bytes[5..9].copy_from_slice(&self.epoch.0.to_be_bytes());
        bytes[9..13].copy_from_slice(&self.snapshot.to_be_bytes());
        bytes
    }

    /// Read a header from the start of a view's stream: refused when the stream is not a view.
    pub async fn read_from<R: tokio::io::AsyncRead + Unpin>(
        input: &mut R,
    ) -> Result<Header, LinkError> {
        let kind = input.read_u8().await?;
        if kind != VIEW {
            return Err(not_a_view(kind));
        }
        Header::read_after_kind(input).await
    }

    /// The rest of a view's header, once its first byte, [`VIEW`], has been read.
    pub async fn read_after_kind<R: tokio::io::AsyncRead + Unpin>(
        input: &mut R,
    ) -> Result<Header, LinkError> {
        Ok(Header {
            view: ViewId(input.read_u32().await?),
            epoch: Epoch(input.read_u32().await?),
            snapshot: input.read_u32().await?,
        })
    }
}

fn not_a_view(kind: u8) -> LinkError {
    LinkError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("a stream that opens with {kind:#04x} is not a view"),
    ))
}

/// A view's number on a link: which terminal it shows. The layer above says which chat that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(pub u32);

/// Which attachment of a view this is. Every re-attach, after a view fell behind, is the next
/// one, and a client resets its terminal for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Epoch(pub u32);

/// The pause before re-attaching a view whose client drew nothing before it fell behind. It
/// doubles with each such attachment in a row, up to [`MOST_BACKOFF`], and a client that
/// draws anything starts again from none.
pub const FIRST_BACKOFF: Duration = Duration::from_millis(250);

/// The longest pause before a re-attach.
pub const MOST_BACKOFF: Duration = Duration::from_secs(8);

/// What the host remembers of a view's last attachment.
struct Attached {
    epoch: Epoch,
    /// Attachments in a row that fell behind with nothing drawn.
    stalled: u32,
    last: Option<Arc<Shared>>,
}

/// The host's views on one link: it numbers each view's attachments, and paces the
/// re-attachment of a client that never draws.
pub struct Attacher {
    opener: Opener,
    limits: Limits,
    views: Mutex<HashMap<ViewId, Attached>>,
}

impl Attacher {
    pub fn new(opener: Opener, limits: Limits) -> Self {
        Attacher {
            opener,
            limits,
            views: Mutex::new(HashMap::new()),
        }
    }

    /// Attach `view`, new or again, with `snapshot`: open its stream, at the next epoch, after
    /// the pause its record calls for.
    pub async fn attach(&self, view: ViewId, snapshot: Bytes) -> Result<Feed, LinkError> {
        let (epoch, pause) = self.next_epoch(view)?;
        tokio::time::sleep(pause).await;
        self.open_at(view, epoch, snapshot).await
    }

    /// Attach `view`, new or again, from `source`: wait out the pause its record calls for
    /// first, then take the source's snapshot and live bytes, so the snapshot is the program as
    /// it is when the view comes back and nothing is held for the view while it waits.
    pub async fn attach_from(
        &self,
        view: ViewId,
        source: &dyn Source,
    ) -> Result<(Feed, BoxStream<'static, Bytes>), LinkError> {
        let (epoch, pause) = self.next_epoch(view)?;
        tokio::time::sleep(pause).await;
        let Attachment { snapshot, live } = source.attach();
        let feed = self.open_at(view, epoch, snapshot).await?;
        Ok((feed, live))
    }

    /// The epoch `view`'s next attachment is, and the pause before it, kept in its record.
    fn next_epoch(&self, view: ViewId) -> Result<(Epoch, Duration), LinkError> {
        {
            let mut views = self
                .views
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let next = match views.get(&view) {
                None => Attached {
                    epoch: Epoch(1),
                    stalled: 0,
                    last: None,
                },
                Some(before) => {
                    let epoch = before.epoch.0.checked_add(1).map(Epoch).ok_or_else(|| {
                        LinkError::Io(std::io::Error::other("a view attached 4 billion times"))
                    })?;
                    let stalled = match &before.last {
                        Some(last)
                            if last.why.get() == Some(&Closed::FellBehind)
                                && last.drawn.load(Ordering::Acquire) == 0 =>
                        {
                            before.stalled.saturating_add(1)
                        }
                        _ => 0,
                    };
                    Attached {
                        epoch,
                        stalled,
                        last: None,
                    }
                }
            };
            let pause = match next.stalled {
                0 => Duration::ZERO,
                n => FIRST_BACKOFF
                    .saturating_mul(1 << (n - 1).min(16))
                    .min(MOST_BACKOFF),
            };
            let epoch = next.epoch;
            views.insert(view, next);
            Ok((epoch, pause))
        }
    }

    /// Open `view`'s stream at `epoch` with `snapshot`, and keep the attachment in its record.
    async fn open_at(
        &self,
        view: ViewId,
        epoch: Epoch,
        snapshot: Bytes,
    ) -> Result<Feed, LinkError> {
        let stream = self.opener.open().await?;
        let (feed, shared) = start(stream, view, epoch, snapshot, self.limits);
        if let Some(attached) = self
            .views
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_mut(&view)
            .filter(|attached| attached.epoch == epoch)
        {
            attached.last = Some(shared);
        }
        Ok(feed)
    }
}

/// The client's views on one link: it takes each view the host opens, and refuses an
/// attachment that is not newer than the last it saw of that view.
#[derive(Default)]
pub struct Viewer {
    seen: HashMap<ViewId, Epoch>,
}

impl Viewer {
    /// Take a view the host opened.
    pub async fn accept(&mut self, stream: Stream) -> Result<Reader, LinkError> {
        let reader = accept(stream).await?;
        self.seen_now(reader)
    }

    /// Take a stream the host opened, of whichever kind its first byte says (1.34): a view, held
    /// to the epochs [`Viewer::accept`] holds it to, or a code stream ([`crate::code`]). Any other
    /// kind is refused.
    pub async fn accept_any(&mut self, stream: Stream) -> Result<Opened, LinkError> {
        let (mut input, acks) = tokio::io::split(stream);
        match input.read_u8().await? {
            VIEW => {
                let header = Header::read_after_kind(&mut input).await?;
                self.seen_now(reading(header, input, acks))
                    .map(Opened::View)
            }
            crate::code::CODE => {
                let header = crate::code::Header::read_after_kind(&mut input).await?;
                Ok(Opened::Code(header, input.unsplit(acks)))
            }
            kind => Err(not_a_view(kind)),
        }
    }

    fn seen_now(&mut self, reader: Reader) -> Result<Reader, LinkError> {
        if let Some(last) = self.seen.get(&reader.view)
            && reader.epoch <= *last
        {
            return Err(LinkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "view {} at epoch {}, which is not newer than {}",
                    reader.view.0, reader.epoch.0, last.0
                ),
            )));
        }
        self.seen.insert(reader.view, reader.epoch);
        Ok(reader)
    }
}

/// What one attach of a view hands over: the program as it is now, and what it does after.
pub struct Attachment {
    /// The bytes that draw the program as it is now into a blank view of its kind: a
    /// terminal's scrollback, screen, cursor and modes, or a browser's latest frame.
    pub snapshot: Bytes,
    /// Every byte the program makes after the snapshot, in order, as it makes it. It ends when
    /// the program does, and the view ends with it.
    pub live: BoxStream<'static, Bytes>,
}

/// A hosted program as a view of one kind shows it: what [`Views::open`] attaches to.
///
/// Each [`Source::attach`] takes the snapshot and starts the live bytes after it together, so
/// nothing the program writes is lost or doubled between the two. A view that falls behind is
/// attached again the same way, so a kind brings its snapshot and nothing else.
pub trait Source: Send + Sync + 'static {
    fn attach(&self) -> Attachment;

    /// **Whether its view keeps one frame in flight** (#2050, spec #2041): a source whose live
    /// stream is its latest frame and each newer one, a browser's, is read for its next only
    /// once the client has drawn everything sent before, so the client is sent the newest frame
    /// and never a queue of older ones. A terminal's bytes are each needed, and are sent up to
    /// the view's watermark.
    fn one_frame_in_flight(&self) -> bool {
        false
    }
}

/// The views open on one link, of whatever kind: what a host's views are, written once.
///
/// Each view follows its source on a task of the link's runtime. When the view falls behind,
/// its live bytes are let go, it waits out [`Attacher`]'s pause when its client drew nothing,
/// and then it is attached again, at the next epoch, from a fresh [`Source::attach`] taken
/// after the pause ([`Attacher::attach_from`]). It numbers its views itself, so a link's views
/// are numbered by one `Views` or by the host's own [`Attacher`], never by both
/// ([`crate::session::Peer::opener`]). When the
/// source's live bytes end, the view ends after what was queued. [`Views::end`], when the link
/// ends, closes every view at once and opens no more.
pub struct Views {
    attacher: Arc<Attacher>,
    next: AtomicU32,
    open: Arc<Mutex<Open>>,
    /// Where a view that ended for good, unasked, is told ([`Views::ends`]).
    ended: mpsc::UnboundedSender<(ViewId, ViewEnd)>,
    ends: Mutex<Option<mpsc::UnboundedReceiver<(ViewId, ViewEnd)>>>,
}

#[derive(Default)]
struct Open {
    /// The link has ended: nothing opens.
    ended: bool,
    /// Each open view, and what closes it.
    views: HashMap<ViewId, CancellationToken>,
}

impl Views {
    /// The views of the link whose streams `opener` opens, each held to `limits`.
    pub fn new(opener: Opener, limits: Limits) -> Views {
        let (ended, ends) = mpsc::unbounded_channel();
        Views {
            attacher: Arc::new(Attacher::new(opener, limits)),
            next: AtomicU32::new(1),
            open: Arc::default(),
            ended,
            ends: Mutex::new(Some(ends)),
        }
    }

    /// Each view that ended for good when nobody asked it to, and why, once its stream has
    /// ended: its program ended, its client acknowledged more than it was sent, or it could not
    /// be attached again. A view the client detached, or one closed with its link, is not told
    /// here. Taken once; [`crate::session::serve`] takes it, and pushes each as `view_ended`.
    pub fn ends(&self) -> Option<mpsc::UnboundedReceiver<(ViewId, ViewEnd)>> {
        self.ends
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }

    fn open_views_lock(&self) -> MutexGuard<'_, Open> {
        self.open
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Open a view of `source`: attach it, open its stream with the snapshot, and follow it.
    /// Answers with the view's number on this link once its stream is open, so a client told
    /// of the view already has its stream. Refused once the link has ended.
    pub async fn open(&self, source: Arc<dyn Source>) -> Result<ViewId, LinkError> {
        if self.open_views_lock().ended {
            return Err(LinkError::Closed);
        }
        let view = ViewId(self.next.fetch_add(1, Ordering::Relaxed));
        let (feed, live) = self.attacher.attach_from(view, &*source).await?;
        let closing = CancellationToken::new();
        {
            let mut open = self.open_views_lock();
            if open.ended {
                feed.close(Closed::LinkEnded);
                return Err(LinkError::Closed);
            }
            open.views.insert(view, closing.clone());
        }
        tokio::spawn(follow(
            view,
            source,
            live,
            feed,
            Arc::clone(&self.attacher),
            closing,
            Arc::clone(&self.open),
            self.ended.clone(),
        ));
        Ok(view)
    }

    /// Close `view`: its stream ends and its source is let go of; the program runs on.
    /// Answers whether it was open.
    pub fn close(&self, view: ViewId) -> bool {
        match self.open_views_lock().views.remove(&view) {
            Some(closing) => {
                closing.cancel();
                true
            }
            None => false,
        }
    }

    /// The link has ended: close every view on it at once, and open no more.
    pub fn end(&self) {
        let mut open = self.open_views_lock();
        open.ended = true;
        for (_, closing) in open.views.drain() {
            closing.cancel();
        }
    }

    /// The views open on this link.
    pub fn open_views(&self) -> usize {
        self.open_views_lock().views.len()
    }
}

/// Follow one view's source until the view is closed, its client loses it, or the program's
/// live bytes end; attach it again whenever it falls behind. A view that ends for good
/// unasked is told on `ended` once its stream has ended.
#[allow(clippy::too_many_arguments)]
async fn follow(
    view: ViewId,
    source: Arc<dyn Source>,
    mut live: BoxStream<'static, Bytes>,
    mut feed: Feed,
    attacher: Arc<Attacher>,
    closing: CancellationToken,
    open: Arc<Mutex<Open>>,
    ended: mpsc::UnboundedSender<(ViewId, ViewEnd)>,
) {
    let mut why = ViewEnd::ProgramEnded;
    let one_at_a_time = source.one_frame_in_flight();
    loop {
        if one_at_a_time {
            tokio::select! {
                biased;
                () = closing.cancelled() => break,
                () = feed.drawn() => {}
            }
        }
        let next = tokio::select! {
            biased;
            () = closing.cancelled() => break,
            next = live.next() => next,
        };
        let Some(bytes) = next else {
            // The program's live bytes ended: the view writes what it holds, then ends.
            break;
        };
        match feed.push(bytes) {
            Ok(()) => {}
            Err(Closed::FellBehind) => {
                // Let go of the program while the view waits out its pause: what it writes
                // meanwhile is in the fresh snapshot, taken after the pause.
                drop(std::mem::replace(
                    &mut live,
                    futures::stream::empty().boxed(),
                ));
                let again = tokio::select! {
                    biased;
                    () = closing.cancelled() => break,
                    again = attacher.attach_from(view, &*source) => again,
                };
                match again {
                    Ok((again, follows)) => {
                        feed = again;
                        live = follows;
                    }
                    Err(_) => {
                        why = ViewEnd::Closed;
                        break;
                    }
                }
            }
            Err(Closed::OverAcknowledged) => {
                why = ViewEnd::OverAcknowledged;
                break;
            }
            Err(_) => {
                why = ViewEnd::Closed;
                break;
            }
        }
    }
    if closing.is_cancelled() {
        let ended = open
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .ended;
        feed.close(if ended {
            Closed::LinkEnded
        } else {
            Closed::Closed
        });
    } else {
        open.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .views
            .remove(&view);
        // Told once the stream has ended, so a client reads the view's last bytes before it
        // hears there are no more; unless the link ended meanwhile, and nobody is there.
        let done = feed.shared.finished.clone();
        drop(feed);
        done.cancelled().await;
        let link_ended = open
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .ended;
        if !link_ended {
            let _ = ended.send((view, why));
        }
    }
}
