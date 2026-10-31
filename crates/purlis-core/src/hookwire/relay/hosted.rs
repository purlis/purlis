// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A `secret exec` of a chat this host composed, run here on the values its desktop sends**
//! (#2047, ADR 0078 §7, spec #2041 stories 14 and 21).
//!
//! A runner holds no vault. When a chat it composed in a sandbox runs one of its persona's
//! declared tools through `purlis secret exec`, the runner's host first finds which tool its own
//! start armed the command as ([`brokered::armed_as`]), and refuses, as a run here would, what
//! is none. It hands the line on to the window with that tool's name ([`brokered::Ask::runs_as`])
//! and a number of this run's own ([`brokered::Ask::run`]), and the window, after its own gate
//! and the person's answer, sends the values for that one run ([`Replied::Handed`]). This host
//! runs the tool only on values sent for exactly that chat, tool and run, held to its own record
//! of the chat's sandbox ([`brokered::serve_handed`]); the values go with the run.
//!
//! **With the desktop away the command waits** for a window to hear this host again, as a
//! browser login does ([`super::A_HANDED_LINE_WAITS_AT_MOST`], admitted within the same bounds,
//! [`Relaying::admit`]), told so on its stderr, and is handed on marked as one that waited. The
//! chat may carry on: the command holds only itself. It is let go of, with nothing run, once it
//! has waited too long, once its asker hangs up, or once its chat is gone.
//!
//! **On a pool that opts in to minting, a minting tool's run asks the cluster first** (#4207, the
//! person's ruling on #3807, Q1): this host's token from the operator's mint port runs it, with
//! nobody asked and the desktop perhaps away, since the admin's opt-in stands as consent and the
//! operator logs each mint. It holds for mint tools alone. A host told of no port, or one the port
//! refuses, hands the line on as above, **so what the window sends still takes the window's gate and
//! the person's Ask**: a refusal never becomes a token without them.
//!
//! **A chat's refused mints are paced** (#4332, [`paced`]): a chat that runs its tool again and again
//! while the port refuses it asks the operator less and less often, and meanwhile goes straight to
//! the window. Once the window has refused such a run too, the next ones are refused here for a
//! while, so a loop does not keep asking the person. Runs the window's values answered are never
//! held back: they clear the pace, and a host told of no port is not paced at all.
//!
//! **An asker that hangs up is told to the window** while the window decides
//! ([`Heard::InputEnded`] for the line, which for such a run carries no input), so the window lets
//! go of the person's Ask and reads no vault for it. The asker writes nothing before its run has
//! started, so its connection is read from the start: lines that come are kept for the run, and
//! its end says the asker hung up.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use super::{A_HANDED_LINE_WAITS_AT_MOST, Heard, Lines, Relaying, Replied, waits_at_most_said};
use crate::secrets::brokered::{self, Asker};

mod paced;
pub(super) use paced::Paced;

/// What runs a `secret exec` on the values the window sent ([`brokered::serve_handed`]).
pub type RunsHanded = Arc<
    dyn Fn(
            &Asker,
            brokered::Wanted,
            brokered::Handed,
            Box<dyn BufRead + Send>,
            Box<dyn Write + Send>,
            brokered::Told,
        ) + Send
        + Sync
        + 'static,
>;

/// **What a minting tool's run gets from the cluster**: for `asker`'s tool named by the second
/// word, the tool's key and the token, or why the port refused. `None` where none is to be asked:
/// the tool mints nothing, or this host was told of no port.
pub type InCluster = Arc<
    dyn Fn(&Asker, &str) -> Option<Result<(String, zeroize::Zeroizing<String>), String>>
        + Send
        + Sync
        + 'static,
>;

/// The real [`InCluster`]: this host's config home's port ([`crate::forge::mintport`]).
#[cfg(unix)]
pub fn in_cluster(
    asker: &Asker,
    tool: &str,
) -> Option<Result<(String, zeroize::Zeroizing<String>), String>> {
    let ready = asker
        .confines
        .as_ref()?
        .tools
        .iter()
        .find(|ready| ready.tool.name == tool && ready.tool.mint.is_some())?;
    let key = ready.tool.values.first()?.key.clone();
    let config_root = crate::forge::apptoken::record_root()?;
    let minted =
        crate::forge::apptoken::token_in_cluster(&asker.root, asker.chat, &ready.tool, &|app| {
            crate::forge::mintport::mint_in_time(&config_root, app)
        });
    match minted {
        Ok(Some(token)) => Some(Ok((key, token))),
        Ok(None) => None,
        Err(why) => Some(Err(why)),
    }
}

/// A host away from unix has no runner to serve.
#[cfg(not(unix))]
pub fn in_cluster(
    _asker: &Asker,
    _tool: &str,
) -> Option<Result<(String, zeroize::Zeroizing<String>), String>> {
    None
}

/// What the asker of `tool` is told where the window refused a run the cluster refused too, moments
/// ago.
fn asked_too_soon_said(tool: &str, left: Duration) -> String {
    format!(
        "the person's desktop refused {tool} moments ago, and the cluster minted it no token, so \
         purlis does not ask them again for {} seconds: run it again after that",
        left.as_secs().max(1)
    )
}

/// What the asker of `tool` is told where the cluster refused it a token.
fn in_cluster_refused_said(tool: &str, why: &str) -> String {
    format!(
        "the cluster minted no token for {tool} ({why}), so purlis asks the person's desktop \
         instead"
    )
}

/// How often a `secret exec` that waits for the window tries to hand itself on, and how often
/// one handed on looks at its asker.
const LOOKED_AT_EVERY: Duration = Duration::from_millis(500);

/// How many lines of the asker's input are kept before its run reads them.
const LINES_KEPT: usize = 64;

/// What the asker of `tool` is told on its stderr while it waits for the person's desktop.
pub fn exec_waits_said(tool: &str) -> String {
    format!(
        "the person's desktop is not connected, so purlis cannot send {tool} its vault value \
         yet: this command waits for them, at most {within}, and runs once they allow it. Run \
         it in the background to carry on with what needs no vault meanwhile",
        within = waits_at_most_said()
    )
}

/// What the asker of `tool` is told where the desktop was not back in time.
fn waited_too_long_said(tool: &str) -> String {
    format!(
        "the person's desktop was not back within {within}, so {tool} did not run and was sent \
         no value: run it again once they are back",
        within = waits_at_most_said()
    )
}

/// What the asker of `tool` is told where too many lines wait already.
fn too_many_wait_said(tool: &str) -> String {
    format!(
        "the person's desktop is not connected, and too many commands and logins wait for it \
         already, so {tool} did not run: run it again once they are back"
    )
}

/// What the asker of `tool` is told where the window went before it sent the values.
fn window_went_said(tool: &str) -> String {
    format!(
        "the person's desktop went before it answered, so {tool} did not run and was sent no \
         value: run it again"
    )
}

/// What the asker of `tool` is told where what came back was not the values for this run.
fn not_for_this_run_said(tool: &str) -> String {
    format!(
        "what the person's desktop sent was not for this run of {tool}, so it did not run: run \
         it again"
    )
}

/// What the asker of `tool` is told where its chat, as this host composed it, is gone or changed
/// by the time its values came.
fn chat_gone_said(tool: &str) -> String {
    format!(
        "this chat, as purlis started it here, is gone or was started again while the person \
         was asked, so {tool} did not run: run it again"
    )
}

/// What the asker of `tool` is told where the values could not cross the link.
fn not_taken_said(tool: &str, why: &str) -> String {
    format!(
        "the person's desktop sent {tool}'s values, and they could not cross the link to this \
         runner ({why}), so it did not run: keep the tool's values smaller"
    )
}

/// The asker's input, read from the start into a channel the run reads later: `ended` once the
/// asker's end of the connection has.
struct Input {
    lines: mpsc::Receiver<String>,
    ended: Arc<AtomicBool>,
}

impl Input {
    fn read(reader: Box<dyn BufRead + Send>) -> Option<Input> {
        let (to, lines) = mpsc::sync_channel(LINES_KEPT);
        let ended = Arc::new(AtomicBool::new(false));
        let ending = Arc::clone(&ended);
        std::thread::Builder::new()
            .name("purlisd-secret-input".into())
            .spawn(move || {
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    if to.send(line).is_err() {
                        break;
                    }
                }
                ending.store(true, Ordering::SeqCst);
            })
            .ok()?;
        Some(Input { lines, ended })
    }

    /// Whether the asker hung up: its connection ended with nothing more to read.
    fn gone(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }
}

/// One line of a chat's admitted to wait for the window ([`Relaying::admit`]), until dropped.
struct Admitted<'a> {
    relaying: &'a Relaying,
    chat: u32,
}

impl Drop for Admitted<'_> {
    fn drop(&mut self) {
        self.relaying.release(self.chat);
    }
}

impl Relaying {
    /// Asks `mints` in place of this host's port: for a test.
    #[cfg(test)]
    pub(super) fn minting_in_cluster_with(self: Arc<Self>, mints: InCluster) -> Arc<Self> {
        let mut relaying = Arc::try_unwrap(self).unwrap_or_else(|_| panic!("held elsewhere"));
        relaying.in_cluster = mints;
        Arc::new(relaying)
    }

    /// Runs each `secret exec` handed its values with `runs` in place of the brokered run: for
    /// a test of what reaches the run.
    #[cfg(test)]
    pub(super) fn running_handed_with(self: Arc<Self>, runs: RunsHanded) -> Arc<Self> {
        let mut relaying = Arc::try_unwrap(self).unwrap_or_else(|_| panic!("held elsewhere"));
        relaying.runs_handed = runs;
        Arc::new(relaying)
    }

    /// **Chat `asker`'s `secret exec` `ask`, the chat composed here** (module docs).
    pub(super) fn secret_exec_composed_here(
        self: &Arc<Self>,
        asker: Asker,
        mut ask: brokered::Ask,
        reader: Box<dyn BufRead + Send>,
        mut writer: Box<dyn Write + Send>,
    ) {
        let tool = match brokered::armed_as(&asker, &ask.secret_exec) {
            Ok(tool) => tool,
            Err(why) => return brokered::refused(writer, why),
        };
        let Some(run) = brokered::a_run_number() else {
            return brokered::refused(
                writer,
                "purlis could not number this run, so it did not run".to_owned(),
            );
        };
        ask.runs_as = Some(tool.clone());
        ask.run = Some(run.clone());
        let Some(input) = Input::read(reader) else {
            return brokered::not_answered(writer);
        };
        // Chat's own pace, for this tool: what it costs the operator and the person (#4332).
        let paced = (asker.root.clone(), ask.chat, tool.clone());
        let in_cluster = match self.port_paced.waits(&paced) {
            Some(waiting) => Some(Err(waiting.words)),
            None => {
                let asked = (self.in_cluster)(&asker, &tool);
                match &asked {
                    Some(Ok(_)) => self.port_paced.cleared(&paced),
                    Some(Err(why)) => self.port_paced.refused(&paced, why),
                    None => {}
                }
                asked
            }
        };
        let cluster_refused = matches!(in_cluster, Some(Err(_)));
        match in_cluster {
            Some(Ok((key, token))) => {
                if input.gone() {
                    return;
                }
                // Held to the chat as this host keeps it now (#3407): a mint takes time.
                let Some(now) = self
                    .at()
                    .and_then(|root| crate::hostedchats::asker(&root, ask.chat))
                    .filter(|now| {
                        brokered::armed_as(now, &ask.secret_exec).as_deref() == Ok(tool.as_str())
                    })
                else {
                    // Its end may have come before this mint looked: nothing is left kept.
                    crate::forge::apptoken::chat_ended(&asker.root, ask.chat);
                    return brokered::refused(writer, chat_gone_said(&tool));
                };
                let handed =
                    brokered::Handed::from_the_cluster(ask.chat, &tool, &run, [(key, token)]);
                return self.runs_on(&now, ask.secret_exec, handed, input, writer);
            }
            Some(Err(why)) => {
                if let Some(waiting) = self.asks_paced.waits(&paced) {
                    return brokered::refused(writer, asked_too_soon_said(&tool, waiting.left));
                }
                tracing::info!(
                    "purlisd: chat {}'s run of {tool} was refused a token in the cluster",
                    ask.chat
                );
                let note = brokered::Frame::Note(in_cluster_refused_said(&tool, &why));
                if serde_json::to_string(&note)
                    .ok()
                    .and_then(|line| {
                        writeln!(writer, "{line}")
                            .and_then(|()| writer.flush())
                            .ok()
                    })
                    .is_none()
                {
                    return;
                }
            }
            None => {}
        }
        let exchanged = match self.exchange(&Heard::SecretExec(ask.clone())) {
            Some(exchanged) => exchanged,
            None => match self.waits_for_a_window(&mut ask, &tool, &input, &mut writer) {
                Ok(exchanged) => exchanged,
                Err(Some(why)) => return brokered::refused(writer, why),
                Err(None) => return,
            },
        };
        let (id, replied) = exchanged;
        let mut told_gone = false;
        loop {
            let reply = match replied.recv_timeout(LOOKED_AT_EVERY) {
                Ok(reply) => reply,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Its asker hung up while the window decides: told once, so the window lets
                    // the person's Ask go and reads nothing for it.
                    if input.gone() && !told_gone {
                        told_gone = true;
                        self.tell(&Heard::InputEnded { of: id });
                    }
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    brokered::refused(writer, window_went_said(&tool));
                    break;
                }
            };
            match reply {
                Replied::Handed(handed) => {
                    self.done(id);
                    if cluster_refused {
                        self.asks_paced.cleared(&paced);
                    }
                    // Only the values sent for this chat's run of this tool, and only while its
                    // asker is there.
                    if !handed.is_for(ask.chat, &tool, &run) {
                        tracing::warn!(
                            "purlisd: chat {}'s run of {tool} was sent values that were not for \
                             it; nothing ran",
                            ask.chat
                        );
                        return brokered::refused(writer, not_for_this_run_said(&tool));
                    }
                    if input.gone() {
                        return;
                    }
                    // Held to the chat as this host keeps it now, not as it was when the command
                    // asked (#3407): the person may have changed its reach while they were asked,
                    // or its chat may have gone. The tool it runs as is the one asked about.
                    let Some(now) = self
                        .at()
                        .and_then(|root| crate::hostedchats::asker(&root, ask.chat))
                    else {
                        return brokered::refused(writer, chat_gone_said(&tool));
                    };
                    if brokered::armed_as(&now, &ask.secret_exec).as_deref() != Ok(tool.as_str()) {
                        return brokered::refused(writer, chat_gone_said(&tool));
                    }
                    return self.runs_on(&now, ask.secret_exec, handed, input, writer);
                }
                // The window's own words: a refusal, said as it said it.
                Replied::Wrote { text } => {
                    if cluster_refused {
                        self.asks_paced.refused(&paced, &text);
                    }
                    if writer
                        .write_all(text.as_bytes())
                        .and_then(|()| writer.flush())
                        .is_err()
                    {
                        break;
                    }
                }
                // A reply the link could not carry, the values too large among them.
                Replied::NotTaken { why } => {
                    brokered::refused(writer, not_taken_said(&tool, &why));
                    break;
                }
                Replied::Ended => break,
                _ => {}
            }
        }
        self.done(id);
    }

    /// Runs `wanted` here on `handed`, its asker's input as read so far.
    fn runs_on(
        self: &Arc<Self>,
        asker: &Asker,
        wanted: brokered::Wanted,
        handed: brokered::Handed,
        input: Input,
        writer: Box<dyn Write + Send>,
    ) {
        let relaying = Arc::clone(self);
        let told: brokered::Told = Arc::new(move |run: brokered::RunBlocked| {
            relaying.tell(&Heard::Blocked(run.block));
        });
        let reader = Box::new(std::io::BufReader::new(Lines {
            lines: input.lines,
            pending: Vec::new(),
            at: 0,
        }));
        (self.runs_handed)(asker, wanted, handed, reader, writer, told)
    }

    /// **Holds `ask` until a window hears this host again** (#2047), at most
    /// [`A_HANDED_LINE_WAITS_AT_MOST`], telling its asker so on stderr; then hands it on marked
    /// as one that waited. `Err` with what to tell the asker, or with nothing where it hung up
    /// or its chat is gone.
    fn waits_for_a_window(
        &self,
        ask: &mut brokered::Ask,
        tool: &str,
        input: &Input,
        writer: &mut Box<dyn Write + Send>,
    ) -> Result<(u64, mpsc::Receiver<Replied>), Option<String>> {
        let chat = ask.chat;
        if !self.admit(chat) {
            return Err(Some(too_many_wait_said(tool)));
        }
        let _admitted = Admitted {
            relaying: self,
            chat,
        };
        let note = brokered::Frame::Note(exec_waits_said(tool));
        if serde_json::to_string(&note)
            .ok()
            .and_then(|line| {
                writeln!(writer, "{line}")
                    .and_then(|()| writer.flush())
                    .ok()
            })
            .is_none()
        {
            return Err(None);
        }
        ask.waited = true;
        let since = Instant::now();
        loop {
            std::thread::sleep(LOOKED_AT_EVERY);
            if input.gone() || !self.still_there(chat) {
                return Err(None);
            }
            if since.elapsed() > A_HANDED_LINE_WAITS_AT_MOST {
                return Err(Some(waited_too_long_said(tool)));
            }
            if let Some(exchanged) = self.exchange(&Heard::SecretExec(ask.clone())) {
                return Ok(exchanged);
            }
        }
    }
}

#[cfg(test)]
mod tests;
