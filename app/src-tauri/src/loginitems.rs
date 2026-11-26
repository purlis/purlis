// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What keeps the session host running, and the login items that keep it** (FD-17, #655;
//! ADR 0068 §2 and §3, X24), as this app reads them: the core decides
//! (`purlis_core::loginitem`), and this gives it the app's own binary, its log folder and this
//! machine's places.
//!
//! **Read at each decision, never cached**: as the window starts (who supervises the host), at
//! each quit (to the background or not) and at the exit (whether the login items stay).

use std::path::PathBuf;
use std::sync::Arc;

use purlis_core::loginitem::{self, Job, Manager, Reason, Supervisor};

/// **A development build's stand-in for a trigger**, for the person's check on hardware before
/// any trigger exists: its value is the trigger's name. A release build never reads it.
pub const STAND_IN: &str = "PURLIS_KEPT_RUNNING_FOR";

/// What keeps the host running on this machine now.
pub fn reasons() -> Vec<Reason> {
    let mut reasons = purlis_core::machine::config_root()
        .map(|config| loginitem::reasons_here(&config))
        .unwrap_or_default();
    if cfg!(debug_assertions)
        && let Some(name) = purlis_core::envvar::var(STAND_IN).filter(|name| !name.is_empty())
    {
        reasons.push(Reason::Trigger { name });
    }
    reasons
}

/// Why the host keeps running, in the window's words, or `None`.
pub fn kept_for() -> Option<String> {
    loginitem::kept_for(&reasons())
}

/// **This platform's service manager, and whether it runs the host this launch**: only while
/// something keeps the host running and the chats run in the host (`hostedsessions`); a window
/// whose chats run in its own process supervises its host itself. `None` where there is no
/// manager purlis may use.
pub fn manager() -> Option<(Arc<dyn Manager>, bool)> {
    let binary = std::env::current_exe().ok()?;
    let places = loginitem::places_here()?;
    let runtime = purlis_core::envvar::var("XDG_RUNTIME_DIR").map(PathBuf::from);
    let systemd_user = loginitem::systemd_user_here(runtime.as_deref());
    let os = loginitem::Os::this();
    let (kind, manager) = loginitem::manager_on(
        os,
        systemd_user,
        places,
        Job {
            program: binary.clone(),
            args: vec![purlis_host::SERVE_AS_A_LOGIN_ITEM.to_owned()],
        },
        Job {
            program: binary,
            args: vec![crate::lifecycle::IN_THE_BACKGROUND.to_owned()],
        },
        crate::sessionhost::managed::log_here()?,
        loginitem::runs_for_real(),
    )?;
    let runs_it = runs_the_host(loginitem::supervisor(&reasons(), os, systemd_user), kind);
    Some((manager, runs_it))
}

/// Whether the manager of kind `manager` runs the host: the supervisor chosen is that manager.
fn runs_the_host(chosen: Supervisor, manager: Supervisor) -> bool {
    chosen == manager && chosen != Supervisor::Window
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manager_runs_the_host_only_when_it_is_chosen() {
        assert!(runs_the_host(Supervisor::Launchd, Supervisor::Launchd));
        assert!(runs_the_host(
            Supervisor::SystemdUser,
            Supervisor::SystemdUser
        ));
        assert!(!runs_the_host(Supervisor::Window, Supervisor::Launchd));
        assert!(!runs_the_host(Supervisor::Launchd, Supervisor::SystemdUser));
    }
}
