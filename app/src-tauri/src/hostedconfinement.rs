// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's confinement in the session host (#1999): its egress proxy, and the folder and ssh
//! route beside it, started and held by the host while the chat runs there.
//!
//! **One plan, one start, wherever it runs.** The window compiles the chat's sandbox as it
//! always has and plans its confinement from it (`purlis_core::sandbox::Applied::plan`). With
//! chats in the window, it starts that plan itself; with chats in the host, it hands the host
//! the plan ([`wire`]), the host reads it back exactly ([`plan_of`]) and starts it with the same
//! core code ([`ConfinesHere`]), so the proxy carries, refuses and asks about the same hosts.
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

use std::path::PathBuf;
use std::sync::Arc;

use purlis_core::sandbox::asks::{self, Asks, Heard};
use purlis_core::sandbox::hosts::Host;
use purlis_core::sandbox::plan::{Asking, Beside, Hearing, Plan, Telling};
use purlis_core::sandbox::reach::{By, Decision, Reach, by_of, layer_of};
use purlis_core::sandbox::{Board, Confinement, HeldElsewhere, Placed, egress};
use purlis_session_protocol::session::{
    self, Answer, Command, ConfinePlan, Confined, Listed, ReachChange, ReachNow, SandboxTold,
};

use crate::hostedsessions::HostLink;

/// **A chat's plan, as the window hands it to the host.**
pub fn wire(plan: &Plan) -> ConfinePlan {
    ConfinePlan {
        listed: plan
            .reach
            .listed
            .iter()
            .map(|(host, by)| Listed {
                host: host.clone(),
                layer: layer_of(*by),
            })
            .collect(),
        never: plan.reach.never.clone(),
        withdrawn: plan.reach.withdrawn.clone(),
        anything: plan.reach.anything,
        tunnel_ports: plan.tunnel_ports.clone(),
        plain_ports: plan.plain_ports.clone(),
        any_port: plan.any_port,
        beside: match &plan.beside {
            Beside::Wrapped => session::Beside::Wrapped,
            Beside::ProxyOnly { routes } => session::Beside::ProxyOnly {
                routes: routes.display().to_string(),
            },
        },
        given: plan.given.clone(),
    }
}

/// **The plan the window handed the host, read back exactly**, or why it does not read: a host
/// policy pins that is not a host is refused, never dropped.
pub fn plan_of(wired: &ConfinePlan) -> Result<Plan, String> {
    let reach = purlis_core::sandbox::reach::Written {
        listed: wired
            .listed
            .iter()
            .map(|listed| (listed.host.clone(), by_of(listed.layer)))
            .collect(),
        never: wired.never.clone(),
        withdrawn: wired.withdrawn.clone(),
        anything: wired.anything,
    };
    // Read once here, so a plan that does not read is refused before anything starts.
    Reach::from_written(&reach)?;
    Ok(Plan {
        reach,
        tunnel_ports: wired.tunnel_ports.clone(),
        plain_ports: wired.plain_ports.clone(),
        any_port: wired.any_port,
        beside: match &wired.beside {
            session::Beside::Wrapped => Beside::Wrapped,
            session::Beside::ProxyOnly { routes } => Beside::ProxyOnly {
                routes: PathBuf::from(routes),
            },
        },
        given: wired.given.clone(),
    })
}

/// **Starts each chat's confinement in the session host** (#1999): the window's binary's
/// [`purlis_host::Confine`], the core's start of the plan the window sent, its proxy telling the
/// window through the host, its board holding a connection only while the host says a window
/// is there to ask, and finding a host already allowed in the project's files as the window's
/// board does.
pub struct ConfinesHere;

impl purlis_host::Confine for ConfinesHere {
    fn confine(
        &self,
        confining: &purlis_host::ToConfine<'_>,
        tell: purlis_host::Tell,
        askable: purlis_host::Askable,
    ) -> Result<Box<dyn purlis_host::Confining>, String> {
        let plan = plan_of(&confining.confine.plan)?;
        let root = confining.project_path.to_path_buf();
        let board = confining.confine.asks.then(|| {
            Asking {
                tell: {
                    let tell = Arc::clone(&tell);
                    Arc::new(move |heard: Heard| {
                        tell(match heard {
                            Heard::Asked(targets) => SandboxTold::Asked { targets },
                            Heard::TimedOut(target) => SandboxTold::TimedOut { target },
                        });
                    })
                },
                askable: Arc::new(move || askable()),
                known: Arc::new(move |host| {
                    purlis_core::sandbox::grant::allowed_already(&root, host).map(By::from)
                }),
            }
            .board()
        });
        let telling = Telling {
            refusals: egress::Refusals::telling({
                let tell = Arc::clone(&tell);
                Arc::new(move |host: &str, port: u16| {
                    tell(SandboxTold::Refused {
                        host: host.to_owned(),
                        port,
                    });
                })
            })
            .telling_local({
                let tell = Arc::clone(&tell);
                Arc::new(move |host: &str, port: u16| {
                    tell(SandboxTold::RefusedLocal {
                        host: host.to_owned(),
                        port,
                    });
                })
            })
            .telling_more({
                let tell = Arc::clone(&tell);
                Arc::new(move |count: usize| {
                    tell(SandboxTold::RefusedMore {
                        count: u64::try_from(count).unwrap_or(u64::MAX),
                    });
                })
            }),
            reached: Some(Arc::new(
                move |target: Option<&str>, by: &'static str, times: u64| {
                    tell(SandboxTold::Reached {
                        target: target.map(str::to_owned),
                        by: by.to_owned(),
                        times,
                    });
                },
            )),
            asks: board.clone(),
        };
        let confinement = plan.start(telling).map_err(|why| why.to_string())?;
        Ok(Box::new(HeldHere { confinement, board }))
    }
}

/// A confinement the host holds: its proxy and folders end as this is dropped.
struct HeldHere {
    confinement: Confinement,
    board: Option<Arc<Asks>>,
}

impl purlis_host::Confining for HeldHere {
    fn confined(&self) -> Confined {
        let placed = self.confinement.placed();
        let shown = |path: Option<PathBuf>| path.map(|path| path.display().to_string());
        Confined {
            confinement: String::new(),
            http: placed.http,
            socks: placed.socks,
            tmp: shown(placed.tmp),
            routed: shown(placed.routed),
            ssh: shown(placed.ssh),
        }
    }

    fn change(&self, change: &ReachChange) -> Result<Vec<String>, String> {
        if let ReachChange::WithdrawPort { port } = change {
            self.confinement.withdraw_port(*port)?;
            return Ok(Vec::new());
        }
        let Some(board) = &self.board else {
            return Err("this chat's proxy asks nobody, so it holds no ask to answer".into());
        };
        Ok(match change {
            ReachChange::Allow { host, layer } => board.allow(&Host::parse(host)?, by_of(*layer)),
            ReachChange::Forget { host, layer } => {
                board.forget(&Host::parse(host)?, by_of(*layer));
                Vec::new()
            }
            ReachChange::KeepBlocked { host } => {
                board.keep_blocked(&Host::parse(host)?);
                Vec::new()
            }
            ReachChange::WithdrawPort { .. } => Vec::new(),
        })
    }

    fn now(&self) -> ReachNow {
        self.board
            .as_ref()
            .map(|board| ReachNow {
                asked: board.asked(),
                allowed_live: board
                    .allowed_live()
                    .into_iter()
                    .map(|(host, by)| Listed {
                        host,
                        layer: layer_of(by),
                    })
                    .collect(),
            })
            .unwrap_or_default()
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

    fn keep_blocked(&self, host: &Host) {
        self.change(ReachChange::KeepBlocked {
            host: host.to_string(),
        });
    }

    fn forget(&self, host: &Host, by: By) {
        self.change(ReachChange::Forget {
            host: host.to_string(),
            layer: layer_of(by),
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

/// Where a confinement the host started is, as the core places one.
pub fn placed(confined: &Confined) -> Placed {
    Placed {
        http: confined.http,
        socks: confined.socks,
        tmp: confined.tmp.as_ref().map(PathBuf::from),
        routed: confined.routed.as_ref().map(PathBuf::from),
        ssh: confined.ssh.as_ref().map(PathBuf::from),
    }
}

#[cfg(test)]
mod tests;
