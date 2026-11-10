# Regenerate runs as a shell the window composes, in the session host, with no network

**2026-11-09, #5962.** Amends §6 of ADR 0122 with how building Regenerate decided to run it,
on the spec's coordinating chat's ruling of the same day (option (b)).

- **The window composes the run and the session host starts it.** No path ran an arbitrary
  program in a chat's sandbox, and a `terminal` link may start no shell at all, so the window
  wraps the command itself (`sandbox::wrapper::line` over `sandbox::seatbelt::profile`, as a
  chat's line is wrapped: Seatbelt on macOS, purlis's own wrap on Linux) and hands the host a
  shell launch of its own (`StartProgram` of kind `shell` with a `launch`,
  `ends_with_its_window`). It waits only for the exit status, at most 300 seconds, then stops
  it. Nothing in the session protocol changed. `artifact::regenerate::launch`,
  `app/src-tauri/src/regenerate.rs`.
- **The profile is a chat's, narrowed**: it reads what a chat reads, under the plane's denial
  classes; it names no proxy port and no socket, so **it reaches no network**; and **it writes
  only a scratch folder of its own** (its out folder, its temporary folder, its log). The command
  is run with no shell between: a fixed `/bin/sh -c 'exec "$@"'` passes its words as arguments
  and writes its output to the log, which is only shown, tail-cut and masked.
- **What is approved is the code, not the command's text**: the first word naming a file inside
  the plane is the script, and its skill's folder (else its own) is fingerprinted, every path,
  mode bit and byte. The approval is each person's, Machine tier
  (`<config>/artifact-regenerate-approvals.json`). It is asked again right before each run, so
  only the approved fingerprint runs; a yes is recorded only for the fingerprint the prompt
  showed.
- **Only the declared data files are written**, from the out folder, each checked as a page's
  read checks it, all renamed into place or none. A run that fails leaves the data and says
  "last good: <date>".
- **The CLI only checks** (`purlis artifact regenerate <pin> --check`); running stays the
  window's, on the person's press. On Windows, Regenerate is refused, as scripted artifacts are
  (§10).
