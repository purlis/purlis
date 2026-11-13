// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync, spawnSync } from "node:child_process";
import {
  appendFileSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import {
  HARNESS,
  PROGRAM_COLUMNS,
  PS_COLUMNS,
  type ProcessRow,
  descendants,
  footprintsOfTheLiving,
  harnessMemory,
  type LivingFootprints,
  parseFootprints,
  parseProcesses,
  runs,
  webContent,
} from "./memory.js";

/**
 * The app under test and the harnesses it runs, as the operating system sees them — and what
 * is left to look at when the app is gone.
 *
 * The app died once while a scenario was opening its fiftieth tab (charter-app#16), and all
 * that run kept was a WebDriver error: the window's server had stopped answering. Whether the
 * process had died or only that server, how, and at what size, nobody could say. Everything
 * here answers one of those questions, and a failed run writes the answers into `logs/`,
 * which CI keeps.
 */

/** Where a failed run's evidence goes: the directory CI uploads when the scenario job fails. */
export const LOGS = join(process.cwd(), "logs");

/**
 * Where the app under test writes a panic on its way out (`CHARTER_PANIC_LOG`): beside the
 * rest of the evidence, so a run that lost the app keeps the one line that says why.
 */
export const PANIC_LOG = join(LOGS, "panics.log");

/** When this run started, so evidence is only what this run left behind. */
const RUN_STARTED = Date.now();

/**
 * **Room for the whole process table.** A wrapped chat's sandbox keeps its profile on its own
 * command line (`purlis _sandbox --profile …`), up to hundreds of KiB, so a few of them pass
 * Node's default 1 MiB and a listing cut off there read as no processes at all.
 */
export const PS_MAX_BUFFER = 256 * 1024 * 1024;

function ps(args: string[]): string {
  try {
    return execFileSync("ps", args, { encoding: "utf8", maxBuffer: PS_MAX_BUFFER });
  } catch (error) {
    // `ps -p` exits non-zero when no such process exists, which is an answer, not a failure.
    // Anything else (a listing too long to hold, `ps` not run at all) is no answer.
    if (typeof (error as { status?: unknown }).status === "number") return "";
    throw error;
  }
}

/** Every process on this machine, with the program each one runs. */
function processes(): ProcessRow[] {
  return parseProcesses(ps(["-A", "-o", PS_COLUMNS]), ps(["-A", "-o", PROGRAM_COLUMNS]));
}

/**
 * How many fake harnesses this machine is running, asked of the operating system.
 *
 * By the program each process RUNS, not any command line that mentions the name: a Claude
 * Code session whose prompt discussed `fake-harness` was once counted three times, and this
 * test failed at 53 of an expected 50 with nothing wrong. `runs` is the one matcher, shared
 * with `harnessMemory`.
 */
export function harnessesRunning(): number {
  return processes().filter((row) => runs(row.program, HARNESS)).length;
}

/**
 * The first argument the window gives its own binary when it starts it as the session host
 * (`purlis_host::SERVE_FOR_THE_WINDOW`, #2085). Spelled here rather than read from the Rust
 * side; `sessionhost/tests.rs` pins the same word.
 */
export const SESSION_HOST_ARGUMENT = "purlis-serve-for-the-window";

/**
 * The first arguments the app gives its own binary when it starts it as a short-lived helper
 * rather than a window (#5586): a Linux chat's sandbox wrap (`purlis_linux_sandbox::COMMAND`
 * and `NO_NAMESPACES`), a browser engine's keeper (`browsers::KEEPER`), git's credential helper
 * (`gitcredential::HELPER_ARG`) and the keyring item writer (`keyhold::HOLD_ARG`). Spelled here
 * as {@link SESSION_HOST_ARGUMENT} is. Counted as windows, a chat's wrap still closing read as a
 * second app and failed the stress test on Linux.
 */
export const HELPER_ARGUMENTS: readonly string[] = [
  "_sandbox",
  "_no-namespaces",
  "purlis-engine-keeper",
  "purlis-git-credential",
  "charter-hold-a-keyring-item",
];

/** The LAST argument of the binary started as a branch's bounded reader (`reader::READ_ARG`). */
export const READER_ARGUMENT = "charter-read-a-branch";

/** The app's processes, by pid: its windows, and the session hosts they started. */
export interface AppProcesses {
  windows: number[];
  sessionHosts: number[];
}

/** Which of the app's processes `command` is, when it runs `binary`; `null` when it does not,
 *  or runs it as a helper ({@link HELPER_ARGUMENTS}, {@link READER_ARGUMENT}). */
function appProcessKind(command: string, binary: string): keyof AppProcesses | null {
  if (!(command === binary || command.startsWith(`${binary} `))) return null;
  const words = command.slice(binary.length).trim().split(/\s+/);
  const first = words[0];
  if (HELPER_ARGUMENTS.includes(first) || words[words.length - 1] === READER_ARGUMENT) return null;
  return first === SESSION_HOST_ARGUMENT ? "sessionHosts" : "windows";
}

/**
 * The processes in `listing` (`ps -A -o pid=,command=`) that run `binary`, split into the app's
 * windows and its session hosts.
 *
 * By the command line, not the parent: the host is the window's own binary started again with
 * {@link SESSION_HOST_ARGUMENT} as its first argument, a helper with one of
 * {@link HELPER_ARGUMENTS} (or {@link READER_ARGUMENT} last) is neither, and a window is that
 * binary started with anything else. Counting by the binary alone found every run's host as a second app (#2384).
 */
export function appProcessesIn(listing: string, binary: string): AppProcesses {
  const found: AppProcesses = { windows: [], sessionHosts: [] };
  for (const raw of listing.split("\n")) {
    const line = raw.trim();
    const pid = Number.parseInt(line, 10);
    const kind = appProcessKind(line.slice(line.indexOf(" ") + 1).trimStart(), binary);
    if (Number.isInteger(pid) && kind !== null) found[kind].push(pid);
  }
  return found;
}

/** The run's app: its windows and session hosts, and every program they started. */
export interface RunsApp extends AppProcesses {
  /**
   * Everything below the windows and hosts that is neither: a chat's harness and what it runs,
   * each in a process group of its own (`purlis_core::executor`), so a host that is killed
   * leaves them running.
   */
  programs: number[];
}

/**
 * The app's processes running `binary` below `launcher` in `rows`: the windows anything the
 * launcher started started, the session hosts those windows started, and every program below
 * them (#5653).
 *
 * By the tree, not the binary alone: another run's app on the same build is not this run's to
 * end. The host runs in a process group of its own (#2006), so its parent is what ties it to
 * the window; a host whose window already went has been handed to `init` and is not found.
 */
export function theRunsApp(rows: ProcessRow[], launcher: number, binary: string): RunsApp {
  const found: RunsApp = { windows: [], sessionHosts: [], programs: [] };
  for (const row of descendants(rows, launcher)) {
    const kind = appProcessKind(row.command, binary);
    if (kind !== null) found[kind].push(row.pid);
  }
  found.windows = withoutCompanions(rows, found.windows);
  const app = new Set([...found.windows, ...found.sessionHosts]);
  for (const pid of app) {
    for (const row of descendants(rows, pid)) {
      if (!app.has(row.pid) && !found.programs.includes(row.pid)) found.programs.push(row.pid);
    }
  }
  return found;
}

/** The app this process started, running `binary`, asked of the operating system. */
export function theRunsAppNow(binary: string): RunsApp {
  return theRunsApp(processes(), process.pid, binary);
}

/**
 * `windows` without the crash handler's companions: on macOS the window starts its own binary
 * again, with nothing on its command line but an environment variable, to keep a native crash's
 * minidump (#5691, `purlis_crash_handler::watch`). Its command line is a window's, so only its
 * parent tells it apart: a "window" whose parent is a window is that companion, not a second app.
 */
function withoutCompanions(rows: ProcessRow[], windows: number[]): number[] {
  const parent = new Map(rows.map((row) => [row.pid, row.ppid]));
  return windows.filter((pid) => !windows.includes(parent.get(pid) ?? -1));
}

/** The app's processes running `binary` in `rows`: its windows and session hosts. */
export function appProcessesOf(rows: ProcessRow[], binary: string): AppProcesses {
  const found: AppProcesses = { windows: [], sessionHosts: [] };
  for (const row of rows) {
    const kind = appProcessKind(row.command, binary);
    if (kind !== null) found[kind].push(row.pid);
  }
  return { ...found, windows: withoutCompanions(rows, found.windows) };
}

/** The app's processes running `binary`, asked of the operating system. */
export function appProcesses(binary: string): AppProcesses {
  return appProcessesOf(processes(), binary);
}

/**
 * The app's windows running `binary`, by pid: the app under test is one of them. Its session
 * host runs the same binary and is left out (#2384).
 */
export function running(binary: string): number[] {
  return appProcesses(binary).windows;
}

/** One look at a process: how much memory it holds, how many threads and files it has open. */
export interface Sample {
  pid: number;
  /** Resident memory, in kilobytes. */
  rssKb: number;
  threads: number;
  /** Open descriptors: `/proc` on Linux, `lsof` on macOS (SC-15); `null` where neither answers. */
  descriptors: number | null;
}

/** A look at `pid`, or `null` when it is not running. */
export function sample(pid: number): Sample | null {
  const rss = Number.parseInt(ps(["-o", "rss=", "-p", String(pid)]).trim(), 10);
  if (!Number.isFinite(rss)) {
    return null;
  }
  let threads: number;
  let descriptors: number | null = null;
  if (process.platform === "linux") {
    threads = Number.parseInt(ps(["-o", "nlwp=", "-p", String(pid)]).trim(), 10);
    try {
      descriptors = readdirSync(`/proc/${pid}/fd`).length;
    } catch {
      descriptors = null;
    }
  } else {
    // macOS has no thread-count column; `-M` prints one line per thread under a header.
    threads =
      ps(["-M", "-p", String(pid)])
        .split("\n")
        .filter((line) => line.trim() !== "").length - 1;
    if (process.platform === "darwin") descriptors = descriptorsOnMacos(pid);
  }
  return { pid, rssKb: rss, threads, descriptors };
}

/**
 * How many descriptors `lsof -F f` listed: its `f` lines that are numbers. The others name what
 * is not a descriptor: the working directory (`cwd`), the program's text (`txt`) and each
 * library mapped into it.
 */
export function descriptorsListed(listed: string): number {
  return listed.split("\n").filter((line) => /^f\d+$/.test(line)).length;
}

/**
 * The app's open descriptors on macOS, which has no `/proc`: `lsof`, the one tool every Mac
 * has that lists another process's descriptors. About a tenth of a second at fifty chats, and
 * a stress look is every ten. `-n -P` keep it from resolving names, `-a -d 0-999999` from
 * listing anything but numbered descriptors in the first place. `null` when it does not
 * answer, as the Linux count is when `/proc` does not.
 */
function descriptorsOnMacos(pid: number): number | null {
  try {
    const listed = execFileSync(
      "lsof",
      ["-n", "-P", "-a", "-p", String(pid), "-d", "0-999999", "-F", "f"],
      { encoding: "utf8", timeout: 20_000 },
    );
    return descriptorsListed(listed);
  } catch (err) {
    // `lsof` exits 1 when it warns about one entry it could not read; what it listed is
    // still the answer.
    const stdout = (err as { stdout?: string }).stdout;
    return stdout ? descriptorsListed(stdout) : null;
  }
}

/**
 * The web content process's memory and the harnesses', for `stress.jsonl` (SC-1, #679). Each
 * field feeds a row of ADR 0086's performance budgets:
 *
 * - `webContentKb`, `webContentProcesses`: the web content processes' resident memory, and how
 *   many there are. **M2 on Linux**, and with the app's own `rssKb` M1 there.
 * - `webContentFootprintKb`, `webContentFootprintPeakKb`: the same processes by `footprint`, on
 *   macOS only: their `phys_footprint` summed, and the `phys_footprint_peak` of the largest.
 *   **M2 on macOS is the peak**, the number ADR 0082's 1,529 MB was. Resident memory reads far
 *   low there. So each system is its own baseline: M2 on macOS and M2 on Linux are not one
 *   series. `null` on Linux.
 * - `footprintError`: why `footprint` gave no reading for some process, in its own words, or
 *   `null`. A missing reading is never a silent `null`.
 * - `harnessKb`, `harnessLargestKb`: the harnesses' memory with their child runs, together and
 *   the largest one. F1's measure. In CI the harness is the fake one, so these say what the
 *   harness side of charter's tree costs, and the real harnesses' F1 values come from the
 *   release scale run (#814), which takes them the same way.
 *
 * Two `ps` calls and, on macOS, one `footprint` call for every web content process together.
 * When footprint cannot find a pid, `ps` is asked again: a web content process that exited in
 * between is dropped from the reading and from `webContentProcesses`, and footprint is asked
 * once more for those still alive (`footprintsOfLiving`).
 */
export interface WebAndHarnessMemory {
  webContentKb: number;
  webContentProcesses: number;
  webContentFootprintKb: number | null;
  webContentFootprintPeakKb: number | null;
  footprintError: string | null;
  harnessKb: number;
  harnessLargestKb: number;
}

/** One `footprint` call for every pid in `pids`, as text: what it printed, both streams. */
function footprint(pids: number[]): string {
  const answered = spawnSync(
    "footprint",
    pids.flatMap((pid) => ["-p", String(pid)]),
    {
      encoding: "utf8",
      timeout: 20_000,
    },
  );
  if (answered.error !== undefined) return `footprint: ${answered.error.message}`;
  return `${answered.stderr ?? ""}\n${answered.stdout ?? ""}`;
}

/**
 * Footprint's reading of the web content processes `pids`. A pid it could not read is checked
 * against a fresh listing: one that has exited is dropped (`footprintsOfTheLiving`), and those
 * still alive are asked once more. Footprint failing a live pid twice is the error.
 */
function footprintsOfLiving(pids: number[]): LivingFootprints {
  let said = footprint(pids);
  const first = parseFootprints(said, pids);
  if (first.error === null) return { ...first, gone: [], unread: [] };
  const alive = () => processes().map((row) => row.pid);
  const unread = footprintsOfTheLiving(said, pids, alive()).unread;
  if (unread.length > 0) said = `${said}\n${footprint(unread)}`;
  return footprintsOfTheLiving(said, pids, alive());
}

/** The web content process's and the harnesses' memory, for the app at `pid`. */
export function webAndHarnessMemory(pid: number): WebAndHarnessMemory {
  const rows = processes();
  const web = webContent(rows, pid, process.platform);
  let webContentFootprintKb: number | null = null;
  let webContentFootprintPeakKb: number | null = null;
  let footprintError: string | null = null;
  let webContentProcesses = web.pids.length;
  if (process.platform === "darwin") {
    if (web.pids.length === 0) {
      footprintError = "no web content process was found to ask footprint about";
    } else {
      const read = footprintsOfLiving(web.pids);
      const each = Object.values(read.byPid);
      footprintError = read.error;
      webContentProcesses -= read.gone.length;
      if (each.length > 0) {
        webContentFootprintKb = each.reduce((sum, one) => sum + one.footprintKb, 0);
        webContentFootprintPeakKb = each.reduce((most, one) => Math.max(most, one.peakKb), 0);
      }
    }
  }
  const harness = harnessMemory(rows, pid);
  return {
    webContentKb: web.rssKb,
    webContentProcesses,
    webContentFootprintKb,
    webContentFootprintPeakKb,
    footprintError,
    harnessKb: harness.rssKb,
    harnessLargestKb: harness.largestKb,
  };
}

/** Adds one line of JSON to a file in `logs/`, for a run to be read back afterwards. */
export function logLine(file: string, record: object): void {
  mkdirSync(LOGS, { recursive: true });
  appendFileSync(join(LOGS, file), `${JSON.stringify({ at: Date.now(), ...record })}\n`);
}

/** Files in `dir` this run left behind whose name `wanted` accepts, newest last. */
function leftBehind(dir: string, wanted: (name: string) => boolean): string[] {
  try {
    return readdirSync(dir)
      .filter(wanted)
      .map((name) => join(dir, name))
      .filter((path) => statSync(path).mtimeMs >= RUN_STARTED)
      .sort((a, b) => statSync(a).mtimeMs - statSync(b).mtimeMs);
  } catch {
    return [];
  }
}

function run(program: string, args: string[]): string {
  try {
    return execFileSync(program, args, { encoding: "utf8", timeout: 20_000 });
  } catch (err) {
    const failed = err as { stdout?: string; message?: string };
    return failed.stdout || `(${program} ${args.join(" ")}: ${failed.message ?? "failed"})`;
  }
}

/**
 * Writes down what can still be learned after a test failed: whether the app is alive, how
 * big it was, whether the operating system killed it or recorded it crashing, and what a
 * panic hook wrote on its way out. Returns where it wrote.
 *
 * Called for every failed test, because a test that fails because the app died looks, from
 * the test, like any other: an element that never came.
 */
export function collectEvidence(app: string, why: string): string {
  mkdirSync(LOGS, { recursive: true });
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const report = join(LOGS, `evidence-${stamp}.txt`);
  const lines: string[] = [`what failed: ${why}`, `app: ${app}`];

  const { windows: pids, sessionHosts } = appProcesses(app);
  if (pids.length === 0) {
    lines.push("the app is NOT running: the process is gone, not only its WebDriver server");
  }
  for (const pid of pids) {
    lines.push(`the app is running as pid ${pid}: ${JSON.stringify(sample(pid))}`);
    lines.push("  so what stopped answering was inside the process, not the process itself");
  }
  for (const pid of sessionHosts) {
    lines.push(`its session host is running as pid ${pid}: ${JSON.stringify(sample(pid))}`);
  }
  if (sessionHosts.length === 0) {
    lines.push("no session host is running (chats ran in the window, or the host is gone)");
  }
  lines.push(`fake harnesses running: ${harnessesRunning()}`);

  lines.push(
    existsSync(PANIC_LOG)
      ? `a panic was written down: see ${PANIC_LOG}`
      : "no panic was written down (a signal, or the operating system, ended it — or it lives)",
  );

  if (process.platform === "darwin") {
    // A crash leaves `purlis-app-*.ips`; a kill for memory leaves `JetsamEvent-*.ips`, which
    // names the processes it killed only inside it — look for the app's name by file name
    // alone and a memory kill is missed.
    const reports = [
      join(homedir(), "Library", "Logs", "DiagnosticReports"),
      "/Library/Logs/DiagnosticReports",
    ].flatMap((dir) =>
      leftBehind(dir, (name) => name.startsWith("purlis-app") || name.startsWith("JetsamEvent")),
    );
    for (const found of reports) {
      const kept = join(LOGS, found.split("/").pop() ?? "report.ips");
      try {
        copyFileSync(found, kept);
        lines.push(`crash or memory-kill report, kept as ${kept}`);
      } catch {
        lines.push(`crash or memory-kill report at ${found} (could not be copied)`);
      }
    }
    if (reports.length === 0) {
      lines.push("no crash report and no memory-kill report was written during this run");
    }
  } else if (process.platform === "linux") {
    // The kernel says when it killed something for memory, or when a process faulted.
    const kernel = run("sudo", ["-n", "dmesg", "--ctime"])
      .split("\n")
      .filter((line) => /oom|killed process|segfault|charter/i.test(line));
    lines.push("what the kernel said about memory kills and faults:");
    lines.push(...(kernel.length > 0 ? kernel.slice(-40) : ["(nothing)"]));
  }

  writeFileSync(report, `${lines.join("\n")}\n`);
  return report;
}
