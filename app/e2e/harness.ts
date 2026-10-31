// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import process from "node:process";
import { LOGS } from "./processes.js";

/**
 * What the app runs when a pane asks for a new session.
 *
 * The app opens the operator's shell, so a scenario test gives it one: a script that runs
 * `fake-harness`, which writes the same output every run and then answers what is typed.
 * Nothing about the app knows it is a test.
 */
export const READY = "session ready";

/**
 * The one directory this run makes anything in — every fixture plane, every config home,
 * every stand-in shell — and the fence every charter process the run starts is held to.
 *
 * **charter-app#129 is why it is one directory and not several.** This repository is checked
 * out at `workspaces/ide/charter-app`, inside the operator's own control plane, so an app
 * started here with nothing pinned walks up and finds THAT plane: a scenario run wrote 49 of
 * its own chats into `/Users/aharon/IdeaProjects/charter/.charter/app/reopen.json`, and the
 * operator found them by opening charter. The record is an execution input (ADR 0035), so
 * that is a test suite leaving programs behind in a real plane, not only a mess.
 *
 * Pinning `$CHARTER_ROOT` is how a run *avoids* that, and it is not enough on its own: a
 * config that stops pinning it goes green and poisons a plane. So every process also carries
 * `$CHARTER_PLANE_FENCE` pointing here, and the binary the scenario tests drive is built with
 * the fence in it (`e2e` turns on `purlis-core/fenced`). A run that resolves any plane this
 * tree does not hold dies naming it, in the job that broke it.
 *
 * Made once per launcher process, and `wdio.state.conf.ts` and `wdio.bench.conf.ts` spread
 * `wdio.conf.ts`, so all three share it — which is what makes it the fence for all of them.
 *
 * **It has to survive the fork, and that is why it goes through the environment.**
 * WebdriverIO runs each spec file in a worker process, and a worker imports this module
 * again: a plain `mkdtempSync` here would give the worker a SECOND tree, so a plane a spec
 * copies for itself — `opener.e2e.ts`'s stranger, `projects.e2e.ts`'s second project — would
 * land outside the fence the launcher gave the app, and the app would die opening the very
 * plane the spec made for it. The variable is inherited by the fork, so both halves of the
 * run name one tree. A worker that somehow did not inherit it makes its own and the app
 * refuses: wrong, but loudly and in the run that is wrong, which is the whole point.
 */
function theRunsTree(): string {
  const shared = process.env.CHARTER_SCENARIO_RUN;
  if (shared) return shared;
  const made = mkdtempSync(join(tmpdir(), "charter-scenario-run-"));
  process.env.CHARTER_SCENARIO_RUN = made;
  return made;
}

export const THE_RUNS_TREE = theRunsTree();

/**
 * The variable that names the file answering the next native dialog, as the `e2e` build reads
 * it (`DIALOG_ANSWER` in `src-tauri/src/opener.rs`; #1680). `dialogs.test.ts` holds the two
 * spellings together.
 */
export const DIALOG_ANSWER_VARIABLE = "PURLIS_E2E_DIALOG_ANSWER";

/** The file every app this run launches reads a native dialog's answer from (`dialogs.ts`). */
export const THE_RUNS_DIALOG_ANSWER = join(THE_RUNS_TREE, "dialog-answer.json");

/**
 * The variable that names the file whose presence has the window's next look for git find none,
 * as the `e2e` build reads it (`GIT_ANSWER` in `src-tauri/src/firstrun.rs`; #1817, `gitAnswer.ts`).
 */
export const GIT_ANSWER_VARIABLE = "PURLIS_E2E_GIT_ANSWER";

/** The file every app this run launches reads that answer from (`gitAnswer.ts`). */
export const THE_RUNS_GIT_ANSWER = join(THE_RUNS_TREE, "git-answer");

/**
 * The variable that names the stand-in forge's directory, as the `e2e` build reads it
 * (`scenario::FORGE` in `src-tauri/src/createproject.rs`; #1819, `standInForge.ts`).
 */
export const FORGE_VARIABLE = "PURLIS_E2E_FORGE";

/** The directory every app this run launches takes as its stand-in forge (`standInForge.ts`). */
export const THE_RUNS_FORGE = join(THE_RUNS_TREE, "stand-in-forge");

/**
 * The variable that names the recorded forge the `e2e` build's window signs in to and lists
 * from (`RECORDED_FORGE` in `src-tauri/src/e2eforge.rs`; #1818). `recordedForge.test.ts` holds
 * the two spellings together.
 */
export const RECORDED_FORGE_VARIABLE = "PURLIS_E2E_RECORDED_FORGE";

/** The file every app this run launches answers its forge calls from (`recordedForge.ts`). */
export const THE_RUNS_RECORDED_FORGE = join(THE_RUNS_TREE, "recorded-forge.json");

/**
 * The variable that names the file naming the run's fake forge, as the `e2e` build reads it
 * (`FAKE_FORGE` in `src-tauri/src/e2eforge.rs`; `fakeGitLab.ts`).
 */
export const FAKE_FORGE_VARIABLE = "PURLIS_E2E_FAKE_FORGE";

/** The file every app this run launches finds its fake forge in, while a spec runs one. */
export const THE_RUNS_FAKE_FORGE = join(THE_RUNS_TREE, "fake-forge.json");

/**
 * The variable that names the file holding a clone's shorter stall limit, in seconds, as the
 * core's test builds read it (`stall_of_a_scenario` in `purlis-core/src/cloning.rs`).
 */
export const CLONE_STALL_VARIABLE = "PURLIS_E2E_CLONE_STALL";

/** The file every app this run launches reads that limit from, while a spec has written one. */
export const THE_RUNS_CLONE_STALL = join(THE_RUNS_TREE, "clone-stall");

/**
 * The environment a charter process this run starts is given: the plane it means, a machine
 * store of its own, and the fence.
 *
 * **The three go together, and that is why they are one function.** `CHARTER_ROOT` says
 * which plane; `CHARTER_CONFIG_HOME` keeps this run's approvals and recents out of the
 * runner's own (see `aConfigHomeOfItsOwn`); `CHARTER_PLANE_FENCE` is what turns "the config
 * forgot one of them" from a silent write into a dead app. A launcher that set two of the
 * three used to typecheck, lint and pass — `wdio.bench.conf.ts` set neither of the first two
 * for months.
 */
export function theRunsEnvironment(
  plane: string,
  extra: Record<string, string> = {},
): Record<string, string> {
  // A launch that names a `$HOME` of its own (`wdio.finder.conf.ts`) has its git config there.
  const home = extra.HOME ?? aHomeOfTheRunsOwn();
  return {
    CHARTER_ROOT: plane,
    CHARTER_CONFIG_HOME: aConfigHomeOfItsOwn(),
    CHARTER_DATA_HOME: aDataHomeOfItsOwn(),
    CHARTER_PLANE_FENCE: THE_RUNS_TREE,
    // The app's diagnostic log (#647) goes with the rest of the run's evidence, never into the
    // runner's own log directory.
    CHARTER_LOG_DIR: LOGS,
    // A `$HOME` of the run's own, holding a git identity (#1250): see `THE_RUNS_HOME`.
    HOME: home,
    GIT_CONFIG_GLOBAL: join(home, ".gitconfig"),
    // Where a spec says what the next native dialog answers, read by the `e2e` build alone
    // (#1680, `dialogs.ts`).
    [DIALOG_ANSWER_VARIABLE]: THE_RUNS_DIALOG_ANSWER,
    // Where a spec has the window's next look for git find none, read by the `e2e` build alone
    // (#1817, `gitAnswer.ts`).
    [GIT_ANSWER_VARIABLE]: THE_RUNS_GIT_ANSWER,
    // The stand-in forge a spec creates a project on, read by the `e2e` build alone (#1819,
    // `standInForge.ts`).
    [FORGE_VARIABLE]: THE_RUNS_FORGE,
    // The forge the window signs in to and lists from, read by the `e2e` build alone (#1818,
    // `recordedForge.ts`): no run reaches a real one.
    [RECORDED_FORGE_VARIABLE]: THE_RUNS_RECORDED_FORGE,
    // The fake forge a spec clones from over the network, and the stall limit it waits out, read
    // by the `e2e` build alone (`fakeGitLab.ts`).
    [FAKE_FORGE_VARIABLE]: THE_RUNS_FAKE_FORGE,
    [CLONE_STALL_VARIABLE]: THE_RUNS_CLONE_STALL,
    ...extra,
  };
}

/** The identity every app this run launches commits with (`THE_RUNS_HOME`). */
export const THE_RUNS_GIT_IDENTITY = { name: "charter e2e", email: "e2e@charter.invalid" };

/**
 * The `$HOME` every app this run launches is given, in place of the runner's own: it holds a
 * `.gitconfig` with a git identity, and nothing else until the app writes something.
 *
 * **A scenario never depends on the runner's git config** (#1250, D-1250-8). A Linux CI runner
 * has no identity, so the doctor's `git identity` finding stood as a trouble Notice at the top
 * of every project's band and pushed the Notice a spec was looking for behind "+N more"; a Mac
 * with one never showed it. A home of the run's own makes both the same.
 *
 * **`$HOME`, not only `GIT_CONFIG_GLOBAL`**: the doctor asks git through charter's hardened
 * runner, which clears the environment and keeps `HOME` alone, so the global config git reads
 * there is `$HOME/.gitconfig`. `GIT_CONFIG_GLOBAL` names the same file for the gits a session
 * runs, which are passed it, so an inherited one cannot point them somewhere else. A fix the
 * app writes to git's global config lands here too, never in the runner's own.
 * `git-identity.e2e.ts` empties the file for a moment to see the finding the other way round,
 * and puts it back. `wdio.finder.conf.ts` names a `$HOME` of its own, with no identity in it, and
 * replaces this one.
 *
 * A fixed path in the run's tree, so a spec worker names the home the launcher gave the app.
 */
export const THE_RUNS_HOME = join(THE_RUNS_TREE, "home");

/** The global git config in `THE_RUNS_HOME`. */
export const THE_RUNS_GIT_CONFIG = join(THE_RUNS_HOME, ".gitconfig");

/** What `THE_RUNS_GIT_CONFIG` holds. */
export const THE_RUNS_GIT_CONFIG_TEXT = `[user]\n\tname = ${THE_RUNS_GIT_IDENTITY.name}\n\temail = ${THE_RUNS_GIT_IDENTITY.email}\n`;

/** `THE_RUNS_HOME`, made the first time it is asked for and left alone after. */
function aHomeOfTheRunsOwn(): string {
  mkdirSync(THE_RUNS_HOME, { recursive: true });
  if (!existsSync(THE_RUNS_GIT_CONFIG))
    writeFileSync(THE_RUNS_GIT_CONFIG, THE_RUNS_GIT_CONFIG_TEXT);
  return THE_RUNS_HOME;
}

/**
 * Exactly the `PATH` macOS gives an app opened from Finder, and nothing else.
 *
 * `launchd` starts a GUI process; no login shell is involved, so none of the directories an
 * operator's shell adds are there. charter-app#134 is what that costs.
 */
export const A_FINDER_LAUNCHS_PATH = "/usr/bin:/bin:/usr/sbin:/sbin";

/**
 * A `$HOME` holding a `claude` where Claude Code's own installer puts it — `~/.local/bin` —
 * and nowhere that `A_FINDER_LAUNCHS_PATH` can see.
 *
 * **This is charter-app#134's whole shape** (`wdio.finder.conf.ts`). The operator's harness
 * was installed, findable by their shell, and invisible to the app, so a double-clicked
 * charter refused every chat on a built-in profile. The program is the same wrapper
 * `declareAProfile` writes — it writes down what charter started it with and then runs the
 * fake harness — but it is reached by its BARE NAME out of the registry, which is the part no
 * other scenario exercises.
 */
/** What a stand-in Claude Code answers `--version` with, as `stand_in::CLAUDE_CODE_VERSION`. */
export const CLAUDE_CODE_VERSION = "2.1.300 (Claude Code)";

/** The line that answers `--version` as Claude Code, wherever among the words it is asked. */
const ANSWERS_AS_CLAUDE_CODE = `for word in "$@"; do [ "$word" = --version ] && { echo '${CLAUDE_CODE_VERSION}'; exit 0; }; done`;

/**
 * `lines`, a script's lines, with Claude Code's answer to `--version` after its shebang:
 * what a stand-in that a Claude Code profile runs needs (#2071).
 */
export function answeringAsClaudeCode(lines: string[]): string[] {
  const [shebang, ...rest] = lines;
  return [shebang, ANSWERS_AS_CLAUDE_CODE, ...rest];
}

export function writeAHarnessOnlyAShellWouldFind(fakeHarness: string): string {
  const home = mkdtempSync(join(THE_RUNS_TREE, "finder-home-"));
  const bin = join(home, ".local", "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "claude"), theProfilesProgram(fakeHarness));
  chmodSync(join(bin, "claude"), 0o755);
  return home;
}

/**
 * The soft open-file limit launchd gives an app opened from the Finder or the Dock (SC-15).
 */
export const LAUNCHDS_LIMIT = 256;

/** Where `launchedUnderLaunchdsLimit`'s launcher writes the soft limit it started the app with. */
export const THE_APPS_STARTING_LIMIT = join(THE_RUNS_TREE, "launchers", "started-with");

/**
 * A launcher that starts `app` with launchd's soft open-file limit, the hard limit left as it
 * is, and writes down the limit it started it with (ADR 0068 §9, SC-15).
 *
 * **Not `ulimit` in the job's shell.** Node raises its own soft limit to the hard one as it
 * starts, so WebdriverIO, its workers and every app they spawn run at 1 048 575 on a macOS
 * runner and 65 536 on Ubuntu whatever the shell was given. That was this file's first try,
 * and the stress run showed it. The limit has to be set between Node and the app, and a
 * launcher is the only place there is.
 *
 * **`exec`, so the launcher is the app.** The Tauri service spawns this file and later ends the
 * process it spawned by that pid, and `running(app)` finds the app by its own binary's path;
 * both see the app itself once the shell has replaced itself with it.
 */
export function launchedUnderLaunchdsLimit(app: string): string {
  const where = join(THE_RUNS_TREE, "launchers");
  mkdirSync(where, { recursive: true });
  const launcher = join(where, "app-under-launchds-limit");
  writeFileSync(
    launcher,
    [
      "#!/bin/sh",
      "# Written by the scenario tests: the app, started with the open-file limit launchd gives it.",
      `ulimit -S -n ${LAUNCHDS_LIMIT} || exit 70`,
      `ulimit -S -n > ${singleQuoted(THE_APPS_STARTING_LIMIT)}`,
      `exec ${singleQuoted(app)} "$@"`,
      "",
    ].join("\n"),
  );
  chmodSync(launcher, 0o755);
  return launcher;
}

export function writeShell(fakeHarness: string): string {
  const where = join(THE_RUNS_TREE, "shells");
  mkdirSync(where, { recursive: true });
  const shell = join(where, "harness-as-a-shell");
  writeFileSync(
    shell,
    [
      "#!/bin/sh",
      "# Written by the scenario tests: the app's idea of a shell, which is the fake harness.",
      `exec ${JSON.stringify(fakeHarness)} \\`,
      "  --synthetic 4096 \\",
      `  --sentinel ${JSON.stringify(READY)} \\`,
      "  --interactive",
      "",
    ].join("\n"),
  );
  chmodSync(shell, 0o755);
  return shell;
}

/**
 * A harness that runs a hook the way a plane's own `.claude/settings.json` spells it —
 * `charter hook sessionstart`, the bare word — and writes down the `PATH` it was given.
 *
 * **charter-app#136.** The hooks charter arms on a chat itself (the bundled plugin's) name the
 * bundled binary by its absolute path, so they never depended on `PATH`. A plane's own
 * settings do: they travel to other machines, so they say `charter` and leave it to the chat's
 * `PATH` — and a chat started from a Finder-launched app got Finder's four directories, where
 * no `charter` is. The operator saw `/bin/sh: charter: command not found` on every
 * `SessionStart`.
 *
 * The payload is the shape Claude Code pipes to a hook, and `CLAUDE_PID` is the harness's own
 * pid — `$PPID` inside the `/bin/sh -c` the fake harness runs the hook in — so the board
 * judges this report exactly as it judges a real Claude Code's (ADR 0024): the conversation
 * charter chose, which `theProfilesProgram` exported from `--session-id`. A hook that cannot
 * find `charter` says so in the pane and the harness carries on, as Claude Code does.
 */
export function writeAPluginHookingShell(fakeHarness: string): string {
  const where = join(THE_RUNS_TREE, "shells");
  mkdirSync(where, { recursive: true });
  const shell = join(where, "plugin-hooking-harness-as-a-shell");
  const recordThePath = [
    'mkdir -p "$CHARTER_ROOT/.charter/scenario"',
    `printf '%s' "$PATH" > "$CHARTER_ROOT/.charter/scenario/path-$CHARTER_CHAT"`,
  ].join(" && ");
  // The hook's stdout is discarded, as Claude Code discards it from the terminal: a hook's
  // stdout is its ANSWER to the harness — `sessionstart`'s is the session briefing, a few
  // thousand characters of JSON on the daily plane — and never something the pane shows. The
  // fake harness runs the hook in the pane itself, so printing it would scroll the sentinel a
  // spec waits for off the screen. A `charter` that is not there still says so, below.
  const thePluginsHook = [
    `printf '{"session_id":"%s","hook_event_name":"SessionStart","source":"startup"}'`,
    '"$CLAUDE_CODE_SESSION_ID"',
    "| CLAUDE_PID=$PPID charter hook sessionstart >/dev/null",
    `|| echo "charter-app#136: this chat's PATH has no charter on it (exit $?)"`,
  ].join(" ");
  writeFileSync(
    shell,
    [
      "#!/bin/sh",
      "# Written by the scenario tests: a harness running a hook by charter's bare word.",
      `exec ${JSON.stringify(fakeHarness)} \\`,
      "  --synthetic 4096 \\",
      `  --sentinel ${JSON.stringify(READY)} \\`,
      `  --hook ${singleQuoted(recordThePath)} \\`,
      `  --hook ${singleQuoted(thePluginsHook)} \\`,
      "  --interactive",
      "",
    ].join("\n"),
  );
  chmodSync(shell, 0o755);
  return shell;
}

/**
 * `text` as one word `/bin/sh` expands nothing in. Inside single quotes nothing is special but
 * the quote itself, which is closed, escaped and reopened; inside double quotes, as
 * `JSON.stringify` gives, `$`, a backtick and `\` would still be read by the shell.
 */
export function singleQuoted(text: string): string {
  return `'${text.replace(/'/g, `'\\''`)}'`;
}

/**
 * The same, for a harness that reports its state the way a real one does — by running
 * `charter hook` from inside its own session.
 *
 * Nothing is faked past the harness itself: the hook is the real binary, the socket is the
 * one the app opened, and it finds both in the environment the app put the session in. It
 * reports a turn beginning before it writes anything, then holds its output until a line is
 * typed (`--wait-for-input`), so a test can see `running` without racing the turn's end —
 * and fires `stop` once the output is done.
 */
/**
 * **While this file is in the run's tree, the reporting harness asks a permission** (#1692):
 * after its output and before its `stop`, it runs Codex's `PermissionRequest` hook through the
 * real binary, for one plain command, and waits for the window's answer as a harness waits.
 * Answered Allow, it says {@link INBOX_ALLOWED} in its pane and carries on to its `stop`. A
 * spec that wants that ask writes the file before it starts its chat and removes it after, so
 * every other chat of the run asks nothing.
 */
export const ASKS_A_PERMISSION = join(THE_RUNS_TREE, "asks-a-permission");

/**
 * **While this file is in the run's tree, the reporting harness uses a skill and holds its turn**
 * (SK-3, #2766). It behaves as the Codex it is declared as: a Codex chat is briefed on its skills
 * (`PURLIS_SKILLS_DIR`) and starts one by reading its `SKILL.md`, so after its output it runs the
 * block word Codex runs after a shell command, for a `cat` of {@link USED_SKILL}'s `SKILL.md`,
 * through the real binary. The window opens a skill run on the chat's tab, and since the harness
 * does not say its turn ended, the run stays open until the chat ends. A spec writes the file
 * before it starts its chat and removes it after.
 */
export const USES_A_SKILL = join(THE_RUNS_TREE, "uses-a-skill");

/** The built-in skill the reporting harness reads while {@link USES_A_SKILL} is there. */
export const USED_SKILL = "secrets";

/** What the reporting harness says once the window allowed its permission ask. */
export const INBOX_ALLOWED = "carried on: the window allowed it";

/** The one command the reporting harness asks permission to run. */
export const ASKED_COMMAND = "echo asked-in-the-inbox";

/**
 * **While this file is in the run's tree, the reporting harness asks about a long command
 * instead** (#1146): {@link LONG_ASKED_COMMAND}, which the Inbox must read whole and wrap. A spec
 * writes it before it starts its chat and removes it after, as {@link ASKS_A_PERMISSION}.
 */
export const ASKS_A_LONG_PERMISSION = join(THE_RUNS_TREE, "asks-a-long-permission");

/**
 * A command of 185 characters, so the ask's line, *Run a command: * and the command, is 200: the
 * most an ask's summary keeps (`SUMMARY_WIDTH`), drawn whole and never cut. Words a line can
 * break between, then a path it can break only inside a word.
 */
export const LONG_ASKED_COMMAND = `echo a long ask the Inbox reads whole and wraps ${"/a-folder-name-that-runs-on".repeat(5)}xx`;

export function writeReportingShell(fakeHarness: string, charter: string): string {
  const where = join(THE_RUNS_TREE, "shells");
  mkdirSync(where, { recursive: true });
  const shell = join(where, "reporting-harness-as-a-shell");
  writeFileSync(
    shell,
    [
      "#!/bin/sh",
      "# Written by the scenario tests: a harness that reports its own state through hooks.",
      "#",
      "# It reports no pid and no conversation id of its own, which is why the profile that",
      "# runs it declares `kind = \"codex\"`. A chat's harness is known from its profile's",
      "# declared kind since M1.2, and Claude Code's rule (ADR 0024) admits a report only if",
      "# it carries a matching `CLAUDE_PID` AND names the conversation charter chose — which",
      "# a shell script cannot do without implementing Claude Code's whole hook payload.",
      "# Codex's rule is the narrow one this stand-in actually meets: the first report of a",
      "# chat is adopted. Declaring it as the kind it BEHAVES like is honest; teaching it to",
      "# impersonate a Claude Code would be a fixture testing itself.",
      "# The turn's rising edge, before a byte of output exists.",
      `${JSON.stringify(charter)} hook userpromptsubmit </dev/null`,
      `exec ${JSON.stringify(fakeHarness)} \\`,
      "  --synthetic 4096 \\",
      "  --wait-for-input \\",
      `  --sentinel ${JSON.stringify(READY)} \\`,
      // The permission ask, only while a spec asks for it (`ASKS_A_PERMISSION`): Codex's hook
      // payload, the decision the hook prints, and the line that says the chat went on.
      `  --hook ${singleQuoted(permissionHook(charter))} \\`,
      `  --hook ${singleQuoted(skillHook(charter))} \\`,
      `  --hook ${singleQuoted(`[ -f ${singleQuoted(USES_A_SKILL)} ] || ${singleQuoted(charter)} hook stop </dev/null`)} \\`,
      "  --interactive",
      "",
    ].join("\n"),
  );
  chmodSync(shell, 0o755);
  return shell;
}

/**
 * The reporting harness's skill hook: nothing unless {@link USES_A_SKILL} is there, and then the
 * end of one shell command that reads {@link USED_SKILL}'s `SKILL.md` from the directory the
 * chat was briefed on, as Codex's `PostToolUse` says it.
 */
function skillHook(charter: string): string {
  const payload = JSON.stringify({
    session_id: "scenario",
    hook_event_name: "PostToolUse",
    tool_name: "Bash",
    tool_use_id: "call_e2e_skill",
    tool_input: { command: `cat %s/${USED_SKILL}/SKILL.md` },
    tool_response: "",
  });
  return [
    `if [ -f ${singleQuoted(USES_A_SKILL)} ]; then`,
    `  printf ${singleQuoted(payload)} "$PURLIS_SKILLS_DIR" | ${singleQuoted(charter)} hook posttooluse-blocked`,
    "fi",
  ].join("\n");
}

/**
 * The reporting harness's permission hook: nothing unless `ASKS_A_PERMISSION` or
 * `ASKS_A_LONG_PERMISSION` is there, and then the ask that file stands for.
 */
function permissionHook(charter: string): string {
  const payload = (command: string) =>
    JSON.stringify({
      session_id: "scenario",
      hook_event_name: "PermissionRequest",
      tool_name: "Bash",
      tool_input: { command },
    });
  const asks = (command: string) => [
    `  decided=$(printf '%s' ${singleQuoted(payload(command))} | ${singleQuoted(charter)} hook permissionrequest)`,
    `  case "$decided" in *'"allow"'*) echo ${singleQuoted(INBOX_ALLOWED)} ;; esac`,
  ];
  return [
    `if [ -f ${singleQuoted(ASKS_A_LONG_PERMISSION)} ]; then`,
    ...asks(LONG_ASKED_COMMAND),
    `elif [ -f ${singleQuoted(ASKS_A_PERMISSION)} ]; then`,
    ...asks(ASKED_COMMAND),
    "fi",
  ].join("\n");
}

/** Where the built binaries are, which CI and a person both pass in. */
export function built(name: string): string {
  const from = process.env.CHARTER_TARGET_DIR ?? join(process.cwd(), "..", "target", "debug");
  return join(from, name);
}

/**
 * A folder of programs a spec stands in for an installed one with, first on the app's `PATH`
 * (`wdio.conf.ts`) and empty unless a spec has put one there for its own length.
 *
 * **Beside the built binaries, never in the run's tree** (#1670): a project that runs its chats
 * sandboxed refuses a program a chat could write, and the run's tree, its `$HOME` among it, is
 * in the temp directory every sandboxed chat may write. The built binaries are in the checkout,
 * which no chat writes.
 */
export function theRunsStandIns(): string {
  const at = built("e2e-stand-ins");
  mkdirSync(at, { recursive: true });
  return at;
}

/**
 * Gives every `<name>.fixture` under `dir` its real name (#2089). A fixture whose name a
 * sandboxed chat may never write (`.claude/settings.json`, `opencode.json`) is tracked under a
 * neutral one, so no checkout of the repository has to write it.
 */
export function fixtureNames(dir: string): void {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    let name = entry.name;
    if (name.endsWith(".fixture") && name.length > ".fixture".length) {
      const real = name.slice(0, -".fixture".length);
      renameSync(join(dir, name), join(dir, real));
      name = real;
    }
    if (entry.isDirectory()) fixtureNames(join(dir, name));
  }
}

/**
 * A writable copy of a fixture plane for the app to be started in.
 *
 * The fixtures are committed and the app writes to a plane, so a scenario test never runs
 * against the ones in the repository. The copy is made fresh for the run and `CHARTER_ROOT`
 * points the app at it, which is how a plane is pinned rather than inherited from whichever
 * directory the test runner happens to be in.
 */
export function copyFixturePlane(name = "daily"): string {
  const from = join(import.meta.dirname, "..", "..", "tests", "fixtures", "planes", name);
  // Inside `THE_RUNS_TREE`, which is the fence: a plane this function made is a plane the
  // run may act on, and there is no other kind (charter-app#129).
  const to = mkdtempSync(join(THE_RUNS_TREE, `plane-${name}-`));
  const root = join(to, name);
  cpSync(from, root, { recursive: true });
  fixtureNames(root);
  // Directories the fixture cannot carry, because git will not track an empty one. The
  // generator records them beside the plane; restoring them matters because "no todos/" and
  // "an empty todos/" are different starting states.
  const listing = join(
    import.meta.dirname,
    "..",
    "..",
    "tests",
    "fixtures",
    "planes",
    `${name}.empty-dirs`,
  );
  if (existsSync(listing)) {
    for (const rel of readFileSync(listing, "utf8").split(/\s+/).filter(Boolean)) {
      mkdirSync(join(root, rel), { recursive: true });
    }
  }
  oneMoment(root, new Date());
  return root;
}

/**
 * Gives every file and directory under `dir` the same modification time, `at`.
 *
 * **A copy's times are the order the copy was made in**, and that order is the filesystem's:
 * `cpSync` walks the directory listing, which ext4 hands out in hash order. A plane's first
 * open pins its most recently active workspaces, newest first, and recency is read off these
 * times (`Store::pin_the_most_active`). Left alone, whether `alpha` or `beta` was copied last
 * decided the strip's order, and a runner could draw `beta, alpha` (#2715). One moment for
 * the whole copy makes every workspace equally active, which the core then orders by name.
 */
function oneMoment(dir: string, at: Date): void {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) oneMoment(path, at);
    if (!lstatSync(path).isSymbolicLink()) utimesSync(path, at, at);
  }
  utimesSync(dir, at, at);
}

/**
 * A `charter.local.toml` in `plane` declaring one profile that runs `program`.
 *
 * The profile's own program is a wrapper around `program`. purlis puts its own words on the
 * line for a Claude Code chat — `--plugin-dir`, `--settings`, `--session-id <uuid> --name
 * <name>` — which the fake harness has no flags for, so the wrapper writes down what it was
 * given and drops its arguments exactly as a real wrapper profile does. Nothing is installed
 * and nothing is asked of it first: the app arms the chat itself.
 *
 * Two profiles are declared, not one. `scenario` is the default, which every spec that just
 * wants a chat picks; `needs-approval` exists only for the picker scenario's approval test.
 * Approving is recorded per profile and the specs share one app process, so a single profile
 * would make that test depend on running before every other spec — and the glob does not
 * promise that.
 */
/** Whether purlis wraps a Codex chat here, at the floor too: macOS and Linux (#1040). */
function wrapsCodex(): boolean {
  return process.platform === "darwin" || process.platform === "linux";
}

export function declareAProfile(plane: string, program: string, kind = "claude"): void {
  const wrapper = theProfilesWrapper(plane);
  mkdirSync(dirname(wrapper), { recursive: true });
  writeFileSync(wrapper, theProfilesProgram(program));
  chmodSync(wrapper, 0o755);
  // On macOS and Linux a Codex chat runs at the floor (#1040 wraps it on Linux), so only on a
  // sandbox-only profile (#2037): the
  // state run's machine store has one, `scenario` (`declareTheStateRunsProfile`), and a
  // project of a state spec's own makes it its default rather than declaring a Codex of its own.
  // That profile runs the program the state run gave it, so `program` is not used here: every
  // state spec passes the same reporting harness the run does.
  if (kind === "codex" && wrapsCodex()) {
    writeFileSync(join(plane, "charter.local.toml"), '[harness]\ndefault = "scenario"\n');
    return;
  }
  writeFileSync(
    join(plane, "charter.local.toml"),
    [
      "[harness]",
      'default = "scenario"',
      "",
      "[harness.scenario]",
      `kind = ${JSON.stringify(kind)}`,
      `command = [${JSON.stringify(wrapper)}]`,
      "",
      "[harness.needs-approval]",
      `kind = ${JSON.stringify(kind)}`,
      `command = [${JSON.stringify(wrapper)}]`,
      "",
    ].join("\n"),
  );
}

/**
 * **The state run's profile** for `plane`, running `program` as Codex: on macOS and Linux,
 * where Codex runs at the floor (#1040 on Linux), a sandbox-only profile in the run's machine
 * store, whose folder of programs is put first on `environment`'s `PATH` (#2037); elsewhere,
 * where no floor wraps Codex, the project's own declared profile, as before.
 */
export function declareTheStateRunsProfile(
  plane: string,
  program: string,
  environment: Record<string, string>,
): void {
  if (!wrapsCodex()) {
    declareAProfile(plane, program, "codex");
    return;
  }
  const programs = declareASandboxOnlyCodexProfile(
    plane,
    program,
    environment.CHARTER_CONFIG_HOME,
    environment.CHARTER_DATA_HOME,
  );
  environment.PATH = [programs, process.env.PATH ?? ""].join(delimiter);
}

/**
 * **A sandbox-only Codex profile that runs `program`** (#2037, the operator's ruling of
 * 2026-10-15): a sandboxed Codex chat, the floor's included, runs only on a profile this machine
 * keeps for sandboxed chats only. So the run's machine store (`configHome`, `dataHome`) gets one,
 * named `scenario`, with its folder made as the store makes it, and `plane`'s local settings make
 * it the default. Its program is `codex` on the folder this answers, which goes first on the
 * app's `PATH`: it answers Codex's `login status` as signed in, and runs the profile's command
 * (`theProfilesProgram`) for everything else. Beside the plane, never in it, as
 * `declareAProfile`'s program is.
 */
export function declareASandboxOnlyCodexProfile(
  plane: string,
  program: string,
  configHome: string,
  dataHome: string,
): string {
  const wrapper = theProfilesWrapper(plane);
  mkdirSync(dirname(wrapper), { recursive: true });
  writeFileSync(wrapper, theProfilesProgram(program));
  chmodSync(wrapper, 0o755);
  const bin = join(`${plane}.programs`, "bin");
  mkdirSync(bin, { recursive: true });
  const codex = join(bin, "codex");
  writeFileSync(
    codex,
    [
      "#!/bin/sh",
      "# Written by the scenario tests: Codex, as a sandbox-only profile runs it.",
      'case "$1" in login) exit 0 ;; esac',
      `exec ${JSON.stringify(wrapper)} "$@"`,
      "",
    ].join("\n"),
  );
  chmodSync(codex, 0o755);
  const folder = join(dataHome, "profiles", "scenario", "config");
  mkdirSync(folder, { recursive: true, mode: 0o700 });
  const store = join(configHome, "charter");
  mkdirSync(store, { recursive: true, mode: 0o700 });
  writeFileSync(
    join(store, "profiles.json"),
    JSON.stringify({
      version: 1,
      profiles: [
        {
          name: "scenario",
          harness: "codex",
          title: "scenario",
          folders: { config: folder },
          sandbox_only: true,
        },
      ],
    }) + "\n",
    { mode: 0o600 },
  );
  writeFileSync(join(plane, "charter.local.toml"), '[harness]\ndefault = "scenario"\n');
  return bin;
}

/**
 * Where `declareAProfile` puts the profile's program for `plane`: beside the plane, never in
 * it. A chat's sandbox, the floor included, starts no program the chat could have written
 * (#2071), and a chat writes its own project.
 */
export function theProfilesWrapper(plane: string): string {
  return join(`${plane}.programs`, "claude-stand-in");
}

/**
 * The shell script a profile's command points at: it writes down what charter started it
 * with, then runs `program`.
 *
 * It answers `--version` as Claude Code does, first and with nothing written down: a
 * sandboxed start asks the program it runs whether it is Claude Code and refuses one that does
 * not answer as it (ruling V87g, #2071).
 *
 * Its own function because two callers write it — `declareAProfile` puts it in the plane as a
 * declared profile's absolute command, and `writeAHarnessOnlyAShellWouldFind` puts the same
 * bytes in a `$HOME/.local/bin/claude` that only a search finds. One copy, or the two drift
 * and the second one stops standing in for the first.
 */
function theProfilesProgram(program: string): string {
  return [
    "#!/bin/sh",
    "# Written by the scenario tests: a profile's command.",
    ANSWERS_AS_CLAUDE_CODE,
    "# What the chat's environment says about charter's footer (ADR 0029), written",
    "# down where a scenario can read it. `charter statusline` is Claude Code's `statusLine`",
    "# command and inherits this environment; the fake harness runs no such command, so this",
    "# file is how a scenario sees what a real one would have been started with. The chat's",
    "# own number keys it, so two chats in one run never write over each other.",
    `if [ -n "\${CHARTER_ROOT:-}" ] && [ -n "\${CHARTER_CHAT:-}" ]; then`,
    '  mkdir -p "$CHARTER_ROOT/.charter/scenario"',
    "  printf '%s' \"${CHARTER_FOOTER:-}\" \\",
    '    > "$CHARTER_ROOT/.charter/scenario/footer-$CHARTER_CHAT"',
    "fi",
    "# Charter starts a Claude Code chat under an id it chose (`--session-id <uuid>`),",
    "# and a report counts as that chat's harness speaking only if it names the SAME",
    "# id (ADR 0024). A real Claude Code reports the id it was given; this stand-in has",
    "# to as well, so it reads the flag off its own command line and puts it where a",
    "# hook looks. Everything else is dropped, which is what a wrapper profile does.",
    "#",
    "# The session id, the settings charter armed this session with, and the hook socket are",
    "# also written down, per chat, for `gauge.e2e.ts`: a real Claude Code runs the",
    "# `statusLine` those settings name, and the fake harness runs nothing, so the scenario",
    "# runs it itself — the command charter armed, with the environment this chat has.",
    `SEEN=""`,
    `if [ -n "\${CHARTER_ROOT:-}" ] && [ -n "\${CHARTER_CHAT:-}" ]; then`,
    '  SEEN="$CHARTER_ROOT/.charter/scenario-harness"',
    '  mkdir -p "$SEEN"',
    `  printf '%s' "\${CHARTER_HOOK_SOCKET:-}" > "$SEEN/socket-$CHARTER_CHAT"`,
    "  # The binary the bundled plugin's hooks run (`purlis_core::plugin::BINARY_ENV`).",
    `  printf '%s' "\${CHARTER_HOOK_BINARY:-}" > "$SEEN/hookbinary-$CHARTER_CHAT"`,
    "fi",
    "while [ $# -gt 0 ]; do",
    '  if [ "$1" = "--session-id" ]; then',
    "    CLAUDE_CODE_SESSION_ID=$2",
    "    export CLAUDE_CODE_SESSION_ID",
    `    [ -n "$SEEN" ] && printf '%s' "$2" > "$SEEN/session-$CHARTER_CHAT"`,
    "  fi",
    '  if [ "$1" = "--plugin-dir" ]; then',
    `    [ -n "$SEEN" ] && printf '%s' "$2" > "$SEEN/plugin-$CHARTER_CHAT"`,
    "  fi",
    '  if [ "$1" = "--settings" ]; then',
    // Claude Code reads `--settings` as JSON or as a file of it; purlis hands a file where the
    // JSON is too long for one word of a command line (SK-9).
    `    if [ -n "$SEEN" ] && [ -f "$2" ]; then cat "$2" > "$SEEN/settings-$CHARTER_CHAT"; elif [ -n "$SEEN" ]; then printf '%s' "$2" > "$SEEN/settings-$CHARTER_CHAT"; fi`,
    "  fi",
    "  shift",
    "done",
    `exec ${JSON.stringify(program)}`,
    "",
  ].join("\n");
}

/**
 * Make the fixture plane's repo directories into real clones.
 *
 * The committed fixture cannot carry them: git will not track a `.git` directory inside a
 * repository, so `tests/fixtures/planes/daily` holds `svc` and `tool` as ordinary
 * directories. The panels are about what git says, so the copy the run works on gets the
 * real thing — built here, from the files the fixture already has.
 *
 * `tool` is left with something uncommitted, because "clean" and "dirty" are two different
 * rows and a fixture where every repo is clean cannot tell them apart.
 */
export function cloneTheFixtureRepos(plane: string, workspace = "alpha"): void {
  for (const name of ["svc", "tool"]) {
    const repo = join(plane, "workspaces", workspace, name);
    if (!existsSync(repo)) continue;
    git(repo, ["init", "-q", "-b", "main", "."]);
    git(repo, ["add", "-A"]);
    git(repo, ["commit", "-q", "-m", "the fixture as it was committed"]);
  }
  writeFileSync(join(plane, "workspaces", workspace, "tool", "scratch.txt"), "not committed\n");
}

/**
 * Cut one real piece off the fixture's `svc` clone, where charter keeps them.
 *
 * `workspaces/<ws>/.worktrees/<repo>/<piece>` is the only place `worktree::list` looks — it
 * filters git's own listing down to registrations under that root, so a tree cut anywhere
 * else is not this workspace's and is not shown. Cut here with plain git and NOT by charter,
 * which is what makes it read `unwired`: the explorer has to say so before a chat is started
 * in a tree that would run with none of the plane's ask/deny rules.
 */
export function cutAFixturePiece(plane: string, workspace = "alpha"): void {
  const repo = join(plane, "workspaces", workspace, "svc");
  if (!existsSync(repo)) return;
  const at = join(plane, "workspaces", workspace, ".worktrees", "svc", "fix-login");
  mkdirSync(join(plane, "workspaces", workspace, ".worktrees", "svc"), { recursive: true });
  git(repo, ["worktree", "add", "-q", "-b", "fix-login", at]);
}

/**
 * The forge cache a refresher would have left behind, keyed the way it keys it: by the
 * checkout's path, written out.
 *
 * Only `svc` gets an entry. `tool` having none is half the point — the panel has to say that
 * nobody has fetched it rather than leaving the cell blank, which reads as green.
 */
export function writeForgeCache(plane: string, workspace = "alpha"): void {
  const cache = join(plane, ".charter", "cache");
  mkdirSync(cache, { recursive: true });
  writeFileSync(
    join(cache, "glstate.json"),
    JSON.stringify({
      [join(plane, "workspaces", workspace, "svc")]: {
        branch: "main",
        ts: Math.floor(Date.now() / 1000) - 120,
        ci: "failed",
        change: 41,
        sigil: "#",
      },
    }),
  );
}

/**
 * git, for the test's own setup. Never the code under test, and never the operator's own
 * configuration: a developer's `init.defaultBranch`, hooks or commit template must not reach
 * a fixture, and a CI runner has no identity configured at all.
 */
function git(cwd: string, args: string[]): void {
  execFileSync(
    "git",
    ["-c", "user.name=charter scenario", "-c", "user.email=scenario@example.invalid", ...args],
    {
      cwd,
      stdio: "pipe",
      env: {
        ...process.env,
        GIT_CONFIG_GLOBAL: "/dev/null",
        GIT_CONFIG_SYSTEM: "/dev/null",
        GIT_AUTHOR_DATE: "2026-01-01T00:00:00+00:00",
        GIT_COMMITTER_DATE: "2026-01-01T00:00:00+00:00",
      },
    },
  );
}

/**
 * Leaves `plane` with no record of what was open, which is what a first launch reads.
 *
 * The app writes `.charter/app/reopen.json` into the plane it was launched in. It always
 * meant to; until the launch and the commands agreed on one resolver it silently did not,
 * because the launch resolved the working directory while every command resolved
 * `$CHARTER_ROOT` (charter-app#109). Now that it does, one plane copy shared by more than one
 * session would hand the second session the first one's chats — a spec testing the record
 * instead of itself. Every session starts from none.
 */
export function anEmptyRecord(plane: string): void {
  rmSync(join(plane, ".charter", "app", "reopen.json"), { force: true });
}

/**
 * A config home of this run's own, so purlis's machine store is empty when the app starts.
 *
 * The store holds which projects this machine remembers and which the operator has approved
 * (ADR 0034), and it lives under `$CHARTER_CONFIG_HOME`, else `$XDG_CONFIG_HOME`,
 * else `~/.config`. Left alone, a scenario run would read and WRITE the runner's own — so
 * "charter asks about a project nobody has approved" would pass on a fresh runner and fail on
 * the second run of the same one, which is the worst kind of green.
 *
 * `$CHARTER_CONFIG_HOME` and not `$XDG_CONFIG_HOME`: `gh` keeps its auth under the second, so
 * redirecting that one to isolate charter silently logs `gh` out. That is the reason the
 * variable exists, and it is `report.py:consent_path`'s reason, unchanged.
 */
export function aConfigHomeOfItsOwn(): string {
  const home = mkdtempSync(join(THE_RUNS_TREE, "config-"));
  aMachineThroughTheSteps(home);
  return home;
}

/**
 * **This run's machine has been through the guided steps** (#1817): the config home's
 * `onboarding.json` says `finished_once`, as a person's machine says after its first project.
 *
 * Without it every spec that opens a project from the strip's `+` would be guided, and the
 * steps would hold its project behind their harness step, on this runner and every later spec
 * of the run. `onboarding.e2e.ts` writes the fresh place it tests, and puts this one back.
 * Written where a fresh config home keeps its store: the `charter` folder, which purlis reads
 * while there is no `purlis` one (`names::CONFIG_HOME`).
 */
function aMachineThroughTheSteps(home: string): void {
  const store = join(home, "charter");
  mkdirSync(store, { recursive: true, mode: 0o700 });
  writeFileSync(
    join(store, "onboarding.json"),
    JSON.stringify({
      version: 1,
      step: "welcome",
      door: null,
      signing_in: false,
      git_missing: false,
      made: null,
      finished_once: true,
    }) + "\n",
    { mode: 0o600 },
  );
}

/**
 * A data home of this run's own (`<data>`, ADR 0075), where the app keeps the host's event log
 * (FD-9). Left alone, a run would append its throwaway chats' events to the runner's own log,
 * and a fenced app would die opening it: the data home is a machine store the fence holds.
 */
export function aDataHomeOfItsOwn(): string {
  return mkdtempSync(join(THE_RUNS_TREE, "data-"));
}

/**
 * Gives `home` a Claude Code status line of its own — a `$HOME/.claude/settings.json` with a
 * `statusLine` in it.
 *
 * **What it is for.** charter arms its own `statusLine` for a chat only where nothing else
 * fills the line (`purlis_core::footerclaim`, the operator's ruling of 2026-09-22), and this
 * is how a scenario puts something there. The Finder launcher uses it, because that launch
 * already has a `$HOME` of its own.
 *
 * **Through `$HOME`**, which that launch already has of its own, so nothing about the
 * machine running the suite is read.
 */
export function writeAStatusLineOfTheirOwn(home: string, command: string): void {
  mkdirSync(join(home, ".claude"), { recursive: true });
  writeFileSync(
    join(home, ".claude", "settings.json"),
    JSON.stringify({ statusLine: { type: "command", command } }),
  );
}
