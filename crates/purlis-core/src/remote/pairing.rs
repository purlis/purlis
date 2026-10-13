//! Pairing's desk (iOS design §4.2): one code at a time, five minutes, single use, shown inside
//! a QR code with the host's public key, its device id and its addresses; a proof the gateway
//! verified is offered here and the window alone confirms it, which pins the device; a cancel,
//! a withdrawal by the gateway, or a window that ends first, pins nothing. Unpairing removes
//! the record and bumps a generation every live link re-checks its key against.
//!
//! **One try per code:** [`Desk::take`] hands the live window out and spends it in one act, so
//! two links finishing their handshakes inside one window cannot both get a try.
//!
//! **A confirmation is atomic against the gateway's deadline:** [`Desk::confirm`] takes the
//! offer off the desk before it writes the record and answers after, and a wait that reaches
//! its deadline in between keeps waiting for the answer in progress, so the host never pins a
//! key while the phone is told "refused".
//!
//! The desk holds no text of the protocol, and the code is never printed.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::linkkey::LinkKey;
use super::peers::{self, Peer};

/// How long a pairing window lasts.
pub const WINDOW: Duration = Duration::from_secs(5 * 60);

/// What the QR code carries, as `purlis-pair:` followed by compact JSON.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
    pub version: u32,
    pub host_device: String,
    /// The host's public link key, hex.
    pub host_key: String,
    /// Where to reach the host, `host:port` each, LAN and Tailscale.
    pub addresses: Vec<String>,
    /// The one-time code, hex.
    pub code: String,
}

impl std::fmt::Debug for Payload {
    /// The code is a secret for five minutes: never in a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Payload")
            .field("version", &self.version)
            .field("host_device", &self.host_device)
            .field("host_key", &self.host_key)
            .field("addresses", &self.addresses)
            .field("code", &"…")
            .finish()
    }
}

const QR_PREFIX: &str = "purlis-pair:";

impl Payload {
    pub fn to_qr_text(&self) -> String {
        format!(
            "{QR_PREFIX}{}",
            serde_json::to_string(self).expect("plain data")
        )
    }

    pub fn from_qr_text(text: &str) -> Result<Payload, String> {
        let json = text
            .strip_prefix(QR_PREFIX)
            .ok_or_else(|| "not a purlis pairing code".to_owned())?;
        serde_json::from_str(json).map_err(|e| format!("not a purlis pairing code: {e}"))
    }
}

/// A device the gateway verified a proof from, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub device: String,
    pub name: String,
    pub key: [u8; 32],
    pub fingerprint: String,
}

/// One offer's handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Paired { device: String },
    Refused { why: String },
}

struct Live {
    code: String,
    ends: Instant,
}

/// Where an offer stands on the desk.
enum Standing {
    /// Shown to the window, waiting for Pair or Cancel.
    Open,
    /// Pair was pressed and the record is being written: no deadline takes it away now.
    Confirming,
    /// Answered, waiting for the gateway's wait to collect it.
    Answered(Verdict),
}

struct Pending {
    ticket: Ticket,
    offer: Offer,
    /// When the window the offer came in ends: a Pair pressed later pins nothing.
    ends: Instant,
    standing: Standing,
}

#[derive(Default)]
struct State {
    window: Option<Live>,
    host_device: String,
    next_ticket: u64,
    pending: VecDeque<Pending>,
    generation: u64,
}

type OnRevoke = Box<dyn Fn(u64) + Send + Sync>;

/// A confirmation in progress: [`Desk::begin_confirm`]'s word that the offer is the window's
/// now, finished by [`Desk::finish_confirm`].
#[must_use = "a confirmation begun must be finished, or the offer stays on the desk"]
pub struct Confirming {
    ticket: Ticket,
    offer: Offer,
    host_device: String,
}

pub struct Desk {
    config_root: PathBuf,
    state: Mutex<State>,
    answered: Condvar,
    on_revoke: Mutex<Vec<OnRevoke>>,
}

impl Desk {
    pub fn new(config_root: PathBuf) -> Desk {
        Desk {
            config_root,
            state: Mutex::new(State::default()),
            answered: Condvar::new(),
            on_revoke: Mutex::new(vec![]),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Open a window: a fresh code, replacing any earlier one, and the payload for the QR code.
    pub fn open_window(
        &self,
        host: &LinkKey,
        host_device: &str,
        addresses: Vec<String>,
        now: Instant,
    ) -> Payload {
        let mut code = [0u8; 32];
        getrandom::fill(&mut code).expect("the OS generator works");
        let code = super::hex(&code);
        let mut state = self.state();
        state.window = Some(Live {
            code: code.clone(),
            ends: now + WINDOW,
        });
        state.host_device = host_device.to_owned();
        Payload {
            version: 1,
            host_device: host_device.to_owned(),
            host_key: host.public_hex(),
            addresses,
            code,
        }
    }

    /// The live code and when it ends, if a window is open at `now`: for the window to show.
    /// Takes nothing; the gateway's one try is [`Desk::take`].
    pub fn window(&self, now: Instant) -> Option<(String, Instant)> {
        let state = self.state();
        state
            .window
            .as_ref()
            .filter(|live| live.ends > now)
            .map(|live| (live.code.clone(), live.ends))
    }

    /// The live window, spent in the same breath: `None` when there is none, or it has ended,
    /// or another link took it first.
    pub fn take(&self, now: Instant) -> Option<(String, Instant)> {
        let mut state = self.state();
        let live = state.window.take()?;
        (live.ends > now).then_some((live.code, live.ends))
    }

    pub fn close_window(&self) {
        self.state().window = None;
    }

    /// A verified proof, for the window to answer before `ends`.
    pub fn offer(&self, offer: Offer, ends: Instant) -> Ticket {
        let mut state = self.state();
        state.next_ticket += 1;
        let ticket = Ticket(state.next_ticket);
        state.pending.push_back(Pending {
            ticket,
            offer,
            ends,
            standing: Standing::Open,
        });
        ticket
    }

    /// The offer the window should show, oldest first: open, and inside its window.
    pub fn pending(&self) -> Option<(Ticket, Offer)> {
        let now = Instant::now();
        self.state()
            .pending
            .iter()
            .find(|p| matches!(p.standing, Standing::Open) && p.ends > now)
            .map(|p| (p.ticket, p.offer.clone()))
    }

    /// The gateway gave the offer up: it leaves the desk, and a Pair pressed later pins
    /// nothing. An offer already answered is left for the wait to collect.
    pub fn withdraw(&self, ticket: Ticket) {
        let mut state = self.state();
        state
            .pending
            .retain(|p| p.ticket != ticket || !matches!(p.standing, Standing::Open));
        drop(state);
        self.answered.notify_all();
    }

    /// The person pressed Pair: pin the device and answer. [`Desk::begin_confirm`] then
    /// [`Desk::finish_confirm`], in one call.
    pub fn confirm(&self, ticket: Ticket, now_secs: u64) -> Result<Verdict, String> {
        let confirming = self.begin_confirm(ticket)?;
        self.finish_confirm(confirming, now_secs)
    }

    /// Take the offer as the window's: from now on no deadline removes it, and the wait for
    /// its verdict holds until [`Desk::finish_confirm`] answers. Refused when the offer is not
    /// open or its window has ended.
    pub fn begin_confirm(&self, ticket: Ticket) -> Result<Confirming, String> {
        let mut state = self.state();
        let host_device = state.host_device.clone();
        let pending = state
            .pending
            .iter_mut()
            .find(|p| p.ticket == ticket && matches!(p.standing, Standing::Open))
            .ok_or_else(|| "no such pairing is pending".to_owned())?;
        if pending.ends <= Instant::now() {
            pending.standing = Standing::Answered(Verdict::Refused {
                why: "the pairing window ended".into(),
            });
            drop(state);
            self.answered.notify_all();
            return Err("the pairing window ended before Pair was pressed".into());
        }
        pending.standing = Standing::Confirming;
        Ok(Confirming {
            ticket,
            offer: pending.offer.clone(),
            host_device,
        })
    }

    /// Write the record and answer `paired`; a record that cannot be written answers
    /// `refused` and pins nothing.
    pub fn finish_confirm(&self, confirming: Confirming, now_secs: u64) -> Result<Verdict, String> {
        let Confirming {
            ticket,
            offer,
            host_device,
        } = confirming;
        let pinned = peers::add(
            &self.config_root,
            Peer {
                device: offer.device,
                name: offer.name,
                key: offer.key,
                paired: now_secs,
            },
        );
        match pinned {
            Ok(()) => {
                let verdict = Verdict::Paired {
                    device: host_device,
                };
                self.answer(ticket, verdict.clone());
                Ok(verdict)
            }
            Err(why) => {
                self.answer(
                    ticket,
                    Verdict::Refused {
                        why: "the desktop could not keep the pairing".into(),
                    },
                );
                Err(why)
            }
        }
    }

    /// The person pressed Cancel.
    pub fn cancel(&self, ticket: Ticket) -> Verdict {
        let verdict = Verdict::Refused {
            why: "not confirmed on the desktop".into(),
        };
        self.answer(ticket, verdict.clone());
        verdict
    }

    fn answer(&self, ticket: Ticket, verdict: Verdict) {
        let mut state = self.state();
        if let Some(p) = state.pending.iter_mut().find(|p| p.ticket == ticket) {
            p.standing = Standing::Answered(verdict);
        }
        drop(state);
        self.answered.notify_all();
    }

    /// The verdict, once given, taking the offer off the desk.
    pub fn take_verdict(&self, ticket: Ticket) -> Option<Verdict> {
        let mut state = self.state();
        let at = state
            .pending
            .iter()
            .position(|p| p.ticket == ticket && matches!(p.standing, Standing::Answered(_)))?;
        match state.pending.remove(at)?.standing {
            Standing::Answered(verdict) => Some(verdict),
            _ => None,
        }
    }

    /// Wait for the window's answer until `until`; past it, an offer still open is refused and
    /// taken off the desk, while one being confirmed is waited for however long the record
    /// takes. An offer withdrawn meanwhile is refused. Blocking: the app calls it off its
    /// runtime.
    pub fn wait_verdict(&self, ticket: Ticket, until: Instant) -> Verdict {
        let mut state = self.state();
        loop {
            let at = state.pending.iter().position(|p| p.ticket == ticket);
            let Some(at) = at else {
                return Verdict::Refused {
                    why: "the pairing was withdrawn".into(),
                };
            };
            let now = Instant::now();
            match &state.pending[at].standing {
                Standing::Answered(_) => {
                    return match state.pending.remove(at).map(|p| p.standing) {
                        Some(Standing::Answered(verdict)) => verdict,
                        _ => Verdict::Refused { why: "gone".into() },
                    };
                }
                Standing::Open if now >= until => {
                    state.pending.remove(at);
                    return Verdict::Refused {
                        why: "the pairing window ended before the desktop answered".into(),
                    };
                }
                Standing::Open => {
                    let (next, _) = self
                        .answered
                        .wait_timeout(state, until - now)
                        .unwrap_or_else(|p| p.into_inner());
                    state = next;
                }
                Standing::Confirming => {
                    state = self.answered.wait(state).unwrap_or_else(|p| p.into_inner());
                }
            }
        }
    }

    /// Remove `device`'s record and tell every live link to re-check its key.
    pub fn unpair(&self, device: &str) -> Result<Option<Peer>, String> {
        let removed = peers::remove(&self.config_root, device)?;
        if removed.is_some() {
            let generation = {
                let mut state = self.state();
                state.generation += 1;
                state.generation
            };
            for f in self
                .on_revoke
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .iter()
            {
                f(generation);
            }
        }
        Ok(removed)
    }

    pub fn generation(&self) -> u64 {
        self.state().generation
    }

    pub fn on_revoke(&self, f: OnRevoke) {
        self.on_revoke
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(f);
    }
}
