// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The kill switch reaches every runner this machine keeps** (#2051, ADR 0078 §7, ADR 0071 as
//! amended by ADR 0078).
//!
//! When this host is stopped, by `stop_all` from any scope or by a stop written elsewhere, it
//! sends a stop down a link of its own to every runner the binary says this machine keeps
//! ([`EveryRunner`]), all at once, each within [`REACHED_WITHIN`]. Each runner's host throws its
//! own switch. **A runner that is not reached keeps its chats running until it is**: the host
//! names it in what it says of the switch, so the window says so, and tries it again every
//! [`TRIED_AGAIN_EVERY`] while it stays stopped. **A link the relay opens while this host is
//! stopped carries a stop first**, before anything of the window's, and is ended where the
//! runner does not take it, so the window is never carried onto a runner that runs. A host that
//! starts stopped sends its stop to every runner once, so a stop written while no host ran, or
//! one a restart came between, still reaches them.
//!
//! **"Reached" is what the runner says**: its host answered that it is stopped now. A runner that
//! says it was stopped already (from its own command line) is not counted as this stop's, so
//! **the window's re-arm re-arms only the runners this stop stopped**, after this host's own, and
//! names them first (`runners` on the switch). A re-arm crosses the link only from here, as the
//! window's own act relayed (ADR 0078 §4, V29b).
//!
//! **Which runners were reached, and which are still owed a stop, are kept** beside the switch,
//! in the host's own folder, so a restart forgets neither. **One send at a time**: every stop and
//! re-arm to runners is sent under one hold, so a re-arm never lands on a runner after a stop
//! that came later, and a stop that comes during a re-arm is sent once it is done.
//!
//! **A runner's words never reach the switch's**: what a runner answered is said in fixed
//! sentences here, never in its own text.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use purlis_session_protocol::hostlink::Linked;
use purlis_session_protocol::session::{Answer, Client, Command};

use crate::relay::Keep;

/// **Every runner this machine keeps**, each named `<pool>/<runner>` with how to keep a link to
/// it, or why there is none: the binary's, read as each stop is sent. Called off the runtime.
pub type EveryRunner = Arc<dyn Fn() -> Vec<(String, Result<Keep, String>)> + Send + Sync>;

/// How long a stop or a re-arm waits for one runner to answer before it is named not reached.
pub const REACHED_WITHIN: Duration = Duration::from_secs(10);

/// How often a runner a stop did not reach is tried again while this host stays stopped.
pub const TRIED_AGAIN_EVERY: Duration = Duration::from_secs(30);

/// The file in the host's own folder that keeps which runners its stop reached and which it owes.
pub const RUNNERS_STOPPED: &str = "runners-stopped.json";

/// The most bytes [`RUNNERS_STOPPED`] is read at: one longer is none kept.
const MOST_KEPT_BYTES: usize = 64 * 1024;

/// The runners a stop of this host reached, and those it still owes one.
pub(crate) struct Runners {
    every: OnceLock<EveryRunner>,
    held: Mutex<Held>,
    /// Held while anything is sent to runners: one send at a time.
    sending: tokio::sync::Mutex<()>,
    /// Where [`RUNNERS_STOPPED`] is kept: the host's own folder; none for a test's host.
    kept: Option<PathBuf>,
    /// How often the runners owed a stop are tried again.
    again_every: Duration,
    /// How long one runner is waited on: [`REACHED_WITHIN`], but for a test.
    reached_within: Duration,
    /// Whether this host's start was looked at: a host that starts stopped sends its stop once.
    started: std::sync::atomic::AtomicBool,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Held {
    /// Stopped by this stop: the runners its re-arm re-arms.
    reached: BTreeSet<String>,
    /// Not reached yet, each with why, in this host's words.
    owed: BTreeMap<String, String>,
    /// When the runners owed a stop were last tried.
    #[serde(skip)]
    tried: Option<Instant>,
}

impl Default for Runners {
    fn default() -> Runners {
        Runners {
            every: OnceLock::new(),
            held: Mutex::default(),
            sending: tokio::sync::Mutex::new(()),
            kept: None,
            again_every: TRIED_AGAIN_EVERY,
            reached_within: REACHED_WITHIN,
            started: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

/// What one runner answered a stop or a re-arm, in this host's words.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Sent {
    /// It is as the command asked now.
    Done,
    /// It was stopped already, by its own command line: this stop did not stop it.
    StoppedAlready,
    /// It was not reached, and why.
    NotReached(String),
}

impl Runners {
    /// The runners kept in `folder`, as the last host left them. A file that cannot be read is
    /// none kept: the host's start sends its stop again, which a runner takes twice safely.
    pub(crate) fn kept_in(folder: &Path) -> Runners {
        let path = folder.join(RUNNERS_STOPPED);
        // Its size is looked at before it is read, and again as read.
        let small = std::fs::symlink_metadata(&path)
            .is_ok_and(|found| found.len() <= MOST_KEPT_BYTES as u64);
        let held = small
            .then(|| purlis_same_user::read_private_file(&path).ok())
            .flatten()
            .filter(|text| text.len() <= MOST_KEPT_BYTES)
            .and_then(|text| serde_json::from_str::<Held>(&text).ok())
            .unwrap_or_default();
        Runners {
            held: Mutex::new(held),
            kept: Some(folder.to_path_buf()),
            ..Runners::default()
        }
    }

    pub(crate) fn given(&self, every: EveryRunner) {
        let _ = self.every.set(every);
    }

    /// The runners owed a stop are tried again every `every` (a test's shorter).
    pub(crate) fn trying_again_every(mut self, every: Duration) -> Runners {
        self.again_every = every;
        self
    }

    /// One runner is waited on for `within` (a test's shorter).
    pub(crate) fn reaching_within(mut self, within: Duration) -> Runners {
        self.reached_within = within;
        self
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Writes what is held, where it is kept. One that cannot be written is said: a restart then
    /// sends the stop again.
    fn keep(&self) {
        let Some(folder) = &self.kept else {
            return;
        };
        let text = match serde_json::to_vec(&*self.held()) {
            Ok(text) => text,
            Err(_) => return,
        };
        let written = purlis_same_user::PrivateDirectory::open(folder)
            .and_then(|folder| folder.replace_file(RUNNERS_STOPPED, &text));
        if let Err(why) = written {
            crate::say(&format!(
                "which runners the stop reached could not be kept ({why}); a host started again \
                 sends its stop to every runner"
            ));
        }
    }

    /// The runners this host's stop has not reached yet, each with why.
    pub(crate) fn owed(&self) -> BTreeMap<String, String> {
        self.held().owed.clone()
    }

    /// The runners this host's stop stopped: what its re-arm re-arms.
    pub(crate) fn reached_now(&self) -> Vec<String> {
        self.held().reached.iter().cloned().collect()
    }

    /// Notes what `runner` answered a stop.
    fn stopped(&self, runner: String, sent: Sent) {
        let mut held = self.held();
        match sent {
            Sent::Done => {
                held.owed.remove(&runner);
                held.reached.insert(runner);
            }
            // Stopped by its own command line, or by this stop before: not owed. Only one this
            // stop stopped is re-armed with it.
            Sent::StoppedAlready => {
                held.owed.remove(&runner);
            }
            Sent::NotReached(why) => {
                crate::say(&format!(
                    "runner {runner} was not reached with the stop, and is tried again: {why}"
                ));
                held.owed.insert(runner, why);
            }
        }
    }

    /// Sends a stop to every runner this machine keeps, and keeps which were reached.
    pub(crate) async fn stop_every(&self) {
        let Some(every) = self.every.get().cloned() else {
            return;
        };
        let _sending = self.sending.lock().await;
        let runners = tokio::task::spawn_blocking(move || every())
            .await
            .unwrap_or_default();
        self.held().tried = Some(Instant::now());
        for (runner, sent) in send_to_each(runners, Command::StopAll, self.reached_within).await {
            self.stopped(runner, sent);
        }
        self.keep();
    }

    /// **As this host starts**: where its switch is stopped already, its stop is sent to every
    /// runner once. Then each look tries again the runners owed one.
    pub(crate) async fn look(&self, stopped: bool) {
        let first = !self.started.swap(true, std::sync::atomic::Ordering::SeqCst);
        if !stopped {
            return;
        }
        if first {
            self.stop_every().await;
        } else {
            self.try_the_owed_again().await;
        }
    }

    /// Tries again the runners a stop has not reached, where [`Runners::again_every`] has passed.
    pub(crate) async fn try_the_owed_again(&self) {
        let Some(every) = self.every.get().cloned() else {
            return;
        };
        let owed: BTreeSet<String> = {
            let held = self.held();
            let due = held
                .tried
                .is_none_or(|tried| tried.elapsed() >= self.again_every);
            if held.owed.is_empty() || !due {
                return;
            }
            held.owed.keys().cloned().collect()
        };
        let _sending = self.sending.lock().await;
        let runners = tokio::task::spawn_blocking(move || every())
            .await
            .unwrap_or_default();
        self.held().tried = Some(Instant::now());
        // A runner no longer kept is owed nothing.
        let kept: BTreeSet<String> = runners.iter().map(|(name, _)| name.clone()).collect();
        self.held().owed.retain(|runner, _| kept.contains(runner));
        let due: Vec<(String, Result<Keep, String>)> = runners
            .into_iter()
            .filter(|(name, _)| owed.contains(name))
            .collect();
        for (runner, sent) in send_to_each(due, Command::StopAll, self.reached_within).await {
            self.stopped(runner, sent);
        }
        self.keep();
    }

    /// Re-arms the runners this host's stop stopped, as its window re-armed it, and forgets the
    /// stop. Answers each runner that was not re-armed, with why. Where this host was stopped
    /// again meanwhile (`stopped`), the runners just re-armed are stopped again with it.
    pub(crate) async fn rearm_the_reached(
        &self,
        stopped: impl Fn() -> bool,
    ) -> BTreeMap<String, String> {
        let Some(every) = self.every.get().cloned() else {
            return BTreeMap::new();
        };
        let _sending = self.sending.lock().await;
        let reached = {
            let mut held = self.held();
            held.owed.clear();
            held.tried = None;
            std::mem::take(&mut held.reached)
        };
        if reached.is_empty() {
            self.keep();
            return BTreeMap::new();
        }
        let runners = tokio::task::spawn_blocking(move || every())
            .await
            .unwrap_or_default();
        let due: Vec<(String, Result<Keep, String>)> = runners
            .into_iter()
            .filter(|(name, _)| reached.contains(name))
            .collect();
        let sent = send_to_each(due, Command::Rearm, self.reached_within).await;
        let mut missed = BTreeMap::new();
        let mut rearmed = BTreeSet::new();
        for (runner, sent) in sent {
            match sent {
                Sent::Done => {
                    rearmed.insert(runner);
                }
                Sent::StoppedAlready => {}
                Sent::NotReached(why) => {
                    missed.insert(runner, why);
                }
            }
        }
        // Kept, so the next re-arm tries each again.
        self.held().reached.extend(missed.keys().cloned());
        // A stop that came while the re-arm was being sent stops them again.
        if stopped() && !rearmed.is_empty() {
            let every = self.every.get().cloned();
            if let Some(every) = every {
                let runners = tokio::task::spawn_blocking(move || every())
                    .await
                    .unwrap_or_default();
                let again: Vec<(String, Result<Keep, String>)> = runners
                    .into_iter()
                    .filter(|(name, _)| rearmed.contains(name))
                    .collect();
                for (runner, sent) in
                    send_to_each(again, Command::StopAll, self.reached_within).await
                {
                    self.stopped(runner, sent);
                }
            }
        }
        self.keep();
        missed
    }
}

/// Sends `command` to each runner at once, each on a link of its own, and answers what each
/// runner answered.
async fn send_to_each(
    runners: Vec<(String, Result<Keep, String>)>,
    command: Command,
    within: Duration,
) -> Vec<(String, Sent)> {
    let sending: Vec<_> = runners
        .into_iter()
        .map(|(runner, keep)| {
            let command = command.clone();
            tokio::spawn(async move {
                let sent = match keep {
                    Ok(keep) => tokio::time::timeout(within, send(keep, command))
                        .await
                        .unwrap_or_else(|_| Sent::NotReached(wait_ran_out(within))),
                    Err(why) => Sent::NotReached(why),
                };
                (runner, sent)
            })
        })
        .collect();
    let mut sent = Vec::new();
    for one in sending {
        if let Ok(answered) = one.await {
            sent.push(answered);
        }
    }
    sent
}

/// Sends `command` on the first link `keep` brings up, and says what the runner answered. A
/// link that is down is waited for: the caller holds the deadline.
async fn send(keep: Keep, command: Command) -> Sent {
    let mut links = keep();
    loop {
        match links.next().await {
            Some(Linked::Up(link)) => {
                links.moved();
                let (client, _views, _events) = Client::new(*link);
                return answered(client.call(command.clone()).await, &command);
            }
            // Its words are not ours: the link is said to be down, and why is not repeated.
            Some(Linked::Down(_)) => {}
            None => return Sent::NotReached(NO_LINK.into()),
        }
    }
}

/// What a runner that never linked is said to be.
const NO_LINK: &str = "no link to it came up";

/// What a runner's answer to `command` says, in this host's words.
fn answered(
    answer: Result<Answer, purlis_session_protocol::session::CallError>,
    command: &Command,
) -> Sent {
    let stopping = matches!(command, Command::StopAll);
    match answer {
        Ok(Answer::KillSwitch(switch)) if switch.stopped == stopping => {
            if stopping && switch.already {
                Sent::StoppedAlready
            } else {
                Sent::Done
            }
        }
        Ok(Answer::KillSwitch(_)) if stopping => {
            Sent::NotReached("it answered that its chats are not stopped".into())
        }
        Ok(Answer::KillSwitch(_)) => {
            Sent::NotReached("it answered that its chats are still stopped".into())
        }
        Ok(_) => Sent::NotReached("it gave an answer purlis does not read".into()),
        Err(refused) => Sent::NotReached(match refused.refusal() {
            Some(_) => "it refused it".into(),
            None => "the link ended before it answered".into(),
        }),
    }
}

/// What a link the relay opens to a runner sends first while this host is stopped: a stop, so a
/// runner that was asleep is stopped as soon as it is reached.
#[derive(Clone)]
pub(crate) struct StopFirst {
    /// The runner the link is to, `<pool>/<runner>`.
    pub(crate) runner: String,
    /// Whether this host is stopped now.
    pub(crate) stopped: Arc<dyn Fn() -> bool + Send + Sync>,
    pub(crate) runners: Arc<Runners>,
}

impl StopFirst {
    /// Sends a stop on `client`, a new link to the runner, where this host is stopped. Answers
    /// whether the link may carry the window on: not where the runner did not take the stop
    /// within [`REACHED_WITHIN`], which is then owed one.
    pub(crate) async fn on(&self, client: &Client) -> bool {
        if !(self.stopped)() {
            return true;
        }
        let _sending = self.runners.sending.lock().await;
        // A re-arm that came while this waited its turn: there is no stop to carry.
        if !(self.stopped)() {
            return true;
        }
        let within = self.runners.reached_within;
        let sent = tokio::time::timeout(within, client.call(Command::StopAll))
            .await
            .map_or_else(
                |_| Sent::NotReached(wait_ran_out(within)),
                |answer| answered(answer, &Command::StopAll),
            );
        let carries_on = !matches!(sent, Sent::NotReached(_));
        self.runners.stopped(self.runner.clone(), sent);
        self.runners.keep();
        carries_on
    }
}

/// Why a runner was not reached when the wait for it ran out: a limit that can't change (#4281).
fn wait_ran_out(within: Duration) -> String {
    format!(
        "it did not answer within {} seconds. {}",
        within.as_secs(),
        purlis_session_protocol::fixedlimits::STOP_WAIT.said()
    )
}

/// What this host says of the runners its stop has not reached, in a sentence, where any.
pub(crate) fn not_reached_said(owed: &BTreeMap<String, String>) -> Option<String> {
    if owed.is_empty() {
        return None;
    }
    let named: Vec<String> = owed
        .iter()
        .map(|(runner, why)| format!("{runner} ({why})"))
        .collect();
    let (runner, keeps) = if owed.len() == 1 {
        ("runner", "Its chats keep")
    } else {
        ("runners", "Their chats keep")
    };
    Some(format!(
        "The stop did not reach {runner} {}. {keeps} running until it does, and purlis keeps \
         trying.",
        named.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use purlis_session_protocol::session::{CallError, KillSwitch, Refusal};

    use super::*;

    #[test]
    fn a_runner_not_reached_is_named_with_why_and_what_becomes_of_its_chats() {
        let owed = BTreeMap::from([("qa/r1".to_owned(), "it did not answer".to_owned())]);
        assert_eq!(
            not_reached_said(&owed).unwrap(),
            "The stop did not reach runner qa/r1 (it did not answer). Its chats keep running \
             until it does, and purlis keeps trying."
        );
        let two = BTreeMap::from([
            ("qa/r1".to_owned(), "a".to_owned()),
            ("qa/r2".to_owned(), "b".to_owned()),
        ]);
        assert!(
            not_reached_said(&two)
                .unwrap()
                .starts_with("The stop did not reach runners qa/r1 (a), qa/r2 (b). Their chats")
        );
        assert_eq!(not_reached_said(&BTreeMap::new()), None);
    }

    #[test]
    fn a_runners_own_words_never_become_the_switchs() {
        let said = |answer| match answered(answer, &Command::StopAll) {
            Sent::NotReached(why) => why,
            other => panic!("{other:?}"),
        };
        let words = "). Every runner is stopped.";
        assert!(
            !said(Ok(Answer::KillSwitch(KillSwitch {
                stopped: false,
                why: Some(words.into()),
                ..Default::default()
            })))
            .contains(words)
        );
        assert!(
            !said(Err(CallError::Refused(Refusal {
                code: "not_allowed".into(),
                why: words.into(),
            })))
            .contains(words)
        );
        assert!(!said(Ok(Answer::Done)).contains("Done"));
    }

    #[test]
    fn a_runner_that_was_stopped_already_is_not_this_stops() {
        let answer = Ok(Answer::KillSwitch(KillSwitch {
            stopped: true,
            already: true,
            ..Default::default()
        }));
        assert_eq!(answered(answer, &Command::StopAll), Sent::StoppedAlready);
    }

    #[test]
    fn what_a_stop_reached_and_owes_is_kept_beside_the_switch_across_a_restart() {
        let folder = tempfile::tempdir().unwrap();
        let runners = Runners::kept_in(folder.path());
        runners.stopped("qa/r1".into(), Sent::Done);
        runners.stopped(
            "qa/r2".into(),
            Sent::NotReached("no link to it came up".into()),
        );
        runners.keep();

        let again = Runners::kept_in(folder.path());

        assert_eq!(again.reached_now(), vec!["qa/r1".to_owned()]);
        assert_eq!(
            again.owed().keys().cloned().collect::<Vec<_>>(),
            vec!["qa/r2"]
        );
    }
}
