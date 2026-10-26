// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner chat's declared tool, handed its vault values from this desktop** (#2047, spec
//! #2041 stories 14 and 21, ADR 0078 §7).
//!
//! A runner holds no vault. When a chat on a pool's runner runs one of its persona's declared
//! tools through `purlis secret exec`, the runner's host checks the command against the tools
//! its own start armed, and hands it on with that tool's name. The window then decides, here,
//! whether this desktop sends the tool's values for that one run, each checked before anyone is
//! asked:
//!
//! - **the chat is one this window has open, started in its sandbox, as a persona**: a runner
//!   chat started without the sandbox is sent nothing (D-2050-11's reason);
//! - **the runner's host says which tool it runs**: a runner of a purlis older than this never
//!   does, and is sent nothing;
//! - **that tool is the persona's, declared and allowed on this machine, asked for with exactly
//!   its vault and hand-over**, and the vault is one the persona may use here
//!   (`purlis_core::secrets::brokered::authorise`), as for a chat of this machine;
//! - **the runner is shown to be the person's own** (`purlis_core::poollogins::Bound::read`,
//!   D-2050-4): until a pool can be marked owned (#2561), a Docker pool whose engine is a local
//!   socket on this machine. A runner not shown to be owned is never sent a value;
//! - **the person allowed that tool of that persona on that pool**, as the pool and its runner are
//!   now ([`PoolTools`]), kept in memory while purlis runs, as a browser login's opt-in is. The
//!   first run asks, as one Ask in the Inbox, **and waits for the answer** at most
//!   [`AN_ANSWER_IS_WAITED_FOR`]: the command completes once the person allows it. A command that
//!   waited for this desktop on its runner is asked about afresh.
//!
//! Then the values of exactly the tool's keys are read here, from the vault, and sent to the
//! runner's host for that one run (`purlis_core::hookwire::relay::hand_values`), which runs the
//! tool and keeps none.

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};
use std::time::Duration;

use purlis_core::poollogins::{Bound, OptIns, Standing};
use purlis_core::secrets::brokered::{self, Frame, Handed};
use purlis_core::secrets::{Ctx, Env};

/// How long a runner chat's command waits for the person to answer its Ask: as long as it waits
/// for this desktop to come back.
pub const AN_ANSWER_IS_WAITED_FOR: Duration = purlis_core::hookwire::relay::A_LOGIN_WAITS_AT_MOST;

/// **A runner chat's declared tool that waits on the person's Allow** for its persona on its
/// pool: what its Ask says and its answer binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolToolWaits {
    pub session: u32,
    pub persona: String,
    /// The pool as it is defined now: what an Allow is bound to.
    pub bound: Bound,
    /// The tool's name, and the tool whole as its Allow on this machine reads it.
    pub tool: String,
    pub whole: String,
    /// The vault its values come from.
    pub vault: String,
    /// When it began to wait, in seconds since 1970.
    pub since: u64,
    /// The person answered before, for the pool as it was: it or its runner changed since.
    pub changed: bool,
}

/// One Ask and every command that waits on it, each by its own number.
struct Waiting {
    waits: PoolToolWaits,
    answered: Vec<(u64, mpsc::Sender<bool>)>,
}

/// **The person's Allows of a persona's tools on a pool, for this run of purlis, and the runner
/// chats' commands that wait on one.**
#[derive(Default)]
pub struct PoolTools {
    allowed: OptIns,
    asks: Mutex<HashMap<(u32, String), Waiting>>,
    /// What numbers each command that waits.
    next: std::sync::atomic::AtomicU64,
}

/// Where a runner chat's tool stands on its pool, as [`PoolTools::ask`] answers it.
#[derive(Debug)]
pub enum Asked {
    /// Allowed for the pool as it is now: its values may be sent.
    Allowed,
    /// The person keeps it off there.
    KeptOff,
    /// The person is asked, and the answer comes on this, the command waiting numbered so.
    Waits(mpsc::Receiver<bool>, u64),
}

impl PoolTools {
    fn asks(&self) -> MutexGuard<'_, HashMap<(u32, String), Waiting>> {
        self.asks.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **Where `waits`' tool stands for its persona on its pool as it is now**: allowed, kept
    /// off, or asked of the person, the Ask raised once for the chat's tool; asked afresh,
    /// whatever was answered before, where the command `waited` for this desktop.
    pub fn ask(&self, mut waits: PoolToolWaits, waited: bool) -> Asked {
        match self
            .allowed
            .standing(&waits.persona, &waits.whole, &waits.bound)
        {
            Standing::Allowed if !waited => return Asked::Allowed,
            Standing::KeptOff => return Asked::KeptOff,
            Standing::Changed => waits.changed = true,
            Standing::Allowed | Standing::Unasked => {}
        }
        let (answer, answered) = mpsc::channel();
        let number = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let key = (waits.session, waits.tool.clone());
        self.asks()
            .entry(key)
            .or_insert_with(|| Waiting {
                waits,
                answered: Vec::new(),
            })
            .answered
            .push((number, answer));
        Asked::Waits(answered, number)
    }

    /// **Every runner chat's tool that waits on the person**, longest waiting first.
    pub fn waiting(&self) -> Vec<PoolToolWaits> {
        let mut waits: Vec<PoolToolWaits> =
            self.asks().values().map(|one| one.waits.clone()).collect();
        waits.sort_by(|a, b| (a.since, a.session, &a.tool).cmp(&(b.since, b.session, &b.tool)));
        waits
    }

    /// **The person answers chat `session`'s Ask about its tool `tool`**: `allow` lets that tool
    /// of its persona be sent its values on the pool as the Ask showed it, Keep off keeps it off
    /// there, each until the pool or its runner changes. Every chat of that persona whose same
    /// tool waits on the pool as shown is answered with it. Answers the Asks answered, or none
    /// where it waits no more.
    pub fn answer(&self, session: u32, tool: &str, allow: bool) -> Vec<PoolToolWaits> {
        let mut asks = self.asks();
        let Some(asked) = asks.remove(&(session, tool.to_owned())) else {
            return Vec::new();
        };
        let (persona, whole, bound) = (
            asked.waits.persona.clone(),
            asked.waits.whole.clone(),
            asked.waits.bound.clone(),
        );
        if allow {
            self.allowed.allow(&persona, &whole, bound.clone());
        } else {
            self.allowed.keep_off(&persona, &whole, bound.clone());
        }
        let same: Vec<(u32, String)> = asks
            .iter()
            .filter(|(_, one)| {
                one.waits.persona == persona && one.waits.whole == whole && one.waits.bound == bound
            })
            .map(|(key, _)| key.clone())
            .collect();
        let mut answered = vec![asked];
        answered.extend(same.into_iter().filter_map(|key| asks.remove(&key)));
        answered
            .into_iter()
            .map(|one| {
                for (_, waiter) in one.answered {
                    let _ = waiter.send(allow);
                }
                one.waits
            })
            .collect()
    }

    /// Chat `session` has closed: its Asks go, and what waits on them is answered no.
    pub fn forget(&self, session: u32) {
        self.asks().retain(|(chat, _), _| *chat != session);
    }

    /// The command numbered `number` of chat `session`'s tool `tool` stopped waiting: where
    /// nothing else waits on its Ask, the Ask goes.
    pub fn gave_up(&self, session: u32, tool: &str, number: u64) {
        let mut asks = self.asks();
        let key = (session, tool.to_owned());
        if let Some(one) = asks.get_mut(&key) {
            one.answered.retain(|(waiting, _)| *waiting != number);
            if one.answered.is_empty() {
                asks.remove(&key);
            }
        }
    }
}

/// Writes `frame` to the asker, a line of its own.
fn say(writer: &mut dyn Write, frame: &Frame) {
    if let Ok(line) = serde_json::to_string(frame) {
        let _ = writeln!(writer, "{line}").and_then(|()| writer.flush());
    }
}

/// What the asker of `tool` on pool `pool` is told while the person is asked.
fn asked_said(persona: &str, tool: &str, pool: &str, changed: bool) -> String {
    let why = if changed {
        format!("pool {pool}'s runner changed since the person allowed {persona}'s {tool} there")
    } else {
        format!("this chat runs on pool {pool}'s runner")
    };
    format!(
        "{why}, so purlis asked the person whether {tool} may be sent its vault value there: this \
         command waits for their answer, at most 15 minutes. Run it in the background to carry on \
         meanwhile"
    )
}

/// What the asker is told where the person keeps `persona`'s `tool` off on `pool`.
fn kept_off_said(persona: &str, tool: &str, pool: &str) -> String {
    format!(
        "the person keeps {persona}'s {tool} off on pool {pool}, so it was sent no vault value \
         and did not run: tell the person what needs it, or go on without it"
    )
}

/// What the asker is told where the person did not answer in time, or the chat went.
fn unanswered_said(tool: &str) -> String {
    format!(
        "the person did not answer within 15 minutes whether {tool} may be sent its vault value, \
         so it did not run: run it again when they are back"
    )
}

/// **Runner chat `ask.chat`'s `secret exec`, heard over pool `pool`'s link, in project `held`**
/// (module docs): its values sent to the runner's host for that one run, or the asker told why
/// not. Returns once it is decided.
pub fn run_on_its_runner(
    held: &crate::planes::Held,
    pool: &str,
    ask: brokered::Ask,
    mut writer: Box<dyn Write + Send>,
) {
    let session = ask.chat;
    match decided(held, pool, &ask, &mut *writer) {
        Ok(handed) => match purlis_core::hookwire::relay::hand_values(handed) {
            Ok(()) => tracing::info!(
                "purlis: chat {session} on pool {pool} ran {} with its values sent from this \
                 desktop",
                ask.runs_as.as_deref().unwrap_or_default()
            ),
            Err(_) => brokered::refused(
                writer,
                "this command did not come from a runner purlis can send a value to, so \
                 nothing ran"
                    .to_owned(),
            ),
        },
        Err(why) => {
            tracing::info!("purlis: chat {session} on pool {pool} was sent no vault value");
            brokered::refused(writer, why);
        }
    }
}

/// The values runner chat `ask.chat` is sent for its tool, once every check and the person have
/// let it; or what it is told instead. The asker is told on `writer` while the person is asked.
fn decided(
    held: &crate::planes::Held,
    pool: &str,
    ask: &brokered::Ask,
    writer: &mut dyn Write,
) -> Result<Handed, String> {
    let session = ask.chat;
    let wanted = &ask.secret_exec;
    let (asker, unconfined) = held
        .chats()
        .login_asker(held.root(), session)
        .ok_or_else(|| format!("chat {session} is not one this app has open"))?;
    if unconfined {
        return Err(format!(
            "this chat runs on pool {pool}'s runner without the sandbox, so purlis sends it no \
             vault value: start it in its sandbox to run its persona's tools"
        ));
    }
    let Some(persona) = asker.persona.clone().filter(|persona| !persona.is_empty()) else {
        return Err(
            "this chat runs as no persona, so it has no tools and is sent no vault value"
                .to_owned(),
        );
    };
    let Some(tool) = ask.runs_as.clone() else {
        return Err(format!(
            "the purlis on pool {pool}'s runner does not say which tool this command is, so it \
             is sent no vault value: update purlis on the runner (removing the pool and adding it \
             again makes it afresh)"
        ));
    };
    let root = held.root();
    let tool = allowed_tool(root, &persona, &tool, wanted)?;
    let ctx = Ctx::new(root, Env::from_process());
    brokered::authorise(&ctx, Some(&persona), &wanted.vault)?;
    let config = held.chats().pools_in().ok_or_else(|| {
        format!(
            "purlis keeps no pools on this machine, so {} was sent no value",
            tool.name
        )
    })?;
    // Only a runner shown to be yours is ever asked about (D-2050-4).
    let bound = Bound::read(&config, pool)
        .map_err(|why| format!("{} was sent no value on pool {pool}: {why}", tool.name))?;
    let waits = PoolToolWaits {
        session,
        persona: persona.clone(),
        bound,
        tool: tool.name.clone(),
        whole: tool.whole(),
        vault: tool.vault.clone(),
        since: crate::sandboxing::now_secs(),
        changed: false,
    };
    match held.pool_tools().ask(waits, ask.waited) {
        Asked::Allowed => {}
        Asked::KeptOff => return Err(kept_off_said(&persona, &tool.name, pool)),
        Asked::Waits(answered, number) => {
            let changed = held
                .pool_tools()
                .waiting()
                .iter()
                .any(|one| one.session == session && one.tool == tool.name && one.changed);
            tracing::info!(
                "purlis: chat {session}'s {} on pool {pool} asks the person whether its value \
                 may be sent there",
                tool.name
            );
            held.chats().asks_may_have_moved();
            say(
                writer,
                &Frame::Note(asked_said(&persona, &tool.name, pool, changed)),
            );
            let answer = answered.recv_timeout(AN_ANSWER_IS_WAITED_FOR);
            if answer.is_err() {
                held.pool_tools().gave_up(session, &tool.name, number);
                held.chats().asks_may_have_moved();
            }
            match answer {
                Ok(true) => {}
                Ok(false) => return Err(kept_off_said(&persona, &tool.name, pool)),
                Err(_) => return Err(unanswered_said(&tool.name)),
            }
        }
    }
    read(&ctx, &tool)
}

/// **Tool `name` of `persona`, as this machine allows it**, where `wanted` asks for exactly its
/// vault and hand-over: the persona's tools as the project declares them and this machine
/// allowed them, read now (the runner armed the chat's tools, of which this window holds no
/// record). Refused in the words a chat of this machine is refused in otherwise.
fn allowed_tool(
    root: &std::path::Path,
    persona: &str,
    name: &str,
    wanted: &brokered::Wanted,
) -> Result<purlis_core::sandbox::tools::Tool, String> {
    use purlis_core::sandbox::tools;
    let (declared, allowed) = tools::declared_and_allowed_here(root, persona);
    let armed: Vec<tools::Ready> = allowed
        .into_iter()
        .filter(|tool| tool.login.is_none())
        .map(|tool| tools::Ready {
            tool,
            program: std::path::PathBuf::new(),
        })
        .collect();
    let ready = tools::declared(
        &armed,
        &declared,
        persona,
        &tools::Asked {
            vault: &wanted.vault,
            env: &wanted.env,
            file: &wanted.file,
            dotenv: &wanted.dotenv,
            program: name,
        },
    )?;
    Ok(ready.tool.clone())
}

/// The value of each of `tool`'s keys, read from its vault here.
fn read(ctx: &Ctx, tool: &purlis_core::sandbox::tools::Tool) -> Result<Handed, String> {
    let vault = purlis_core::secrets::cmd::provider(ctx, &tool.vault).map_err(|e| e.message)?;
    let mut values = Vec::new();
    for value in &tool.values {
        if values
            .iter()
            .any(|(key, _): &(String, String)| *key == value.key)
        {
            continue;
        }
        let read =
            purlis_core::secrets::cmd::get_value(ctx, &vault, &value.key).map_err(|e| e.message)?;
        values.push((value.key.clone(), read));
    }
    Ok(Handed::of(values))
}

/// What a pool tool Ask answered by nothing any more says.
const WAITS_NO_MORE: &str = "Nothing is asking about that tool on a pool for that chat any more.";

/// **Allow or Keep off on chat `session`'s Ask about its tool `tool` on its pool** (#2047): with
/// `allow`, this desktop sends that tool of the chat's persona its vault values on the pool's
/// runner, for each run, while the app runs and the pool stays as the Ask showed it; Keep off
/// refuses it there until the pool changes. Every chat of that persona whose same tool waits on
/// that pool is answered with it, and each waiting command goes on or is refused at once.
#[tauri::command]
#[specta::specta]
pub async fn answer_pool_tool(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    tool: String,
    allow: bool,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("answering a pool's tool", move || {
        pool_tool_answered(&held, session, &tool, allow)
    })
    .await
}

/// [`answer_pool_tool`]'s work.
pub fn pool_tool_answered(
    held: &crate::planes::Held,
    session: u32,
    tool: &str,
    allow: bool,
) -> Result<(), String> {
    if held.pool_tools().answer(session, tool, allow).is_empty() {
        return Err(WAITS_NO_MORE.to_owned());
    }
    held.chats().asks_may_have_moved();
    Ok(())
}

#[cfg(test)]
mod tests;
