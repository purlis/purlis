//! "Keep this Mac awake while chats are running" (iOS design §6.4, D10): while the setting is
//! on and a chat is `working` or `needs_you`, a child holds the operating system's idle
//! assertion; it is released when none is, when the setting turns off, and when the app
//! quits. A child, not an FFI call: the assertion API needs `unsafe`, which AGENTS.md keeps
//! to two audited blocks. The child's stdin is a pipe this process holds, and the child ends
//! when the pipe closes, so a release, a drop and a crash all let the machine sleep: on macOS
//! `caffeinate -w <pid>` is bound to this process as well; on Linux `systemd-inhibit` holds
//! idle for as long as the `cat` under it reads the pipe.

use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Whether the assertion is wanted: the setting, and a chat that is working or waiting on
/// the person. Off either way otherwise, so an idle desktop sleeps as it always did.
pub fn wanted(keep_awake: bool, any_working: bool) -> bool {
    keep_awake && any_working
}

/// The program that holds the assertion while it runs, and while its stdin is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program(Vec<String>);

impl Program {
    /// The platform's own: `caffeinate -i -w <pid>` on macOS, bound to this process;
    /// `systemd-inhibit … cat` on Linux, holding idle until the pipe `cat` reads closes.
    pub fn real() -> Program {
        let pid = std::process::id().to_string();
        if cfg!(target_os = "macos") {
            Program(vec!["caffeinate".into(), "-i".into(), "-w".into(), pid])
        } else {
            Program(vec![
                "systemd-inhibit".into(),
                "--what=idle".into(),
                "--who=purlis".into(),
                "--why=chats are running".into(),
                "cat".into(),
            ])
        }
    }

    /// A child that holds nothing and lives while its stdin is open: for a test of the holder.
    #[cfg(test)]
    pub fn test_double() -> Program {
        Program(vec!["cat".into()])
    }

    pub fn argv(&self) -> &[String] {
        &self.0
    }

    fn spawn(&self) -> std::io::Result<Child> {
        let mut command = Command::new(&self.0[0]);
        command
            .args(&self.0[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        purlis_core::forklock::spawn(&mut command)
    }
}

/// Holds the assertion through the child while it is wanted.
pub struct Holder {
    program: Program,
    child: Mutex<Option<Child>>,
}

impl Holder {
    pub fn new(program: Program) -> Holder {
        Holder {
            program,
            child: Mutex::new(None),
        }
    }

    fn child(&self) -> MutexGuard<'_, Option<Child>> {
        self.child.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hold or release, as `wanted` says: the same answer twice changes nothing, and a child
    /// that died on its own is started again when still wanted.
    pub fn set(&self, wanted: bool) {
        let mut child = self.child();
        let died = child
            .as_mut()
            .is_some_and(|running| matches!(running.try_wait(), Ok(Some(_))));
        match (wanted, child.as_mut()) {
            (true, None) => *child = self.spawn(),
            (true, Some(_)) if died => *child = self.spawn(),
            (false, Some(running)) => {
                // The pipe closes first, which is what ends a child that reads it; the kill is
                // for one that does not.
                drop(running.stdin.take());
                let _ = running.kill();
                let _ = running.wait();
                *child = None;
            }
            _ => {}
        }
    }

    fn spawn(&self) -> Option<Child> {
        match self.program.spawn() {
            Ok(child) => Some(child),
            Err(why) => {
                tracing::warn!(
                    "purlis: keep-awake could not start {} ({why})",
                    self.program.argv()[0]
                );
                None
            }
        }
    }

    /// Whether a child holds the assertion now.
    #[cfg(test)]
    pub fn held(&self) -> bool {
        self.child().is_some()
    }

    /// The child's pid, while one runs: for a test of idempotence.
    #[cfg(test)]
    pub fn child_pid(&self) -> Option<u32> {
        self.child().as_ref().map(Child::id)
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        self.set(false);
    }
}
