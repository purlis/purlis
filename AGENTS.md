# Working in purlis

Read the spec before changing behaviour: [`docs/spec.md`](docs/spec.md). The decisions and their
reasons are in [`docs/adr/`](docs/adr/), starting at ADR 0025; the plane on disk is
[`docs/plane-format.md`](docs/plane-format.md). ADRs 0001 to 0024 are the Python charter's and stay
in `purlis/purlis-plane` as history (ADR 0044). A new decision is the next number in
`docs/adr/`. An amendment to a decision is a file of its own,
`docs/adr/<NNNN>/<yyyy-mm-dd>-<slug>.md`, never a section added to the record, so pull requests
amending the same record do not conflict (`tools/adr-amendments.mjs` holds it).

## Priorities, in order

1. **Development experience.** Quick to change, quick to check.
2. **Robustness through standard practice.** Use mature, standard tools the way they are meant to
   be used. Never build custom tooling where a standard tool exists.
3. **Speed a person can notice.** Optimise only against the spec's limits.

## Rules that are easy to break

- **The core never depends on Tauri or the UI.** `purlis-core` is plain Rust. The app and the
  CLI call into it.
- **Nothing parses harness output to decide anything.** A run's state moves only by a named cause (ADR 0076): its hooks, or, for a chat at level 3, its structured protocol (ADR 0073); its program's exit; an act of the host, the operator or a policy; or, for a remote chat, what its vendor reports.
- **The plane on disk has the format `docs/plane-format.md` records.** Never change it here
  without that document changing first.
- **A new store names its tier before it ships** (ADR 0069): Plane, Clone state, Machine
  (syncable or device-bound) or Keyring, and rebuildable when it is derived. It goes in
  `docs/plane-format.md`, under its own heading or as a row of a table of paths, with a
  `**Tier:**` line, and
  `crates/purlis-core/tests/it/every_store_the_plane_format_names_has_a_tier.rs` fails until it
  has one. This is part of every feature's definition of done.
- **Nothing depends on the Python charter, shipped or not.** No message, doc page or code path in
  the app or the `purlis` binary tells anyone to install or run it, and nothing in CI or the
  tests installs it or contacts `purlis/purlis-plane`: its answers are frozen into recorded
  fixtures (ADR 0044, ADR 0045, ADR 0046). No Python in the shipped path.
- **A recorded answer changes only on purpose.** `tests/fixtures/recorded/behaviour.jsonl` is
  what the Python charter answered for 404 scenarios, replayed against every build by
  `cargo test -p purlis-cli --test recorded_behaviour` (add scenario names after `--` for
  fewer). When a change is meant to move one, re-record it with
  `CHARTER_RECORDED_BLESS=1 cargo test -p purlis-cli --test recorded_behaviour -- <name>`, read
  the fixture's diff, and say in the PR which contract moved and why. Never re-record to make a
  red run green without that sentence (ADR 0046).
- **A story's baseline is drawn only by its pull request's `stories` run** (#2901). A component's
  stories (`app/src/**/*.stories.tsx`) run in Chromium and WebKit with `npm run test:stories`,
  failing on its keyboard test, on axe at WCAG 2.2 AA and, in CI, on its screenshot. Never write
  or edit a PNG under `__screenshots__/` by hand or from a local run. When only pictures fail,
  the run keeps the baselines it drew: take them with
  `node tools/story-baselines-pull.mjs <pull request>` in your checkout of the head, review them,
  and push them with your next change (or put the `update-baselines` label on, whose commit
  leaves the other checks waiting for approval) (`docs/component-stories.md`).
- **No `unsafe`, with five audited exceptions** (`unsafe_code = "deny"` workspace-wide), each
  by the operator's ruling:
  1. `purlis_core::executor::inherit_nothing_else`: the `pre_exec` hook that closes every
     descriptor above 2 in an extension's program, because nothing but code run between `fork`
     and `exec` can. Ruling, 2026-09-23: *"Allow one audited block."*
  2. `purlis_same_user`'s `ancestry::pidinfo::read`: the one `proc_pidinfo` call, behind a
     safe wrapper, that reads a process's parent and start time in-process on macOS, where
     each hook line cost a `/bin/ps` run before. Ruling D-1407-8, 2026-10-07: a second audited
     block, in `purlis-same-user`, for `proc_pidinfo`.
  3. `purlis_linux_sandbox`'s `linux::notify::raw`: the seccomp notification calls (install a
     filter with a listener, receive, answer, check and hand over a descriptor) that the Linux
     sandbox's supervisor makes, where no safe crate has them. Ruling of 2026-10-16 (#1040):
     one more small, isolated, audited `unsafe` module, with no new C dependency.
  4. `purlis-app`'s `specimen::photograph::snapshot`: WebKit's snapshot of the theme
     specimen's hidden webview on macOS, which only `unsafe` reaches (the platform webview is a
     raw pointer and the WebKit calls are `unsafe fn`s). Ruling of 2026-10-17 (#2330): a fourth
     audited block, for the specimen snapshot.
  5. `purlis-crash-handler`'s `attach::on_crash`: the one call of `crash-handler`'s `unsafe fn`
     `make_crash_event`, which alone attaches the native crash handler, now that purlis owns the
     glue `minidumper-child` held. Ruling of 2026-11-10 (#5691, ADR 0083 as amended that day):
     *"go"*, to one small, commented `unsafe` call.

  Each has a `// SAFETY:` comment (clippy's `undocumented_unsafe_blocks` is denied), and
  `crates/purlis-core/tests/it/five_unsafe_blocks.rs` names all five and fails if `unsafe` or an
  allow of the lint appears anywhere else. A sixth block is a new ruling, not an edit.
- **UI is built from the component families in `app/src/ui`, never hand-rolled markup** (ADR
  0104): each wraps its React Aria primitive once, and lint refuses a control drawn by hand. Before
  any UI work, follow [`docs/building-ui.md`](docs/building-ui.md): the families, parts, stories
  and checks that make it done. Every family has moved and Radix is gone (#2930):
  `app/src/radixGone.guard.test.ts` fails on a `@radix-ui` import or dependency. A shadcn/ui
  component's source **copied into the repo is allowed** (ADR 0037, amended 2026-09-22).
  **Beyond the families there is one house set:** the six settings pieces in
  `app/src/settings/components.tsx` (SettingsLayout, SettingGroup, SettingRow, Field, Choice,
  SettingActions; ADR 0037, amended 2026-10-04, V89f and V89j). Another piece needs a new
  amendment.
  `docs/ui-primitives.md` says which, why, and what it costs; `docs/design-system.md` says what a
  copy has to satisfy.
- **A standing line in the window is a `Notice`** (`app/src/Notice.tsx`; ADR 0037, amended
  2026-10-05, V91d): its props require a way out (a fix, a link, Copy command or Dismiss) and a
  cause key. `app/src/Notice.guard.test.ts` fails on one built by hand, and lists every Notice
  whose only remedy is Copy command as debt.
- **No colour is written anywhere but `app/src/theme/`.** A theme is a data file; the CSS
  custom properties and xterm's theme object are both generated from it. Semantic tokens only,
  no arbitrary Tailwind values, and a test fails the build on either.
  `docs/design-system.md` says why.

- **Window copy follows `docs/ui-copy.md`.** Sentence case, no stock phrases, and an error says
  what happened and what to do. `app/src/copy.test.ts` fails the build on the rules a machine
  can check; a review reads the rest.

- **A change people would notice gets a changelog fragment, never a CHANGELOG.md edit.** Add
  `changes/<slug>.md` with a `### Added` / `### Changed` / `### Fixed` / `### Security` heading
  and the entry under it, as it will read in CHANGELOG.md (`changes/README.md`). Release prep
  folds the fragments in with `node tools/changelog-fold.mjs`; editing CHANGELOG.md's
  `## [Unreleased]` directly makes every open pull request conflict on it.

## Checks

Run what CI runs before pushing (commands in `docs/development.md`). Clippy runs with
`-D warnings`. Tests describe behaviour in their names. A test is only trusted once it has been
seen to fail for the right reason.

Two that have each cost a red `main`:

- **In a React test, a precondition waits with Testing Library's `waitFor`, not `vi.waitFor`.**
  Only the first polls inside `act`, so only the first guarantees React has committed what the
  next line reads. `vi.waitFor` is fine for a closing assertion about something outside React,
  such as which commands the core was sent.
- **A scenario spec asks for a control by its `role`, never by its tag.** `input[type="radio"]`
  is a fact about the markup; `[role="radio"]` is the thing the operator and the screen reader
  get, and it survives whatever draws it. `e2e/opening.ts` holds the picker's selectors so
  there is one copy to change.

Mutation testing runs on `purlis-core` only (`.github/workflows/mutants.yml`) and never gates a
PR. Its nightly schedule is paused since 2026-10-29 (30 nights, none green); run it by hand, and
see #3837 for running it on a pull request's own diff. It is a report, so the only thing that
matters about it is that its red is readable:

- **`baseline` red** — purlis-core's own tests do not pass. Nothing else in the run is evidence.
- **`core (N)` red** — shard N did not finish. Its verdict says whether the budget was too small
  (add shards; the crate went from 3,640 mutants to 6,555 in a day in September) or the runner
  went away.
- **`survivors` red** — a change to purlis-core that no test notices, and that was NOT there
  before. This is the one to read. The table is on the run's summary page.

`.github/mutants-survivors.txt` is the backlog of survivors already known, and only a survivor
missing from it turns `survivors` red. Adding a line to it is a decision with a reason, not a
way to make a run green — and a mutation that provably cannot change any answer does not go in
it at all: prove it and write it into the source, as `realpath` in `pypath.rs` does.

A TIMEOUT is a **hang**, listed apart and not red, when `--timeout` sits at least half again
above the slowest whole suite of the same shard: a mutant that loops for ever is caught, and no
test could ever retire it. Closer than that, or with no whole suite measured on that shard, a
TIMEOUT stays a survivor, because it may be a suite cut short. Shards run on different runners,
so one shard's suite says nothing about another's: on 2026-09-25 ten slow shards reported 432
TIMEOUTs and no MISSED mutant, and this rule is what keeps such a night red.

When the nightly is not clean it keeps one issue in this repo up to date. Sunday (while the
schedule runs) tests one of
13 weekly slices of the crate (whole files), so the issue records which files are not known
clean: a night that is not clean adds the files it tested, a slice that runs clean clears only
its own, and the issue closes, with any copies, only when none is left: a full cycle of clean
slices. A partial or cancelled run never closes it, and a dispatched run (even a full one)
never touches it. Five consecutive red nights went unread in September
2026 while fifty PRs merged past them; that is what the issue is for. Which issue, and what it
is told, is `tools/mutants-report.py notice`, tested in `tools/mutants-report.test.mjs`.

## Notes from sandboxed agent runs

- `purlis worktree commit` takes `--all` (tracked files only) or named paths (a new file must be
  named), never both; it, `push`, `update` and `dispatch` each run alone in their Bash call.
- A bare `sleep` is refused. Wait on CI with `timeout 585 bash -c 'until <check>; do sleep 30; done'`
  or `timeout 580 gh run watch <id>`.
- `gh pr edit --add-label` fails on the classic-projects deprecation; use
  `gh api -X POST repos/purlis/purlis/issues/<n>/labels -f 'labels[]=<label>'`.
- A test that runs `cargo` gets a `CARGO_TARGET_DIR` of its own: one that builds in the
  workspace's folder (the resolver seam's compile check did) leaves the test build stale, and the
  next cargo run builds it all again (14 minutes on Linux, 21 on macOS). Check it with
  `CARGO_LOG=cargo::core::compiler::fingerprint=info cargo test --no-run` after the test; see
  `docs/sandbox-scenarios-ci.md`.

- A scenario test (`app/src-tauri/src/scenarios/`) returns early in a sandboxed chat, because the
  OS wrap does not nest: its red and green are CI's. The planted-fault tests of
  `scenarios/window_person.rs` need no wrap and run here:
  `cargo test -p purlis-app --lib --locked -- scenarios::window_person::<name>` (a cold build is
  about 8 minutes; `cargo clippy -p purlis-app --tests --locked -- -D warnings` about 5).
- `cargo clippy -p purlis-app --tests --locked -j 4 -- -D warnings` is the quick local check for
  the scenarios code (clippy's `too_many_arguments` is 7; a lib-test build is about 8 minutes cold,
  so run `cargo test` in the background with `run_in_background`, not under a 600 s timeout).
  A CI job ending "runner has received a shutdown signal" (exit 143) is a lost runner: re-run it.
- To undo a probe's mutations in a sandboxed chat, save them with `git diff > patch` and take them
  out with `git apply -R patch`: `git checkout` and `git restore` write the index, which is outside
  what the chat may write. For a file `git diff` does not hold (a new one), copy it aside first.
- A running CI run's finished job can be read before the run ends:
  `gh api repos/purlis/purlis/actions/jobs/<job id>/logs` (`gh run view --log` waits for the whole
  run). A push cancels the run before it: the macOS jobs run an hour, so read them first.
- A PR whose base moved under it (`CONFLICTING`) gets no workflow runs at all: `purlis worktree
  update --base <branch>` rebases it; on a conflict, write each file's resolution and run
  `purlis worktree update --continue` (it stages for you). `git merge-file -p mine base theirs`
  three-way merges a file without touching the index.
- A heredoc whose body a guard reads as a merge is refused: write the script with the Write tool
  into the scratchpad and run it, with `python3 -I`.
- The real-harness legs (`scenarios/real_leg.rs`) are only CI's to prove; every finding came from
  the failed job's log. Make a red run say what the harness saw (the stand-in's request log, the
  screen's tail, the profile's logs) before guessing: a Codex stand-in request that offers no tool
  was the model offer's Enter, found in the request's own `model`.
- The cells runner (`app/src-tauri/src/scenarios/cells.rs`) plays only the cells a `scenario` cover
  in `coverage.json` names, each batch and dialect in a child process of its own (scenarios of one
  process share one device log, so a second batch is judged on the first's events). What a cell
  means is `tests/scenarios/src/play.rs`, which needs no chat:
  `cargo test -p purlis-scenarios --lib --locked -j 4 play::`. CI tests the merge with the current
  base, so cells other pull requests landed run under yours.
- The base-anchored gate tests run here, since they only read git:
  `SCENARIOS_BASE=$(git rev-parse origin/phase-1-proven) cargo test -p purlis-scenarios --locked --test coverage -- --ignored the_marks_keep the_list_keeps`.
  The ratchet allows no issue on an existing cell's marks that its base marks do not already name.
- A red `sandbox-scenarios-linux` says why in its `sandbox-scenarios-report-linux` artifact: a
  failing cell's `reason` lists what each step's window and the chat held. Download with
  `gh api repos/purlis/purlis/actions/artifacts/<id>/zip` into a directory made by `mktemp -d`
  (clearing a directory with `rm -rf *` is refused).
- `purlis worktree push` pushes the branch under its own name only. The scenario workflows run on
  pull requests into `phase-1-proven` or main, so a round branch with no pull request of its own
  gets no CI: the pull request's head branch has to be moved to the round's commit (or a draft
  pull request opened) before the cells can be played.
- The scenario crate's tests need local port binding, which a sandboxed chat lacks: `fake_forge`
  and `resolver_seam` fail there for that reason alone. `cargo test -p purlis-scenarios --locked
  -j 4 --no-fail-fast` shows the rest, and `cargo fmt -p purlis-app -p purlis-scenarios` is quick.
  `cargo test -p purlis-app --lib --locked -j 4 -- scenarios::window_person scenarios::report`
  (about 8 minutes cold) runs the checks' planted-fault tests.
- `gh api -X PATCH repos/purlis/purlis/pulls/<n> -F body=@<file>` edits a pull request's body;
  `gh pr edit` fails on the classic-projects deprecation.
- In a sandboxed chat, `purlis-app`'s `projectsettings::` tests (FSEvent) and `purlis-core`'s keychain,
  socket and `no_sandbox` tests fail with "Operation not permitted": that is the sandbox, not the
  change. `settings::tests` and `sandbox::personal::tests` run here.
- A cold `purlis-core` lib test build with `-j 2`, `CARGO_INCREMENTAL=0` and
  `CARGO_PROFILE_DEV_DEBUG=0` took 12 minutes on macOS (not 70), and a rebuild after an edit to the
  crate about as long. Run `cargo` with `run_in_background` and wait with
  `timeout 590 bash -c "until grep -q 'test result\|^error' <log>; do sleep 30; done"`; `nohup`
  fails here ("can't detach from console").
- `npm ci --ignore-scripts` in `app/` works in the sandbox (11 s), so `npx vitest run <files>`,
  `npx tsc --noEmit` and `npx eslint <files>` run here. `app/src/asksGroups.guard.test.ts` reads
  `AskSource` out of `bindings.ts` up to the first `;`: a `;` in a variant's doc comment hides every
  later kind from the guard.
- `cargo test -p purlis-cli --test recorded_behaviour` cannot run in a sandboxed chat: copying its
  fixture plane fails with "Operation not permitted" on the plane's vault folder (the same denial
  `git status` prints for it). It is CI's.
- A core test cannot loosen a policy family to `off` through `charter.toml`: a loosening takes
  effect only with the person's acceptance, which the app alone verifies. `log` and `enforce` are
  testable here. A `purlis-core` lib test can still run a family `off`: make the thread not the
  app and hand it the app's answer (`enforcement::set_in_the_app_for_this_test(false)`, then
  `answered_by_the_app(Some(Resolved::of(&Settings::from_text(Some("[policy]\n…")), &Locks::default())))`,
  and `set_in_the_app_for_this_test(true)` after), as `clone_it_has_no_log_mode` does.
- `bindings.ts` and `uiRpc.ts` are written by `cargo test -p purlis-app --lib --locked -j 2 --
  --ignored regenerate_the_typescript` (about 9 minutes cold); `the_typescript_the_ui_imports_…`
  checks them. A doc comment on a command is copied into both, line for line.
- A new `hookwire::Ask` variant breaks four exhaustive matches that name every kind: in
  `hookwire/tests.rs`, `purlis-cli/tests/it/handoff_from_inside_the_app.rs`,
  `purlis-core/tests/it/every_waiting_purlis_command_says_where_it_waits.rs` and
  `app/src-tauri/src/handoff/tests.rs` (`every_ask`, and its `0..=N` count).
- A new window-only press (a Tauri command the person's row sends) goes in
  `crates/session-protocol/src/ui.rs`'s `WINDOW_ONLY`, or a link or chat can press it.
- The guard reads any `gh api` endpoint with a shell-filled word (`$id` in a loop, or `-q "$Q"`) as
  a possible merge and refuses it: write each endpoint and each `-q` filter out literally, one call
  per id. `mktemp -d` fails in the sandbox; `mkdir -p $TMPDIR/<name>` works. Compare a green and a
  red CI job by saving both logs (`gh api repos/purlis/purlis/actions/jobs/<id>/logs`) and diffing
  them with their timestamps stripped: for the `fake-harness under the wrap` jobs, a cold
  `Swatinem/rust-cache` (red) against a restored one (green) is a correlation, not shown to be the
  cause.

## Agent skills

### Issue tracker

GitHub Issues on `purlis/purlis`, through `gh`. See `docs/agents/issue-tracker.md`. Every issue gets
a milestone when it is filed, unless it is filed with `needs-triage` (triage sets one) or labelled
`bot-maintained`; that file says which milestone.

### Triage labels

The five default roles, each label named as its role. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the root. See `docs/agents/domain.md`.

### Landing a pull request

See `docs/agents/skills/landing-a-pr/SKILL.md`; `tools/land.mjs` does the mechanics.

### The daily Report intake routine

See `docs/agents/skills/report-intake/SKILL.md`; `tools/report-intake.mjs` does the mechanics.
- A test that starts a chat (`a_chat_as`, `a_shell_chat`) fails in a sandboxed chat: no pty ("failed to
  openpty") and no event log, so the `handoff/tests` and anything through `Held` is CI's. A
  listener test in `purlis-cli`'s `tests/it` fails too (`Listener::bind`: "Operation not
  permitted"); a refusal test that sets no `PURLIS_HOOK_SOCKET` runs here. `proposals::tests`,
  `memoryproposal::` and `a_chats_memory_for_another_store_is_proposed::the_refusals` run here.
- A `-- brokered` filter on `purlis-core` also takes `secrets::brokered` (126 sandbox failures):
  filter on the module you mean. After `cargo fmt`, a mutation script's anchor text may have been
  reflowed: assert each anchor is found, or a mutation run reports green because nothing changed.
- `pgrep` and `ps` see no other process in the sandbox, so a wait written as `until ! pgrep …` ends at once. Wait on a background command's output file instead:
  `timeout 585 bash -c "until grep -q 'Finished\|error' <file>; do sleep 20; done"`.
- `git fetch` and `git merge` cannot write the worktree's git folder in the sandbox: bring the base in with `purlis worktree update --merge --base origin/<base>` (alone in its call), resolve, then `--continue`. The ratchet judges marks against the base's commit time, so after a merge a mark on an existing cell for an issue filed before that commit fails `the_marks_keep_their_ratchet_against_the_base`.
- A wall-clock test (`every_guard_reads_a_hostile_command_in_linear_time::the_whole_verdict_…`) is run alone and last under nextest (`threads-required = "num-cpus"`, `.config/nextest.toml`, #7514). In a loaded sandbox its release run reads 0.9 to 1.0 s on the two slowest shapes against a 1 s bound, on `main` as on `m75-policy`: judge a regression by comparing both on one machine, and set `TIMING_OUT`-style file logging, because this test binary swallows stdout and stderr even with `--nocapture`.
