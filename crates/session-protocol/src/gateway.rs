//! The gateway: one accepted stream, served by what its key is (iOS design §4.2, §4.3).
//!
//! After the Noise handshake ([`crate::noise::accept`]) the device's static key is known and
//! nothing else has crossed. Then:
//!
//! - **pinned** ([`Pins::scope_of`] says a scope): the link is served the session protocol as
//!   that scope ([`crate::link::serve_as_a_device`], [`crate::session::serve`]), and closed at
//!   once when a revocation bumps the generation and the key is no longer pinned;
//! - **unpinned, inside a pairing window** ([`PairingDesk::window`]): the link may carry one
//!   frame, the proof ([`crate::pairing`]); the code is spent whatever comes; a right proof is
//!   offered to the window ([`PairingDesk::offered`]) and its verdict is the link's one answer;
//!   a wrong proof, anything but a proof, a second frame, or a window that ends first, closes
//!   the link;
//! - **unpinned otherwise**: dropped before a frame is read, and the refusal is reported with
//!   the key's fingerprint.
//!
//! Everything the host should write down reaches it as a [`Heard`], which holds keys and ids
//! and never any text of the protocol.

use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::sync::watch;

use crate::auth::Scope;
use crate::link::{self, LinkError};
use crate::noise::{self, Identity, fingerprint};
use crate::pairing::{self, Code, Verdict};
use crate::session::{self, Host};
use crate::version::HANDSHAKE_TIMEOUT;

/// How long a pairing window lasts (iOS design §4.2).
pub const WINDOW: Duration = Duration::from_secs(5 * 60);

/// What the host has pinned: which scope a key is admitted as, if any.
pub trait Pins: Send + Sync {
    fn scope_of(&self, key: &[u8; 32]) -> Option<Scope>;
}

/// A device offering to pair: what the window shows ("Pair 'Noro's iPhone'? Key fingerprint …").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub device: String,
    pub name: String,
    pub key: [u8; 32],
    pub fingerprint: String,
}

/// A live pairing code and when it ends.
#[derive(Debug, Clone)]
pub struct Window {
    pub code: Code,
    pub ends: Instant,
}

/// The window's side of a pairing: the live code, taken and spent in one act; the person's
/// answer to an offer; and the gateway's word that it gave an offer up. The desk pins the key
/// itself when it answers `Paired`, and pins nothing for an offer withdrawn.
pub trait PairingDesk: Send + Sync {
    /// The live window, spent in the same breath: one try per code, so a second taker finds
    /// none even when two handshakes finish inside one window.
    fn take(&self) -> Option<Window>;
    /// The gateway has given this offer up (a second frame, the phone went away): nothing the
    /// window presses afterwards may pin it.
    fn withdraw(&self, offer: &Offered);
    fn offered(&self, offer: Offered)
    -> Pin<Box<dyn std::future::Future<Output = Verdict> + Send>>;
}

/// A generation the host bumps at every unpair: a served link re-checks its key then.
pub type Revoked = watch::Receiver<u64>;

/// What happened on a link, for the audit. Keys and ids only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Linked { key: [u8; 32], scope: Scope },
    Refused { key: [u8; 32] },
    PairOffered { key: [u8; 32] },
    Paired { key: [u8; 32], device: String },
    PairRefused { key: [u8; 32], why: String },
    Unlinked { key: [u8; 32] },
}

pub struct Gateway<H: Host> {
    pub identity: Identity,
    /// This host's device id: the prologue, and the `paired` verdict's answer.
    pub host_device: String,
    pub pins: Arc<dyn Pins>,
    pub desk: Arc<dyn PairingDesk>,
    pub revoked: Revoked,
    /// The host a pinned link is served: made for the link, so it knows which device it
    /// serves (the audit names the device, iOS design §4.4).
    pub host_for: Arc<dyn Fn(&link::ProvenDevice) -> H + Send + Sync>,
    pub on_event: Arc<dyn Fn(Heard) + Send + Sync>,
}

impl<H: Host> Gateway<H> {
    /// Serve one accepted stream to its end. `Err` says why it ended early, in words that hold
    /// no text of the protocol.
    pub async fn serve_one<S>(&self, io: S) -> Result<(), String>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let secured = tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            noise::accept(io, &self.identity, &self.host_device),
        )
        .await
        .map_err(|_| "the handshake did not finish in time".to_owned())?
        .map_err(|e| format!("the handshake failed: {e}"))?;
        let key = secured.remote_public();
        match self.pins.scope_of(&key) {
            Some(scope) => self.serve_pinned(secured, key, scope).await,
            // Taking the window spends its code: whatever this link does next, no other link
            // gets a try on the same code.
            None => match self.desk.take() {
                Some(window) if window.ends > Instant::now() => {
                    self.pair(secured, key, window).await
                }
                _ => {
                    (self.on_event)(Heard::Refused { key });
                    drop(secured);
                    Err(format!(
                        "a key this host has not pinned ({}) outside a pairing window",
                        fingerprint(&key)
                    ))
                }
            },
        }
    }

    async fn serve_pinned<S>(
        &self,
        secured: noise::Secured<S>,
        key: [u8; 32],
        scope: Scope,
    ) -> Result<(), String>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (device, stream) = secured.proven_as(scope);
        let host = (self.host_for)(&device);
        let link = link::serve_as_a_device(stream, session::speaks(), device)
            .await
            .map_err(|e| format!("the link did not open: {e}"))?;
        (self.on_event)(Heard::Linked { key, scope });
        let mut revoked = self.revoked.clone();
        let pins = Arc::clone(&self.pins);
        let outcome = {
            // The served future owns the link: when this block ends it is dropped, the yamux
            // connection closes, and the client's next call fails.
            let served = session::serve(link, host, None);
            tokio::pin!(served);
            loop {
                tokio::select! {
                    done = &mut served => break match done {
                        Ok(()) | Err(LinkError::Closed) => Ok(()),
                        Err(e) => Err(e.to_string()),
                    },
                    changed = revoked.changed() => {
                        if changed.is_err() || pins.scope_of(&key) != Some(scope) {
                            break Err(format!("the key {} was unpaired", fingerprint(&key)));
                        }
                    }
                }
            }
        };
        (self.on_event)(Heard::Unlinked { key });
        outcome
    }

    async fn pair<S>(
        &self,
        secured: noise::Secured<S>,
        key: [u8; 32],
        window: Window,
    ) -> Result<(), String>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let host_public = self.identity.public();
        let mut plain = noise::pump(secured);
        let left = window.ends.saturating_duration_since(Instant::now());
        let proof = match tokio::time::timeout(left, pairing::read_proof(&mut plain)).await {
            // A proof with bytes behind it is two frames in one write: the link carries one.
            Ok(Ok((_, more))) if !more.is_empty() => {
                (self.on_event)(Heard::PairRefused {
                    key,
                    why: "a second frame".into(),
                });
                return Err("a second frame on a pairing link".into());
            }
            Ok(Ok((proof, _))) => proof,
            Ok(Err(e)) => {
                (self.on_event)(Heard::PairRefused {
                    key,
                    why: "not a proof".into(),
                });
                return Err(format!("the first frame was not a pairing proof: {e}"));
            }
            Err(_) => {
                (self.on_event)(Heard::PairRefused {
                    key,
                    why: "no proof in time".into(),
                });
                return Err("no pairing proof before the window ended".into());
            }
        };
        if let Err(why) = proof.check() {
            let _ = pairing::send_verdict(&mut plain, &Verdict::Refused { why: why.clone() }).await;
            (self.on_event)(Heard::PairRefused {
                key,
                why: "malformed".into(),
            });
            return Err(format!("a malformed pairing proof: {why}"));
        }
        if !pairing::verify(&window.code, &proof, &key, &host_public, &self.host_device) {
            let why = "the pairing code did not match".to_owned();
            let _ = pairing::send_verdict(&mut plain, &Verdict::Refused { why: why.clone() }).await;
            (self.on_event)(Heard::PairRefused {
                key,
                why: "wrong code".into(),
            });
            return Err(why);
        }
        (self.on_event)(Heard::PairOffered { key });
        let offer = Offered {
            device: proof.device.clone(),
            name: proof.name.clone(),
            key,
            fingerprint: fingerprint(&key),
        };
        let left = window.ends.saturating_duration_since(Instant::now());
        // The person answers in the window, or the window ends first. Meanwhile the phone may
        // send nothing: a second frame, or the phone going away, ends the pairing.
        let verdict = {
            let answer = self.desk.offered(offer.clone());
            let mut nothing_more = [0u8; 1];
            tokio::select! {
                verdict = tokio::time::timeout(left, answer) => match verdict {
                    Ok(verdict) => verdict,
                    Err(_) => {
                        // The window ended with no answer: the offer leaves the desk, so a
                        // Pair pressed later pins nothing.
                        self.desk.withdraw(&offer);
                        Verdict::Refused { why: "the pairing window ended".into() }
                    }
                },
                _ = plain.read(&mut nothing_more) => {
                    // A second frame, or the phone went away: the offer is withdrawn before
                    // the window can confirm it.
                    self.desk.withdraw(&offer);
                    (self.on_event)(Heard::PairRefused {
                        key,
                        why: "a second frame".into(),
                    });
                    return Err("a second frame on a pairing link, or the device went away".into());
                }
            }
        };
        let _ = pairing::send_verdict(&mut plain, &verdict).await;
        match verdict {
            Verdict::Paired { .. } => {
                (self.on_event)(Heard::Paired {
                    key,
                    device: proof.device,
                });
                Ok(())
            }
            Verdict::Refused { why } => {
                (self.on_event)(Heard::PairRefused {
                    key,
                    why: "not confirmed".into(),
                });
                Err(format!("pairing refused: {why}"))
            }
        }
    }

    /// Accept links on `listener` until the future is dropped: the gateway turned off, which
    /// closes the listener and **every link it accepted** with it (the links are tasks of a
    /// `JoinSet` the future owns). One task per link, each its own handshake deadline,
    /// `TCP_NODELAY` so a keystroke is not held for a full segment. An accept that fails (a
    /// phone resetting mid-handshake, no descriptor free) is not the end of listening: the loop
    /// waits a moment and accepts again.
    pub async fn serve_links(self: Arc<Self>, listener: tokio::net::TcpListener)
    where
        H: 'static,
    {
        let mut links = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok((tcp, _peer)) => {
                        let _ = tcp.set_nodelay(true);
                        let gateway = Arc::clone(&self);
                        links.spawn(async move {
                            let _ = gateway.serve_one(tcp).await;
                        });
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                },
                // A link that ended is reaped, so the set holds only live ones.
                Some(_) = links.join_next() => {}
            }
        }
    }

    /// [`Self::serve_links`] on a task of its own, ended by aborting it.
    pub fn listen(self: Arc<Self>, listener: tokio::net::TcpListener) -> tokio::task::JoinHandle<()>
    where
        H: 'static,
    {
        tokio::spawn(self.serve_links(listener))
    }
}
