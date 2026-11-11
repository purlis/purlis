#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// A native crash, and a panic, each sent as a Crash report: the app ended by a crash and started
// again (#5691, spec #5683, ruling O9).
//
// Like `tools/relaunch.mjs`, this owns the app's process, because no scenario spec can: the
// WebdriverIO service keeps one app process for a whole run. Run it from `app/`, with the app
// built with the `e2e` feature:
//
//   npm run e2e:crash
//
// The Report intake is a recording of Sentry's envelope endpoint on 127.0.0.1, which an `e2e`
// build is pointed at through `PURLIS_REPORT_INTAKE_DSN` (a fenced build alone reads it at run
// time). Each launch has a config home of its own that says yes to Crash reports. An `e2e` build
// crashes on purpose when `PURLIS_E2E_CRASH` says how: `abort` (native, as an uncaught
// Objective-C exception ends) or `panic` (a Rust panic, which a shipped build turns into an
// abort).
//
// What it proves, through the real binary and its real crash handler:
// - a native crash leaves a minidump, sends nothing on its way down, and is sent on the next
//   launch as the fixed fields and its crashing thread's stack alone (ruling (b), #5691), never
//   the minidump, which is listed and kept on the device;
// - a panic is sent on its way down, is in the sent list after the relaunch, and leaves no
//   minidump to be sent a second time;
// - nothing but the Report intake was sent anything of the crash.
//
// macOS only, for now: the app watches itself with the crash handler only there (#5691 lists
// Linux and Windows as follow-ups).

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { appEnv } from "./app-env.mjs";

const ROOT = resolve(import.meta.dirname, "..");
const APP = join(
  process.env.CHARTER_TARGET_DIR ?? join(ROOT, "target", "debug"),
  "purlis-app",
);
/** How long a launch is given to crash, or to send what waited. A CI runner is slow. */
const PATIENCE = 90_000;
/**
 * The fields a native crash's Crash report carries, and no others: those every Crash report
 * carries (spec #5683, Q5), and its crashing thread's stack (ruling (b), #5691).
 */
const NATIVE = [
  "contexts",
  "culprit",
  "debug_meta",
  "environment",
  "event_id",
  "exception",
  "fingerprint",
  "level",
  "platform",
  "release",
  "tags",
  "timestamp",
];
/**
 * What the app says, with `PURLIS_LAUNCH_LOG`, once its crash handler watches it: the switch
 * `nativecrash::STARTED` is on (#5691), so a launch that does not say it is a failure.
 */
const HANDLER_ON = "the crash handler: watching";
/** What the app says, with `PURLIS_LAUNCH_LOG`, once it has sent the crashes that waited. */
const WAITING_SENT = "the native crashes that waited are reported";

if (process.platform !== "darwin") {
  console.log(
    "skipped: the app watches itself with the crash handler on macOS only (#5691)",
  );
  process.exit(0);
}
if (!existsSync(APP)) {
  console.error(
    `no app at ${APP}: build it first (in app/: npx tauri build --debug --no-bundle --features e2e ` +
      "--config src-tauri/tauri.e2e.conf.json)",
  );
  process.exit(1);
}

const failures = [];
function check(what, ok, detail = "") {
  console.log(
    `${ok ? "ok  " : "FAIL"} ${what}${ok || !detail ? "" : `\n${detail}`}`,
  );
  if (!ok) failures.push(what);
}

/** The Report intake: every envelope it is sent, kept whole. */
async function anIntake() {
  const received = [];
  const server = createServer((request, answer) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => {
      received.push({ path: request.url, body: Buffer.concat(chunks) });
      answer.writeHead(200, { "content-type": "application/json" });
      answer.end("{}");
    });
  });
  await new Promise((listening) => server.listen(0, "127.0.0.1", listening));
  const { port } = server.address();
  return { received, server, dsn: `http://e2e0key@127.0.0.1:${port}/1` };
}

/** A config home of its own, with a yes to Crash reports and the first-run steps behind it. */
function aConfigHome() {
  const home = mkdtempSync(join(tmpdir(), "purlis-crash-config-"));
  const store = join(home, "charter");
  mkdirSync(store, { recursive: true, mode: 0o700 });
  const keep = (name, value) =>
    writeFileSync(join(store, name), JSON.stringify(value) + "\n", {
      mode: 0o600,
    });
  keep("reports.json", { version: 1, crash_reports: true, asked: true });
  keep("onboarding.json", {
    version: 1,
    step: "welcome",
    door: null,
    signing_in: false,
    git_missing: false,
    made: null,
    finished_once: true,
  });
  return home;
}

/**
 * The minidumps the crash handler left under `home` with `ending` (`.dmp` waiting, `.kept` once
 * reported), under either spelling of its folder.
 */
function minidumps(home, ending = ".dmp") {
  return ["purlis", "charter"].flatMap((name) => {
    const crashes = join(home, name, "crashes");
    if (!existsSync(crashes)) return [];
    return readdirSync(crashes)
      .filter((file) => file.endsWith(ending))
      .map((file) => join(crashes, file));
  });
}

/** The sent list under `home`. */
function sentList(home) {
  for (const name of ["purlis", "charter"]) {
    const file = join(home, name, "reports-sent.json");
    if (existsSync(file)) return JSON.parse(readFileSync(file, "utf8")).sent;
  }
  return [];
}

/** An envelope's JSON lines up to its attachment, and the attachment's bytes, if any. */
function parse(body) {
  const lines = [];
  let rest = body;
  while (rest.length > 0) {
    const end = rest.indexOf(0x0a);
    const line = JSON.parse(
      rest.subarray(0, end === -1 ? rest.length : end).toString("utf8"),
    );
    lines.push(line);
    rest = end === -1 ? Buffer.alloc(0) : rest.subarray(end + 1);
    if (line.type === "attachment") {
      return { lines, attachment: rest.subarray(0, line.length) };
    }
  }
  return { lines, attachment: undefined };
}

async function until(done, patience = PATIENCE) {
  const end = Date.now() + patience;
  while (Date.now() < end) {
    if (done()) return true;
    await new Promise((tick) => setTimeout(tick, 100));
  }
  return done();
}

/**
 * Starts the app on `home`, with `extra` in its environment, and answers with the process, its
 * output so far, and a way to end it.
 */
function launch(home, intake, extra = {}) {
  const plane = mkdtempSync(join(tmpdir(), "purlis-crash-plane-"));
  writeFileSync(join(plane, "charter.toml"), "");
  const app = spawn(APP, [], {
    cwd: plane,
    stdio: ["ignore", "pipe", "pipe"],
    env: appEnv(process.env, {
      PURLIS_LAUNCH_LOG: "1",
      PURLIS_ROOT: plane,
      PURLIS_PLANE_FENCE: tmpdir(),
      PURLIS_CONFIG_HOME: home,
      PURLIS_DATA_HOME: mkdtempSync(join(tmpdir(), "purlis-crash-data-")),
      PURLIS_PANIC_LOG: join(home, "panics.log"),
      PURLIS_REPORT_INTAKE_DSN: intake.dsn,
      ...extra,
    }),
  });
  const run = { app, output: "" };
  app.stdout.on("data", (chunk) => (run.output += chunk));
  app.stderr.on("data", (chunk) => (run.output += chunk));
  run.ended = () => app.exitCode !== null || app.signalCode !== null;
  run.end = async () => {
    if (run.ended()) return;
    app.kill("SIGTERM");
    await until(run.ended, 5_000);
  };
  return run;
}

async function aNativeCrash() {
  console.log("# a native crash, sent on the next launch");
  const home = aConfigHome();
  const intake = await anIntake();
  try {
    const crashing = launch(home, intake, { PURLIS_E2E_CRASH: "abort" });
    await until(crashing.ended);
    check(
      "the app was watched by its crash handler",
      crashing.output.includes(HANDLER_ON),
      crashing.output,
    );
    check(
      "the app crashed natively",
      crashing.app.signalCode === "SIGABRT",
      crashing.output,
    );
    await until(() => minidumps(home).length > 0, 15_000);
    check(
      "the crash handler left one minidump",
      minidumps(home).length === 1,
      crashing.output,
    );
    check("nothing was sent on the way down", intake.received.length === 0);

    const again = launch(home, intake);
    await until(() => intake.received.length > 0 || again.ended());
    await until(
      () => again.output.includes(WAITING_SENT) || again.ended(),
      10_000,
    );
    await again.end();
    check(
      "the next launch sent one Crash report",
      intake.received.length === 1,
      again.output,
    );
    const [sent] = intake.received;
    if (sent) {
      const { lines, attachment } = parse(sent.body);
      const [header, item, event, more] = lines;
      check(
        "to the Report intake's envelope endpoint",
        sent.path === "/api/1/envelope/",
      );
      check("under the intake's DSN", header?.dsn === intake.dsn);
      check("an event first", item?.type === "event");
      check(
        "with only the fixed fields and the stack",
        JSON.stringify(Object.keys(event ?? {}).sort()) ===
          JSON.stringify(NATIVE),
        JSON.stringify(event),
      );
      const frames = event?.exception?.values?.[0]?.stacktrace?.frames ?? [];
      check(
        "the crashing thread's frames, each a module and an offset",
        frames.length > 0 &&
          frames.every(
            (frame) =>
              JSON.stringify(Object.keys(frame).sort()) ===
              JSON.stringify([
                "addr_mode",
                "instruction_addr",
                "package",
                "trust",
              ]),
          ),
        JSON.stringify(frames),
      );
      check(
        "and nothing after the event: no minidump",
        more === undefined && attachment === undefined,
      );
    }
    const listed = sentList(home);
    check(
      "the sent list holds exactly what was sent",
      listed.length === 1 &&
        listed[0].title.startsWith("purlis crashed") &&
        sent !== undefined &&
        listed[0].bytes === sent.body.toString("utf8"),
      JSON.stringify(listed),
    );
    check(
      "the minidump is not sent again, and stays on the device",
      minidumps(home).length === 0 && minidumps(home, ".kept").length === 1,
    );
  } finally {
    intake.server.close();
  }
}

async function aPanic() {
  console.log("# a panic, sent on its way down and listed after the relaunch");
  const home = aConfigHome();
  const intake = await anIntake();
  try {
    const crashing = launch(home, intake, { PURLIS_E2E_CRASH: "panic" });
    await until(crashing.ended);
    check(
      "the panic ended the app",
      crashing.app.signalCode === "SIGABRT",
      crashing.output,
    );
    const panicked = intake.received.map((sent) => parse(sent.body).lines[2]);
    check(
      "one Crash report of the panic was sent",
      panicked.length === 1 &&
        panicked[0]?.exception?.values?.[0]?.type === "panic",
      JSON.stringify(panicked),
    );
    // The handler is told before the abort, so it keeps nothing of it: give it the time a
    // minidump takes, and look.
    await new Promise((wait) => setTimeout(wait, 5_000));
    check(
      "the panic left no minidump to send twice",
      minidumps(home).length === 0,
    );

    const again = launch(home, intake);
    await until(() => again.output.includes(WAITING_SENT) || again.ended());
    await again.end();
    check(
      "the relaunch reported what waited",
      again.output.includes(WAITING_SENT),
      again.output,
    );
    check("and sent nothing more", intake.received.length === 1);
    const listed = sentList(home);
    check(
      "the panic is in the sent list after the relaunch",
      listed.length === 1 && listed[0].title.startsWith("purlis panicked at "),
      JSON.stringify(listed),
    );
  } finally {
    intake.server.close();
  }
}

await aNativeCrash();
await aPanic();
if (failures.length > 0) {
  console.log(`\n${failures.length} failed: ${failures.join("; ")}`);
}
process.stdout.write("", () => process.exit(failures.length > 0 ? 1 : 0));
