// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill change at the person's terminal on an org's project, audited by the session host**
//! (SK-11a, #6030; ADR 0100 as amended 2026-11-10). An org's project audits every install, update
//! and removal of a skill installed from a source before it is made, and only the session host
//! and the window write the event log. So this asks the host, as `terminal`, to write the change
//! (session protocol 1.82), and the change is made only once the host says it is durable.
//!
//! **Nothing a chat does writes such an entry.** A command run inside a chat is refused here,
//! before anything is asked: the courtesy. The enforcement is the host's, which refuses a peer
//! inside a chat every scope, and the sandbox's, which denies a chat the `terminal` credential
//! and the host's socket (ADR 0068 §5). The terminal hands the host the change, never an entry:
//! the host composes it, and says it was the person at the terminal.

use std::path::Path;

use purlis_core::skills::source::policy::Audited;

/// What a command in a chat is told.
const IN_A_CHAT: &str = "this project audits every skill install, update and removal, and a \
     chat cannot write the audit: ask the person to run it in their own terminal, or to make the \
     change in the Skill studio";

/// What the core's audit hook is for `purlis skill` at `root`: asked only on an org's project,
/// once per change, before it is made.
pub(crate) fn hook(root: &Path) -> impl FnMut(&Audited) -> Result<(), String> + '_ {
    move |audited| audit(root, audited)
}

/// Asks the session host to audit `audited`, and waits until it is durable.
fn audit(root: &Path, audited: &Audited) -> Result<(), String> {
    audit_in(&purlis_core::envvar::var, root, audited)
}

/// [`audit`], with the environment read through `env`: refused inside a chat before the host is
/// looked for.
fn audit_in(
    env: &dyn Fn(&str) -> Option<String>,
    root: &Path,
    audited: &Audited,
) -> Result<(), String> {
    if crate::gradelink::in_a_chat(env) {
        return Err(IN_A_CHAT.to_owned());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|why| format!("the session host could not be asked ({why})"))?;
    runtime.block_on(asking(root, audited))
}

#[cfg(not(unix))]
async fn asking(_root: &Path, _audited: &Audited) -> Result<(), String> {
    Err("the session host runs on macOS and Linux only".to_owned())
}

#[cfg(unix)]
async fn asking(root: &Path, audited: &Audited) -> Result<(), String> {
    use crate::gradelink::NotLinked;
    use purlis_session_protocol::session::{Client, UNKNOWN_COMMAND};
    // Said inside the core's sentence, which names what was not done: lowercase, no stop.
    let link = crate::gradelink::linked_as_terminal()
        .await
        .map_err(|not| match not {
            NotLinked::NoHost => NO_HOST.to_owned(),
            NotLinked::Refused(why) => format!("the session host refused this terminal ({why})"),
        })?;
    let (client, _views, _events) = Client::new(link);
    let path = root.display().to_string();
    client
        .audit_skill_change(&path, audited.told())
        .await
        .map_err(|why| match why.refusal() {
            Some(refusal) if refusal.code == UNKNOWN_COMMAND => {
                "the running purlis is older than this command and cannot audit a change made at \
                 a terminal: update it, or make the change in the Skill studio"
                    .to_owned()
            }
            Some(refusal) => refusal.why.clone(),
            None => format!("the session host did not answer ({why})"),
        })
}

/// What a terminal is told with no session host to ask.
#[cfg(unix)]
const NO_HOST: &str = "no purlis session host is running to write it: open purlis, or start \
     a host with `purlis serve`, then run this again";

#[cfg(test)]
mod tests;
