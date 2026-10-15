# Amended for #2170, the behaviour suite speaks to a session host process, 2026-10-14

Spec #1974's testing decision (the operator's rulings of 2026-10-14) is that the behaviour suite
speaks the session protocol to a real host process over its real socket, with the repo's fake
harness as the chat's program, and that every test ends the host it started. Item 9 of the
amendment for #1998 recorded that the suite's host target was the host's code over an in-memory
link until this target existed. This note replaces that departure; the rest of item 9 stands.
Both items are **decided in implementation, to be confirmed** (D-2170-1 and D-2170-2).

1. **The host target is a process: the window's own binary, started as the window starts it.**
   The window's own start of its host (`SessionHost`) starts it, reads its ready line and
   connects to its socket as `local-ui` with the credential it minted, and the suite's window
   runs its chats over that link. A test gives its host a config home and a runtime directory
   of its own, so each host serves at a place of its own and keeps its kill switch there, and
   ends it as the window's quit does. Every behaviour runs on it, and so does the host's restart
   with the kill switch held across it. `purlis serve` is the same host, but it hears no
   project's hooks and confines no chat yet, so it joins as a target with the runners (spec
   #2041). *D-2170-1, to be confirmed.*
2. **A chat's environment is the host's.** The host process is started with a `PATH` that
   leads with a folder the test's own does not have, and a runtime directory of its own; a
   chat started there is checked to have the `PATH`, `HOME` and runtime directory the window's
   own composition gives from what the host inherited. *D-2170-2, to be confirmed.*
3. **Where it runs.** On Linux, in the existing Rust test job, which already builds the
   window's binary. On macOS the host's socket is in the user's own temporary directory, which
   a test cannot move, so there the host's code over an in-memory link remains the host
   target.
