//! The gateway's `Pins`, `PairingDesk` and revocation over core's peers and desk, and its
//! `Heard` into the audit (iOS design §4). Nothing here holds text of the protocol: keys,
//! ids and verdicts only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use purlis_core::remote::pairing::{self, Desk, Offer, Ticket};
use purlis_core::remote::peers;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::gateway::{self, Offered, PairingDesk, Revoked, Window};
use purlis_session_protocol::pairing::{Code, Verdict};
use tokio::sync::watch;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Which keys are paired: `peers.json` under the config root, read at every link, so an
/// unpair holds for the next link the moment it is written.
pub struct Pins {
    config: PathBuf,
}

impl Pins {
    pub fn new(config: PathBuf) -> Pins {
        Pins { config }
    }
}

impl gateway::Pins for Pins {
    fn scope_of(&self, key: &[u8; 32]) -> Option<Scope> {
        peers::scope_of(&self.config, key).and_then(Scope::from_word)
    }
}

/// Core's pairing desk as the gateway's: the window taken once, each offer a ticket the
/// window answers, and the verdict waited for off the runtime.
pub struct DeskAdapter {
    desk: Arc<Desk>,
    /// When the window the last take spent ends: an offer's deadline.
    ends: Mutex<Option<Instant>>,
    /// Each offer's ticket by the device's key, so the gateway can withdraw by the offer.
    tickets: Mutex<HashMap<[u8; 32], Ticket>>,
}

impl DeskAdapter {
    pub fn new(desk: Arc<Desk>) -> DeskAdapter {
        DeskAdapter {
            desk,
            ends: Mutex::new(None),
            tickets: Mutex::new(HashMap::new()),
        }
    }
}

impl PairingDesk for DeskAdapter {
    fn take(&self) -> Option<Window> {
        let (code, ends) = self.desk.take(Instant::now())?;
        let code = Code::from_hex(&code).ok()?;
        *lock(&self.ends) = Some(ends);
        Some(Window { code, ends })
    }

    fn withdraw(&self, offer: &Offered) {
        if let Some(ticket) = lock(&self.tickets).remove(&offer.key) {
            self.desk.withdraw(ticket);
        }
    }

    fn offered(
        &self,
        offer: Offered,
    ) -> Pin<Box<dyn std::future::Future<Output = Verdict> + Send>> {
        let ends = lock(&self.ends).unwrap_or_else(|| Instant::now() + gateway::WINDOW);
        let key = offer.key;
        let ticket = self.desk.offer(
            Offer {
                device: offer.device,
                name: offer.name,
                key,
                fingerprint: offer.fingerprint,
            },
            ends,
        );
        lock(&self.tickets).insert(key, ticket);
        let desk = Arc::clone(&self.desk);
        Box::pin(async move {
            let verdict =
                tokio::task::spawn_blocking(move || desk.wait_verdict(ticket, ends)).await;
            match verdict {
                Ok(pairing::Verdict::Paired { device }) => Verdict::Paired { device },
                Ok(pairing::Verdict::Refused { why }) => Verdict::Refused { why },
                Err(_) => Verdict::Refused {
                    why: "the desk went away".into(),
                },
            }
        })
    }
}

/// The gateway's revocation watch: bumped with the desk's generation at every unpair.
pub fn revocations(desk: &Desk) -> Revoked {
    let (tx, rx) = watch::channel(desk.generation());
    desk.on_revoke(Box::new(move |generation| {
        let _ = tx.send(generation);
    }));
    rx
}

/// The `remote.*` events (iOS design §4.4), through the app's event log where it has one.
pub mod audit {
    use std::path::Path;
    use std::sync::PoisonError;

    use purlis_core::remote::audit::{self, Act};
    use purlis_core::remote::peers;
    use purlis_session_protocol::gateway::Heard;

    use crate::hooks::Events;

    /// Record `act`, or nothing where the app has no event log.
    pub fn act(events: &Option<Events>, act: Act) {
        let Some(events) = events else {
            return;
        };
        let mut recorder = events.lock().unwrap_or_else(PoisonError::into_inner);
        if let Err(why) = audit::record(&mut recorder, act) {
            tracing::warn!("purlis: a remote.* event was not recorded ({why})");
        }
    }

    /// What the gateway heard, as the audit records it: a link opened or refused, a pairing
    /// made or refused. An offer shown and a link ended are not acts of their own.
    pub fn heard(config: &Path, events: &Option<Events>, heard: Heard) {
        let what = match heard {
            Heard::Linked { key, .. } => Act::Linked {
                device: device_of(config, &key),
                key,
            },
            Heard::Refused { key } | Heard::PairRefused { key, .. } => Act::Refused { key },
            Heard::Paired { key, device } => Act::Paired { device, key },
            Heard::PairOffered { .. } | Heard::Unlinked { .. } => return,
        };
        act(events, what);
    }

    /// The device id `peers.json` pairs `key` with, where it does.
    pub fn device_of(config: &Path, key: &[u8; 32]) -> Option<String> {
        peers::read(config)
            .ok()?
            .into_iter()
            .find(|peer| peer.key == *key)
            .map(|peer| peer.device)
    }
}

/// [`audit::device_of`], for a board that must name its phone: `unknown` where the record
/// is gone (an unpair that raced the link; the link closes next).
pub fn device_name(config: &Path, key: &[u8; 32]) -> String {
    audit::device_of(config, key).unwrap_or_else(|| "unknown".into())
}
