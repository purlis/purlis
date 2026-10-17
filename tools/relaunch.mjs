#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The half of the lifecycle a scenario test cannot reach: the app ended and started again.
//
// WebdriverIO's Tauri service keeps one app process for every spec file in a run, so a spec
// that quits takes the rest of the run with it. This owns the process instead. Run it from
// the repository root, with the app and `fake-harness` already built:
//
//   node tools/relaunch.mjs
//
// What it proves, end to end and through the real binary: a record on disk brings a chat
// back under `--resume`, and the app writes that record itself, so the second launch reads
// what the first one wrote rather than what a test hand-wrote.
//
// **And that a launch asks first** (charter-app#250): reopen every session, or start fresh.
// Nothing starts while the question is up, so this answers it the way an operator does — by
// pressing a button in the window, through the WebDriver server a build with the `e2e` feature
// carries (`tauri-plugin-wdio-webdriver`). Plain W3C WebDriver over `fetch`, one request per
// step; nothing is evaluated inside the page.
//
// **And the launch a Restart to update restarts into** (charter-app#251). The restart itself
// needs an update this build has really installed, so it is not driven here; what is driven is
// what it leaves behind — each record saying a restart to update wrote it, and the
// `restarted-to-update` word beside the machine store — and that the launch reading them says
// so, with Reopen all as the answer in front, and spends the word.
//
// **And the upgrade to chats in the session host** (#2004): the first launch runs its chats in
// the window's own process, as every build before #2004 did (`PURLIS_CHATS_IN_THE_HOST=0`), and
// every launch after it runs them in the session host, this build's default, so the second
// launch is an upgrade that has to bring the chat back on its conversation. Ending the app with
// a signal is not a quit: its host keeps the chats for the grace period, for a window that comes
// back to reattach to (ADR 0068 §2, #2006). Each launch here stands for a quit, so the host that
// launch started is ended with it, by its pid.

import { execFileSync, spawn } from "node:child_process";
import { createServer } from "node:net";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { annotations, linesWritten, stepTimer } from "./relaunch-report.mjs";

const ROOT = resolve(import.meta.dirname, "..");
const APP = join(process.env.CHARTER_TARGET_DIR ?? join(ROOT, "target", "debug"), "charter-app");
/** The conversation the chat is under. The stand-in harness only has to echo it back. */
const CONVERSATION = "11111111-2222-4333-8444-555555555555";
/**
 * How long the app is given to start its chats.
 *
 * It spawns them in `setup`, which Tauri runs inside `run()` — after the window is up — so
 * this covers the whole launch and not just the chat. Generous because a CI runner is slow
 * and a launch that is merely slow should read as slow, not as broken: the marker log says
 * which step took the time.
 */
const PATIENCE = 90_000;

/**
 * A machine store of this run's own. The store holds which projects this machine remembers
 * and which the operator has approved (ADR 0034); left alone, this script would write its
 * throwaway planes into the operator's `~/.config/charter`.
 */
const CONFIG_HOME = mkdtempSync(join(tmpdir(), "charter-relaunch-config-"));
// The host's event log (FD-9) goes here, not into the operator's own data home.
const DATA_HOME = mkdtempSync(join(tmpdir(), "charter-relaunch-data-"));

if (!existsSync(APP)) {
  console.error(
    `no app at ${APP} — build it first, with the WebDriver server this answers the launch's ` +
      "question through (in app/: npx tauri build --debug --no-bundle --features e2e " +
      "--config src-tauri/tauri.e2e.conf.json)",
  );
  process.exit(1);
}

const failures = [];
/**
 * The launch the checks are about: its name, and the timer of its steps (stopped once it ends).
 * A failed check is annotated with both (#2183), so the checks API says where the run stopped.
 */
let launch = { name: "before the first launch", timer: undefined };
const stepsOf = ({ timer }) => timer?.summary() ?? "";
function check(what, ok, detail = "") {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${detail ? ` — ${detail}` : ""}`);
  if (!ok) failures.push({ launch: launch.name, what, detail, steps: stepsOf(launch) });
}

/**
 * Ends the run with `code`, the failures as annotations first when on GitHub. It exits only once
 * stdout has drained: on macOS stdout to a pipe is written asynchronously, and an exit at once
 * cut the one FAIL line of #2183 at 64 KiB.
 */
function finish(code) {
  if (process.env.GITHUB_ACTIONS === "true") {
    for (const line of annotations(failures)) console.log(line);
  }
  process.stdout.write("", () => process.exit(code));
}

// A run that throws (the app's WebDriver server refusing, a record that is not JSON) stops at a
// launch's step too, and says which.
process.on("uncaughtException", (error) => {
  console.error(error);
  failures.push({
    launch: launch.name,
    what: "the run stopped",
    detail: String(error?.stack ?? error),
    steps: stepsOf(launch),
  });
  finish(1);
});

/** A plane with nothing in it but the file that makes it one. */
function aPlane() {
  const root = mkdtempSync(join(tmpdir(), "charter-relaunch-"));
  writeFileSync(join(root, "charter.toml"), "");
  return root;
}

/**
 * A program called `claude`, so the app treats it as the harness it is standing in for, which
 * writes down the arguments it was given and then waits to be ended.
 *
 * It answers `--version` as Claude Code does, first and with nothing written down, and it lives
 * beside the project, not in it: a chat starts at the floor (#2071), which asks the program
 * whether it is Claude Code and starts none the chat could have written.
 */
function aClaude(where, argvFile) {
  const dir = join(where, "bin");
  mkdirSync(dir, { recursive: true });
  const claude = join(dir, "claude");
  writeFileSync(
    claude,
    [
      "#!/bin/sh",
      `for word in "$@"; do [ "$word" = --version ] && { echo '2.1.300 (Claude Code)'; exit 0; }; done`,
      `printf '%s\\n' "$*" >> ${JSON.stringify(argvFile)}`,
      // Short sleeps, so that if anything ever does outlive its parent it is gone in a
      // second rather than sitting on the machine for ten minutes.
      "while :; do sleep 1; done",
      "",
    ].join("\n"),
  );
  chmodSync(claude, 0o755);
  return claude;
}

/** A port nothing on this machine is listening on, for the app's WebDriver server. */
async function aFreePort() {
  return new Promise((found, failed) => {
    const probe = createServer();
    probe.on("error", failed);
    probe.listen(0, "127.0.0.1", () => {
      const { port } = probe.address();
      probe.close(() => found(port));
    });
  });
}

/** One W3C WebDriver request to the app's own server, answering with its `value`. */
async function webdriver(port, method, path, body) {
  const answer = await fetch(`http://127.0.0.1:${port}${path}`, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const { value } = await answer.json();
  if (!answer.ok) throw new Error(`${method} ${path}: ${JSON.stringify(value)}`);
  return value;
}

/** An element reference's id, whatever key the server filed it under. */
const idOf = (element) => Object.values(element)[0];

/**
 * Waits for the launch's question and presses `answer` in it — the button whose text it is.
 *
 * Before pressing, it waits `hold` ms with the question up and reports how many chats the
 * stand-in harness had seen started by then, which is the whole of "nothing starts while the
 * question is up" — and what the question said, and the text of the control that had the
 * keyboard, which is the answer a stray Return would give.
 */
function pressing(answer, argvFile, hold = 2_000) {
  return async (port, until, steps) => {
    steps.step("waiting for the app's WebDriver server");
    let session;
    while (Date.now() < until && session === undefined) {
      session = await webdriver(port, "POST", "/session", { capabilities: { alwaysMatch: {} } })
        .then((made) => made.sessionId)
        .catch(() => undefined);
      if (session === undefined) await new Promise((tick) => setTimeout(tick, 250));
    }
    if (session === undefined) throw new Error("the app's WebDriver server never answered");
    steps.step("waiting for the launch's question");
    const buttons = async () =>
      webdriver(port, "POST", `/session/${session}/elements`, {
        using: "css selector",
        value: '[role="alertdialog"] button',
      });
    while (Date.now() < until && (await buttons()).length === 0) {
      await new Promise((tick) => setTimeout(tick, 250));
    }
    steps.step("holding with the question up");
    await new Promise((tick) => setTimeout(tick, hold));
    steps.step("reading the question");
    const startedWhileAsking = argvSeen(argvFile).length;
    const textOf = (element) =>
      webdriver(port, "GET", `/session/${session}/element/${idOf(element)}/text`);
    const said = await textOf(
      await webdriver(port, "POST", `/session/${session}/element`, {
        using: "css selector",
        value: '[role="alertdialog"]',
      }),
    );
    const focused = await textOf(
      await webdriver(port, "GET", `/session/${session}/element/active`),
    );
    for (const button of await buttons()) {
      const text = await textOf(button);
      if (text.trim() === answer) {
        steps.step(`pressing ${answer}`);
        await webdriver(port, "POST", `/session/${session}/element/${idOf(button)}/click`, {});
        return { startedWhileAsking, said, focused: focused.trim() };
      }
    }
    throw new Error(`the launch's question has no "${answer}" button`);
  };
}

/**
 * Runs the app in `plane` until `done()` is true, then ends it and everything it started.
 *
 * `answer`, when given, is run beside the wait with the app's WebDriver port — it is how the
 * launch's question is answered — and what it returns comes back as `answered`.
 */
async function theAppRuns(name, plane, done, answer, extra = {}) {
  const steps = stepTimer();
  launch = { name, timer: steps };
  steps.step("starting the app");
  const port = await aFreePort();
  // `CHARTER_LAUNCH_LOG` makes the app say how far up it got. A launch that never reaches
  // its first chat says nothing at all otherwise, which cost three round trips of guessing.
  //
  // **The plane is PINNED and not left to the working directory** (charter-app#129). This
  // spread the environment and set `cwd`, which reads as enough and is not: charter puts
  // `$CHARTER_ROOT` into every chat it starts, so running this from inside a charter chat
  // inherited that chat's plane — and `$CHARTER_ROOT` beats the walk. The app would then
  // read and rewrite the operator's reopen record, which is a list of programs it starts.
  // The fence says which planes this run may touch at all; the store and the panic log keep
  // the rest of the run off the operator's own.
  const app = spawn(APP, [], {
    cwd: plane,
    stdio: ["ignore", "pipe", "pipe"],
    env: {
      ...process.env,
      CHARTER_LAUNCH_LOG: "1",
      CHARTER_ROOT: plane,
      CHARTER_PLANE_FENCE: tmpdir(),
      CHARTER_CONFIG_HOME: CONFIG_HOME,
      CHARTER_DATA_HOME: DATA_HOME,
      TAURI_WEBDRIVER_PORT: String(port),
      ...extra,
    },
  });
  let output = "";
  app.stdout.on("data", (chunk) => (output += chunk));
  app.stderr.on("data", (chunk) => (output += chunk));
  try {
    const until = Date.now() + PATIENCE;
    let answered;
    let refused;
    if (answer) {
      answer(port, until, steps).then(
        (said) => {
          answered = said;
          steps.step("waiting for what the launch should do");
        },
        (why) => (refused = why),
      );
    } else {
      steps.step("waiting for what the launch should do");
    }
    while (Date.now() < until) {
      if (refused) return { ok: false, output: `${output}\n(answering failed: ${refused})` };
      if (done() && (!answer || answered)) return { ok: true, output, answered };
      if (app.exitCode !== null) return { ok: false, output: `${output}\n(the app exited)` };
      await new Promise((tick) => setTimeout(tick, 100));
    }
    return { ok: false, output, answered };
  } finally {
    steps.step("ending the app");
    // The pid this started, never a name: other things on this machine are called claude.
    // Its session host first, while the app is still its parent: a signal is not a quit, and
    // the host would keep the chats for a window that comes back (#2006).
    const hosts = app.exitCode === null ? hostsOf(app.pid) : [];
    app.kill("SIGTERM");
    await new Promise((gone) => {
      app.on("exit", gone);
      setTimeout(gone, 3_000);
    });
    for (const host of hosts) {
      if (alive(host)) process.kill(host, "SIGTERM");
    }
    const until = Date.now() + 5_000;
    while (hosts.some(alive) && Date.now() < until) {
      await new Promise((tick) => setTimeout(tick, 100));
    }
    steps.stop();
  }
}

/** The argument the app starts itself again with, as its session host (`purlis_host`). */
const SERVE_FOR_THE_WINDOW = "purlis-serve-for-the-window";

/** The pids of the session hosts the app at `pid` started, as `ps` lists them now. */
function hostsOf(pid) {
  const listed = execFileSync("ps", ["-A", "-o", "pid=,ppid=,command="], { encoding: "utf8" });
  return listed
    .split("\n")
    .map((line) => line.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/))
    .filter((found) => found && Number(found[2]) === pid && found[3].includes(SERVE_FOR_THE_WINDOW))
    .map((found) => Number(found[1]));
}

/** Whether a process of this user's is still there at `pid`. */
function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/**
 * Every line the stand-in harness has finished writing, which is one per time it was started. A
 * line still being written is not one yet (#2183): read early, it lacks the `--resume` at its end.
 */
function argvSeen(file) {
  try {
    return linesWritten(readFileSync(file, "utf8"));
  } catch {
    return [];
  }
}

const plane = aPlane();
const argvFile = join(plane, "argv.txt");
const claude = aClaude(`${plane}.programs`, argvFile);
const record = join(plane, ".charter", "app", "reopen.json");
/** When the test says the record was written. Nothing asserts on it; it is there because a
 *  record nobody can date is one nobody can debug. */
const STAMPED = Math.floor(Date.now() / 1000) - 60;

// A record as the app itself writes one, holding one chat under a conversation — and one tab
// that holds a view rather than a chat (ADR 0043, as amended), beside it.
mkdirSync(join(plane, ".charter", "app"), { recursive: true });
writeFileSync(
  record,
  JSON.stringify(
    {
      version: 1,
      at: STAMPED,
      chats: [
        {
          program: claude,
          args: [],
          cwd: plane,
          name: "ide.7",
          resume: CONVERSATION,
          active: true,
        },
      ],
      views: [{ view: "persona", key: "steward", title: "steward", at: 1 }],
    },
    null,
    2,
  ),
);

console.log(`a plane at ${plane}`);

// The build before the upgrade: chats in the window's own process.
const first = await theAppRuns(
  "the first launch",
  plane,
  () => argvSeen(argvFile).length >= 1,
  pressing("Reopen all sessions", argvFile),
  { PURLIS_CHATS_IN_THE_HOST: "0" },
);
check(
  "an ordinary relaunch does not say it followed an update",
  first.answered?.said !== undefined && !first.answered.said.includes("install an update"),
  `the question said: ${JSON.stringify(first.answered?.said)}`,
);
check(
  "nothing starts while the launch's question is up",
  first.answered?.startedWhileAsking === 0,
  `${first.answered?.startedWhileAsking} chat(s) had started before the answer`,
);
check(
  "the app starts the chat its record holds, once the operator reopens all",
  first.ok,
  first.ok ? "" : `the app said:\n${first.output}`,
);
const started = argvSeen(argvFile);
check(
  "it is started as a resume of the conversation that was recorded",
  started[0]?.includes(`--resume ${CONVERSATION}`),
  `it was given: ${JSON.stringify(started[0] ?? "")}`,
);
check(
  "and under the name the chat had",
  started[0]?.includes("--name ide.7"),
  `it was given: ${JSON.stringify(started[0] ?? "")}`,
);

// The record a launch leaves behind is what the next launch reads, so it has to still hold
// the chat and its conversation. Asserted on content, never on the file's timestamp: a
// window that has loaded reports which chat is in front, and that is a change the app
// rightly writes — so whether the stamp moved depends on how far the UI got, which is not
// something this test should be pinning.
//
// (That the app writes the record when something changes is `chats.rs`'s own tests, where
// the change can be made directly. Nothing here can click.)
const written = JSON.parse(readFileSync(record, "utf8"));
check(
  "the record still holds the chat, so the next launch has something to read",
  written.chats?.length === 1 && written.chats[0]?.name === "ide.7",
  `the record holds ${JSON.stringify(written.chats?.map((c) => c.name))}`,
);
check(
  "and kept the conversation, so the chat can be resumed again",
  written.chats?.[0]?.resume === CONVERSATION,
  `the record says ${JSON.stringify(written.chats?.[0]?.resume)}`,
);

// A view tab starts nothing, so nothing above could see it come back. What a relaunch owes it is
// that the launch neither drops it on the way in nor writes it out of the record on the way out:
// the core holds what the record put back until the window says otherwise, and a window that
// loaded said the same tabs back.
check(
  "the record still holds the view tab, so the next launch puts that back too",
  written.views?.some((view) => view.view === "persona" && view.key === "steward") === true,
  `the record holds ${JSON.stringify(written.views)}`,
);

// The second launch reads what the first one wrote, not what this script hand-wrote.
const second = await theAppRuns(
  "the second launch, reading what the first wrote",
  plane,
  () => argvSeen(argvFile).length >= 2,
  pressing("Reopen all sessions", argvFile, 0),
);
check(
  "a second launch, chats now in the session host, brings the chat back again",
  second.ok,
  second.ok ? "" : `the app said:\n${second.output}`,
);
check(
  "and runs it in the session host, the default after the upgrade",
  second.output.includes("chats run in the session host on this launch"),
  second.ok ? "the launch never said where its chats run" : "",
);
check(
  "still as a resume of the same conversation",
  argvSeen(argvFile)[1]?.includes(`--resume ${CONVERSATION}`),
  `it was given: ${JSON.stringify(argvSeen(argvFile)[1] ?? "")}`,
);

// The other answer. Nothing is started, and the record is cleared of what it held — but keeps
// the numbers it has dealt, so no later chat inherits a closed one's workspace (charter-app#90).
//
// The counter is set by hand first, because nothing above spends a number that the record would
// carry: the chat this script wrote has none, so the app deals it one and may not write that
// down until something changes.
const DEALT = 7;
writeFileSync(record, JSON.stringify({ ...JSON.parse(readFileSync(record, "utf8")), dealt: DEALT }));
const before = argvSeen(argvFile).length;
const recordCleared = () => {
  try {
    const now = JSON.parse(readFileSync(record, "utf8"));
    return (now.chats?.length ?? 0) === 0 && (now.views?.length ?? 0) === 0;
  } catch {
    return false;
  }
};
const third = await theAppRuns(
  "the third launch, answered Start fresh",
  plane,
  recordCleared,
  pressing("Start fresh", argvFile, 0),
);
check("start fresh clears the record", third.ok, third.ok ? "" : `the app said:\n${third.output}`);
check(
  "and starts no chat",
  argvSeen(argvFile).length === before,
  `the stand-in harness was started ${argvSeen(argvFile).length - before} more time(s)`,
);
const cleared = JSON.parse(readFileSync(record, "utf8"));
check(
  "and keeps the numbers already dealt",
  cleared.dealt >= DEALT,
  `the record says dealt ${JSON.stringify(cleared.dealt)}`,
);

// The launch a Restart to update restarts into. The restart wrote the record with the flag and
// left its word beside the machine store (`Planes::let_go_of_all_to_update`); both are written
// here the way it writes them, because the restart itself needs a real installed update.
const RESTARTED = join(CONFIG_HOME, "charter", "restarted-to-update");
writeFileSync(
  record,
  JSON.stringify(
    {
      version: 1,
      at: STAMPED,
      chats: [
        {
          program: claude,
          args: [],
          cwd: plane,
          name: "ide.7",
          resume: CONVERSATION,
          active: true,
        },
      ],
      relaunch_after_update: true,
    },
    null,
    2,
  ),
);
mkdirSync(join(CONFIG_HOME, "charter"), { recursive: true, mode: 0o700 });
writeFileSync(RESTARTED, "");
const beforeRestart = argvSeen(argvFile).length;
const fourth = await theAppRuns(
  "the fourth launch, after a restart to update",
  plane,
  () => argvSeen(argvFile).length > beforeRestart,
  pressing("Reopen all sessions", argvFile, 0),
);
check(
  "the launch after a restart to update says why it is asking",
  fourth.answered?.said?.includes("purlis restarted to install an update.") === true,
  `the question said: ${JSON.stringify(fourth.answered?.said)}`,
);
check(
  "with Reopen all as the answer in front",
  fourth.answered?.focused === "Reopen all sessions",
  `the keyboard was on ${JSON.stringify(fourth.answered?.focused)}`,
);
check(
  "and Reopen all resumes the chat the restart recorded",
  fourth.ok && argvSeen(argvFile).at(-1)?.includes(`--resume ${CONVERSATION}`),
  fourth.ok ? `it was given: ${JSON.stringify(argvSeen(argvFile).at(-1) ?? "")}` : fourth.output,
);
check(
  "and the word of the restart is spent, so no later launch says it again",
  !existsSync(RESTARTED),
  `${RESTARTED} is still there`,
);
rmSync(RESTARTED, { force: true });

console.log(failures.length === 0 ? "\nall good" : `\n${failures.length} failed`);
finish(failures.length === 0 ? 0 : 1);
