# Recordings of the real harnesses

The fidelity anchor the fake harness reproduces (T1.1-03, #6850; ticket 4 uses them). Each
harness, at the version its file in `tests/fixtures/harness-versions` pins, runs one covering
script under purlis's wrap at Full, against a stand-in model, with its hook entries naming the
recorder in place of `purlis`.

**What every covering recording is made under: Full sandbox, harness prompts bypassed
(bypassPermissions / OpenCode ask allowed once).** Each recording says so in its header
(`conditions.summary`, with Claude Code's `conditions.permission_mode` and
`conditions.sessionend_budget`, the 10 s its SessionEnd hooks are given so one is never cut off
before it begins), and so does its turn script: the fake harness reproduces a recording under these conditions, never takes them for what
the harness does. Claude Code's harness prompts are kept in one more pass, `asks`
(`asks-recording.json`): permission mode `default`, each of its prompts allowed once, so its
PermissionRequest and Notification are recorded.

| Path | What it is |
|---|---|
| `scripts/covering.json` | The covering script, written once for every harness: each step a kind of call (`bash`, `write`, `edit`, `read`, `web`, `subagent`) the stand-in asks for with the tool the harness offers. The refused calls are the steps named `refused-*`. Its first turn ends normally (Stop); its second is typed after it and ends with Esc. |
| `scripts/asks.json` | Claude Code's `asks` pass: one call its own prompt asks about. |
| `recorded/<harness>/<version>/recording.json` | A recording, masked: every hook call (input, the name of every variable of its environment, order, reply), what the stand-in asked for, and what the model was shown. |
| `recorded/<harness>/<version>/turns.json` | The turn script that reproduces it in the fake harness. |
| `recorded/claude/<version>/asks-recording.json`, `asks-turns.json` | The `asks` pass and its turn script. |
| `src/recorder.rs`, `src/bin/purlis-recorder.rs` | The recorder: writes each `hook` call, then runs the real `purlis hook`; any other command is the real `purlis`'s. It keeps every variable's name and no variable's value. |
| `src/standin.rs` | The stand-in model: Messages (Claude Code), Responses (Codex) and Chat Completions (OpenCode), on a loopback port. |
| `src/mask.rs` | The mask, a closed list: session ids, tool-use ids, absolute paths, timestamps and gaps, and the order of the calls of a parallel response the stand-in scripted, or of a run of hooks `RACY_ORDERS` names with the CI runs that saw it begun both ways; and the reply of a hook `RACY_REPLIES` names, one a harness cuts off or lets answer as it exits (Claude Code's SessionEnd). Hooks that merely overlapped keep the order they began in. A secret-shaped value is never masked: it fails the recording, as a PreToolUse and PostToolUse of one tool use that overlap do. |
| `src/drift.rs`, `src/bin/harness-drift.rs` | The nightly re-record's diff: drift opens a `harness-drift` issue; a refused call that changed, found by what it aimed at, is an escape (Q301). |
| `tests/record.rs` | The recording driver (`#[ignore]`d; Linux CI runs it). |

## Making or changing a recording

Recordings are made on Linux only, by CI: `.github/workflows/harness-recordings.yml` records each
harness twice on a pull request into `phase-1-proven`, fails unless the two runs wrote the same
files, and uploads `harness-recording-<harness>`. Take
the files from that run's artifact into `recorded/`, read their diff, and say in the pull request
which contract moved and why. Never edit a recording by hand.

A change to the mask is applied to what is checked in with
`cargo run -p purlis-recordings --bin recording-remask -- tests/recordings/recorded/*/*/recording.json`,
which masks each recording again and makes its turn script again; nothing else in it moves.

Bumping a harness: change its version file, its `package.json` and lockfile
(`npm install --package-lock-only` there), push, and check in what the run recorded.
`every_pinned_version_has_a_recording_and_a_turn_script` fails until it is.

The nightly re-records at the pinned versions and at each harness's latest release, and diffs
against what is checked in, in two entries each. `harness-rerecord-pinned` and
`harness-rerecord-latest` open drift issues and are red on an ordinary failure (an install, a
build, a recording that timed out, `gh`). `harness-escapes-pinned` and `harness-escapes-latest`
are `kind: escape` and red on an escape alone: a refused action that happened (a file written
outside, a dial that arrived at the run's loopback listener, the refused vault's contents in front
of the model), or a refused call whose recording changed.

`cargo test -p purlis-recordings` runs everything that needs no harness.
