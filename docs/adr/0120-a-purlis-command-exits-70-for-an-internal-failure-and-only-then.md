# A `purlis` command exits 70 for an internal failure, and only then

**Accepted 2026-11-08**, for #5697 (layer 1 of the spec *people and agents report purlis's
bugs*, #5683, Q19 and Q23): one dedicated exit code for an internal failure, used at the main
internal-failure paths and never by a refusal or a usage error.

## Where purlis was

A `purlis` command exited 1 for everything that went wrong: a guard's refusal, a missing
sign-in, a usage error, and purlis's own bug alike. A panic ended a shipped build with SIGABRT
(`panicend`) and a debug build with 101. Nothing outside the process could tell a refusal on
purpose from a failure of purlis's own, so a hook could not draft a Report of one without
drafting every refusal too, and nothing may parse what a command printed to decide it.

## The decision

- **Exit 70 means a failure inside purlis**, sysexits' `EX_SOFTWARE` ("an internal software
  error"): `purlis_core::caught::INTERNAL_EXIT`. A refusal, a policy's block, a missing sign-in
  and a usage error exit 1 (clap's 2 is already turned into 1), and never 70.
- **Where it is used**: a panic on any thread of a command a chat runs (an `expect` on a state
  that cannot be, an `unreachable!` branch), which ends the process at once with one line on
  standard error, `purlis: internal failure: …`; and an answer from the app that a command has
  no arm for (`purlis-cli`'s `internal::failed`). A process that keeps its own end keeps it: a
  hook (a crashed guard refuses with 2), the MCP server, the session host, the shell guard, a
  runner's bridge and `listen`. Other paths move to it one at a time, as they are found; this is
  not a sweep.
- **A purlis tool marks the same in its error**: `the tool failed inside purlis: …`
  (`caught::tool_failed_inside`), and only for a failure inside purlis.
- **What reads it**: purlis's own hook, after a shell command whose last program is `purlis`
  exited 70 (`posttoolusefailure-blocked`), and after a purlis tool whose error carries the mark
  (`posttoolusefailure-purlis`). It hands the app the call, the exit and the error, which the
  Reports module scrubs and holds as a Report draft with the person's yes to Agent drafts.
  `purlis secret exec`, which ends with its command's own status, is never read. An extension's
  command ends with its program's status too, and one that exits 70 would be read as purlis's:
  a known gap, a dismissed draft at worst.

## What it costs

A script that compared a `purlis` command's status with 101 or 134 to spot a crash now sees 70.
None in this repository does.
