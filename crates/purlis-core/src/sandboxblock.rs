// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A sandbox block, sorted** (#1338, spec #1330): what a sandboxed chat's sandbox refused, read
//! from the harness's own report of it, reduced to an operation and the kind of path or host it
//! was about — and nothing else.
//!
//! # Only a sandboxed chat, and the sandbox's own evidence first
//!
//! A tool hook runs outside the chat's sandbox, after the tool came back. It reads anything only
//! in a chat the app started sandboxed (`crate::sandbox::chat_is_sandboxed`): a refusal in a chat
//! that opted out, or by macOS's privacy controls or system integrity protection, is not the
//! sandbox's, and is never shown as one. Then ([`detect`]):
//!
//! - **Claude Code's `<sandbox_violations>` block**, the primary signal, and the sandbox's own
//!   record ([`denial`]): one Seatbelt line per refusal, `cargo(123) deny(1) file-write-create
//!   /path`, naming the process, the operation and the exact target whatever the program
//!   printed or ended with. Only the last block, and only when it ends the text but for the
//!   harness's own note after it, which is where Claude Code appends it. A connection Claude Code's proxy
//!   refused is said there in the proxy's own shape, with no process: `deny network-outbound
//!   host:443 (reason)` (#1631). It names the host to allow, and each host refused is its own
//!   block.
//! - **An egress proxy's refusal**: purlis's own ("purlis's sandbox does not allow host:443"),
//!   Claude Code's ("blocked by network allowlist"), or a client's word for the `403` a proxy
//!   answered a tunnel with; and Go's certificate check failing on the system service the
//!   sandbox keeps from it (`x509: OSStatus -26276`). A lookup the sandbox refused a program
//!   that resolves a host itself, past the proxy (macOS's "nodename nor servname provided",
//!   curl's "Could not resolve host"), and a local socket Docker's client or Go's dial was
//!   refused (#1631): neither comes with a violation line.
//! - **A program's own "Operation not permitted"** that names its path, where no violation block
//!   said more, and only for a path outside what the chat may write (its folder and the temporary
//!   folders): inside them, the sandbox did not refuse it.
//!
//! The last two are read from a command's **standard error** (`PostToolUse`'s
//! `tool_response.stderr`) wherever they are on a line. **A refusal of the network is read from
//! standard output too** (#1663): a command run with `2>&1`, and a failed command's `error`
//! (which holds both streams). There a line counts only when it ends where the refusal's own
//! words do ([`ends_as_a_refusal`]); and a program's own refusal of a path is never read there
//! at all.
//!
//! **Printed text is never a block** (#1972): a block is raised only for a refusal a command
//! may have met. A result with no failure beside it says nothing of how each program of its
//! line ended, since a pipe drops a status, so its line is read for that one answer
//! ([`runs_a_program`]): a line whose every program only reads or prints (`cat`, `sed -n`,
//! `grep`, `git show`) merely printed what it handed back, and raises no refusal of the network
//! from it, on either stream or as one string. Nor does a background command's output read back
//! later, with the harness's file tool or its own tool for it: nothing beside it says how that
//! command ended, unless the harness's own record of the task says it failed
//! ([`background`], #2203), when it is that command's own error. A failed command's error, the
//! sandbox's own report and purlis's own proxy are read whatever the line runs.
//!
//! **What a command printed on standard output never names a host to allow**: a file or a
//! server's page can say any host. A refused lookup names the host the program said it looked
//! up, from either stream ([`detect_with_targets`]): it is shown, and never offered to allow,
//! since a host the project allows would not let such a program through. **A socket is named
//! only by a command that failed, or by one whose line runs a container engine's client**
//! ([`runs_a_container_client`]): a pipe drops the client's status, and the harness then hands
//! its words over with no failure beside them.
//!
//! **Each harness's adapter hands its result over in that shape** (#1353). opencode returns a
//! shell command's standard output and error as one text, with its exit status beside it. Its
//! shim (`crate::opencode`) passes a command that did not fail as one mixed text, read as
//! Codex's is, below. A command that failed is passed as standard error marked
//! `mixed`, and all three kinds of evidence are read from it, standard output included, which is
//! wider than Claude Code's route: a failing command that prints a refusal's words raises a
//! block. So from a mixed stream nothing but a violation line names a target for a grant
//! ([`detect_with_targets`]): such a Notice names no host or path, and the person types it.
//! Codex hands a shell command's result over as one string of both streams too, after every
//! command (#1663: its shell is `Bash` to a hook, `crate::plugin::CODEX` cites its source), with
//! no exit status beside it, so it is read as opencode's command that did not fail: a refusal
//! of the network on a line that ends as one, where the line runs a program other than a
//! reader, and a lookup's host shown and never offered. A
//! program's own refusal of a path is not read from it. A chat of either runs behind purlis's
//! own proxy (`crate::sandbox::egress`), which tells the app each host it refused by name, as a
//! brokered run's does; their hook leaves a refused host to it ([`the_proxy_tells_hosts`]), so one
//! refusal is one Notice.
//!
//! # Whose operation it was
//!
//! **Per violation line, by the process it names**: a line whose process is `purlis` (or a name
//! it is installed under) is purlis's own, which is a purlis bug. Nothing else is: a line with no
//! process, a proxy's refusal, a program's own words. So `cargo build; purlis status` does not
//! make cargo's block purlis's. A chat can still print a line that names `purlis`, as it can
//! send the app any line its token admits; what that buys is a Notice on its own tab and one
//! more in the count ([`Throttle`] bounds it), and a Report that names only fixed words.
//!
//! # What is kept of it
//!
//! **An operation, a kind, and whether it was purlis's own** ([`Block`]). A path is read here,
//! in the hook, to sort it, and goes no further than the Notice's Allow: no path, argument or
//! output is kept. **The host travels** (#1663, settling #1353): the host a refused connection
//! or lookup named goes to the app on the block's line ([`detect_with_targets`]), and is kept
//! with the block, checked as a grant checks a host ([`named_host`]): a refused connection's as
//! the host Allow may name, a refused lookup's as one never offered to allow
//! ([`record::Entry::looked_up`]).
//! **A socket's path and a service's name travel too** (ADR 0093): what a grant names, offered
//! to allow with what allowing it means, and judged again as a grant before anything is kept.
//!
//! The app keeps each block it hears in this machine's network record ([`record`], #1662) for
//! 30 days, with the chat it came from and, for a refused connection, the host and port. That
//! is what `purlis doctor` counts ([`record::counts`]), Settings' Network page and a chat's
//! Network view list, and the window shows it as a notice on the chat's tab.
//! A block of purlis's own is a purlis bug, and the notice offers a Report whose draft is made
//! from the block alone (`report::Draft::of_sandbox_block`). Nothing is sent without a press.

use std::path::{Component, Path, PathBuf};

/// What the sandbox refused to let a program do.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    /// Create, change, rename or remove a file or folder.
    Write,
    /// Read a file, or list a folder.
    Read,
    /// Open a connection: to a host, or to a local socket.
    Connect,
    /// Reach a system service by name (macOS's `mach-lookup`).
    Lookup,
    /// Start a program.
    Run,
    /// Something done to a file that the report does not say: a bare "Operation not permitted".
    File,
    /// Anything else the sandbox names.
    Other,
}

impl Operation {
    /// Every operation, in the order a count lists them.
    pub const ALL: [Operation; 7] = [
        Self::Write,
        Self::Read,
        Self::Connect,
        Self::Lookup,
        Self::Run,
        Self::File,
        Self::Other,
    ];

    /// The word it is stored and counted under.
    pub fn word(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Read => "read",
            Self::Connect => "connect",
            Self::Lookup => "lookup",
            Self::Run => "run",
            Self::File => "file",
            Self::Other => "other",
        }
    }

    /// The operation `word` names, as [`Operation::word`] spells it.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.word() == word)
    }

    /// The operation a Seatbelt word names: `file-write-create`, `network-outbound`, …. Of
    /// the network's words only a connection out is one: a refused listen on a port
    /// (`network-bind`, `network-inbound`) is of this machine's own port, and no host's.
    fn of_seatbelt(word: &str) -> Self {
        if word.starts_with("file-write") {
            Self::Write
        } else if word.starts_with("file-read") {
            Self::Read
        } else if word.starts_with("network-outbound") {
            Self::Connect
        } else if word.starts_with("mach-lookup") {
            Self::Lookup
        } else if word.starts_with("process-exec") {
            Self::Run
        } else if word.starts_with("file-") {
            Self::File
        } else {
            Self::Other
        }
    }

    /// The operation as a sentence names it, before what it was done to.
    fn phrase(self) -> &'static str {
        match self {
            Self::Write => "a write to",
            Self::Read => "a read of",
            Self::Connect => "a connection to",
            Self::Lookup => "a lookup of",
            Self::Run => "starting a program in",
            Self::File => "a file operation in",
            Self::Other => "an operation on",
        }
    }
}

/// The kind of path or host a block was about. A closed set: nothing the chat wrote can
/// become one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The project's own files, outside the chat's folder: records, memory, `workspace.md`.
    ProjectFiles,
    /// purlis's state folder at the project root (`.purlis/` or `.charter/`).
    ProjectState,
    /// A file a later program loads code or settings from, which the sandbox protects
    /// wherever it is (`.git/config`, `.vscode`; `sandbox::PLANTED`), or a project manifest
    /// where it could change a chat's sandbox (`sandbox::manifests_held`).
    ProtectedFile,
    /// The chat's own folder.
    ChatFolder,
    /// A toolchain's package cache in the home folder (`~/.cargo`, `~/.npm`, `~/Library/Caches`).
    ToolchainCache,
    /// Elsewhere in the home folder.
    Home,
    /// A temporary folder.
    Temp,
    /// macOS's per-user temporary folder (`/var/folders/<a>/<b>/T`, what
    /// `getconf DARWIN_USER_TEMP_DIR` answers), which some tools ask the system for in place of
    /// `TMPDIR` (#1120): `mktemp` without a path, Foundation's `NSTemporaryDirectory`, and
    /// `xcrun`'s cache, which only warns. No sandboxed chat may write it (D-1342-11). The two
    /// other per-user folders beside it that hold no cache, `…/0` (`DARWIN_USER_DIR`) and `…/X`,
    /// are sorted here too (#1416): no new kind is stored for them, its sentence is said of all
    /// three, and like this one no grant is offered for them.
    SystemTemp,
    /// macOS's per-user cache folder (`/var/folders/<a>/<b>/C`, `DARWIN_USER_CACHE_DIR`), where
    /// clang and Swift keep their module cache. A wrapped chat's module cache is its own temp
    /// directory (`sandbox::seatbelt::TEMP_ENV`); a Claude Code chat's is not yet (#1416), so
    /// those builds fail there. No sandboxed chat may write it: it holds what other programs load.
    SystemCache,
    /// Anywhere else on the machine.
    System,
    /// An internet host the project does not allow.
    Host,
    /// An internet address a program connected to directly, past the sandbox's proxy (#2423):
    /// no host a grant could name, since no host is allowed but through the proxy.
    Address,
    /// A local socket.
    LocalSocket,
    /// A service on this Mac: whatever listens on one port of the loopback interface, which a
    /// program reached directly rather than through the sandbox's proxy (qw384).
    LocalService,
    /// The system's certificate check, which Go's TLS asks a system service for.
    CertificateCheck,
    /// Another system service.
    SystemService,
    /// A service on the runner a chat runs on (#2510): one port of that runner's loopback
    /// interface, left out of the chat's plan because it was allowed for this chat alone on the
    /// person's own machine, which the runner's proxy asks about.
    RunnerService,
}

impl Kind {
    /// Every kind, in the order they are listed.
    pub const ALL: [Kind; 17] = [
        Self::ProjectFiles,
        Self::ProjectState,
        Self::ProtectedFile,
        Self::ChatFolder,
        Self::ToolchainCache,
        Self::Home,
        Self::Temp,
        Self::SystemTemp,
        Self::SystemCache,
        Self::System,
        Self::Host,
        Self::Address,
        Self::LocalSocket,
        Self::LocalService,
        Self::CertificateCheck,
        Self::SystemService,
        Self::RunnerService,
    ];

    /// The word it is stored under.
    pub fn word(self) -> &'static str {
        match self {
            Self::ProjectFiles => "project-files",
            Self::ProjectState => "project-state",
            Self::ProtectedFile => "protected-file",
            Self::ChatFolder => "chat-folder",
            Self::ToolchainCache => "toolchain-cache",
            Self::Home => "home",
            Self::Temp => "temp",
            Self::SystemTemp => "system-temp",
            Self::SystemCache => "system-cache",
            Self::System => "system",
            Self::Host => "host",
            Self::Address => "address",
            Self::LocalSocket => "local-socket",
            Self::LocalService => "local-service",
            Self::CertificateCheck => "certificate-check",
            Self::SystemService => "system-service",
            Self::RunnerService => "runner-service",
        }
    }

    /// The kind `word` names, as [`Kind::word`] spells it.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.word() == word)
    }

    /// The kind as a sentence names it.
    pub fn phrase(self) -> &'static str {
        match self {
            Self::ProjectFiles => "the project's own files",
            Self::ProjectState => "purlis's state for this project",
            Self::ProtectedFile => "a protected file later programs load, such as .git/config",
            Self::ChatFolder => "this chat's own folder",
            Self::ToolchainCache => "a toolchain's package cache",
            Self::Home => "your home folder",
            Self::Temp => "a temporary folder",
            // One sentence for `T`, `0` and `X`, which this kind holds alike (#1416): it names
            // no one tool's way round, since `0` and `X` are no temporary folder and no one
            // option fits each program that writes there. The chat's own `$TMPDIR` does.
            Self::SystemTemp => {
                "one of macOS's per-user folders, which some programs write in place of this \
                 chat's own temporary folder. Such a program can usually be told to write under \
                 \"$TMPDIR\" instead"
            }
            // No harness named (#1416): a chat purlis wraps has its module cache moved into its
            // own temp, and one whose harness runs its own sandbox does not yet.
            Self::SystemCache => {
                "macOS's per-user cache folder, where Swift and clang builds keep compiled \
                 modules. Swift and clang builds cannot write it yet in a chat whose harness runs \
                 its own sandbox"
            }
            Self::System => "a system folder",
            Self::Host => "an internet host this project does not allow",
            Self::Address => "an internet address, dialled past the sandbox's proxy",
            Self::LocalSocket => "a local socket",
            Self::LocalService => "a service on this Mac",
            Self::CertificateCheck => "the system's certificate check",
            Self::SystemService => "a system service",
            Self::RunnerService => "a service on the runner this chat runs on",
        }
    }
}

/// One block: what was refused, the kind of thing it was refused on, and whether it was
/// purlis's own operation. Nothing more is kept of it, anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub operation: Operation,
    pub kind: Kind,
    /// Whether the process the sandbox refused was purlis itself: a purlis bug, not the chat's.
    #[serde(default)]
    pub ours: bool,
}

impl Block {
    /// What was blocked, as the notice says it: "a write to the project's own files".
    pub fn said(&self) -> String {
        // A refused lookup is of a host the project may well allow (#1631): what was refused
        // is the program's own lookup, past the proxy, so the host is never said to be one
        // the project does not allow, which would have the person allow it to no effect.
        if (self.operation, self.kind) == (Operation::Lookup, Kind::Host) {
            return "a lookup of an internet host by a program that does not go through the \
                    sandbox's proxy"
                .to_owned();
        }
        // Likewise a connection to an address dialled past the proxy (#2423).
        if (self.operation, self.kind) == (Operation::Connect, Kind::Address) {
            return "a connection to an internet address by a program that does not go through \
                    the sandbox's proxy"
                .to_owned();
        }
        format!("{} {}", self.operation.phrase(), self.kind.phrase())
    }
}

/// **What the chat is told, in the turn it was blocked**, of the blocks the app took (#1631):
/// what was blocked, in this module's fixed words, that the person has a Notice about it on
/// the chat's tab, and to say what was blocked and wait rather than go around it. None when
/// the app took none, since then no Notice is up to point at. The briefing says how blocks
/// work once, at the start (`briefing::SANDBOXED_NOTE`); this is said where it happens, so a
/// chat many turns in does not route around a block without a word. A write to macOS's
/// per-user folders, which no Allow can open, is told its way round instead
/// ([`SYSTEM_TEMP_WAY_ROUND`], #1416), and only the rest is waited on.
pub fn told_the_chat(taken: &[Block]) -> Option<String> {
    if taken.is_empty() {
        return None;
    }
    let mut said: Vec<String> = Vec::new();
    for block in taken {
        let one = block.said();
        if !said.contains(&one) {
            said.push(one);
        }
    }
    let ours = if taken.iter().any(|block| block.ours) {
        " Where it was purlis's own operation, it is a purlis bug, and the Notice offers a \
         Report rather than an Allow."
    } else {
        ""
    };
    // macOS's per-user folders are never granted (D-1342-11), so no answer can let the write
    // through: the chat is told the way round, in fixed words, and waits only for the rest.
    let per_user = |block: &Block| block.kind == Kind::SystemTemp && !block.ours;
    let way_round = if taken.iter().any(per_user) {
        SYSTEM_TEMP_WAY_ROUND
    } else {
        ""
    };
    if taken.iter().all(per_user) {
        return Some(format!(
            "⬢ purlis's sandbox blocked {}. The person has a Notice about it on this chat's \
             tab.{way_round}",
            said.join("; ")
        ));
    }
    Some(format!(
        "⬢ purlis's sandbox blocked {}. The person has a Notice about it on this chat's tab, \
         with what they can do about it.{ours} Say in a line what was blocked and what you \
         need it for, then wait for their answer: do not work around the block (another host, \
         another tool, another way in) and do not retry before they answer. If they allow it, \
         this chat is started again on this conversation and told to retry.{way_round}",
        said.join("; ")
    ))
}

/// **What the chat is told of blocks found in what it read back** ([`read_back`], #1884): that
/// the words are a refusal's, that the person has a quiet line about it which nothing waits on,
/// and to carry on. It is not told to wait: no command of this call was refused, and the person
/// is not asked anything. None when the app took none.
pub fn told_the_chat_of_a_read(taken: &[Block]) -> Option<String> {
    if taken.is_empty() {
        return None;
    }
    let mut said: Vec<String> = Vec::new();
    for block in taken {
        let one = block.said();
        if !said.contains(&one) {
            said.push(one);
        }
    }
    Some(format!(
        "⬢ What you just read holds the words of a sandbox refusal: {}. The person has a quiet \
         line about it on this chat's tab, and nothing waits on their answer, so carry on. If a \
         command of yours needs what was refused, say so in a line and run it again where its \
         result comes back to you: a refusal of a command you run raises a Notice the person can \
         answer. Do not work around the block.",
        said.join("; ")
    ))
}

/// What a chat is told of a write to macOS's per-user folders (#1416): fixed words, the same
/// whatever it ran. No Allow can come for them, and the chat's own `$TMPDIR` is a folder it may
/// already write, so this names no folder it could not write before.
const SYSTEM_TEMP_WAY_ROUND: &str = " No Allow is offered for macOS's per-user folders, so do \
     not wait for one there: write the file under \"$TMPDIR\" instead (mktemp -p \"$TMPDIR\", or the \
     tool's own option for its temporary folder), and do not retry the same write.";

/// The folder the app started the chat in, in its environment (the app's `chats::open_it`):
/// what a block is sorted against as the chat's own folder. Not the payload's `cwd`, which moves
/// with the chat. Claude Code's own `CLAUDE_PROJECT_DIR` says the same where this is not set.
pub const CHAT_DIR_ENV: &str = "PURLIS_CHAT_DIR";

/// Where the chat that was blocked works: what a path is sorted against.
#[derive(Debug, Clone, Copy)]
pub struct Place<'a> {
    /// The project's root.
    pub root: &'a Path,
    /// The folder the app started the chat in ([`CHAT_DIR_ENV`]).
    pub chat: &'a Path,
    /// The folder the command ran in, which a relative path is read from.
    pub cwd: &'a Path,
    /// The home folder, when there is one.
    pub home: Option<&'a Path>,
}

/// The most blocks one tool result yields: a command that failed on a thousand files is one
/// pattern, not a thousand notices.
pub const AT_MOST_PER_RESULT: usize = 8;

/// The blocks in a tool hook's `payload`, each sorted against `place`, at most
/// [`AT_MOST_PER_RESULT`] and none twice. The caller asks only in a sandboxed chat.
///
/// A `PostToolUse` payload's `tool_response.stderr` is read for all three kinds of evidence; a
/// `PostToolUseFailure` payload's `error`, and a `tool_response` that is one string (opencode's
/// and Codex's), hold the command's standard output too, so only their violation block and the
/// network's refusals as standard output says them are read. The command is read for one
/// answer, whether its line runs a container engine's client ([`runs_a_container_client`]), and
/// nothing of it is kept.
pub fn detect(payload: &serde_json::Value, place: &Place<'_>) -> Vec<Block> {
    detect_with_targets(payload, place)
        .into_iter()
        .map(|(block, _)| block)
        .collect()
}

/// **What a grant could name**, beside each block [`detect`] finds (#1342): the host a refused
/// connection was to, where the report names one, and the whole path of a refused write
/// outside purlis's own state and the protected files, but for a branch folder's git record,
/// which names no grant and whose path chooses the Notice's way out ([`way_out`], #1476).
/// `None` for every other block, and for a connection whose report names no host (Claude
/// Code's proxy says none): the person types it.
///
/// A `tool_response` marked `mixed` holds what the command printed on both streams in its
/// `stderr` (opencode's, #1353): from it only a violation line names a target.
///
/// **A result that came back from a line that runs a container engine's client names the
/// socket its output says it was refused** ([`runs_a_container_client`]), on either stream and
/// in one string: the line dropped the client's status (`docker run … 2>&1 | tail`), so no
/// failure stands beside its words. It names a socket and nothing else.
///
/// **A refused certificate check is named wherever the line runs a program**
/// ([`runs_a_program`]): its one service is purlis's own word, never the output's, so only a
/// result that merely read the words (a line of readers, a file tool's read) leaves it unnamed.
///
/// It goes to the app alone, on the block's line, for the Notice's Allow. It is never kept with
/// the block ([`record`] keeps a [`Block`]), counted, or put in a Report.
pub fn detect_with_targets(
    payload: &serde_json::Value,
    place: &Place<'_>,
) -> Vec<(Block, Option<String>)> {
    detect_with_record(payload, place, Ended::Unrecorded)
}

/// **How the background command whose output a result read back ended**, by the harness's own
/// record of it ([`background::how_it_ended`], #2203).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ended {
    /// No record says: the command still runs, the harness keeps none, or the result read no
    /// background command's output back.
    #[default]
    Unrecorded,
    /// The harness's record says the command failed.
    Failed,
}

/// [`detect_with_targets`], with how the background command whose output `payload` read back by
/// file ended (#2203): a file tool's read of a background command's output file
/// ([`background::outputs_read`]), or a line whose every program only reads or prints that
/// reads one, is that command's own error where its record says it failed, read as a failed
/// command's is. With no record it merely printed what it handed back, as before.
pub fn detect_with_record(
    payload: &serde_json::Value,
    place: &Place<'_>,
    ended: Ended,
) -> Vec<(Block, Option<String>)> {
    let mut found: Vec<(Block, Option<String>)> = Vec::new();
    let client = runs_a_container_client(payload);
    let read_back_failed = ended == Ended::Failed && !background::outputs_read(payload).is_empty();
    // A result with no failure beside it says nothing of how each program of its line ended
    // (a pipe drops a status), so its line is the one signal left (#1972): a line whose every
    // program only reads or prints merely printed what it handed back, and raises nothing
    // from it; any other line may have met the refusal its programs said.
    let runs = runs_a_program(payload);
    // Read only where the answer is asked: it may read the repository's config (#2212).
    let may_run = std::cell::OnceCell::new();
    let may_run = || *may_run.get_or_init(|| may_run_a_program(payload, place));
    let said = if bash_output_failed(payload) || (read_back_failed && !may_run()) {
        Said::Failed
    } else if client {
        Said::Client
    } else if may_run() {
        Said::Unsaid
    } else {
        Said::Printed
    };
    // A file tool's read is no command's result (#1972): nothing it read says how a command
    // ended, so nothing it read is a block, but a background command's output file whose
    // record says it failed: that command's own error (#2203).
    if payload["tool_name"].as_str() == Some(READ_TOOL) {
        if let Some(content) = payload["tool_response"]["file"]["content"].as_str()
            && said == Said::Failed
        {
            found.extend(on_stdout(content, place, Said::Failed));
        }
        return at_most(found);
    }
    match &payload["tool_response"] {
        serde_json::Value::String(mixed) => {
            // One string with nothing of how the command ended beside it: opencode's for a
            // command that did not fail, Codex's for every command. A violation line in it may
            // be one the command printed, so it raises a block and names no target to allow; a
            // target comes only from the sandbox's own report or a failed command's own error
            // (D-sock-5), or a container client's own words for its socket.
            found.extend(
                violations(mixed, place)
                    .into_iter()
                    .map(|(block, _)| (block, None)),
            );
            found.extend(on_stdout(mixed, place, said));
        }
        response => {
            if let Some(stderr) = response["stderr"].as_str() {
                if response["mixed"].as_bool() == Some(true) {
                    // Standard error and output in one text (opencode's, #1353): what the
                    // command printed may name any host or path, so only a violation line,
                    // which is the sandbox's own, names a target to allow. A lookup's host is
                    // said, and never offered. The certificate check keeps to its own rule,
                    // below: what a line of readers printed names none here either.
                    let own = violations(stderr, place);
                    found.extend(on_stderr(stderr, place).into_iter().map(|(block, target)| {
                        let kept = target.is_some()
                            && (own.contains(&(block, target.clone()))
                                || (names_a_lookup(&block)
                                    && !matches!(
                                        block.kind,
                                        Kind::CertificateCheck | Kind::LocalService
                                    )));
                        (block, if kept { target } else { None })
                    }));
                } else {
                    // A command that succeeded (D-sock-5): only the sandbox's own line names a
                    // socket or a service to allow, never what the program printed: the same
                    // rule as its standard output's ([`on_stdout`]). A container client whose
                    // status the line dropped names its socket there too. A line that only
                    // reads or prints raises no refusal of the network from it (#1972).
                    let own = violations(stderr, place);
                    let mut read = reported_or_of_paths(stderr, place);
                    if said == Said::Failed {
                        // A background command that ended failed: its own error, read as a
                        // failed command's is.
                        found.extend(on_stdout(stderr, place, Said::Failed));
                    } else if said != Said::Printed {
                        read.extend(of_the_network(stderr, place));
                    }
                    found.extend(read.into_iter().map(|(block, target)| {
                        let kept = !matches!(
                            block.kind,
                            Kind::LocalSocket
                                | Kind::LocalService
                                | Kind::CertificateCheck
                                | Kind::SystemService
                        ) || own.contains(&(block, target.clone()))
                            || (client && block.kind == Kind::LocalSocket);
                        (block, if kept { target } else { None })
                    }));
                }
            }
            if let Some(stdout) = response["stdout"].as_str() {
                found.extend(on_stdout(stdout, place, said));
            }
        }
    }
    if let Some(mixed) = payload["error"].as_str() {
        found.extend(violations(mixed, place));
        found.extend(on_stdout(mixed, place, Said::Failed));
    }
    // The certificate check is one service, purlis's own word for it: a line that runs a
    // program names it however the line ended ([`runs_a_program`]). Read only where a check
    // was left unnamed, so no other result's line is read for it.
    let unnamed =
        |(block, target): &(Block, Option<String>)| target.is_none() && is_the_check(block);
    if found.iter().any(unnamed) && runs {
        for (block, target) in &mut found {
            if target.is_none() && is_the_check(block) {
                *target = target_of(block, crate::sandbox::services::TRUSTD, place);
            }
        }
    }
    at_most(found)
}

/// Whether `block` is a refused certificate check that a grant may name: the one kind with a
/// single service behind it ([`crate::sandbox::services::TRUSTD`]), and not purlis's own
/// lookup, which is a purlis bug and never a grant.
fn is_the_check(block: &Block) -> bool {
    (block.operation, block.kind) == (Operation::Lookup, Kind::CertificateCheck) && !block.ours
}

/// Whose words a text read as standard output is: whether it is a refusal the command met at
/// all (#1972), and whether a socket or the certificate check it names is one a grant may name
/// ([`on_stdout`], D-sock-5).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Said {
    /// A command that failed: its client's own error.
    Failed,
    /// A command that came back from a line that runs a container engine's client
    /// ([`runs_a_container_client`]): its words name its socket, and nothing else.
    Client,
    /// A command that came back from a line that runs a program other than a reader
    /// ([`runs_a_program`]): it may be a refusal one of its programs met, and it names no
    /// socket, port or service, since it could say any.
    Unsaid,
    /// What a command merely printed: one that came back from a line whose every program only
    /// reads or prints, or a result with no line beside it. Never a block (#1972).
    Printed,
}

impl Said {
    /// Whether these words may name `kind`'s target, of the two only a failure's own words
    /// name; any other kind is named by whatever said it.
    fn names(self, kind: Kind) -> bool {
        match kind {
            Kind::LocalSocket => matches!(self, Self::Failed | Self::Client),
            // A port only from the failed program's own error (qw384): a container client's
            // words name a socket, never a port.
            Kind::LocalService | Kind::CertificateCheck => self == Self::Failed,
            _ => true,
        }
    }
}

/// The clients of a container engine, by the name a line runs one under.
const CONTAINER_CLIENTS: [&str; 5] = [
    "docker",
    "docker-compose",
    "podman",
    "podman-compose",
    "nerdctl",
];

/// **Whether the command line of `payload` runs a container engine's client** (ADR 0093,
/// D-sock-5): one of its own programs is one of [`CONTAINER_CLIENTS`] by name, once the shell's
/// plumbing is off the front of each part of the line (assignments, redirections, `timeout`,
/// `env`: [`crate::shellwrap::split_env_chdir`]).
///
/// **The failure signal for a result that came back.** A harness says whether a command failed
/// by the status of its whole line, and a line often drops its client's: `docker run … 2>&1 |
/// tail` ends with `tail`'s, `docker run …; echo $?` with `echo`'s. Claude Code's result then
/// holds what was printed and nothing of how any program ended, and Codex's never does. The line
/// is the one signal left in the payload, and it is the chat's own, as a failed command's status
/// is: a chat that wants a socket named can make a command fail. What it rules out is the case
/// a failure ruled out, text that names a socket with no program refused one: a line that only
/// reads (`cat` of a log, `git show`, a page `curl` fetched) runs no client, and names none. A
/// client prints its own error, or what a daemon it already reached sent it.
///
/// Read whole or not at all: a line too big for the guards, one that does not tokenize, and a
/// client only inside a string a shell is handed (`sh -c '…'`) run none here.
fn runs_a_container_client(payload: &serde_json::Value) -> bool {
    let Some(line) = payload["tool_input"]["command"].as_str() else {
        return false;
    };
    if line.len() > crate::guardcaps::MAX_COMMAND_BYTES
        || !CONTAINER_CLIENTS
            .iter()
            .any(|client| crate::shellwrap::may_name(line, client))
    {
        return false;
    }
    let (runs, too_deep) = crate::shellseg::too_deep_within(|| {
        crate::shellseg::joined_argv(line).is_some_and(|segments| {
            segments.iter().any(|segment| {
                let call = crate::shellwrap::split_env_chdir(&segment.argv);
                !call.too_deep
                    && CONTAINER_CLIENTS
                        .contains(&crate::shellwrap::base_lower(&call.prog).as_str())
            })
        })
    });
    runs && !too_deep
}

/// The programs that only read or print: none of them makes a connection of its own, so none
/// is ever refused the certificate check. By the name a line runs one under.
pub(crate) const READERS: [&str; 62] = [
    "cat", "tac", "head", "tail", "less", "more", "bat", "nl", "wc", "grep", "egrep", "fgrep",
    "zgrep", "rg", "ag", "ack", "sed", "awk", "gawk", "cut", "tr", "sort", "uniq", "column",
    "fold", "rev", "tee", "strings", "xxd", "hexdump", "od", "jq", "yq", "diff", "cmp", "find",
    "ls", "stat", "file", "echo", "printf", "true", "false", "test", "[", "cd", "pwd", "sleep",
    // The shell's own, which run nothing.
    "read", "export", "set", "unset", "local", "declare", "exit", ":", "[[", "break", "continue",
    "return", "shift", "wait",
];

/// The words that open a loop's or a choice's header, whose next word names no program
/// (`for f in *.log`).
const HEADERS: [&str; 3] = ["for", "select", "case"];

/// The subcommands of `git` that only read the clone they stand in.
const GIT_READERS: [&str; 14] = [
    "show",
    "log",
    "diff",
    "grep",
    "blame",
    "cat-file",
    "status",
    "ls-files",
    "ls-tree",
    "rev-parse",
    "rev-list",
    "reflog",
    "describe",
    "shortlog",
];

/// How many strings handed to a shell, one inside another, are read for what they run
/// (`sh -c "sh -c '…'"`); a line that nests deeper is read as one that only reads.
const MOST_SHELLS_DEEP: usize = 2;

/// **Whether the command line of `payload` runs a program that could have been refused the
/// certificate check** (ADR 0093, D-sock-5): any program but one that only reads or prints
/// ([`READERS`], and `git`'s reading subcommands), once the shell's plumbing is off the front of
/// each part of the line, as [`runs_a_container_client`] reads it. A string a shell is handed
/// is read the same way, so `sh -c 'cat gh.log'` runs none.
///
/// **Why any program, and not a list of clients.** The check's service is one, and its name is
/// purlis's own constant: naming it puts nothing a chat printed in the Notice, where a socket's
/// path comes from the output. And every Go program checks certificates this way (`gh`, `glab`,
/// `terraform`, a project's own tools behind `make`), so no list could be kept whole. What is
/// ruled out is what a failure ruled out, a result that merely read the words: a log, a file of
/// tests, a page of search results, none of which can make its reader a program that connects.
///
/// **A here-document's body is text, never a line the shell runs.** Its opener reads it, so a
/// body that holds the check's words, or a line that looks like a command, is what a reader
/// printed: every body line is left out before the line is read ([`without_bodies`]). A shell
/// that opens one is a program on the opener's own line, whatever its body says.
///
/// Read whole or not at all: a result with no line beside it (a file tool's, a background
/// command's output read back), a line too big for the guards, and one that does not tokenize
/// run none here.
fn runs_a_program(payload: &serde_json::Value) -> bool {
    let Some(line) = payload["tool_input"]["command"].as_str() else {
        return false;
    };
    if line.len() > crate::guardcaps::MAX_COMMAND_BYTES {
        return false;
    }
    let line = without_bodies(line);
    let (runs, too_deep) = crate::shellseg::too_deep_within(|| runs_past_readers(&line, 0));
    runs && !too_deep
}

/// `line` without the body of any here-document it opens, terminators too
/// ([`crate::heredoc::heredoc_layout`]). Every body goes, a shell's as well: leaving a line out
/// can only make this reading find fewer programs, the way it is allowed to be wrong.
fn without_bodies(line: &str) -> std::borrow::Cow<'_, str> {
    if !line.contains("<<") {
        return line.into();
    }
    crate::heredoc::heredoc_layout(line)
        .into_iter()
        .filter(|row| !row.body)
        .map(|row| row.text)
        .collect::<Vec<_>>()
        .join("\n")
        .into()
}

/// [`runs_a_program`] of one line, itself `shells` strings deep in the line a chat ran.
fn runs_past_readers(line: &str, shells: usize) -> bool {
    reads_past_readers(line, shells, Reading::Naming)
}

/// How a line is read for what it runs: to name the certificate check, where a line not read
/// whole runs none ([`runs_a_program`]), or to decide whether what it handed back is a block
/// at all, where a line not read whole, and a reader that can start a program, runs one
/// ([`may_run_a_program`]).
///
/// Raising carries where the line ran, which a git reader's repository is found from
/// ([`drivers::names_a_program`], #2212).
#[derive(Clone, Copy)]
enum Reading<'a> {
    Naming,
    Raising(&'a Place<'a>),
}

/// **Whether the command line of `payload` may have run a program that met a refusal** (#1972):
/// the reading that decides whether what a result with no failure beside it handed back is a
/// block at all. It errs toward a program: a line too big for the guards, one that does not
/// tokenize, a string a shell is handed nested past what is read, and a reader that can start
/// a program of its own ([`starts_a_program`]) each run one, as does a git reader in a
/// repository whose config names a program it runs ([`drivers::names_a_program`], #2212), or in
/// a folder the line names in a way that is not read. Only a line read whole, whose every
/// program only reads or prints, runs none; and a result with no line beside it runs none,
/// since nothing in it is a command this call ran.
fn may_run_a_program(payload: &serde_json::Value, place: &Place<'_>) -> bool {
    let Some(line) = payload["tool_input"]["command"].as_str() else {
        return false;
    };
    if line.len() > crate::guardcaps::MAX_COMMAND_BYTES {
        return true;
    }
    let line = without_bodies(line);
    let (runs, too_deep) =
        crate::shellseg::too_deep_within(|| reads_past_readers(&line, 0, Reading::Raising(place)));
    runs || too_deep
}

/// Whether a reader's call, `base` with `argv` (the program first), can start a program of its
/// own: `find` with `-exec`, `-execdir`, `-ok` or `-okdir`; `awk`, whose script can run one
/// (`system()`, a pipe); `sed` with any script but a list of lines to print or delete, since
/// GNU `sed` runs one with `e`; `rg` with `--pre`; `sort` with `--compress-program`; and `less`
/// and `more`, whose input preprocessor (`LESSOPEN`) is a program the environment names
/// (#2203).
fn starts_a_program(base: &str, argv: &[String]) -> bool {
    let has_option =
        |full: &str, shortest: &str| argv.iter().any(|word| long_option(word, full, shortest));
    match base {
        "find" => argv
            .iter()
            .any(|word| matches!(word.as_str(), "-exec" | "-execdir" | "-ok" | "-okdir")),
        "awk" | "gawk" | "less" | "more" => true,
        "sed" => !sed_only_prints(&argv[1.min(argv.len())..]),
        // ripgrep takes a long option only whole; GNU sort takes any unambiguous prefix.
        "rg" => has_option("--pre", "--pre"),
        "sort" => has_option("--compress-program", "--co"),
        _ => false,
    }
}

/// Whether `word` gives the long option `full`, by its whole name or by any prefix of it at
/// least as long as `shortest`, the shortest the program takes as that option alone; with its
/// value after `=` or not.
fn long_option(word: &str, full: &str, shortest: &str) -> bool {
    let name = word.split('=').next().unwrap_or_default();
    name.starts_with(shortest) && full.starts_with(name)
}

/// **Whether a `git` call that only reads can start a program of its own** (#2203): `env`, its
/// assignments, and the `globals` and `rest` of its argv ([`crate::shellwrap::git_globals`]).
/// Configuration given on the line (`-c`, `--config-env`, or `GIT_CONFIG_*` in its environment)
/// can name one (`diff.external`, `core.fsmonitor`), as can `GIT_EXTERNAL_DIFF`, `--ext-diff`,
/// and a `grep` that opens its files in a pager (`-O`, `--open-files-in-pager`). A repository
/// or a config named past the folder it runs in (`--git-dir`, `--work-tree`, `GIT_DIR`,
/// `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `HOME`, `XDG_CONFIG_HOME`) may name one too (#2212).
fn git_starts_a_program(env: &[String], globals: &[String], rest: &[String]) -> bool {
    let configured = globals.iter().any(|word| {
        word.starts_with("-c")
            || word.starts_with("--config-env")
            || long_option(word, "--git-dir", "--git-dir")
            || long_option(word, "--work-tree", "--work-tree")
    });
    let in_env = env.iter().any(|assignment| {
        let name = assignment.split('=').next().unwrap_or_default();
        name.starts_with("GIT_CONFIG")
            || matches!(
                name,
                "GIT_EXTERNAL_DIFF"
                    | "GIT_DIR"
                    | "GIT_WORK_TREE"
                    | "GIT_COMMON_DIR"
                    | "HOME"
                    | "XDG_CONFIG_HOME"
            )
    });
    let args = rest.get(1..).unwrap_or(&[]);
    // git takes any unambiguous prefix of a subcommand's long option.
    let external = args
        .iter()
        .any(|word| long_option(word, "--ext-diff", "--ext"));
    let pager = rest.first().is_some_and(|word| word == "grep")
        && args.iter().any(|word| {
            long_option(word, "--open-files-in-pager", "--op")
                || (word.starts_with("-O") && !word.starts_with("--"))
        });
    configured || in_env || external || pager
}

/// Whether `sed`'s arguments only print or delete lines by their number (`sed -n 1,80p f`,
/// `sed -n '5p;10p' f`): its flags only `-n`, `-E`, `-r`, `-s`, `-u`, `-z`, and its one script
/// a list of such commands. Any other script, `-e`, `-f` or `-i` is read as one that may run a
/// program.
fn sed_only_prints(args: &[String]) -> bool {
    let mut script = None;
    for arg in args {
        if let Some(flags) = arg.strip_prefix('-').filter(|flags| !flags.is_empty()) {
            if !flags.chars().all(|flag| "nErsuz".contains(flag)) {
                return false;
            }
        } else if script.is_none() {
            script = Some(arg.as_str());
        }
    }
    script.is_some_and(|script| {
        script.split([';', '\n']).all(|command| {
            let command = command.trim();
            let Some(body) = command.strip_suffix(['p', 'd', '=']) else {
                return command.is_empty();
            };
            let body = body.trim_end().trim_end_matches('!').trim_end();
            body.split(',').count() <= 2
                && body
                    .split(',')
                    .all(|address| address == "$" || address.chars().all(|c| c.is_ascii_digit()))
        })
    })
}

/// [`runs_past_readers`] as `reading` asks: a line not read whole runs a program only when the
/// question is whether to raise a block.
fn reads_past_readers(line: &str, shells: usize, reading: Reading<'_>) -> bool {
    let place = match reading {
        Reading::Raising(place) => Some(place),
        Reading::Naming => None,
    };
    let unread = place.is_some();
    let Some(segments) = crate::shellseg::joined_argv(line) else {
        return unread;
    };
    // Every folder the line may have moved to by each part (#2212): where it ran, and where
    // each `cd` before took it; `None` once a `cd` names one that is not read.
    let mut folders: Option<Vec<PathBuf>> = place.map(|place| vec![place.cwd.to_path_buf()]);
    for segment in &segments {
        if segment
            .argv
            .first()
            .is_some_and(|word| HEADERS.contains(&word.as_str()))
        {
            continue;
        }
        let call = crate::shellwrap::split_env_chdir(&segment.argv);
        if call.too_deep {
            if unread {
                return true;
            }
            continue;
        }
        if call.prog.is_empty() {
            continue;
        }
        let base = crate::shellwrap::base_lower(&call.prog);
        if matches!(base.as_str(), "cd" | "pushd")
            && let (Some(place), Some(seen)) = (place, folders.as_mut())
        {
            let target = cd_target(&call.argv[1..]);
            let moved = seen
                .last()
                .and_then(|from| moved_to(from, target, place.home));
            match moved {
                Some(to) => seen.push(to),
                None => folders = None,
            }
        }
        if READERS.contains(&base.as_str()) {
            if unread && starts_a_program(&base, &call.argv) {
                return true;
            }
            continue;
        }
        if base == "git" {
            let (globals, rest) = crate::shellwrap::git_globals(&call.argv);
            if !rest
                .first()
                .is_some_and(|word| GIT_READERS.contains(&word.as_str()))
                || (unread && git_starts_a_program(&call.env, &globals, &rest))
            {
                return true;
            }
            if let Some(place) = place
                && git_reader_runs_a_program(folders.as_deref(), &call.chdir, &globals, place)
            {
                return true;
            }
            continue;
        }
        // A shell handed a string runs what the string does; one handed none runs a file.
        let scripts = crate::shellwrap::shell_scripts_of(&call.prog, &call.argv);
        if scripts.is_empty()
            || if shells < MOST_SHELLS_DEEP {
                scripts
                    .iter()
                    .any(|script| reads_past_readers(script, shells + 1, reading))
            } else {
                unread
            }
        {
            return true;
        }
    }
    false
}

/// The folder word `cd` or `pushd` is given, past its options (`-L`, `-P`, `-e`, `-@`, and
/// `--`), or `None` for none; `Some("-")` for a word that is not read (`pushd +1`).
fn cd_target(args: &[String]) -> Option<&str> {
    let mut words = args.iter().map(String::as_str);
    while let Some(word) = words.next() {
        if word == "--" {
            return words.next();
        }
        if word.len() > 1 && word.starts_with('-') && word[1..].chars().all(|c| "LPe@".contains(c))
        {
            continue;
        }
        if word.starts_with('+') || (word.starts_with('-') && word.len() > 1) {
            return Some("-");
        }
        return Some(word);
    }
    None
}

/// The folder `cd` with `target` moves to from `from`, or `None` for one that is not read: a
/// word the shell expands (a variable, a substitution, a glob), `cd -`, and a home that is
/// not known.
fn moved_to(from: &Path, target: Option<&str>, home: Option<&Path>) -> Option<PathBuf> {
    let target = target.unwrap_or("~");
    if target == "-" || target.contains(['$', '`', '*', '?', '[']) {
        return None;
    }
    let to = match target.strip_prefix('~') {
        Some("") => home?.to_path_buf(),
        Some(rest) if rest.starts_with('/') => home?.join(&rest[1..]),
        Some(_) => return None,
        None => from.join(target),
    };
    Some(lexical(&to))
}

/// **Whether a git reader may run a program its repository's config names** (#2212): in any
/// folder the line may have moved it to (`folders`, `None` for one not read), then a
/// wrapper's own folder (`chdir`) and each `-C` of its `globals`
/// ([`drivers::names_a_program`]).
fn git_reader_runs_a_program(
    folders: Option<&[PathBuf]>,
    chdir: &str,
    globals: &[String],
    place: &Place<'_>,
) -> bool {
    let Some(folders) = folders else {
        return true;
    };
    let mut moves: Vec<&str> = Vec::new();
    if !chdir.is_empty() {
        moves.push(chdir);
    }
    let mut words = globals.iter();
    while let Some(word) = words.next() {
        if word == "-C" {
            match words.next() {
                Some(dir) => moves.push(dir),
                None => return true,
            }
        } else if let Some(dir) = word.strip_prefix("-C") {
            moves.push(dir);
        }
    }
    folders.iter().any(|folder| {
        let mut at = Some(folder.clone());
        for to in &moves {
            at = at.and_then(|from| moved_to(&from, Some(to), place.home));
        }
        at.is_none_or(|at| drivers::names_a_program(&at, place.home))
    })
}

/// **Whether `payload` is the harness's own tool reading back a background command that ended
/// failed** (#1972): `BashOutput` with `status` `failed`, or a non-zero `exitCode`. Its output is
/// that command's own error, read as a failed command's is, and still marked read back
/// ([`read_back`]).
fn bash_output_failed(payload: &serde_json::Value) -> bool {
    let response = &payload["tool_response"];
    payload["tool_name"].as_str() == Some("BashOutput")
        && (response["status"].as_str() == Some("failed")
            || response["exitCode"].as_i64().is_some_and(|code| code != 0))
}

/// The harness's file tool, which the block words also hear (#1683); nothing it read is a
/// block (#1972).
const READ_TOOL: &str = "Read";

/// The tools a block word hears that run nothing: each reads back what a background command
/// printed earlier (#1683), the file tool and the harness's own tool for it.
const READING_TOOLS: [&str; 2] = [READ_TOOL, "BashOutput"];

/// **Whether `payload` is of a tool that ran nothing** (#1884): a file tool's read, or the
/// harness's tool that reads a background command's output back. A block found in one is of
/// words read, not of a command this call ran: the app shows it as a quiet line nobody has to
/// answer, and the chat is not told to wait on it ([`told_the_chat_of_a_read`]). A command a
/// chat runs (`Bash`, and Codex's and opencode's shells, which are `Bash` to a hook) is never
/// one, whatever it prints.
pub fn read_back(payload: &serde_json::Value) -> bool {
    payload["tool_name"]
        .as_str()
        .is_some_and(|tool| READING_TOOLS.contains(&tool))
}

/// `found`, each block once ([`is_the_same`]) and at most [`AT_MOST_PER_RESULT`]. **A block
/// read twice keeps its name**: where one reading names a target and the other none (the
/// sandbox's own record beside the program's words for the same refusal), the one block names
/// it, whichever was read first. Of two names, the first stands.
fn at_most(found: Vec<(Block, Option<String>)>) -> Vec<(Block, Option<String>)> {
    let mut blocks: Vec<(Block, Option<String>)> = Vec::new();
    for (block, target) in found {
        let read = (block, target);
        if let Some(seen) = blocks.iter_mut().find(|seen| is_the_same(seen, &read)) {
            if seen.1.is_none() {
                seen.1 = read.1;
            }
        } else if blocks.len() < AT_MOST_PER_RESULT {
            blocks.push(read);
        }
    }
    blocks
}

/// Whether two blocks of one result are one: the same block, and for a refused host the same
/// host (#1631), as [`Throttle::lets_on`] hears them, so each host a command was refused gets
/// its own Allow. A refusal that names no host (a client's `403`) is the one a line beside it
/// named. Every other block is one pattern whatever it names: a command refused a thousand
/// files is one block.
fn is_the_same(
    (one, on): &(Block, Option<String>),
    (other, other_on): &(Block, Option<String>),
) -> bool {
    one == other
        && (one.kind != Kind::Host
            || match (on, other_on) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => true,
            })
}

/// The target a grant could name for `block`, refused on `named` (a host, or a path read
/// against `place`), where one may be granted at all.
fn target_of(block: &Block, named: &str, place: &Place<'_>) -> Option<String> {
    let named = named.trim();
    if named.is_empty() {
        return None;
    }
    match (block.operation, block.kind) {
        (Operation::Connect | Operation::Lookup, Kind::Host) => Some(named.to_owned()),
        // A local socket by its whole path, as a grant names it (D-sock-1); a refusal that names
        // its socket as a URL (`unix:///var/run/docker.sock`) names the same path.
        (Operation::Connect, Kind::LocalSocket) => {
            let path = named.strip_prefix("unix://").unwrap_or(named);
            path.starts_with('/').then(|| path.to_owned())
        }
        // A service on this Mac by its one port (qw384), from a loopback address with its port
        // or the port alone, judged as a grant judges one.
        (Operation::Connect, Kind::LocalService) => {
            let port = crate::sandbox::localport::loopback_port(named)
                .map_or_else(|| named.to_owned(), |port| port.to_string());
            crate::sandbox::localport::port(&port)
                .ok()
                .map(|port| port.to_string())
        }
        // The certificate check is one service, whatever the line said of it.
        (Operation::Lookup, Kind::CertificateCheck) => {
            Some(crate::sandbox::services::TRUSTD.to_owned())
        }
        (Operation::Lookup, Kind::SystemService) => crate::sandbox::services::service(named).ok(),
        (
            Operation::Write | Operation::File,
            Kind::ProjectFiles | Kind::Home | Kind::ToolchainCache | Kind::System,
        ) => Some(lexical(&place.cwd.join(named)).display().to_string()),
        // A branch folder's own git record (#1476): no grant names it, and its path lets the
        // Notice name the command that commits there ([`way_out`]).
        (Operation::Write | Operation::File, Kind::ProtectedFile) => {
            let path = lexical(&place.cwd.join(named));
            in_a_branch_folders_record(&path).then(|| path.display().to_string())
        }
        _ => None,
    }
}

/// **The purlis command that does what a refused write of `path` was for** (#1408, #1476), in
/// the project at `root`: the way out a block's Notice names in place of a grant, in fixed words
/// that hold no path. A file purlis writes for a chat (ADR 0067 §2: session records, the
/// memories, todos and `workspace.md`) names the command that writes it; a branch folder's git
/// record (`.git/worktrees/<name>`, where `git add` and `git commit` there write) names
/// `purlis worktree commit`. `None` for anything else, which keeps its own way out.
///
/// Chosen by the path alone: the block keeps its [`Kind`], and nothing is kept of the path.
pub fn way_out(path: &Path, root: &Path) -> Option<&'static str> {
    let path = unprivate(&lexical(path));
    if in_a_branch_folders_record(&path) {
        return Some(WORKTREE_COMMIT_WAY);
    }
    let within = path.strip_prefix(unprivate(&lexical(root))).ok()?;
    let parts: Vec<&str> = within
        .components()
        .map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        ["sessions", _] | ["workspaces", _, "sessions", _] => Some(
            "purlis writes session records for a chat: write one with `purlis session record`.",
        ),
        ["workspaces", _, "memory", _, ..] => Some(
            "purlis writes a workspace's memory for a chat: record one with `purlis workspace \
             remember \"<fact>\"`.",
        ),
        ["personas", _, "memory", _, ..] => Some(
            "purlis writes a persona's memory for a chat: record one with `purlis persona \
             remember`, and `--shared` for shared memory.",
        ),
        ["workspaces", _, "todos", _, ..] => Some(
            "purlis writes a workspace's todos for a chat: add one with `purlis workspace todo \
             \"<text>\"`.",
        ),
        // A new workspace's own folder (#1408): purlis makes a workspace for a chat.
        ["workspaces", _] => Some(NEW_WORKSPACE_WAY),
        ["workspaces", _, "workspace.md"] => Some(
            "purlis writes workspace.md for a chat: set its vision with `purlis workspace vision \
             \"<text>\"`, and add to a section with the workspace_section tool. A new workspace \
             is made with `purlis workspace create <name> --vision \"<text>\"`.",
        ),
        _ => None,
    }
}

/// What a Notice says of a refused write of a new workspace's folder (#1408): the brokered
/// `workspace create` (`crate::brokered`'s `WorkspaceCreate`), with no name in it, since the
/// Notice keeps no path.
const NEW_WORKSPACE_WAY: &str = "purlis makes a workspace for a chat: create one with `purlis \
     workspace create <name> --vision \"<text>\"`.";

/// What a Notice says of a refused write in a branch folder's git record (#1476, #1055).
const WORKTREE_COMMIT_WAY: &str = "A branch folder keeps its git record in its clone's .git, \
     which no chat writes, so git add and git commit are refused there. Commit with `purlis \
     worktree commit -m \"<message>\" --all`, or with paths in place of --all: the commit is made on \
     the folder's branch, outside the sandbox.";

/// Whether `path` is in a linked worktree's own record in its clone's git directory
/// (`.git/worktrees/<name>/…`), where git keeps a branch folder's index.
fn in_a_branch_folders_record(path: &Path) -> bool {
    let parts: Vec<&std::ffi::OsStr> = path
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    parts
        .windows(3)
        .any(|three| three[0] == ".git" && three[1] == "worktrees")
}

/// Whether a line of standard output may name `block`'s target: a refused lookup of a host,
/// which is shown and never offered to allow; a local socket, whose path a client says only in
/// its own error, so only a failed command or a container client names one (D-sock-5, [`Said`]), shown
/// whole with what allowing it means and judged again as a grant (a socket on this machine now,
/// where no chat writes); and the certificate check, which names one fixed service whatever the
/// line said.
fn names_a_lookup(block: &Block) -> bool {
    matches!(
        (block.operation, block.kind),
        (Operation::Lookup, Kind::Host)
            | (Operation::Connect, Kind::LocalSocket)
            | (Operation::Connect, Kind::LocalService)
            | (Operation::Lookup, Kind::CertificateCheck)
    )
}

/// What a command's standard output says of the network (#1663): each line that ends as a
/// refusal does ([`ends_as_a_refusal`]). It never names a host to allow, only the host a
/// lookup was of; a violation block and a program's own refusal of a path are never read here.
///
/// Only a refusal the command may have met is read (#1972): what a line that only reads or
/// prints handed back ([`Said::Printed`]) is none. A local
/// socket is named only where the command failed, or its line runs a container engine's client
/// (`said`, D-sock-5): a client's own error is the one place the path is said.
fn on_stdout(text: &str, place: &Place<'_>, said: Said) -> Vec<(Block, Option<String>)> {
    if said == Said::Printed {
        return Vec::new();
    }
    text.lines()
        .filter(|line| ends_as_a_refusal(line))
        .filter_map(network_refusal)
        .map(|(operation, kind, host)| {
            let block = Block {
                operation,
                kind,
                ours: false,
            };
            let target = host
                // A socket or a service only from a client's own error (D-sock-5).
                .filter(|_| names_a_lookup(&block) && said.names(block.kind))
                .and_then(|host| target_of(&block, &host, place));
            (block, target)
        })
        .collect()
}

/// What a command's standard error says: its violation block, else a program's own refusals of
/// paths outside what the chat may write; and any proxy's or certificate check's refusal.
fn on_stderr(text: &str, place: &Place<'_>) -> Vec<(Block, Option<String>)> {
    let mut out = reported_or_of_paths(text, place);
    out.extend(of_the_network(text, place));
    out
}

/// What standard error says but for the network: its violation block, else purlis's own words
/// and a program's own refusals of paths outside what the chat may write.
fn reported_or_of_paths(text: &str, place: &Place<'_>) -> Vec<(Block, Option<String>)> {
    let mut out = violations(text, place);
    // Claude Code's own lines say exactly what was refused; a program's words for the same
    // refusal would only count it again, less well.
    if out.is_empty() {
        let lines: Vec<&str> = text.lines().collect();
        // purlis's own words for a write the chat's sandbox refused it (#1421): purlis's own
        // block, which no grant names, as a violation line naming `purlis` is.
        out.extend(
            lines
                .iter()
                .filter_map(|line| purlis_refusal(line, place))
                .map(|kind| {
                    (
                        Block {
                            operation: Operation::Write,
                            kind,
                            ours: true,
                        },
                        None,
                    )
                }),
        );
        out.extend(
            (0..lines.len())
                .filter(|at| purlis_refusal(lines[*at], place).is_none())
                .filter_map(|at| refused_path(&lines, at, place))
                .filter(|(_, kind, _)| !matches!(kind, Kind::ChatFolder | Kind::Temp))
                // A read there that macOS's privacy controls refuse (`du`, `find`, `ls`) says
                // the same words: only a write is the sandbox's.
                .filter(|(operation, kind, _)| {
                    *operation == Operation::Write
                        || !matches!(kind, Kind::SystemTemp | Kind::SystemCache)
                })
                .map(|(operation, kind, path)| {
                    let block = Block {
                        operation,
                        kind,
                        ours: false,
                    };
                    let target = target_of(&block, &path, place);
                    (block, target)
                }),
        );
    }
    out
}

/// Any proxy's, lookup's, socket's or certificate check's refusal on a line of standard error,
/// with what it names.
fn of_the_network(text: &str, place: &Place<'_>) -> Vec<(Block, Option<String>)> {
    text.lines()
        .filter_map(network_refusal)
        .map(|(operation, kind, host)| {
            let block = Block {
                operation,
                kind,
                ours: false,
            };
            let target = host.and_then(|host| target_of(&block, &host, place));
            (block, target)
        })
        .collect()
}

/// The denials Claude Code reported for the command, in the block it appended to `text`
/// ([`denial::reported`]), each sorted: the sandbox's own record, read before anything the
/// program said.
fn violations(text: &str, place: &Place<'_>) -> Vec<(Block, Option<String>)> {
    denial::reported(text)
        .iter()
        .map(|denial| denial.block(place))
        .collect()
}

/// The text between the last two quotes (`'`, `` ` `` or `"`) in `field`: a destination comes
/// after its source (`cannot copy 'a' to 'b'`), and a word's apostrophe before them is not one
/// of them (`couldn't create cache file '/x'`, #1416).
fn last_quoted(field: &str) -> Option<&str> {
    for quote in ['\'', '`', '"'] {
        let parts: Vec<&str> = field.split(quote).collect();
        if parts.len() >= 3 {
            return Some(parts[parts.len() - 2]);
        }
    }
    None
}

/// Whether `said` names a write: the verb or a stem of one.
fn writes(program: &str, said: &str) -> bool {
    let said = said.to_ascii_lowercase();
    [
        "touch", "mkdir", "cp", "mv", "ln", "rm", "tee", "install", "rmdir", "mktemp",
    ]
    .contains(&program)
        || [
            "lock",
            "creat",
            "writ",
            "renam",
            "remov",
            "delet",
            "copy",
            "mkdir",
            "mkstemp",
            "mkdtemp",
            "output file",
        ]
        .iter()
        .any(|stem| said.contains(stem))
}

/// The kind of path a line of purlis's own refusal wording names, where `line` is one: the
/// sandboxed [`crate::rewrite::refused_write`] ("this chat's sandbox refused writing <path>
/// (Operation not permitted). …"), or a sentence carrying [`crate::rewrite::os_words`]'s clause
/// ("… <path> (Operation not permitted: this chat's sandbox refused it) …"). In the second the
/// path is the caller's, read as the last absolute path before the clause (a relative one there
/// is the project's, not the command's folder's); a line that names none is the project's
/// files, which is what purlis writes.
fn purlis_refusal(line: &str, place: &Place<'_>) -> Option<Kind> {
    use crate::rewrite::{NOT_PERMITTED, SANDBOX_REFUSED_IT, SANDBOX_REFUSED_WRITING};
    if let Some(at) = line.find(SANDBOX_REFUSED_WRITING) {
        let rest = &line[at + SANDBOX_REFUSED_WRITING.len()..];
        let path = rest
            .split(NOT_PERMITTED)
            .next()
            .filter(|_| rest.contains(NOT_PERMITTED))?;
        return Some(kind_of(Path::new(path), place));
    }
    let at = line.find(&format!("Operation not permitted: {SANDBOX_REFUSED_IT}"))?;
    let named = line[..at]
        .split_whitespace()
        .rev()
        .map(|word| word.trim_matches(|c: char| "'\"`(),:".contains(c)))
        .find(|word| word.starts_with('/'));
    Some(named.map_or(Kind::ProjectFiles, |path| kind_of(Path::new(path), place)))
}

/// A program's own "Operation not permitted" at line `at` of `lines`, naming its path, sorted.
fn refused_path(lines: &[&str], at: usize, place: &Place<'_>) -> Option<(Operation, Kind, String)> {
    let line = lines[at].trim_end();
    // Node: `EPERM: operation not permitted, mkdir '/opt/x'`.
    if let Some(start) = line.find("EPERM: operation not permitted, ") {
        let rest = &line[start + "EPERM: operation not permitted, ".len()..];
        let (call, quoted) = rest.split_once(' ')?;
        let path = quoted.trim().strip_prefix('\'')?.split('\'').next()?;
        let operation = match call {
            "connect" => return Some((Operation::Connect, Kind::LocalSocket, String::new())),
            "mkdir" | "rmdir" | "rename" | "unlink" | "write" | "copyfile" | "symlink" | "link"
            | "utime" | "chmod" => Operation::Write,
            "scandir" | "read" => Operation::Read,
            _ => Operation::File,
        };
        return Some((operation, kind_of(Path::new(path), place), path.to_owned()));
    }
    // Python: `PermissionError: [Errno 1] Operation not permitted: '/opt/x'`.
    if let Some(start) = line.find("[Errno 1] Operation not permitted: '") {
        let path = line[start + "[Errno 1] Operation not permitted: '".len()..]
            .split('\'')
            .next()?;
        return Some((
            Operation::File,
            kind_of(Path::new(path), place),
            path.to_owned(),
        ));
    }
    // Foundation, as a Swift program prints its error (#1416): `Error Domain=NSCocoaErrorDomain
    // Code=513 "…" UserInfo={NSFilePath=/x/y, NSUnderlyingError=0x… {Error
    // Domain=NSPOSIXErrorDomain Code=1 "Operation not permitted"}}`. 513 is a refused write, and
    // the underlying POSIX error says it was not permitted. A line without the path names
    // nothing to sort, and is no block. The path is read from the last `NSFilePath=`, past the
    // quoted description a file's own name is said in, to the next `, NS` key or the closing
    // braces, so a path holding a comma or a brace is read whole.
    if line.contains("NSCocoaErrorDomain Code=513 ")
        && line.contains("NSPOSIXErrorDomain Code=1 \"Operation not permitted\"")
    {
        let rest = line.rsplit_once("NSFilePath=")?.1;
        let path = match rest.find(", NS") {
            Some(end) => &rest[..end],
            None => rest.trim_end().trim_end_matches('}'),
        }
        .trim();
        return path.starts_with('/').then(|| {
            (
                Operation::Write,
                kind_of(Path::new(path), place),
                path.to_owned(),
            )
        });
    }
    let lower = line.to_ascii_lowercase();
    // Rust's io error on a line of its own, its path on a line before (cargo's `Caused by:`):
    // `failed to create directory `/x``, then `Operation not permitted (os error 1)`.
    if lower.trim() == "operation not permitted (os error 1)" {
        let named = lines[..at]
            .iter()
            .rev()
            .take(4)
            .find_map(|before| last_quoted(before).map(|path| (*before, path)))?;
        let operation = if writes("", named.0) {
            Operation::Write
        } else {
            Operation::File
        };
        return Some((
            operation,
            kind_of(Path::new(named.1), place),
            named.1.to_owned(),
        ));
    }
    // A program's own, in any case: `touch: /opt/x: Operation not permitted`,
    // `mkdir: cannot create directory '/opt/x': Operation not permitted`,
    // `fatal: could not create work tree dir 'x': Operation not permitted`,
    // `open /x/y: operation not permitted` (Go), `… `/x`: Operation not permitted (os error 1)`,
    // swiftc's and clang's `error: unable to open output file '/x': 'Operation not permitted'`,
    // and xcrun's `couldn't create cache file '/x' (errno=Operation not permitted)` (#1416).
    // The line has to END there, so a line quoting one (a test's source, a log) is not one.
    let cut = [
        ": operation not permitted (os error 1)",
        ": operation not permitted",
        ": 'operation not permitted'",
        " (errno=operation not permitted)",
    ]
    .iter()
    .find_map(|suffix| lower.strip_suffix(suffix).map(str::len))?;
    let before = &line[..cut];
    let (program, said) = match before.split_once(": ") {
        Some((program, said)) if is_program(program) => (program, said),
        _ => ("", before),
    };
    let field = said.rsplit(": ").next().unwrap_or(said);
    let path = match last_quoted(field) {
        Some(quoted) => quoted,
        None => {
            let word = field.rsplit(' ').next().unwrap_or(field);
            if !(word.contains('/') || word.starts_with('.')) {
                return None;
            }
            word
        }
    };
    if path.is_empty() {
        return None;
    }
    let verb = program.rsplit('/').next().unwrap_or(program);
    let operation = if writes(verb, said) {
        Operation::Write
    } else {
        Operation::File
    };
    Some((operation, kind_of(Path::new(path), place), path.to_owned()))
}

/// Whether `word` reads as the program a line begins with: `touch`, `git`, `/usr/bin/cp`, `fatal`.
fn is_program(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | '+'))
}

/// Whether Docker's own "permission denied" sentence, with no `EPERM` beside it, is the
/// sandbox's on this platform (#1637): on macOS, Seatbelt refuses a socket so and Docker says
/// only that. Elsewhere it is a user outside the docker group, which no grant or Start without
/// the sandbox mends.
const DOCKER_WORDS_ARE_THE_SANDBOXS: bool = cfg!(target_os = "macos");

/// A refusal of a connection or of the certificate check, from whichever program said it, as
/// this platform reads it ([`network_refusal_on`]).
fn network_refusal(line: &str) -> Option<(Operation, Kind, Option<String>)> {
    network_refusal_on(line, DOCKER_WORDS_ARE_THE_SANDBOXS)
}

/// [`network_refusal`], with `docker_words` saying whether Docker's "permission denied" sentence
/// alone is the sandbox's ([`DOCKER_WORDS_ARE_THE_SANDBOXS`]).
fn network_refusal_on(line: &str, docker_words: bool) -> Option<(Operation, Kind, Option<String>)> {
    const REFUSED: [&str; 4] = [
        // purlis's own egress proxy (`sandbox::egress`).
        "purlis's sandbox does not allow ",
        // Claude Code's proxy, in its body and its header.
        "blocked by network allowlist",
        "blocked-by-allowlist",
        // curl's word for the 403 a proxy answered its tunnel with.
        "CONNECT tunnel failed, response 403",
    ];
    if REFUSED.iter().any(|said| line.contains(said)) {
        // purlis's own proxy names the host and port it refused: `… does not allow h:443: …`.
        let host = line
            .split_once(REFUSED[0])
            .and_then(|(_, rest)| rest.split(": ").next())
            .filter(|host| !host.is_empty() && !host.contains(char::is_whitespace))
            .map(str::to_owned);
        return Some((Operation::Connect, Kind::Host, host));
    }
    if line.contains("x509") && line.contains("-26276") {
        return Some((
            Operation::Lookup,
            Kind::CertificateCheck,
            Some(crate::sandbox::services::TRUSTD.to_owned()),
        ));
    }
    let lower = line.to_ascii_lowercase();
    // A program that looks a host up itself, not through the proxy: the sandbox refuses the
    // lookup, and macOS's resolver says only that the name is not known (`EAI_NONAME`), in its
    // words, curl's, Go's or Node's (#1631, #1663). Allowing the host would not let such a
    // program through, so the host it names is said and never offered ([`looked_up`]).
    if lower.contains("nodename nor servname provided, or not known")
        || lower.contains("could not resolve host: ")
        || (lower.contains("lookup ") && lower.contains(": no such host"))
        || lower.contains("getaddrinfo enotfound ")
    {
        return Some((Operation::Lookup, Kind::Host, looked_up(line)));
    }
    // A service on this Mac (qw384): Go's or Node's connect to a loopback port refused by the
    // sandbox (`EPERM`). A port only, never a host: allowing a host reaches nothing here.
    if let Some(port) = crate::sandbox::localport::refused_in(line) {
        return Some((
            Operation::Connect,
            Kind::LocalService,
            Some(port.to_string()),
        ));
    }
    // A local socket: Go's dial refused by the sandbox (`EPERM`), on any platform; or Docker's
    // client in its own words where the sandbox says them (#1637). A socket's own file mode
    // (`EACCES`, "connect: permission denied") is never the sandbox's.
    let eperm = lower.contains("dial unix ") && lower.contains("connect: operation not permitted");
    let docker = docker_words
        && lower.contains("permission denied while trying to connect to the docker")
        && !lower.contains("connect: permission denied");
    (eperm || docker).then(|| (Operation::Connect, Kind::LocalSocket, socket_named(line)))
}

/// The socket a refused connection's line names: Go's `dial unix /path: connect`, or Docker's
/// `… at unix:///path` (#1637). None where it names none whole.
fn socket_named(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let path = if let Some(at) = lower.find("dial unix ") {
        line[at + "dial unix ".len()..].split(':').next()
    } else if let Some(at) = lower.find("unix://") {
        line[at + "unix://".len()..]
            .split(|c: char| c.is_whitespace() || c == ':' || c == '"')
            .next()
    } else {
        None
    }?;
    let path = path.trim().trim_end_matches(['.', ',', ';']);
    (path.starts_with('/') && path.len() <= 1024 && !path.contains(char::is_control))
        .then(|| path.to_owned())
}

/// The host a refused lookup's line names, in whichever program's words: psql's quoted
/// `host name "h"`, ssh's `hostname h:`, curl's and git's `host: h`, Go's `lookup h:` or
/// `lookup h on …`, Node's `ENOTFOUND h`. None where it names none, or where the word there
/// could be no host ([`a_host_word`]).
fn looked_up(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let after = |marker: &str| -> Option<&str> {
        let at = lower.rfind(marker)? + marker.len();
        Some(&line[at..])
    };
    let word = if let Some(rest) = after("could not translate host name \"") {
        rest.split('"').next()
    } else if let Some(rest) = after("could not resolve hostname ") {
        rest.split(':').next()
    } else if let Some(rest) = after("could not resolve host: ") {
        rest.split_whitespace().next()
    } else if let Some(rest) = after("getaddrinfo enotfound ") {
        rest.split_whitespace().next()
    } else if let Some(rest) = after("lookup ") {
        rest.split([':', ' ']).next()
    } else {
        None
    }?;
    let word = word.trim().trim_end_matches(['.', ',', ';']);
    a_host_word(word).then(|| word.to_owned())
}

/// Whether `word` could be a host name, with a port or not: letters, digits, dots and hyphens,
/// a colon before a port, and no longer than a name may be. What the app shows is checked again
/// as a grant checks a host.
fn a_host_word(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= 260
        && word.contains(|c: char| c.is_ascii_alphanumeric())
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
}

/// **Whether `line` ends where a refusal's own words do**: how a line of standard output is
/// read (#1663), so a file that quotes the words is not a refusal. The words that end a
/// refusal (macOS's resolver, Go's, a refused socket, a proxy's), else a host word right after
/// the words that name one (curl's `host:`, Node's `ENOTFOUND`), else Docker's own sentence
/// opening the line. A full stop after them is the program's.
///
/// Go's refused certificate check ends where its status does (`x509: OSStatus -26276`): a Go
/// program that failed on it, `gh` or `glab`, exits non-zero, so its words reach the hook only
/// in the failure's `error`, which is read here. Without the ending, no Notice came up for it.
fn ends_as_a_refusal(line: &str) -> bool {
    let lower = line.trim().trim_end_matches('.').to_ascii_lowercase();
    const ENDINGS: [&str; 7] = [
        "nodename nor servname provided, or not known",
        ": no such host",
        "connect: operation not permitted",
        "connect tunnel failed, response 403",
        "blocked by network allowlist",
        "or host of this project lists it",
        "x509: osstatus -26276",
    ];
    if ENDINGS.iter().any(|ending| lower.ends_with(ending)) {
        return true;
    }
    // Node's refused connect ends with its own local address: `connect EPERM 127.0.0.1:5432 -
    // Local (0.0.0.0:0)` (qw384).
    if lower.contains("connect eperm ")
        && lower.ends_with(')')
        && crate::sandbox::localport::refused_in(line).is_some()
    {
        return true;
    }
    let host_last = ["could not resolve host: ", "getaddrinfo enotfound "]
        .iter()
        .any(|marker| {
            lower
                .rsplit_once(marker)
                .is_some_and(|(_, rest)| a_host_word(rest.trim()))
        });
    if host_last {
        return true;
    }
    let opening = lower
        .strip_prefix("docker: ")
        .or_else(|| lower.strip_prefix("got "))
        .unwrap_or(&lower);
    opening.starts_with("permission denied while trying to connect to the docker")
}

/// **Whether purlis's own proxy tells the app each host a chat of `harness` was refused**
/// (#1663): a harness purlis wraps (Codex, opencode) reaches the network only through it
/// (`crate::sandbox::egress`), and the app hears its refusals by name. Such a chat's hook leaves
/// a refused host to it, so one refusal is one Notice; a lookup or a socket it still reads.
pub fn the_proxy_tells_hosts(harness: Option<&str>) -> bool {
    matches!(
        harness.and_then(crate::harness::Harness::of_registry_or_kind),
        Some(crate::harness::Harness::Codex | crate::harness::Harness::Opencode)
    )
}

/// **The local sockets the system's name lookup is reached through** on Linux (#2428):
/// systemd-resolved's, nscd's (under either name of its folder), sssd's and avahi's (mDNS). No
/// profile purlis compiles reaches one, since a chat's names are looked up by purlis's proxy,
/// so a program's own lookup is refused there: a lookup past the proxy, not a socket to allow.
pub const RESOLVER_SOCKETS: [&str; 7] = [
    "/run/systemd/resolve/io.systemd.Resolve",
    "/run/nscd/socket",
    "/var/run/nscd/socket",
    "/var/lib/sss/pipes/nss",
    "/run/avahi-daemon/socket",
    "/var/run/avahi-daemon/socket",
    "/run/systemd/resolve/io.systemd.Resolve.Monitor",
];

/// **Whether `block` is purlis's proxy's to tell** where a harness purlis wraps runs behind it
/// ([`the_proxy_tells_hosts`]): a refused connection to an internet host, which the proxy names
/// by host. Neither the chat's hook nor its wrap's supervisor ([`supervised`]) tells one.
pub fn is_the_proxys(block: &Block) -> bool {
    (block.operation, block.kind) == (Operation::Connect, Kind::Host)
}

/// What a block hook of a chat of `harness` on `os` tells the app of `found`: all of it, but for
/// a refused host where purlis's own proxy tells that itself ([`the_proxy_tells_hosts`]), and,
/// where purlis wraps the chat on Linux, every file, local socket and service its supervisor
/// tells by name ([`supervised`], #2344). One refusal is one Notice.
pub fn the_hooks_own(
    found: Vec<(Block, Option<String>)>,
    harness: Option<&str>,
    os: crate::sandbox::Os,
) -> Vec<(Block, Option<String>)> {
    if !the_proxy_tells_hosts(harness) {
        return found;
    }
    let supervised = harness
        .and_then(crate::harness::Harness::of_registry_or_kind)
        .is_some_and(|harness| os.purlis_wraps(harness));
    found
        .into_iter()
        .filter(|(block, _)| !is_the_proxys(block))
        .filter(|(block, _)| !(supervised && supervised::tells(block)))
        .collect()
}

/// **`payload` as a chat of `harness` reports a command** (qw384): a harness purlis wraps
/// (Codex, opencode) runs in purlis's own sandbox, which appends no violation report, so a
/// `<sandbox_violations>` block in its result is one the command printed. It is read there as
/// any other printed text, never as the sandbox's own report. Any other harness's payload is
/// as it came.
pub fn as_its_harness_reports(
    payload: &serde_json::Value,
    harness: Option<&str>,
) -> serde_json::Value {
    const OPEN: &str = "<sandbox_violations>";
    if !the_proxy_tells_hosts(harness) {
        return payload.clone();
    }
    fn unreport(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(text) if text.contains(OPEN) => {
                *text = text.replace(OPEN, "<sandbox_violations printed>");
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(unreport),
            serde_json::Value::Object(fields) => fields.values_mut().for_each(unreport),
            _ => {}
        }
    }
    let mut out = payload.clone();
    for field in ["tool_response", "error"] {
        if let Some(value) = out.get_mut(field) {
            unreport(value);
        }
    }
    out
}

/// **The port a command's own printed words name** for a refused service on this Mac (qw384):
/// Go's or Node's refusal of a loopback port, anywhere in what the command handed back. Never a
/// target: the Notice pre-fills its port field with it, says it came from the command's output
/// for the person to check, and a grant judges it again.
pub fn printed_port(payload: &serde_json::Value) -> Option<u16> {
    let response = &payload["tool_response"];
    [
        response.as_str(),
        response["stderr"].as_str(),
        response["stdout"].as_str(),
        payload["error"].as_str(),
    ]
    .into_iter()
    .flatten()
    .flat_map(str::lines)
    .find_map(crate::sandbox::localport::refused_in)
}

/// The kind of `path`, read against `place` and never kept, with this process's temporary
/// folder as the chat's.
pub fn kind_of(path: &Path, place: &Place<'_>) -> Kind {
    kind_of_in(path, place, &std::env::temp_dir())
}

/// [`kind_of`], with `temp` the chat's temporary folder: what the wrap's supervisor, which
/// starts with none of the chat's environment, was told it is (#2460).
pub fn kind_of_in(path: &Path, place: &Place<'_>, temp: &Path) -> Kind {
    let path = lexical(&place.cwd.join(path));
    if is_planted(&path) {
        return Kind::ProtectedFile;
    }
    let within = |dir: &Path| {
        let dir = lexical(dir);
        path.starts_with(&dir) || unprivate(&path).starts_with(unprivate(&dir))
    };
    // A manifest where the sandbox holds one: at the root, or in a folder from the chat's up to
    // it (#1336).
    if crate::sandbox::manifests_held(place.root, Some(place.chat))
        .iter()
        .any(|held| unprivate(&path) == unprivate(&lexical(&held.path)))
    {
        return Kind::ProtectedFile;
    }
    // Under either spelling of the state folder: the one the project has may not be the one
    // a chat was refused writing.
    if crate::names::STATE_DIR
        .spellings()
        .any(|name| within(&place.root.join(name)))
    {
        return Kind::ProjectState;
    }
    if within(place.chat) {
        return Kind::ChatFolder;
    }
    if within(place.root) {
        return Kind::ProjectFiles;
    }
    // Before the temp folders: a hook whose own `TMPDIR` is the per-user folder itself would
    // otherwise take a tool's write there for the chat's own.
    if let Some(parts) = per_user_folder(&unprivate(&path))
        && !(within(temp)
            && per_user_folder(&unprivate(&lexical(temp))).is_some_and(|at| at.len() > 1))
    {
        return if parts[0] == "C" {
            Kind::SystemCache
        } else {
            Kind::SystemTemp
        };
    }
    if [
        "/tmp",
        "/private/tmp",
        "/var/folders",
        "/private/var/folders",
    ]
    .iter()
    .any(|dir| within(Path::new(dir)))
        || within(temp)
    {
        return Kind::Temp;
    }
    if let Some(home) = place.home.filter(|home| within(home)) {
        let inside = unprivate(&path)
            .strip_prefix(unprivate(&lexical(home)))
            .map(Path::to_path_buf)
            .or_else(|_| path.strip_prefix(lexical(home)).map(Path::to_path_buf))
            .unwrap_or_default();
        let first = inside.components().next();
        let caches = [
            ".cargo",
            ".rustup",
            ".npm",
            ".pnpm-store",
            ".yarn",
            ".bun",
            ".deno",
            ".cache",
            ".gradle",
            ".m2",
            "go",
        ];
        let cached = first.is_some_and(|first| caches.iter().any(|c| first.as_os_str() == *c))
            || inside.starts_with("Library/Caches");
        return if cached {
            Kind::ToolchainCache
        } else {
            Kind::Home
        };
    }
    Kind::System
}

/// What `path` names below one of macOS's per-user folders, as its parts from that folder on:
/// `["T", "tmp.x"]`. `None` anywhere else. Read from the path's shape alone, so the hook asks
/// the system nothing. The four are `/var/folders/<a>/<b>/T` (`getconf DARWIN_USER_TEMP_DIR`),
/// `…/C` (`DARWIN_USER_CACHE_DIR`), `…/0` (`DARWIN_USER_DIR`, where system services keep a
/// user's state) and `…/X` (the system's per-user code-signing clones) (#1416): none of them
/// is a temporary folder a chat may write, so none is sorted as one.
fn per_user_folder(path: &Path) -> Option<Vec<&std::ffi::OsStr>> {
    let mut parts = path.components();
    if parts.next() != Some(Component::RootDir) {
        return None;
    }
    let names: Vec<&std::ffi::OsStr> = parts
        .map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect::<Option<_>>()?;
    match names.as_slice() {
        [var, folders, _, _, which, ..]
            if *var == "var"
                && *folders == "folders"
                && ["T", "C", "0", "X"].iter().any(|one| *which == *one) =>
        {
            Some(names[4..].to_vec())
        }
        _ => None,
    }
}

/// Whether `path` is, or is under, a name the sandbox protects wherever it is
/// ([`crate::sandbox::PLANTED`]).
///
/// A lock file git writes beside one (`.git/config.lock`) is that file: git is refused there
/// first.
pub(crate) fn is_planted(path: &Path) -> bool {
    let mut parts: Vec<&str> = path
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    if let Some(last) = parts.last_mut() {
        *last = last.strip_suffix(".lock").unwrap_or(last);
    }
    crate::sandbox::PLANTED.iter().any(|planted| {
        if planted.path.contains("**") {
            return false;
        }
        let wanted: Vec<&str> = planted.path.split('/').collect();
        parts.windows(wanted.len()).enumerate().any(|(at, here)| {
            here == wanted.as_slice()
                && (planted.reach == crate::sandbox::Reach::AndBelow
                    || at + wanted.len() == parts.len())
        })
    })
}

/// `path` with `.` and `..` taken out, without asking the file system.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// macOS's `/private/var/…` and `/var/…` are one folder: the sandbox reports one, a project
/// may be named by the other. So are `/System/Volumes/Data/…` and `/…`, the data volume's own
/// spelling of what is firmlinked into the root (#1416): `/System/Volumes/Data/private/var/…`
/// is `/var/…` too.
fn unprivate(path: &Path) -> PathBuf {
    let path = match path.strip_prefix("/System/Volumes/Data") {
        Ok(rest) if rest.components().next().is_some() => Path::new("/").join(rest),
        _ => path.to_path_buf(),
    };
    match path.strip_prefix("/private") {
        Ok(rest)
            if ["var", "tmp", "etc"]
                .iter()
                .any(|top| rest.starts_with(top)) =>
        {
            Path::new("/").join(rest)
        }
        _ => path,
    }
}

// ---- how often the app takes one -------------------------------------------------------------

/// How long one chat's same block is heard once, and the window [`Throttle::PER_CHAT`] counts in.
pub const THROTTLE_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

/// **What the app lets through, per chat** (#1338): the same block once a minute, and at most
/// [`Throttle::PER_CHAT`] blocks a minute in all. A chat holds its own token, so it can send
/// the app as many block lines as it likes; each one kept rewrites the store, and this keeps
/// that to a handful a minute and the real blocks in it from being pushed out.
///
/// **The same block is the same block on the same target** ([`Throttle::lets_on`]): a host
/// refused a minute after another is its own block, with its own Notice to allow it, or the
/// second would have no Allow for a minute. Hosts compare without case.
///
/// **A repeat held back is counted, not dropped** (#1681): how many times a block let through
/// came again within its minute is told once that minute is over ([`Throttle::repeats_over`]),
/// for one line of the record, so what the record counts is what the sandbox refused, and a
/// chat flooding the app still writes at most a line per block a minute.
#[derive(Debug, Default)]
pub struct Throttle {
    heard: std::collections::HashMap<u32, Vec<Heard>>,
    /// Repeats whose minute ended while their chat's list was read, not told yet.
    over: Vec<Repeated>,
}

/// One block a chat was heard on in the window, and the repeats of it held back since.
#[derive(Debug)]
struct Heard {
    block: Block,
    target: Option<String>,
    at: std::time::Instant,
    repeats: u64,
    last: std::time::Instant,
    /// Whether it was found in what a tool that ran nothing read back ([`read_back`]).
    read: bool,
}

/// **The repeats of one block the throttle held back in its minute** (#1681): chat `chat`'s
/// `block` on `target`, `times` more than the one let through, the last of them at `last`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repeated {
    pub chat: u32,
    pub block: Block,
    pub target: Option<String>,
    pub times: u64,
    pub last: std::time::Instant,
}

impl Throttle {
    /// The most blocks one chat is heard in a [`THROTTLE_WINDOW`].
    pub const PER_CHAT: usize = 10;

    /// Whether chat `chat`'s `block`, naming no target, heard at `now`, is let through;
    /// remembered if it is.
    pub fn lets(&mut self, chat: u32, block: &Block, now: std::time::Instant) -> bool {
        self.lets_on(chat, block, None, now)
    }

    /// Whether chat `chat`'s `block` on `target` (the host or path a grant would name, where
    /// the block names one), heard at `now`, is let through; remembered if it is, and counted
    /// as a repeat if it is the same block again within its minute.
    pub fn lets_on(
        &mut self,
        chat: u32,
        block: &Block,
        target: Option<&str>,
        now: std::time::Instant,
    ) -> bool {
        self.lets_as(chat, block, target, false, now)
    }

    /// [`Self::lets_on`] for a block found in what a tool that ran nothing read back
    /// ([`read_back`], #1884). It is never the same block as one a command was refused: a chat
    /// that reads a refusal back and then runs the command is heard on both, so the command's
    /// own block, the one a person answers, is not held back behind the quiet line of the read.
    pub fn lets_on_read(
        &mut self,
        chat: u32,
        block: &Block,
        target: Option<&str>,
        now: std::time::Instant,
    ) -> bool {
        self.lets_as(chat, block, target, true, now)
    }

    fn lets_as(
        &mut self,
        chat: u32,
        block: &Block,
        target: Option<&str>,
        read: bool,
        now: std::time::Instant,
    ) -> bool {
        let heard = self.heard.entry(chat).or_default();
        let over = &mut self.over;
        heard.retain(|one| {
            let within = now.saturating_duration_since(one.at) < THROTTLE_WINDOW;
            if !within && one.repeats > 0 {
                over.push(one.repeated(chat));
            }
            within
        });
        // A lookup that names no host (one the system's name lookup was refused, #2428) is the
        // same as any lookup of the chat's in the window: one failed lookup, one Notice.
        let a_lookup = (block.operation, block.kind) == (Operation::Lookup, Kind::Host);
        let same = |one: &Heard| {
            one.block == *block
                && one.read == read
                && match (one.target.as_deref(), target) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                    _ => a_lookup,
                }
        };
        if let Some(again) = heard.iter_mut().find(|one| same(one)) {
            again.repeats = again.repeats.saturating_add(1);
            again.last = now;
            return false;
        }
        if heard.len() >= Self::PER_CHAT {
            return false;
        }
        heard.push(Heard {
            block: *block,
            target: target.map(str::to_owned),
            at: now,
            repeats: 0,
            last: now,
            read,
        });
        true
    }

    /// **The repeats held back of each block whose minute is over at `now`**, each told once,
    /// oldest first: what the record keeps a line of its own for (#1681).
    pub fn repeats_over(&mut self, now: std::time::Instant) -> Vec<Repeated> {
        let mut over = std::mem::take(&mut self.over);
        for (chat, heard) in &mut self.heard {
            heard.retain(|one| {
                let within = now.saturating_duration_since(one.at) < THROTTLE_WINDOW;
                if !within && one.repeats > 0 {
                    over.push(one.repeated(*chat));
                }
                within
            });
        }
        self.heard.retain(|_, heard| !heard.is_empty());
        over.sort_by_key(|one| one.last);
        over
    }
}

impl Throttle {
    /// **The repeats held back of chat `chat`'s blocks, told now**: when it goes quiet or ends,
    /// so its count is not left for the next block the project hears. Each block stays held
    /// back for the rest of its minute, and a repeat after this is counted afresh.
    pub fn repeats_of(&mut self, chat: u32) -> Vec<Repeated> {
        let (mut told, kept): (Vec<Repeated>, Vec<Repeated>) = std::mem::take(&mut self.over)
            .into_iter()
            .partition(|one| one.chat == chat);
        self.over = kept;
        for one in self.heard.get_mut(&chat).into_iter().flatten() {
            if one.repeats > 0 {
                told.push(one.repeated(chat));
                one.repeats = 0;
            }
        }
        told.sort_by_key(|one| one.last);
        told
    }

    /// **Every repeat held back, told now**: when the app lets go of the project.
    pub fn every_repeat(&mut self) -> Vec<Repeated> {
        let mut chats: Vec<u32> = self.heard.keys().copied().collect();
        chats.extend(self.over.iter().map(|one| one.chat));
        chats.sort_unstable();
        chats.dedup();
        let mut told: Vec<Repeated> = chats
            .into_iter()
            .flat_map(|chat| self.repeats_of(chat))
            .collect();
        told.sort_by_key(|one| one.last);
        told
    }
}

impl Heard {
    fn repeated(&self, chat: u32) -> Repeated {
        Repeated {
            chat,
            block: self.block,
            target: self.target.clone(),
            times: self.repeats,
            last: self.last,
        }
    }
}

// ---- what the app keeps ----------------------------------------------------------------------

/// **The network record** (#1662): every block the app heard, and every Allow and its removal,
/// per project in purlis's data home for 30 days. What `purlis doctor` counts.
#[path = "sandboxblock_record.rs"]
pub mod record;

/// **`target` as a host a block may name**, in the canonical form a grant names it, or None:
/// what passes the grant's check of a host and is no wildcard (a real refusal names one host).
/// What a Notice offers to allow, and what the network record keeps of a refused connection
/// or lookup (#1663).
pub fn named_host(target: &str) -> Option<String> {
    crate::sandbox::grant::host(target)
        .ok()
        .map(|host| host.to_string())
        .filter(|host| !host.starts_with('*'))
}

#[path = "sandboxblock_denial.rs"]
pub mod denial;

#[path = "sandboxblock_background.rs"]
pub mod background;

pub mod supervised;

#[path = "sandboxblock_drivers.rs"]
pub mod drivers;

#[cfg(test)]
#[path = "sandboxblock_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "sandboxblock_network_tests.rs"]
mod network_tests;

#[cfg(test)]
#[path = "sandboxblock_printed_tests.rs"]
mod printed_tests;

#[cfg(test)]
#[path = "sandboxblock_background_tests.rs"]
mod background_tests;

#[cfg(test)]
#[path = "sandboxblock_drivers_tests.rs"]
mod drivers_tests;
