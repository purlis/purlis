# Amended for #2005, the window's own path is deleted, 2026-11-27

Spec #1974 ruled the migration on 2026-10-14: both ways of running a chat in one build behind a
start-time switch, the session host made the default once the behaviour suite was green on it,
and then the window's own path deleted, never a fallback at run time (Ruled 3). #2004 made the
host the default and kept `PURLIS_CHATS_IN_THE_HOST=0` as the way back; the person ruled on
2026-10-17 that the deletion lands only after a stable release had shipped that default with
its way back. 0.5.0 (2026-11-05) and 0.5.1 (2026-11-10) both did. This note deletes the path.

1. **A chat runs only in the session host.** The switch is gone and nothing reads it to choose
   anything; a launch that still has it set (a person who ran `launchctl setenv
   PURLIS_CHATS_IN_THE_HOST 0`) logs once that it is no longer read, and runs its chats in the
   host. Amends item 1 of the amendment for #2004.
2. **With no host, nothing runs.** A plane's chats run on a host that refuses every start, with
   why, until the launch hands it the session host; a platform with no session host (Windows,
   which has never been built) runs none. Amends item 1 of the amendment for #2004, which ran
   Windows's chats in the window.
3. **The way back after a crash goes with it.** A window no longer ends a host it finds serving
   before putting its chats back, and putting chats back waits on no gate: a window always
   reattaches to the host serving its config home (#2006). Item 6 of the amendment for #2004 is
   retired.
4. **The behaviour suite's targets are the host's alone**: its code over an in-memory link, a
   real host process, through a link, and in a Docker pool. The upgrade test of item 3 of the
   amendment for #2004 keeps its in-process half: its first launch, a build before #2004, is
   played by an in-process stand-in that only a test build compiles. The relaunch check
   (`tools/relaunch.mjs`) no longer plays an upgrade: both its launches run chats in the host,
   since no build it could start runs them anywhere else. The chats' and planes' own tests are
   still written against the stand-in until they move to the host's code over an in-memory
   link.

**Why this is safe to do.** Nothing a person keeps moves: the session host has read and written
every record since 0.5.0. A person who set the way back loses only the window's own process,
which no released default has used since 0.5.0. A profile whose credentials come from a vault
was the one chat the host refused (#2036) and the window ran; in this version none is handed
any (a key waits for the key proxy, #822, and a cloud's credentials for #2151), so none loses a
place to run.
