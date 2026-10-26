// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A denial as the sandbox itself reported it** (ADR 0093, "A block is learned from the
//! sandbox first"): one record of what it refused, for which process, on which exact target.
//!
//! The kernel knows what it refused whatever the refused program went on to print or end
//! with: a pipe that dropped its status, a tool that blamed its own token, a build runner that
//! swallowed the error. So a record of the sandbox's own is read before anything a program
//! said, and names the target a grant would name.
//!
//! # Who reports one
//!
//! - **The kernel** ([`Witness::Kernel`]): macOS's Seatbelt writes `Sandbox: <process>(<pid>)
//!   deny(1) <operation> <target>` to the unified log for each refusal. Claude Code reads its
//!   own commands' entries there and appends them to the command's result.
//! - **A proxy** ([`Witness::Proxy`]): Claude Code's says a connection it refused as `deny
//!   network-outbound <host>:<port> (<reason>)`, with no process. Its request-level lines
//!   (`deny http-request …`, `deny socks-tunnel …`) are a request its policy refused, which no
//!   host's Allow mends, and are not read.
//!
//! purlis's own proxy beside a chat it wraps reports its refusals to the app directly
//! (`crate::sandbox::egress::Refusals`), and is the model this follows.
//!
//! # Where one is read from
//!
//! **The report a harness appended to a command's result** ([`reported`]): Claude Code's
//! `<sandbox_violations>` block, the last one, where nothing follows it but space or the
//! harness's own note after it ([`TRAILING_NOTE`]), one paragraph that ends the text. A block
//! with anything else after it is what the command printed.
//!
//! # What a record is not
//!
//! **It only ever adds a name.** A record becomes a [`Block`] by the one sorter every block
//! takes ([`Denial::block`]), and the target it names is judged again as a grant before
//! anything is offered, then needs the person's Allow. Nothing of it is kept but that block.
//! A harness hands its report over in the text of a result, so a program can still print one:
//! what that buys is what any block line buys ([`super::Throttle`]).

use super::{Block, Kind, Operation, Place, target_of};
use std::path::Path;

/// Who said a denial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Witness {
    /// The kernel's sandbox, which names the process it refused.
    Kernel,
    /// An egress proxy, which names the host it refused and no process.
    Proxy,
}

/// One denial, as reported: borrowed from the line it was read from, and never kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Denial<'a> {
    pub witness: Witness,
    /// The process the kernel refused, by the name it gave. None for a proxy's line, and for
    /// a line whose words there are not `name(pid)`.
    pub process: Option<&'a str>,
    /// The operation, in the sandbox's own word: `file-write-create`, `mach-lookup`, ….
    pub operation: &'a str,
    /// What it was refused on, whole: a path, a service's name, a host and port. Empty where
    /// the line names none.
    pub target: &'a str,
}

impl<'a> Denial<'a> {
    /// The denial `line` reports, in the kernel's shape or a proxy's. None for any other line.
    pub fn of_line(line: &'a str) -> Option<Self> {
        if let Some(refused) = Self::of_a_proxy(line) {
            return Some(refused);
        }
        let at = line.find("deny(")?;
        let process = process_of(&line[..at]);
        let after = &line[at..];
        let after = &after[after.find(')')? + 1..];
        let mut words = after.trim().splitn(2, char::is_whitespace);
        let operation = words.next().filter(|word| !word.is_empty())?;
        let target = words.next().unwrap_or("").trim().trim_matches('"');
        Some(Self {
            witness: Witness::Kernel,
            process,
            operation,
            target,
        })
    }

    /// **Claude Code's proxy's line** for a connection it refused (#1631): `deny
    /// network-outbound api.example.com:443 (host is not on the allow list)`. No process and
    /// no `deny(`, and its reason after the host, which is never read.
    fn of_a_proxy(line: &'a str) -> Option<Self> {
        let rest = line.trim().strip_prefix("deny ")?;
        let (operation, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        if !operation.starts_with("network") {
            return None;
        }
        let named = after.split_whitespace().next().unwrap_or("");
        Some(Self {
            witness: Witness::Proxy,
            process: None,
            operation,
            target: if named.starts_with('(') { "" } else { named },
        })
    }

    /// Whether the process refused was purlis itself, under any name it is installed by: a
    /// purlis bug, never something to grant. A proxy names no process, so its line is nobody's.
    pub fn is_purlis_own(&self) -> bool {
        self.process.is_some_and(crate::cliname::is_recognised)
    }

    /// **The block this denial is, and what a grant would name for it**, sorted against
    /// `place`: the operation by the sandbox's word, the kind by the target. A target that is
    /// not a file's (a device class, a kernel setting, this machine's own port) is never read
    /// as a path. purlis's own denial names nothing to grant.
    pub fn block(&self, place: &Place<'_>) -> (Block, Option<String>) {
        self.block_in(place, &std::env::temp_dir())
    }

    /// [`Self::block`], with `temp` the chat's temporary folder ([`super::kind_of_in`]).
    pub fn block_in(&self, place: &Place<'_>, temp: &Path) -> (Block, Option<String>) {
        let operation = Operation::of_seatbelt(self.operation);
        let kind = match operation {
            // The certificate check's own service by its whole name, never a part of one
            // (#1927): a service that only has that name in its own is a system service.
            Operation::Lookup if self.target == crate::sandbox::services::TRUSTD => {
                Kind::CertificateCheck
            }
            Operation::Lookup => Kind::SystemService,
            Operation::Connect if self.target.starts_with('/') => Kind::LocalSocket,
            // This machine's loopback at a port (qw384): a service on this Mac, never a host.
            Operation::Connect
                if crate::sandbox::localport::loopback_port(self.target).is_some() =>
            {
                Kind::LocalService
            }
            Operation::Connect => Kind::Host,
            Operation::Other => Kind::System,
            _ if self.target.is_empty() => Kind::System,
            _ => super::kind_of_in(Path::new(self.target), place, temp),
        };
        let ours = self.is_purlis_own();
        let block = Block {
            operation,
            kind,
            ours,
        };
        let target = (!ours)
            .then(|| target_of(&block, self.target, place))
            .flatten();
        (block, target)
    }
}

/// How Claude Code's note of the connections its agent proxy failed begins: the one thing it
/// writes after a failed command's report, a blank line below it.
const TRAILING_NOTE: &str = "[agent-proxy] ";

/// **The denials in the report a harness appended to `text`**: the lines of the last
/// `<sandbox_violations>` block, where nothing follows it but space or the harness's own note
/// ([`TRAILING_NOTE`], [`ends_the_text`]). None where a block is left open, or has anything
/// else after it.
pub fn reported(text: &str) -> Vec<Denial<'_>> {
    const OPEN: &str = "<sandbox_violations>";
    const CLOSE: &str = "</sandbox_violations>";
    let Some(at) = text.rfind(OPEN) else {
        return Vec::new();
    };
    let inside = &text[at + OPEN.len()..];
    let Some(end) = inside.find(CLOSE) else {
        return Vec::new();
    };
    if !ends_the_text(&inside[end + CLOSE.len()..]) {
        return Vec::new();
    }
    inside[..end].lines().filter_map(Denial::of_line).collect()
}

/// Whether `after`, the text below a report, leaves the report the harness's own: nothing but
/// space, or the harness's note ([`TRAILING_NOTE`]) on a line of its own and as **one paragraph
/// that ends the text**. A note with a blank line and more below it is a report quoted in what
/// a command printed (a log read back), with the rest of that output after it.
fn ends_the_text(after: &str) -> bool {
    let Some(at) = after.find(|c: char| !c.is_whitespace()) else {
        return true;
    };
    let note = after[at..].trim_end();
    after[..at].contains('\n')
        && note.starts_with(TRAILING_NOTE)
        && note.lines().all(|line| !line.trim().is_empty())
}

/// The process a kernel line names before its `deny(`: `cargo` of `Sandbox: cargo(123) `.
/// None when the words there are not `name(pid)`.
fn process_of(before: &str) -> Option<&str> {
    let word = before.split_whitespace().next_back()?;
    let (name, pid) = word.strip_suffix(')')?.rsplit_once('(')?;
    (!name.is_empty() && !pid.is_empty() && pid.chars().all(|c| c.is_ascii_digit())).then_some(name)
}

#[cfg(test)]
#[path = "sandboxblock_denial/tests.rs"]
mod tests;
