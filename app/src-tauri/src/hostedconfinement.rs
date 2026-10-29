// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's confinement in the session host (#1999): its egress proxy, and the folder and ssh
//! route beside it, started and held by the host while the chat runs there.
//!
//! **One plan, one start, wherever it runs.** The window compiles the chat's sandbox as it
//! always has and plans its confinement from it (`purlis_core::sandbox::Applied::plan`). With
//! chats in the window, it starts that plan itself; with chats in the host, it hands the host
//! the plan ([`wire`]), and the host reads it back exactly and starts it with the same core
//! code (`purlis_core::sandbox::hosted::start`, through [`ConfinesHere`]; `purlis serve` starts
//! one with the same function), so the proxy carries, refuses and asks about the same hosts.
//!
//! **What the proxy does reaches the window by the same closures.** The host's proxy tells the
//! host, which pushes each refusal, connection and ask to the window (`sandbox`, 1.4); the
//! window hands each to what it would have told had the proxy run in it ([`Hears`]). Every
//! answer to an ask is the window's, sent back to the host's board ([`HostBoard`]); nothing a
//! chat sends reaches a board.
//!
//! **Its proxy outlives the window's link.** A confinement a chat runs in is the chat's in the
//! host, so a chat keeps reaching its hosts while the window is gone. The window lets go of it
//! when it closes the chat ([`HeldInTheHost`]).

use std::sync::Arc;

use purlis_core::sandbox::asks::{self, Heard};
use purlis_core::sandbox::hosts::Host;
use purlis_core::sandbox::plan::{Hearing, Plan};
use purlis_core::sandbox::reach::{By, Decision, by_of, layer_of};
use purlis_core::sandbox::{Board, HeldElsewhere, Placed, egress};
use purlis_session_protocol::session::{
    Answer, Command, ConfinePlan, Confined, ReachChange, ReachNow, SandboxTold,
};

use crate::hostedsessions::HostLink;

/// **A chat's plan, as the window hands it to the host** ([`Plan::wired`]).
pub fn wire(plan: &Plan) -> ConfinePlan {
    plan.wired()
}

/// **The plan the window handed the host, read back exactly** ([`Plan::from_wire`]).
pub fn plan_of(wired: &ConfinePlan) -> Result<Plan, String> {
    Plan::from_wire(wired)
}

/// **Starts each chat's confinement in the session host** (#1999): the window's binary's
/// [`purlis_host::Confine`], the core's start of the plan the window sent
/// ([`purlis_core::sandbox::hosted::start`]), the same `purlis serve` starts one with.
pub struct ConfinesHere;

impl purlis_host::Confine for ConfinesHere {
    fn fits(&self, confining: &purlis_host::ToConfine<'_>) -> Result<(), String> {
        purlis_core::sandbox::hosted::fits_here(confining.project_path, &confining.confine.plan)
    }

    fn gives_now(&self, project_path: &std::path::Path, port: u16) -> Result<(), String> {
        purlis_core::sandbox::hosted::gives_here(project_path, port)
    }

    fn confine(
        &self,
        confining: &purlis_host::ToConfine<'_>,
        tell: purlis_host::Tell,
        askable: purlis_host::Askable,
    ) -> Result<Box<dyn purlis_host::Confining>, String> {
        purlis_core::sandbox::hosted::start(
            confining.project_path,
            confining.confine,
            match confining.planned {
                purlis_host::Planned::Here => purlis_core::sandbox::hosted::Planned::Here,
                purlis_host::Planned::Composed => purlis_core::sandbox::hosted::Planned::Composed,
                purlis_host::Planned::Elsewhere => purlis_core::sandbox::hosted::Planned::Elsewhere,
            },
            confining.alone,
            tell,
            askable,
        )
        .map(|hosted| Box::new(HeldHere(hosted)) as Box<dyn purlis_host::Confining>)
    }
}

/// A confinement the host holds: its proxy and folders end as this is dropped.
struct HeldHere(purlis_core::sandbox::hosted::Hosted);

impl purlis_host::Confining for HeldHere {
    fn confined(&self) -> Confined {
        self.0.confined()
    }

    fn change(&self, change: &ReachChange) -> Result<Vec<String>, String> {
        self.0.change(change)
    }

    fn now(&self) -> ReachNow {
        self.0.now()
    }

    fn give_back(&self, port: u16) -> Result<(), String> {
        self.0.give_back_port(port)
    }
}

/// **What the window tells of a proxy the host runs** (#1999): the closures it would have
/// handed a proxy of its own ([`Hearing`]), told as the host pushes each thing the proxy did.
pub struct Hears {
    refusals: egress::Refusals,
    more: Option<egress::MoreTold>,
    reached: Option<egress::Reached>,
    asked: Option<asks::Tell>,
}

impl Hears {
    /// What `hearing` is told by a proxy in the host. Its board is the host's: the window keeps
    /// only who hears an ask.
    pub fn of(hearing: Hearing) -> Hears {
        Hears {
            refusals: hearing.refusals,
            more: hearing.more,
            reached: hearing.reached,
            asked: hearing.asks.map(|asking| asking.tell),
        }
    }

    /// The proxy did `told`.
    pub fn told(&self, told: SandboxTold) {
        match told {
            SandboxTold::Refused { host, port } => self.refusals.heard(&host, port),
            SandboxTold::RefusedLocal { host, port } => self.refusals.heard_local(&host, port),
            SandboxTold::RefusedMore { count } => {
                if let Some(more) = &self.more {
                    more(usize::try_from(count).unwrap_or(usize::MAX));
                }
            }
            SandboxTold::Reached { target, by, times } => {
                // Only a decision's own word: what the record keeps a connection by.
                if let (Some(reached), Some(by)) = (&self.reached, Decision::word_of(&by)) {
                    reached(target.as_deref(), by, times);
                }
            }
            SandboxTold::Asked { targets } => {
                if let Some(asked) = &self.asked {
                    asked(Heard::Asked(targets));
                }
            }
            SandboxTold::TimedOut { target } => {
                if let Some(asked) = &self.asked {
                    asked(Heard::TimedOut(target));
                }
            }
        }
    }
}

/// **A chat's confinement the session host holds, as the window reaches it** (#1999): its
/// board and its ports go over the window's link, and dropping it lets the host end it.
pub struct HeldInTheHost {
    id: String,
    link: Arc<HostLink>,
    board: Option<Arc<dyn Board>>,
}

impl HeldInTheHost {
    /// Confinement `id` in the host `link` reaches, whose proxy asks live where `asks`.
    pub fn new(id: String, link: Arc<HostLink>, asks: bool) -> HeldInTheHost {
        let board = asks.then(|| {
            Arc::new(HostBoard {
                id: id.clone(),
                link: Arc::clone(&link),
            }) as Arc<dyn Board>
        });
        HeldInTheHost { id, link, board }
    }
}

impl std::fmt::Debug for HeldInTheHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeldInTheHost")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl HeldElsewhere for HeldInTheHost {
    fn id(&self) -> &str {
        &self.id
    }

    /// Where the host cannot be told, the chat's whole confinement is let go of, so its proxy
    /// carries nothing rather than a port whose grant was removed; and the window is told.
    fn withdraw_port(&self, port: u16) -> Result<(), String> {
        let Err(why) = change(&self.link, &self.id, ReachChange::WithdrawPort { port }) else {
            return Ok(());
        };
        tracing::warn!("purlis: the session host did not withdraw port {port}: {why}");
        let ended = self.link.call(Command::LetGoOfConfinement {
            confinement: self.id.clone(),
        });
        Err(match ended {
            Ok(_) => format!(
                "purlis could not tell the session host that this chat may no longer reach port \
                 {port} ({why}), so it ended the chat's network: restart the chat to reach what \
                 it may."
            ),
            Err(also) => format!(
                "purlis could not tell the session host that this chat may no longer reach port \
                 {port} ({why}), nor end the chat's network ({also}): close the chat."
            ),
        })
    }

    fn board(&self) -> Option<Arc<dyn Board>> {
        self.board.clone()
    }
}

impl Drop for HeldInTheHost {
    fn drop(&mut self) {
        self.link.unhear(&self.id);
        self.link.send(Command::LetGoOfConfinement {
            confinement: self.id.clone(),
        });
    }
}

/// The board of live asks the session host holds beside a chat's proxy, answered from the
/// window over its link (#1999).
struct HostBoard {
    id: String,
    link: Arc<HostLink>,
}

impl std::fmt::Debug for HostBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostBoard")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl HostBoard {
    fn now(&self) -> ReachNow {
        match self.link.call(Command::ReachOf {
            confinement: self.id.clone(),
        }) {
            Ok(Answer::Reach(now)) => now,
            Ok(other) => {
                tracing::warn!("purlis: the session host answered a board with {other:?}");
                ReachNow::default()
            }
            Err(why) => {
                tracing::warn!("purlis: the session host did not read a chat's board: {why}");
                ReachNow::default()
            }
        }
    }

    fn change(&self, change_to: ReachChange) -> Vec<String> {
        change(&self.link, &self.id, change_to).unwrap_or_else(|why| {
            tracing::warn!("purlis: the session host did not take an answer to an ask: {why}");
            Vec::new()
        })
    }
}

impl Board for HostBoard {
    fn asks_about(&self, host: &Host) -> bool {
        asks::asks_about_among(&self.now().asked, host)
    }

    fn allow(&self, host: &Host, by: By) -> Vec<String> {
        self.change(ReachChange::Allow {
            host: host.to_string(),
            layer: layer_of(by),
        })
    }

    /// Whether the board held a live Allow it ends, as the host's board said before the change:
    /// one read, and none where the host cannot be read, whose proxy is then gone too.
    fn keep_blocked(&self, host: &Host) -> bool {
        let ends = self.allowed_live().iter().any(|(entry, _)| {
            Host::parse(entry).is_ok_and(|allowed| host.covers(&allowed) || allowed.covers(host))
        });
        self.change(ReachChange::KeepBlocked {
            host: host.to_string(),
        });
        ends
    }

    fn forget(&self, host: &Host, by: By) {
        self.change(ReachChange::Forget {
            host: host.to_string(),
            layer: layer_of(by),
        });
    }

    /// Sent as an Allow of the port at the host's loopback, which the host takes only for a
    /// service it left out of the chat's plan (#2510): its refusal, or a link that cannot carry
    /// it, is said, never taken as an Allow.
    fn allow_left_out(&self, port: u16) -> Result<Vec<String>, String> {
        change(
            &self.link,
            &self.id,
            ReachChange::Allow {
                host: left_out_at(port),
                layer: layer_of(By::Chat),
            },
        )
    }

    /// Sent as a Keep blocked of the port at the host's loopback (#2510).
    fn keep_left_out_blocked(&self, port: u16) {
        self.change(ReachChange::KeepBlocked {
            host: left_out_at(port),
        });
    }

    fn allowed_live(&self) -> Vec<(String, By)> {
        self.now()
            .allowed_live
            .into_iter()
            .map(|listed| (listed.host, by_of(listed.layer)))
            .collect()
    }
}

/// How a service on `port` the host left out of a chat's plan is named to it: its loopback
/// address, as its proxy asked about it (#2510).
fn left_out_at(port: u16) -> String {
    format!("{}:{port}", purlis_core::sandbox::tunnel::LOOPBACK)
}

/// Sends `change` to confinement `id`, and waits for the hosts it tells to retry.
fn change(link: &HostLink, id: &str, change: ReachChange) -> Result<Vec<String>, String> {
    match link.call(Command::ChangeReach {
        confinement: id.to_owned(),
        change,
    })? {
        Answer::ReachChanged { retry } => Ok(retry),
        other => Err(format!("the session host answered a change with {other:?}")),
    }
}

/// Where a confinement the host started is, as the core places one ([`Placed::of_wire`]).
pub fn placed(confined: &Confined) -> Placed {
    Placed::of_wire(confined)
}

#[cfg(test)]
mod tests;
