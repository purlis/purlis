// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/pool-limits, never merged to main).
 *
 * A pool's limits, on fake data held in memory: what each is called, its unit, its default, the
 * team's highest allowed on a cluster pool, what the runner uses of it now, and the plain words
 * for a value.
 */

export type PoolId = "docker" | "cluster";
export type Kind = "size" | "time" | "count" | "cpus" | "pixels" | "rate";
export type Area = "code" | "signin" | "runner";

export interface LimitDef {
  id: string;
  label: string;
  kind: Kind;
  area: Area;
  /** Defaults in the base unit: MiB, seconds, or the count itself. */
  byDefault: Record<PoolId, number>;
  /** The team's highest allowed, on a cluster pool. */
  teamHighest: number;
  /** PROTOTYPE: the highest a Docker pool takes, where this prototype needs one. Not decided. */
  dockerHighest?: number;
  /** Shapes the runner: applies when the runner is next replaced. */
  shaping?: boolean;
  clusterOnly?: boolean;
  advanced?: boolean;
  /** PROTOTYPE: its default is a placeholder; the real one comes from measurement. */
  placeholder?: boolean;
  live: { said: string; use: number };
}

const MIB = 1;
const GIB = 1024;
const MIN = 60;
const HOUR = 3600;

const same = (value: number): Record<PoolId, number> => ({ docker: value, cluster: value });

export const LIMITS: readonly LimitDef[] = [
  {
    id: "code-size",
    label: "Largest code sent to a runner",
    kind: "size",
    area: "code",
    byDefault: same(GIB),
    teamHighest: 2 * GIB,
    live: { said: "Code sent", use: 310 * MIB },
  },
  {
    id: "code-time",
    label: "Longest code send",
    kind: "time",
    area: "code",
    byDefault: same(10 * MIN),
    teamHighest: 30 * MIN,
    dockerHighest: 2 * HOUR,
    live: { said: "Last send took", use: 3 * MIN + 40 },
  },
  {
    id: "profile-size",
    label: "Most a profile may send: size",
    kind: "size",
    area: "signin",
    byDefault: same(256 * MIB),
    teamHighest: 512 * MIB,
    dockerHighest: 4 * GIB,
    placeholder: true,
    live: { said: "Profile sent", use: 18 * MIB },
  },
  {
    id: "profile-files",
    label: "Most a profile may send: files",
    kind: "count",
    area: "signin",
    byDefault: same(20000),
    teamHighest: 50000,
    placeholder: true,
    live: { said: "Files sent", use: 1840 },
  },
  {
    id: "signin-wait",
    label: "Longest wait for a sign-in",
    kind: "time",
    area: "signin",
    byDefault: same(15 * MIN),
    teamHighest: 30 * MIN,
    live: { said: "Last sign-in took", use: 2 * MIN },
  },
  {
    id: "vouch-wait",
    label: "Longest wait for a cluster runner to be vouched for",
    kind: "time",
    area: "signin",
    byDefault: same(5 * MIN),
    teamHighest: 10 * MIN,
    clusterOnly: true,
    live: { said: "Last vouching took", use: 4 },
  },
  {
    id: "cpus",
    label: "CPUs",
    kind: "cpus",
    area: "runner",
    byDefault: { docker: 2, cluster: 4 },
    teamHighest: 8,
    shaping: true,
    live: { said: "CPUs in use", use: 0.6 },
  },
  {
    id: "memory",
    label: "Memory",
    kind: "size",
    area: "runner",
    byDefault: { docker: 4 * GIB, cluster: 8 * GIB },
    teamHighest: 16 * GIB,
    shaping: true,
    live: { said: "Memory in use", use: 1229 * MIB },
  },
  {
    id: "processes",
    label: "Processes",
    kind: "count",
    area: "runner",
    byDefault: same(1024),
    teamHighest: 4096,
    shaping: true,
    live: { said: "Processes running", use: 143 },
  },
  {
    id: "tmp",
    label: "/tmp",
    kind: "size",
    area: "runner",
    byDefault: same(GIB),
    teamHighest: 4 * GIB,
    shaping: true,
    live: { said: "/tmp in use", use: 210 * MIB },
  },
  {
    id: "suspend",
    label: "Suspend after, with no link from this machine",
    kind: "time",
    area: "runner",
    byDefault: { docker: 24 * HOUR, cluster: 8 * HOUR },
    teamHighest: 72 * HOUR,
    shaping: true,
    live: { said: "No link for", use: 12 * MIN },
  },
  {
    id: "frames",
    label: "Browser frames a second",
    kind: "rate",
    area: "runner",
    byDefault: same(15),
    teamHighest: 30,
    shaping: true,
    live: { said: "Browser frames a second", use: 14 },
  },
  {
    id: "width",
    label: "Browser width",
    kind: "pixels",
    area: "runner",
    byDefault: same(1280),
    teamHighest: 1920,
    shaping: true,
    live: { said: "Browser width", use: 1280 },
  },
  {
    id: "answer-wait",
    label: "Longest wait for a runner's answer",
    kind: "time",
    area: "runner",
    byDefault: same(30),
    teamHighest: 2 * MIN,
    advanced: true,
    live: { said: "Slowest answer this hour", use: 1.2 },
  },
];

export const limit = (id: string): LimitDef => {
  const found = LIMITS.find((one) => one.id === id);
  if (found === undefined) throw new Error(`no limit ${id}`);
  return found;
};

export const POOLS: Record<PoolId, { name: string; said: string; chats: number }> = {
  docker: { name: "builds", said: "On your Docker", chats: 2 },
  cluster: { name: "team-gpu", said: "Pool gpu in team-prod, namespace dev", chats: 1 },
};

export const AREAS: readonly { id: Area; label: string; help: string }[] = [
  { id: "code", label: "Code", help: "What purlis sends to the runner." },
  { id: "signin", label: "Sign-in", help: "Profiles going to the runner and signing in there." },
  { id: "runner", label: "Runner", help: "What the runner is made with." },
];

export const APPLIES_REPLACED = "Applies when this runner is next replaced";
export const APPLIES_NEXT = "Applies from the next attempt";

/** Each unit a kind is typed in, and how many base units it holds. */
export const UNITS: Record<Kind, readonly { unit: string; of: number }[]> = {
  size: [
    { unit: "MiB", of: MIB },
    { unit: "GiB", of: GIB },
  ],
  time: [
    { unit: "s", of: 1 },
    { unit: "min", of: MIN },
    { unit: "h", of: HOUR },
  ],
  count: [{ unit: "", of: 1 }],
  cpus: [{ unit: "CPUs", of: 1 }],
  pixels: [{ unit: "pixels", of: 1 }],
  rate: [{ unit: "a second", of: 1 }],
};

const round = (value: number) => String(Math.round(value * 100) / 100);

/** A value as a box shows it: the number and the unit it reads best in. */
export function boxed(kind: Kind, base: number): { text: string; unit: string } {
  if (kind === "size" && base >= GIB) return { text: round(base / GIB), unit: "GiB" };
  if (kind === "size") return { text: round(base), unit: "MiB" };
  if (kind === "time" && base >= HOUR && base % HOUR === 0)
    return { text: round(base / HOUR), unit: "h" };
  if (kind === "time" && base >= MIN && base % MIN === 0)
    return { text: round(base / MIN), unit: "min" };
  if (kind === "time") return { text: round(base), unit: "s" };
  return { text: round(base), unit: UNITS[kind][0].unit };
}

/** A value in words. */
export function say(kind: Kind, base: number): string {
  if (kind === "time" && base > MIN && base < HOUR && base % MIN !== 0) {
    return `${Math.floor(base / MIN)} min ${round(base % MIN)} s`;
  }
  const { text, unit } = boxed(kind, base);
  const number = kind === "count" || kind === "pixels" ? Number(text).toLocaleString("en") : text;
  if (kind === "cpus") return `${number}`;
  if (kind === "rate") return number;
  return unit === "" ? number : `${number} ${unit}`;
}

export const defaultOf = (def: LimitDef, pool: PoolId) => def.byDefault[pool];
export const highestOf = (def: LimitDef, pool: PoolId) =>
  pool === "cluster" ? def.teamHighest : def.dockerHighest;

export type Judged =
  | { state: "empty" }
  | { state: "refused"; why: string }
  | { state: "ok"; value: number };

/** What the box holds, as the pool would take it. Empty is the default. */
export function judge(def: LimitDef, pool: PoolId, text: string, unit: string): Judged {
  const typed = text.trim();
  if (typed === "") return { state: "empty" };
  if (!/^\d+(\.\d+)?$/.test(typed)) return { state: "refused", why: "Write a number." };
  const whole = def.kind === "count" || def.kind === "pixels" || def.kind === "rate";
  if (whole && typed.includes(".")) return { state: "refused", why: "Write a whole number." };
  const of = UNITS[def.kind].find((one) => one.unit === unit)?.of ?? 1;
  const value = Number(typed) * of;
  if (value <= 0) return { state: "refused", why: "Write a number above 0." };
  const highest = highestOf(def, pool);
  if (highest !== undefined && value > highest) {
    return {
      state: "refused",
      why:
        pool === "cluster"
          ? `Refused: your team's highest allowed on this pool is ${say(def.kind, highest)}.`
          : `Refused: the highest allowed is ${say(def.kind, highest)}.`,
    };
  }
  return { state: "ok", value };
}

export const SUGGESTIONS = ["none", "profile", "wait", "fixed"] as const;
export type Suggestion = (typeof SUGGESTIONS)[number];
export const SUGGESTION_SAID: Record<Suggestion, string> = {
  none: "No error",
  profile: "Profile over its limit",
  wait: "A timed wait",
  fixed: "A fixed limit",
};

/** The profile the first error is about. Its limit is set low so that it is over. */
export const PROFILE_NEEDS = 18;
export const PROFILE_SET_LOW = 16;
export const PROFILE_SUGGESTS = 32;
