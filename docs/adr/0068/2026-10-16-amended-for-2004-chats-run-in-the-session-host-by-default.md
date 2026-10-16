# Amended for #2004, chats run in the session host by default, 2026-10-16

Spec #1974 ruled the migration on 2026-10-14: both chat implementations live in one build
behind a start-time switch, the window's own process the default until the behaviour suite is
green on the host path, then the default flips and the next change deletes the window's own
path; never a fallback at run time. The suite is green on the host path, a real host process
among its targets (#2170); every part of a chat has moved into the host (#1998 to #2003, #2006,
#2007, #2194); and a host the window left for the grace period hears `purlis stop --all` itself
(ADR 0071, amended for #2190). This note flips the default. The operator's condition of
2026-10-14, "users should not lose their profiles and sessions", is held by a test (item 3).
Item 1 is that ruling; items 2 to 4 are **decided in implementation, to be confirmed** (D-2004-2
to D-2004-4).

1. **The session host is the default** (ruled 2026-10-14; amends item 2 of the amendment for
   #1998). A launch runs its chats in the session host unless `PURLIS_CHATS_IN_THE_HOST` is set
   to `0`, which runs them in the window's own process. The switch is still read once as the
   app starts and never moves while it runs, and with no host of the window's own every start is
   still refused with why (Ruled 3). The value `0` is the way back while both paths are built,
   and goes with the window's own path (#2005). Windows, where there is no session host, runs
   chats in the window's process as before.
2. **A config home named apart has a session host of its own** (amends D-1997-1, D-1997-2;
   #2114). The socket keeps its place and its name, `purlisd.sock`, for the config home a
   person's purlis finds by itself. Where `PURLIS_CONFIG_HOME` names another one (a scenario
   run, a development build beside an installed one), the socket in the same folder is
   `purlisd-<key>.sock`, `<key>` the first 16 hex digits of the SHA-256 of that config home as
   written, and the lock beside it takes the socket's name with `.lock`. Such a purlis keeps its
   own credentials, kill switch, event log and machine store, so it serves its own chats rather
   than finding another's host it holds no credential for. One host still serves every project
   of a config home (§1, Ruled 1). Rejected: refusing chats in the second purlis and saying why,
   which stops scenario runs and a development build beside an installed one; and connecting it
   to the host that serves, which would hand it another config home's chats, switch and log.
   *D-2004-2, to be confirmed.*
3. **Nothing is lost across the upgrade, and a test holds it.** A project holds chats on two
   profiles the person declared, one of them in a worktree on a branch, and a session record.
   One launch runs them in the window's own process and quits to install an update; the next
   runs them in the session host, through the same composition the app's launch makes, and
   reopens what was open. Each chat comes back on its own conversation, on its own profile with
   that profile's command and environment and no approval asked again, in its own folder; the
   branch, its commit and the session record are as they were. Each launch is a process of its
   own; the host is the host's code over an in-memory link, so the test runs on macOS and Linux
   alike. The relaunch check (`tools/relaunch.mjs`) does the same with the real binary: its first
   launch runs chats in the window's process and the next in the session host, and since a signal
   is not a quit, it ends the host each launch started, by its pid. *D-2004-3, to be confirmed.*
4. **Nothing an upgrading person keeps moves.** The record of what was open, the profiles and
   their approvals, conversations, worktrees, session records, the kill switch's files and the
   event log are read where they were; the session host writes the event log the window wrote,
   at the same place, and takes a thrown kill switch as it starts (the amendment of ADR 0071 for
   #2001). A chat whose profile is handed credentials from a vault is not started in the host
   (#2036), and no profile is handed any in this version: a key waits for the key proxy (#822)
   and a cloud's credentials for #2151, which deliver them through the host when they come.
   *D-2004-4, to be confirmed.*

**What this does not do.** An update still ends every chat until the upgrade handoff (#665).
The window's own path stays built until #2005 deletes it.
