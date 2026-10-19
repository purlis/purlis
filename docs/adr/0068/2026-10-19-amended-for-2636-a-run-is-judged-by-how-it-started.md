# Amended for #2636, a run is judged by how it started, 2026-10-19

Whether a chat runs sandboxed is decided from a record of how its run started, never from what
the window holds for it now nor from the project's setting, so a chat reattached after that
setting changed is judged by how it runs. This note adds to the amendments for #2006 and #2007; everything
else stands as accepted. Item 1 is the operator's decision of 2026-10-19; the rest are
**decided in implementation, to be confirmed** (D-2636-2 to D-2636-5).

1. **The session host records how it applied the sandbox to each run, and reads it from what
   it starts, never from the start** (the operator's decision, 2026-10-19). A start says
   nothing of it. The host reads it from the program it runs and whether a confinement holds
   it, and keeps it with the program and on the host record's `started` line. Session protocol
   1.28, a minor: each program `programs` answers says it (`sandbox`). *D-2636-1.*
2. **How a run is read** (`purlis_core::sandbox::ran`): a program Seatbelt wraps is in
   `seatbelt`; one purlis's own Linux wrap runs is in `landlock`; Claude Code started with its
   own sandbox on in its settings is in that sandbox (`seatbelt` on macOS, `bubblewrap` on
   Linux); any other a confinement holds is in `proxy`; and one with none of these is `off`. A
   confined run is never read as off, whatever its line says. *D-2636-2.*
3. **A browser login judges a run by that record.** For a run the window starts itself, its
   own record of the sandbox it compiled; for one it reattaches, the host's. A chat a runner
   composes by its names is taken as sandboxed, unless the person started it without the
   sandbox and the runner composed no confinement for it, which is off. A run recorded as off
   is reattached with no sandbox of now standing in for it. *D-2636-3.*
4. **For a browser login, a run whose record cannot be read is taken as sandboxed.** A host
   before 1.28 says nothing of it: the window takes the run as sandboxed, with no sandbox of
   now standing in for it, so it is offered a restart as any chat a sandbox change left behind
   is (#1428). *D-2636-4.*
5. **The paths where a sandboxed chat is allowed more keep the window's compiled record of the
   start.** Dispatching to another persona, a handoff's crossing, a dispatch while you are
   away, a git action the app makes for a chat and the network record each grant a sandboxed
   chat more than an unsandboxed one. There a run with no record must count as unsandboxed, so
   they go on reading what the window compiled for the chat's start, which a reattached chat
   whose sandbox changed does not have. *D-2636-5.*
