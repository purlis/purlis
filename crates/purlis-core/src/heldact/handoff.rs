// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app's side of the proxy's hand-off** (one login F2; ADR 0128 D8.1): [`Handoff`] is the
//! [`HeldSink`] the app gives the forge proxy. A request the proxy refused and [`super::recognise`]
//! read as a held act arrives here as a [`Requested`]; it is asked of [`HeldActs`] with what the
//! app reads from the forge ([`HeldContext::world`]), and the chat's answer is the sentence of
//! [`super::said`] **with the ask named**: the chat can say which ask it is waiting on.
//!
//! A merge is the merge hold's ([`crate::mergehold`]): the context raises it and says what the
//! chat is told.

use std::sync::Arc;

use super::{
    Asked, HeldActs, HeldForge, Recorded, Refused, Requested, Trees, WorkflowCredential, World,
    said,
};
use crate::forge::proxy::{ChatId, HeldSink};
use crate::mergehold::{Repo, Request};

/// The forge and the clone a held act of one repo reads, owned for one ask.
pub struct OwnedWorld {
    pub forge: Arc<dyn HeldForge>,
    pub trees: Arc<dyn Trees>,
    pub workflow: Arc<dyn WorkflowCredential>,
    pub recorded: Arc<dyn Recorded>,
}

impl OwnedWorld {
    /// What an ask and a press borrow.
    pub fn as_world(&self) -> World<'_> {
        World {
            forge: self.forge.as_ref(),
            trees: self.trees.as_ref(),
            workflow: self.workflow.as_ref(),
            recorded: self.recorded.as_ref(),
        }
    }
}

/// **What the app knows that a held act's ask needs**: which chat is which session, whether the
/// person is at a desktop, the time, and the forge and clone of a repo.
pub trait HeldContext: Send + Sync {
    /// The session number of chat `chat`, while it is open.
    fn session(&self, chat: &ChatId) -> Option<u32>;
    /// Whether a person is at a connected desktop who can answer.
    fn attended(&self) -> bool;
    /// Seconds since the epoch.
    fn now(&self) -> u64;
    /// The forge (as the person), the clone's trees and the recorded repos for `repo`, for chat
    /// `session`: read at the ask, and again at each press.
    fn world(&self, session: u32, repo: &Repo) -> Result<OwnedWorld, String>;
    /// A merge of `request`: raised on the merge hold, and the sentence the chat is told.
    fn merge(&self, chat: &ChatId, request: &Request) -> String;
}

/// The proxy's [`HeldSink`], over the app's [`HeldActs`].
pub struct Handoff {
    held: Arc<HeldActs>,
    context: Arc<dyn HeldContext>,
}

impl Handoff {
    pub fn new(held: Arc<HeldActs>, context: Arc<dyn HeldContext>) -> Handoff {
        Handoff { held, context }
    }
}

/// The repo a request is of.
fn repo_of(requested: &Requested) -> Option<&Repo> {
    match requested {
        Requested::Merge(request) => request.repo.as_ref(),
        Requested::Tag { repo, .. }
        | Requested::Release { repo, .. }
        | Requested::CiPush { repo, .. } => Some(repo),
        Requested::Fork { source } => Some(source),
    }
}

impl HeldSink for Handoff {
    fn ask(&self, chat: &ChatId, requested: &Requested) -> String {
        if let Requested::Merge(request) = requested {
            return self.context.merge(chat, request);
        }
        let not_asked = |why: Refused| said::not_asked(requested, &why);
        let Some(session) = self.context.session(chat) else {
            return not_asked(Refused::Forge("this chat is not open".to_owned()));
        };
        let Some(repo) = repo_of(requested) else {
            return not_asked(Refused::Forge("the request names no repo".to_owned()));
        };
        let world = match self.context.world(session, repo) {
            Ok(world) => world,
            Err(why) => return not_asked(Refused::Forge(why)),
        };
        let asked = self.held.ask(
            &world.as_world(),
            session,
            self.context.attended(),
            requested,
            self.context.now(),
        );
        match asked {
            Asked::Held { id, .. } => {
                let naming = self
                    .held
                    .shown(id)
                    .map(|(shown, _)| format!(" {}", said::naming(id, &shown)))
                    .unwrap_or_default();
                format!("{}{naming}", said::asked(requested))
            }
            Asked::Refused(why) => not_asked(why),
            Asked::TooMany => not_asked(Refused::Text(
                "this chat already has as many held acts waiting as one chat may have",
            )),
        }
    }
}
