// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's confinement as a session host starts it** (#1999, #2217): the plan it was handed,
//! read back exactly and started here with the code the window's own path runs, its proxy
//! telling the host what it does, and its board of live asks answered from the window.
//!
//! A plan made on another machine, as a runner is sent one through the link between hosts, is
//! started only where it fits this one ([`fits_here`], #2445).
//!
//! Both binaries' hosts start a confinement with this: the window's own binary on a desktop,
//! and `purlis serve` on a runner, so a chat's proxy carries, refuses and asks about the same
//! hosts wherever it runs. Each binary hands it to its host through the host crate's `Confine`
//! seam, which links no core.

use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_session_protocol::session::{
    self, Confined, Listed, ReachChange, ReachNow, SandboxTold,
};

use super::Confinement;
use super::asks::{Asks, Heard};
use super::egress;
use super::hosts::Host;
use super::internal::Internal;
use super::plan::{Asking, Here, Plan, Telling};
use super::reach::{By, by_of, layer_of};

/// Who hears what a confinement's proxy does: the host, which pushes it to the window.
pub type Tell = Arc<dyn Fn(SandboxTold) + Send + Sync + 'static>;

/// Whether a confinement's proxy may hold a connection while the person is asked, now.
pub type Askable = Arc<dyn Fn() -> bool + Send + Sync + 'static>;

/// **Where a confinement's plan was made** (#2445).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Planned {
    /// On this machine, by its own window, or by the host for a chat it composes.
    Here,
    /// On another machine, as the desktop's window plans a chat on a runner: it is started only
    /// where it fits this machine ([`fits_here`]), and its proxy reaches an internal address
    /// only where this machine's policy lists it ([`super::internal`]).
    Elsewhere,
}

/// **A confinement a session host started**: its proxy, the folders beside it and its board of
/// live asks, where its proxy asks live. Dropping it ends its proxy and removes its folders.
pub struct Hosted {
    confinement: Confinement,
    board: Option<Arc<Asks>>,
    /// The services of this machine its plan gave the chat for it alone, which this machine's
    /// sandbox for the project does not give, so it was started without them (#2510). Its
    /// proxy asks about each, and the person may allow it for this chat alone.
    left_out: Vec<u16>,
    /// Where its plan was made.
    planned: Planned,
}

/// **Starts the confinement `confine` plans, in the project at `project_path`**, its proxy
/// telling `tell` what it does, and holding a connection while the person is asked only while
/// `askable` says so; a host already allowed in the project's files is found as the window's
/// board finds one. Or why it was not started, in a sentence.
pub fn start(
    project_path: &Path,
    confine: &session::Confine,
    planned: Planned,
    tell: Tell,
    askable: Askable,
) -> Result<Hosted, String> {
    let planned_as = Plan::from_wire(&confine.plan)?;
    // A plan made elsewhere is started without what it gives for this chat alone and this
    // machine does not give (#2510).
    let (plan, left_out) = match planned {
        Planned::Here => (planned_as, Vec::new()),
        Planned::Elsewhere => planned_as.as_the_runner_starts_it(&given_here(project_path)),
    };
    let root = project_path.to_path_buf();
    let board = confine.asks.then(|| {
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
            known: Arc::new(move |host| super::grant::allowed_already(&root, host).map(By::from)),
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
    let confinement = match planned {
        Planned::Here => plan.start(telling),
        Planned::Elsewhere => {
            plan.start_held(telling, Internal::of(project_path), left_out.clone())
        }
    }
    .map_err(|why| why.to_string())?;
    Ok(Hosted {
        confinement,
        board,
        left_out,
        planned,
    })
}

/// **Whether `wired`, a plan another machine made, fits the project at `project_path` on this
/// machine, the runner** (#2445, [`Plan::fits`]), or why not, in a sentence. Refused where the
/// runner's own addresses cannot be read: it could not tell them from another's.
pub fn fits_here(project_path: &Path, wired: &session::ConfinePlan) -> Result<(), String> {
    let resolve = |name: &str, port: u16| {
        (name, port)
            .to_socket_addrs()
            .map(|found| found.map(|at| at.ip()).collect())
            .unwrap_or_default()
    };
    fits_with(
        project_path,
        wired,
        super::hosts::own_networks_read(),
        &resolve,
    )
}

/// [`fits_here`], with this machine's networks `own` as they were read (`None` where they
/// could not be) and names resolved by `resolve`.
pub(crate) fn fits_with(
    project_path: &Path,
    wired: &session::ConfinePlan,
    own: Option<Vec<super::hosts::Network>>,
    resolve: &dyn Fn(&str, u16) -> Vec<std::net::IpAddr>,
) -> Result<(), String> {
    let refused = |why: String| format!("the runner does not start the chat's confinement: {why}");
    let plan = Plan::from_wire(wired).map_err(refused)?;
    let own = own.ok_or_else(|| refused("it could not read its own addresses".into()))?;
    let given = given_here(project_path);
    plan.fits(&Here {
        routes: &super::tunnel::routes_folder(project_path),
        given: &given,
        own: &own,
        internal: &Internal::of(project_path),
        resolve,
    })
    .map_err(refused)
}

/// The services on this machine every chat of the project at `root` is given here: yours for
/// the project on this machine, at the scope policy leaves open, as a start compiles them
/// ([`super::Widened::and_ports`]).
fn given_here(root: &Path) -> Vec<u16> {
    let locks = super::policy::Locks::of(root);
    let yours = if locks.keeps_scope(super::grant::Level::You) {
        super::local::granted_ports(root)
    } else {
        Vec::new()
    };
    super::Widened::default().and_ports(&yours, &locks).ports
}

/// What an Allow the window sent allows ([`allowed_as`]).
#[derive(Debug, PartialEq, Eq)]
enum Allowing {
    /// A host, at the scope the person chose.
    Host(Host, By),
    /// The service on this port of this machine, for this chat alone, while its confinement
    /// runs (#2510).
    ThisMachine(u16),
}

/// **What an Allow of `typed` at `by` allows, in a confinement planned `planned`** (#2510, ruled
/// 2026-10-18). A host off this machine is allowed as it says. A service of this machine's
/// loopback is allowed only in a confinement planned on another machine, and only where it is
/// one the runner `left_out` of what the plan gave for this chat alone: then for this chat, for
/// as long as the confinement runs. Any other is refused in a sentence, as a host of this machine
/// always is.
fn allowed_as(planned: Planned, left_out: &[u16], typed: &str, by: By) -> Result<Allowing, String> {
    match super::localport::loopback_port(typed) {
        Some(port) if planned == Planned::Elsewhere && left_out.contains(&port) => {
            Ok(Allowing::ThisMachine(port))
        }
        Some(port) if planned == Planned::Elsewhere => Err(format!(
            "the runner does not let this chat reach port {port} of its own: it was not \
             allowed for this chat alone where the chat was planned"
        )),
        _ => Host::parse(typed).map(|host| Allowing::Host(host, by)),
    }
}

impl Hosted {
    /// Where it is: its proxy's ports and the folders beside it. The id is the host's to fill.
    pub fn confined(&self) -> Confined {
        let placed = self.confinement.placed();
        let shown = |path: Option<PathBuf>| path.map(|path| path.display().to_string());
        Confined {
            confinement: String::new(),
            http: placed.http,
            socks: placed.socks,
            tmp: shown(placed.tmp),
            routed: shown(placed.routed),
            ssh: shown(placed.ssh),
            left_out: self.left_out.clone(),
        }
    }

    /// The window's answer to one of its asks, or a port withdrawn: the hosts held connections
    /// gave up on before an Allow, which the chat is told to retry.
    pub fn change(&self, change: &ReachChange) -> Result<Vec<String>, String> {
        if let ReachChange::WithdrawPort { port } = change {
            self.confinement.withdraw_port(*port)?;
            return Ok(Vec::new());
        }
        let Some(board) = &self.board else {
            return Err("this chat's proxy asks nobody, so it holds no ask to answer".into());
        };
        Ok(match change {
            ReachChange::Allow { host, layer } => {
                match allowed_as(self.planned, &self.left_out, host, by_of(*layer))? {
                    Allowing::Host(host, by) => board.allow(&host, by),
                    Allowing::ThisMachine(port) => board.allow_this_machine(port),
                }
            }
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

    /// What its board of live asks holds now.
    pub fn now(&self) -> ReachNow {
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

#[cfg(test)]
mod tests;
