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

## Notes from working here (agent)

- `cargo test -p purlis-core --lib -- <filter>` runs only the named unit tests (the lib test binary
  links once; a filter keeps it quick). `cargo test -p purlis-core --test it -- <filter>` does the
  same for the integration binary. A committed project rule needs `// purlis-policy 1` first and an
  `@id` that starts with its family's prefix (`processes-`), or the whole file reads as broken and
  the project as strict.
- `purlis worktree update`, `commit` and `push` each go alone in their call (the guard refuses them
  beside another command), and `cd` carries to the next call: use absolute paths.
- Clippy's `disallowed-methods` has one `clippy.toml` per crate (core and the app here), so a rule
  that must hold across crates also needs a test that reads the workspace's files, as
  `the_press_is_made_only_by_the_windows_command_layer` does.
- In a sandboxed chat sccache cannot start (`Operation not permitted`), so `cargo` fails at
  `rustc -vV`: run it with `RUSTC_WRAPPER=` (empty) and `CARGO_INCREMENTAL=0`. A cold
  `purlis-core` test build is 5 to 7 minutes: batch the filters into one `cargo test --lib --test it`.
- A shell command whose heredoc body is prose about merging is held by the merge guard: write
  files with the Write or Edit tool, not `cat <<EOF` or `python3 - <<EOF`.
- The policy in force (M75-14) is `policy/force.rs`: a decision is `force::judge` over the
  committed project, the accepted copy and every sealed floor version, and its tests are unit
  tests (`force_tests.rs`) because they need the sealed store's test seam. A test that needs the
  accepted copy hands `judge` a hand-built `Standing`, so it never names the person's press.
- The deny state's slice (M75-07b) keeps what is outside any plane in `policy/locality.rs`
  (`locate`, `judge`, `target_of`), read by `purlis-cli`'s `guard::judged_merge` for a process the
  app did not start. `purlis-cli`'s hook tests bind a socket, so they cannot run in the sandbox:
  CI is their first run. Edition 2024 here, but keep `if let` out of match guards.
- A sandboxed chat's `cargo test` of a hook test needs `CHARTER_PLANE_FENCE=$TMPDIR` (the build is
  fenced to the machine's temp folder, not the sandbox's). Even then a merge-hook test that expects
  "no chat" cannot pass in a chat: the process is sandboxed, so it reads as a chat's (CI is its
  first run). Filter by module path (`policy::`, `secrets::answers`, `start::tests`), not `start::`,
  which also matches `didnotstart` and `dispatchrestart`; those, `secrets::keyring` and
  `sandbox::policy` fail inside a dispatched sandboxed chat for environment reasons.
- A hand-written edit script whose text mentions merging is also held when it is a heredoc in a
  Bash call: Write the script to the scratchpad and run `python3 -I <file>`. In a sandboxed chat
  the `purlis-cli` integration tests (`a_merge_from_no_chat…` among them) and the
  `sandbox::opencode_tests` confine tests fail for environment reasons (`Operation not
  permitted`): CI is their first run. `cargo clippy -j 4 -p purlis-core -p purlis-cli
  --all-targets -- -D warnings` takes about 5 minutes cold, and a cold `cargo test -p purlis-core
  --lib` about 9.
- `locality::reach_of` fails closed (review round 3): a merge that is not a top-level `gh`/`glab`
  segment, a shell or string runner, a substitution or here-document, an unnameable repo or any
  `gh api` without a `repos/o/r` path is `Unreadable`, and `guard::judged_without_a_chat` refuses it
  wherever `answers::anchored_roots()` is not empty.
- The proof infrastructure (M75-09) is `crates/purlis-core/tests/it/policy_proof/`: `PolicyInput`,
  `Batch`/`Cell`, `engine` (over `decide::decide`) and `oracle` (`oracle.rs`: its own Cedar
  request, policy set and outcome derivation, straight to `Authorizer::is_authorized`; it shares
  only `policy::load`, as a gate). A cell must be one the schema can ask: every context field its
  action declares (`merge` needs `via`), the right types. The oracle panics on one that is not,
  because the host answers an unfit request with a deny and no rule, which would agree with an
  oracle that said the same. Its test run is
  `cargo test -p purlis-core --test it -- policy_proof the_policy_proof`, about 30 s warm.
- The anchored planes the hook can read are `app/anchored-planes/` (the app writes it as the
  registry gains a plane, `answers::anchored_roots` reads the union): the sealed registry is
  unreadable outside the app.
- The sandbox compile (M75-22) is `policy/sandbox_rules.rs` (Cedar TPE; `compile`, `terms_of`) over
  `policy/resolver.rs` (the one path resolver). TPE writes `a != b` as `!(a == b)`, so a residual
  reader must take both. Its tests are lib tests: `cargo test -p purlis-core --lib --
  policy::sandbox_rules` (about 10 s warm). loading a policy (`levels::load_with`, `assemble_checked`) refuses `attended`,
  `harness_mode`, `like`, a residual outside the subset (`Policy::sandbox_problems_cached`, about 24
  compiles: free when every sandbox rule is the shipped default's, else cached per rule set) and a `Path` literal not spelt normally, on any action; `force::world_of` builds a `Path`'s world
  with the same resolver. An unguarded `context.machine` does not validate, so no loaded rule errs
  on a missing device: build `ActionRules.erred` by hand to test it.
- A sandboxed chat can run `cargo test -j 2 -p purlis-core --test it -- <filters>` and the lib's
  `floor_tests`; failures that say `Operation not permitted` (creating a `.charter/vaults` folder,
  binding a port, `sandbox_apply`) are the chat's sandbox, not the code: CI is their first run.
  A grant check that walks `place.denied` must honour `Reach::Itself` (`grant::holds_the_folder`).
- A limits-table key may carry `min` and `max` (`limits.toml`; `Spec::bounded`): a number outside
  reads as the nearest end, with a Notice. `[policy] limits = "off"` and a `[policy.limits]` table
  are the same TOML key, so one file cannot say both: a test of the mode and of the numbers
  loads two. `cargo test --lib --test it` stops at the lib's first (sandbox) failure and never
  runs `it`: run `--test it` alone.
- A project rule on `start` needs an `@id` that starts with `trust-` (its family), or the whole
  file reads as broken and the rule never fires. A `Command` resource is read by its program name
  (the last path segment) before any rule sees it (`policy.rs::program_name`). In a sandboxed chat
  `git checkout -- <file>` is refused (index.lock): restore a file with `git show HEAD:<path> > <path>`.
  A cold lib-test build can pass the 10-minute tool limit: run it in the background and read its
  output file, and print `^error` lines too, or a compile failure looks like a silent pass.
- A wall-clock test (`every_guard_reads_a_hostile_command_in_linear_time::the_whole_verdict_…`) is run alone and last under nextest (`threads-required = "num-cpus"`, `.config/nextest.toml`, #7514). In a loaded sandbox its release run reads 0.9 to 1.0 s on the two slowest shapes against a 1 s bound, on `main` as on `m75-policy`: judge a regression by comparing both on one machine, and set `TIMING_OUT`-style file logging, because this test binary swallows stdout and stderr even with `--nocapture`.
- An Audited event's kind must start with `trust.` and use only lowercase letters, `.` and `_`
  (`eventlog/entry.rs::trust_kind`); a hyphen or another prefix is refused at write time, and
  `Hooks::audit` only logs it. A test of a Journal line needs `Hooks::relayed` (no socket) with
  `Events::here(Recorder::new(..))`: `listening_on` cannot bind in a sandboxed chat. BSD `sed -i`
  needs `''`: use `perl -pi -e` to rename across files.
- A sandboxed chat can run `npm ci --ignore-scripts`, `tsc --noEmit`, `eslint` and
  `vitest run --project unit --maxWorkers=2` (about 10 minutes; run it in the background). Its
  only failures are `e2e/` files that bind a port or run git (`listen EPERM`), and the stories
  project cannot start at all (no local port): CI's `stories` run is the first for a story.
  `cargo test -p purlis-core --test it` fails about 425 tests there for the same reasons; the
  `every_store_the_plane_format_names_has_a_tier` ones pass bar the child that binds a socket.
- The sealed store's keyring seam is `cfg(test)` in `purlis-core` only, so an app test cannot hold
  entries in it: `app/src-tauri/src/timedgrants.rs` reads and changes the store through its small
  `Store` trait, and its tests use an in-memory one. `cargo test -p purlis-app -- --ignored`
  rewrites `app/src/bindings.ts`; `npm ci --ignore-scripts` in `app/` gives tsc, eslint and vitest.
