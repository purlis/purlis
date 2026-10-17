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
use super::plan::{Asking, Here, Plan, Telling};
use super::reach::{By, by_of, layer_of};

/// Who hears what a confinement's proxy does: the host, which pushes it to the window.
pub type Tell = Arc<dyn Fn(SandboxTold) + Send + Sync + 'static>;

/// Whether a confinement's proxy may hold a connection while the person is asked, now.
pub type Askable = Arc<dyn Fn() -> bool + Send + Sync + 'static>;

/// **A confinement a session host started**: its proxy, the folders beside it and its board of
/// live asks, where its proxy asks live. Dropping it ends its proxy and removes its folders.
pub struct Hosted {
    confinement: Confinement,
    board: Option<Arc<Asks>>,
}

/// **Starts the confinement `confine` plans, in the project at `project_path`**, its proxy
/// telling `tell` what it does, and holding a connection while the person is asked only while
/// `askable` says so; a host already allowed in the project's files is found as the window's
/// board finds one. Or why it was not started, in a sentence.
pub fn start(
    project_path: &Path,
    confine: &session::Confine,
    tell: Tell,
    askable: Askable,
) -> Result<Hosted, String> {
    let plan = Plan::from_wire(&confine.plan)?;
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
    let confinement = plan.start(telling).map_err(|why| why.to_string())?;
    Ok(Hosted { confinement, board })
}

/// **Whether `wired`, a plan another machine made, fits the project at `project_path` on this
/// machine** (#2445), or why not, in a sentence: its ssh route goes in this machine's own folder
/// for the project ([`super::tunnel::routes_folder`]), and every service on this machine it
/// gives the chat is one this machine's sandbox for the project gives a chat started here,
/// with no grant of another machine's.
pub fn fits_here(project_path: &Path, wired: &session::ConfinePlan) -> Result<(), String> {
    let plan = Plan::from_wire(wired)?;
    let given = given_here(project_path);
    let own = super::hosts::own_addresses();
    plan.fits(&Here {
        routes: &super::tunnel::routes_folder(project_path),
        given: &given,
        own: &own,
    })
    .map_err(|why| format!("this machine does not start the chat's confinement as planned: {why}"))
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
