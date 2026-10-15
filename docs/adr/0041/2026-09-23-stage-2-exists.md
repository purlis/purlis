# Amendment, 2026-09-23: stage 2 exists — the executor, the gate item by item, and what it is not

**This amendment is not a sign-off and does not claim one.** It records that the runtime this
record gates has been built, in purlis#212 (unmerged when this was written), and
it goes through the gate one item at a time so that whether the gate was honoured is a thing a
reader checks rather than a thing a brief asserts. The operator has not ruled on any of it.

**What was built**, in `diazoxide/charter`:

- `crates/purlis-core/src/executor.rs` — the executor. It starts an approved extension's
  declared program, hands it one question, reads one answer, and stops it.
- `crates/purlis-core/src/handed.rs` — the one file that says what purlis hands a program,
  and the one sentence the consent prompt says about it, held to each other by a test.
- A third word in the manifest vocabulary, **`views`**: a surface the operator opens, filled by
  asking the extension's program. It is this record's second minimum capability — *"a named
  palette command that, when the operator invokes it, sends one request to the plugin and
  displays the text that comes back"* — with *the text* widened to ADR 0043's panel vocabulary,
  and nothing else widened.
- `crates/persona-statistics` — the first consumer, written as a stranger's extension would be:
  it does not link purlis's core, and it knows only the protocol.

## The design, read against decision 1

**A subprocess over a unix socket, as ruled — and the socket is a `socketpair`.** purlis makes
both ends, keeps one, and hands the program the other as its standard input and output. That is
inside the ruling's words and is argued rather than assumed, because it is a choice the record
did not make: decision 1 reached for `hookwire`'s bound socket file and its `0600`/`0700`
guards. An unnamed pair has **no path**, so there is no mode to get right, no directory to
create at the right mode and no bind-then-chmod race — the review-found defect decision 1
cites cannot recur because its subject does not exist — and one extension's program cannot
reach another's channel, because there is no address to reach it at. It also keeps decision 1's
first reason whole: the program reads a line on stdin and prints a line on stdout, so an author
debugs it by pasting a request into a terminal. Windows refuses, as `hookwire`'s Windows arm
already does and for the same reason: no `AF_UNIX` in Rust's standard library there.

**One process per question, not a long-lived plugin host.** The program is started when the
operator opens a view and stopped when it has answered — in its own process group, and the
whole group is killed, so a helper it left running goes with it. The consequence the record
worried about under *"a subprocess is a second process to start, supervise and reap"* is
therefore small: there is nothing to supervise between questions, and the app's `Exit` kills
whatever is mid-answer. The cost is a spawn per question, measured below.

**The gate is re-taken at the press.** The executor re-reads the extension record and
re-fingerprints the extension's **whole directory** (purlis#152's tree hash) at the moment
it is about to start the program, and starts nothing unless the result is *approved, these
bytes, at this path*. A survey taken when the window opened decides which buttons are drawn,
never what runs. This is what makes the consent dialog's sentence — *"charter has read every
file in this extension's directory … and will ask again if any of them changes"* — true at the
only moment it matters.

**A yes given before the executor existed does not cover running.** Until now the prompt said of
a declared program *"this charter has no extension runtime and does not start it"*. An
extension that declares a program now carries the executor's protocol number inside its
fingerprint, so every such approval reads as changed and is asked for again, under the prompt
that says it will run. A theme-only extension is not re-asked; nothing its yes covered moved.

## The gate, item by item

1. **ADR 0031 signed off.** Met, 2026-09-22 (unchanged).
2. **The tool guard is wired.** **Met by purlis#181** (merged 2026-09-22): *"charter answers
   the Bash guard: A7 is ported, the eight arms are one verdict, and the switch flips"* — M3.1
   stage 6, and `charter hook pretooluse` answers from the guard rather than a blanket `exit 2`.
   The paragraph under the previous amendment that says the guard is still one switch was true
   when written and is not now. The irony this record opens with is resolved in the direction it
   asked for: the guard ran before the runtime did.
3. **#112 and #123 closed.** Met — both closed. The executor reuses the fixed path rather than
   copying it: it calls `extension::read_at`, whose reads are the `O_NOFOLLOW`, bounded,
   descriptor-checked reads #112 asked for, and whose fingerprint and declarations are one read,
   which is #123's shape.
4. **A named plugin wants a capability the theme vocabulary cannot express, written down in this
   sequence before the runtime.** The plugin is **persona statistics**, and the want was written
   down first by ADR 0043 (*"statistics … not expressible … the missing piece is a producer"*).
   The capability is written down here: **a view about personas is handed this plane's persona
   names, which one is the default, and the stamp each of their memories was written under —
   never a title and never a body.** That sentence is `handed::what`, the value is
   `handed::personas`, and a test fails if the two stop agreeing. It lands in the same change as
   the runtime rather than a sequence before it, and that is said rather than smoothed over:
   0043 is the earlier record of the want, and this is the record of the grant.
5. **The protocol refuses, per platform, what it cannot express.** Met: on anything that is not
   unix the executor answers a refusal naming ADR 0031 before it reads anything, and the
   extension record itself was already refused there.
6. **Every unreadable state asks, each with a test seen to go red with its guard removed.** The
   executor's gate refuses, with a test for each, for: not installed, installed and not approved,
   the program's bytes changed, an undeclared file added beside it, the directory moved away from
   the approved path, the record unreadable, an approval borrowed from another id naming the same
   directory, the program not executable, a view the manifest does not declare. Where the program
   could have run, a marker it would have written is checked absent. The mutation run that removed
   each guard and watched its test go red is in purlis#212's body.
7. **Confinement decided.** Met, 2026-09-22: explicitly not a sandbox. Unchanged, and see below.
8. **The round trip is measured before the protocol is fixed.** Measured on the operator's
   machine (macOS, Apple silicon), 20 rounds after a warm-up:
   - a `/bin/sh` program that answers at once — the protocol's floor: **gate 0.16 ms, round
     trip 2.9 ms**;
   - the real persona statistics producer, release build (616 KiB): **gate 1.9 ms, round trip
     4.3 ms, 6.7 ms for the whole ask** including the plane read;
   - the same, debug build (2 MiB): gate 77 ms — the tree hash in an unoptimised build, which
     is what a developer iterating on an extension pays and an operator does not.

   Against ADR 0026, the nearest limit a person perceives is *a tab or pane switch within
   100 ms*; a view opens inside a tenth of that. **One cost outside those numbers:** macOS
   assesses a program file the first time it is executed. It was ~200 ms alone and 4.2 s under
   a parallel test run on this machine, and it falls inside the executor's five-second deadline,
   so a first open of a freshly built extension on a loaded machine can time out once. Measured,
   not fixed.

## What it is not, in the record as well as in the code

- **Not a sandbox**, as ruled. The program runs as the operator. It can write anywhere he can —
  outside its state directory included — and the consent prompt says so from the core
  (`RUNS_AS_YOU`, and a state-directory note that now says writes outside the extension's
  directory are not something purlis sees at all).
- **Not proof against a program set on outliving its question.** The process-group kill closes
  the ordinary case. A program that calls `setsid` or double-forks out of its group escapes it,
  because it runs as the operator; what would stop that is the sandbox that was declined.
- **Not atomic with the fingerprint.** The tree is hashed and then the program is started by
  path; a write between the two runs unhashed. That is ADR 0028's accepted race, and its own
  amendment of 2026-09-22 already answered it for this runtime: with no sandbox, a writer who can
  win the race already runs as the operator and needs no race. Closing it would need `fexecve`
  (`unsafe`) or a private copy of the binary per press (a new unseen executable each time, which
  is the macOS assessment above on every click).

## What purlis hands a program, and what it keeps back

The request is one line: the protocol number, the extension and view ids, the subject, the
persona the view was opened from (when it was), and the value `handed.rs` built. The program is
started with **an empty environment plus eight variables a program needs to be a program**
(`PATH`, `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_ALL`, `LC_CTYPE`, `TMPDIR`) and three that say
what it is (`PURLIS_EXTENSION`, `PURLIS_PROTOCOL`, and `PURLIS_EXTENSION_STATE` when it has a
state directory) — decision 2's row *"the harness environment: No"*, applied to purlis's own
environment. It is started through `forklock`, so it cannot inherit a chat's half-open terminal
(purlis#53). It is started in the extension's own directory.

## What is still open

- **No second grant exists, and none was invented.** A view cannot put a purlis verb on a row
  (`panel::NO_VERB` refuses it in an answer exactly as in a manifest); it cannot subscribe,
  cannot be pushed to, cannot run on a timer, and is handed nothing but its subject's facts.
  Each of those is a capability a later plugin may want, and this record's rule still applies:
  written down for a plugin that wants it, never ahead of one.
- **Marketplace vetting and untrusted-source warnings** remain deferred, as the previous
  amendment recorded.
