// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * Stub data for the runner sign-in prototype: two pools, three profiles, and the sign-in states
 * the floating bar steps through. Nothing here calls the core.
 */

export type PrototypeHarness = "claude" | "codex" | "copilot";

export interface PrototypeProfile {
  name: string;
  harness: PrototypeHarness;
  /** What the harness is called in a sentence. */
  harnessSaid: string;
}

export const PROTOTYPE_PROFILES: readonly PrototypeProfile[] = [
  { name: "work", harness: "claude", harnessSaid: "Claude Code" },
  { name: "personal", harness: "claude", harnessSaid: "Claude Code" },
  { name: "oss", harness: "codex", harnessSaid: "Codex" },
];

/** Shown only in the device-code state: none of the three profiles signs in with a device code. */
export const PROTOTYPE_COPILOT: PrototypeProfile = {
  name: "team",
  harness: "copilot",
  harnessSaid: "GitHub Copilot",
};

export interface PrototypePool {
  name: string;
  kind: "docker" | "cluster";
  /** The row's second line, as the real Pools group says it. */
  where: string;
  /** The profiles this pool keeps (`profiles = [..]` in runners.json). */
  profiles: readonly string[];
  /** A cluster pool on a fresh claim: its sign-ins last only for this claim. */
  freshClaim?: boolean;
}

export const PROTOTYPE_POOLS: readonly PrototypePool[] = [
  {
    name: "dev-box",
    kind: "docker",
    where: "2 CPUs, 4096 MiB of memory, 512 processes, 1024 MiB of /tmp",
    profiles: ["work", "personal", "oss"],
  },
  {
    name: "team-gpu",
    kind: "cluster",
    where: "Pool gpu-small in team-prod, namespace purlis-runners",
    profiles: ["work"],
    freshClaim: true,
  },
];

/** The profile a new chat in the open project would get: the default at Add a pool. */
export const PROTOTYPE_DEFAULT_PROFILE = "work";

/** The open project, for the sentences that name it. */
export const PROTOTYPE_PROJECT = "smart-ide";

/** The chat that is moved to a runner where its profile is not signed in. */
export const PROTOTYPE_MOVE = {
  chat: "Fix the flaky upload test",
  profile: "personal",
  to: "team-gpu",
} as const;

/** The sign-in states the bar steps through, in order. */
export const SIGN_IN_STATES = [
  "choose",
  "opening",
  "paste",
  "approve",
  "device",
  "signed-in",
  "terminal",
  "failed",
  "fresh-claim",
] as const;
export type SignInState = (typeof SIGN_IN_STATES)[number];

/** What the bar calls each state. */
export const SIGN_IN_STATE_SAID: Record<SignInState, string> = {
  choose: "Choose profiles",
  opening: "Opening the browser",
  paste: "Paste code (Claude Code)",
  approve: "Waiting for approval (Codex)",
  device: "Device code (Copilot)",
  "signed-in": "Signed in",
  terminal: "Couldn't follow: live terminal",
  failed: "Failed, with the reason",
  "fresh-claim": "Fresh claim note",
};

/** Where the sign-in is asked. */
export const PROTOTYPE_MOMENTS = ["settings", "add-pool", "move"] as const;
export type PrototypeMoment = (typeof PROTOTYPE_MOMENTS)[number];
export const PROTOTYPE_MOMENT_SAID: Record<PrototypeMoment, string> = {
  settings: "Settings › Pools",
  "add-pool": "End of Add a pool",
  move: "Move to runner",
};

/** Which profile on which pool a state is about, and how the others stand meanwhile. */
export interface Focus {
  pool: string;
  profile: PrototypeProfile;
}

export function focusOf(state: SignInState, moment: PrototypeMoment): Focus {
  const byName = (name: string) =>
    PROTOTYPE_PROFILES.find((one) => one.name === name) ?? PROTOTYPE_PROFILES[0];
  if (moment === "move" && state !== "device" && state !== "approve") {
    return { pool: PROTOTYPE_MOVE.to, profile: byName(PROTOTYPE_MOVE.profile) };
  }
  switch (state) {
    case "approve":
    case "failed":
      return { pool: "dev-box", profile: byName("oss") };
    case "device":
      return { pool: "dev-box", profile: PROTOTYPE_COPILOT };
    case "terminal":
      return { pool: "dev-box", profile: byName("personal") };
    case "fresh-claim":
      return { pool: "team-gpu", profile: byName("work") };
    default:
      return { pool: "dev-box", profile: byName("work") };
  }
}

/** How a profile stands on a pool when it is not the one being signed in. */
export function standingOf(pool: string, profile: string): "signed-in" | "signed-out" {
  if (pool === "dev-box" && profile === "personal") return "signed-out";
  if (pool === "dev-box" && profile === "oss") return "signed-out";
  return "signed-in";
}

/** The sentence a pool row says of a profile's standing (the repo's "signed in on <pool>"). */
export function standingSaid(pool: string, standing: "signed-in" | "signed-out"): string {
  return standing === "signed-in"
    ? `Signed in on ${pool}.`
    : `Not signed in on ${pool}: a chat with it does not start there until it is.`;
}

/** The fake output of `claude auth login` that purlis could not follow. */
export const FAKE_TERMINAL_LINES = [
  "$ claude auth login",
  "Welcome to Claude Code v2.4.1",
  "",
  "Select login method:",
  " ❯ 1. Claude account with subscription",
  "   2. Anthropic Console account",
  "   3. 3rd-party platform",
  "",
  "Press ↑↓ to navigate, Enter to select",
];

/** How a profile stands on a pool at a state: signed in or not, or being signed in now. */
export type Standing = "signed-in" | "signed-out" | "signing-in" | "failed";

/** The profiles pool `pool` keeps at a state: its own, plus the one a state brings to it (the
 *  device-code borrow, or the chat moved there with its profile). */
export function profilesOn(
  pool: string,
  focus: Focus,
  moment: PrototypeMoment,
): PrototypeProfile[] {
  const kept = PROTOTYPE_POOLS.find((one) => one.name === pool)?.profiles ?? [];
  const own = PROTOTYPE_PROFILES.filter((one) => kept.includes(one.name));
  const brought =
    focus.pool === pool && !own.some((one) => one.name === focus.profile.name)
      ? [focus.profile]
      : [];
  if (moment === "move" && pool === PROTOTYPE_MOVE.to) {
    const moved = PROTOTYPE_PROFILES.find((one) => one.name === PROTOTYPE_MOVE.profile);
    if (moved && !own.some((one) => one.name === moved.name) && brought.length === 0)
      brought.push(moved);
  }
  return [...own, ...brought];
}

/** How `profile` stands on `pool` at `state`. */
export function standingAt(
  pool: string,
  profile: string,
  state: SignInState,
  focus: Focus,
): Standing {
  if (focus.pool === pool && focus.profile.name === profile) {
    if (state === "signed-in") return "signed-in";
    if (state === "failed") return "failed";
    if (state === "choose") return "signed-out";
    return "signing-in";
  }
  if (pool === PROTOTYPE_MOVE.to && profile === PROTOTYPE_MOVE.profile) return "signed-out";
  return standingOf(pool, profile);
}

/** What a row says of a standing, in a few words. */
export const STANDING_SAID: Record<Standing, string> = {
  "signed-in": "Signed in",
  "signed-out": "Not signed in",
  "signing-in": "Signing in…",
  failed: "Did not sign in",
};

/** At the end of Add a pool: the pool just added (the one the state is about), and the other. */
export function addedPool(state: SignInState): { added: PrototypePool; other: PrototypePool } {
  const at = focusOf(state, "add-pool").pool;
  const added = PROTOTYPE_POOLS.find((one) => one.name === at) ?? PROTOTYPE_POOLS[0];
  const other = PROTOTYPE_POOLS.find((one) => one !== added) ?? PROTOTYPE_POOLS[1];
  return { added, other };
}
