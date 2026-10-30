// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A subscription profile is signed in through its harness's own flow** (#2035, ADR 0095 §3).
//!
//! Signing in runs the harness's own sign-in command (`claude auth login`, `codex login`,
//! `opencode auth login`) with the profile's folder variables set, in a terminal the person
//! drives: a shell tab in the window, or their own terminal with `purlis profile sign-in`. The
//! harness keeps what it signs in with in the profile's folder. purlis never opens a file it
//! writes ([`Login`]), and reads what it prints only where it drives it with no terminal: for its
//! page, its prompt and its words of refusal ([`driven`], #4042).
//!
//! **Whether a profile is signed in is the harness's own answer** ([`standing`]): its status
//! command's exit status where it has one (Claude Code's `auth status`, Codex's
//! `login status`), with nothing it prints read; otherwise whether its credential file is
//! there, by its name alone (opencode's `auth.json`), never opened. Each harness's words are
//! here, beside [`crate::harness_declaration`]'s own list of sign-in variables, not yet
//! declaration data. Every [`Look`] can run a program and look for a name, and none can read a
//! file.
//!
//! **A chat is refused only where the harness says it is signed out** ([`refusal`]), and only
//! on a profile purlis made: an adopted folder is signed in by the person's own sign-in and is
//! never asked, and a built-in or a clone's profile runs as it always did. A harness that cannot
//! tell, or does not answer in time, never stops a chat that might work. A refused start
//! leaves one wait per project and profile ([`waits_on`], [`waiting`]), which the window shows
//! as one Ask, "Sign in <profile>", until the harness says it is signed in or the person
//! dismisses it.
//!
//! **What a harness says is remembered for a minute** ([`REMEMBERED`]), so many chats opened at
//! once ask it once; Sign in forgets it ([`forget`]).
//!
//! **opencode is signed in to a plan its declaration lists** ([`providers`], `[sign_in] plans`,
//! #2039): ChatGPT or GitHub Copilot, by its own `--provider`, and never Anthropic, whose plan
//! only Claude Code's own declaration may list.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::MachineProfile;
use crate::harness_declaration::Plan;
use crate::profiles::{Profile, Source};

/// How long a status command has to answer: as long as a program's `--version`
/// ([`crate::sandbox::program::PATIENCE`]), since a program the system has not seen before is
/// assessed on its first run.
pub const PATIENCE: Duration = crate::sandbox::program::PATIENCE;

/// Whether a profile is signed in on this device, as its harness says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The harness says it is signed in.
    SignedIn,
    /// The harness says it is not: a chat on it is refused, with Sign in as the way out.
    SignedOut,
    /// The harness did not say, or has no way to: a chat on it starts.
    CannotTell,
    /// The runner is fetching the harness version the profile pins (#4008), which a look
    /// started, so its harness cannot say yet: a chat on it starts, as where it cannot tell.
    Fetching,
}

impl Standing {
    /// The word the window and the command line say it by.
    pub fn word(self) -> &'static str {
        match self {
            Self::SignedIn => "signed-in",
            Self::SignedOut => "signed-out",
            Self::CannotTell => "cannot-tell",
            Self::Fetching => "fetching",
        }
    }
}

/// What purlis may do to tell whether a profile is signed in: run a program and see how it
/// exits, and look for a name. Nothing here reads a file, and nothing a program prints is kept.
pub trait Look {
    /// The exit status of `program`, found where a chat's program is found, run with `args`,
    /// `env` set over this process's own environment, its input empty and its output thrown
    /// away; `None` where it could not be found or run, was ended by a signal, or did not end
    /// within [`PATIENCE`].
    fn status(&self, program: &str, args: &[&str], env: &[(String, String)]) -> Option<i32>;
    /// Whether something is at `path`, by its name, without opening it.
    fn is_there(&self, path: &Path) -> bool;
    /// **The program of the version `record` is pinned to, where this machine installs it**
    /// (#3446, #4007): on a runner keeping it for its desktop with a pin, that version once it
    /// is installed, which a status is asked of in place of a harness on `PATH`. `None` for
    /// every other profile and machine, and while the version is still being fetched.
    fn pinned_program(&self, record: &MachineProfile) -> Option<PathBuf> {
        let _ = record;
        None
    }
    /// **Whether the version `record` is pinned to is being fetched** (#4008) by a fetch a look
    /// started ([`Look::pinned_program`]): what tells a runner's "fetching" from "cannot tell".
    fn fetching(&self, record: &MachineProfile) -> bool {
        let _ = record;
        false
    }
}

/// This machine: programs run for real, and names looked for on its disk.
pub struct ThisMachine;

impl Look for ThisMachine {
    fn status(&self, program: &str, args: &[&str], env: &[(String, String)]) -> Option<i32> {
        use std::process::{Command, Stdio};
        let program = crate::programs::resolve(program).ok()?;
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(env.iter().map(|(n, v)| (n, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = crate::forklock::spawn(&mut command).ok()?;
        let until = Instant::now() + PATIENCE;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.code(),
                Ok(None) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }

    fn is_there(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }

    fn pinned_program(&self, record: &MachineProfile) -> Option<PathBuf> {
        super::installs::for_a_look(record)
    }

    fn fetching(&self, record: &MachineProfile) -> bool {
        super::installs::fetching_for_a_look(record)
    }
}

/// How a harness's status is told.
enum Status {
    /// Its own command, run in the profile's folder: exit 0 signed in, 1 signed out.
    Command(&'static [&'static str]),
    /// A file of this name under its own folder in the profile's data folder: there, it is
    /// signed in; not there, it may still run on a model that needs no sign-in.
    Named {
        own: &'static str,
        file: &'static str,
    },
}

/// A harness's own sign-in, and how its status is told.
struct Flow {
    kind: &'static str,
    /// The words after its program that sign it in, in its own terminal flow.
    login: &'static [&'static str],
    /// The flag that names the provider, followed by one of [`providers`]' ids, for a harness
    /// whose declaration lists plans. Empty for one that serves its own vendor's plan alone.
    provider_flag: &'static str,
    status: Status,
}

/// The harnesses purlis ships, each by its own documentation and help: Claude Code 2.1's
/// `auth login` and `auth status`, Codex's `login` and `login status`, and opencode's
/// `auth login`, which keeps what it signs in with in `opencode/auth.json` under its data
/// folder and has no status that says signed out.
const FLOWS: [Flow; 3] = [
    Flow {
        kind: "claude",
        login: &["auth", "login"],
        provider_flag: "",
        status: Status::Command(&["auth", "status"]),
    },
    Flow {
        kind: "codex",
        login: &["login"],
        provider_flag: "",
        status: Status::Command(&["login", "status"]),
    },
    Flow {
        kind: "opencode",
        login: &["auth", "login"],
        // opencode 1.18.33's `auth login --provider`: provider selection skipped.
        provider_flag: "--provider",
        status: Status::Named {
            own: "opencode",
            file: "auth.json",
        },
    },
];

fn flow(kind: &str) -> Option<&'static Flow> {
    FLOWS.iter().find(|flow| flow.kind == kind)
}

/// The plans a subscription of harness `kind` is signed in to, where it serves several and
/// purlis names the one: its declaration's `[sign_in] plans` (#2039), opencode's ChatGPT and
/// GitHub Copilot, and never a Claude plan but in Claude Code's own. Empty for a harness that
/// serves its own vendor's plan alone, or one purlis does not ship.
pub fn providers(kind: &str) -> &'static [Plan] {
    if flow(kind).is_none() {
        return &[];
    }
    crate::harness_declaration::builtin(kind).map_or(&[], |declared| &declared.sign_in.plans)
}

/// **Whether `provider` may be named for a sign-in of harness `kind`** (#2049): none, or one of
/// its plans ([`providers`]); anything else is refused in words, and nothing is typed.
pub fn plan_named(kind: &str, provider: Option<&str>) -> Result<(), String> {
    match provider {
        Some(provider) if !providers(kind).iter().any(|plan| plan.provider == provider) => {
            Err(format!(
                "'{}' is no plan {kind} signs in to, so no sign-in was typed.",
                crate::shown::short(provider)
            ))
        }
        _ => Ok(()),
    }
}

/// The value `profile` sets its harness's `variable` to, from its declared folder variables.
fn folder(profile: &Profile, variable: Option<&str>) -> Option<PathBuf> {
    let variable = variable?;
    profile
        .env
        .iter()
        .find(|(name, _)| name == variable)
        .map(|(_, value)| PathBuf::from(value))
}

/// **Whether `profile` is signed in on this device**, as its harness says it through `look`.
pub fn standing(profile: &Profile, look: &dyn Look) -> Standing {
    let Some(flow) = flow(&profile.kind) else {
        return Standing::CannotTell;
    };
    match flow.status {
        Status::Command(words) => {
            let Some(program) = program_of(profile) else {
                return Standing::CannotTell;
            };
            match look.status(&program, words, &profile.env) {
                Some(0) => Standing::SignedIn,
                Some(1) => Standing::SignedOut,
                _ => Standing::CannotTell,
            }
        }
        Status::Named { own, file } => {
            let data = crate::harness_declaration::builtin(&profile.kind)
                .and_then(|d| folder(profile, d.folders.data_env.as_deref()));
            match data {
                Some(data) if look.is_there(&data.join(own).join(file)) => Standing::SignedIn,
                _ => Standing::CannotTell,
            }
        }
    }
}

/// How long what a harness said of a profile is remembered.
pub const REMEMBERED: Duration = Duration::from_secs(60);

/// What each profile's harness said, by its kind and its folders, and when.
type Said = Vec<((String, Vec<(String, String)>), Standing, Instant)>;

static SAID: Mutex<Said> = Mutex::new(Vec::new());

fn key_of(profile: &Profile) -> (String, Vec<(String, String)>) {
    (profile.kind.clone(), profile.env.clone())
}

/// [`standing`], **as its harness said it within the last [`REMEMBERED`]**, or asked now and
/// remembered: what a chat's start and the window read, so reopening many chats on one profile
/// runs its status once.
pub fn standing_remembered(profile: &Profile, look: &dyn Look) -> Standing {
    standing_at(profile, look, Instant::now())
}

fn standing_at(profile: &Profile, look: &dyn Look, now: Instant) -> Standing {
    if let Some(said) = remembered_at(profile, now) {
        return said;
    }
    let key = key_of(profile);
    let said = standing(profile, look);
    let mut all = SAID.lock().unwrap_or_else(PoisonError::into_inner);
    all.retain(|(k, _, _)| *k != key);
    all.push((key, said, now));
    said
}

/// Forgets what `profile`'s harness said, so it is asked again: what Sign in does.
pub fn forget(profile: &Profile) {
    let key = key_of(profile);
    SAID.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(k, _, _)| *k != key);
}

/// `profile`'s program, as its command names it.
fn program_of(profile: &Profile) -> Option<String> {
    let home = crate::profiles::home().unwrap_or_default();
    crate::profiles::expanded_command(profile, &home)
        .into_iter()
        .next()
}

/// **Why a chat on `profile` may not start, or `None`**: only a profile purlis made on this
/// machine, signed in by a subscription as far as purlis can see (no key or cloud switch it is
/// handed, by its `env` or by `passed`, the project's `[chat_env] pass`), whose harness says it
/// is signed out.
pub fn refusal(profile: &Profile, passed: &[String], look: &dyn Look) -> Option<String> {
    if profile.source != Source::Machine {
        return None;
    }
    let declared = crate::harness_declaration::builtin(&profile.kind)?;
    if declared.profile_sign_in(&profile.env, passed).is_some() {
        return None;
    }
    (standing_remembered(profile, look) == Standing::SignedOut).then(|| {
        format!(
            "the profile '{name}' is not signed in on {here}. Nothing was started. It waits in \
             the Inbox as Sign in {name}, which runs {title}'s own sign-in in a shell tab, or \
             run purlis profile sign-in {name} in your own terminal; then start the chat again.",
            name = crate::shown::short(&profile.name),
            here = super::this_device(),
            title = declared.title,
        )
    })
}

/// **A profile a start was refused on**, waiting to be signed in: one per project and profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wait {
    /// The project the start was refused in.
    pub root: PathBuf,
    /// The profile, as the start read it.
    pub profile: Profile,
    /// Its harness as people call it: `Claude Code`.
    pub harness: String,
    /// When the first start was refused, in seconds since 1970.
    pub since: u64,
}

static WAITS: Mutex<Vec<Wait>> = Mutex::new(Vec::new());

/// Records that a start in the project at `root` was refused because `profile` is not signed
/// in: one wait per project and profile, however many starts were refused.
pub fn waits_on(root: &Path, profile: &Profile) {
    let harness = crate::harness_declaration::builtin(&profile.kind)
        .map_or_else(|| profile.kind.clone(), |d| d.title.clone());
    let mut all = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    if all
        .iter()
        .any(|w| w.root == root && w.profile.name == profile.name)
    {
        return;
    }
    all.push(Wait {
        root: root.to_path_buf(),
        profile: profile.clone(),
        harness,
        since: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    });
}

/// What `profile`'s harness said within the last [`REMEMBERED`] before `now`, if it said.
fn remembered_at(profile: &Profile, now: Instant) -> Option<Standing> {
    let key = key_of(profile);
    SAID.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|(k, _, at)| *k == key && now.saturating_duration_since(*at) < REMEMBERED)
        .map(|(_, said, _)| *said)
}

/// **The profiles the project at `root` waits on to be signed in**, oldest first, as their
/// harnesses last said, with no program run: what the window lists, on its own thread. A wait
/// whose harness has since said anything but signed out is let go; one it has not been asked
/// about lately still waits.
pub fn waiting(root: &Path) -> Vec<Wait> {
    let now = Instant::now();
    let mut all = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    all.retain(|w| {
        w.root != root
            || remembered_at(&w.profile, now).is_none_or(|said| said == Standing::SignedOut)
    });
    all.iter().filter(|w| w.root == root).cloned().collect()
}

/// **Asks `profile`'s harness again every `every`, for at most `until`, until it says anything
/// but signed out**, then lets its waits go and calls `told`: what Sign in starts, so its Ask
/// leaves the Inbox once the person has signed in, in the shell tab or anywhere else. Runs on a
/// thread of its own.
pub fn watch(
    profile: Profile,
    look: impl Look + Send + 'static,
    every: Duration,
    until: Duration,
    told: impl FnOnce() + Send + 'static,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let ends = Instant::now() + until;
        while Instant::now() < ends {
            std::thread::sleep(every);
            forget(&profile);
            if standing_remembered(&profile, &look) != Standing::SignedOut {
                WAITS
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .retain(|w| w.profile.name != profile.name || w.profile.env != profile.env);
                told();
                return;
            }
        }
    })
}

/// The person put the wait for `name` in the project at `root` away.
pub fn dismiss(root: &Path, name: &str) {
    WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|w| !(w.root == root && w.profile.name == name));
}

/// A harness's own sign-in, ready to run in a terminal the person drives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    /// The program, found where a chat's is.
    pub program: String,
    /// Its own sign-in words.
    pub args: Vec<String>,
    /// The profile's folder variables, set over the terminal's own environment.
    pub env: Vec<(String, String)>,
    /// Variables of the terminal's own environment taken away before it runs (`env -u`).
    pub cleared: Vec<String>,
}

/// The `env` a typed line runs through, by its path, so no `env` earlier on the shell's `PATH`
/// is the one that runs.
pub const ENV: &str = "/usr/bin/env";

impl Login {
    /// **The one line a shell runs it by**: `/usr/bin/env [-u VAR]… VAR=<folder> … <program>
    /// <words>`, each word
    /// quoted for the shell, and a line break. `env` sets the folders in every shell a person
    /// may have. A word with a control character, a quote or a backslash in it is refused, so
    /// every shell reads each word as one, and nothing typed can break the line in two. None is
    /// typed on Windows, whose shells have no `env`.
    pub fn line(&self) -> Result<String, String> {
        self.line_on(crate::sandbox::Os::this())
    }

    /// [`Self::line`] on `os`.
    pub fn line_on(&self, os: crate::sandbox::Os) -> Result<String, String> {
        if os == crate::sandbox::Os::Windows {
            return Err(format!(
                "purlis types no sign-in into a shell on Windows yet. Sign in with {} {} in \
                 your own terminal, with the profile's folder set.",
                self.program,
                self.args.join(" ")
            ));
        }
        let mut words = vec![ENV.to_owned()];
        for name in &self.cleared {
            words.extend(["-u".to_owned(), name.clone()]);
        }
        words.extend(self.env.iter().map(|(n, v)| format!("{n}={v}")));
        words.push(self.program.clone());
        words.extend(self.args.iter().cloned());
        if let Some(bad) = words
            .iter()
            .find(|w| w.chars().any(|c| c.is_control() || c == '\'' || c == '\\'))
        {
            return Err(format!(
                "{} has a control character, a quote or a backslash in it, so purlis types no \
                 sign-in with it. Run purlis profile sign-in in your own terminal instead.",
                crate::shown::short(bad)
            ));
        }
        let quoted: Vec<String> = words
            .iter()
            .map(|w| crate::shellseg::shell_quote(w))
            .collect();
        Ok(format!("{}\n", quoted.join(" ")))
    }
}

/// The sign-in of `profile`, with `program` as its program, to `provider` where its harness
/// serves several: one of [`providers`], and none for a harness that serves one.
fn login_for(profile: &Profile, program: &str, provider: Option<&str>) -> Result<Login, String> {
    let Some(flow) = flow(&profile.kind) else {
        return Err(format!(
            "purlis cannot sign the profile '{}' in: it runs no harness purlis ships.",
            crate::shown::short(&profile.name)
        ));
    };
    let mut args: Vec<String> = flow.login.iter().map(|w| (*w).to_owned()).collect();
    match (providers(&profile.kind), provider) {
        ([], None) => {}
        ([], Some(_)) => {
            return Err(format!(
                "{} signs in to its own plan alone, so no provider is named.",
                profile.kind
            ));
        }
        (plans, Some(id)) if plans.iter().any(|p| p.provider == id) => {
            args.extend([flow.provider_flag.to_owned(), id.to_owned()]);
        }
        (plans, _) => {
            let named: Vec<String> = plans
                .iter()
                .map(|p| format!("{} ({})", p.title, p.provider))
                .collect();
            return Err(format!(
                "{} signs a plan in to the provider you choose: {}. A Claude plan is never \
                 signed in through it.",
                profile.kind,
                named.join(" or ")
            ));
        }
    }
    Ok(Login {
        program: program.to_owned(),
        args,
        env: profile.env.clone(),
        cleared: Vec::new(),
    })
}

/// **The sign-in of `record`**, one of this machine's profiles: its harness's own, in its
/// folder, run by `pinned` where this machine installed the version it is pinned to (a runner's,
/// [`super::installs::on_this_runner`], #4007), else by the harness found on `PATH`. Refused for
/// a folder adopted in place, which is signed in by the person's own sign-in, and for a profile
/// whose harness purlis cannot find.
pub fn login_of(
    record: &MachineProfile,
    provider: Option<&str>,
    pinned: Option<&Path>,
) -> Result<Login, String> {
    let name = crate::shown::short(&record.name);
    if record.adopted.is_some() && record.kept_for.is_none() {
        return Err(format!(
            "'{name}' is {}'s own folder on {}, signed in by its own sign-in, so purlis has \
             nothing to sign in.",
            record.title,
            super::this_device()
        ));
    }
    let profile = profile_of(record)
        .filter(|profile| flow(&profile.kind).is_some())
        .ok_or_else(|| {
            format!("purlis cannot sign the profile '{name}' in: it runs no harness purlis ships.")
        })?;
    let program = match pinned {
        Some(program) => program.display().to_string(),
        None => {
            let home = crate::profiles::home().unwrap_or_default();
            let argv = crate::profiles::expanded_command(&profile, &home);
            crate::programs::resolve_argv(&argv)
                .map_err(|gone| format!("{} Nothing was signed in.", gone.said()))?
                .into_iter()
                .next()
                .ok_or_else(|| format!("the profile '{name}' names no program."))?
        }
    };
    login_for(&profile, &program, provider)
}

/// Forgets what the harness of `record`, one of this machine's profiles, said of it, so it is
/// asked again: what Sign in does, in the window and on the command line.
pub fn forget_of(record: &MachineProfile) {
    if let Some(profile) = profile_of(record) {
        forget(&profile);
    }
}

/// **The profile a chat on `record` starts on**: the one purlis made, or, for a runner's own
/// default folder kept for its desktop's adopted one (#2049), the runner's built-in, which
/// reaches that folder with no variable set. `None` for any other folder adopted in place.
pub fn profile_of(record: &MachineProfile) -> Option<Profile> {
    if record.adopted.is_none() {
        return crate::profiles::of_record(record);
    }
    record.kept_for.as_ref()?;
    crate::profiles::builtins()
        .into_iter()
        .find(|builtin| builtin.name == record.name && builtin.kind == record.harness)
}

/// Whether `record`, one of this machine's profiles, is signed in, as its harness says: what
/// the window and `purlis profile sign-in --check` say. `None` for a folder adopted in place,
/// which is signed in by the person's own sign-in and never asked.
pub fn standing_of(record: &MachineProfile, look: &dyn Look) -> Option<Standing> {
    asked_of(record, look).map(|profile| standing_remembered(&profile, look))
}

/// **The profile whether `record` is signed in is asked of, through `look`**: [`profile_of`],
/// its program the pinned version `look` installed for it where it has one (#4007).
pub(super) fn asked_of(record: &MachineProfile, look: &dyn Look) -> Option<Profile> {
    let mut profile = profile_of(record)?;
    if let Some(program) = look.pinned_program(record) {
        let program = program.display().to_string();
        match profile.command.first_mut() {
            Some(first) => *first = program,
            None => profile.command.push(program),
        }
    }
    Some(profile)
}

pub mod driven;

#[cfg(test)]
mod tests;
