# Amended for #1997, the window starts its session host, 2026-10-14

The window now starts the session host and is its first client: one host per OS user per
device, serving every open project (§1 and Ruled 1, unchanged). No chat runs in it yet. §1, §2
and §5 are left as accepted, and this note amends them. Item 1 is the operator's ruling of
2026-10-14 (round 2 of spec #1974's grill, "agree at all"). Items 2 to 6 are **decided in
implementation, to be confirmed** (D-1997-1 to D-1997-7), within that ruling and this record.

1. **On a desktop the window's own binary runs the host code** (ruled 2026-10-14; amends §1's
   *"the same `purlis` binary"*). The app starts itself again with the argument
   `purlis-serve-for-the-window`, which `main` answers before anything of the app starts, so
   the host is the same program with the same Keychain identity and nothing prompts. On a
   machine with no window the host is `purlis serve`. Both run one crate, `purlis-host` (Ruled
   8), which links the session protocol and the same-user checks and nothing of git, HTTP or
   the keyring, and a test holds that. It does not link the core: the core names the places it
   uses and hands them to it, so how a hosted program's start is composed (#1998) is open
   either way. *D-1997-7, to be confirmed.*
2. **The socket's place** (§1). On macOS, `purlis/purlisd.sock` in the user's own temporary
   directory as the system names it (`_CS_DARWIN_USER_TEMP_DIR`), never `$TMPDIR`. On Linux,
   `$XDG_RUNTIME_DIR/purlis/purlisd.sock` where that variable names an absolute directory this
   user owns that nobody else may enter. Otherwise `/tmp/purlis-<uid>/purlisd.sock`: the uid
   keeps two accounts apart in a shared `/tmp`. One function names the place for the window,
   `purlis serve` and a chat's sandbox alike. The folder is made `0700` and checked to be this
   user's and not a link before anything is put in it, and the window makes it before any chat
   starts; the socket is made `0600`. The credentials stay in `<config>/purlisd/` (§5). A
   chat's sandbox denies reading and writing the socket's folder, as it denies the credentials'
   (ADR 0067 §5, class 3). *D-1997-1, to be confirmed.*
3. **One host, held by a lock.** The host takes an exclusive `flock` on `purlisd.lock` beside
   the socket before it touches the socket or mints a credential. A second host of the same
   user finds it held, says that a session host already serves this user on this device, and
   exits with code 3, leaving the first one's socket and credentials as they were. The lock's
   holder alone removes a socket a host left behind. *D-1997-2, to be confirmed.*
4. **While chats run in the window, a process below the window is inside a chat** (§5, V16a;
   FD-27's item 2). Chats still run in the window's process (spec #1974's start-time switch,
   in the window by default), so the host the window started takes the window as the process
   every chat runs below: a peer below it, or in a session the window leads, is refused every
   person's scope whatever it proves, and the window itself is admitted. It is the protocol
   crate's one check, `purlis_same_user::inside_a_chat`, with the window as one more chat's
   program; every doubt is a refusal. A host started as `purlis serve` has no window beside
   it. *D-1997-3, to be confirmed.*
5. **The host ends with the window** (§2). The window holds the other end of the host's
   standard input, and the host stops when it closes, so a quit, a crash and a kill of the
   window all end it; on the way out it removes its socket. At a quit the window closes it and
   waits two seconds before it kills the host. The grace period after a crash comes with
   reattach (#646): nothing runs in the host yet. *D-1997-4, to be confirmed.*
6. **For now, the window connects only to the host it started.** Once that host says it is
   ready, the window connects as `local-ui` with the credential the host minted and holds the
   link. A window whose host found another serving says so in its log and does not connect.
   This is interim, until reattach (#646), which connects to a host that outlived its window.
   *D-1997-5, to be confirmed.* With no program in it, the host lists none, refuses
   `start_program` as `no_such_project`, and refuses every command about a program as
   `no_such_chat`. *D-1997-6, to be confirmed.*
