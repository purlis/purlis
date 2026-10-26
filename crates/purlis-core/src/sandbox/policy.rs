// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Policy** (#1343; ADR 0067 §1 and §4, ruling C9): an administrator's locks on what a
//! project's sandbox and the people working in it may widen. The strictest value wins: a lock
//! only ever takes away from what the project, you, a persona or a chat's grant would allow.
//!
//! # Where it is read from
//!
//! **This machine's policy file, [`MACHINE_FILE`]**, and nothing else yet. ADR 0067 §4 names
//! C9's other layers (an MDM profile, a Windows policy key, the organisation's policy on a
//! server) without saying where any of them lives, so they are not read (D-1343-1).
//!
//! **Only a file a chat cannot have written is read** ([`judge`]): a regular file, never a
//! link, owned by the system's administrator (root), in a folder owned by root, neither of
//! them writable by anyone else by its permission bits.
//!
//! **An access-control list is not read** (D-1423-5). On Linux one cannot hide from the bits:
//! the group bits of a file that carries a POSIX ACL are its mask, the most any named user or
//! group is given, so an ACL that grants write shows as group-writable and is refused. On macOS
//! an ACL is kept apart from the bits and is read only through `acl_get_link_np`, a C call
//! this crate cannot make without `unsafe`, so a file or folder whose ACL grants someone else
//! write passes. Only root can set one on a file root owns, so it is an administrator's
//! misconfiguration and never a chat's way in; `ls -le /etc/purlis` shows one. The folders
//! above the policy's own are not judged either: they are root's on every system purlis reads
//! a policy on, as for `sudoers`.
//!
//! Its folder is denied to every chat's writes as well
//! ([`super::Denied::of`], [`super::Class::HumanPowers`]). A file that is there and fails any
//! of that, a folder that is there and fails it with or without the file in it ([`judge_folder`],
//! D-1343-11), a file that does not parse, or one that says anything this version does not
//! know, is **refused, and refused closed** (D-1343-3): every lock but the presets is set,
//! because purlis cannot tell what the administrator meant, and the window says why and names
//! the file.
//!
//! # What it can lock
//!
//! ```json
//! {
//!   "owner": "Platform team <platform@example.com>",
//!   "sandbox": {
//!     "presets": ["model-providers", "forge"],
//!     "hosts": ["*.corp.example.com", "api.example.com"],
//!     "personal-hosts": false,
//!     "persona-hosts": false,
//!     "persona-tools": false,
//!     "opt-out": false,
//!     "write-grants": false,
//!     "vault-grants": false,
//!     "live-asks": false,
//!     "socket-grants": false,
//!     "proposed-changes": "off",
//!     "allow-scopes": ["you", "machine"],
//!     "never-hosts": ["*.paste.example"],
//!     "relayed-internal": ["10.96.0.1:443", "*.svc.cluster.local"]
//!   },
//!   "dispatch": {
//!     "running-per-chat": 4,
//!     "depth": 2,
//!     "may-run-at-once": 3,
//!     "allow": false,
//!     "locked": [{ "from": "steward", "to": "devops" }]
//!   }
//! }
//! ```
//!
//! - `owner`: who set it, as Settings and a Notice name them. Absent, it is "this machine's
//!   administrator".
//! - `presets`: the Internet access presets a project may turn on; any other is off.
//! - `hosts`: the hosts a project (its own and its forges'), you, a persona or a chat's grant
//!   may add; any other is refused. A preset's fixed list is the `presets` lock's. A `*.domain` entry allows every name under it; an entry without a port allows
//!   every port.
//! - `personal-hosts`, `persona-hosts`: `false` forbids your own hosts on this machine (and a
//!   block's Allow for this chat or for you), and a persona's own hosts.
//! - `persona-tools`: `false` forbids a persona's tools (`[sandbox.personas.<p>] tools`,
//!   [`super::tools`]): no chat gets a tool's shim, so a plain `kubectl` there is the program
//!   itself and never runs through `purlis secret exec`. A project file that adds one is
//!   refused when it is saved. It takes nothing else away: `purlis secret exec` typed out in
//!   full is not a persona tool and stays as it is.
//! - `opt-out`: `false` forbids a person's opt-out, and so **requires the sandbox** (D-1423-1,
//!   the operator's ruling of 2026-10-07): no chat on this machine starts without the sandbox,
//!   from the new-chat picker or a block's Notice, in any project. A project with no `[sandbox]`,
//!   or one that has not turned it on, runs every chat sandboxed here as if it had, with the
//!   default presets ([`super::Plane::in_force`]); `presets` and `hosts` hold it like any other.
//!   On a system purlis has no sandbox backend for, no harness chat starts at all
//!   ([`super::NotStarted::RequiredWithoutBackend`]).
//! - `write-grants`: `false` forbids every folder a block's Allow or Settings would let a chat
//!   write.
//! - `vault-grants`: `false` forbids letting a persona's chats use a vault the vault registry
//!   does not tag for it (a refused vault's Allow, #1430), and takes away any such grant
//!   already made: only the registry's tags open a vault to a chat.
//! - `live-asks`: `false` turns off the live ask (#1666): a connection to a host nothing lists
//!   is refused at once, as before purlis asked while it waited, and the Notice says policy
//!   turned asking off.
//! - `socket-grants`: `false` forbids every local socket and system service a block's Allow
//!   would let a chat reach (D-sock-1), and takes away any already made, the certificate check
//!   among them: also where the project's `certificate-checks` turns it on.
//! - `proposed-changes`: `"off"` turns proposed changes off (#1975, PC-14): a chat's change to
//!   purlis's own configuration is refused as before, with the way that works, and never shown
//!   to the person to apply. `"on"` is the same as absent.
//! - `allow-scopes`: the scopes an Allow may keep anything at, of `once` (this chat's next run
//!   alone: a local socket or a system service only), `chat` (only this chat), `you` (this
//!   project on this machine), `machine` (this machine, for every project: a local socket or a
//!   system service only, the operator's ruling C3 of 2026-10-13) and `project` (everyone in
//!   the project); any other is removed from every Notice and refused. **Absent, every scope
//!   stands.** A policy that lists scopes and leaves one out removes it, `once` and `machine`
//!   like any other: no word stands in for another, so a list without `once` removes "Allow
//!   once" too, and the Notice says the policy did ([`Locks::scopes_removed`]). **What was kept before the policy arrived is judged against it at every
//!   start** (#1927): a folder, a socket or a service kept at a scope the list leaves out, and
//!   anything one chat holds at one, reaches no chat while the policy stands
//!   ([`Locks::holds_kept`]). It stays on this machine's record, listed as held, with Remove.
//!   A host an Allow kept for you or for the project is held the same way, by this machine's
//!   record of what an Allow kept; a line someone wrote in the file stays the file's, under
//!   `personal-hosts`, `hosts` and `never-hosts`.
//! - `never-hosts`: hosts no level may ever add and no Allow may ever keep, written as `hosts`
//!   is; a chat's connection to one is refused and never asked about. A preset's fixed list is
//!   still the `presets` lock's.
//! - `relayed-internal`: on a runner, the internal services a chat the desktop planned may
//!   reach (#2445, ruled 2026-10-18): an address of a private network, a cluster's or a link's,
//!   or of the runner itself, is carried for such a chat only where this list names it, written
//!   as `hosts` is or as a range, `10.96.0.0/12` or `[fd00:10:96::/112]`, with an optional
//!   `:port` (#2530). Absent, such a chat reaches none. An entry that is neither is not taken,
//!   and is said in a sentence of its own ([`Locks::relayed_internal_refused`]); the rest of
//!   the file stands. A name or a `*.domain` entry trusts DNS:
//!   it carries whatever internal address the name resolves to, so a resolver someone else can
//!   change widens it. A cloud metadata service is never carried. It gives nothing to a chat
//!   this machine plans itself, and `hosts` gives nothing to a relayed one.
//! - `dispatch.allow`: `false` forbids one chat dispatching to another at all (#1437), a chat's
//!   own persona included.
//! - `dispatch.locked`: the pairs no chat may dispatch across, each a `from` persona and a `to`
//!   persona. No grant covers a locked pair, one already made included, and its Notice offers
//!   no Allow.
//!
//! - `dispatch`: a **ceiling** on each dispatch limit it names (#1440,
//!   [`crate::dispatchlimits`]): no project, workspace, persona or person's own setting gives
//!   more. `may-dispatch` and `may-run-at-once` cap every persona. 0 switches dispatch off on
//!   this machine, and nothing lifts it. A file that is refused switches dispatch off, and is
//!   said as refused ([`crate::dispatchlimits::Source::PolicyRefused`]).
//!
//! Absent keys lock nothing. `true` is the same as absent.

use std::path::{Path, PathBuf};

use super::Preset;
use super::grant;
use super::hosts::{Granted, Host, Level};
use super::internal::Entry;

/// **This machine's policy file**: in a system folder only an administrator writes.
pub const MACHINE_FILE: &str = "/etc/purlis/policy.json";

/// The most of a policy file read; a larger one is refused.
const MOST_BYTES: u64 = 64 * 1024;

/// The longest `owner` kept, in characters.
const MOST_OWNER_CHARS: usize = 120;

/// The longest sentence kept for why an entry is not taken, in characters.
const MOST_REFUSAL_CHARS: usize = 240;

/// Who an administrator is called where the policy names nobody.
const NOBODY_NAMED: &str = "this machine's administrator";

/// Where a policy came from, and who set it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    file: PathBuf,
    /// The policy's `owner`, one line.
    owner: Option<String>,
    /// Why the file was refused, where it was: every lock is then set.
    refused: Option<String>,
}

/// **An administrator's locks** (ADR 0067 §1 and §4): asked of every preset, host, grant and
/// opt-out before it is allowed, and the strictest answer wins. [`Locks::none`] where there is
/// no policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Locks {
    source: Option<Source>,
    /// The presets a project may turn on, where the policy fixes them.
    presets: Option<Vec<Preset>>,
    /// The hosts any level may add, where the policy fixes them.
    hosts: Option<Vec<Host>>,
    no_personal_hosts: bool,
    no_persona_hosts: bool,
    /// Whether a persona's tools are forbidden (#1407).
    no_persona_tools: bool,
    no_opt_out: bool,
    no_write_grants: bool,
    no_vault_grants: bool,
    /// Whether the live ask is turned off (#1666).
    no_live_asks: bool,
    /// Whether every socket and service grant is forbidden, the certificate check's included
    /// (D-sock-1).
    no_socket_grants: bool,
    /// Whether proposed changes are turned off (#1975, PC-14): a chat's edit of purlis's own
    /// configuration is then refused as before, never shown to the person to apply.
    no_proposed_changes: bool,
    /// The scopes an Allow may keep anything at, where the policy fixes them (#1666): `machine`
    /// among them only where the policy names it (ruling C3).
    allow_scopes: Option<Vec<grant::Level>>,
    /// The hosts no level may add and no Allow may keep (#1666).
    never_hosts: Vec<Host>,
    /// The internal services a chat planned on another machine reaches on this one (#2445).
    relayed_internal: Vec<Entry>,
    /// Each entry of `relayed-internal` that is not taken, and why (#2530).
    relayed_internal_refused: Vec<String>,
    /// The most each dispatch limit may be, where the policy says (#1440).
    dispatch: crate::dispatchlimits::Level,
    /// Whether every dispatch from one chat to another is forbidden (#1437).
    no_dispatch: bool,
    /// The pairs no chat may dispatch across: asking persona, target persona.
    dispatch_locked: Vec<(String, String)>,
    /// The project's own `[sandbox] policy = "never"` (#2071 PR C): nobody runs below the full
    /// sandbox there, as where this machine's policy forbids the opt-out, and said as the
    /// project's.
    team_never: bool,
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    /// A test's seam: the locks [`Locks::of`] answers, in place of this machine's file, which a
    /// test can neither write nor rely on. Only a test build has it (`test-support` is turned on
    /// from `[dev-dependencies]` alone), so no build anyone is given reads anything but the file.
    pub(crate) static TEST_LOCKS: std::cell::RefCell<Locks> =
        std::cell::RefCell::new(Locks::none());
}

/// **A test's locks** for this thread, in place of this machine's file: what [`Locks::of`]
/// answers until another call. Only in a test build.
#[cfg(any(test, feature = "test-support"))]
pub fn set_for_this_test(locks: Locks) {
    TEST_LOCKS.with(|held| *held.borrow_mut() = locks);
}

impl Locks {
    /// No locks: no policy.
    pub fn none() -> Self {
        Self::default()
    }

    /// **The locks in force for the project at `root` on this machine**: this machine's policy
    /// file's ([`MACHINE_FILE`]). No project setting moves where it is read from, and nothing
    /// in the environment does either: one a chat could set would be one a chat could point at
    /// a file of its own.
    ///
    /// With the project's own `[sandbox] policy = "never"` beside them (#2071 PR C), which
    /// requires the full sandbox as [`Self::forbids_opt_out`] does, said as the project's.
    pub fn of(root: &Path) -> Self {
        #[cfg(any(test, feature = "test-support"))]
        let locks = TEST_LOCKS.with(|locks| locks.borrow().clone());
        #[cfg(not(any(test, feature = "test-support")))]
        let locks = Self::read(Path::new(MACHINE_FILE));
        let held = Self {
            team_never: super::team::Team::of(root).policy == super::team::Policy::Never,
            ..locks
        };
        // A grade's chat keeps no Allow of any scope (SK-16, [`holding_no_kept_grants`]).
        if HOLDING_NO_KEPT_GRANTS.get() {
            return Self {
                allow_scopes: Some(Vec::new()),
                ..held
            };
        }
        held
    }

    /// The policy file at `path`: none where there is no file, its locks where it is one purlis
    /// trusts and reads, and every lock set where it is there and refused.
    pub fn read(path: &Path) -> Self {
        match read_trusted(path) {
            Ok(None) => Self::none(),
            Ok(Some(text)) => Self::parse(&text, path),
            Err(why) => Self::refused(path, &why),
        }
    }

    /// The policy `text`, read from `file`: its locks, or every lock where it says anything
    /// this version does not take.
    pub fn parse(text: &str, file: &Path) -> Self {
        parsed(text, file).unwrap_or_else(|why| Self::refused(file, &why))
    }

    /// Every lock set, for a file refused for `why`: purlis cannot tell what it meant. The
    /// presets stay the project's (D-1343-3): fixing them to none would leave no chat a way to
    /// its model, and whoever could make the file untrusted is an administrator already.
    fn refused(file: &Path, why: &str) -> Self {
        Self {
            source: Some(Source {
                file: file.to_path_buf(),
                owner: None,
                refused: Some(why.to_owned()),
            }),
            presets: None,
            hosts: Some(Vec::new()),
            no_personal_hosts: true,
            no_persona_hosts: true,
            no_persona_tools: true,
            no_opt_out: true,
            no_write_grants: true,
            no_vault_grants: true,
            no_live_asks: true,
            no_socket_grants: true,
            no_proposed_changes: true,
            allow_scopes: Some(Vec::new()),
            never_hosts: Vec::new(),
            relayed_internal: Vec::new(),
            relayed_internal_refused: Vec::new(),
            dispatch: crate::dispatchlimits::ceiling_when_refused(),
            no_dispatch: true,
            dispatch_locked: Vec::new(),
            team_never: false,
        }
    }

    /// Whether any policy is in force.
    pub fn any(&self) -> bool {
        self.source.is_some()
    }

    /// Who set the policy: its `owner`, or this machine's administrator.
    pub fn owner(&self) -> String {
        self.source
            .as_ref()
            .and_then(|source| source.owner.clone())
            .unwrap_or_else(|| NOBODY_NAMED.to_owned())
    }

    /// The file the policy was read from.
    pub fn file(&self) -> Option<&Path> {
        self.source.as_ref().map(|source| source.file.as_path())
    }

    /// Why the policy file was refused, where it was.
    pub fn refused_because(&self) -> Option<&str> {
        self.source
            .as_ref()
            .and_then(|source| source.refused.as_deref())
    }

    /// `lead`, then who set the policy and in which file; or, for a refused file, why it is
    /// refused and that everything it could lock is locked.
    pub fn led_by(&self, lead: &str) -> String {
        let file = self.file().map_or_else(
            || MACHINE_FILE.to_owned(),
            |file| file.display().to_string(),
        );
        match self.refused_because() {
            Some(why) => format!(
                "{lead}: this machine's policy file, {file}, is refused ({why}), so everything \
                 it could lock is locked until {NOBODY_NAMED} fixes it."
            ),
            None => format!("{lead}, set by {} in {file}.", self.owner()),
        }
    }

    /// **"Locked by policy", and who set it**: the words every locked value is shown with.
    pub fn locked_by(&self) -> String {
        self.led_by("Locked by policy")
    }

    /// **"On, required by policy", and who set it**: what Settings says of the mode where
    /// policy requires the sandbox ([`Self::forbids_opt_out`]); none where it does not.
    pub fn required_by(&self) -> Option<String> {
        if self.no_opt_out {
            return Some(self.led_by("On, required by policy"));
        }
        self.team_never.then(|| {
            format!(
                "On, required by the project's policy ({}.{} = \"never\" in {}).",
                super::TABLE,
                super::team::KEY,
                super::FILE
            )
        })
    }

    /// The presets of `asked` the policy lets a project turn on, in `asked`'s order.
    pub fn presets(&self, asked: &[Preset]) -> Vec<Preset> {
        asked
            .iter()
            .copied()
            .filter(|preset| self.allows_preset(*preset))
            .collect()
    }

    /// Whether the policy lets a project turn `preset` on.
    pub fn allows_preset(&self, preset: Preset) -> bool {
        self.presets
            .as_ref()
            .is_none_or(|allowed| allowed.contains(&preset))
    }

    /// Whether the policy fixes the presets.
    pub fn fixes_presets(&self) -> bool {
        self.presets.is_some()
    }

    /// The hosts the policy lets any level add, where it fixes them.
    pub fn allowed_hosts(&self) -> Option<&[Host]> {
        self.hosts.as_deref()
    }

    /// Whether policy forbids your own hosts on this machine.
    pub fn forbids_personal_hosts(&self) -> bool {
        self.no_personal_hosts
    }

    /// Whether policy forbids a persona's own hosts.
    pub fn forbids_persona_hosts(&self) -> bool {
        self.no_persona_hosts
    }

    /// Whether policy forbids a persona's tools (#1407): a refused file does too.
    pub fn forbids_persona_tools(&self) -> bool {
        self.no_persona_tools
    }

    /// **Why `persona`'s tool `tool` is locked out**, if it is (#1407): policy forbids every
    /// persona's tools. The sentence names the tool, the persona, the policy and who set it.
    pub fn refuses_tool(&self, persona: &str, tool: &str) -> Option<String> {
        self.no_persona_tools.then(|| {
            format!(
                "{tool} is a tool of {persona}'s, and policy forbids a persona's tools. {}",
                self.locked_by()
            )
        })
    }

    /// Whether policy forbids starting a chat without the sandbox, **and so requires the
    /// sandbox** (D-1423-1): in every project on this machine, whatever the project's own file
    /// says ([`super::Plane::in_force`]).
    pub fn forbids_opt_out(&self) -> bool {
        self.no_opt_out || self.team_never
    }

    /// **The most each dispatch limit may be** (#1440): a ceiling over the project's, a
    /// workspace's, a persona's and your own. A limit it does not name is not capped.
    pub fn dispatch_ceiling(&self) -> &crate::dispatchlimits::Level {
        &self.dispatch
    }

    /// Whether policy forbids every folder a grant would let a chat write.
    pub fn forbids_write_grants(&self) -> bool {
        self.no_write_grants
    }

    /// Whether policy turns the live ask off (#1666): a connection to a host nothing lists is
    /// refused at once.
    pub fn forbids_live_asks(&self) -> bool {
        self.no_live_asks
    }

    /// Why a connection is not held while the person is asked, where policy turns that off: the
    /// sentence a Notice says (#1666).
    pub fn live_asks_refused(&self) -> Option<String> {
        self.no_live_asks.then(|| {
            format!(
                "Policy turns off asking while a connection waits, so the command was refused \
                 and an Allow reaches the chat once it restarts. {}",
                self.locked_by()
            )
        })
    }

    /// **Why `host` can never be allowed**, where policy pins it so (#1666).
    pub fn never_allows(&self, host: &Host) -> Option<String> {
        self.never_hosts
            .iter()
            .any(|pinned| pinned.covers(host))
            .then(|| format!("Policy never allows {host}. {}", self.locked_by()))
    }

    /// **The internal services a chat planned on another machine reaches on this one** (#2445,
    /// ruled 2026-10-18): the `relayed-internal` list, and nothing else of the policy's.
    pub fn relayed_internal(&self) -> &[Entry] {
        &self.relayed_internal
    }

    /// **Each entry of `relayed-internal` not taken** (#2530): the entry, as the file wrote it,
    /// and why, on one line. It carries nothing, and nothing else is locked for it.
    pub fn relayed_internal_refused(&self) -> &[String] {
        &self.relayed_internal_refused
    }

    /// The hosts policy pins as never allowed (#1666).
    pub fn never_hosts(&self) -> &[Host] {
        &self.never_hosts
    }

    /// **Why `granted` is locked out**, if it is: a host it pins as never allowed, a level
    /// policy forbids, or a host it does not allow.
    pub fn refuses(&self, granted: &Granted) -> Option<String> {
        let host = &granted.host;
        if let Some(never) = self.never_allows(host) {
            return Some(never);
        }
        let why = match granted.level {
            Level::Persona if self.no_persona_hosts => Some(format!(
                "{host} is a persona's host, and policy forbids a persona's own hosts."
            )),
            Level::You if self.no_personal_hosts => Some(format!(
                "{host} is a host of yours, and policy forbids hosts of your own."
            )),
            Level::Project | Level::You | Level::Persona => None,
        }
        .or_else(|| {
            self.hosts
                .as_ref()
                .filter(|allowed| !allowed.iter().any(|one| one.covers(host)))
                .map(|_| format!("{host} is not a host policy allows."))
        })?;
        Some(format!("{why} {}", self.locked_by()))
    }

    /// **Why a grant of `what` at `level` is locked out**, if it is: a host as
    /// [`Self::refuses`] judges it at the level it would be kept at, and a folder where policy
    /// forbids write grants.
    pub fn refuses_grant(&self, what: &grant::What, level: grant::Level) -> Option<String> {
        if let grant::What::Host(host) = what
            && let Some(never) = self.never_allows(host)
        {
            return Some(never);
        }
        if let Some(removed) = self.scope_removed(level) {
            return Some(removed);
        }
        // Every project on this machine is a socket's or a service's alone (R2).
        if level == grant::Level::Machine && !what.machine_wide() {
            return Some(
                "purlis allows only a local socket or a system service for every project on \
                 this machine: a host and a folder are a project's."
                    .to_owned(),
            );
        }
        match what {
            grant::What::Host(host) => self.refuses(&Granted {
                host: host.clone(),
                level: level.hosts_level(),
            }),
            grant::What::Write(_) => self.write_grants_refused(),
            grant::What::Socket(_) | grant::What::Service(_) => self.socket_grants_refused(),
            grant::What::Port(_) => self.port_grants_refused(),
        }
    }

    /// **Whether an Allow may keep anything at `level`**: every scope where the policy fixes
    /// none, and otherwise only the ones it lists. "On this machine, for every project"
    /// ([`grant::Level::Machine`]) is listed by its own word, `machine` (ruling C3), and is
    /// left out where the list leaves it out, as `you` is.
    pub fn keeps_scope(&self, level: grant::Level) -> bool {
        self.allow_scopes
            .as_ref()
            .is_none_or(|scopes| scopes.contains(&level))
    }

    /// **Why no Allow is kept at `level`**, where the policy's `allow-scopes` leaves it out
    /// ([`Self::keeps_scope`]): the sentence a Notice says, whatever the Allow would name.
    pub fn scope_removed(&self, level: grant::Level) -> Option<String> {
        (!self.keeps_scope(level)).then(|| {
            format!(
                "Policy removed Allow {}. {}",
                level.said(),
                self.locked_by()
            )
        })
    }

    /// **Why what was kept at `level` reaches no chat now**, where it does not (#1927): the
    /// lock on its kind, or a scope `allow-scopes` leaves out. What a person allowed before a
    /// policy arrived is judged against the policy as it stands at every start
    /// ([`super::Compiled::granted`]): one it holds contributes nothing, stays on this
    /// machine's record, and is listed as held, with Remove.
    ///
    /// **A host kept for you or for the project is judged apart** ([`super::host_held`]): its
    /// list is one Settings edits too, so only a host this machine's record says an Allow
    /// kept is held by a scope, and a line someone wrote stays under `personal-hosts`, `hosts`
    /// and `never-hosts` ([`Self::refuses`]). This answers none for such a host. A host one
    /// chat holds is an Allow's alone, and is held like anything else one chat holds.
    pub fn holds_kept(&self, what: &grant::What, level: grant::Level) -> Option<String> {
        let locked = match what {
            grant::What::Write(_) => self.write_grants_refused(),
            grant::What::Socket(_) | grant::What::Service(_) => self.socket_grants_refused(),
            grant::What::Port(_) => self.port_grants_refused(),
            grant::What::Host(_) => None,
        };
        let a_list_of_hosts = matches!(what, grant::What::Host(_))
            && !matches!(level, grant::Level::Once | grant::Level::Chat);
        locked.or_else(|| (!a_list_of_hosts).then(|| self.scope_held(level)).flatten())
    }

    /// **What a Notice says of the scopes among `offered` the policy removed**, each named,
    /// with who set it; none where it removed none of them. A scope a policy removes is never
    /// only missing from a Notice: "Allow once" is the first answer a blocked socket's Notice
    /// offers, and a list without `once` removes it, so the Notice says the policy did.
    pub fn scopes_removed(&self, offered: &[grant::Level]) -> Option<String> {
        let removed: Vec<String> = offered
            .iter()
            .filter(|level| !self.keeps_scope(**level))
            .map(|level| format!("Allow {}", level.said()))
            .collect();
        (!removed.is_empty()).then(|| {
            format!(
                "Policy removed {}. {}",
                removed.join("; "),
                self.locked_by()
            )
        })
    }

    /// **Why what an Allow kept at `level` reaches no chat**, where `allow-scopes` leaves that
    /// scope out: the sentence the Allowed list and Doctor say, with who set the policy.
    pub fn scope_held(&self, level: grant::Level) -> Option<String> {
        (!self.keeps_scope(level)).then(|| {
            format!(
                "Held by policy, which leaves out what was allowed {}. {}",
                level.said(),
                self.locked_by()
            )
        })
    }

    /// Why no folder may be granted, where policy forbids it.
    pub fn write_grants_refused(&self) -> Option<String> {
        self.no_write_grants.then(|| {
            format!(
                "Policy forbids allowing a chat to write a folder. {}",
                self.locked_by()
            )
        })
    }

    /// Whether policy forbids every socket and service grant (D-sock-1).
    pub fn forbids_socket_grants(&self) -> bool {
        self.no_socket_grants
    }

    /// Why no socket or service may be granted, where policy forbids it.
    pub fn socket_grants_refused(&self) -> Option<String> {
        self.no_socket_grants.then(|| {
            format!(
                "Policy forbids allowing a chat a local socket or a system service. {}",
                self.locked_by()
            )
        })
    }

    /// Why no service on this Mac may be allowed by its port (qw384), where policy forbids it:
    /// the lock that forbids socket and service grants holds ports too, since what listens on
    /// this machine's loopback is as much this machine's as a socket is.
    pub fn port_grants_refused(&self) -> Option<String> {
        self.no_socket_grants.then(|| {
            format!(
                "Policy forbids allowing a chat a service on this Mac. {}",
                self.locked_by()
            )
        })
    }

    /// Why no chat's change to purlis's own configuration is shown to the person to apply,
    /// where policy turns proposed changes off (#1975, PC-14): the chat is then refused as
    /// before.
    pub fn proposed_changes_refused(&self) -> Option<String> {
        self.no_proposed_changes.then(|| {
            format!(
                "Policy turns proposed changes off, so a chat's change to purlis's own \
                 configuration is not shown to you to apply. {}",
                self.locked_by()
            )
        })
    }

    /// Whether policy forbids letting a persona's chats use a vault it is not tagged for.
    pub fn forbids_vault_grants(&self) -> bool {
        self.no_vault_grants
    }

    /// Why no persona may be allowed a vault the registry does not tag for it, where policy
    /// forbids it (#1430).
    pub fn vault_grants_refused(&self) -> Option<String> {
        self.no_vault_grants.then(|| {
            format!(
                "Policy forbids allowing a persona a vault it is not tagged for. {}",
                self.locked_by()
            )
        })
    }

    /// Whether policy forbids every dispatch from one chat to another (#1437).
    pub fn forbids_dispatch(&self) -> bool {
        self.no_dispatch
    }

    /// The pairs policy lets no chat dispatch across: asking persona, target persona.
    pub fn locked_pairs(&self) -> &[(String, String)] {
        &self.dispatch_locked
    }

    /// **Why a chat running as `asking` may not dispatch to `target`**, where policy says so
    /// (#1437): every dispatch is forbidden, or this pair is locked. The sentence names the
    /// policy and who set it. `asking` is `None` for a chat on no persona, which no pair names.
    pub fn dispatch_refused(&self, asking: Option<&str>, target: &str) -> Option<String> {
        let why = if self.no_dispatch {
            "Policy forbids one chat dispatching to another.".to_owned()
        } else {
            let asking = asking?;
            self.dispatch_locked
                .iter()
                .any(|(from, to)| from == asking && to == target)
                .then(|| format!("Policy forbids {asking} chats dispatching to {target}."))?
        };
        Some(format!("{why} {}", self.locked_by()))
    }

    /// Why no chat may start without the sandbox, where policy forbids it.
    pub fn opt_out_refused(&self) -> Option<String> {
        if self.no_opt_out {
            return Some(format!(
                "Policy forbids starting a chat without the sandbox. {}",
                self.locked_by()
            ));
        }
        self.team_never.then(super::team::never_said)
    }
}

/// The keys `sandbox` may hold, each a lock.
const KEYS: [&str; 14] = [
    "presets",
    "hosts",
    "personal-hosts",
    "persona-hosts",
    "persona-tools",
    "opt-out",
    "write-grants",
    "vault-grants",
    "live-asks",
    "socket-grants",
    "proposed-changes",
    "allow-scopes",
    "never-hosts",
    "relayed-internal",
];

/// `text` as a policy read from `file`, or why it is refused.
fn parsed(text: &str, file: &Path) -> Result<Locks, String> {
    let top: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "it is not JSON".to_owned())?;
    let top = top
        .as_object()
        .ok_or_else(|| "it is not a JSON object".to_owned())?;
    if let Some(key) = top
        .keys()
        .find(|key| !["owner", "sandbox", "dispatch"].contains(&key.as_str()))
    {
        return Err(format!(
            "it says \"{}\", which purlis does not know",
            clip(key)
        ));
    }
    let owner = match top.get("owner") {
        None => None,
        Some(serde_json::Value::String(owner)) => {
            Some(crate::shown::one_line(owner.trim(), MOST_OWNER_CHARS)).filter(|o| !o.is_empty())
        }
        Some(_) => return Err("its \"owner\" is not text".to_owned()),
    };
    let empty = serde_json::Map::new();
    let sandbox = match top.get("sandbox") {
        None => &empty,
        Some(serde_json::Value::Object(sandbox)) => sandbox,
        Some(_) => return Err("its \"sandbox\" is not an object".to_owned()),
    };
    let (dispatch, no_dispatch, dispatch_locked) = dispatch(top.get("dispatch"))?;
    if let Some(key) = sandbox.keys().find(|key| !KEYS.contains(&key.as_str())) {
        return Err(format!(
            "its sandbox says \"{}\", which purlis does not know",
            clip(key)
        ));
    }
    let words = |key: &str| -> Result<Option<Vec<String>>, String> {
        match sandbox.get(key) {
            None => Ok(None),
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("its \"{key}\" holds something that is not text"))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            Some(_) => Err(format!("its \"{key}\" is not a list")),
        }
    };
    let forbids = |key: &str| -> Result<bool, String> {
        match sandbox.get(key) {
            None => Ok(false),
            Some(serde_json::Value::Bool(allowed)) => Ok(!allowed),
            Some(_) => Err(format!("its \"{key}\" is not true or false")),
        }
    };
    let presets = words("presets")?
        .map(|words| {
            words
                .iter()
                .map(|word| {
                    Preset::of_word(word).ok_or_else(|| {
                        format!(
                            "its presets name \"{}\", which is not one of {}",
                            clip(word),
                            Preset::listed()
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let hosts_of = |key: &str| -> Result<Option<Vec<Host>>, String> {
        words(key)?
            .map(|words| {
                words
                    .iter()
                    .map(|word| {
                        Host::parse(word)
                            .map_err(|why| format!("its {key} name \"{}\": {why}", clip(word)))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
    };
    // `"off"` or `"on"` (PC-14, the operator's ruling of 2026-10-14): a word, not true or
    // false, as the ruling wrote it.
    let no_proposed_changes = match sandbox.get("proposed-changes") {
        None => false,
        Some(serde_json::Value::String(word)) if word == "off" => true,
        Some(serde_json::Value::String(word)) if word == "on" => false,
        Some(_) => return Err("its \"proposed-changes\" is not \"on\" or \"off\"".to_owned()),
    };
    let hosts = hosts_of("hosts")?;
    let never_hosts = hosts_of("never-hosts")?.unwrap_or_default();
    let (relayed_internal, relayed_internal_refused) =
        relayed_internal(sandbox.get("relayed-internal"))?;
    let allow_scopes = words("allow-scopes")?
        .map(|words| {
            words
                .iter()
                .map(|word| {
                    // `machine` is "every project on this machine" (R2), a word of its own
                    // since the operator's ruling C3: listed, it is kept; left out, it is
                    // removed with every other scope the list leaves out.
                    grant::Level::of_word(word).ok_or_else(|| {
                        format!(
                            "its allow-scopes name \"{}\", which is not once, chat, you, machine \
                             or project",
                            clip(word)
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    Ok(Locks {
        source: Some(Source {
            file: file.to_path_buf(),
            owner,
            refused: None,
        }),
        presets,
        hosts,
        no_personal_hosts: forbids("personal-hosts")?,
        no_persona_hosts: forbids("persona-hosts")?,
        no_persona_tools: forbids("persona-tools")?,
        no_opt_out: forbids("opt-out")?,
        no_write_grants: forbids("write-grants")?,
        no_vault_grants: forbids("vault-grants")?,
        no_live_asks: forbids("live-asks")?,
        no_socket_grants: forbids("socket-grants")?,
        no_proposed_changes,
        allow_scopes,
        never_hosts,
        relayed_internal,
        relayed_internal_refused,
        dispatch,
        no_dispatch,
        dispatch_locked,
        team_never: false,
    })
}

/// **What a policy's `relayed-internal` lists** (#2530): each entry it takes, and a sentence for
/// each it does not, naming it. One that is not an entry is left out alone, since it could
/// only have carried more; a value that is not a list refuses the file.
fn relayed_internal(
    value: Option<&serde_json::Value>,
) -> Result<(Vec<Entry>, Vec<String>), String> {
    let items = match value {
        None => return Ok((Vec::new(), Vec::new())),
        Some(serde_json::Value::Array(items)) => items,
        Some(_) => return Err("its \"relayed-internal\" is not a list".to_owned()),
    };
    let mut taken: Vec<Entry> = Vec::new();
    let mut refused = Vec::new();
    for item in items {
        let said = |why: &str| {
            format!(
                "{}: {}",
                clip(&item.to_string()),
                crate::shown::one_line(why, MOST_REFUSAL_CHARS)
            )
        };
        match item.as_str().map(Entry::parse) {
            Some(Ok(entry)) => {
                if !taken.contains(&entry) {
                    taken.push(entry);
                }
            }
            Some(Err(why)) => refused.push(said(&why)),
            None => refused.push(said("an entry is written as text in quotes.")),
        }
    }
    Ok((taken, refused))
}

/// The keys of `dispatch` that are locks; every other key is a limit's ceiling.
const DISPATCH_LOCKS: [&str; 2] = ["allow", "locked"];

/// What one dispatch reads of a policy: the limits' ceiling, whether every dispatch is
/// forbidden, and the pairs it locks.
type Dispatch = (crate::dispatchlimits::Level, bool, Vec<(String, String)>);

/// **What a policy's `dispatch` says**, the one reader of that object: the ceiling on each
/// dispatch limit it names (#1440, [`crate::dispatchlimits::ceiling`]), whether every dispatch
/// is forbidden, and the pairs it locks (#1437); or why the file is refused. A key that is
/// neither a lock nor a limit refuses the file.
fn dispatch(value: Option<&serde_json::Value>) -> Result<Dispatch, String> {
    let said = match value {
        None => return Ok((crate::dispatchlimits::Level::unset(), false, Vec::new())),
        Some(serde_json::Value::Object(said)) => said,
        Some(_) => return Err("its \"dispatch\" is not an object".to_owned()),
    };
    let limits: serde_json::Map<String, serde_json::Value> = said
        .iter()
        .filter(|(key, _)| !DISPATCH_LOCKS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let ceiling = crate::dispatchlimits::ceiling(&limits)?;
    let forbidden = match said.get("allow") {
        None => false,
        Some(serde_json::Value::Bool(allowed)) => !allowed,
        Some(_) => return Err("its dispatch \"allow\" is not true or false".to_owned()),
    };
    let pairs = match said.get("locked") {
        None => Vec::new(),
        Some(serde_json::Value::Array(pairs)) => pairs
            .iter()
            .map(|pair| {
                let persona = |key: &str| {
                    pair.get(key)
                        .and_then(serde_json::Value::as_str)
                        .filter(|name| crate::personas::valid_name(name))
                        .map(str::to_owned)
                };
                let only_these = pair
                    .as_object()
                    .is_some_and(|pair| pair.keys().all(|key| key == "from" || key == "to"));
                match (only_these, persona("from"), persona("to")) {
                    (true, Some(from), Some(to)) => Ok((from, to)),
                    _ => Err(
                        "its dispatch \"locked\" holds something that is not a \"from\" persona \
                         and a \"to\" persona"
                            .to_owned(),
                    ),
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err("its dispatch \"locked\" is not a list".to_owned()),
    };
    Ok((ceiling, forbidden, pairs))
}

/// A word of the file as a refusal quotes it: one line, short.
fn clip(word: &str) -> String {
    crate::shown::one_line(word, 40)
}

/// What [`judge`] is told of one file or folder, without following a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    /// Whether it is a link.
    pub link: bool,
    /// Whether it is a regular file (for the file) or a directory (for its folder).
    pub expected_type: bool,
    /// Its owner's user id.
    pub uid: u32,
    /// Its permission bits.
    pub mode: u32,
}

/// **Whether a policy file, as `file` and its folder `folder` are seen, is one purlis trusts**,
/// or why not: neither a link, each the type it should be, each owned by root (user id 0), and
/// neither writable by its group or anyone else. So no one but an administrator could have
/// written it, a chat included.
pub fn judge(file: Seen, folder: Seen) -> Result<(), String> {
    judge_folder(folder)?;
    if file.link {
        return Err("it is a link".to_owned());
    }
    if !file.expected_type {
        return Err("it is not a regular file".to_owned());
    }
    if file.uid != 0 {
        return Err("it is not owned by the system's administrator".to_owned());
    }
    if file.mode & 0o022 != 0 {
        return Err("it can be written by others than the administrator".to_owned());
    }
    Ok(())
}

/// **Whether the policy's folder, as seen, is one purlis trusts**, or why not: not a link, a
/// folder, owned by root and writable by no one else. Asked whenever the folder is there, file
/// or no file: whoever could write an untrusted folder could take the file out of it, and a
/// policy taken out that way is never read as no policy (D-1343-11).
pub fn judge_folder(folder: Seen) -> Result<(), String> {
    if folder.link {
        return Err("its folder is a link".to_owned());
    }
    if !folder.expected_type {
        return Err("its folder is not a folder".to_owned());
    }
    if folder.uid != 0 {
        return Err("its folder is not owned by the system's administrator".to_owned());
    }
    if folder.mode & 0o022 != 0 {
        return Err("its folder can be written by others than the administrator".to_owned());
    }
    Ok(())
}

/// The text of the policy file at `path`, `None` where there is none, or why it is refused.
/// Neither the file nor its folder is followed through a link, the file is opened without
/// following one, and what was opened is judged again before it is read.
#[cfg(unix)]
fn read_trusted(path: &Path) -> Result<Option<String>, String> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let seen = |meta: &std::fs::Metadata, dir: bool| Seen {
        link: meta.file_type().is_symlink(),
        expected_type: if dir {
            meta.file_type().is_dir()
        } else {
            meta.file_type().is_file()
        },
        uid: meta.uid(),
        mode: meta.mode(),
    };
    // The folder first, whenever it is there: no folder is no policy, and an untrusted one is
    // refused whether or not the file is in it (D-1343-11).
    let dir = path.parent().ok_or_else(|| "it has no folder".to_owned())?;
    let folder = match std::fs::symlink_metadata(dir) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("its folder cannot be read: {err}")),
        Ok(meta) => meta,
    };
    judge_folder(seen(&folder, true))?;
    let file = match std::fs::symlink_metadata(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("it cannot be read: {err}")),
        Ok(meta) => meta,
    };
    judge(seen(&file, false), seen(&folder, true))?;
    let opened = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|err| format!("it cannot be opened: {err}"))?;
    let now = opened
        .metadata()
        .map_err(|err| format!("it cannot be read: {err}"))?;
    if now.ino() != file.ino() || now.dev() != file.dev() {
        return Err("it changed while it was read".to_owned());
    }
    judge(seen(&now, false), seen(&folder, true))?;
    let mut bytes = Vec::new();
    opened
        .take(MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("it cannot be read: {err}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MOST_BYTES {
        return Err("it is larger than purlis reads".to_owned());
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "it is not UTF-8 text".to_owned())
}

/// No policy file is read off Unix yet: a Windows policy key is one of C9's layers no ADR
/// places (D-1343-1).
#[cfg(not(unix))]
fn read_trusted(_path: &Path) -> Result<Option<String>, String> {
    Ok(None)
}

/// The folder [`MACHINE_FILE`] is in: every chat is denied writing it.
pub fn machine_folder() -> PathBuf {
    Path::new(MACHINE_FILE)
        .parent()
        .map_or_else(|| PathBuf::from(MACHINE_FILE), Path::to_path_buf)
}

thread_local! {
    /// Whether this thread is deciding a grade's chat's sandbox ([`holding_no_kept_grants`]).
    static HOLDING_NO_KEPT_GRANTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **Decides `decide` as for a grade's chat** (SK-16, the operator's ruling of 2026-10-26 on
/// #3229): no Allow of any scope is kept, so none of the person's own folders, sockets, services
/// or ports, kept for this project or for every project on this machine, reaches the chat, as an
/// administrator's `allow-scopes` of none would hold it. The chat's own grants are none. Only
/// the thread that asks is held, and only while `decide` runs.
pub fn holding_no_kept_grants<T>(decide: impl FnOnce() -> T) -> T {
    struct Held(bool);
    impl Drop for Held {
        fn drop(&mut self) {
            HOLDING_NO_KEPT_GRANTS.set(self.0);
        }
    }
    let _held = Held(HOLDING_NO_KEPT_GRANTS.replace(true));
    decide()
}

#[cfg(test)]
mod tests;
