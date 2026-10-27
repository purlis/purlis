// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import {
  commands,
  type BrowserOnADevice,
  type ChatOnADevice,
  type DevicesShown,
  type PlaneId,
  type PoolRequests,
  type RunnerAsked,
  type RunnerOnTheList,
} from "./bindings";

/**
 * **The Devices view, as the window holds it** (#2052, spec #2041 story 54): this machine and
 * every runner it keeps, read once as the view comes on screen and again on Refresh, then each
 * runner asked how it is on a link of its own (`device_runner`), so one that does not answer
 * holds up no other row.
 *
 * **A runner still being asked is not asked again**: a Refresh, or the view opened again, waits
 * on the ask in flight ({@link inFlight}), and the core joins a second ask of one runner to the
 * first, so one runner never has two links open to it for this view. Refresh is always offered,
 * and asks again every runner that is not being asked. **Nothing waits for ever**: an ask not
 * answered within {@link answerWithinMs} says the runner does not answer, and the next Refresh
 * asks it again. An ask purlis could not make is said in purlis's words ({@link Asked}).
 *
 * What is drawn is made here, from plain inputs ({@link arranged}, {@link usageOf},
 * {@link healthSaid}, {@link contactSaid}), so the view only draws it.
 */
type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; shown: DevicesShown };

/** What a runner answered, that it is being asked, or why purlis could not ask it. */
export type Asked = RunnerAsked | "asking" | { failed: string };

/** What `asked` answered, where it did. */
export function answerOf(asked: Asked | undefined): RunnerAsked | undefined {
  return asked === undefined || asked === "asking" || "failed" in asked ? undefined : asked;
}

export type DevicesHeld = {
  read: Read;
  /** What each runner answered, or that it is being asked, by `<pool>/<runner>`. */
  asked: Readonly<Record<string, Asked>>;
};

const START: DevicesHeld = { read: { state: "reading" }, asked: {} };
let held: DevicesHeld = START;
const listeners = new Set<() => void>();
/** The newest read out: an answer to an older one is dropped. */
let reading = 0;

function set(change: (was: DevicesHeld) => DevicesHeld) {
  held = change(held);
  for (const one of listeners) one();
}

/** A runner's key in {@link DevicesHeld.asked}. */
export const runnerKey = (runner: Pick<RunnerOnTheList, "pool" | "runner">) =>
  `${runner.pool}/${runner.runner}`;

type Said<T> = { status: "ok"; data: T } | { status: "error"; error: string };

function caught<T>(sent: Promise<Said<T>>): Promise<Said<T>> {
  return sent.catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
}

/** Each runner's ask in flight, by its key: a second read waits on it rather than asking again. */
const inFlight = new Map<string, Promise<Said<RunnerAsked>>>();

/**
 * How long the window waits on one ask of a runner, in milliseconds: longer than the core takes
 * to give up on its own (each Docker run is held to 20 seconds, a link to two times 20, a joined
 * ask to 60), so this is only for a core that answers nothing at all.
 */
let answerWithinMs = 90_000;

/** For a test: how long an ask is waited on. */
export function answerWithinForTests(ms: number) {
  answerWithinMs = ms;
}

/** For a test: forget every ask in flight, so the next view asks each runner anew. */
export function forgetDevicesForTests() {
  inFlight.clear();
  reading += 1;
}

/** What a runner not answered in time is said to be: not known. */
const UNANSWERED: RunnerAsked = {
  standing: { state: "unanswered" },
  link: null,
  version: null,
  contacted: null,
  holds: { open: [], browsers_open: [], chats: 0, browsers: 0 },
};

/** Asks runner `one` how it is, or waits on the ask of it in flight; at most {@link answerWithinMs}. */
function ask(one: RunnerOnTheList): Promise<Said<RunnerAsked>> {
  const key = runnerKey(one);
  const already = inFlight.get(key);
  if (already !== undefined) return already;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<Said<RunnerAsked>>((resolve) => {
    timer = setTimeout(() => resolve({ status: "ok", data: UNANSWERED }), answerWithinMs);
  });
  const asking = Promise.race([caught(commands.deviceRunner(one.pool, one.runner)), late]).finally(
    () => {
      clearTimeout(timer);
      // Answered or not, the next ask of it asks again.
      if (inFlight.get(key) === asking) inFlight.delete(key);
    },
  );
  inFlight.set(key, asking);
  return asking;
}

/** Reads every device again, then asks each runner how it is. */
export async function rereadDevices() {
  const mine = ++reading;
  const said = await caught(commands.devices());
  if (mine !== reading) return;
  if (said.status === "error") {
    set((was) => ({ ...was, read: { state: "trouble", trouble: said.error } }));
    return;
  }
  const shown = said.data;
  if (typeof shown !== "object" || shown === null || !Array.isArray(shown.placement?.runners)) {
    set((was) => ({
      ...was,
      read: { state: "trouble", trouble: "purlis did not say which devices this machine has." },
    }));
    return;
  }
  const runners = shown.placement.runners;
  set(() => ({
    read: { state: "read", shown },
    asked: Object.fromEntries(runners.map((one) => [runnerKey(one), "asking" as const])),
  }));
  await Promise.all(
    runners.map(async (one) => {
      const answer = await ask(one);
      if (mine !== reading) return;
      const asked: Asked =
        answer.status === "ok" && typeof answer.data === "object" && answer.data !== null
          ? answer.data
          : { failed: answer.status === "error" ? answer.error : "purlis did not say." };
      set((was) => ({ ...was, asked: { ...was.asked, [runnerKey(one)]: asked } }));
    }),
  );
}

function subscribe(listener: () => void) {
  if (listeners.size === 0) {
    held = START;
    void rereadDevices();
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useDevices(): DevicesHeld {
  return useSyncExternalStore(subscribe, () => held);
}

/** One device's row, as the view draws it. */
export type DeviceRow = {
  /** `here` for this machine, else `<pool>/<runner>`. */
  key: string;
  runner?: RunnerOnTheList;
  chats: ChatOnADevice[];
  browsers: BrowserOnADevice[];
};

const sameChat = (one: { plane: PlaneId; session: number }, two: typeof one) =>
  one.plane === two.plane && one.session === two.session;

/** The pool of a device's key: none for this machine. */
const poolOf = (key: string) => (key === "here" ? undefined : key.split("/")[0]);

/**
 * **Each device and the chats and browsers on it** (#2052): where the window started each, unless
 * a runner that answered says it holds it, which puts it on that runner's row: a runner of a pool
 * that moved on keeps the chats and browsers it had (#2865). **A runner is believed only about
 * its own pool**: it never takes a chat or browser off this machine or off another pool's runner
 * (the core holds the same rule, and this is its second guard).
 */
export function arranged(shown: DevicesShown, asked: Readonly<Record<string, Asked>>): DeviceRow[] {
  const said = (key: string) => answerOf(asked[key])?.holds;
  const claims = shown.placement.runners.map((one) => ({
    key: runnerKey(one),
    holds: said(runnerKey(one)),
  }));
  // Where the window placed it, if that runner says it holds it too (N6); else the runner of the
  // same pool that says it holds it; else where the window placed it.
  const moved = (on: string, holds: (one: RunnerAsked["holds"]) => boolean) => {
    const pool = poolOf(on);
    if (pool === undefined) return on;
    const placed = claims.find((one) => one.key === on);
    if (placed?.holds !== undefined && holds(placed.holds)) return on;
    const claim = claims.find(
      (one) => poolOf(one.key) === pool && one.holds !== undefined && holds(one.holds),
    );
    return claim?.key ?? on;
  };
  const chats = [
    ...shown.placement.here.chats.map((chat) => ({ chat, on: "here" })),
    ...shown.placement.runners.flatMap((runner) =>
      runner.on.chats.map((chat) => ({ chat, on: runnerKey(runner) })),
    ),
  ].map(({ chat, on }) => ({
    chat,
    on: moved(on, (holds) => holds.open.some((one) => sameChat(one, chat))),
  }));
  const browsers = [
    ...shown.placement.here.browsers.map((browser) => ({ browser, on: "here" })),
    ...shown.placement.runners.flatMap((runner) =>
      runner.on.browsers.map((browser) => ({ browser, on: runnerKey(runner) })),
    ),
  ].map(({ browser, on }) => ({
    browser,
    on: moved(on, (holds) => holds.browsers_open.includes(browser.program)),
  }));
  const on = (key: string) => ({
    chats: chats.filter((one) => one.on === key).map((one) => one.chat),
    browsers: browsers.filter((one) => one.on === key).map((one) => one.browser),
  });
  return [
    { key: "here", ...on("here") },
    ...shown.placement.runners.map((runner) => ({
      key: runnerKey(runner),
      runner,
      ...on(runnerKey(runner)),
    })),
  ];
}

/**
 * What a runner's row says of how it is: its badge, its first line, and what Docker or the link
 * said, which the row draws apart from purlis's own words, as said elsewhere.
 */
export type Health = { badge: string; line: string; said?: { by: string; text: string } };

/** **How a runner is, in words** (#2052): `asked` as its row says it. */
export function healthSaid(asked: Asked | undefined): Health {
  if (asked === undefined || asked === "asking")
    return { badge: "asking…", line: "Asking the runner how it is…" };
  // In purlis's own words: nothing Docker or a runner said.
  if ("failed" in asked)
    return { badge: "not asked", line: `purlis could not ask this runner: ${asked.failed}` };
  const standing = asked.standing;
  if (standing.state === "unanswered")
    return {
      badge: "does not answer",
      line: "It did not answer in time. purlis asks again when you press Refresh.",
    };
  if (standing.state === "suspended")
    return {
      badge: "suspended",
      line: "Its container is stopped and its home is kept. It starts again when one of its chats is next used.",
    };
  if (standing.state === "unknown")
    return {
      badge: "not read",
      line: "purlis could not read its container. Check that Docker runs, then press Refresh.",
      said: { by: "Docker said", text: standing.why },
    };
  const link = asked.link;
  if (link === null || link.state === "linked") return { badge: "answers", line: "Answers" };
  if (link.state === "key_changed")
    return {
      badge: "key changed",
      line: "It answered with another key than the one this machine paired with, so purlis does not link to it. Remove its pool in Settings › You › This machine › Pools, then add the pool again.",
    };
  // Suspended is said by the container alone: this view's link is never made to one, and so
  // never says so (D-2052-9).
  return {
    badge: "does not answer",
    line: "It does not answer. purlis asks again when you press Refresh.",
    said: link.state === "down" ? { by: "The link said", text: link.why } : undefined,
  };
}

/** **Which version a device runs, against this machine's** (#2052). */
export function versionSaid(runs: string | null, desktop: string): string | undefined {
  if (runs === null) return undefined;
  return runs === desktop ? `purlis ${runs}` : `purlis ${runs}, and this machine runs ${desktop}`;
}

/**
 * **When a runner last answered** (#2052), as this window heard it: a time today, a date and a
 * time before, and none since purlis started said as such, never as a time nobody saw.
 */
export function contactSaid(contacted: number | null, now: Date, locale?: string): string {
  if (contacted === null) return "Not reached since purlis started";
  const at = new Date(contacted * 1000);
  const time = at.toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });
  const today = at.toDateString() === now.toDateString();
  return today
    ? `Last answered at ${time}`
    : `Last answered ${at.toLocaleDateString(locale, { day: "numeric", month: "short" })} at ${time}`;
}

/** Who a pool's row names, where its runners are not known to be the person's own (D-2050-4). */
export const NOT_KNOWN_TO_BE_YOURS = "Not known to be yours";

/** One pool's use by one person, as the usage table's row draws it. */
export type PoolUse = {
  pool: string;
  /** Who claims it: "You" only where its runners are shown to be the person's own. */
  person: string;
  running: number;
  suspended: number;
  /** CPUs requested by its runners that run, as a row says them. */
  cpus: string;
  /** Memory requested by its runners that run, in MiB. */
  memoryMib: number;
  /** How many of its runners have not answered yet: its counts are not whole until none. */
  asking: number;
};

/**
 * **Each pool's use per person** (#2052, story 55): its runners that run and that are suspended,
 * and the CPUs and memory those that run request (a runner requests its pool's limits). A Docker
 * pool on this machine's own engine is the person's (`owned`, D-2050-4), so its one row is
 * "You"; one on an engine elsewhere is not known to be anyone's in particular. Money is never
 * shown: no pool declares a price per hour yet, and a cost is never guessed.
 */
export function usageOf(
  pools: readonly PoolRequests[],
  runners: readonly RunnerOnTheList[],
  asked: Readonly<Record<string, Asked>>,
  owned: boolean,
): PoolUse[] {
  return pools.map((pool) => {
    const of = runners.filter((one) => one.pool === pool.pool);
    const standing = of.map((one) => {
      const answer = asked[runnerKey(one)];
      return answer === undefined || answer === "asking"
        ? "asking"
        : (answerOf(answer)?.standing.state ?? "unknown");
    });
    const running = standing.filter((one) => one === "running").length;
    const millicpus = running * pool.millicpus;
    return {
      pool: pool.pool,
      person: owned ? "You" : NOT_KNOWN_TO_BE_YOURS,
      running,
      suspended: standing.filter((one) => one === "suspended").length,
      cpus: cpusSaid(millicpus),
      memoryMib: running * pool.limits.memory_mib,
      asking: standing.filter((one) => one === "asking").length,
    };
  });
}

/** What a use row's Running cell says: the count, and those not answered yet apart. */
export function runningSaid(use: Pick<PoolUse, "running" | "asking">): string {
  return use.asking === 0 ? `${use.running}` : `${use.running}, ${use.asking} not answered yet`;
}

/** `millicpus`, as a number of CPUs to the thousandth: `2`, `1.5`. */
function cpusSaid(millicpus: number): string {
  const whole = Math.floor(millicpus / 1000);
  const part = millicpus % 1000;
  return part === 0 ? `${whole}` : `${whole}.${`${part}`.padStart(3, "0").replace(/0+$/, "")}`;
}
