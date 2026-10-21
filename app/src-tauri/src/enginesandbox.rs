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
//! The window's alone (`ui::WINDOW_ONLY`): only the person chooses which program runs as their
//! browsers' engine.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use purlis_core::browsers::sandboxing::{self, NO_ENGINE_SANDBOX};

/// What a chat is told where its browser cannot start for the engine's sandbox.
pub const NO_BROWSER_HERE: &str = "A browser is not available on this machine: purlis cannot \
     start its browser engine safely here, and the person has been shown how to fix that. Don't \
     retry; go on with what needs no browser, and tell the person if the task needs one.";

/// What the person is told where a browser they open cannot start for the engine's sandbox.
pub const NOT_STARTED_FOR_THE_PERSON: &str = "Browsers cannot start on this machine yet: the Notice about the browser engine says how to fix it.";

/// The event that says a browser's start was refused for the engine's sandbox: the window shows
/// the Notice.
pub const ENGINE_SANDBOX_NEEDED: &str = "engine-sandbox-needed";

/// Whether a start was refused for the engine's sandbox since this launch, and not answered.
static NEEDED: AtomicBool = AtomicBool::new(false);

/// What tells the window, set as the app starts.
static TELLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Tells the window with `tell` each time a start is refused for the engine's sandbox.
pub fn telling(tell: Box<dyn Fn() + Send + Sync>) {
    let _ = TELLER.set(tell);
}

/// Whether the host's `why` for a start it refused is the engine's sandbox.
pub fn is_no_sandbox(why: &str) -> bool {
    why.contains(NO_ENGINE_SANDBOX)
}

/// A start was refused for the engine's sandbox: the Notice stands until it is answered.
pub fn refused() {
    NEEDED.store(true, Ordering::SeqCst);
    if let Some(tell) = TELLER.get() {
        tell();
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
    /// This machine's own Chrome, where one is installed.
    pub chrome: Option<String>,
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
    }
}

/// **The Notice, while a start was refused for the engine's sandbox** since this launch.
#[tauri::command]
#[specta::specta]
pub async fn engine_sandbox() -> Result<Option<EngineSandboxShown>, String> {
    crate::off_the_window("reading why browsers cannot start", notice).await
}

/// [`engine_sandbox`]'s work: the Notice for this machine's engine and its Chrome, while a start
/// was refused for the engine's sandbox.
pub fn notice() -> Result<Option<EngineSandboxShown>, String> {
    if !NEEDED.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let data = purlis_core::datahome::root()
        .ok_or_else(|| "There is no data home on this machine.".to_owned())?;
    Ok(Some(shown(
        &purlis_core::browsers::engines_in(&data),
        purlis_core::browsers::machine_chrome(),
    )))
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
