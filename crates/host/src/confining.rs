// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's confinement, started and held by the session host (#1999): its egress proxy, and
//! the folder and ssh route beside it, which the chat's network goes through.
//!
//! **The window plans it, the host starts it.** The window compiles a chat's sandbox as it
//! always has, and hands the host the plan of its confinement ([`session::Confine`]): every host
//! its proxy carries, with its layer, and what runs beside it. How a plan becomes a running
//! proxy is the binary's ([`Confine`]), built on the core with the same code the window's own
//! path runs, so a chat's proxy decides alike wherever it runs and this crate links no core.
//!
//! **A plan made on another machine fits this one, or nothing starts** (#2445). A plan the
//! desktop's window sends a runner through the link between hosts is held to the runner's own
//! project before it is started ([`Confine::fits`]): the folders it names are the runner's own
//! for the project, and the services of the runner it gives the chat are ones the runner's own
//! sandbox for the project gives. One that does not fit is refused `not_allowed`. The window's
//! plan on its own host, and a confinement the host plans itself for a chat it composes, are
//! started as planned.
//!
//! **It lives as long as the program in it.** A confinement nothing runs in yet belongs to the
//! link that made it and ends with it; once a start names it, it belongs to that program and
//! ends when the program is closed, let go of, or the host ends. So a chat keeps its network
//! while the window's link is gone.
//!
//! **Only the window hears it and answers it.** What a proxy refused, carried and asks about is
//! pushed only to a link that may confine and that the confinement is named to, and every
//! answer to its asks comes from one (`crate::grants`' row for 1.4, #2444). Its proxy holds a
//! connection while the person is asked only once a program runs in it and such a link is there
//! to ask. A confinement the host composes for a chat is named to the link that started it,
//! where the start asked (1.16, #2407).

use std::path::Path;
use std::sync::Arc;

use purlis_session_protocol::session::{self, Confined, ReachChange, ReachNow, SandboxTold};

/// A confinement to start, as the host hands it to its [`Confine`]: the plan the window sent,
/// and where its project is on this machine, as the host holds it.
pub struct ToConfine<'a> {
    pub project_path: &'a Path,
    pub confine: &'a session::Confine,
}

/// Who hears what a confinement's proxy did: the window, through the host.
pub type Tell = Arc<dyn Fn(SandboxTold) + Send + Sync + 'static>;

/// Whether a confinement's proxy may hold a connection while the person is asked, now.
pub type Askable = Arc<dyn Fn() -> bool + Send + Sync + 'static>;

/// Starts a confinement from a plan: the binary's, built on the core (#1999).
pub trait Confine: Send + Sync + 'static {
    /// Whether the plan `confining` names, made on another machine, fits this one (#2445): every
    /// folder it names is this machine's own for its project, and every service of this machine
    /// it gives the chat is one this machine's own sandbox for the project gives. Or why not, in
    /// a sentence. Asked before [`Self::confine`] of every plan a link other than this
    /// machine's window sends; nothing starts from one that does not fit.
    fn fits(&self, confining: &ToConfine<'_>) -> Result<(), String>;

    /// The confinement `confining` plans, telling `tell` what its proxy does and holding a
    /// connection while the person is asked only while `askable` says so; or why it cannot be
    /// started, in a sentence.
    fn confine(
        &self,
        confining: &ToConfine<'_>,
        tell: Tell,
        askable: Askable,
    ) -> Result<Box<dyn Confining>, String>;
}

/// A confinement the host holds. Dropping it ends its proxy and removes its folders; the host
/// drops it off its runtime.
pub trait Confining: Send + Sync + 'static {
    /// Where it is: its proxy's ports and the folders beside it.
    fn confined(&self) -> Confined;
    /// The window's answer to one of its asks, or a port withdrawn: the hosts held connections
    /// gave up on before an Allow, which the chat is told to retry.
    fn change(&self, change: &ReachChange) -> Result<Vec<String>, String>;
    /// What its board of live asks holds now.
    fn now(&self) -> ReachNow;
}

/// A host whose binary starts no confinement: every one is refused with a sentence.
pub struct ConfinesNothing;

impl Confine for ConfinesNothing {
    fn fits(&self, _confining: &ToConfine<'_>) -> Result<(), String> {
        Err("this session host was given no way to start a chat's confinement".into())
    }

    fn confine(
        &self,
        _confining: &ToConfine<'_>,
        _tell: Tell,
        _askable: Askable,
    ) -> Result<Box<dyn Confining>, String> {
        Err("this session host was given no way to start a chat's confinement".into())
    }
}
