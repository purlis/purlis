// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's own world in each frame it reads or acts on (#2138): its constant scripts run
//! there, apart from the page's own scripts, so what they read and do is the document's own.
//!
//! A world is used only as the engine told of it: its creation names it, its frame and its id
//! unique across every process, and every script names the world by that unique id. A world is
//! forgotten when the engine says it ended, when its frame loads another document or goes, and
//! when its session ends; a call into a world the engine no longer has is refused, and a new
//! world is made.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{Browser, lock};
use crate::cdp::CallError;

/// What purlis's worlds are called in the engine.
pub(super) const WORLD_NAME: &str = "purlis";
/// The most worlds remembered; past it every one is forgotten, and made again when next used.
const WORLDS_KEPT: usize = 256;
/// How long a world just made waits for the engine to tell of it.
const WORLD_TOLD_WITHIN: Duration = Duration::from_secs(2);

/// One world of purlis's own, as the engine told of it: its id among its process's worlds (what
/// an element is resolved into) and its id unique across every process (what scripts name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct World {
    pub id: i64,
    pub unique: String,
}

/// The worlds the engine told of, by the session that holds the frame and the frame's id; and
/// how often every world was forgotten, which names the worlds made after (#2028).
#[derive(Default)]
pub(crate) struct Worlds(
    Mutex<HashMap<(String, String), World>>,
    std::sync::atomic::AtomicU64,
);

/// Whether a world the engine told of, by its name, is one of purlis's own: [`WORLD_NAME`],
/// or that name and a number after every world was forgotten.
pub(super) fn is_ours(name: &str) -> bool {
    name == WORLD_NAME
        || name
            .strip_prefix(WORLD_NAME)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

impl Worlds {
    /// The engine made `context` on `session` (`Runtime.executionContextCreated`): kept when it
    /// is a world of purlis's own.
    pub(super) fn told(&self, session: &str, context: &Value) {
        let aux = &context["auxData"];
        let (Some(id), Some(unique), Some(frame)) = (
            context["id"].as_i64(),
            context["uniqueId"].as_str(),
            aux["frameId"].as_str(),
        ) else {
            return;
        };
        if !context["name"].as_str().is_some_and(is_ours) || aux["type"] != "isolated" {
            return;
        }
        let mut worlds = lock(&self.0);
        if worlds.len() >= WORLDS_KEPT {
            worlds.clear();
        }
        worlds.insert(
            (session.to_owned(), frame.to_owned()),
            World {
                id,
                unique: unique.to_owned(),
            },
        );
    }

    /// The engine ended a context on `session` (`Runtime.executionContextDestroyed`).
    pub(super) fn ended(&self, session: &str, event: &Value) {
        let unique = event["executionContextUniqueId"].as_str();
        let id = event["executionContextId"].as_i64();
        lock(&self.0).retain(|(on, _), world| {
            on != session || (Some(world.unique.as_str()) != unique && Some(world.id) != id)
        });
    }

    /// Every world on `session` is gone: its contexts were cleared, or the session ended.
    pub(super) fn cleared(&self, session: &str) {
        lock(&self.0).retain(|(on, _), _| on != session);
    }

    /// Every world is forgotten: the next use of each frame makes a fresh one, under a name the
    /// engine has not made one by, so it is never handed the world it had (#2028).
    pub(super) fn forget_all(&self) {
        let mut worlds = lock(&self.0);
        worlds.clear();
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// The name the next world is made by.
    pub(super) fn name(&self) -> String {
        match self.1.load(std::sync::atomic::Ordering::SeqCst) {
            0 => WORLD_NAME.to_owned(),
            n => format!("{WORLD_NAME}-{n}"),
        }
    }

    /// `frame` loaded another document, or went: its worlds, on any session, are gone.
    pub(super) fn left(&self, frame: &str) {
        lock(&self.0).retain(|(_, of), _| of != frame);
    }

    fn get(&self, session: &str, frame: &str) -> Option<World> {
        lock(&self.0)
            .get(&(session.to_owned(), frame.to_owned()))
            .cloned()
    }

    fn forget(&self, session: &str, frame: &str, world: &World) {
        let mut worlds = lock(&self.0);
        let key = (session.to_owned(), frame.to_owned());
        if worlds.get(&key) == Some(world) {
            worlds.remove(&key);
        }
    }
}

impl Browser {
    /// Runs `run` with purlis's own world in `frame` (held by `session`), making the world
    /// when there is none. A world the engine no longer has is made again, once.
    pub(super) fn in_world(
        &self,
        session: &str,
        frame: &str,
        run: impl Fn(&World) -> Result<Value, CallError>,
    ) -> Result<Value, CallError> {
        let worlds = &self.inner.worlds;
        if let Some(world) = worlds.get(session, frame) {
            match run(&world) {
                Err(CallError::Engine { .. }) => worlds.forget(session, frame, &world),
                done => return done,
            }
        }
        let world = self.make_world(session, frame, None)?;
        run(&world)
    }

    /// [`Browser::in_world`], a world made for it made by `deadline` (#2753).
    pub(super) fn in_world_by(
        &self,
        session: &str,
        frame: &str,
        deadline: Instant,
        run: impl Fn(&World) -> Result<Value, CallError>,
    ) -> Result<Value, CallError> {
        let worlds = &self.inner.worlds;
        if let Some(world) = worlds.get(session, frame) {
            match run(&world) {
                Err(CallError::Engine { .. }) => worlds.forget(session, frame, &world),
                done => return done,
            }
        }
        let world = self.make_world(session, frame, Some(deadline))?;
        run(&world)
    }

    /// Makes a world of purlis's own in `frame`, and waits for the engine to tell of it, by
    /// `deadline` where one is given.
    fn make_world(
        &self,
        session: &str,
        frame: &str,
        deadline: Option<Instant>,
    ) -> Result<World, CallError> {
        let asked = json!({
            "frameId": frame,
            "worldName": self.inner.worlds.name(),
            "grantUniveralAccess": false,
        });
        let made = match deadline {
            None => self
                .devtools()
                .call("Page.createIsolatedWorld", asked, Some(session))?,
            Some(by) => {
                let left = by.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(CallError::TimedOut);
                }
                self.devtools().call_within(
                    "Page.createIsolatedWorld",
                    asked,
                    Some(session),
                    left,
                )?
            }
        };
        let untold = || CallError::Engine {
            code: 0,
            message: "the engine did not tell of the world it made".into(),
        };
        let id = made
            .get("executionContextId")
            .and_then(Value::as_i64)
            .ok_or_else(untold)?;
        let told_by = Instant::now() + WORLD_TOLD_WITHIN;
        let deadline = deadline.map_or(told_by, |by| by.min(told_by));
        let mut state = lock(&self.inner.state);
        loop {
            if let Some(world) = self.inner.worlds.get(session, frame)
                && world.id == id
            {
                return Ok(world);
            }
            let now = Instant::now();
            if now >= deadline || state.ended {
                return Err(untold());
            }
            state = self
                .inner
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
}
