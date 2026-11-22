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
use super::timings::Timings;

/// Who hears what a confinement's proxy does: the host, which pushes it to the window.
pub type Tell = Arc<dyn Fn(SandboxTold) + Send + Sync + 'static>;

/// Whether a confinement's proxy may hold a connection while the person is asked, now.
pub type Askable = Arc<dyn Fn() -> bool + Send + Sync + 'static>;

/// **Where a confinement's plan was made** (#2445).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Planned {
    /// On this machine, by its own window.
    Here,
    /// On this machine, by the host, for a chat it composes by its names (#2590): what the
    /// window allowed for that chat alone and this machine's plan does not give is left out of
    /// it, and asked about where its proxy asks live.
    Composed,
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
    /// sandbox for the project does not give, so it was started without them (#2510). Where
    /// its proxy asks live, it asks about each, and the person may allow it for this chat.
    left_out: Vec<u16>,
    /// Where its plan was made.
    planned: Planned,
}

/// **Starts the confinement `confine` plans, in the project at `project_path`**, its proxy
/// telling `tell` what it does, and holding a connection while the person is asked only while
/// `askable` says so; a host already allowed in the project's files is found as the window's
/// board finds one. Or why it was not started, in a sentence. `alone` is what the window allowed
/// a chat the host composes for it alone ([`Planned::Composed`]); nothing, for any other. Every
/// timer of the confinement, its board's and its proxy's, waits by `timings` (T1.1-01).
pub fn start(
    project_path: &Path,
    confine: &session::Confine,
    planned: Planned,
    alone: &[u16],
    tell: Tell,
    askable: Askable,
    timings: &Timings,
) -> Result<Hosted, String> {
    starting(
        project_path,
        confine,
        planned,
        alone,
        tell,
        askable,
        timings,
        #[cfg(any(test, feature = "test-support"))]
        None,
    )
}

/// **[`start`], its proxy looking names up and dialling through a scenario's resolver seam**
/// (T1.1-06), a test build's alone: what the scenario host's own `Confine` starts each chat's
/// confinement with, so the session host's proxies are counted as the window's are.
#[cfg(any(test, feature = "test-support"))]
#[allow(clippy::too_many_arguments)]
pub fn start_through(
    project_path: &Path,
    confine: &session::Confine,
    planned: Planned,
    alone: &[u16],
    tell: Tell,
    askable: Askable,
    timings: &Timings,
    names: Arc<super::names::Names>,
) -> Result<Hosted, String> {
    starting(
        project_path,
        confine,
        planned,
        alone,
        tell,
        askable,
        timings,
        Some(names),
    )
}

#[allow(clippy::too_many_arguments)]
fn starting(
    project_path: &Path,
    confine: &session::Confine,
    planned: Planned,
    alone: &[u16],
    tell: Tell,
    askable: Askable,
    timings: &Timings,
    #[cfg(any(test, feature = "test-support"))] names: Option<Arc<super::names::Names>>,
) -> Result<Hosted, String> {
    let planned_as = Plan::from_wire(&confine.plan)?;
    let (plan, left_out) = started_here(planned, planned_as, alone, project_path);
    let root = project_path.to_path_buf();
    // This machine's own locks hold over what the window named (the review of #7105): a family
    // an administrator holds enforced here is never lifted, whoever relayed the chat.
    let locks = super::policy::Locks::of(project_path);
    let board = (confine.asks && hosts_askable(planned, project_path)).then(|| {
        Asking {
            tell: {
                let tell = Arc::clone(&tell);
                Arc::new(move |heard: Heard| {
                    tell(match heard {
                        Heard::Asked(targets) => SandboxTold::Asked { targets },
                        Heard::TimedOut(target) => SandboxTold::TimedOut { target },
                        // So the window audits a waiting once's end (1.88, T1-34, Q278).
                        Heard::UsedNext(target) => SandboxTold::UsedNext { target },
                        Heard::DroppedNext(target) => SandboxTold::DroppedNext { target },
                    });
                })
            },
            askable: Arc::new(move || askable()),
            known: Arc::new(move |host| super::grant::allowed_already(&root, host).map(By::from)),
            // The `asks` family off or logged (Q308), as the window resolved the project's policy
            // for this chat and named it ([`session::Confine::lifted`]): answered Allow, and told
            // to the window, which knows the chat and writes its line (the follow-up, item 3).
            // Only the app resolves the policy: a confinement the window named nothing for, the
            // host's own composed start among them, asks as before.
            answered: lifted(
                &confine.lifted,
                crate::enforcement::Family::Asks,
                &locks,
                &tell,
            ),
        }
        .board(timings)
    });
    let network = lifted(
        &confine.lifted,
        crate::enforcement::Family::Network,
        &locks,
        &tell,
    );
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
        timings: *timings,
        #[cfg(any(test, feature = "test-support"))]
        names,
        network,
    };
    let confinement = match planned {
        Planned::Here => plan.start(telling),
        Planned::Composed => plan.start_leaving_out(telling, left_out.clone()),
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

/// **What a confinement tells of a host it let through only because the window named `family`
/// as not enforced for its chat** (`named`, [`session::Confine::lifted`], 1.94): told to the
/// window as [`SandboxTold::Lifted`]. None, and the family is enforced, where the window did not
/// name it, or where `locks`, this machine's, hold it enforced: a runner applies its own locks
/// over what a desktop relays.
fn lifted(
    named: &[String],
    family: crate::enforcement::Family,
    locks: &super::policy::Locks,
    tell: &Tell,
) -> Option<super::asks::Answered> {
    if !named.iter().any(|named| named == family.key())
        || locks.enforcement_floor(family) == Some(crate::enforcement::Mode::Enforce)
    {
        return None;
    }
    let tell = Arc::clone(tell);
    let family = family.key().to_owned();
    Some(Arc::new(move |host: &str, port: u16| {
        tell(SandboxTold::Lifted {
            family: family.clone(),
            target: egress::host_and_port(host, port),
        });
    }))
}

/// **Whether `wired`, a plan another machine made, fits the project at `project_path` on this
/// machine, the runner** (#2445, [`Plan::fits`]), or why not, in a sentence. Refused where the
/// runner's own addresses cannot be read: it could not tell them from another's.
pub fn fits_here(project_path: &Path, wired: &session::ConfinePlan) -> Result<(), String> {
    // Through the one place purlis asks the system's resolver, so a test build records every
    // name a plan's fit looked up (`sandbox::names::asked_the_system`, T1.1-06).
    let resolve = |name: &str, port: u16| {
        egress::system_resolve(name, port)
            .into_iter()
            .map(|at| at.ip())
            .collect()
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

/// **Whether this machine's sandbox for the project at `project_path` gives a chat the service
/// on `port` now** (#2738), as [`fits_here`] holds a plan's services to it, or why not, in a
/// sentence: what a port withdrawn for want of a window is checked against before it is given
/// back after a re-link.
pub fn gives_here(project_path: &Path, port: u16) -> Result<(), String> {
    if given_here(project_path).contains(&port) {
        Ok(())
    } else {
        Err(format!(
            "this machine's sandbox for the project no longer gives the service on port {port}"
        ))
    }
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

/// **The plan as this machine starts it, and the services it leaves out to be asked about**
/// (#2510, #2590): a plan made elsewhere without what it gives for this chat alone and this
/// machine does not give; one composed here without what the window allowed the chat alone and
/// it does not give; each only as this machine's policy lets one be asked about
/// ([`askable_here`]). A plan the window made here leaves nothing out.
fn started_here(
    planned: Planned,
    planned_as: Plan,
    alone: &[u16],
    project_path: &Path,
) -> (Plan, Vec<u16>) {
    // A plan made elsewhere is started without what it gives for this chat alone and this
    // machine does not give (#2510); one composed here, without what the window allowed for
    // the chat alone and it does not give (#2590).
    match planned {
        Planned::Here => (planned_as, Vec::new()),
        Planned::Composed => {
            let left_out = left_out_of(&planned_as, alone);
            (planned_as, askable_here(project_path, left_out))
        }
        Planned::Elsewhere => {
            let (plan, left_out) = planned_as.as_the_runner_starts_it(&given_here(project_path));
            (plan, askable_here(project_path, left_out))
        }
    }
}

/// **The services left out of a relayed or composed plan that may be asked about here**
/// (#2510, #2590): `left_out`, unless this machine's own administrator's policy
/// (`Locks::of(project_path)`) forbids a chat a service on it, turns the live ask off, or keeps
/// an Allow at neither one-chat scope. Then none is asked about, none is said left out, and the
/// chat does not reach it.
fn askable_here(project_path: &Path, left_out: Vec<u16>) -> Vec<u16> {
    use super::grant::Level;
    let locks = super::policy::Locks::of(project_path);
    let forbidden = locks.port_grants_refused().is_some()
        || locks.forbids_live_asks()
        || (!locks.keeps_scope(Level::Once) && !locks.keeps_scope(Level::Chat));
    if forbidden { Vec::new() } else { left_out }
}

/// **Whether a confinement's proxy may ask the window about a host**: always where the window on
/// this machine planned it, which applied this machine's locks itself; for a plan relayed from
/// another machine or one composed here, only while this machine's own administrator's policy
/// (`Locks::of(project_path)`) leaves the live ask on and keeps an Allow at a one-chat scope,
/// as [`askable_here`] holds a service. Otherwise its proxy refuses an unlisted host and the
/// window is asked nothing.
fn hosts_askable(planned: Planned, project_path: &Path) -> bool {
    use super::grant::Level;
    if planned == Planned::Here {
        return true;
    }
    let locks = super::policy::Locks::of(project_path);
    !locks.forbids_live_asks() && (locks.keeps_scope(Level::Once) || locks.keeps_scope(Level::Chat))
}

/// The services of `alone` that `composed` does not give, once each and in order (#2590).
fn left_out_of(composed: &Plan, alone: &[u16]) -> Vec<u16> {
    let mut left_out: Vec<u16> = alone
        .iter()
        .copied()
        .filter(|port| !composed.given.contains(port))
        .collect();
    left_out.sort_unstable();
    left_out.dedup();
    left_out
}

/// What the window's answer to an ask names ([`answered_about`]).
#[derive(Debug, PartialEq, Eq)]
enum Answered {
    /// A host.
    Host(Host),
    /// The service on this port of this machine, left out of the chat's plan (#2510).
    LeftOut(u16),
}

/// **What the window's answer about `typed` names, in a confinement planned `planned`** (#2510,
/// ruled 2026-10-18). A host off this machine is as it says. A service of this machine's
/// loopback is answered only in a confinement planned on another machine or composed here for a
/// chat started by its names (#2590), and only where it is one the runner `left_out` of what
/// the window allowed for this chat alone: allowed, it is carried for this chat for as long as
/// the confinement runs. Any other is refused in a sentence, as a host of this machine always
/// is.
fn answered_about(planned: Planned, left_out: &[u16], typed: &str) -> Result<Answered, String> {
    match super::localport::loopback_port(typed) {
        Some(port) if planned != Planned::Here && left_out.contains(&port) => {
            Ok(Answered::LeftOut(port))
        }
        Some(port) if planned != Planned::Here => Err(format!(
            "the runner does not let this chat reach port {port} of its own: it was not \
             allowed for this chat alone where the chat was planned"
        )),
        _ => Host::parse(typed).map(Answered::Host),
    }
}

/// **Allow next time · once, and its removal, on the host's `board`** (1.88, T1-34): a host, as
/// the window judged it from the chat's block. The proxy still refuses what it never carries,
/// whatever is allowed. None for any other change.
fn next_changed(board: Option<&Asks>, change: &ReachChange) -> Option<Result<Vec<String>, String>> {
    let (ReachChange::AllowNext { host } | ReachChange::ForgetNext { host }) = change else {
        return None;
    };
    let Some(board) = board else {
        return Some(Err(
            "this chat's proxy asks nobody, so it holds no next attempt".into(),
        ));
    };
    Some(Host::parse(host).map(|host| {
        if matches!(change, ReachChange::AllowNext { .. }) {
            board.allow_next(&host);
        } else {
            board.forget_next(&host);
        }
        Vec::new()
    }))
}

impl Hosted {
    /// How long its board holds a connection while the person is asked (T1.1-06): `None` where
    /// its proxy asks nobody.
    pub fn holds_asks_for(&self) -> Option<std::time::Duration> {
        self.board.as_ref().map(|board| board.holds_for())
    }

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
            // Named only where it left a service out, which the window then says it of.
            host_name: (!self.left_out.is_empty()).then(crate::dispatch::host),
        }
    }

    /// The window's answer to one of its asks, or a port withdrawn: the hosts held connections
    /// gave up on before an Allow, which the chat is told to retry.
    pub fn change(&self, change: &ReachChange) -> Result<Vec<String>, String> {
        if let ReachChange::WithdrawPort { port } = change {
            self.confinement.withdraw_port(*port)?;
            return Ok(Vec::new());
        }
        // While the person is Quiet (#6007): a proxy that asks nobody holds nothing open.
        if let ReachChange::HoldAsksOpen { open } = change {
            if let Some(board) = &self.board {
                board.hold_open(*open);
            }
            return Ok(Vec::new());
        }
        // The chat moved on (R-290): a proxy that asks nobody holds nothing to give up.
        if let ReachChange::GiveUp { target } = change {
            if let Some(board) = &self.board {
                board.give_up(target);
            }
            return Ok(Vec::new());
        }
        if let Some(changed) = next_changed(self.board.as_deref(), change) {
            return changed;
        }
        let Some(board) = &self.board else {
            return Err("this chat's proxy asks nobody, so it holds no ask to answer".into());
        };
        Ok(match change {
            ReachChange::Allow { host, layer } => {
                match answered_about(self.planned, &self.left_out, host)? {
                    Answered::Host(host) => board.allow(&host, by_of(*layer)),
                    // For this chat alone, whatever scope it was sent at.
                    Answered::LeftOut(port) => board.allow_left_out(port),
                }
            }
            ReachChange::Forget { host, layer } => {
                match answered_about(self.planned, &self.left_out, host)? {
                    Answered::Host(host) => board.forget(&host, by_of(*layer)),
                    Answered::LeftOut(port) => board.forget_left_out(port),
                }
                Vec::new()
            }
            ReachChange::KeepBlocked { host } => {
                match answered_about(self.planned, &self.left_out, host)? {
                    Answered::Host(host) => {
                        board.keep_blocked(&host);
                    }
                    Answered::LeftOut(port) => board.keep_left_out_blocked(port),
                }
                Vec::new()
            }
            ReachChange::WithdrawPort { .. }
            | ReachChange::HoldAsksOpen { .. }
            | ReachChange::GiveUp { .. }
            | ReachChange::AllowNext { .. }
            | ReachChange::ForgetNext { .. } => Vec::new(),
        })
    }

    /// **`port` of this machine, withdrawn because no window was left to withdraw it, is
    /// carried again as the plan gave it** (#2625): its window took the chat back.
    pub fn give_back_port(&self, port: u16) -> Result<(), String> {
        self.confinement.give_back_port(port)
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
                // What waits for the chat's next attempt (1.88, T1-34): the Allowed list's.
                next: board.next_attempts(),
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
