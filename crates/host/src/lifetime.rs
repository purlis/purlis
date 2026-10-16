// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How long the window's session host lives** (ADR 0068 §2, #2006).
//!
//! The window says which way it goes. A quit says so (`quit`), and the host ends every program
//! as a quit and stops ([`Hosting::until_it_quits`]). A window whose link ends without a word
//! crashed, or was killed: the host keeps every program, its project's hook channel, its kept
//! board, its confinements and its saving for the **grace period** (ruled, V22a: ten minutes),
//! so a window that starts within it takes them back where they are.
//!
//! **Each project waits for the grace period on its own.** A project no link holds is let go of
//! once the grace period has passed since its last hold went, as a quit would: its programs end
//! all at once, each kept as ended by a quit, then its confinements, its saving, its hook
//! channel and its board go. So a window that comes back and does not open a project does not
//! keep that project's agents running unseen, and a project whose programs have all ended is
//! let go of too (#2262), after a window had the grace period to read how each ended.
//!
//! **The host stops** once no window is linked and either nothing is left that a window could
//! come back for, or the grace period has passed since the last window went. A host whose window
//! has not linked yet waits for it, until the window that started it goes
//! (`starter_went`, the end of the host's standard input, which the window holds).
//!
//! Nothing polls: the host sleeps until the next grace period ends, or until a window links or
//! goes, or a project is held or let go of. A host started as `purlis serve` lives until it is
//! ended, as a runner's does: chats on a runner keep running with the desktop away.

use std::time::Duration;

use crate::hosting::Hosting;

/// The grace period after a window went without a quit (ruled, V22a).
pub const GRACE: Duration = Duration::from_secs(10 * 60);

impl Hosting {
    /// Lives as the window's host does, with a grace period of `grace`, until there is nothing
    /// more to wait for, or the window quits. `starter_went` resolves once the window that
    /// started this host has gone. What a project let go of here ends, it ends off the runtime.
    pub async fn while_a_window_may_come_back(
        &self,
        grace: Duration,
        starter_went: impl Future<Output = ()>,
    ) {
        tokio::pin!(starter_went);
        let mut starter_gone = false;
        loop {
            let now = std::time::Instant::now();
            let hosting = self.clone();
            let _ = tokio::task::spawn_blocking(move || {
                hosting.let_go_of_what_nobody_came_back_for(grace, now);
            })
            .await;
            if self.nobody_to_come_back_for(grace, now) {
                return;
            }
            let next = self.next_grace_ends(grace);
            let until_then = async {
                match next {
                    Some(then) => tokio::time::sleep_until(then.into()).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                () = &mut starter_went, if !starter_gone => {
                    starter_gone = true;
                    self.its_starter_went();
                }
                () = self.changed() => {}
                () = until_then => {}
                () = self.until_it_quits() => return,
            }
        }
    }
}
