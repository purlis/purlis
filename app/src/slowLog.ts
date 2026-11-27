// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  Profiler,
  createContext,
  createElement,
  useContext,
  useLayoutEffect,
  useMemo,
  useRef,
  type ProfilerOnRenderCallback,
  type ReactNode,
} from "react";
import { commands, type SlowMoment } from "./bindings";
import { thisWindow } from "./here";

/**
 * **A slow moment the person felt is written down, so it can be read afterwards** (#4725).
 *
 * The window says an answer is late only at 20 s (`answerWatch.ts`), and a stall of its own
 * thread it never says at all. A second's wait for a tab, or half a second of a frozen window
 * while fifty chats work, is felt and then gone, with nothing left to look at. Each one is
 * written here, as one JSON line in `slow.jsonl` in the app's log folder, which the core keeps
 * to the newest of each kind up to that kind's cap (`purlis_core::slowlog`'s `Kind::keeping`,
 * which also holds each kind's threshold): machine-local, never in a plane.
 *
 * - **An answer that took {@link SLOW_MS} or longer**, with its command, how long it took, when it
 *   was asked and how many chats were hot. Every command `answerWatch` times, so the ones
 *   that wait on a person or the network by design (`WAITS_BY_DESIGN`) are left out here too.
 * - **A stall of {@link STALL_MS} or longer**: a frame that came that long after the one before
 *   it ({@link watchForStalls}), with how many chats were hot, the tab in front, and whether
 *   a view of a chat was being opened, and **what the window's thread did in that frame**
 *   (felt speed round 4, H9; #7891), so its causes can be told apart: `render_ms`, what React's
 *   renders of the measured regions took of the gap, and the top {@link TOP} of each: `react`,
 *   each region rendered ({@link Measured}) by its own time, with the time of the regions in it
 *   and its commits; `events`, the core's events handled, each once, and `listeners`, the calls
 *   of the window's listeners they made ({@link listenerCalled}); `answers`, the commands whose
 *   answers were handled; and `marks`, what happened that costs a frame ({@link marked}).
 * - **The window's visibility**: whether it is visible as it starts watching, then each change
 *   to hidden or shown, with when ({@link watchForStalls}).
 *
 * Each stall and visibility row names its window by label ({@link thisWindow}), so with the
 * window split a stall can be read beside the visibility of its own window.
 *
 * Only written: nothing in the window reads it back or says it. Writing it is the one command
 * that is not timed (`answerWatch.ts`), so a slow write is not a slow answer to write.
 */

/** An answer this long or longer is written down: the core's threshold for an answer, which a
 *  quicker one is not even sent to. */
export const SLOW_MS = 1_000;

/** A gap between frames this long or longer is a stall, and written down: the core's threshold
 *  for a stall. */
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

/** What one region's renders added up to over a frame: its whole time, its own and its commits. */
type Region = { ms: number; own: number; commits: number };

/** What the window's thread did since the last frame, each by name. */
const sinceLastFrame = {
  react: new Map<string, Region>(),
  events: new Map<string, number>(),
  listeners: new Map<string, number>(),
  answers: new Map<string, number>(),
  marks: new Map<string, number>(),
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

function region(name: string): Region {
  let one = sinceLastFrame.react.get(name);
  if (one === undefined) {
    one = { ms: 0, own: 0, commits: 0 };
    sinceLastFrame.react.set(name, one);
  }
  return one;
}

/** The {@link Measured} region a component is drawn in, by its id. */
const InRegion = createContext<string | undefined>(undefined);

/**
 * React's word on a commit of the {@link Measured} region `id`, drawn inside the region `parent`:
 * its time added to its own and to its whole, and taken from its parent's own. React calls a
 * region's `onRender` for every commit that rendered anything in it, so a parent's whole always
 * holds its child's, and its own is what is left: PlaneView's body apart from the side views.
 */
function committed(id: string, parent: string | undefined, took: number): void {
  const one = region(id);
  one.ms += took;
  one.own += took;
  one.commits += 1;
  if (parent !== undefined) region(parent).own -= took;
}

/**
 * **A region of the window whose renders a stall can name** (H9, #7891): React's `Profiler`, its
 * time added up by `id` over the frame, with the regions drawn inside it told apart. Only the
 * main regions wear one, so a stall names a handful of places, not every component.
 */
export function Measured({ id, children }: { id: string; children?: ReactNode }) {
  const parent = useContext(InRegion);
  const onRender = useMemo<ProfilerOnRenderCallback>(
    () => (name, _phase, actualDuration) => committed(name, parent, actualDuration),
    [parent],
  );
  return createElement(
    InRegion.Provider,
    { value: id },
    createElement(Profiler, { id, onRender }, children),
  );
}

/** The regions rendered since the last frame, by their own time, the most first, and what they
 *  took all told. */
function rendered(): { render_ms: number; react: SlowStall["react"] } {
  const all = [...sinceLastFrame.react].map(([name, { ms, own, commits }]) => ({
    name,
    ms,
    own: Math.max(own, 0),
    commits,
  }));
  return {
    render_ms: Math.round(all.reduce((sum, one) => sum + one.own, 0)),
    react: all
      .sort((a, b) => b.own - a.own)
      .slice(0, TOP)
      .map(({ name, ms, own, commits }) => ({
        name,
        ms: Math.round(ms),
        self_ms: Math.round(own),
        commits,
      })),
  };
}

type SlowStall = Extract<SlowMoment, { kind: "stall" }>;

/**
 * The window handled one of the core's events, `name`, with one call: counted as an event and
 * as a listener's call, for the frame it is handled in. What a terminal's piece of output is.
 */
export function eventHandled(name: string): void {
  add(sinceLastFrame.events, name, 1);
  add(sinceLastFrame.listeners, name, 1);
}

/** The events whose listeners were called in the task running now, by name. */
const heardThisTask = new Set<string>();

/**
 * **A listener of the core's event `name` was called** (`here.ts`): counted as a listener's
 * call, and as the event only the first time in the task (#7891). Tauri calls every listener of
 * one event in one go, a loop in one script (`tauri::event`'s emit), and each event in a script
 * of its own, so the listeners of one event are called before any microtask runs: one cleared
 * then sees each event once, however many listeners it calls.
 */
export function listenerCalled(name: string): void {
  add(sinceLastFrame.listeners, name, 1);
  if (heardThisTask.has(name)) return;
  if (heardThisTask.size === 0) queueMicrotask(() => heardThisTask.clear());
  heardThisTask.add(name);
  add(sinceLastFrame.events, name, 1);
}

/** Something that costs a frame happened, `name` (a chat's pane mounted, a tab switched):
 *  counted for the frame it happens in. */
export function marked(name: string): void {
  add(sinceLastFrame.marks, name, 1);
}

/** Marks `name` as the component is mounted, in the frame it is first drawn in: a chat's pane. */
export function useMarkedOnMount(name: string): void {
  useLayoutEffect(() => marked(name), [name]);
}

/**
 * Marks `name` each time `value` changes to another that is not `undefined` once the component is
 * drawn, never for the value it is first drawn with: a tab switched, and not the first one shown.
 * Marked as the change is committed, in the frame it is drawn in.
 */
export function useMarkedOnChange(name: string, value: unknown): void {
  const was = useRef(value);
  useLayoutEffect(() => {
    if (Object.is(was.current, value)) return;
    was.current = value;
    if (value !== undefined) marked(name);
  }, [name, value]);
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

/** How often the thread is asked whether it is free, while stalls are watched. */
export const PROBE_MS = 100;

/**
 * **What says how long the thread last went without answering**: the longest gap, since it was
 * last asked, between runs of a timer asked to run every {@link PROBE_MS}. A gap in the frames
 * the thread answered all through was not the thread's: the webview withheld its frames, as it
 * does while the screen is locked or the display asleep, and for a window covered whole that
 * WebKit does not report as hidden (felt speed round 4, H9). Ten timer runs a second, and nothing else.
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
 * visible all the same, and time away is not a stall the person felt. Each change of visibility
 * is written too, and the visibility the window starts with (#7891), so a log begins with the
 * window's state and a stall can be read beside whether the person could see it. On macOS
 * WebKit fires `visibilitychange` for a window minimised or hidden, and, by its occlusion
 * handling, for one covered whole by other windows, which is unverified until checked by hand;
 * what WebKitGTK on Linux reports is not known. Answers how to stop watching (a test's).
 */
export function watchForStalls(
  frames: Frames = (next) => requestAnimationFrame(next),
  page: Page = document,
): () => void {
  if (watching) return watching;
  const thread = threadProbe();
  let last: number | undefined;
  let stopped = false;
  const label = thisWindow();
  let visible = page.visibilityState === "visible";
  write({ kind: "visibility", at: Date.now(), window: label, visible });
  // No frame comes while the window is hidden, so the frame before a hide cannot be what the
  // first frame after it is measured from: the change itself starts the clock again.
  const startAgain = () => {
    last = undefined;
    const now = page.visibilityState === "visible";
    if (now === visible) return;
    visible = now;
    write({ kind: "visibility", at: Date.now(), window: label, visible });
  };
  page.addEventListener("visibilitychange", startAgain);
  const onFrame = (time: number) => {
    if (stopped) return;
    const unanswered = thread.longestUnanswered();
    if (page.visibilityState !== "visible") last = undefined;
    else {
      if (last !== undefined && time - last >= STALL_MS) {
        write({
          kind: "stall",
          ms: Math.round(time - last),
          at: Date.now(),
          window: label,
          hot: hotChats(),
          front: front?.name ?? null,
          opening: opening > 0,
          dark: unanswered < STALL_MS,
          ...rendered(),
          events: top(sinceLastFrame.events),
          listeners: top(sinceLastFrame.listeners),
          answers: top(sinceLastFrame.answers),
          marks: top(sinceLastFrame.marks),
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
