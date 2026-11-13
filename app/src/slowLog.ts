// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Profiler, createElement, type ProfilerOnRenderCallback, type ReactNode } from "react";
import { commands, type SlowMoment } from "./bindings";

/**
 * **A slow moment the person felt is written down, so it can be read afterwards** (#4725).
 *
 * The window says an answer is late only at 20 s (`answerWatch.ts`), and a stall of its own
 * thread it never says at all. A second's wait for a tab, or half a second of a frozen window
 * while fifty chats work, is felt and then gone, with nothing left to look at. Each one is
 * written here, as one JSON line in `slow.jsonl` in the app's log folder, which the core keeps
 * to its newest 1,000 (`purlis_core::slowlog`): machine-local, never in a plane.
 *
 * - **An answer that took {@link SLOW_MS} or longer**, with its command, how long it took, when it
 *   was asked and how many chats were hot. Every command `answerWatch` times, so the ones
 *   that wait on a person or the network by design (`WAITS_BY_DESIGN`) are left out here too.
 * - **A stall of {@link STALL_MS} or longer**: a frame that came that long after the one before
 *   it ({@link watchForStalls}), with how many chats were hot, the tab in front, and whether
 *   a view of a chat was being opened, and **what the window's thread did in that frame**
 *   (felt speed round 4, H9), the top {@link TOP} of each: `react`, the commit time of each
 *   region drawn ({@link Measured}); `events`, the core's events handled, by name and count
 *   ({@link eventHandled}); `answers`, the commands whose answers were handled, by name and
 *   count; and `scripts`, the scripts the webview says made a long frame, where it says so
 *   (Chromium's long-animation-frame timing; WebKit gives none).
 *
 * Only written: nothing in the window reads it back or says it. Writing it is the one command
 * that is not timed (`answerWatch.ts`), so a slow write is not a slow answer to write.
 */

/** An answer this long or longer is written down. */
export const SLOW_MS = 1_000;

/** A gap between frames this long or longer is a stall, and written down. */
export const STALL_MS = 200;

/** The command that writes a slow moment: never timed itself. */
export const WRITES_THE_LOG = "recordSlow";

/** What says how many chats one project holds are hot, each counted as it is asked. */
const hotSources = new Set<() => number>();

/** The name of the tab in front of the project in front, as the person reads it. */
let front: { name: string | undefined } | undefined;

/** How many views of a chat are being opened now. */
let opening = 0;

/** How many chats are hot (CONTEXT.md) in every project the window holds. */
export function hotChats(): number {
  let hot = 0;
  for (const count of hotSources) hot += count();
  return hot;
}

/** `count` says how many chats of one project are hot, until the answer is called. */
export function countsHotChats(count: () => number): () => void {
  hotSources.add(count);
  return () => void hotSources.delete(count);
}

/** The tab named `name` is in front, until the answer is called (and nothing has replaced it). */
export function frontTabIs(name: string | undefined): () => void {
  const mine = { name };
  front = mine;
  return () => {
    if (front === mine) front = undefined;
  };
}

/** A view of a chat is being opened, until the answer is called. Calling it again does nothing. */
export function viewOpening(): () => void {
  opening += 1;
  let done = false;
  return () => {
    if (done) return;
    done = true;
    opening -= 1;
  };
}

/** How many of each kind of culprit a stall names: the largest. */
export const TOP = 5;

/** What the window's thread did since the last frame, each a name and what it adds up to. */
const sinceLastFrame = {
  react: new Map<string, number>(),
  events: new Map<string, number>(),
  answers: new Map<string, number>(),
  scripts: new Map<string, number>(),
};

function add(tally: Map<string, number>, name: string, by: number): void {
  tally.set(name, (tally.get(name) ?? 0) + by);
}

/** The {@link TOP} largest of `tally`, largest first, each rounded to a whole number. */
function top(tally: Map<string, number>): { name: string; n: number }[] {
  return [...tally]
    .sort((a, b) => b[1] - a[1])
    .slice(0, TOP)
    .map(([name, n]) => ({ name, n: Math.round(n) }));
}

function startFrameAgain(): void {
  for (const tally of Object.values(sinceLastFrame)) tally.clear();
}

/** React's word on a commit of a {@link Measured} region: its time, added to the frame's. */
const committed: ProfilerOnRenderCallback = (id, _phase, actualDuration) => {
  add(sinceLastFrame.react, id, actualDuration);
};

/**
 * **A region of the window whose commits a stall can name** (H9): React's `Profiler`, its
 * time added up by `id` over the frame. Only the main regions wear one, so a stall names a
 * handful of places, not every component.
 */
export function Measured({ id, children }: { id: string; children?: ReactNode }) {
  return createElement(Profiler, { id, onRender: committed }, children);
}

/** The window handled the core's event `name`: counted for the frame it is handled in. */
export function eventHandled(name: string): void {
  add(sinceLastFrame.events, name, 1);
}

function write(moment: SlowMoment): void {
  // A log that cannot be written is let go: it is a record, never a reason to stop the window.
  void commands.recordSlow(moment).catch(() => {});
}

/** `command` took `ms` to answer: written down when that is {@link SLOW_MS} or longer. */
export function answered(command: string, ms: number, askedAt: number): void {
  add(sinceLastFrame.answers, command, 1);
  if (ms < SLOW_MS) return;
  write({ kind: "answer", command, ms: Math.round(ms), at: askedAt, hot: hotChats() });
}

/** What schedules the next frame, and what it hands the callback: the browser's own. */
type Frames = (next: (time: number) => void) => unknown;

/** What says whether the window is visible, and tells when that changes: the document. */
type Page = EventTarget & { readonly visibilityState: DocumentVisibilityState };

/** One script the webview says ran in a long animation frame (its `PerformanceScriptTiming`). */
export type LongScript = {
  readonly startTime: number;
  readonly duration: number;
  readonly invoker: string;
  readonly sourceFunctionName: string;
  readonly sourceURL: string;
};

/** A long animation frame the webview reported, with the scripts it says ran in it. */
export type LongFrame = {
  readonly startTime: number;
  readonly duration: number;
  readonly scripts: readonly LongScript[];
};

/** What hands over the long frames reported since it was last asked, none where none are. */
type LongFrames = () => readonly LongFrame[];

/**
 * **The webview's long animation frames**, where it reports them: Chromium's
 * `long-animation-frame` timing (WebKit has none, and then this is nothing). The entries are
 * taken as a stall is written, not as they are delivered, since the stalled frame's is still
 * queued then.
 */
function webviewLongFrames(): LongFrames {
  const types: readonly string[] =
    typeof PerformanceObserver === "undefined"
      ? []
      : (PerformanceObserver.supportedEntryTypes ?? []);
  if (!types.includes("long-animation-frame")) return () => [];
  let kept: LongFrame[] = [];
  const observer = new PerformanceObserver((list) => {
    kept.push(...(list.getEntries() as unknown as LongFrame[]));
    // Only the frames of the last few seconds can be in a stall.
    if (kept.length > 50) kept = kept.slice(-50);
  });
  observer.observe({ type: "long-animation-frame", buffered: false });
  return () => {
    const taken = [...kept, ...(observer.takeRecords() as unknown as LongFrame[])];
    kept = [];
    return taken;
  };
}

/** A script's name: its function and file where the webview says them, else what called it. */
function scriptName(script: LongScript): string {
  if (!script.sourceFunctionName) return script.invoker || "(script)";
  const file = script.sourceURL.split(/[?#]/)[0].split("/").pop();
  return file ? `${script.sourceFunctionName} (${file})` : script.sourceFunctionName;
}

/** How often the thread is asked whether it is free, while stalls are watched. */
export const PROBE_MS = 100;

/**
 * **What says how long the thread last went without answering**: the longest gap, since it was
 * last asked, between runs of a timer asked to run every {@link PROBE_MS}. A gap in the frames
 * the thread answered all through was not the thread's: the webview withheld its frames, as it
 * does while the screen is locked, the display asleep or the window covered (felt speed round
 * 4, H9). Ten timer runs a second, and nothing else.
 */
function threadProbe(): { longestUnanswered: () => number; stop: () => void } {
  let alive = performance.now();
  let longest = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const probe = () => {
    const now = performance.now();
    longest = Math.max(longest, now - alive);
    alive = now;
    timer = setTimeout(probe, PROBE_MS);
  };
  timer = setTimeout(probe, PROBE_MS);
  return {
    longestUnanswered: () => {
      const now = performance.now();
      const answer = Math.max(longest, now - alive);
      longest = 0;
      alive = now;
      return answer;
    },
    stop: () => clearTimeout(timer),
  };
}

let watching: (() => void) | undefined;

/**
 * Watches the window's frames for the life of the window: a frame that comes {@link STALL_MS} or
 * more after the one before it is a stall, and written down with what was going on.
 *
 * A window that is not visible is sent no frames on purpose, so the gap across a hide is not a
 * stall: the clock starts again, at every change of visibility, with the next frame the visible
 * window is sent. A stall the thread was free all through ({@link threadProbe}) is still
 * written, marked `dark`: a webview that sends no frames while the screen is locked says it is
 * visible all the same, and time away is not a stall the person felt. Answers how to stop
 * watching (a test's).
 */
export function watchForStalls(
  frames: Frames = (next) => requestAnimationFrame(next),
  page: Page = document,
  longFrames?: LongFrames,
): () => void {
  if (watching) return watching;
  const longFramesSince = longFrames ?? webviewLongFrames();
  const thread = threadProbe();
  let last: number | undefined;
  let stopped = false;
  // No frame comes while the window is hidden, so the frame before a hide cannot be what the
  // first frame after it is measured from: the change itself starts the clock again.
  const startAgain = () => {
    last = undefined;
  };
  page.addEventListener("visibilitychange", startAgain);
  const onFrame = (time: number) => {
    if (stopped) return;
    const unanswered = thread.longestUnanswered();
    if (page.visibilityState !== "visible") last = undefined;
    else {
      if (last !== undefined && time - last >= STALL_MS) {
        const from = last;
        for (const frame of longFramesSince()) {
          if (frame.startTime + frame.duration <= from || frame.startTime >= time) continue;
          for (const script of frame.scripts)
            add(sinceLastFrame.scripts, scriptName(script), script.duration);
        }
        write({
          kind: "stall",
          ms: Math.round(time - last),
          at: Date.now(),
          hot: hotChats(),
          front: front?.name ?? null,
          opening: opening > 0,
          dark: unanswered < STALL_MS,
          react: top(sinceLastFrame.react).map(({ name, n }) => ({ name, ms: n })),
          events: top(sinceLastFrame.events),
          answers: top(sinceLastFrame.answers),
          scripts: top(sinceLastFrame.scripts).map(({ name, n }) => ({ name, ms: n })),
        });
      }
      last = time;
    }
    startFrameAgain();
    frames(onFrame);
  };
  frames(onFrame);
  watching = () => {
    stopped = true;
    thread.stop();
    page.removeEventListener("visibilitychange", startAgain);
    watching = undefined;
  };
  return watching;
}

/**
 * **The log's recent entries on this machine, newest last**, or `null` when the core could not
 * read it: what a person's diagnostics carry (#4774). An empty list is a log with nothing in it.
 */
export async function recentSlowLog(): Promise<readonly SlowMoment[] | null> {
  try {
    const answer = await commands.slowLogRecent();
    return answer.status === "ok" ? answer.data : null;
  } catch {
    return null;
  }
}
