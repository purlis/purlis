// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's own world in each frame it reads or acts on (#2138): its constant scripts run
//! there, apart from the page's own scripts, so what they read and do is the document's own.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{Value, json};

use super::{Browser, lock};
use crate::cdp::CallError;

/// What purlis's worlds are called in the engine.
const WORLD_NAME: &str = "purlis";
/// The most worlds remembered; past it every one is forgotten, and made again when next used.
const WORLDS_KEPT: usize = 256;

/// The world made in each frame, by the session that holds the frame and the frame's id: its
/// execution context. A world ends with its document; a call into one that has ended is
/// refused by the engine, and a new world is made.
#[derive(Default)]
pub(super) struct Worlds(Mutex<HashMap<(String, String), i64>>);

impl Browser {
    /// Runs `run` with purlis's own world in `frame` (held by `session`), making the world
    /// when there is none. A world the engine no longer has is made again, once.
    pub(super) fn in_world(
        &self,
        session: &str,
        frame: &str,
        run: impl Fn(i64) -> Result<Value, CallError>,
    ) -> Result<Value, CallError> {
        let key = (session.to_owned(), frame.to_owned());
        let kept = lock(&self.worlds.0).get(&key).copied();
        if let Some(world) = kept {
            match run(world) {
                Err(CallError::Engine { .. }) => {
                    let mut worlds = lock(&self.worlds.0);
                    if worlds.get(&key) == Some(&world) {
                        worlds.remove(&key);
                    }
                }
                done => return done,
            }
        }
        let world = self.make_world(session, frame)?;
        let mut worlds = lock(&self.worlds.0);
        if worlds.len() >= WORLDS_KEPT {
            worlds.clear();
        }
        worlds.insert(key, world);
        drop(worlds);
        run(world)
    }

    fn make_world(&self, session: &str, frame: &str) -> Result<i64, CallError> {
        let made = self.devtools().call(
            "Page.createIsolatedWorld",
            json!({ "frameId": frame, "worldName": WORLD_NAME, "grantUniveralAccess": false }),
            Some(session),
        )?;
        made.get("executionContextId")
            .and_then(Value::as_i64)
            .ok_or_else(|| CallError::Engine {
                code: 0,
                message: "the engine made no world".into(),
            })
    }
}
