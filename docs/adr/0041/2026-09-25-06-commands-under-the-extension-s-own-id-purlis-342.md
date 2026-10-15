# Amended 2026-09-25: commands under the extension's own id (purlis#342)

The `cli` capability (ADR 0053, protocol 2) lets a chat or a script start an extension's program
from a terminal: `purlis <extension id> <command> …`. Decision 1 is unchanged: one process per
command, the gate taken again first, the same deadline and kill. Decision 2's table is unchanged.
What it adds to the threat model:

- **A new caller: whoever can run `purlis`, a chat included.** Until now a person in the window
  started every question, or a core action the person took. A command is started by a command
  line, and a chat can type one. So the tool guard judges it like any `purlis` call. The
  persona tool gate never waves through one that writes. It prompts for any extension command
  unless the installed manifest declares that command `"writes": false`, and that includes a
  command the extension does not declare and an extension that is not installed. It reads the
  manifest, not the approval: a manifest edited to call a writing command a reading one is
  changed, and the executor refuses to run it.
- **The same gate as a view.** The record comes first, then the project's and the workspace's
  on or off, then the fingerprint over the whole tree. An extension that was never approved,
  is turned off, or has changed on disk says so on stderr, exits 1 and runs nothing.
- **What the program prints reaches the caller unchanged.** That includes a chat's context,
  as the output of any program the chat runs does. It is not quoted as data the way a briefing
  section is, because the caller asked for it by name. The output is bounded at 512 KiB for
  each stream. purlis's only addition is its own line on stderr after the program's: a write
  outside the declared paths, or any plane change at all from a command that says it only
  reads.
- **No extension can stand where a core command does.** An extension id that is a core command
  word is refused at every read of the manifest. The `purlis` binary asks its own parser
  before it looks for an extension. A test fails the build for a new core command that the
  refusal does not know.
- **The command line does not reach built-in extensions yet.** It passes no bundle, as it does
  for badges, events and the briefing (#340, #343). It cannot tell the app's bundle from a
  folder claiming to be one, so a built-in's commands would be trusted on a guessed path. No
  built-in declares a command today. A word that is neither purlis's nor an installed
  extension's gets clap's own error, followed by one line saying so. Reaching built-ins needs
  the binary to find the bundle it shipped in, with the same containment the app uses. That is
  its own change.
- **What it does not close.** The program runs as whoever runs `purlis` (`RUNS_AS_YOU`). A
  command that says it only reads and writes anyway is reported, not stopped, as for an action.
