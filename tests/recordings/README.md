# Recordings of the real harnesses

The fidelity anchor the fake harness reproduces (T1.1-03, #6850; ticket 4 uses them). Each
harness, at the version its file in `tests/fixtures/harness-versions` pins, runs one covering
script under purlis's wrap at Full, against a stand-in model, with its hook entries naming the
recorder in place of `purlis`.

| Path | What it is |
|---|---|
| `scripts/covering.json` | The covering script, written once for every harness: each step a kind of call (`bash`, `write`, `edit`, `read`, `web`, `subagent`) the stand-in asks for with the tool the harness offers. The refused calls are the steps named `refused-*`. |
| `recorded/<harness>/<version>/recording.json` | A recording, masked: every hook call (input, environment, order, reply), what the stand-in asked for, and what the model was shown. |
| `recorded/<harness>/<version>/turns.json` | The turn script that reproduces it in the fake harness. |
| `src/recorder.rs`, `src/bin/purlis-recorder.rs` | The recorder: writes each `hook` call, then runs the real `purlis hook`; any other command is the real `purlis`'s. A credential's value is never recorded. |
| `src/standin.rs` | The stand-in model: Messages (Claude Code), Responses (Codex) and Chat Completions (OpenCode), on a loopback port. |
| `src/mask.rs` | The mask, a closed list: session ids, tool-use ids, absolute paths, timestamps and gaps, and the order of a parallel group's calls. A secret-shaped value is never masked: it fails the recording. |
| `src/drift.rs`, `src/bin/harness-drift.rs` | The nightly re-record's diff: drift opens a `harness-drift` issue; a refused call that changed is an escape (Q301). |
| `tests/record.rs` | The recording driver (`#[ignore]`d; Linux CI runs it). |

## Making or changing a recording

Recordings are made on Linux only, by CI: `.github/workflows/harness-recordings.yml` records each
harness on a pull request into `phase-1-proven` and uploads `harness-recording-<harness>`. Take
the files from that run's artifact into `recorded/`, read their diff, and say in the pull request
which contract moved and why. Never edit a recording by hand.

Bumping a harness: change its version file, its `package.json` and lockfile
(`npm install --package-lock-only` there), push, and check in what the run recorded.
`every_pinned_version_has_a_recording_and_a_turn_script` fails until it is.

The nightly entries `harness-rerecord-pinned` and `harness-rerecord-latest` re-record at the
pinned versions and at each harness's latest release, and diff against what is checked in.

`cargo test -p purlis-recordings` runs everything that needs no harness.
