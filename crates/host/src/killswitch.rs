// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The kill switch as the session host holds it (#2001, ADR 0071, amended 2026-10-14): the
//! authority on whether this user's agents are stopped, for every program the host runs.
//!
//! **Kept in the host's own folder**, as one file, [`KILL_SWITCH`]: the word `stopped` or
//! `armed`, `0600`, written whole through the folder's held descriptor
//! (`purlis_same_user::PrivateDirectory`), and read back only when it is a regular file, not a
//! link, this user's and nobody else's (`purlis_same_user::read_private_file`). The folder is
//! the one the host keeps its credentials in, which every chat's sandbox denies reading and
//! writing, so the agents the switch stops cannot reach it.
//!
//! **It fails closed.** No file is a host never stopped. A file that cannot be read, or says
//! anything else, holds every agent stopped, and the host says why, in its log and to each start
//! it refuses. Only the window's re-arm moves it back.
//!
//! **A stop holds even when it cannot be kept**: the host stops in memory whatever the disk
//! says, and the answer says the stop will not outlive the host. A re-arm that cannot be kept
//! does not happen.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use purlis_session_protocol::session::KillSwitch;

/// The switch's file in the host's own folder.
pub const KILL_SWITCH: &str = "kill-switch";

/// **What tells the host a stop was written elsewhere** (#2190): `purlis stop --all` throws the
/// switch by writing ADR 0071's two files in the config home, and the binary that builds the
/// host reads them through this, as it composes a start through `Compose`, so the host links no
/// core.
pub trait StopsElsewhere: Send + Sync + 'static {
    /// The newest stop the files hold, named so that each stop written is told from the one
    /// before; `None` while they say every agent may run.
    fn last_stop(&self) -> Option<String>;

    /// **A runner's desktop stopped or re-armed this host over its link** (#2051, ADR 0071 as
    /// amended): written where a stop elsewhere is written, by the link, and the event log's
    /// entry recording it with the desktop's `device`, for the host to record. Nothing, for a
    /// binary that keeps no such files.
    fn link_switched(&self, rearm: bool, device: &str) -> Option<serde_json::Value> {
        let _ = (rearm, device);
        None
    }
}

/// What a host is given when its binary gives it nothing to read: it hears only `stop_all`.
pub struct NoStopsElsewhere;

impl StopsElsewhere for NoStopsElsewhere {
    fn last_stop(&self) -> Option<String> {
        None
    }
}

/// How often the host looks for a stop written elsewhere when nothing else has made it look:
/// as often as the window's own watch looks at the files (D-88l).
pub const LOOK_EVERY: std::time::Duration = std::time::Duration::from_secs(3);

const STOPPED: &str = "stopped";
const ARMED: &str = "armed";

/// Whether every agent is stopped, and where that is kept.
pub(crate) struct Switch {
    /// The host's own folder, where [`KILL_SWITCH`] is kept; none for a host kept in memory
    /// only (a test's).
    kept: Option<PathBuf>,
    held: Mutex<Held>,
}

#[derive(Default)]
struct Held {
    stopped: bool,
    /// What the host has to say of the switch: a state it could not read, or a stop it could
    /// not keep.
    why: Option<String>,
}

impl Switch {
    /// A switch nobody has thrown, kept in memory only.
    pub(crate) fn in_memory() -> Switch {
        Switch {
            kept: None,
            held: Mutex::default(),
        }
    }

    /// The switch kept in `folder`, as it was left there: stopped when it says so, and when it
    /// cannot be read.
    pub(crate) fn kept_in(folder: &Path) -> Switch {
        let path = folder.join(KILL_SWITCH);
        let held = match purlis_same_user::read_private_file(&path) {
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Held::default(),
            Ok(said) if said.trim() == ARMED => Held::default(),
            Ok(said) if said.trim() == STOPPED => Held {
                stopped: true,
                why: None,
            },
            Ok(_) => unreadable("it says neither stopped nor armed"),
            Err(why) => unreadable(&why.to_string()),
        };
        if let Some(why) = &held.why {
            crate::say(why);
        }
        Switch {
            kept: Some(folder.to_path_buf()),
            held: Mutex::new(held),
        }
    }

    /// The host's own folder the switch is kept in, where it is kept.
    pub(crate) fn folder(&self) -> Option<&Path> {
        self.kept.as_deref()
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.held().stopped
    }

    /// The switch as it stands.
    pub(crate) fn state(&self) -> KillSwitch {
        let held = self.held();
        KillSwitch {
            stopped: held.stopped,
            why: held.why.clone(),
            ..Default::default()
        }
    }

    /// Why a start is refused while stopped.
    pub(crate) fn refusal(&self) -> String {
        let said = "every agent is stopped by the kill switch; nothing starts until it is \
                    re-armed in purlis's window";
        match &self.held().why {
            Some(why) => format!("{said} ({why})"),
            None => said.to_owned(),
        }
    }

    /// Stops every agent: in memory at once, then kept. Answers the switch as it stands, which
    /// says so when the stop could not be kept.
    pub(crate) fn stop(&self) -> KillSwitch {
        let mut held = self.held();
        held.stopped = true;
        if let Err(why) = self.keep(STOPPED) {
            let why = format!(
                "every agent is stopped, but the stop could not be kept ({why}), so a session \
                 host started again will not know it"
            );
            crate::say(&why);
            held.why = Some(why);
        }
        KillSwitch {
            stopped: true,
            why: held.why.clone(),
            ..Default::default()
        }
    }

    /// Lets programs start again: kept first, so a re-arm that cannot be kept does not happen.
    /// The window's alone, which the session protocol's grants decide before this is asked.
    pub(crate) fn rearm(&self) -> Result<KillSwitch, String> {
        let mut held = self.held();
        self.keep(ARMED).map_err(|why| {
            format!("every agent is still stopped: the re-arm could not be kept ({why})")
        })?;
        *held = Held::default();
        Ok(KillSwitch {
            stopped: false,
            why: None,
            ..Default::default()
        })
    }

    fn keep(&self, word: &str) -> std::io::Result<()> {
        let Some(folder) = &self.kept else {
            return Ok(());
        };
        purlis_same_user::PrivateDirectory::open(folder)?
            .replace_file(KILL_SWITCH, format!("{word}\n").as_bytes())
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn unreadable(why: &str) -> Held {
    Held {
        stopped: true,
        why: Some(format!(
            "the kill switch's state could not be read ({why}), so every agent is held stopped \
             until purlis's window re-arms it"
        )),
    }
}
