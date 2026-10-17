// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A host that relays to another host (ADR 0078 §4: *"The window reaches a runner through its
//! own `local-ui` connection to its own host, which relays"*, #2042).
//!
//! **Every client link gets a link of its own to the other host**, dialed with the client's
//! first command and ended with the client's link. Each command is sent on as it came, and
//! answered with the other host's answer, or its refusal, word for word. Each view's stream the
//! other host opens is joined to a stream on the client's link, byte for byte both ways
//! (header, snapshot, live bytes one way, acknowledgements the other), so the client draws and
//! acknowledges it as one the host opened itself. What the other host pushes reaches the
//! client once it subscribes.
//!
//! **What only the window may send, the relay sends on for the window alone** (ruled
//! 2026-10-15, #2042): holding a project, dealing its numbers, a start with the window's own
//! `launch`, the window's reply to a line a chat's hooks wrote, and a chat's confinement (its
//! start, the window's answers to its asks, a port withdrawn from it, #2217, and a start that
//! names the confinement the other host composes for it, #2407) cross only for a
//! `local-ui` client, whatever else this host's grants let a client send; and what only the
//! window hears (a held project's hook lines, how saving it goes, what a chat's sandbox did)
//! reaches only a `local-ui` client. This is the one place that rule is enforced: the other
//! host cannot tell a relayed command from one its peer composed, and trusts its peer, this
//! host, as it trusts the window. A start marked as opting out of the sandbox, and a launch
//! that names a harness, are refused by the other host from any scope but its own window's, so
//! one sent on is refused there: a runner composes its harness chats by their names, with its
//! own sandbox. Everything else the other host decides: it serves its link
//! as the scope that link was admitted as, `remote-link` for the link between hosts, and its
//! own check of that scope's grants (FD-27) refuses what the scope may not send, before its
//! host hears of it; the client is told that refusal.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::grants::Power;
use purlis_session_protocol::link::{Acceptor, Link, Opener};
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Events, Peer, Pushed, Refusal, Since, Told,
};
use tokio::sync::{OnceCell, mpsc};
use tokio::task::AbortHandle;

/// How a relay reaches the other host: a new link to it, or why there is none.
pub type Dial = Arc<dyn Fn() -> BoxFuture<'static, Result<Link, String>> + Send + Sync>;

/// The most pushed events the relay holds for a client that has not read them.
const MOST_PUSHED_QUEUED: usize = 1024;

/// Each client link's link to the other host, once dialed, by the client link's number.
type Links = Arc<Mutex<HashMap<u64, Arc<OnceCell<Arc<Relayed>>>>>>;

/// Relays every client link's commands to the other host that `dial` reaches. A clone is the
/// same relay.
#[derive(Clone)]
pub struct Relay {
    dial: Dial,
    links: Links,
}

/// One client link's link to the other host.
struct Relayed {
    client: Client,
    /// Where what the other host pushes goes: the client's subscription, while it has one.
    subscriber: Arc<Mutex<Option<mpsc::Sender<Pushed>>>>,
    tasks: Vec<AbortHandle>,
}

impl Drop for Relayed {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl Relay {
    pub fn new(dial: Dial) -> Relay {
        Relay {
            dial,
            links: Arc::default(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Arc<OnceCell<Arc<Relayed>>>>> {
        self.links
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The link to the other host for `peer`'s link, dialed with its first command. A link that
    /// has ended gets none: a call it sent that is carried out after its end dials nothing, and
    /// leaves nothing behind.
    async fn relayed(&self, peer: &Peer) -> Result<Arc<Relayed>, Refusal> {
        let ended = || Refusal {
            code: session::NOT_STARTED.into(),
            why: "the link this was sent on has ended".into(),
        };
        if peer.has_ended() {
            return Err(ended());
        }
        let cell = Arc::clone(self.lock().entry(peer.link()).or_default());
        let relayed = cell
            .get_or_try_init(|| async {
                let link = (self.dial)().await.map_err(|why| Refusal {
                    code: session::NOT_STARTED.into(),
                    why: format!("the host this one relays to could not be reached: {why}"),
                })?;
                Ok(Arc::new(relay(link, peer.opener().clone(), peer.scope())))
            })
            .await
            .cloned()?;
        // The link ended while this one dialed: what it made goes with it.
        if peer.has_ended() {
            self.lock().remove(&peer.link());
            return Err(ended());
        }
        Ok(relayed)
    }
}

/// The link to the other host for one client link, whose client's link is of `scope`.
fn relay(link: Link, to_the_client: Opener, scope: Scope) -> Relayed {
    let (client, views, events) = Client::new(link);
    let subscriber: Arc<Mutex<Option<mpsc::Sender<Pushed>>>> = Arc::default();
    let joining = tokio::spawn(join_views(views, to_the_client)).abort_handle();
    let pushing = tokio::spawn(push_on(events, Arc::clone(&subscriber), scope)).abort_handle();
    Relayed {
        client,
        subscriber,
        tasks: vec![joining, pushing],
    }
}

/// Joins each view's stream the other host opens to a new stream on the client's link.
async fn join_views(mut views: Acceptor, to_the_client: Opener) {
    while let Ok(mut from_the_other_host) = views.accept().await {
        let to_the_client = to_the_client.clone();
        tokio::spawn(async move {
            if let Ok(mut to_the_client) = to_the_client.open().await {
                let _ = tokio::io::copy_bidirectional(&mut from_the_other_host, &mut to_the_client)
                    .await;
            }
        });
    }
}

/// Hands what the other host pushes to the client's subscription, while it has one: what only
/// the window hears reaches it only where the client's link, of `scope`, is the window's.
async fn push_on(
    mut events: Events,
    subscriber: Arc<Mutex<Option<mpsc::Sender<Pushed>>>>,
    scope: Scope,
) {
    while let Some(told) = events.recv().await {
        let Told::Pushed(pushed) = told else {
            // The relay fell behind the other host: the client's own cursor is what it
            // subscribes again from, and a reader that fell behind is told so by its own link.
            continue;
        };
        if scope != Scope::LocalUi && heard_by_the_window_alone(&pushed) {
            continue;
        }
        let target = subscriber
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(target) = target
            && target.send(pushed).await.is_err()
        {
            *subscriber
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        }
    }
}

/// Whether `command` is one only the window sends: what the relay sends on for a `local-ui`
/// client alone.
fn the_windows_alone(command: &Command) -> bool {
    match command {
        Command::StartProgram(start) => {
            start.launch.is_some() || start.without_sandbox || start.confinement.is_some()
        }
        command => the_windows_power(command.power()),
    }
}

/// Whether `power` is one only the window holds of those the relay meets in a command or a
/// push: holding a project, replying to its hook lines, saving it, and confining a chat.
fn the_windows_power(power: Power) -> bool {
    matches!(
        power,
        Power::Hold | Power::Heard | Power::Save | Power::Confine
    )
}

/// Whether `pushed` is one only the window hears: a held project's hook lines, how saving it
/// goes, and what a chat's sandbox did. The other host pushes them to the relay's link for
/// every client, as it pushes them to a link that may hear them; only a `local-ui` client is
/// handed them.
fn heard_by_the_window_alone(pushed: &Pushed) -> bool {
    pushed.heard_with().is_some_and(the_windows_power)
}

fn refused(why: &session::CallError) -> Refusal {
    why.refusal().cloned().unwrap_or_else(|| Refusal {
        code: session::NOT_STARTED.into(),
        why: format!("the host this one relays to did not answer: {why}"),
    })
}

impl session::Host for Relay {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        if the_windows_alone(&command) && peer.scope() != Scope::LocalUi {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: format!(
                    "`{}` is relayed to another host for purlis's window only, and this link is \
                     `{}`",
                    command.word(),
                    peer.scope()
                ),
            });
        }
        let relayed = self.relayed(peer).await?;
        relayed
            .client
            .call(command)
            .await
            .map_err(|why| refused(&why))
    }

    async fn link_ended(&self, peer: &Peer) {
        self.lock().remove(&peer.link());
    }

    async fn subscribe(
        &self,
        since: Since,
        peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        let relayed = self.relayed(peer).await?;
        let (told, pushed) = mpsc::channel(MOST_PUSHED_QUEUED);
        *relayed
            .subscriber
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(told);
        relayed
            .client
            .subscribe(since)
            .await
            .map_err(|why| refused(&why))?;
        Ok(pushed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_session_protocol::session::{Event, Exit, SandboxTold, ViewEnd};

    /// One of each thing a host pushes.
    fn every_push() -> Vec<Pushed> {
        vec![
            Pushed::Event(Event {
                v: 1,
                device_id: "01J9ZZDEVICE0000000000000".into(),
                seq: 1,
                ulid: "01J9ZZEVENT00000000000000".into(),
                chat: None,
                run: None,
                parent_run: None,
                kind: "hook.stop".into(),
                body: serde_json::Value::Null,
            }),
            Pushed::Missed {
                device: "01J9ZZDEVICE0000000000000".into(),
                after: 1,
                resumes_at: 2,
            },
            Pushed::Ended {
                program: "01J9ZZPRGRM00000000000000".into(),
                run: None,
                exit: Exit::Code(0),
            },
            Pushed::ViewEnded {
                view: 1,
                why: ViewEnd::Closed,
            },
            Pushed::Heard {
                project: "01J9PRJCT00000000000000000".into(),
                heard: 1,
                line: serde_json::Value::Null,
            },
            Pushed::Sandbox {
                confinement: "01J9CNFNMNT0000000000000000".into(),
                told: SandboxTold::RefusedMore { count: 1 },
            },
            Pushed::Saving {
                project: "01J9PRJCT00000000000000000".into(),
                saved: true,
                moved_tree: false,
            },
            Pushed::WhoIsMidTurn {
                question: 1,
                project: "01J9PRJCT00000000000000000".into(),
                workspace: "alpha".into(),
            },
        ]
    }

    /// FD-27's table, as the relay meets it: a push reaches a client of the relay that is not
    /// the window exactly when some scope that is neither the window's nor a link between hosts
    /// may hear it on a host of its own.
    #[test]
    fn what_the_relay_keeps_from_a_client_that_is_not_the_window_is_what_the_table_grants_the_window_alone()
     {
        for pushed in every_push() {
            let anyones = pushed.heard_with().is_none_or(|power| {
                Scope::ALL
                    .into_iter()
                    .filter(|scope| !matches!(scope, Scope::LocalUi | Scope::RemoteLink))
                    .any(|scope| scope.may(power))
            });
            assert_eq!(!heard_by_the_window_alone(&pushed), anyones, "{pushed:?}");
        }
    }
}
