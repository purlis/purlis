// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What charter keeps **outside** a plane, and the consent that gates opening one.
//!
//! charter's founding rule is that the plane is the state: everything charter knows lives in
//! a `charter.toml` and the directories beside it, committed, and travelling with the clone.
//! This module is an exception the rule cannot express — the app was started from **nowhere**.
//! A double-clicked `.app` has `/` for a working directory, so `plane::resolve` answers
//! `NotFound` and the app has no plane, no recents and no way to be given one. What the opener
//! needs to know belongs to the machine and to no plane on it, so it cannot be kept in one.
//!
//! **It is the second such file, not the first**, and that matters because the rule it follows
//! is already shipped rather than invented here. `charter/report.py:consent_path` has kept
//! consent-to-publish under the human's config home since charter ADR 0003, and its docstring
//! carries the argument for exactly this case: *"Not STATE_DIR: that is per control plane, so
//! a Reporter with several planes would be asked repeatedly until the safeguard became a
//! reflex."* charter-app also already writes its panic log to Tauri's `app_log_dir()`.
//!
//! **Nine things, and nothing else:**
//!
//! - **the planes recently opened**, so the opener has something to offer;
//! - **whether the operator approved each one**, and *what it would do when opened* at the
//!   moment they said yes ([`Contribution`]);
//! - **which planes were open in which windows at the last quit**, so a cold launch restores
//!   the window set;
//! - **how the operator arranged what this file already names** — which of those planes are
//!   pinned, and which workspaces inside them ([`Recent::pinned`],
//!   [`Recent::pinned_workspaces`]), and whether charter has pinned a plane's most active
//!   workspaces for them once ([`Recent::most_active_pinned`], ADR 0054), which is a fact
//!   about those pins and not a sixth thing; and which pool each of those workspaces runs its
//!   chats on, on this machine ([`Recent::chats_run_on`], #2679), which the person ruled on
//!   2026-10-20 is set per machine and per workspace, never committed: a pool is defined on this
//!   machine alone (`runners.json`), so a team's file must never send another person's chats
//!   to it;
//! - **which stream this machine takes charter from** ([`Store::channel`]).
//! - **this device's id** ([`Store::device`], [`device_id`]), argued in ADR 0066, which
//!   amends ADR 0034 for it: a random ULID minted at the first launch that finds none, so an
//!   event, a record or the audit can say which device it happened on without a hostname.
//! - **the ISO week this machine last fetched the weekly manifest in** ([`Store::weekly`],
//!   OB-17, ADR 0083 as amended for it). It is the updater's state, as the channel is, and it
//!   is here for the channel's reasons. It is not an identifier: every machine that checked in
//!   a week holds the same value, and it never leaves the device.
//! - **the browser limits the person set on this machine** ([`Store::browsers`], #2023): the most
//!   browsers a project's chats hold, the most this device runs, and how long a chat's browser
//!   may go unused. The person ruled these keys and their home on 2026-10-18: the device's budget
//!   follows this machine's memory, so it belongs to no project, and a team's committed file must
//!   never set another person's machine. Each is absent until set, and absent is purlis's default.
//! - **the flags turned on on this machine** ([`Store::flags`], #2764): a feature that lands in
//!   slices stays behind one until it is whole, and is then removed with its flag. Today there
//!   is one, `skill-studio` (spec #2762). A flag is how one person tries what is unfinished, so
//!   it belongs to no project, and a committed file must never turn it on for a team. It is
//!   neither the channel nor the build: both channels' binaries are the same (ADR 0092), and a
//!   flag is off on either until it is turned on here. Each is absent until turned on.
//!
//! **The fourth was three until ADR 0040, and the fifth and sixth are newer still; the count
//! is load-bearing.** ADR 0040 and ADR 0066 are the amendments: ADR 0066 argues the sixth (a
//! device needs one id that every event and record names it by, and no project can hold it,
//! because one machine holds many projects). For the fourth, the operator ruled on 2026-09-22 that a
//! pin is how one operator likes their window rather than a fact about the plane, so it
//! cannot be committed to `charter.toml`, where it would arrive with every clone and put
//! somebody else's workspace first on a strip its operator never arranged. The whole value of
//! "and nothing else" is that each addition had to be argued for; if it can be appended to,
//! it is not a limit.
//!
//! **The fifth is the first entry here that is not about planes at all, so it is argued
//! separately.** The update channel is a fact about *this installation of the app* — which of
//! two release streams it takes its next version from. It is not a copy of anything a plane
//! answers, so the rule the other four are held to (never a second answer to a question the
//! plane already answers) has nothing to catch; and there is no plane it could belong to,
//! because one binary serves every plane on the machine and cannot be on two channels at
//! once. What was weighed against putting it here was a second small file beside this one,
//! and that lost on two counts: it would duplicate this module's hardened read — the
//! `open_no_link`, the `fstat` of the descriptor, the size bound, the `0700` directory — in a
//! second place that has to get containment right a second time, and it would need a lock of
//! its own, because the app and a `charter` in a terminal both write it. [`update`]'s `flock`
//! already makes this file's read-modify-write one act across processes, which is exactly the
//! problem a second file would reintroduce.
//!
//! **Never plane content.** A chat, a memory, a workspace, a persona, a profile: all of those
//! belong to a plane and stay in it. Each plane's own `.charter/app/reopen.json` still
//! restores *its* chats ([`crate::reopen`]) and this module never writes it — the split is
//! "which planes were open" here, "what was open inside one" there. What this module does read
//! from that record is a **fingerprint**, because it is an execution input; see
//! [`Contribution`].
//!
//! **A pinned workspace's name does not breach that, and the distinction is the one the rule
//! was making.** What is forbidden is a *copy* of an answer the plane already gives, because a
//! copy is a second answer that nothing invalidates when the plane changes underneath it. A
//! pin is not a copy: the plane answers "which workspaces exist", and the pin answers "which
//! of them this operator wants first", which the plane does not answer and must not. The name
//! is a **reference** into the plane — the same kind of thing as the plane paths this file has
//! always held, and held to the same rule, which is that a reference is checked when it is
//! read and one that no longer resolves is dropped with a reason. **A chat pin is not here**,
//! for the reason stated above in as many words: a chat is numbered per plane, and its number
//! means nothing outside the plane that issued it. That one lives in the plane's own
//! `.charter/app/reopen.json`.
//!
//! # Where it lives
//!
//! `$CHARTER_CONFIG_HOME`, else `$XDG_CONFIG_HOME`, else `~/.config` — then `charter/`, and
//! [`FILE`] inside it. That is `report.py:consent_path`'s ladder, rung for rung, because a
//! machine should have **one** `charter/` in **one** place and charter already ships one at
//! that address.
//!
//! **This is deliberately not the per-platform application-data directory**, and on macOS the
//! difference is real: `dirs::data_dir()` is `~/Library/Application Support`, which is the
//! platform convention and is where an earlier draft of this module put the store. The cost of
//! taking it is that `charter report`'s consent would sit in `~/.config/charter/` and this
//! store in `~/Library/Application Support/charter/` — two `charter/` directories on one
//! machine, holding two records of the same kind of thing (what the operator has agreed to).
//! One address is worth more than the platform convention for a tool whose operators already
//! live in `~/.config`, and it is the address the shipped half is already at.
//!
//! **`$CHARTER_CONFIG_HOME` is honoured, and it exists for a measured reason** that is not
//! "somebody wanted an override": `gh` keeps its own auth under `$XDG_CONFIG_HOME`, so
//! redirecting *that* variable to isolate charter — in a test, a sandbox, a second account —
//! silently logs `gh` out, which turns a publish into the no-`gh` fallback path. The variable
//! is the way to isolate charter without that side effect, and a Rust charter that ignored it
//! would isolate half the product.
//!
//! The residual, said plainly: a process that can set `$CHARTER_CONFIG_HOME` in charter's
//! environment can point it at a store full of approvals the operator never gave. It is not a
//! way in — the same process can write the real store, which is the same account's file — and
//! the shipped consent file has carried exactly this shape since ADR 0003. It is written down
//! so nobody has to rediscover it.
//!
//! # Windows refuses rather than degrades
//!
//! Every `chmod` call site in this repo is `#[cfg(unix)]` with nothing off it, and on Windows
//! `chmod 0o755` leaves a file at `0o666` (measured, charter-app#98). The `0600`/`0700` this
//! store depends on therefore has no Windows expression today, and ADR 0031 (charter) settles
//! what to do about that: **a guard that cannot be expressed refuses rather than degrades.**
//! So on any platform that is not unix every entry point here returns
//! [`io::ErrorKind::Unsupported`] and charter keeps no machine-level state at all. That is a
//! known quantity; a world-readable list of the operator's projects, and a trust record any
//! account on the machine can edit, is not.
//!
//! # Everything read back is attacker-influenced
//!
//! The file is `0600` in a `0700` directory, so on an honest machine only the operator writes
//! it. That is a reason to treat what comes back as untrusted, not a reason to trust it: the
//! store is the one charter file that is **not** in a plane's git history, so nothing else
//! vouches for it, and a plane path it names is a path charter is about to open a window on.
//! So on read every stored path is held to being absolute, free of `..` and free of a NUL,
//! and an entry that fails is **dropped with a reason** ([`Dropped`]) rather than raised. An
//! opener that shows one fewer row and says why is usable; an error dialog at launch, before
//! there is a window, is not.
//!
//! Whether a remembered path is still *there*, and still a plane, is a different question and
//! is deliberately **not** asked on the read path — see [`still_a_plane`].

use std::collections::BTreeMap;
use std::io;
use std::path::{Component, Path, PathBuf};

/// charter's own directory inside the config home — the one the Python `charter report` kept
/// its publish consent in. The Rust `charter report` keeps none (ADR 0059).
pub const DIR: &str = "charter";

/// The store, inside [`DIR`].
pub const FILE: &str = "machine.json";

/// The one version of this file charter writes and reads. Any other version reads as an empty
/// store: there is no migration that would be honest about an approval recorded under rules
/// this charter does not know.
pub const VERSION: u32 = 1;

/// The most this file may be. It is a bounded list of short entries, it is read whole, and it
/// is read before there is a window — so a planted giant here is a launch that never
/// finishes, exactly as it is for [`crate::reopen`]'s record.
pub const MAX_BYTES: u64 = 1 << 20;

/// How many **unpinned** planes are remembered. The list is bounded because the file is read
/// at every launch and nothing else prunes it.
///
/// **Falling off the end forgets the approval too**, because the approval is a field of the
/// entry (see [`Recent`]). That direction is the safe one: the plane is asked about again the
/// next time it is opened.
///
/// **A pinned plane does not count against it** (ADR 0040). Losing a pinned project to
/// the sixty-fifth plane opened would make pinning meaningless on exactly the machine that
/// most needs it — the one with a lot of planes — and pinning is the only thing an operator
/// can say to mean "not this one". [`MOST_PINNED`] is what keeps the file bounded all the
/// same.
pub const MOST_RECENTS: usize = 64;

/// How many planes may be pinned, on top of [`MOST_RECENTS`].
///
/// A pin exempts an entry from the recents bound, so without a bound of its own the file would
/// have none — and it is read whole at a cold launch, before there is a window. The operator
/// can only pin a plane they have opened, so reaching this by hand takes thirty-two deliberate
/// acts; a file that claims more was not written by charter.
pub const MOST_PINNED: usize = 32;

/// How many workspaces may be pinned inside one plane.
///
/// The same reason as [`MOST_PINNED`], one scope down: the strip a pin orders holds ten
/// workspaces at ADR 0026's limits, and a plane claiming a thousand pinned workspaces is a
/// file charter did not write.
pub const MOST_PINNED_WORKSPACES: usize = 32;

/// How many workspaces of one plane may run their chats on a pool (#2679). The file is read
/// whole at a cold launch, so this is bounded for the reason [`MOST_PINNED_WORKSPACES`] is.
pub const MOST_CHATS_RUN_ON: usize = 64;

/// How many workspaces [`Store::pin_the_most_active`] pins in a plane charter has not pinned
/// any in before (ADR 0054): an operator with many workspaces works in about three.
pub const FIRST_OPEN_PINS: usize = 3;

/// The two bounds, counted apart, in one place.
///
/// **Three callers and one rule.** The size of this file is decided three times — when an
/// open puts a plane at the front ([`Store::trim`]), when the file is read back ([`load`])
/// and when it is written out ([`OnDisk::from`]) — and a pinned entry has to be exempt from
/// [`MOST_RECENTS`] in all three or a pin quietly stops working in whichever one was missed.
/// Three implementations of one rule is the drift this module's own docstring is about.
#[derive(Default)]
struct Room {
    unpinned: usize,
    pinned: usize,
}

impl Room {
    /// Whether one more entry of this kind fits, counting it in if it does.
    fn fits(&mut self, pinned: bool) -> bool {
        let (counted, most) = if pinned {
            (&mut self.pinned, MOST_PINNED)
        } else {
            (&mut self.unpinned, MOST_RECENTS)
        };
        *counted += 1;
        *counted <= most
    }

    /// Why one did not, in the words a dropped row carries.
    fn why(pinned: bool) -> String {
        if pinned {
            format!("is past the {MOST_PINNED} projects purlis pins")
        } else {
            format!("is past the {MOST_RECENTS} planes purlis remembers")
        }
    }
}

/// The most a workspace name may be, in bytes.
///
/// A workspace is a directory beside `charter.toml`, so this is the longest name the file
/// systems charter runs on will give one. It is a bound on what is read back, not a rule about
/// what a workspace may be called.
const LONGEST_WORKSPACE_NAME: usize = 255;

/// The environment variable that moves charter's config home, and **only** charter's.
///
/// `report.py:consent_path` reads it first for a reason worth keeping in one piece: `gh` keeps
/// its own auth under `$XDG_CONFIG_HOME`, so isolating charter by redirecting that variable
/// logs `gh` out and silently turns a publish into the no-`gh` fallback path.
pub const HOME_VAR: &str = "PURLIS_CONFIG_HOME";

/// The human's config home: `$CHARTER_CONFIG_HOME`, else `$XDG_CONFIG_HOME`, else
/// `~/.config`. `None` when there is no home to put one in.
///
/// `report.py:consent_path`'s ladder, rung for rung, so that charter has one config home per
/// machine rather than one per implementation. An empty variable is treated as unset, which
/// is what an exported-but-blank `XDG_CONFIG_HOME` means everywhere else.
///
/// `dirs::home_dir` for the last rung rather than `$HOME` read by hand: it is the mature,
/// standard answer, it is already in this workspace's lockfile (Tauri depends on it), and it
/// knows the cases a hand-rolled `$HOME` does not.
pub fn config_root() -> Option<PathBuf> {
    let found = config_root_unheld();
    // The store is the OTHER thing a run reaches past its own fixture into, and it is not in
    // a plane: a launcher that pinned `$CHARTER_ROOT` and forgot `$CHARTER_CONFIG_HOME` wrote
    // its throwaway projects and their trust into the operator's `~/.config/charter`, which
    // is where charter decides what it may open without asking. `wdio.bench.conf.ts` did
    // exactly that. So a fenced build is held here too (charter-app#129).
    if let Some(root) = &found {
        crate::fence::hold(crate::fence::Act::Store, root);
    }
    found
}

/// [`config_root`]'s ladder as this process's environment answers it, with no fence asked:
/// the one lookup every reader of the config home shares, so `$PURLIS_CONFIG_HOME`, its
/// `CHARTER_` fallback and `$XDG_CONFIG_HOME` are read the same way everywhere. A caller that
/// reads a store through it holds the fence itself, or, like
/// [`crate::names::moved_by_rename_local`], asks the fence and treats outside as nothing.
pub(crate) fn config_root_unheld() -> Option<PathBuf> {
    rooted(
        crate::envvar::var_os(HOME_VAR),
        std::env::var_os("XDG_CONFIG_HOME"),
        dirs::home_dir(),
    )
}

/// [`config_root`], for `charter doctor`'s plugin rows, which read the copy `charter plugin
/// install` keeps under it: fenced once there is a directory there to read, as
/// [`config_root_if_there`] is, and otherwise the answer with nothing asked — a directory that
/// does not exist holds no copy, and a doctor run in a fixture must still be able to say so.
pub fn config_root_to_read_the_plugin_copy() -> Option<PathBuf> {
    let found = config_root_unheld()?;
    if found.is_dir() {
        crate::fence::hold(crate::fence::Act::Store, &found);
    }
    Some(found)
}

/// [`config_root`], for a reader that only reads: `None` as well when there is no directory
/// there yet, since a store that does not exist holds nothing to read.
///
/// **For `charter statusline`'s hot path** (charter-app#340), which reads the extension record
/// for badges every turn. A fenced test build refuses a store outside its fence, and a store
/// that is not there yet cannot be shown to be inside it — its path does not resolve — so the
/// fence is asked only about a store that exists. Nothing is read from one that does not.
pub fn config_root_if_there() -> Option<PathBuf> {
    let found = config_root_unheld()?;
    if !found.is_dir() {
        return None;
    }
    crate::fence::hold(crate::fence::Act::Store, &found);
    Some(found)
}

/// [`config_root_if_there`], with the environment asked through `env` rather than read off the
/// process — for a hook, which is handed its environment as a lookup and is tested that way.
pub fn config_root_in(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let found = rooted(
        env(HOME_VAR).map(Into::into),
        env("XDG_CONFIG_HOME").map(Into::into),
        dirs::home_dir(),
    )?;
    if !found.is_dir() {
        return None;
    }
    crate::fence::hold(crate::fence::Act::Store, &found);
    Some(found)
}

/// The plane as a machine store may have remembered it: as spelled, and as resolved.
///
/// The app remembers a plane under its resolved path; a caller may hold it as typed. Both are
/// asked, and nothing else — a store is never searched for a path that merely resembles one.
pub fn spellings_of(root: &Path) -> Vec<PathBuf> {
    let mut planes = vec![root.to_path_buf()];
    if let Ok(real) = std::fs::canonicalize(root)
        && real != root
    {
        planes.push(real);
    }
    planes
}

/// [`config_root`]'s ladder for a process whose home is `home` and whose environment is
/// otherwise this one's: where a cluster runner's host will keep its config once its pod sets
/// `HOME` to `home`, as its pod's first step reads it (#2057).
pub fn config_root_under(home: &Path) -> PathBuf {
    rooted(
        crate::envvar::var_os(HOME_VAR),
        std::env::var_os("XDG_CONFIG_HOME"),
        Some(home.to_path_buf()),
    )
    .unwrap_or_else(|| home.join(".config"))
}

/// [`config_root`]'s ladder, with the three answers handed in.
///
/// Split out because the environment is the one thing this module's tests cannot drive:
/// `std::env::set_var` is `unsafe` in this edition and the workspace is
/// `unsafe_code = "forbid"`, and it is process-global besides, so a test that set it would
/// race every other test in the same binary. The ladder is the part worth testing, so the
/// ladder is what is testable.
pub(crate) fn rooted(
    charter_home: Option<std::ffi::OsString>,
    xdg: Option<std::ffi::OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    for set in [charter_home, xdg] {
        // An exported-but-blank variable is unset, which is what it means everywhere else.
        if let Some(set) = set.filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(set));
        }
    }
    home.map(|home| home.join(".config"))
}

/// charter's directory inside `config_root`: `purlis/` once rename-local has moved it there (or
/// it is the one there), else `charter/` ([`crate::names::Name::folder_at`], V93e).
///
/// **The one source of truth for the config home's folder.** Everything under it — the store,
/// the kill switch, the window's prefs, the session host's folder and rename-local's own record
/// ([`crate::names::moved_by_rename_local`]) — is joined to this, so all of it follows the move
/// together, and nothing reads one spelling while writing the other.
pub fn dir(config_root: &Path) -> PathBuf {
    crate::names::CONFIG_HOME.folder_at(config_root)
}

/// Every spelling of charter's directory inside `config_root`, the purlis one first: for a
/// sandbox, which must hold each one, there or not — a folder a chat could make under the
/// purlis name would be read as the config home the moment it is there (V93e).
pub fn dirs_spelled(config_root: &Path) -> Vec<PathBuf> {
    crate::names::CONFIG_HOME
        .spellings()
        .map(|name| config_root.join(name))
        .collect()
}

/// A runner's one peer, the desktop it is paired with, inside [`dir`]
/// (`crate::runnerlink`, ADR 0078 §8).
pub const PEERS_FILE: &str = "peers.json";

/// The runners this desktop pinned, inside [`dir`] (`crate::runners`, ADR 0078 §8), and the
/// lock its one read-modify-write takes.
pub const RUNNERS_FILE: &str = "runners.json";
pub const RUNNERS_LOCK: &str = "runners.json.lock";

/// The session host's folder's old name inside [`dir`] (`names::DAEMON_DIR`).
pub const DAEMON_DIR: &str = "charterd";

/// Where the session host keeps one credential file per human client scope
/// (`<config>/purlisd/<scope>`, ADR 0068 §5, plane-format.md): a directory a chat's sandbox
/// denies reading and writing (ADR 0067 §5, class 3; FD-27). Its socket is under the runtime
/// directory ([`crate::hostsocket`]).
///
/// `purlisd/` once rename-local has moved it, else `charterd/` (V93a, RN-5).
pub fn charterd(config_root: &Path) -> PathBuf {
    crate::names::DAEMON_DIR.folder_at(&dir(config_root))
}

/// The store's own path inside `config_root`.
pub fn file(config_root: &Path) -> PathBuf {
    dir(config_root).join(FILE)
}

/// Whether charter keeps machine-level state on this platform at all.
///
/// See the module docstring: on Windows the `0600`/`0700` this store depends on has no
/// expression, and ADR 0031 says such a guard refuses.
#[cfg(unix)]
pub(crate) fn supported() -> io::Result<()> {
    Ok(())
}

/// Off unix only. On unix, where the mutation run happens, `supported` IS `Ok(())`, so the
/// mutant that makes it so is excluded in `.cargo/mutants.toml`: the refusal below is code no
/// unix build compiles, and the unix twin above already is the mutant.
#[cfg(not(unix))]
pub(crate) fn supported() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "purlis keeps no machine-level store on this platform: the 0600 on the file and the \
         0700 on its directory have no expression here (charter-app#98), and a guard that \
         cannot be expressed refuses rather than degrades (ADR 0031)",
    ))
}

/// What opening `plane` would do to this machine — everything the approval is *of*.
///
/// **Three sources.** The first two are below; the third is each persona's tool grant
/// ([`grants`](Self::grants)), because the persona tool gate lets a granted program run
/// without a prompt and a grant is committed text that arrives with a pull.
///
/// *The settings that travel.* `layer::WORKSPACE_KEYS` is `["enabledPlugins", "env"]`: those
/// two keys travel out of a plane's own committed `.claude/settings.json` into the
/// `.claude/settings.json` a harness reads. So opening a stranger's plane lets its author
/// choose the plugins that run and the environment every chat is started under — `PATH`,
/// `NODE_OPTIONS`, a base URL a harness talks to. The two existing limits on that travel are
/// not weakened here and are why this covers those two keys and not three: `permissions`
/// travels only as `ask`/`deny` and **never** as `allow` (`layer::RESTRICTIVE`), so it cannot
/// make anything run that would not have run anyway; and a harness profile is machine-local
/// (ADR 0022), so a plane cannot bring a command line with it.
///
/// *The record that starts programs.* `app/src-tauri/src/lib.rs`'s `setup` calls
/// `chats.put_back(&record, root, STARTING)`, and its own comment says **"Before a single
/// session is started, because `put_back` below starts them."** `Chats::start_recorded` then
/// takes a chat with no profile straight to `Chats::start`, whose doc says what runs is
/// *"decided from the record alone"*. So `.charter/app/reopen.json` is an **execution
/// input**, read before there is a window and with nothing to click. `reopen`'s own module
/// docstring already says so: *"this file is, for whoever can write it, a way to have a
/// command run at every later launch."*
///
/// **`.charter/` is gitignored, and that is not the reassurance it sounds like.** It keeps the
/// record out of a `git clone`; the opener opens a **directory**, and directories arrive by
/// zip, shared folder, USB and download.
///
/// So [`starts`](Self::starts) fingerprints every chat the record would launch by its own
/// program and arguments, and [`profiles`](Self::profiles) the ones that name a harness
/// profile instead. They are separate because only one of them is a grant — see
/// [`Consent::must_ask`].
///
/// **Values are recorded, not only names.** An `env` whose `PATH` gains a directory is a
/// different grant from the one approved, and a record of names alone could not see it. What
/// is recorded comes from files inside the plane, so this moves nothing into the machine store
/// that was not already readable in the plane.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contribution {
    /// Each plugin the plane enables, and what it enables it as.
    pub plugins: BTreeMap<String, String>,
    /// Each environment variable the plane sets, and its value.
    pub env: BTreeMap<String, String>,
    /// One key per recorded chat that names its **own** program: the JSON of its program, its
    /// arguments and its working directory. The value is always empty — the whole launch is
    /// the identity, so two chats running the same program in different directories are two
    /// entries and neither can hide the other.
    pub starts: BTreeMap<String, String>,
    /// One key per recorded chat that names a harness **profile** instead: the JSON of the
    /// profile's name and the working directory.
    pub profiles: BTreeMap<String, String>,
    /// One key per persona that grants anything: the tools the persona tool gate lets it run
    /// without a prompt, and for each one it ships in its own `bin/`, the digest of that file
    /// ([`crate::personagrant::grant`], as [`crate::personagrant::grant_text`] writes it).
    ///
    /// **A grant, and the gate answers from this record rather than from the file.** A
    /// persona's `tools:` is committed text, so it arrives with a pull like everything else in
    /// the plane; what [`crate::personagate::decide`] waves through is only what the operator
    /// approved here, and a grant that differs from it meets the harness's own prompt until
    /// the plane is approved again.
    pub grants: BTreeMap<String, String>,
    /// One key per font file the plane's theme folders bring, at its root and in its
    /// workspaces: its path in the plane, to the sha-256 of its bytes and its size
    /// ([`crate::theme::levels::plane_fonts`], under one budget for the whole plane).
    ///
    /// **A grant, by the operator's ruling (2026-10-18; ADR 0098 as amended).** A theme's
    /// colours and folders need no approval, and its font files do: a face from the project or
    /// a workspace is drawn only when this record holds its file with that digest, and a face
    /// that is new or changed asks again.
    pub fonts: BTreeMap<String, String>,
}

impl Contribution {
    /// What opening `plane` would do, right now — for a caller that will not act on the
    /// record.
    ///
    /// Use [`Self::read`] wherever the same act both *shows* the operator a contribution and
    /// *starts* what it describes: this throws the record away, so such a caller has to read
    /// it a second time, and a write landing between the two reads is executed without having
    /// been shown (charter-app#123).
    pub fn of(plane: &Path) -> Self {
        Self::read(plane).contributes
    }

    /// [`Self::of`], keeping in `measured` what listing the plane's font files hashed, for the
    /// theme read made with it (#2696, [`crate::theme::Folders::of_measured`]): a window's
    /// creation checks the project it opens on is trusted and draws its theme with one hashing
    /// of its fonts.
    pub fn of_measured(plane: &Path, measured: &crate::theme::Measured) -> Self {
        Self::read_measured(plane, measured).contributes
    }

    /// What opening `plane` would do, right now, **and the record that says so**.
    ///
    /// Both halves fail **closed and quiet**: settings charter cannot read contribute nothing,
    /// and a reopen record charter would refuse contributes nothing — because a record the app
    /// refuses is one it starts no chats from either (`lib.rs` logs the refusal and puts back
    /// `Record::default()`). Neither is credited with whatever it might have said.
    ///
    /// **The record comes back because it is an execution input** (charter-app#123). What the
    /// dialog drew is read out of `.charter/app/reopen.json`; what `put_back` starts used to
    /// be read out of it again, a moment later. Two reads of a file that decides what runs is
    /// a window, however short, in which a write is executed without having been shown. So the
    /// bytes this read got are handed back, and `Planes::approve_and_open` passes *those* to
    /// `put_back` — the window closes by construction rather than by being narrowed.
    ///
    /// **Bounded, and the bound is stated rather than implied.** ADR 0035 says of its own
    /// fingerprint that it is *"not a defence against an agent that set out to forge the
    /// fingerprint"*, and that sentence is unchanged by this. What closes is the window
    /// between a human clicking a button and the app reading a file. Nothing here guards an
    /// approved plane, and nothing here holds the file.
    pub fn read(plane: &Path) -> Reading {
        Self::read_measured(plane, &crate::theme::Measured::default())
    }

    /// [`Self::read`], keeping what the font files' listing hashed in `measured`.
    fn read_measured(plane: &Path, measured: &crate::theme::Measured) -> Reading {
        let mut out = Self::default();
        if let Some(settings) = crate::layer::plane_settings(plane, crate::layer::SETTINGS) {
            out.plugins = settings
                .get("enabledPlugins")
                .map(plugins_of)
                .unwrap_or_default();
            out.env = settings
                .get("env")
                .and_then(serde_json::Value::as_object)
                .map(|map| map.iter().map(|(k, v)| (k.clone(), word(v))).collect())
                .unwrap_or_default();
        }
        for persona in crate::personagrant::list_personas(plane) {
            let grant = crate::personagrant::grant(plane, &persona);
            if !grant.is_empty() {
                out.grants
                    .insert(persona, crate::personagrant::grant_text(&grant));
            }
        }
        out.fonts = crate::theme::levels::plane_fonts(plane, measured);
        let record = out.with_record(plane);
        Reading {
            contributes: out,
            record,
        }
    }

    /// **The half of what opening `plane` would do that charter itself writes**: the chats its
    /// reopen record would start ([`Self::starts`], [`Self::profiles`]), and nothing else. What
    /// [`Store::vouch`] refreshes, read without the settings, the personas or the font files,
    /// which a vouch keeps as they were approved.
    pub fn of_record(plane: &Path) -> Self {
        let mut out = Self::default();
        // A record charter would refuse contributes nothing, as in [`Self::read`].
        let _ = out.with_record(plane);
        out
    }

    /// The chats `plane`'s reopen record would start, added to this; and the record as read.
    fn with_record(&mut self, plane: &Path) -> Result<crate::reopen::Record, io::Error> {
        let out = self;
        // Through `reopen`, never by reading the file here: that read is already gated on the
        // exact path it opens, bounded, and refuses a FIFO — and a second reader of the same
        // file would be a second set of rules about it.
        let record = crate::reopen::read_or_refusal(plane);
        if let Ok(record) = &record {
            for chat in &record.chats {
                let cwd = chat
                    .cwd
                    .as_ref()
                    .map(|cwd| cwd.display().to_string())
                    .unwrap_or_default();
                match &chat.profile {
                    // The profile is looked up again at every launch out of machine-local
                    // `charter.local.toml` (ADR 0022, `Chats::start_recorded`), so the record
                    // chooses WHICH of the operator's own profiles runs and never what it
                    // runs. Recorded so a change can be reported; not a grant.
                    Some(name) => out.profiles.insert(
                        serde_json::json!({ "profile": name, "cwd": cwd }).to_string(),
                        String::new(),
                    ),
                    None => out.starts.insert(
                        serde_json::json!({
                            "program": chat.program,
                            "args": chat.args,
                            "cwd": cwd,
                        })
                        .to_string(),
                        String::new(),
                    ),
                };
            }
        }
        record
    }

    /// Every way `now` differs from what this recorded, in a stable order.
    pub fn against(&self, now: &Self) -> Vec<Change> {
        let mut out = Vec::new();
        diff(
            &self.plugins,
            &now.plugins,
            Change::PluginAdded,
            Change::PluginRemoved,
            Change::PluginChanged,
            &mut out,
        );
        diff(
            &self.env,
            &now.env,
            Change::EnvAdded,
            Change::EnvRemoved,
            Change::EnvChanged,
            &mut out,
        );
        // The whole launch is the key and the value is always empty, so the "changed" arm
        // cannot fire. It is spelled as the ASKING variant anyway, so that a later version
        // which does put a value there fails closed rather than silently stops asking.
        diff(
            &self.starts,
            &now.starts,
            Change::StartsAdded,
            Change::StartsRemoved,
            Change::StartsAdded,
            &mut out,
        );
        diff(
            &self.profiles,
            &now.profiles,
            Change::ProfileAdded,
            Change::ProfileRemoved,
            Change::ProfileAdded,
            &mut out,
        );
        diff(
            &self.grants,
            &now.grants,
            Change::GrantAdded,
            Change::GrantRemoved,
            Change::GrantChanged,
            &mut out,
        );
        diff(
            &self.fonts,
            &now.fonts,
            Change::FontAdded,
            Change::FontRemoved,
            Change::FontChanged,
            &mut out,
        );
        out
    }
}

/// One read of a plane: what opening it would contribute, and the reopen record those
/// contributions were taken from.
///
/// **The two travel together because they are one answer** (charter-app#123). The record is an
/// execution input — putting it back starts the programs it names — and the contribution is
/// what the operator is shown about it. A caller that has the first and re-reads the second is
/// showing one file and running another, with however many milliseconds between them for a
/// write to land. Handing both back from one read makes that impossible to write by accident:
/// `Planes::approve_and_open` passes this record straight to `put_back`.
///
/// The record is kept as the `Result` the read gave, not flattened to an `Option`. A record
/// charter **refused** and a plane with **no record at all** are different things to tell an
/// operator — `Held::reopen` prints the refusal and says nothing is reopened until it is
/// repaired — and a reader that lost the reason would have to go back to the disk to find it,
/// which is the second read this whole type exists to remove.
#[derive(Debug)]
pub struct Reading {
    /// What opening the plane would put in force.
    pub contributes: Contribution,
    /// The record as it was read, or why charter would not read it.
    pub record: Result<crate::reopen::Record, io::Error>,
}

/// `enabledPlugins` as a name-to-value map, whatever shape the key is in.
///
/// An object is the shape a settings file uses (`{"name@market": true}`), and an array of
/// names is the other one anyone writes by hand. Anything else — a string, a number, `null` —
/// is keyed by its own rendering, so a plane that changes it is still **noticed** rather than
/// quietly read as contributing nothing. A reader that assumed a shape would be the crash a
/// launch cannot have.
fn plugins_of(value: &serde_json::Value) -> BTreeMap<String, String> {
    match value {
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), word(v))).collect(),
        serde_json::Value::Array(items) => items.iter().map(|v| (word(v), String::new())).collect(),
        other => [(word(other), String::new())].into_iter().collect(),
    }
}

/// A JSON value as one comparable word: a string as itself, anything else as its compact
/// rendering.
fn word(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn diff(
    was: &BTreeMap<String, String>,
    now: &BTreeMap<String, String>,
    added: fn(String) -> Change,
    removed: fn(String) -> Change,
    changed: fn(String) -> Change,
    out: &mut Vec<Change>,
) {
    for (name, value) in now {
        match was.get(name) {
            None => out.push(added(name.clone())),
            Some(before) if before != value => out.push(changed(name.clone())),
            Some(_) => {}
        }
    }
    for name in was.keys() {
        if !now.contains_key(name) {
            out.push(removed(name.clone()));
        }
    }
}

/// One way a plane's contribution differs from the one that was approved.
///
/// **The name only, never the value.** The caller holds both contributions and can show
/// whatever it needs; a variant carrying a value would put the contents of every plane's
/// `env` into every string a refusal, a log line or a report is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    PluginAdded(String),
    PluginRemoved(String),
    PluginChanged(String),
    EnvAdded(String),
    EnvRemoved(String),
    EnvChanged(String),
    /// A chat the reopen record would start on a program of its own choosing.
    StartsAdded(String),
    StartsRemoved(String),
    /// A chat the reopen record would start on one of this machine's harness profiles.
    ProfileAdded(String),
    ProfileRemoved(String),
    /// A persona that now grants tools, and one whose grant is not the approved one. Any
    /// difference asks, a narrower grant included: the value is one fingerprint, and reading
    /// which way it moved would be a second grammar of it.
    GrantAdded(String),
    GrantChanged(String),
    GrantRemoved(String),
    /// A font file a theme folder of the plane brings that is new, or whose bytes are not the
    /// approved ones; and one that is gone.
    FontAdded(String),
    FontChanged(String),
    FontRemoved(String),
}

impl Change {
    /// Whether this change is about a font file a theme folder of the plane brings.
    pub fn is_a_font(&self) -> bool {
        matches!(
            self,
            Self::FontAdded(_) | Self::FontChanged(_) | Self::FontRemoved(_)
        )
    }

    /// Whether this change can make something run that the approval did not cover.
    ///
    /// An addition or a changed value can; a removal cannot. That asymmetry is the same one
    /// `layer::RESTRICTIVE` already makes about `ask`/`deny`, and it is what [`Consent`]
    /// turns into "ask again" or "just say so".
    ///
    /// **A profile is the one addition that is not a grant**, and the reason is measured
    /// rather than assumed: `Chats::start_recorded` looks the profile up again in
    /// machine-local `charter.local.toml` at every launch — *"never taken from the record"* —
    /// and `profiles::Source` decides whether `profiletrust::approval_needed` shows its
    /// command line first. A profile that is gone skips the chat by name. So the record
    /// chooses **which** of the operator's own, already-gated profiles runs, and never what
    /// it runs. Asking again here would be asking a second time about a command line
    /// `profiletrust` is about to show.
    pub fn is_a_grant(&self) -> bool {
        match self {
            Self::PluginAdded(_)
            | Self::PluginChanged(_)
            | Self::EnvAdded(_)
            | Self::EnvChanged(_)
            | Self::StartsAdded(_)
            | Self::GrantAdded(_)
            | Self::GrantChanged(_)
            | Self::FontAdded(_)
            | Self::FontChanged(_) => true,
            Self::PluginRemoved(_)
            | Self::EnvRemoved(_)
            | Self::StartsRemoved(_)
            | Self::ProfileAdded(_)
            | Self::ProfileRemoved(_)
            | Self::GrantRemoved(_)
            | Self::FontRemoved(_) => false,
        }
    }

    /// The name — or, for a launch, the whole recorded command line — this change is about.
    pub fn name(&self) -> &str {
        match self {
            Self::PluginAdded(name)
            | Self::PluginRemoved(name)
            | Self::PluginChanged(name)
            | Self::EnvAdded(name)
            | Self::EnvRemoved(name)
            | Self::EnvChanged(name)
            | Self::StartsAdded(name)
            | Self::StartsRemoved(name)
            | Self::ProfileAdded(name)
            | Self::ProfileRemoved(name)
            | Self::GrantAdded(name)
            | Self::GrantChanged(name)
            | Self::GrantRemoved(name)
            | Self::FontAdded(name)
            | Self::FontChanged(name)
            | Self::FontRemoved(name) => name,
        }
    }
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (what, how) = match self {
            Self::PluginAdded(_) => ("plugin", "is new"),
            Self::PluginRemoved(_) => ("plugin", "is gone"),
            Self::PluginChanged(_) => ("plugin", "is enabled differently"),
            Self::EnvAdded(_) => ("environment variable", "is new"),
            Self::EnvRemoved(_) => ("environment variable", "is gone"),
            Self::EnvChanged(_) => ("environment variable", "has a different value"),
            Self::StartsAdded(_) => ("chat this plane would start", "is new"),
            Self::StartsRemoved(_) => ("chat this plane would start", "is gone"),
            Self::ProfileAdded(_) => ("chat on one of your harness profiles", "is new"),
            Self::ProfileRemoved(_) => ("chat on one of your harness profiles", "is gone"),
            Self::GrantAdded(_) => ("tool grant of persona", "is new"),
            Self::GrantChanged(_) => ("tool grant of persona", "has changed"),
            Self::GrantRemoved(_) => ("tool grant of persona", "is gone"),
            Self::FontAdded(_) => ("font file", "is new"),
            Self::FontChanged(_) => ("font file", "has changed"),
            Self::FontRemoved(_) => ("font file", "is gone"),
        };
        write!(f, "the {what} {} {how}", self.name())
    }
}

/// Whether a plane may be opened without asking, and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consent {
    /// Nothing has recorded this plane. Ask.
    New,
    /// It was approved, and it contributes exactly what was approved.
    Unchanged,
    /// It was approved, and it would now do **more** — a plugin, an environment variable, a
    /// different value for one it already had, a chat it would start on a program of its
    /// own, or a persona tool grant that is new or not the one approved. Ask again, and say
    /// what is new.
    Grew(Vec<Change>),
    /// It was approved, and what changed cannot make anything more run. Say so; do not ask.
    Noted(Vec<Change>),
}

impl Consent {
    /// Whether the operator has to be asked before this plane is opened.
    ///
    /// **A change re-asks; a withdrawal only reports.** The argument is the blast radius of
    /// what actually changed, and it is `layer::RESTRICTIVE`'s argument applied one level up.
    ///
    /// *Why an addition re-asks.* An approval is consent to a **contribution**, not to a
    /// path. `enabledPlugins` is code that will run inside the operator's harness and `env`
    /// decides what that harness talks to — so a plane that gains either after it was
    /// approved has been handed a grant nobody looked at. Reporting it and opening anyway
    /// would put the notice in the one place an operator has already decided not to read: a
    /// window that opened successfully. [`crate::profiletrust`] settled the identical
    /// question for a profile's command line — `Approval::Changed` asks — and a plugin is
    /// strictly more than a command line, so a weaker rule here would be the same decision
    /// made twice with different answers, which is the defect this repo keeps finding.
    ///
    /// *Why a withdrawal does not.* Removing a plugin or an environment variable cannot make
    /// anything run that would not have run under the approval already given; it can only
    /// make less run. A prompt that never carries risk is a prompt an operator learns to
    /// answer yes to without reading, which spends the attention the real question needs.
    /// So a shrinkage is reported — the store still records what changed — and nothing stops.
    ///
    /// *Mixed.* One addition among ten removals is [`Consent::Grew`] and asks: the question
    /// is whether anything new was granted, never how much was given back.
    ///
    /// *Why a new chat in the reopen record asks.* It is a program and an argument list the
    /// app runs synchronously, in `setup`, before there is a window to close or a tray to
    /// quit from. There is no shape that separates a harness the operator installed from
    /// anything else — `reopen` says so in those words — so the only honest gate is consent,
    /// and the only moment it can be given is before the launch.
    ///
    /// *Why a profile chat does not.* See [`Change::is_a_grant`]: the record chooses which of
    /// this machine's own profiles runs, `profiletrust` gates what any of them runs, and a
    /// profile that is gone skips the chat.
    ///
    /// **This only works if charter vouches for its own writes.** charter rewrites the reopen
    /// record every time a chat opens or closes, so a fingerprint that were only ever taken at
    /// approval would disagree with the operator's own next action and ask again about a chat
    /// they just started — the training-to-click-yes failure, by the other road. [`Store::
    /// vouch`] is the answer, and the wiring contract is written there.
    pub fn must_ask(&self) -> bool {
        matches!(self, Self::New | Self::Grew(_))
    }

    /// **Whether it asks again about font files and nothing else** (#2647): it was approved,
    /// and every way it differs now is a font file of its theme folders. Only such an ask may
    /// be offered from a refused theme's Notice. A chat that can write the project can change
    /// a face whenever it likes, so a question it can raise on demand is kept to the faces;
    /// anything more is asked where the project is opened.
    pub fn grew_in_fonts_alone(&self) -> bool {
        matches!(self, Self::Grew(changes) if changes.iter().all(Change::is_a_font))
    }

    /// What changed, for a caller that wants to say so. Empty unless something did.
    pub fn changes(&self) -> &[Change] {
        match self {
            Self::New | Self::Unchanged => &[],
            Self::Grew(changes) | Self::Noted(changes) => changes,
        }
    }
}

/// What the operator approved about one plane, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trust {
    /// Seconds since the epoch.
    pub approved: u64,
    /// What the plane contributed at that moment — not merely that it was approved. The
    /// difference is the whole of [`Consent`].
    pub contributed: Contribution,
}

/// One plane the operator has opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recent {
    /// Absolute, `..`-free, as it was when it was opened. Whether anything is still there is
    /// [`still_a_plane`]'s question.
    pub plane: PathBuf,
    /// Seconds since the epoch.
    pub opened: u64,
    /// The approval, where there is one.
    ///
    /// **A field of the entry, so there are never two lists to keep in step.** Forgetting a
    /// plane forgets its approval with it, which is both the simple implementation and the
    /// safe direction: the plane is asked about again.
    pub trust: Option<Trust>,
    /// Whether the operator pinned this project (ADR 0039, stored per ADR 0040).
    ///
    /// A field of the entry for the same reason the approval is: forgetting a plane forgets
    /// its pin, and there is never a second list of pins to go stale against this one. It is
    /// also what exempts the entry from [`MOST_RECENTS`].
    pub pinned: bool,
    /// The workspaces inside this plane the operator pinned, by name.
    ///
    /// **References into the plane, never a copy of what the plane says.** The plane is what
    /// says which workspaces exist and in what order; this says which of them this operator
    /// wants first, which is a fact about the operator and false inside any one plane. A name
    /// here that no longer names a workspace is a dangling reference and reads as one — see
    /// [`Store::pinned_workspaces`].
    ///
    /// **In the order they were pinned in**, each name once: a pin is appended, an unpin takes
    /// its name out, and the workspace strip draws them in this order (ADR 0054, charter#402).
    /// This was a set drawn in the plane's order until then; a file written that way holds its
    /// names sorted, which is the plane's own order, so it reads back as the arrangement the
    /// operator already saw ([`read`]).
    pub pinned_workspaces: Vec<String>,
    /// Whether [`Store::pin_the_most_active`] has run for this plane (ADR 0054).
    ///
    /// **Beside the pins, because it is a fact about them**: that this operator's workspace
    /// pins in this plane were started for them once, so that what is pinned now is theirs.
    /// Without it an operator who unpinned everything would be pinned again at the next open,
    /// and the strip would be arranging itself. Forgetting the plane forgets it with the pins,
    /// and the next open pins again — which is the arrangement a first open gets.
    pub most_active_pinned: bool,
    /// Which pool each of this plane's workspaces runs its chats on, on this machine, by the
    /// workspace's name (#2679, ruled 2026-10-20): a workspace not named here runs its chats on
    /// this machine. A pool is named here, never defined: what it is, and its runner, are in
    /// `runners.json` ([`crate::runners`]), and a name not kept there starts no chat.
    pub chats_run_on: BTreeMap<String, String>,
}

/// One window, and the planes it had open as tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// Left to right, as the tabs were.
    pub planes: Vec<PathBuf>,
    /// Which tab was in front. Always a valid index into `planes`, which is not something a
    /// file can promise — see [`read`].
    pub active: usize,
}

/// Everything charter keeps outside a plane.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Store {
    /// Most recently opened first.
    pub recents: Vec<Recent>,
    pub windows: Vec<Window>,
    /// Which stream this machine takes charter from ([`crate::updates::Channel`]).
    ///
    /// The fifth thing, and the first that is not about planes at all — the module note above
    /// says why it is here and why the count had to be argued again.
    pub channel: crate::updates::Channel,
    /// This device's id, once one has been asked for ([`device_id`]).
    pub device: Option<Device>,
    /// The ISO week this machine last fetched the weekly manifest in
    /// ([`crate::updates::weekly_due`], OB-17). Never sent anywhere.
    pub weekly: Option<crate::updates::IsoWeek>,
    /// The browser limits set on this machine (#2023, ruled 2026-10-18): each absent one is
    /// purlis's default ([`crate::browserlimits`]).
    pub browsers: BrowserLimits,
    /// The flags turned on on this machine (#2764): each off until turned on.
    pub flags: Flags,
}

/// **The browser limits set on this machine** (#2023): `browsers.per_project`,
/// `browsers.on_this_device` and `browsers.idle_minutes` in the file. `None` is not set, and
/// purlis's default is in force; a value out of [`crate::browserlimits`]'s bounds is read as
/// not set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrowserLimits {
    pub per_project: Option<u32>,
    pub on_this_device: Option<u32>,
    /// `0` is never: a chat's browser does not stop for going unused.
    pub idle_minutes: Option<u32>,
    /// `browsers.engine` is `"machine"`: browsers start on this machine's own Chrome where one
    /// is installed, not the pinned engine (#2835, ADR 0042's choice). Absent, the pinned one.
    pub machine_chrome: bool,
}

/// **The flags turned on on this machine** (#2764): `flags.<name>` in the file, `true` when on.
/// Anything else, or a name this build does not know, is off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    /// `skill-studio`: the Skill studio's slices, until its finish line (spec #2762, SK-23).
    pub skill_studio: bool,
}

/// A device's id (ADR 0066): random, minted at the first launch that finds none, and the only
/// key anything charter records about where it happened uses. The hostname is a label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// A ULID.
    pub id: String,
    /// When it was minted, in seconds since the epoch.
    pub created: u64,
}

/// This device's id, minting it the first time it is asked for.
///
/// Through [`update`], so two processes asking at once mint one id between them, and an
/// unreadable store refuses rather than minting a second identity over the first. Where the
/// store refuses (not unix, ADR 0031) there is no device id, and this says so.
pub fn device_id(config_root: &Path) -> io::Result<String> {
    let loaded = update(config_root, |store| {
        mint_into(store);
    })?;
    loaded
        .store
        .device
        .map(|device| device.id)
        .ok_or_else(|| io::Error::other("the machine store kept no device id"))
}

/// A new id for this device, minted now, or `None` when the store already holds one, which it
/// keeps: what a runner's pairing asks ([`crate::runnerlink::pair`]), since a runner's id is
/// minted by the runner itself and never comes with its home.
///
/// Through [`update`], so of two processes asking at once one mints and the other is told
/// there is one already.
pub fn mint_device_id(config_root: &Path) -> io::Result<Option<String>> {
    let mut minted = None;
    update(config_root, |store| minted = mint_into(store))?;
    Ok(minted)
}

/// Mints a device id into `store` when it holds none, and says the id it minted.
fn mint_into(store: &mut Store) -> Option<String> {
    if store.device.is_some() {
        return None;
    }
    let id = ulid::Ulid::generate().to_string();
    store.device = Some(Device {
        id: id.clone(),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default(),
    });
    Some(id)
}

/// This device's id if one has been minted, and `None` otherwise: never mints one.
///
/// For a writer that only names a file by it (FD-25): a hook or a `charter` command must not
/// write the machine store as a side effect of appending a log line, and the id is minted at
/// the first launch that finds none (ADR 0066).
pub fn known_device_id(config_root: &Path) -> Option<String> {
    read(config_root).store.device.map(|device| device.id)
}

/// This device's id, from the config home's machine store, minting it the first time: the one
/// lookup a `charter` command makes for it. Refused, in words, where there is no config home or
/// the store keeps none.
pub fn this_device_id() -> Result<String, String> {
    let config = config_root()
        .ok_or_else(|| "there is no config home to keep this device's id in".to_string())?;
    device_id(&config).map_err(|e| e.to_string())
}

/// The name this device's logs are filed under (FD-25, ADR 0066): its device id from the
/// machine store at `config`, so two machines that share a hostname write two files and a
/// renamed machine keeps its one. `host` ([`crate::dispatch::host`]) only where no id has been
/// minted yet, or can be (ADR 0031): there it is the name the file had before.
///
/// It names all five per-device logs: the dispatch, skill-use, piece, landing and pending
/// logs. Never mints the id: asking for a log's name writes nothing outside the project. The
/// hostname stays a label wherever a line shows one (the piece claim log's `host`).
pub fn log_name(config: Option<&Path>, host: &str) -> String {
    config
        .and_then(known_device_id)
        .unwrap_or_else(|| host.to_string())
}

/// [`log_name`] for this process: the config home's store, if there is one, and this
/// machine's [`crate::dispatch::host`].
pub fn this_log_name() -> String {
    log_name(config_root_if_there().as_deref(), &crate::dispatch::host())
}

/// The browser limits off the file's `browsers` table: each one in its bounds, or not set.
fn browsers_of(raw: Option<&serde_json::Value>) -> BrowserLimits {
    use crate::browserlimits::{self as limits, Limit};
    let one = |limit: Limit| {
        raw.and_then(|table| table.get(limit.key()))
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| limits::within(limit, *n))
    };
    BrowserLimits {
        per_project: one(Limit::PerProject),
        on_this_device: one(Limit::OnThisDevice),
        idle_minutes: one(Limit::IdleMinutes),
        machine_chrome: raw
            .and_then(|table| table.get(BROWSER_ENGINE))
            .and_then(serde_json::Value::as_str)
            == Some(MACHINE_CHROME),
    }
}

/// The key of the `browsers` table that says which engine browsers start on (#2835).
const BROWSER_ENGINE: &str = "engine";

/// Its one value: this machine's own Chrome.
const MACHINE_CHROME: &str = "machine";

/// **Browsers on this machine start on its own Chrome**, or with `false` on the pinned engine
/// again (#2835), in the store under `config_root`.
pub fn set_machine_chrome(config_root: &Path, on: bool) -> Result<(), String> {
    update(config_root, |store| store.browsers.machine_chrome = on)
        .map(|_| ())
        .map_err(|why| format!("purlis could not keep it, so nothing was changed: {why}"))
}

/// The flags off the file's `flags` table: a flag is on only where it is `true`.
fn flags_of(raw: Option<&serde_json::Value>) -> Flags {
    let on = |name: &str| {
        raw.and_then(|table| table.get(name))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    Flags {
        skill_studio: on(crate::skills::STUDIO_FLAG),
    }
}

/// The device off the file: an id that is not a ULID is no id, and the next ask mints one.
fn device_of(raw: Option<&serde_json::Value>) -> Option<Device> {
    let raw = raw?;
    let id = raw.get("id")?.as_str()?;
    ulid::Ulid::from_string(id).ok()?;
    Some(Device {
        id: id.to_owned(),
        created: raw
            .get("created")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
    })
}

impl Store {
    /// What is remembered about `plane`, if anything.
    pub fn recent(&self, plane: &Path) -> Option<&Recent> {
        self.recents.iter().find(|entry| entry.plane == plane)
    }

    /// Put `plane` at the front of the list, keeping whatever was approved about it.
    ///
    /// **An open is not an approval**, so an existing [`Trust`] is carried over untouched and
    /// a plane that had none still has none. Opening a plane a hundred times must not turn
    /// into consent.
    pub fn remember(&mut self, plane: &Path, when: u64) {
        let entry = match self.recents.iter().position(|e| e.plane == plane) {
            Some(at) => {
                let mut entry = self.recents.remove(at);
                entry.opened = when;
                entry
            }
            None => Recent {
                plane: plane.to_path_buf(),
                opened: when,
                trust: None,
                pinned: false,
                pinned_workspaces: Vec::new(),
                most_active_pinned: false,
                chats_run_on: BTreeMap::new(),
            },
        };
        self.recents.insert(0, entry);
        self.trim();
    }

    /// Drops the oldest **unpinned** entries past [`MOST_RECENTS`], keeping every pinned one
    /// up to [`MOST_PINNED`] ([`Room`]).
    ///
    /// **Not `truncate`, and ADR 0040 is why.** A pin is the only thing an operator
    /// can say to mean "not this one", so a pin that the sixty-fifth plane opened could
    /// silently undo would be a feature that stops working on the machine it is for.
    /// [`MOST_PINNED`] is what keeps the file bounded in its place; a pin beyond that is
    /// refused rather than granted, because the alternative is an unbounded file read at a
    /// cold launch.
    fn trim(&mut self) {
        let mut room = Room::default();
        self.recents.retain(|entry| room.fits(entry.pinned));
    }

    /// Whether another plane may be pinned on this machine.
    pub fn room_to_pin(&self) -> bool {
        self.recents.iter().filter(|entry| entry.pinned).count() < MOST_PINNED
    }

    /// Pins or unpins a project, answering whether anything changed.
    ///
    /// **A plane charter does not already remember cannot be pinned**, and that is the rule
    /// rather than an omission: this file's own test is that deleting it costs the operator
    /// their arrangement and nothing else, and an entry that exists only to hold a pin would
    /// be a plane path in the store that no open ever put there.
    pub fn pin(&mut self, plane: &Path, pinned: bool) -> Result<bool, String> {
        if pinned && !self.room_to_pin() && !self.recent(plane).is_some_and(|one| one.pinned) {
            return Err(format!(
                "purlis pins at most {MOST_PINNED} projects on one machine. Unpin one first."
            ));
        }
        let Some(entry) = self.recents.iter_mut().find(|one| one.plane == plane) else {
            return Err(
                "purlis does not remember that project, so there is nothing to pin.".to_owned(),
            );
        };
        let moved = entry.pinned != pinned;
        entry.pinned = pinned;
        // Unpinning puts the entry back under the recents bound, where it may now be past it.
        //
        // Only then, though the trim would be harmless otherwise: every other change here keeps
        // the store inside both bounds, and the trim leaves such a store as it is. So `||` for
        // this `&&` is an equivalent mutant, excluded in `.cargo/mutants.toml` on that ground.
        //
        // **The entry just unpinned is kept** (#1240 F2): its Undo pins it back, and the
        // operator just touched it. It counts first, so the oldest of the others goes instead.
        if moved && !pinned {
            let mut room = Room::default();
            room.fits(false);
            self.recents
                .retain(|entry| entry.plane == plane || room.fits(entry.pinned));
        }
        Ok(moved)
    }

    /// Pins or unpins a workspace inside a plane, answering whether anything changed.
    ///
    /// The name is held to being a plain directory name here as well as on the way back in
    /// ([`usable_workspace`]): a pin written with a `/` in it would be a path, and a path in
    /// this field is the beginning of a second kind of thing living in it.
    pub fn pin_workspace(
        &mut self,
        plane: &Path,
        workspace: &str,
        pinned: bool,
    ) -> Result<bool, String> {
        if pinned {
            // A new pin goes last: the order is the order they were pinned in.
            return self.pin_workspace_at(plane, workspace, usize::MAX);
        }
        usable_workspace(workspace)?;
        let Some(entry) = self.recents.iter_mut().find(|one| one.plane == plane) else {
            return Err(
                "purlis does not remember that project, so there is nothing to pin in.".to_owned(),
            );
        };
        let had = entry.pinned_workspaces.len();
        entry.pinned_workspaces.retain(|one| one != workspace);
        Ok(entry.pinned_workspaces.len() != had)
    }

    /// Pins a workspace at place `at` among the plane's workspace pins — the end, where `at` is
    /// past it — answering whether anything changed (ST-2, #1226).
    ///
    /// **The Undo of an unpin**: the pin goes back where it was, so the strip the operator
    /// arranged is the strip they get back. Every rule [`Self::pin_workspace`] keeps is kept
    /// here, because that is the one pinning path and this is it: the name is a plain name,
    /// the plane is one charter remembers, and the bound holds. A pin already there keeps its
    /// own place.
    pub fn pin_workspace_at(
        &mut self,
        plane: &Path,
        workspace: &str,
        at: usize,
    ) -> Result<bool, String> {
        usable_workspace(workspace)?;
        let Some(entry) = self.recents.iter_mut().find(|one| one.plane == plane) else {
            return Err(
                "purlis does not remember that project, so there is nothing to pin in.".to_owned(),
            );
        };
        if entry.pinned_workspaces.iter().any(|one| one == workspace) {
            return Ok(false);
        }
        if entry.pinned_workspaces.len() >= MOST_PINNED_WORKSPACES {
            return Err(format!(
                "purlis pins at most {MOST_PINNED_WORKSPACES} workspaces in one project. \
                 Unpin one first."
            ));
        }
        let at = at.min(entry.pinned_workspaces.len());
        entry.pinned_workspaces.insert(at, workspace.to_owned());
        Ok(true)
    }

    /// Puts a plane's pinned workspaces in the order `order` names them — the order the
    /// operator dragged them into on the workspace strip (ADR 0039, as amended by SI-6) —
    /// answering whether anything changed.
    ///
    /// **Arranging is not pinning.** A name in `order` that is not pinned is passed over:
    /// pinning has its own bound and its own refusal ([`Self::pin_workspace`]), and a
    /// drag that crosses from the unpinned tabs to the pinned ones pins first and arranges
    /// second.
    ///
    /// **Only what the window drew moves.** The pins `order` names take, in `order`'s
    /// sequence, the places those same pins held; a pin it does not name keeps its place. The
    /// window draws only the pins whose workspace still exists ([`Self::pinned_workspaces`]),
    /// so a dangling pin is one it never saw, and it is not the window's to move.
    pub fn arrange_workspaces(&mut self, plane: &Path, order: &[&str]) -> Result<bool, String> {
        let Some(entry) = self.recents.iter_mut().find(|one| one.plane == plane) else {
            return Err(
                "purlis does not remember that project, so there is nothing to arrange in."
                    .to_owned(),
            );
        };
        let mut named = order
            .iter()
            .filter(|name| entry.pinned_workspaces.iter().any(|one| one == *name));
        let arranged: Vec<String> = entry
            .pinned_workspaces
            .iter()
            .map(|one| {
                if order.contains(&one.as_str()) {
                    named
                        .next()
                        .map_or_else(|| one.clone(), |name| (*name).to_owned())
                } else {
                    one.clone()
                }
            })
            .collect();
        let changed = arranged != entry.pinned_workspaces;
        entry.pinned_workspaces = arranged;
        Ok(changed)
    }

    /// **Sets where `workspace` of `plane` runs its chats on this machine** (#2679): on `pool`, or
    /// on this machine where it is `None`. Answers whether anything changed. A pool's name is
    /// checked as a name only: whether this machine keeps it is asked as each chat starts, so a
    /// pool removed later starts no chat and says so ([`crate::runners::runner_for`]).
    pub fn set_chats_run_on(
        &mut self,
        plane: &Path,
        workspace: &str,
        pool: Option<&str>,
    ) -> Result<bool, String> {
        usable_workspace(workspace).map_err(|why| format!("The workspace's name {why}."))?;
        if let Some(pool) = pool {
            crate::runners::usable_pool_name(pool)?;
        }
        let Some(entry) = self.recents.iter_mut().find(|one| one.plane == plane) else {
            return Err("purlis does not remember that project, so nothing was set.".to_owned());
        };
        let Some(pool) = pool else {
            return Ok(entry.chats_run_on.remove(workspace).is_some());
        };
        if !entry.chats_run_on.contains_key(workspace)
            && entry.chats_run_on.len() >= MOST_CHATS_RUN_ON
        {
            return Err(format!(
                "purlis runs at most {MOST_CHATS_RUN_ON} workspaces of one project on a pool."
            ));
        }
        Ok(entry
            .chats_run_on
            .insert(workspace.to_owned(), pool.to_owned())
            .as_deref()
            != Some(pool))
    }

    /// Which pool `workspace` of `plane` runs its chats on, on this machine: `None` is this
    /// machine (#2679).
    pub fn chats_run_on(&self, plane: &Path, workspace: &str) -> Option<&str> {
        self.recent(plane)?
            .chats_run_on
            .get(workspace)
            .map(String::as_str)
    }

    /// Every workspace of `plane` that runs its chats on a pool on this machine, with its pool
    /// (#2046).
    pub fn every_chats_run_on(&self, plane: &Path) -> Vec<(&str, &str)> {
        self.recent(plane)
            .map(|entry| {
                entry
                    .chats_run_on
                    .iter()
                    .map(|(workspace, pool)| (workspace.as_str(), pool.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Moves a workspace's pin to the name it was renamed to (charter#367), keeping its
    /// place in the order, and answers whether anything changed.
    ///
    /// A pin under the old name would otherwise dangle, which [`Self::pinned_workspaces`] then
    /// names as gone, and the renamed workspace would drop off the strip. Nothing is pinned
    /// that was not: a workspace nobody pinned is renamed unpinned.
    pub fn rename_workspace(&mut self, plane: &Path, old: &str, new: &str) -> bool {
        if usable_workspace(new).is_err() {
            return false;
        }
        let Some(entry) = self.recents.iter_mut().find(|one| one.plane == plane) else {
            return false;
        };
        let already = entry.pinned_workspaces.iter().any(|one| one == new);
        let mut changed = false;
        entry.pinned_workspaces.retain_mut(|one| {
            if one != old {
                return true;
            }
            changed = true;
            if already {
                return false;
            }
            new.clone_into(one);
            true
        });
        // Where its chats run goes with it, unless the new name already has its own (#2679).
        if let Some(pool) = entry.chats_run_on.remove(old) {
            entry.chats_run_on.entry(new.to_owned()).or_insert(pool);
            changed = true;
        }
        changed
    }

    /// The workspaces pinned in `plane` that **still exist**, and the pins that no longer name
    /// one.
    ///
    /// **A dangling pin is named, never drawn.** A workspace renamed or removed on disk leaves
    /// a pin with nothing under it, and a window that drew it would be offering a workspace
    /// the plane does not have — the same hazard ADR 0034 already names for a trust entry
    /// keyed on a path, one level down. `there` is the plane's own list and says which pins
    /// still resolve; the answer is in the order they were pinned in, which is the order the
    /// workspace strip draws them in (ADR 0054, charter#402).
    pub fn pinned_workspaces<'a>(
        &self,
        plane: &Path,
        there: &[&'a str],
    ) -> (Vec<&'a str>, Vec<String>) {
        let Some(entry) = self.recent(plane) else {
            return (Vec::new(), Vec::new());
        };
        let kept = entry
            .pinned_workspaces
            .iter()
            .filter_map(|name| there.iter().copied().find(|one| one == name))
            .collect();
        let gone = entry
            .pinned_workspaces
            .iter()
            .filter(|name| !there.contains(&name.as_str()))
            .cloned()
            .collect();
        (kept, gone)
    }

    /// Pins the [`FIRST_OPEN_PINS`] most recently active workspaces in `plane`, **once**
    /// (ADR 0054), answering whether it ran.
    ///
    /// The workspace strip draws the pinned workspaces and the one you are in, so a plane
    /// opened before that rule, or opened here for the first time, would draw almost nothing.
    /// This is its one-time arrangement: the workspaces most recently worked in, ranked by
    /// `briefing`'s `last_active` so "most recently active" means what the session-start
    /// briefing means by it. **Only in a plane the operator has pinned nothing in**: one they
    /// have arranged already is theirs, and it is marked done without a pin added.
    ///
    /// **Once, and recorded** ([`Recent::most_active_pinned`]). An operator who then unpins
    /// everything is not pinned again: a strip that pins and unpins on its own is the moving
    /// target ADR 0039 refused. A plane charter does not remember has nowhere to record it
    /// and is not touched; one whose workspaces cannot be listed is left to try again at the
    /// next open, because recording a run that pinned nothing it could see would be a lie
    /// the operator pays for with an empty strip.
    pub fn pin_the_most_active(&mut self, plane: &Path) -> bool {
        let Some(entry) = self.recents.iter_mut().find(|one| one.plane == plane) else {
            return false;
        };
        if entry.most_active_pinned {
            return false;
        }
        // The operator has arranged this plane already, and adding to it would be the strip
        // arranging itself. Spent, so unpinning everything later does not set it off either.
        if !entry.pinned_workspaces.is_empty() {
            entry.most_active_pinned = true;
            return false;
        }
        let Ok(names) = crate::workspaces::Plane::open(plane).workspaces() else {
            return false;
        };
        let mut ranked: Vec<(f64, String)> = names
            .into_iter()
            .map(|name| {
                let when = crate::briefing::last_active(plane, &name).unwrap_or(0.0);
                (when, name)
            })
            .collect();
        // Newest first, and a tie by name: the strip draws the pins in this order, so it
        // must not hang on which of two equally active workspaces the disk listed first.
        ranked.sort_by(|one, other| other.0.total_cmp(&one.0).then_with(|| one.1.cmp(&other.1)));
        for (_, name) in ranked.into_iter().take(FIRST_OPEN_PINS) {
            if entry.pinned_workspaces.len() >= MOST_PINNED_WORKSPACES {
                break;
            }
            entry.pinned_workspaces.push(name);
        }
        entry.most_active_pinned = true;
        true
    }

    /// Record that the operator approved `plane` while it contributed `contributed`.
    pub fn approve(&mut self, plane: &Path, when: u64, contributed: Contribution) {
        self.remember(plane, when);
        if let Some(entry) = self.recents.first_mut() {
            entry.trust = Some(Trust {
                approved: when,
                contributed,
            });
        }
    }

    /// Re-fingerprint an **already approved** plane, because charter itself just changed what
    /// opening it would do.
    ///
    /// **The wiring contract, and the whole design rests on it:** whoever writes a plane's
    /// `.charter/app/reopen.json` calls this immediately afterwards. charter rewrites that
    /// record every time a chat opens or closes (`Chats::write_it_down`), so without this the
    /// stored fingerprint would go stale on the operator's own first action and
    /// [`Consent::must_ask`] would fire on a chat they started themselves. With it, the
    /// fingerprint tracks charter's own writes and can only ever disagree when **something
    /// that is not this charter** wrote the record — which is exactly the case the question
    /// exists for.
    ///
    /// **It never creates an approval**, only refreshes one. A plane nobody has approved stays
    /// [`Consent::New`] however many times charter writes its record, so a wiring mistake
    /// cannot turn charter's own bookkeeping into consent.
    ///
    /// Order the two writes as record-then-vouch or vouch-then-record as suits the caller: a
    /// crash between them leaves a fingerprint that does not match the record on disk, which
    /// is one spurious question at the next launch. That is the direction that is safe, and it
    /// is the only one.
    ///
    /// **Only the record's half is refreshed.** charter writes the reopen record; it does not
    /// write the plane's settings or its personas, which arrive with whatever is committed to
    /// the plane. So [`Contribution::starts`] and [`Contribution::profiles`] are taken from
    /// `contributed` and everything else stays as it was approved — a vouch that took the rest
    /// too would approve a pulled change on the operator's next chat, which is a question
    /// answered without being asked.
    pub fn vouch(&mut self, plane: &Path, contributed: Contribution, when: u64) {
        if let Some(entry) = self.recents.iter_mut().find(|e| e.plane == plane)
            && let Some(trust) = entry.trust.as_mut()
        {
            trust.approved = when;
            trust.contributed.starts = contributed.starts;
            trust.contributed.profiles = contributed.profiles;
        }
    }

    /// The grant the operator approved for `persona` in `plane`, as
    /// [`crate::personagrant::grant_text`] wrote it — `None` when the plane was never
    /// approved on this machine, or approved without that persona granting anything.
    pub fn approved_grant(&self, plane: &Path, persona: &str) -> Option<&str> {
        self.recent(plane)?
            .trust
            .as_ref()?
            .contributed
            .grants
            .get(persona)
            .map(String::as_str)
    }

    /// Drop `plane` from the list, and with it any approval, its pins and its tabs, answering
    /// whether it was remembered at all.
    pub fn forget(&mut self, plane: &Path) -> bool {
        let had = self.recents.len();
        self.recents.retain(|entry| entry.plane != plane);
        for window in &mut self.windows {
            window.planes.retain(|open| open != plane);
        }
        self.windows.retain(|window| !window.planes.is_empty());
        for window in &mut self.windows {
            window.active = window.active.min(window.planes.len() - 1);
        }
        self.recents.len() != had
    }

    /// Take back the approval of `plane`, keeping it remembered and pinned, and answer whether
    /// there was one (ST-2, #1226).
    ///
    /// **The safe direction, and only that one.** The next open asks again, exactly as a first
    /// open does ([`Consent::New`]), and a persona's approved grant goes with it. There is no
    /// way back from here but the ask: an approval is only ever minted by the operator
    /// answering it ([`Self::approve`]), never restored.
    pub fn revoke(&mut self, plane: &Path) -> bool {
        self.recents
            .iter_mut()
            .find(|entry| entry.plane == plane)
            .and_then(|entry| entry.trust.take())
            .is_some()
    }

    /// Re-point the remembered `gone` at `found` (NO-5, #1237), answering whether `gone` was
    /// remembered at all. [`locate_project`] is the way in, and it checks `found` first.
    ///
    /// The entry keeps its place in the list, its pins and every tab a window would put back,
    /// because those are the operator's arrangement and the project is the one they mean.
    /// **Its approval does not travel.** An approval is of what was at a path, and charter
    /// cannot tell a project that moved from another folder that holds a manifest, so the next
    /// open asks, exactly as a first open does: [`Self::revoke`]'s safe direction.
    ///
    /// When `found` is already remembered, that entry is the project's and keeps its place and
    /// its approval; `gone` is folded into it, bringing its project pin and the workspace pins
    /// it lacks, and a window holding both keeps one tab.
    pub fn relocate(&mut self, gone: &Path, found: &Path) -> bool {
        let Some(at) = self.recents.iter().position(|entry| entry.plane == gone) else {
            return false;
        };
        // The disk came back and the operator picked the same folder: nothing to re-point.
        if gone == found {
            return true;
        }
        if self.recent(found).is_some() {
            let folded = self.recents.remove(at);
            if let Some(entry) = self.recents.iter_mut().find(|entry| entry.plane == found) {
                // The pins are the operator's arrangement, so they join the project's own:
                // its own first, then the gone entry's that it lacks, within the bound. The
                // count of pinned projects cannot grow, since the gone entry's pin goes.
                entry.pinned |= folded.pinned;
                for name in folded.pinned_workspaces {
                    if entry.pinned_workspaces.len() < MOST_PINNED_WORKSPACES
                        && !entry.pinned_workspaces.contains(&name)
                    {
                        entry.pinned_workspaces.push(name);
                    }
                }
            }
        } else {
            let entry = &mut self.recents[at];
            entry.plane = found.to_path_buf();
            entry.trust = None;
        }
        for window in &mut self.windows {
            let front = window.planes.get(window.active).cloned();
            let mut planes: Vec<PathBuf> = Vec::with_capacity(window.planes.len());
            for plane in window.planes.drain(..) {
                let plane = if plane == gone {
                    found.to_path_buf()
                } else {
                    plane
                };
                if !planes.contains(&plane) {
                    planes.push(plane);
                }
            }
            let front = front.map(|plane| {
                if plane == gone {
                    found.to_path_buf()
                } else {
                    plane
                }
            });
            window.active = front
                .and_then(|front| planes.iter().position(|plane| *plane == front))
                .unwrap_or(0);
            window.planes = planes;
        }
        true
    }

    /// Whether `plane` may be opened without asking, given what it contributes now.
    pub fn consent(&self, plane: &Path, contributes: &Contribution) -> Consent {
        let Some(trust) = self.recent(plane).and_then(|entry| entry.trust.as_ref()) else {
            return Consent::New;
        };
        let changes = trust.contributed.against(contributes);
        if changes.is_empty() {
            Consent::Unchanged
        } else if changes.iter().any(Change::is_a_grant) {
            Consent::Grew(changes)
        } else {
            Consent::Noted(changes)
        }
    }
}

/// Forget `plane` on this machine (You › This machine, ST-2): the recent, its approval, its
/// pins, any tab a window would put back, and the name its window link kept (#2738). Answers
/// whether it was remembered.
///
/// One call for the window and the CLI alike. A project that is gone from the disk is
/// forgotten the same way, which is how a gone recent stops coming back at every launch.
pub fn forget_project(config_root: &Path, plane: &Path) -> io::Result<bool> {
    let mut forgot = false;
    update(config_root, |store| forgot = store.forget(plane))?;
    // The name its window's link kept goes with it (#2738), so the project, opened again, takes
    // back no runner chat of the one forgotten. A name that cannot be let go of leaves the
    // project forgotten all the same: the name only ages out.
    if let Err(why) = crate::windownames::forget(config_root, plane) {
        tracing::warn!(
            "purlis: the window's name for the link of {} is still kept: {why}",
            plane.display()
        );
    }
    Ok(forgot)
}

/// Revoke the approval of `plane` on this machine (ST-2): see [`Store::revoke`]. Answers
/// whether there was one.
pub fn revoke_approval(config_root: &Path, plane: &Path) -> io::Result<bool> {
    let mut revoked = false;
    update(config_root, |store| revoked = store.revoke(plane))?;
    Ok(revoked)
}

/// **Locate…** a remembered project that is gone (NO-5, #1237): re-point it at the folder the
/// operator picked, and answer the project found there.
///
/// `picked` is checked here, before anything is written, and the check is the open's: the
/// folder is followed to its real path and walked up to the project it is in (a folder inside
/// a project locates that project), and what is found must pass [`still_a_plane`]. A picked
/// path is followed through a link rather than refused, because no approval stands behind it
/// yet and none is carried to it ([`Store::relocate`]).
///
/// Refused, with the store untouched, when the folder is not in a project or `gone` is not
/// remembered. Never done on charter's own initiative: a disk that is unplugged may come back.
pub fn locate_project(config_root: &Path, gone: &Path, picked: &Path) -> Result<PathBuf, String> {
    let shown = crate::shown::short(&picked.display().to_string());
    let not_a_project = || {
        format!(
            "{shown} is not a project: purlis found no {} there or in any folder above it",
            crate::plane::MANIFEST
        )
    };
    let here = picked.canonicalize().map_err(|_| not_a_project())?;
    let found = crate::plane::find_root(&here).map_err(|_| not_a_project())?;
    still_a_plane(&found).map_err(|why| {
        format!(
            "{} {why}",
            crate::shown::short(&found.display().to_string())
        )
    })?;
    let mut located = false;
    update(config_root, |store| located = store.relocate(gone, &found))
        .map_err(|why| format!("purlis could not re-point that project: {why}"))?;
    if !located {
        return Err(format!(
            "purlis does not remember {}, so there is nothing to re-point",
            crate::shown::short(&gone.display().to_string())
        ));
    }
    Ok(found)
}

/// Something the store held that charter would not take back.
///
/// Every one of these is a **drop with a reason**, never an error a launch has to handle: an
/// opener that shows one fewer row and can say why is usable, and a dialog before there is a
/// window is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dropped {
    /// The file parsed as something, and that something is not this store.
    TheStore(String),
    /// One remembered plane.
    Recent { plane: String, why: String },
    /// One window, or one tab in one.
    Window { plane: String, why: String },
    /// One pinned workspace, inside a plane that was itself taken.
    Pin { workspace: String, why: String },
    /// Where one workspace's chats run, inside a plane that was itself taken (#2679): the
    /// workspace runs its chats on this machine.
    RunsOn { workspace: String, why: String },
    /// The update channel, because the word is not one charter writes. The machine stays on
    /// the default, which is stable.
    Channel { named: String },
}

impl std::fmt::Display for Dropped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TheStore(why) => write!(f, "purlis's machine store {why}"),
            Self::Recent { plane, why } => write!(f, "the remembered plane '{plane}' {why}"),
            Self::Window { plane, why } => write!(f, "the open plane '{plane}' {why}"),
            Self::Pin { workspace, why } => write!(f, "the pinned workspace '{workspace}' {why}"),
            Self::RunsOn { workspace, why } => write!(
                f,
                "where the workspace '{workspace}' runs its chats {why}, so it runs them on this \
                 machine"
            ),
            Self::Channel { named } => write!(
                f,
                "'{named}' is not an update channel purlis knows, so this machine stays on \
                 {}",
                crate::updates::Channel::default().name()
            ),
        }
    }
}

/// What a read of the store came back with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loaded {
    pub store: Store,
    /// What was in the file and is not in the store, each with its reason.
    pub dropped: Vec<Dropped>,
    /// Why the file could not be read **at all** — a link, a FIFO, a giant, no permission, or
    /// a platform charter keeps no store on.
    ///
    /// This is not the same as a file that parsed badly, and [`update`] treats the two
    /// differently: content charter could read and did not understand is replaced, and a file
    /// charter could not read is left exactly where it is.
    pub unreadable: Option<String>,
}

/// Everything charter kept outside a plane, with whatever it would not take back named.
///
/// **This never fails**, because every caller is a launch. A missing file is a first launch;
/// anything else is an empty store plus a reason.
///
/// The read is gated the way every read of charter's own state is (ADR 0028):
///
/// - [`crate::contain::open_no_link`] against `config_root`, so a link at `charter/` or at the
///   file itself cannot make this answer out of somebody else's file — and so the last
///   component's answer is the **kernel's, at the instant of the open**, rather than
///   charter's a moment earlier;
/// - the plain-file and size questions asked of the **open descriptor**, which no swap can
///   get between, exactly as [`crate::reopen::read_or_refusal`] asks them;
/// - `O_NONBLOCK`, which comes with the same open, because a FIFO is not a link and reading
///   one blocks for ever — here, at a cold launch, before there is a window or a tray.
pub fn read(config_root: &Path) -> Loaded {
    let text = match read_text(config_root) {
        Ok(None) => return Loaded::default(),
        Ok(Some(text)) => text,
        Err(why) => {
            return Loaded {
                unreadable: Some(why.to_string()),
                ..Loaded::default()
            };
        }
    };
    let mut dropped = Vec::new();
    let store = match parse(&text) {
        Ok(doc) => load(&doc, &mut dropped),
        Err(why) => {
            dropped.push(Dropped::TheStore(why));
            Store::default()
        }
    };
    Loaded {
        store,
        dropped,
        unreadable: None,
    }
}

/// The text as a store document of **this** version, or why it is not one.
///
/// Read as a `serde_json::Value` and asked about by hand, the way
/// [`crate::profiletrust`] reads its record, rather than deserialised into a shape: every
/// level of this file can be anything, one bad entry must cost one row and not the list,
/// and this crate turns `arbitrary_precision` on — which changes what a `Value` is and is a
/// reason not to route a typed read back through one.
fn parse(text: &str) -> Result<serde_json::Value, String> {
    let doc: serde_json::Value =
        serde_json::from_str(text).map_err(|why| format!("is not a store purlis wrote: {why}"))?;
    if !doc.is_object() {
        return Err("is not a store purlis wrote: it is not an object".to_owned());
    }
    match doc.get("version").and_then(serde_json::Value::as_u64) {
        Some(found) if found == u64::from(VERSION) => Ok(doc),
        Some(found) => Err(format!(
            "is version {found}, and this purlis writes version {VERSION}"
        )),
        None => Err("says no version, so purlis cannot say what it means".to_owned()),
    }
}

/// The file's bytes, `None` for no file at all, an error for a file charter will not read.
fn read_text(config_root: &Path) -> io::Result<Option<String>> {
    read_beside(config_root, FILE, MAX_BYTES, "purlis's machine store")
}

/// The bytes of `name` in charter's own directory, gated exactly as the store's own read is.
///
/// **One implementation for every file in that directory** (`crate::beside` reads the
/// window's layout and the operator's theme through it). The module docstring's reason for
/// keeping a fifth thing in this file rather than beside it was that a second file would have
/// to get containment right a second time; this is how a file that has to be beside it — one
/// an operator edits by hand — gets it right the first time instead.
///
/// `what` names the file in a refusal, as the operator would: "charter's machine store".
pub(crate) fn read_beside(
    config_root: &Path,
    name: &str,
    max_bytes: u64,
    what: &str,
) -> io::Result<Option<String>> {
    supported()?;
    let target = dir(config_root).join(name);
    let mut open = match crate::contain::open_no_link(config_root, &target) {
        Ok(open) => open,
        Err(gone) if gone.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(refused) => return Err(refused),
    };
    // `fstat` of the descriptor the read will use, never of the name: the two cannot be
    // handed two different files.
    let found = open.metadata()?;
    if !found.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} is not a plain file, and {what} is read from nothing else",
                target.display()
            ),
        ));
    }
    if found.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} is {} bytes, and {what} is never larger than {max_bytes}",
                target.display(),
                found.len()
            ),
        ));
    }
    // The size was asked of the descriptor, and a file can still grow after it was: the read
    // itself stops one byte past the cap, so what is held is never more than was allowed.
    let mut text = String::new();
    io::Read::read_to_string(&mut io::Read::take(&mut open, max_bytes + 1), &mut text)?;
    if text.len() as u64 > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} grew past {max_bytes} bytes while it was read, and {what} is never larger",
                target.display()
            ),
        ));
    }
    Ok(Some(text))
}

/// Read the store, change it, and write it back.
///
/// **A store charter could not read is never overwritten.** The difference between "this file
/// says nothing charter understands" and "charter could not read this file" is the difference
/// between content worth replacing and a path that is compromised or a disk that is failing —
/// and clobbering the second destroys the operator's list and their approvals to fix nothing.
/// So the first is replaced and the second refuses, loudly, with the reason attached.
pub fn update(config_root: &Path, change: impl FnOnce(&mut Store)) -> io::Result<Loaded> {
    // **Held across the read AND the write, which is the whole of it** (charter-app#123). This
    // is a read-modify-write of one small file, and every writer of it rewrites the WHOLE
    // store — so two of them interleaving does not merge, it drops one. What is dropped is an
    // approval, a pin, or the list of projects a window had open.
    //
    // It was always possible and it is now ordinary. Tauri runs commands on a thread pool, so
    // two opens racing is two threads; `Records::vouch` fires on every chat that opens or
    // closes; and `remember_arrangement` fires on every tab change, which #125's author
    // measured as *far* more often than an approval. A second charter process makes it
    // cross-process, which is why this is `flock` and not a `Mutex`.
    let _held = Lock::on(config_root);
    let mut loaded = read(config_root);
    if let Some(why) = &loaded.unreadable {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("purlis will not overwrite a machine store it could not read: {why}"),
        ));
    }
    change(&mut loaded.store);
    write(config_root, &loaded.store)?;
    Ok(loaded)
}

/// The lock file's name, beside the store in charter's own `0700` directory.
///
/// A separate file and not the store itself: [`write`] replaces the store by `rename`, so a
/// lock held on the store's inode would be a lock on an inode that is no longer the store the
/// moment the first writer finished, and the second writer would take a lock on nothing.
const LOCK: &str = "machine.json.lock";

/// Held for the length of one [`update`], so a read-modify-write of the store is one act.
///
/// **Advisory, and every taker of it is in this module** — `update` is the only
/// read-modify-write, `write` on its own replaces the whole document, and nothing outside this
/// crate can reach either without going through them. A lock cannot guard a call that does not
/// take it, and here the calls that must take it are three lines apart.
///
/// **Best effort, with the consequence named.** A lock charter could not create or could not
/// take does not stop the update: the alternative is an app that cannot record an approval
/// because a `0700` directory is on a filesystem with no `flock` (many network mounts answer
/// `ENOTSUP`), which trades a rare lost write for a permanent one. What is lost when the lock
/// is missing is exactly what was lost before it existed, so this can only ever add safety.
///
/// It is taken **blocking**. `flock` is released by the kernel when the descriptor closes,
/// including on a process that was killed, so there is no holder that outlives a couple of
/// syscalls and nothing here can wedge on a stale lock.
///
/// [`Lock::named`] takes the same kind of lock on another file beside the store, for another
/// read-modify-write of a file in this directory (`windowprefs`' layout, NO-2).
pub(crate) struct Lock(Option<std::fs::File>);

impl Lock {
    fn on(config_root: &Path) -> Self {
        Self::named(config_root, LOCK)
    }

    /// Takes the lock whose file is `name`, beside the store.
    pub(crate) fn named(config_root: &Path, name: &str) -> Self {
        // A platform charter keeps no store on gets no directory made for one either.
        if supported().is_err() {
            return Self(None);
        }
        let Ok(dir) = private_dir(config_root) else {
            return Self(None);
        };
        Self::at(&dir.join(name))
    }

    /// Takes the lock whose file is `path`, in a directory the caller has made: for a
    /// read-modify-write of a file in a folder of its own under this one (`forge-budget`,
    /// #1164). The same best effort as [`Lock::named`].
    pub(crate) fn at(path: &Path) -> Self {
        // `create` and not `create_new`: the lock file is the *name* two processes agree on,
        // it holds nothing, and one left behind by a previous run is the ordinary case.
        let Ok(file) = std::fs::File::create(path) else {
            return Self(None);
        };
        #[cfg(unix)]
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive) {
            Ok(()) => Self(Some(file)),
            Err(_) => Self(None),
        }
        // Nothing to lock off unix, and nothing to lose: [`supported`] refuses the store
        // outright there (ADR 0031, charter-app#98), so `update` never reaches a write.
        #[cfg(not(unix))]
        {
            drop(file);
            Self(None)
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        if let Some(file) = self.0.take() {
            #[cfg(unix)]
            let _ = rustix::fs::flock(&file, rustix::fs::FlockOperation::Unlock);
            drop(file);
        }
    }
}

/// Write the store, creating charter's directory at `0700` if it is not there.
///
/// **Crash-safe by replacement, and the object being replaced is an inode** (the lesson
/// charter-app#82 left about `ETXTBSY`: reason about the inode, never about the path). The
/// bytes go to a file beside the store, are flushed to the disk, and are then `rename`d over
/// the name. A launch reading at that moment holds a descriptor on the old inode and reads
/// the whole of the old store; a launch opening afterwards opens the new one. Neither can see
/// half of one, and a process killed between the two leaves the previous store intact with a
/// stray temp file beside it.
///
/// That is [`crate::rewrite::replace`] with [`crate::rewrite::Mode::Private`] (#434), gated
/// from the config home: the walk covers the store's path **and** the temp file the bytes
/// actually land on — the mistake this repo has had six review rounds on, and the one that
/// put a whole `reopen.json` outside a plane through a committed
/// `reopen.json.writing -> elsewhere`. A store that is itself a link is refused.
pub fn write(config_root: &Path, store: &Store) -> io::Result<()> {
    supported()?;
    let dir = private_dir(config_root)?;
    let text = serde_json::to_string_pretty(&OnDisk::from(store))
        .expect("the store is plain data serde can always write");
    write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())
}

/// Replace `target`, a file in charter's own directory, whole: 0600, through the walk from
/// `config_root`, never through a link (#434). Every writer of that directory — the store,
/// the extension record, the window's layout — goes through here.
pub(crate) fn write_beside(config_root: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    crate::rewrite::replace(config_root, target, bytes, crate::rewrite::Mode::Private)
}

/// charter's directory under `config_root`, private to the operator.
///
/// **Tightened even when it is already there**, which is where this differs from
/// [`crate::profiletrust::private_dir`]'s rule of leaving an existing directory exactly as it
/// was found. That rule exists because `$CHARTER_HOME` can point a plane's state directory at
/// a home or a team share the operator chose, and charter has no business re-moding it. No
/// such escape hatch reaches here: this directory is charter's own, at a path charter alone
/// decides, holding a list of every project the operator opens and their approvals of each.
///
/// The tightening is best-effort, as every `chmod` in this crate is: a filesystem with fixed
/// permissions (exFAT, many network mounts) cannot hold a mode, and refusing to keep state to
/// protect a mode the filesystem was never going to keep helps nobody. The mode that the
/// guard rests on is the one set at **creation**, which is not best-effort.
pub(crate) fn private_dir(config_root: &Path) -> io::Result<PathBuf> {
    // The config home itself is made without a mode: `~/.config` belongs to the operator
    // and to every application on the machine, and charter creating it at 0700 would quietly
    // re-mode a directory that is not its own. `0700` starts at charter's own level.
    std::fs::create_dir_all(config_root)?;
    let dir = dir(config_root);
    // Refuses a symlink at the directory and creates it at 0700, which is exactly what is
    // wanted here — one implementation, because two drift.
    crate::profiletrust::private_dir(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Whether a remembered path still names a plane, or why it should be dropped.
///
/// **Deliberately not asked by [`read`], and that is a decision rather than an omission.**
/// Every question here is a `stat`, and a remembered plane can be on a network mount, an
/// unplugged external disk or an automounted share. A cold launch that stats sixty-four of
/// them before it can draw the opener is a launch that hangs on the one that is gone, with
/// nothing on screen to say so. So the read is lexical and total, and the disk is asked per
/// row, by whoever is about to show or open one.
///
/// A path that is a **symlink now** is dropped rather than followed. It may be honest — a
/// project moved to another volume and linked back — but the approval recorded against it was
/// recorded for what the path pointed at then, and charter cannot tell the two apart. The
/// repair is in the operator's hands and costs one dialog: open it again by its real path,
/// which asks about it again. (A link *above* the plane — a symlinked `$HOME`, `/tmp` on
/// macOS — is ordinary and is not asked about, exactly as `contain` does not ask about the
/// components above the root it is given.)
pub fn still_a_plane(plane: &Path) -> Result<(), String> {
    let found = std::fs::symlink_metadata(plane).map_err(|_| "is no longer there".to_owned())?;
    if found.file_type().is_symlink() {
        return Err(
            "is a symlink now, and purlis opens a plane by the path that was approved".to_owned(),
        );
    }
    if !found.is_dir() {
        return Err("is not a directory any more".to_owned());
    }
    if !crate::names::has_manifest(plane) {
        return Err(format!(
            "is not a plane any more: it holds no {}",
            crate::plane::MANIFEST
        ));
    }
    Ok(())
}

/// Whether a path off the file may be used, and why not.
///
/// Lexical only, and total: these are the questions that can be asked of a **string** off a
/// file charter did not write this run.
fn usable(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() {
        return Err("is empty".to_owned());
    }
    // A NUL terminates the string inside the C library, so the path charter checked and the
    // path the kernel opened would be two different strings.
    if raw.contains('\0') {
        return Err("holds a NUL, so the kernel would see a shorter path".to_owned());
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(
            "is not absolute, and a relative path resolves against a working directory that is \
             '/' for an app nobody launched from a terminal"
                .to_owned(),
        );
    }
    if path.components().any(|part| part == Component::ParentDir) {
        return Err("walks up through '..', and purlis's own paths never do".to_owned());
    }
    Ok(path)
}

/// A pinned workspace's name held to being a plain directory name, or why it is not one.
///
/// **A name and never a path.** A workspace is a directory beside `charter.toml` and nothing
/// in this file ever joins this string onto anything — but a separator, a `..` or a NUL in it
/// is the shape of a value that a later reader would join, and the cheapest place to refuse
/// that is where it comes off the disk. The same check runs on the way in
/// ([`Store::pin_workspace`]) so that a name charter would not read back is never written.
fn usable_workspace(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("is empty, and a workspace has a name".to_owned());
    }
    if name.len() > LONGEST_WORKSPACE_NAME {
        return Err(format!(
            "is {} bytes, and a workspace's name is never longer than {LONGEST_WORKSPACE_NAME}",
            name.len()
        ));
    }
    if name.contains('\0') {
        return Err("holds a NUL, which no name on a file system does".to_owned());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("is a path and not a name, and purlis pins a workspace by name".to_owned());
    }
    if name == "." || name == ".." {
        return Err("names a directory rather than a workspace in one".to_owned());
    }
    Ok(())
}

/// The file's contents held to what a store may be, with every refusal recorded.
///
/// Every value is asked about rather than assumed: a `recents` that is a number, an entry
/// that is a string, a `trust` that is a list. Each costs the one row it is, because a store
/// is read at a cold launch and one bad row must not be a lost list.
fn load(doc: &serde_json::Value, dropped: &mut Vec<Dropped>) -> Store {
    let mut recents: Vec<Recent> = Vec::new();
    // The two bounds, counted apart. One definition, in `Room`.
    let mut room = Room::default();
    for raw in array(doc.get("recents")) {
        let named = raw
            .get("plane")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let why = if named.is_empty() && !raw.is_object() {
            Err("is not an entry purlis wrote".to_owned())
        } else {
            usable(&named)
        };
        let plane = match why {
            Ok(plane) => plane,
            Err(why) => {
                dropped.push(Dropped::Recent { plane: named, why });
                continue;
            }
        };
        if recents.iter().any(|kept| kept.plane == plane) {
            dropped.push(Dropped::Recent {
                plane: named,
                why: "is in the list twice, and the later entry is the stale one".to_owned(),
            });
            continue;
        }
        // A `pinned` that is not a boolean is not a pin. That direction is the safe one: the
        // entry then spends the recents bound like any other and can fall off the end, which
        // costs an arrangement rather than letting a malformed file grow the file's bound.
        let pinned = raw
            .get("pinned")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !room.fits(pinned) {
            dropped.push(Dropped::Recent {
                plane: named,
                why: Room::why(pinned),
            });
            continue;
        }
        recents.push(Recent {
            plane,
            opened: number(raw.get("opened")),
            // A `trust` that is not one is NO trust, so the plane is asked about again.
            // There is no shape of malformed approval that it is safe to read as a yes.
            trust: raw.get("trust").and_then(trust_of),
            pinned,
            pinned_workspaces: pinned_workspaces_of(raw.get("pinnedWorkspaces"), dropped),
            // Anything but `true` is "not yet", which costs one pinning at the next open. The
            // other direction would leave a strip with nothing on it but where you are.
            most_active_pinned: raw
                .get("mostActivePinned")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            chats_run_on: chats_run_on_of(raw.get("chatsRunOn"), dropped),
        });
    }

    let mut windows = Vec::new();
    for raw in array(doc.get("windows")) {
        let mut planes = Vec::new();
        for named in array(raw.get("planes")) {
            let named = named.as_str().unwrap_or_default().to_owned();
            match usable(&named) {
                Ok(plane) => planes.push(plane),
                Err(why) => dropped.push(Dropped::Window { plane: named, why }),
            }
        }
        if planes.is_empty() {
            continue;
        }
        windows.push(Window {
            // Clamped rather than dropped: an index past the end is what a dropped tab
            // leaves behind, and a window that opens on the wrong tab is a smaller wrong
            // than a window that does not open.
            active: (number(raw.get("active")) as usize).min(planes.len() - 1),
            planes,
        });
    }

    Store {
        recents,
        windows,
        channel: channel_of(doc.get("channel"), dropped),
        device: device_of(doc.get("device")),
        weekly: doc
            .get("weekly")
            .and_then(serde_json::Value::as_str)
            .and_then(crate::updates::IsoWeek::named),
        browsers: browsers_of(doc.get("browsers")),
        flags: flags_of(doc.get("flags")),
    }
}

/// The pinned workspace names off one entry, with every refusal recorded.
///
/// A name charter would not write is dropped rather than raised, exactly as a remembered plane
/// is: a pin is an arrangement, and losing one costs the operator a click.
///
/// **In the file's order, which is the order they were pinned in** (charter#402). A file an
/// older charter wrote held the names as a set, written sorted — the plane's own order, since
/// the plane lists its workspaces sorted by name — so it reads back as the arrangement its
/// operator already had, with nothing to migrate. A name written twice is kept once, where it
/// was first.
fn pinned_workspaces_of(
    raw: Option<&serde_json::Value>,
    dropped: &mut Vec<Dropped>,
) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for value in array(raw) {
        let Some(name) = value.as_str() else {
            dropped.push(Dropped::Pin {
                workspace: String::new(),
                why: "is not a name purlis wrote".to_owned(),
            });
            continue;
        };
        if let Err(why) = usable_workspace(name) {
            dropped.push(Dropped::Pin {
                workspace: name.to_owned(),
                why,
            });
            continue;
        }
        // Asked before the bound, so a repeat of a name already kept is merged rather than
        // reported as past it.
        if kept.iter().any(|one| one == name) {
            continue;
        }
        if kept.len() >= MOST_PINNED_WORKSPACES {
            dropped.push(Dropped::Pin {
                workspace: name.to_owned(),
                why: format!(
                    "is past the {MOST_PINNED_WORKSPACES} workspaces purlis pins in one project"
                ),
            });
            continue;
        }
        kept.push(name.to_owned());
    }
    kept
}

/// Where each workspace runs its chats, off one entry (#2679), with every refusal recorded. A
/// workspace or pool name purlis would not write is dropped, and that workspace runs its chats
/// on this machine.
fn chats_run_on_of(
    raw: Option<&serde_json::Value>,
    dropped: &mut Vec<Dropped>,
) -> BTreeMap<String, String> {
    let mut kept = BTreeMap::new();
    let Some(raw) = raw else {
        return kept;
    };
    let Some(table) = raw.as_object() else {
        dropped.push(Dropped::RunsOn {
            workspace: String::new(),
            why: "is not a table purlis wrote".to_owned(),
        });
        return kept;
    };
    for (workspace, pool) in table {
        let named = usable_workspace(workspace).and_then(|()| {
            let pool = pool.as_str().ok_or_else(|| "names no pool".to_owned())?;
            crate::runners::usable_pool_name(pool)?;
            Ok(pool.to_owned())
        });
        match named {
            Ok(_) if kept.len() >= MOST_CHATS_RUN_ON => dropped.push(Dropped::RunsOn {
                workspace: workspace.clone(),
                why: format!(
                    "is past the {MOST_CHATS_RUN_ON} workspaces purlis runs on a pool in one \
                     project"
                ),
            }),
            Ok(pool) => {
                kept.insert(workspace.clone(), pool);
            }
            Err(why) => dropped.push(Dropped::RunsOn {
                workspace: workspace.clone(),
                why,
            }),
        }
    }
    kept
}

/// Which stream the file says this machine is on, defaulting to stable and saying when it did.
///
/// **Every way of not knowing lands on stable**, because the two directions are not
/// symmetric. Reading a corrupt file as *stable* costs an operator who had chosen dev one
/// trip back to the setting; reading it as *dev* puts a machine on a stream cut from any
/// green `main` without anybody asking for it. A file charter did not write must never be
/// able to do the second, and the safe answer is also the default answer, so nothing has to
/// remember which it was.
///
/// The word is [`crate::updates::Channel::named`]'s, which is exact: no trimming, no case
/// folding. What is dropped is recorded, so a store hand-edited to `"Dev"` says why it did
/// not take rather than silently doing nothing.
fn channel_of(
    raw: Option<&serde_json::Value>,
    dropped: &mut Vec<Dropped>,
) -> crate::updates::Channel {
    let Some(value) = raw else {
        // No field at all is the ordinary case: the store charter wrote before channels
        // existed, and the store it writes for every machine on the default.
        return crate::updates::Channel::default();
    };
    let named = value.as_str().unwrap_or_default();
    match crate::updates::Channel::named(named) {
        Some(channel) => channel,
        None => {
            dropped.push(Dropped::Channel {
                named: named.to_owned(),
            });
            crate::updates::Channel::default()
        }
    }
}

/// One approval off the file, or `None` for anything that is not one.
fn trust_of(raw: &serde_json::Value) -> Option<Trust> {
    if !raw.is_object() {
        return None;
    }
    Some(Trust {
        approved: number(raw.get("approved")),
        contributed: Contribution {
            plugins: words(raw.get("plugins")),
            env: words(raw.get("env")),
            starts: words(raw.get("starts")),
            profiles: words(raw.get("profiles")),
            grants: words(raw.get("grants")),
            fonts: words(raw.get("fonts")),
        },
    })
}

/// A JSON array, or nothing at all — never an error.
fn array(value: Option<&serde_json::Value>) -> &[serde_json::Value] {
    value
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// A JSON number as seconds or an index, or zero. A value that is not a number is zero: it is
/// an ordering hint and a tab index, and neither is worth dropping a row over.
fn number(value: Option<&serde_json::Value>) -> u64 {
    value
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default()
}

/// A JSON object of strings. A value that is not a string is dropped, which reads as the
/// plane contributing one thing fewer than it did — and therefore as [`Consent::Grew`] the
/// next time it is compared, which asks.
fn words(value: Option<&serde_json::Value>) -> BTreeMap<String, String> {
    value
        .and_then(serde_json::Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(name, value)| {
                    value.as_str().map(|value| (name.clone(), value.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The store as JSON, and the only place this file's field names are written down.
///
/// Writing is a derive and reading is by hand, deliberately: what charter writes is always
/// the same shape, and what it reads is whatever is on the disk.
#[derive(serde::Serialize)]
struct OnDisk {
    version: u32,
    /// When it was written, in seconds since the epoch. Nothing reads it; it is here because
    /// a file nobody can date is one nobody can debug.
    at: u64,
    recents: Vec<RecentOnDisk>,
    windows: Vec<WindowOnDisk>,
    /// Left out on the default, exactly as `pinned` is, so a machine that never chose a
    /// channel writes the file it wrote before channels existed — byte for byte. An older
    /// charter reading this ignores the field; this one reads its absence as stable, which is
    /// what was true.
    #[serde(skip_serializing_if = "is_the_default_channel")]
    channel: &'static str,
    /// Left out until one is minted, so a store that never needed one is the file it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    device: Option<DeviceOnDisk>,
    /// Left out until the first weekly count, for the same reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    weekly: Option<String>,
    /// Left out until a browser limit is set, for the same reason (#2023).
    #[serde(skip_serializing_if = "BrowsersOnDisk::is_unset")]
    browsers: BrowsersOnDisk,
    /// Left out while no flag is on, for the same reason (#2764).
    #[serde(skip_serializing_if = "FlagsOnDisk::is_unset")]
    flags: FlagsOnDisk,
}

#[derive(serde::Serialize)]
struct FlagsOnDisk {
    #[serde(rename = "skill-studio", skip_serializing_if = "is_false")]
    skill_studio: bool,
}

impl FlagsOnDisk {
    fn is_unset(&self) -> bool {
        !self.skill_studio
    }
}

#[derive(serde::Serialize)]
struct BrowsersOnDisk {
    #[serde(skip_serializing_if = "Option::is_none")]
    per_project: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_this_device: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<&'static str>,
}

impl BrowsersOnDisk {
    fn is_unset(&self) -> bool {
        self.per_project.is_none()
            && self.on_this_device.is_none()
            && self.idle_minutes.is_none()
            && self.engine.is_none()
    }
}

#[derive(serde::Serialize)]
struct DeviceOnDisk {
    id: String,
    created: u64,
}

#[derive(serde::Serialize)]
struct RecentOnDisk {
    plane: String,
    opened: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust: Option<TrustOnDisk>,
    /// Left out when it is false, and the empty set left out when it is empty, so a store
    /// with nothing pinned is byte for byte the store charter wrote before pins existed. An
    /// older charter reading this file ignores both fields; this one reads their absence as
    /// "nothing was pinned", which is what was true.
    #[serde(skip_serializing_if = "is_false")]
    pinned: bool,
    #[serde(rename = "pinnedWorkspaces", skip_serializing_if = "Vec::is_empty")]
    pinned_workspaces: Vec<String>,
    #[serde(rename = "mostActivePinned", skip_serializing_if = "is_false")]
    most_active_pinned: bool,
    /// Left out while every workspace runs its chats on this machine (#2679).
    #[serde(rename = "chatsRunOn", skip_serializing_if = "BTreeMap::is_empty")]
    chats_run_on: BTreeMap<String, String>,
}

#[derive(serde::Serialize)]
struct TrustOnDisk {
    approved: u64,
    plugins: BTreeMap<String, String>,
    env: BTreeMap<String, String>,
    starts: BTreeMap<String, String>,
    profiles: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    grants: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    fonts: BTreeMap<String, String>,
}

/// Whether a flag is at its default, so the file leaves it out. Spelled here rather than as
/// `std::ops::Not::not`, which serde resolves through the reference impl and reads as a puzzle.
fn is_false(value: &bool) -> bool {
    !*value
}

/// Whether the channel is the one an absent field already means.
fn is_the_default_channel(value: &&'static str) -> bool {
    *value == crate::updates::Channel::default().name()
}

#[derive(serde::Serialize)]
struct WindowOnDisk {
    planes: Vec<String>,
    active: usize,
}

impl From<&Store> for OnDisk {
    fn from(store: &Store) -> Self {
        Self {
            version: VERSION,
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default(),
            channel: store.channel.name(),
            device: store.device.as_ref().map(|device| DeviceOnDisk {
                id: device.id.clone(),
                created: device.created,
            }),
            weekly: store.weekly.map(|week| week.to_string()),
            browsers: BrowsersOnDisk {
                per_project: store.browsers.per_project,
                on_this_device: store.browsers.on_this_device,
                idle_minutes: store.browsers.idle_minutes,
                engine: store.browsers.machine_chrome.then_some(MACHINE_CHROME),
            },
            flags: FlagsOnDisk {
                skill_studio: store.flags.skill_studio,
            },
            recents: store
                .recents
                .iter()
                // **Both bounds, counted apart** — `Room`, the same rule `Store::trim` and
                // `load` keep. A `take(MOST_RECENTS)` here would drop a pinned project that
                // the other two deliberately kept.
                .scan(Room::default(), |room, entry| {
                    Some(room.fits(entry.pinned).then_some(entry))
                })
                .flatten()
                // `to_str` and never `display`, which SUBSTITUTES for a byte that is not
                // UTF-8. JSON holds a string, so a path that is not one cannot be written
                // here honestly — and writing the lossy rendering would remember a path that
                // is not the one that was opened, and later open it.
                .filter_map(|entry| {
                    Some(RecentOnDisk {
                        plane: entry.plane.to_str()?.to_owned(),
                        opened: entry.opened,
                        pinned: entry.pinned,
                        pinned_workspaces: entry
                            .pinned_workspaces
                            .iter()
                            .take(MOST_PINNED_WORKSPACES)
                            .cloned()
                            .collect(),
                        most_active_pinned: entry.most_active_pinned,
                        chats_run_on: entry
                            .chats_run_on
                            .iter()
                            .take(MOST_CHATS_RUN_ON)
                            .map(|(workspace, pool)| (workspace.clone(), pool.clone()))
                            .collect(),
                        trust: entry.trust.as_ref().map(|trust| TrustOnDisk {
                            approved: trust.approved,
                            plugins: trust.contributed.plugins.clone(),
                            env: trust.contributed.env.clone(),
                            starts: trust.contributed.starts.clone(),
                            profiles: trust.contributed.profiles.clone(),
                            grants: trust.contributed.grants.clone(),
                            fonts: trust.contributed.fonts.clone(),
                        }),
                    })
                })
                .collect(),
            windows: store
                .windows
                .iter()
                .map(|window| WindowOnDisk {
                    planes: window
                        .planes
                        .iter()
                        .filter_map(|plane| Some(plane.to_str()?.to_owned()))
                        .collect(),
                    active: window.active,
                })
                .collect(),
        }
    }
}

/// The store keeps nothing off unix, so its tests are unix's. What the other platforms do
/// instead is one refusal, and [`supported`] is where it is said.
///
/// Two attributes rather than `cfg(all(test, unix))`, because cargo-mutants recognises test
/// code by a bare `#[cfg(test)]`: under the combined form it mutated this module's own
/// helpers and reported a changed fixture as untested production code.
#[cfg(test)]
#[cfg(unix)]
mod tests;
