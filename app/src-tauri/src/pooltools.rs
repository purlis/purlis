// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner chat's declared tool, handed its vault values from this desktop, one run at a
//! time** (#2047, spec #2041 stories 14 and 21, ADR 0078 §7; the person's ruling of 2026-10-26:
//! every run of a tool on a runner asks the person).
//!
//! A runner holds no vault. When a chat on a pool's runner runs one of its persona's declared
//! tools through `purlis secret exec`, the runner's host checks the command against the tools
//! its own start armed, and hands it on with that tool's name and a number of the run's own. The
//! window then decides, here, whether this desktop sends the tool's values for that one run,
//! each checked before anyone is asked ([`checked`]):
//!
//! - **the chat is one this window has open, started in its sandbox, as a persona**: a runner
//!   chat started without the sandbox is sent nothing;
//! - **the runner's host says which tool it runs, and numbered the run**: a runner of a purlis
//!   older than this does neither, and is sent nothing;
//! - **that tool is the persona's, declared and allowed on this machine, asked for with exactly
//!   its vault and hand-over**, and the vault is one the persona may use here, as for a chat of
//!   this machine;
//! - **the runner is shown to be the person's own**
//!   (`purlis_core::poollogins::Bound::over_the_link`, the one check browser logins use,
//!   D-2050-4): until a pool can be marked owned (#2561), a Docker pool whose engine is a local
//!   socket on this machine. A runner not shown to be owned is never sent a value, and neither is
//!   a hosted one. The runner is the one the chat's link reached, as the desktop's host said it
//!   (1.48, #3417): a draining one for a chat that stayed on it, judged by its pool and that
//!   runner as they are kept.
//!
//! Then **the person is asked about this run** ([`PoolTools`]), as one Ask in the Inbox, and the
//! command waits for the answer at most [`purlis_core::hookwire::relay::A_HANDED_LINE_WAITS_AT_MOST`].
//! There is no standing Allow: each run asks. An asker that hangs up meanwhile is told here by
//! its runner, and its Ask goes with it; a chat that closes takes its Asks with it. On Allow the
//! pool and its runner are read again and must be as the Ask showed them; then the values of
//! exactly the tool's keys are read here, from the vault, and sent to the runner's host for that
//! one run (`purlis_core::hookwire::relay::hand_values`), bound to the chat, the tool and the run,
//! which runs the tool and keeps none.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{Duration, Instant};

use purlis_core::hookwire::relay::{A_HANDED_LINE_WAITS_AT_MOST, waits_at_most_said};
use purlis_core::poollogins::Bound;
use purlis_core::sandbox::tools::{self, Tool};
use purlis_core::secrets::brokered::{self, Forwarded, Frame, Handed, Sent};
use purlis_core::secrets::{Ctx, Env, Zeroizing};

/// How often a command that waits on the person looks at its asker.
const LOOKED_AT_EVERY: Duration = Duration::from_millis(200);

/// **One run of a runner chat's declared tool that waits on the person**: what its Ask says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolToolWaits {
    pub session: u32,
    /// This window's number for the run: what its Ask is answered by.
    pub run: u32,
    pub persona: String,
    /// The pool as it is defined now: what the Ask shows, and what must still hold on Allow.
    pub bound: Bound,
    /// The tool's name.
    pub tool: String,
    /// The vault its values come from.
    pub vault: String,
    /// When it began to wait, in seconds since 1970.
    pub since: u64,
    /// It waited on its runner for this desktop to come back.
    pub waited: bool,
}

/// What came of a run's Ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answered {
    Allowed,
    KeptOff,
    /// Its chat closed before the person answered.
    ChatClosed,
}

/// **Every run of a runner chat's tool that waits on the person**, each its own Ask (the
/// person's ruling of 2026-10-26: no standing Allow).
/// Each run that waits, by its chat and its number: what its Ask says, and what it is told.
type Asks = HashMap<(u32, u32), (PoolToolWaits, mpsc::Sender<Answered>)>;

/// The persona's tools as the project declares them, and as this machine allowed them.
pub type ToolsOf<'a> = &'a dyn Fn(&Path, &str) -> (Vec<Tool>, Vec<Tool>);

/// The value of each of a tool's keys, from its vault here, each wiped as it is dropped (#3420).
pub type Reads<'a> = &'a dyn Fn(&Tool) -> Result<Vec<(String, Zeroizing<String>)>, String>;

#[derive(Default)]
pub struct PoolTools {
    asks: Mutex<Asks>,
    next: AtomicU32,
}

impl PoolTools {
    fn asks(&self) -> MutexGuard<'_, Asks> {
        self.asks.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **Asks the person about the run `waits` says** (its `run` is numbered here): what comes
    /// of it arrives on the answer, with the run's number.
    pub fn ask(&self, mut waits: PoolToolWaits) -> (mpsc::Receiver<Answered>, u32) {
        let run = self.next.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        waits.run = run;
        let (answer, answered) = mpsc::channel();
        self.asks().insert((waits.session, run), (waits, answer));
        (answered, run)
    }

    /// **Every run that waits on the person**, longest waiting first.
    pub fn waiting(&self) -> Vec<PoolToolWaits> {
        let mut waits: Vec<PoolToolWaits> = self
            .asks()
            .values()
            .map(|(waits, _)| waits.clone())
            .collect();
        waits.sort_by_key(|one| (one.since, one.session, one.run));
        waits
    }

    /// **The person answers run `run` of chat `session`'s tool**: Allow for that run alone, or
    /// Keep off. The run it answered, or none where it waits no more.
    pub fn answer(&self, session: u32, run: u32, allow: bool) -> Option<PoolToolWaits> {
        let (waits, answer) = self.asks().remove(&(session, run))?;
        let _ = answer.send(if allow {
            Answered::Allowed
        } else {
            Answered::KeptOff
        });
        Some(waits)
    }

    /// Chat `session` has closed: each of its runs that waits is told so, and its Ask goes.
    pub fn forget(&self, session: u32) {
        let mut asks = self.asks();
        let closed: Vec<(u32, u32)> = asks
            .keys()
            .filter(|(chat, _)| *chat == session)
            .copied()
            .collect();
        for key in closed {
            if let Some((_, answer)) = asks.remove(&key) {
                let _ = answer.send(Answered::ChatClosed);
            }
        }
    }

    /// Run `run` of chat `session`'s waits no more: its asker went, or the time is up. Its Ask
    /// goes.
    pub fn gave_up(&self, session: u32, run: u32) {
        self.asks().remove(&(session, run));
    }
}

/// Writes `frame` to the asker, a line of its own.
fn say(writer: &mut dyn Write, frame: &Frame) {
    if let Ok(line) = serde_json::to_string(frame) {
        let _ = writeln!(writer, "{line}").and_then(|()| writer.flush());
    }
}

/// What the asker of `tool` on pool `pool` is told while the person is asked.
fn asked_said(tool: &str, pool: &str) -> String {
    format!(
        "this chat runs on pool {pool}'s runner, so purlis asked the person whether {tool} may be \
         sent its vault value for this run: this command waits for their answer, at most \
         {within}. Run it in the background to carry on meanwhile",
        within = waits_at_most_said()
    )
}

/// What the asker is told where the person keeps this run of `tool` off.
fn kept_off_said(tool: &str) -> String {
    format!(
        "the person kept this run of {tool} off, so it was sent no vault value and did not run: \
         tell the person what needs it, or go on without it"
    )
}

/// What the asker is told where the person did not answer in time.
fn unanswered_said(tool: &str) -> String {
    format!(
        "the person did not answer within {within} whether {tool} may be sent its vault value, \
         so it did not run: run it again when they are back",
        within = waits_at_most_said()
    )
}

/// What the asker is told where its chat closed before the person answered.
fn chat_closed_said(tool: &str) -> String {
    format!("this chat was closed before the person answered, so {tool} did not run")
}

/// What the asker is told where the pool or its runner changed while the person was asked.
fn changed_said(tool: &str, pool: &str) -> String {
    format!(
        "pool {pool} or its runner changed while the person was asked, so {tool} was sent no \
         value and did not run: run it again, and they are asked about the pool as it is now"
    )
}

/// **What [`checked`] reads**: the run's line, this window's record of its chat, and what this
/// machine says, each a seam a test hands in.
pub struct Gate<'a> {
    pub pool: &'a str,
    pub ask: &'a brokered::Ask,
    /// The chat as this window has it open: its persona, and whether it runs with no sandbox.
    /// `None` for a chat this window does not have open.
    pub chat: Option<(Option<String>, bool)>,
    /// The project.
    pub root: &'a Path,
    /// Where this machine keeps its pools, where it keeps any.
    pub pools_in: Option<PathBuf>,
    /// The persona's tools as the project declares them and as this machine allowed them
    /// (`tools::declared_and_allowed_here`).
    pub tools: ToolsOf<'a>,
    /// Whether the persona may use the vault here (`brokered::authorise`).
    pub authorised: &'a dyn Fn(&str, &str) -> Result<(), String>,
    /// The pool as it is now, with the runner the chat's link reached, where that runner is shown
    /// to be the person's own (`Bound::over_the_link`).
    pub owned: &'a dyn Fn(&Path, &str) -> Result<Bound, String>,
}

/// **What passed the gate**: the persona, its tool, the pool as it is now, and the run's number
/// its runner made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub persona: String,
    pub tool: Tool,
    pub bound: Bound,
    pub run: String,
}

/// **The window's gate for one run of a runner chat's tool** (module docs), before anyone is
/// asked: what passed it, or what the asker is told.
pub fn checked(gate: &Gate<'_>) -> Result<Checked, String> {
    let (pool, ask) = (gate.pool, gate.ask);
    let session = ask.chat;
    let wanted = &ask.secret_exec;
    let Some((persona, unconfined)) = &gate.chat else {
        return Err(format!(
            "chat {session} on pool {pool} is not one this app has open, so it is sent no vault \
             value"
        ));
    };
    if *unconfined {
        return Err(format!(
            "this chat runs on pool {pool}'s runner without the sandbox, so purlis sends it no \
             vault value: start it in its sandbox to run its persona's tools"
        ));
    }
    let Some(persona) = persona.clone().filter(|persona| !persona.is_empty()) else {
        return Err(
            "this chat runs as no persona, so it has no tools and is sent no vault value"
                .to_owned(),
        );
    };
    let (Some(name), Some(run)) = (ask.runs_as.clone(), ask.run.clone()) else {
        return Err(format!(
            "the purlis on pool {pool}'s runner does not say which tool and run this command is, \
             so it is sent no vault value: update purlis on the runner (removing the pool and \
             adding it again makes it afresh)"
        ));
    };
    let tool = allowed_tool(gate, &persona, &name, wanted)?;
    (gate.authorised)(&persona, &wanted.vault)?;
    let config = gate.pools_in.as_deref().ok_or_else(|| {
        format!(
            "purlis keeps no pools on this machine, so {} was sent no value",
            tool.name
        )
    })?;
    // Only a runner shown to be yours is ever asked about (D-2050-4).
    let bound = (gate.owned)(config, pool)
        .map_err(|why| format!("{} was sent no value on pool {pool}: {why}", tool.name))?;
    Ok(Checked {
        persona,
        tool,
        bound,
        run,
    })
}

/// **Tool `name` of `persona`, as this machine allows it**, where `wanted` asks for exactly its
/// vault and hand-over: the persona's tools as the project declares them and this machine
/// allowed them, read now (the runner armed the chat's tools, of which this window holds no
/// record). Refused in the words a chat of this machine is refused in otherwise.
fn allowed_tool(
    gate: &Gate<'_>,
    persona: &str,
    name: &str,
    wanted: &brokered::Wanted,
) -> Result<Tool, String> {
    let (declared, allowed) = (gate.tools)(gate.root, persona);
    let armed: Vec<tools::Ready> = allowed
        .into_iter()
        .filter(|tool| tool.login.is_none())
        .map(|tool| tools::Ready {
            tool,
            program: PathBuf::new(),
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

/// **What a run that passed the gate waits on**: the person's answer, whether its asker went,
/// and what reads the values.
pub struct Waiting<'a> {
    pub tools: &'a PoolTools,
    /// Set once the run's asker has gone.
    pub gone: &'a AtomicBool,
    /// Told once the run's Ask is raised or goes.
    pub moved: &'a dyn Fn(),
    /// The value of each of the tool's keys, from its vault here.
    pub read: Reads<'a>,
    /// How long the person is waited for.
    pub within: Duration,
    /// Now, in seconds since 1970.
    pub now: u64,
}

/// **What the person allowed for one run**: its values, and what passed the gate, which this
/// desktop's record of the values it sent names ([`sent`]).
#[derive(Debug)]
pub struct Approved {
    pub handed: Handed,
    pub checked: Checked,
}

/// **Asks the person about one run that passed the gate, and waits for the answer** (module
/// docs): its values on Allow, read once the pool, as `gate` reads it again, is still as the Ask
/// showed it; or what the asker is told. The asker is told on `writer` while the person is asked.
pub fn answered(
    gate: &Gate<'_>,
    checked: Checked,
    waiting: &Waiting<'_>,
    writer: &mut dyn Write,
) -> Result<Approved, String> {
    let ask = gate.ask;
    let (answered, run) = waiting.tools.ask(PoolToolWaits {
        session: ask.chat,
        run: 0,
        persona: checked.persona.clone(),
        bound: checked.bound.clone(),
        tool: checked.tool.name.clone(),
        vault: checked.tool.vault.clone(),
        since: waiting.now,
        waited: ask.waited,
    });
    (waiting.moved)();
    say(
        writer,
        &Frame::Note(asked_said(&checked.tool.name, gate.pool)),
    );
    let began = Instant::now();
    let gave_up = |why: String| {
        waiting.tools.gave_up(ask.chat, run);
        (waiting.moved)();
        Err(why)
    };
    let answer = loop {
        match answered.recv_timeout(LOOKED_AT_EVERY) {
            Ok(answer) => break answer,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Its asker went: nobody waits for the values, and none is read.
                if waiting.gone.load(Ordering::SeqCst) {
                    return gave_up(format!("the command of {} went", checked.tool.name));
                }
                if began.elapsed() >= waiting.within {
                    return gave_up(unanswered_said(&checked.tool.name));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(chat_closed_said(&checked.tool.name));
            }
        }
    };
    match answer {
        Answered::Allowed => {}
        Answered::KeptOff => return Err(kept_off_said(&checked.tool.name)),
        Answered::ChatClosed => return Err(chat_closed_said(&checked.tool.name)),
    }
    if waiting.gone.load(Ordering::SeqCst) {
        return Err(format!("the command of {} went", checked.tool.name));
    }
    // The pool and its runner as the person was shown them, still.
    let now = gate
        .pools_in
        .as_deref()
        .map(|config| (gate.owned)(config, gate.pool));
    if !matches!(&now, Some(Ok(bound)) if *bound == checked.bound) {
        return Err(changed_said(&checked.tool.name, gate.pool));
    }
    let values = (waiting.read)(&checked.tool)?;
    Ok(Approved {
        handed: Handed::of(ask.chat, &checked.tool.name, &checked.run, values),
        checked,
    })
}

/// **Sends the values the person allowed through `hand`, and records on this desktop, in
/// project `root`, that they were sent** (#3406, ADR 0078 §10): `secret.forwarded`, naming the
/// vault, the keys, the runner's device id, the pool, the chat, its persona, the tool and the
/// request, and never a value. Written once `hand` has handed them to the link, measured to fit
/// a frame ([`hand_on_the_link`]), so a run whose values were not handed on is never recorded as
/// sent; a reply the session host refuses after that is told only in its log, so it may
/// over-record (D-3406-2).
pub fn sent(
    root: &Path,
    approved: Approved,
    hand: impl FnOnce(Handed) -> Result<(), ()>,
) -> Result<(), ()> {
    let Approved { handed, checked } = approved;
    let forwarded = Forwarded::of(&handed);
    hand(handed)?;
    forwarded.record(
        root,
        &Sent {
            persona: &checked.persona,
            vault: &checked.tool.vault,
            pool: &checked.bound.pool,
            runner: &checked.bound.runner,
        },
    );
    Ok(())
}

/// **Runner chat `ask.chat`'s `secret exec`, heard over pool `pool`'s link, in project `held`**
/// (module docs): its values sent to the runner's host for that one run, or the asker told why
/// not. `reader` is the run's input, which for a run its runner runs ends only as its asker goes.
/// Returns once it is decided.
pub fn run_on_its_runner(
    held: &crate::planes::Held,
    pool: &str,
    ask: brokered::Ask,
    reader: Box<dyn std::io::BufRead + Send>,
    mut writer: Box<dyn Write + Send>,
) {
    let session = ask.chat;
    let gone = Arc::new(AtomicBool::new(false));
    watch_the_asker(reader, Arc::clone(&gone));
    let root = held.root();
    let ctx = Ctx::new(root, Env::from_process());
    let authorised = |persona: &str, vault: &str| brokered::authorise(&ctx, Some(persona), vault);
    let read = |tool: &Tool| read(&ctx, tool);
    let moved = || held.chats().asks_may_have_moved();
    // The runner this chat's link reached, as the desktop's host said it (#3417).
    let reached = crate::hooks::reached_over_the_link();
    let owned = |config: &Path, pool: &str| Bound::over_the_link(config, pool, reached.as_deref());
    let gate = Gate {
        pool,
        ask: &ask,
        chat: held
            .chats()
            .login_asker(root, session)
            .map(|(asker, unconfined)| (asker.persona, unconfined)),
        root,
        pools_in: held.chats().pools_in(),
        tools: &tools::declared_and_allowed_here,
        authorised: &authorised,
        owned: &owned,
    };
    let decided = checked(&gate).and_then(|checked| {
        tracing::info!(
            "purlis: chat {session}'s {} on pool {pool} asks the person whether its values may \
             be sent for this run",
            checked.tool.name
        );
        answered(
            &gate,
            checked,
            &Waiting {
                tools: held.pool_tools(),
                gone: &gone,
                moved: &moved,
                read: &read,
                within: A_HANDED_LINE_WAITS_AT_MOST,
                now: crate::sandboxing::now_secs(),
            },
            &mut *writer,
        )
    });
    match decided {
        Ok(approved) => match sent(
            root,
            approved,
            while_its_asker_waits(&gone, hand_on_the_link),
        ) {
            Ok(()) => tracing::info!(
                "purlis: chat {session} on pool {pool} ran {} with its values sent from this \
                 desktop",
                ask.runs_as.as_deref().unwrap_or_default()
            ),
            Err(()) => brokered::refused(
                writer,
                "purlis could not send this run its values over the link to the runner, so \
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

/// **`hand`, only while the run's asker is still there** (#3408): a command that went after the
/// person answered, its values read already, is sent nothing, so nothing is recorded as sent.
pub fn while_its_asker_waits<'a>(
    gone: &'a AtomicBool,
    hand: impl FnOnce(Handed) -> Result<(), ()> + 'a,
) -> impl FnOnce(Handed) -> Result<(), ()> + 'a {
    move |handed| {
        if gone.load(Ordering::SeqCst) {
            return Err(());
        }
        hand(handed)
    }
}

/// **Hands `handed` to the link it is replied on** (`purlis_core::hookwire::relay::hand_values`),
/// once it is measured to fit a frame of it (`hostedsessions::fits_a_reply`): one that does not
/// is never handed on, where the link would carry a refusal in its place. Whatever is not handed
/// on is wiped as it goes (#3420).
fn hand_on_the_link(handed: Handed) -> Result<(), ()> {
    let replied = purlis_core::hookwire::relay::Replied::Handed(handed);
    if !crate::hostedsessions::fits_a_reply(&replied) {
        return Err(());
    }
    match replied {
        purlis_core::hookwire::relay::Replied::Handed(handed) => {
            purlis_core::hookwire::relay::hand_values(handed).map_err(drop)
        }
        _ => Err(()),
    }
}

/// Reads `reader` to its end on a thread of its own, and sets `gone` there: the run's asker went.
fn watch_the_asker(mut reader: Box<dyn std::io::BufRead + Send>, gone: Arc<AtomicBool>) {
    let spawned = std::thread::Builder::new()
        .name("purlis-pool-tool-asker".into())
        .spawn(move || {
            let mut sink = [0u8; 1024];
            while reader.read(&mut sink).is_ok_and(|read| read > 0) {}
            gone.store(true, Ordering::SeqCst);
        });
    if let Err(why) = spawned {
        tracing::warn!("purlis: a runner chat's command is not watched for its asker ({why})");
    }
}

/// The value of each of `tool`'s keys, read from its vault here: each the one copy, wiped as
/// it is dropped, a refusal part way through too (#3420).
fn read(ctx: &Ctx, tool: &Tool) -> Result<Vec<(String, Zeroizing<String>)>, String> {
    let vault =
        purlis_core::secrets::cmd::provider(ctx, &tool.vault).map_err(|e| e.message.to_string())?;
    let mut values: Vec<(String, Zeroizing<String>)> = Vec::new();
    for value in &tool.values {
        if values.iter().any(|(key, _)| *key == value.key) {
            continue;
        }
        let read = purlis_core::secrets::cmd::get_value(ctx, &vault, &value.key)
            .map(Zeroizing::new)
            .map_err(|e| e.message.to_string())?;
        values.push((value.key.clone(), read));
    }
    Ok(values)
}

/// What a pool tool Ask answered by nothing any more says.
const WAITS_NO_MORE: &str = "Nothing is asking about that run of a tool on a pool any more.";

/// **Allow or Keep off on run `run` of chat `session`'s tool on its pool** (#2047): with
/// `allow`, this desktop sends the tool its vault values on the pool's runner for that run
/// alone; Keep off refuses that run. The command that waits goes on, or is refused, at once.
#[tauri::command]
#[specta::specta]
pub async fn answer_pool_tool(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    run: u32,
    allow: bool,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("answering a run of a pool's tool", move || {
        pool_tool_answered(&held, session, run, allow)
    })
    .await
}

/// [`answer_pool_tool`]'s work.
pub fn pool_tool_answered(
    held: &crate::planes::Held,
    session: u32,
    run: u32,
    allow: bool,
) -> Result<(), String> {
    if held.pool_tools().answer(session, run, allow).is_none() {
        return Err(WAITS_NO_MORE.to_owned());
    }
    held.chats().asks_may_have_moved();
    Ok(())
}

#[cfg(test)]
mod tests;
