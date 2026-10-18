# Amended for #2636, a run is judged by how it started, 2026-10-19

Whether a chat runs sandboxed is decided from a record of how its run started, never from what
the window holds for it now nor from the project's setting, so a chat reattached after that
setting changed is judged by how it runs. This note adds to the amendments for #2006 and #2007; everything
else stands as accepted. Item 1 is the operator's decision of 2026-10-19; the rest are
**decided in implementation, to be confirmed** (D-2636-2 to D-2636-4).

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
3. **The window judges a run by that record.** For a run it starts itself, its own record of
   the sandbox it compiled; for one it reattaches, the host's. A chat a runner composes by its
   names is the runner's to record, and the window takes it as sandboxed. A browser login, and
   every other path that treats a sandboxed chat and an unsandboxed one apart, reads it. A run
   recorded as off is reattached with no sandbox of now standing in for it. *D-2636-3.*
4. **A run whose record cannot be read is taken as sandboxed.** A host before 1.28 says
   nothing of it: the window takes the run as sandboxed, with no sandbox of now standing in
   for it, so it is offered a restart as any chat a sandbox change left behind is (#1428).
   *D-2636-4.*
