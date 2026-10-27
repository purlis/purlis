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
 * What is drawn is made here, from plain inputs ({@link arranged}, {@link usageOf},
 * {@link healthSaid}, {@link contactSaid}), so the view only draws it.
 */
type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; shown: DevicesShown };

export type DevicesHeld = {
  read: Read;
  /** What each runner answered, or that it is being asked, by `<pool>/<runner>`. */
  asked: Readonly<Record<string, RunnerAsked | "asking">>;
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
      const answer = await caught(commands.deviceRunner(one.pool, one.runner));
      if (mine !== reading) return;
      const asked: RunnerAsked =
        answer.status === "ok" && typeof answer.data === "object" && answer.data !== null
          ? answer.data
          : {
              standing: {
                state: "unknown",
                why: answer.status === "error" ? answer.error : "purlis did not say.",
              },
              link: null,
              version: null,
              contacted: null,
              holds: { open: [], chats: 0, browsers: 0 },
            };
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

/**
 * **Each device and the chats and browsers on it** (#2052): where the window started each, unless
 * a runner that answered says it holds the chat, which puts it on that runner's row: a runner
 * of a pool that moved on keeps the chats it had (#2865), and what a runner says it holds is
 * where the chat is.
 */
export function arranged(
  shown: DevicesShown,
  asked: Readonly<Record<string, RunnerAsked | "asking">>,
): DeviceRow[] {
  const said = (key: string) => {
    const one = asked[key];
    return one === undefined || one === "asking" ? [] : one.holds.open;
  };
  const claimed = shown.placement.runners.flatMap((one) =>
    said(runnerKey(one)).map((chat) => ({ ...chat, key: runnerKey(one) })),
  );
  // Each chat with the device the window placed it on, then the one a runner says holds it.
  const placed = [
    ...shown.placement.here.chats.map((chat) => ({ chat, on: "here" })),
    ...shown.placement.runners.flatMap((runner) =>
      runner.on.chats.map((chat) => ({ chat, on: runnerKey(runner) })),
    ),
  ].map(({ chat, on }) => ({ chat, on: claimed.find((one) => sameChat(one, chat))?.key ?? on }));
  const chatsOn = (key: string) => placed.filter((one) => one.on === key).map((one) => one.chat);
  return [
    { key: "here", chats: chatsOn("here"), browsers: shown.placement.here.browsers },
    ...shown.placement.runners.map((runner) => ({
      key: runnerKey(runner),
      runner,
      chats: chatsOn(runnerKey(runner)),
      browsers: runner.on.browsers,
    })),
  ];
}

/** What a runner's row says of how it is: a badge's word and, where there is more, a line. */
export type Health = { badge: string; line?: string; trouble: boolean };

/** **How a runner is, in words** (#2052): `asked` as its row says it. */
export function healthSaid(asked: RunnerAsked | "asking" | undefined): Health {
  if (asked === undefined || asked === "asking") return { badge: "asking…", trouble: false };
  const standing = asked.standing;
  if (standing.state === "suspended")
    return {
      badge: "suspended",
      line: "Its container is stopped and its home is kept. It starts again when one of its chats is next used.",
      trouble: false,
    };
  if (standing.state === "unknown")
    return {
      badge: "not read",
      line: `purlis could not read its container: ${standing.why}`,
      trouble: true,
    };
  const link = asked.link;
  if (link === null || link.state === "linked") return { badge: "answers", trouble: false };
  if (link.state === "key_changed")
    return {
      badge: "key changed",
      line: "It answered with another key than the one this machine paired with, so purlis does not link to it. Remove its pool in Settings › You › This machine › Pools, then add the pool again.",
      trouble: true,
    };
  return { badge: "does not answer", line: link.why, trouble: true };
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

/** One pool's use by one person, as the usage table's row draws it. */
export type PoolUse = {
  pool: string;
  /** Who claims it: on this machine's Docker, always the person at it. */
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
 * pool on this machine is the person's own, so it has one row, theirs. Money is never shown: no
 * pool declares a price per hour yet, and a cost is never guessed.
 */
export function usageOf(
  pools: readonly PoolRequests[],
  runners: readonly RunnerOnTheList[],
  asked: Readonly<Record<string, RunnerAsked | "asking">>,
): PoolUse[] {
  return pools.map((pool) => {
    const of = runners.filter((one) => one.pool === pool.pool);
    const standing = of.map((one) => {
      const answer = asked[runnerKey(one)];
      return answer === undefined || answer === "asking" ? "asking" : answer.standing.state;
    });
    const running = standing.filter((one) => one === "running").length;
    const millicpus = running * pool.millicpus;
    return {
      pool: pool.pool,
      person: "You",
      running,
      suspended: standing.filter((one) => one === "suspended").length,
      cpus: cpusSaid(millicpus),
      memoryMib: running * pool.limits.memory_mib,
      asking: standing.filter((one) => one === "asking").length,
    };
  });
}

/** `millicpus`, as a number of CPUs to the thousandth: `2`, `1.5`. */
function cpusSaid(millicpus: number): string {
  const whole = Math.floor(millicpus / 1000);
  const part = millicpus % 1000;
  return part === 0 ? `${whole}` : `${whole}.${`${part}`.padStart(3, "0").replace(/0+$/, "")}`;
}
