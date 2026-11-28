// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's shared compile cache** (#6388, #3560; ruling Q324): with
//! `[sandbox] compile-cache = "sccache"` in its committed `charter.toml`, a project's sandboxed
//! chats wrap rustc in sccache, so one branch folder's compile of a crate is the next one's hit
//! instead of each building from scratch.
//!
//! **Off unless the file says so.** The key is the future policy key `sandbox.compile-cache`
//! (Q307 Q6), whose default is `"none"`: no wrapper, and nothing added to what a chat writes.
//!
//! **Only the resolved sccache, judged again at every start** ([`resolved`]). purlis never copies
//! a `rustc-wrapper` from the person's cargo config ([`super::caches::cargo_config`]). The one
//! wrapper a chat is handed is the `sccache` the person's `PATH` finds, as its real file, named
//! `sccache`, outside every folder the chat's compiled sandbox lets it write, and neither it nor
//! its folder writable by anyone but its owner, an owner that may not write it unless that is
//! root. A machine with none gets no wrapper, and its chats build as before.
//!
//! **Only the chat's sandboxed commands are pointed at it, never its harness** ([`script`]).
//! The variables go in the file Claude Code reads before each of its Bash commands
//! (`CLAUDE_ENV_FILE`), and never in the harness's own environment. A program the harness starts
//! outside its sandbox (a hook, a language server) never learns the socket, so it never starts an
//! sccache server there that the chat's commands would then reach, and through which they would
//! run a compiler outside the sandbox. Claude Code reads that file before a command the person
//! runs with `!` too, outside its sandbox, so the file hands the cache and starts the server only
//! where it finds itself inside the chat's sandbox (#7935). The server a chat reaches is one its
//! own commands started, inside its sandbox. The same file drops every other `SCCACHE_*`
//! variable the chat inherited, and points `SCCACHE_CONF` at an empty file purlis writes
//! ([`CONF`]), so a chat never reaches the person's team cache (a bucket, redis, memcached) nor a
//! distributed-compile scheduler: its cache is the local folder alone.
//!
//! **A cache folder per persona and level, in the project's cache home** ([`folder`]): written by
//! that project's sandboxed chats running as that persona at that level, and by no other chat.
//! Compiled objects are code a later build links, so a chat of a narrow persona never writes an
//! object a chat of a wider persona, or of a looser level, links. The trust boundary that stays is
//! within one folder: every chat of one project, one persona and one level shares what it builds,
//! as its package caches are shared (D-1337-6). The folders are trees of the cache home, so each
//! is made and pinned by purlis like the others, a `.git` at its top is denied, and a link planted
//! for it is taken out. They give no write anywhere else.
//!
//! **A server of the chat's own, on a socket in the chat's own temp folder** ([`socket_in`]).
//! sccache's client hands each compile to a server, which runs the compiler. The default server
//! listens on loopback port 4226, where any program of this Mac may answer, the person's own
//! unsandboxed sccache among them. A chat that reached that one would have its compiles run
//! outside its sandbox. So purlis points the client at a unix socket in the chat's own temp folder
//! (`SCCACHE_SERVER_UDS`), and lets the chat bind and connect to that one path. No loopback port is
//! opened, and no chat reaches another chat's server. The file the chat's commands read starts
//! that server itself, inside the sandbox, where none answers on the socket ([`script`], #7935):
//! sccache's own start binds a second socket the sandbox refuses.
//!
//! **Where purlis can grant it.** A Claude Code chat on macOS at the full sandbox, whose sandbox
//! grants a socket by path (`network.allowUnixSockets`) and which reads the file before each
//! command. Elsewhere the chat gets no wrapper, a declared gap: Claude Code's sandbox on Linux
//! reaches no unix socket, a wrapped harness's profile does not name the socket yet (#7062), and
//! the floor has no package caches.

use std::path::{Path, PathBuf};

use super::{Level, Machine};

/// The `[sandbox]` key that turns a project's compile cache on.
pub const KEY: &str = "compile-cache";

/// The one value [`KEY`] takes that turns anything on, and the name of the program it wraps
/// rustc in.
pub const SCCACHE: &str = "sccache";

/// The value [`KEY`] has where the file says nothing: the default.
pub const NONE: &str = "none";

/// The cache's folder in the project's cache home ([`super::caches::CacheHome`]), holding a
/// folder per level, and in each a folder per persona ([`folder`]).
pub const DIR: &str = "sccache";

/// The empty sccache config purlis writes at the top of the cache home, where no chat writes,
/// and points every chat's sccache at.
pub const CONF: &str = "sccache.conf";

/// The persona folder of a chat that runs as no persona: no persona's name begins with `_`.
pub const NO_PERSONA: &str = "_none";

/// The name of the server's socket in the chat's own temp folder.
pub const SOCKET: &str = "sccache.sock";

/// What a project's `[sandbox]` asks of its compile cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompileCache {
    /// No compile cache: the default.
    #[default]
    None,
    /// rustc wrapped in sccache, with a cache folder of the project's own.
    Sccache,
}

impl CompileCache {
    /// The value as the file writes it.
    pub fn word(self) -> &'static str {
        match self {
            Self::None => NONE,
            Self::Sccache => SCCACHE,
        }
    }

    /// What `written` asks for, or `None` where it is not a value of [`KEY`].
    pub fn of_word(written: &str) -> Option<Self> {
        [Self::None, Self::Sccache]
            .into_iter()
            .find(|one| one.word() == written)
    }
}

/// A project's compile cache, as compiled for one chat: its folder, its config, and where its
/// program is looked for at the chat's start ([`Declared::program`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The cache folder of the chat's persona at its level ([`folder`]).
    pub dir: PathBuf,
    /// The folders purlis makes between the cache home and [`Self::dir`], outermost first.
    pub above: Vec<PathBuf>,
    /// The empty config purlis writes at the top of the cache home ([`CONF`]).
    pub conf: PathBuf,
    /// Where the person's `PATH` looks for programs, as the chat was compiled.
    pub search: Vec<PathBuf>,
}

impl Declared {
    /// The compile cache of the project whose cache home is `home`, for a chat at `level` running
    /// as `persona`, on `machine`. `None` where the persona's name is not one a folder may have.
    pub fn of(home: &Path, machine: &Machine, level: Level, persona: Option<&str>) -> Option<Self> {
        let dir = folder(home, level, persona)?;
        let top = home.join(DIR);
        Some(Self {
            above: vec![top.clone(), top.join(level_word(level))],
            dir,
            conf: home.join(CONF),
            search: crate::programs::search_dirs_from(
                machine.env.get("PATH").as_deref().map(std::ffi::OsStr::new),
                machine.home.as_deref(),
            ),
        })
    }

    /// The sccache a chat that may write `writable` is handed as it starts: found again, and
    /// judged again ([`resolved`]), at every start. `None` where there is none it may be handed.
    pub fn program(&self, writable: &[PathBuf]) -> Option<PathBuf> {
        resolved(&crate::programs::find(SCCACHE, &self.search)?, writable)
    }
}

/// The level's folder name in [`DIR`].
fn level_word(level: Level) -> &'static str {
    match level {
        Level::Full => "full",
        Level::Floor => "floor",
    }
}

/// The cache folder of a chat at `level` running as `persona` in the cache home `home`:
/// `sccache/<level>/<persona>`, [`NO_PERSONA`] for a chat of none. `None` where `persona` is not
/// a name purlis could have minted ([`crate::personas::valid_name`]).
pub fn folder(home: &Path, level: Level, persona: Option<&str>) -> Option<PathBuf> {
    let persona = match persona {
        None => NO_PERSONA,
        Some(name) if crate::personas::valid_name(name) => name,
        Some(_) => return None,
    };
    Some(home.join(DIR).join(level_word(level)).join(persona))
}

/// Where a chat's sccache server listens: in the folder Claude Code roots the chat's commands'
/// `$TMPDIR` in, inside `tmp`, the chat's own temp folder (#5675). The chat writes there and no
/// other chat does, and the path stays short enough for a unix socket.
pub fn socket_in(tmp: &Path) -> PathBuf {
    tmp.join(format!("claude-{}", rustix::process::getuid().as_raw()))
        .join(SOCKET)
}

/// `found` as the wrapper a chat may be handed, or `None`. It must be absolute; its real file
/// must be a file named `sccache`; neither it nor its real file may lie in any of `writable`, the
/// folders a chat may write; and its real file and that file's folder must be owned as a
/// program's are ([`owned_as_a_program`]). A chat must never point its builds' wrapper at a
/// program of its own, nor at one it, or anyone but its owner, could change.
pub fn resolved(found: &Path, writable: &[PathBuf]) -> Option<PathBuf> {
    if !found.is_absolute() {
        return None;
    }
    let real = found.canonicalize().ok()?;
    if !real.is_file() || real.file_name()? != SCCACHE {
        return None;
    }
    let inside = |path: &Path| {
        writable.iter().any(|dir| {
            path.starts_with(dir) || dir.canonicalize().is_ok_and(|dir| path.starts_with(dir))
        })
    };
    if inside(found) || inside(&real) || !owned_as_a_program(&real) {
        return None;
    }
    Some(real)
}

/// Whether the program at `real`, a real file, is owned as one a chat may be handed: neither it
/// nor its folder writable by its group or by others, its folder owned by root or by this user,
/// and the file itself writable by nobody (as Homebrew installs one) or owned by root. One
/// `cargo install` left writable by its owner is not taken until `chmod a-w` makes it so.
#[cfg(unix)]
pub fn owned_as_a_program(real: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let me = rustix::process::getuid().as_raw();
    let Some(folder) = real.parent() else {
        return false;
    };
    let (Ok(file), Ok(folder)) = (std::fs::metadata(real), std::fs::metadata(folder)) else {
        return false;
    };
    file.mode() & 0o022 == 0
        && folder.mode() & 0o022 == 0
        && (folder.uid() == 0 || folder.uid() == me)
        && (file.uid() == 0 || (file.uid() == me && file.mode() & 0o200 == 0))
}

/// Off unix no program is owned as one: no wrapper is handed there.
#[cfg(not(unix))]
pub fn owned_as_a_program(_real: &Path) -> bool {
    false
}

/// The name of the log the chat's own server writes, beside its socket ([`script`]).
pub const LOG: &str = "sccache.log";

/// The mark a start that never answered leaves beside the socket ([`script`]): later commands
/// start the server again without waiting for it, until one answers.
pub const NOT_STARTED: &str = "sccache.not-started";

/// How many times, a tenth of a second apart, a command waits for the server it started to
/// answer on its socket ([`script`]) before it goes on without it.
pub const WAITS: u32 = 50;

/// What the file Claude Code reads before each of the chat's commands says for the compile
/// cache: every `SCCACHE_*` variable the command inherited dropped, then the wrapper `program`,
/// the cache's folder, its config and the chat's own server's `socket` set. Read by the chat's
/// sandboxed commands alone, never by its harness ([`self`]).
///
/// **Only inside the chat's sandbox** (#7935). Claude Code reads the file before a command the
/// person runs with `!` too, and runs that one outside its sandbox. A server started there would
/// run every later compile of the chat's outside the sandbox, and a chat can bring that about:
/// stop its server, and wait for the next `!`. So the file first asks whether a write to the
/// cache's config ([`Declared::conf`]) is refused with "Operation not permitted", as the chat's
/// profile refuses it and nothing outside the sandbox does (the person owns the file, and the
/// write sets its times to what they are). Outside, or where the config is not there, it hands
/// nothing and starts nothing. Nor where the wrapper is not there to run (`brew upgrade` takes
/// away the Cellar path a chat was handed): its builds then go unwrapped.
///
/// **Then the chat's own server, started by the command where none answers** (#7935).
/// sccache's client starts a missing server itself, and first binds a socket at a random path
/// in `$TMPDIR` that the server reports back on: the sandbox lets the chat bind its one socket
/// and no other, so that start fails with "Operation not permitted", and so does every compile.
/// A server started in the foreground (`SCCACHE_NO_DAEMON`) reports on no socket, so the file
/// starts one that way, in the background, inside the chat's sandbox, and waits until it
/// answers. None is started where one answers already. **A server that stopped leaves its socket
/// behind** (once idle for its timeout, or stopped, or killed), and a client that finds the file
/// but no server falls back on its own start: so whether one is there is asked of the socket
/// (`nc -U`, given a second, and asked again where a socket is there), never of the file, and a
/// socket nothing answers on is taken away before the start. The server writes what it reports
/// in [`LOG`], beside its socket. Nothing is started, written or removed where the socket's
/// folder, the log or the mark below is a link: a chat could plant one to another chat's folder,
/// or to a file of the person's.
///
/// The server is its own session (`setsid`, through the system's `perl`), so a command that is
/// interrupted does not take it down, and zsh does not lower its priority as a background job's
/// (`BG_NICE`). **A server that does not answer holds up one command**, for [`WAITS`] tenths of a
/// second, and leaves [`NOT_STARTED`] beside its socket: later commands start it again without
/// waiting, and the first to find one answering takes the mark away.
///
/// Two commands that start at once may each start one: the later takes the socket, and the
/// earlier, which nothing reaches any more, ends at its idle timeout. Either runs inside the
/// chat's sandbox, on the chat's one socket and its one cache folder, so nothing is loosened.
pub fn script(program: &Path, declared: &Declared, socket: &Path) -> String {
    let quoted = |path: &Path| super::tunnel::sh_quoted(&path.display().to_string());
    let folder = quoted(socket.parent().unwrap_or(socket));
    let log = quoted(&socket.with_file_name(LOG));
    let mark = quoted(&socket.with_file_name(NOT_STARTED));
    let conf = quoted(&declared.conf);
    let program = quoted(program);
    let answers = "/usr/bin/nc -w 1 -U \"$SCCACHE_SERVER_UDS\" </dev/null >/dev/null 2>&1";
    format!(
        "# The project's compile cache (Q324): rustc through sccache, its server this chat's own.\n\
         # Only inside the chat's sandbox (#7935): a command the person runs with `!` reads this\n\
         # file too, unsandboxed. The sandbox alone refuses a write to the config, with EPERM.\n\
         case $(LC_ALL=C /usr/bin/touch -c -r {conf} {conf} 2>&1) in\n\
         *'Operation not permitted'*) purlis_sccache_inside=1 ;;\n\
         *) purlis_sccache_inside= ;;\n\
         esac\n\
         if [ -n \"$purlis_sccache_inside\" ] && [ -x {program} ]; then\n\
         for purlis_sccache_var in $(/usr/bin/env | /usr/bin/sed -n 's/^\\(SCCACHE_[A-Za-z0-9_]*\\)=.*/\\1/p'); do \
         unset \"$purlis_sccache_var\"; done\n\
         unset purlis_sccache_var\n\
         RUSTC_WRAPPER={program}\n\
         SCCACHE_DIR={dir}\n\
         SCCACHE_CONF={conf}\n\
         SCCACHE_SERVER_UDS={socket}\n\
         export RUSTC_WRAPPER SCCACHE_DIR SCCACHE_CONF SCCACHE_SERVER_UDS\n\
         # Its server, started here where none answers (#7935): sccache's own start binds a\n\
         # socket the sandbox refuses. A stopped server leaves its socket, so ask the socket.\n\
         # Never through a link planted in the chat's temp folder.\n\
         if [ ! -L {folder} ] && [ ! -L {log} ] && [ ! -L {mark} ]; then\n\
         \x20 if {answers} || {{ [ -S \"$SCCACHE_SERVER_UDS\" ] && /bin/sleep 0.2 && {answers}; }}; then\n\
         \x20   [ -e {mark} ] && /bin/rm -f {mark}\n\
         \x20 else\n\
         \x20   [ -S \"$SCCACHE_SERVER_UDS\" ] && /bin/rm -f \"$SCCACHE_SERVER_UDS\"\n\
         \x20   (setopt no_bg_nice 2>/dev/null; SCCACHE_START_SERVER=1 SCCACHE_NO_DAEMON=1 \
         /usr/bin/perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV or exit 127' \"$RUSTC_WRAPPER\" \
         </dev/null >/dev/null 2>>{log} &) 2>/dev/null\n\
         \x20   if [ ! -e {mark} ]; then\n\
         \x20     purlis_sccache_wait=0\n\
         \x20     until {answers} || [ \"$purlis_sccache_wait\" -ge {WAITS} ]; do\n\
         \x20       /bin/sleep 0.1; purlis_sccache_wait=$((purlis_sccache_wait + 1))\n\
         \x20     done\n\
         \x20     unset purlis_sccache_wait\n\
         \x20     {answers} || : >{mark}\n\
         \x20   fi\n\
         \x20 fi\n\
         fi\n\
         fi\n\
         unset purlis_sccache_inside\n",
        dir = quoted(&declared.dir),
        socket = quoted(socket),
    )
}
