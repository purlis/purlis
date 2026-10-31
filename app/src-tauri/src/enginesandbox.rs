// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **When the browser engine cannot make its sandbox here** (#2835, ruled 2026-10-20).
//!
//! On Ubuntu 23.10 and later the pinned engine makes its sandbox only under an AppArmor profile
//! that names it (`purlis_core::browsers::sandboxing`). The `.deb` installs one. Where none
//! names this machine's engine, the session host refuses every browser's start in
//! [`NO_ENGINE_SANDBOX`]'s words, and purlis never runs the engine without its sandbox:
//!
//! - **the chat is told a browser is not available on this machine** ([`NO_BROWSER_HERE`]),
//!   never what the engine wrote as it ended;
//! - **the person is shown a Notice** ([`engine_sandbox`]) with the two ways out: the command
//!   that adds the profile, which they run themselves since purlis never runs anything as root,
//!   and, where this machine has its own Google Chrome or Chromium, using it for every browser
//!   ([`use_this_machines_chrome`], ADR 0042's choice, kept in `machine.json`).
//!
//! **A runner's engine is its own machine's** (#2990). A Docker pool's runner made before #3298
//! runs a seccomp profile that refuses the engine's sandbox, so the runner's Notice says to
//! remove the pool and add it again too: the engine's refusal reads the same whichever wall
//! stopped it, so the Notice names both ways out. Where a pool's runner refuses a browser's
//! start for its engine's sandbox and the desktop relays the refusal, the Notice names that
//! runner and offers the command to run there, which installs the profile the `.deb` installs
//! ([`sandboxing::install_shipped_command`]): the window does not know the runner's own data
//! folder, and that profile names the engine in any user's default one. This machine's Chrome
//! is not offered for it, and its chat is told a browser is not available on that machine.
//!
//! The window's alone (`ui::WINDOW_ONLY`): only the person chooses which program runs as their
//! browsers' engine.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use purlis_core::browsers::sandboxing::{self, NO_ENGINE_SANDBOX};

/// What a chat is told where its browser cannot start for the engine's sandbox.
pub const NO_BROWSER_HERE: &str = "A browser is not available on this machine: purlis cannot \
     start its browser engine safely here, and the person has been shown how to fix that. Don't \
     retry; go on with what needs no browser, and tell the person if the task needs one.";

/// What a chat on pool `pool`'s runner is told where its browser cannot start there for the
/// engine's sandbox (#2990).
pub fn no_browser_on(pool: &str) -> String {
    format!(
        "A browser is not available on the runner of pool {pool}, where this chat runs: purlis \
         cannot start its browser engine safely there, and the person has been shown how to fix \
         that. Don't retry; go on with what needs no browser, and tell the person if the task \
         needs one."
    )
}

/// What the person is told where a browser they open cannot start for the engine's sandbox.
pub const NOT_STARTED_FOR_THE_PERSON: &str = "Browsers cannot start on this machine yet: the Notice about the browser engine says how to fix it.";

/// The event that says a browser's start was refused for the engine's sandbox: the window shows
/// the Notice.
pub const ENGINE_SANDBOX_NEEDED: &str = "engine-sandbox-needed";

/// Whether a start was refused for the engine's sandbox since this launch, and not answered.
static NEEDED: AtomicBool = AtomicBool::new(false);

/// The pools whose runner refused a start for its engine's sandbox since this launch (#2990).
static ON_RUNNERS: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// What tells the window, set as the app starts.
static TELLER: OnceLock<Box<dyn Fn(EngineMachine) + Send + Sync>> = OnceLock::new();

/// Tells the window with `tell` each time a start is refused for the engine's sandbox, or a
/// runner that refused one starts a browser again, naming the machine whose engine it is.
pub fn telling(tell: Box<dyn Fn(EngineMachine) + Send + Sync>) {
    let _ = TELLER.set(tell);
}

/// **The machine whose engine a Notice is about** (#2990): this one, or a pool's runner.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EngineMachine {
    /// The machine the window runs on.
    Here,
    /// The runner of pool `pool`.
    Runner { pool: String },
}

impl EngineMachine {
    fn of(pool: Option<&str>) -> Self {
        match pool {
            None => EngineMachine::Here,
            Some(pool) => EngineMachine::Runner {
                pool: pool.to_owned(),
            },
        }
    }
}

fn tell(machine: EngineMachine) {
    if let Some(tell) = TELLER.get() {
        tell(machine);
    }
}

/// Whether the host's `why` for a start it refused is the engine's sandbox.
pub fn is_no_sandbox(why: &str) -> bool {
    why.contains(NO_ENGINE_SANDBOX)
}

/// A start was refused for the engine's sandbox on this machine, or, with `pool`, on that pool's
/// runner (#2990): the Notice for that machine stands until it is answered.
pub fn refused_on(pool: Option<&str>) {
    match pool {
        None => NEEDED.store(true, Ordering::SeqCst),
        Some(pool) => {
            ON_RUNNERS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(pool.to_owned());
        }
    }
    tell(EngineMachine::of(pool));
}

/// **A browser started on pool `pool`'s runner** (#2990): its engine makes its sandbox there now,
/// so the runner's Notice no longer holds, and goes.
pub fn started_on(pool: &str) {
    let held = ON_RUNNERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(pool);
    if held {
        tell(EngineMachine::of(Some(pool)));
    }
}

/// What the Notice says and offers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct EngineSandboxShown {
    /// The sentence.
    pub says: String,
    /// The command that adds the profile, which the person runs themselves: none for a data
    /// folder whose path holds a control character.
    pub command: Option<String>,
    /// This machine's own Chrome, where one is installed: never for a runner's.
    pub chrome: Option<String>,
    /// The machine whose engine it is (#2990).
    pub machine: EngineMachine,
}

/// What the Notice shows for the pinned engines in `engines`, and `chrome`, this machine's own.
pub fn shown(engines: &Path, chrome: Option<PathBuf>) -> EngineSandboxShown {
    let way = if chrome.is_some() {
        "Add purlis's AppArmor profile for it with the command, or use this machine's Chrome for \
         every browser."
    } else {
        "Add purlis's AppArmor profile for it with the command."
    };
    EngineSandboxShown {
        says: format!(
            "Browsers cannot start here: this machine lets the browser engine make its sandbox \
             only under a profile that names it, and purlis never runs it without. {way}"
        ),
        command: sandboxing::install_command(engines),
        chrome: chrome.map(|path| path.display().to_string()),
        machine: EngineMachine::Here,
    }
}

/// **What the Notice shows for pool `pool`'s runner** (#2990): it names the runner, and offers
/// the command to run there.
pub fn shown_on_runner(pool: &str) -> EngineSandboxShown {
    EngineSandboxShown {
        says: format!(
            "Browsers cannot start on the runner of pool {pool}: its browser engine cannot make \
             its sandbox there, and purlis never runs the engine without it. A Docker pool's \
             runner made by an earlier purlis does not let the engine make it: remove the pool \
             and add it again. A pool on a team's own image is made so only once that image \
             carries a purlis as new as this one, which lets the browser engine alone make \
             namespaces on the runner: rebuild the image on it, then remove the pool and add it \
             again. If that runner's Linux allows the sandbox only under an AppArmor \
             profile, as Ubuntu 23.10 and later do, run the command on the runner, not on this \
             machine."
        ),
        command: Some(sandboxing::install_shipped_command()),
        chrome: None,
        machine: EngineMachine::of(Some(pool)),
    }
}

/// **The Notices, while a start was refused for the engine's sandbox** since this launch: this
/// machine's, and one for each pool's runner that refused one (#2990).
#[tauri::command]
#[specta::specta]
pub async fn engine_sandbox() -> Result<Vec<EngineSandboxShown>, String> {
    crate::off_the_window("reading why browsers cannot start", notice).await
}

/// [`engine_sandbox`]'s work: the Notice for this machine's engine and its Chrome, while a start
/// was refused for the engine's sandbox here, and each runner's. This machine's is left out
/// where it has no data home to name its engine by; a runner's still stands.
pub fn notice() -> Result<Vec<EngineSandboxShown>, String> {
    let mut notices = Vec::new();
    if NEEDED.load(Ordering::SeqCst) {
        match purlis_core::datahome::root() {
            Some(data) => notices.push(shown(
                &purlis_core::browsers::engines_in(&data),
                purlis_core::browsers::machine_chrome(),
            )),
            None => tracing::warn!(
                "purlis: no Notice about this machine's browser engine: it has no data home"
            ),
        }
    }
    notices.extend(
        ON_RUNNERS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|pool| shown_on_runner(pool)),
    );
    Ok(notices)
}

/// **Every browser starts on this machine's own Chrome from now on** (ADR 0042's choice): the
/// person's press on the Notice. The next browser call starts on it.
#[tauri::command]
#[specta::specta]
pub async fn use_this_machines_chrome() -> Result<(), String> {
    crate::off_the_window("choosing this machine's Chrome", || {
        if purlis_core::browsers::machine_chrome().is_none() {
            return Err(
                "Nothing was changed: this machine has no Google Chrome or Chromium.".into(),
            );
        }
        let config = purlis_core::machine::config_root()
            .ok_or_else(|| "There is no config home on this machine to keep it in.".to_owned())?;
        purlis_core::machine::set_machine_chrome(&config, true)?;
        NEEDED.store(false, Ordering::SeqCst);
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests;
