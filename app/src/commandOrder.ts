// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";

/**
 * **The core's answers come back in the order the window asked, where that order matters**
 * (#1949).
 *
 * The core answers a command that touches the disk, git or a process off the window's thread
 * (`off_the_main_thread.rs`), so the window keeps drawing while it works. Such commands no
 * longer answer in the order they were asked: a read asked just before a write can answer just
 * after it, and draw what the write had already changed. A window regaining focus as a notice's
 * button is pressed does exactly that: the focus reads the sandbox's state again, the press
 * changes it.
 *
 * So the commands that read and change the same thing are one lane each, per project, and a
 * command in a lane is sent only once the one asked before it in the same lane has answered:
 * the order a synchronous core kept, kept here, without the window waiting for any of it.
 * Commands in no lane are sent at once, as before. A lane's first argument is always the
 * project.
 */
export const LANES = {
  /** The project's sandbox: its state, and every answer that changes it. */
  sandbox: [
    "sandboxState",
    "answerSandboxOffer",
    "acknowledgeProjectHosts",
    "acknowledgeProjectPresets",
    "allowProjectExpects",
    "declineProjectExpects",
    "allowPersonaHosts",
    "grantableFolders",
    "listGrantableFolder",
    "unlistGrantableFolder",
  ],
  /** A chat holding the asking chat's persona grants, and the Allow that ends it. */
  personaGrants: ["personaGrantsHeld", "allowPersonaGrants"],
  /** Settings' dispatch grants beside the named ones, and what changes them. */
  dispatchStanding: [
    "dispatchStanding",
    "liftDispatchNever",
    "revokeDispatchToAny",
    "declineProjectDispatch",
    "declineProjectDispatchIn",
    "removeDormantDispatch",
  ],
  /** The dispatches a chat waits on, and the person's answers to them. */
  dispatchHeld: [
    "dispatchGrantsNeeded",
    "allowDispatch",
    "allowDispatchAnywhere",
    "neverDispatch",
    "keepDispatchBlocked",
  ],
  /** What was refused while nobody was there, and the answers to it. */
  dispatchAway: ["dispatchAway", "allowDispatchAway", "neverDispatchAway", "dismissDispatchAway"],
  /** This machine's network record: what it lists, and the answers kept in it. */
  network: [
    "sandboxNetwork",
    "chatNetwork",
    "sandboxKeptBlocked",
    "allowBlockedHost",
    "allowSandboxBlock",
    "keepSandboxBlock",
    "forgetSandboxBlock",
    "allowSandboxBlockForTasks",
    "keepSandboxBlockForTasks",
  ],
  /** The work item a chat is linked to. */
  workLink: ["chatWorkItem", "chatWorkLink", "chatWorkUnlink"],
  /** The person's pins in a project. */
  pins: ["planePins", "pinProject", "pinWorkspace", "arrangeWorkspacePins"],
} as const satisfies Record<string, readonly (keyof typeof commands)[]>;

type Ask = (...args: unknown[]) => Promise<unknown>;

/** The last ask of each lane of each project, until it has answered. */
const tails = new Map<string, Promise<unknown>>();

/**
 * `ask`, sent once every ask before it in `lane` for the same project has answered. Answered
 * as `ask` answers, a refusal or an error included.
 */
export function inOrder(lane: string, ask: Ask): Ask {
  return (...args: unknown[]) => {
    const key = `${lane}\u0000${String(args[0])}`;
    const before = tails.get(key);
    // Nothing ahead of it: sent now, in the same tick, as it was sent before there were lanes.
    const answer = before === undefined ? ask(...args) : before.then(() => ask(...args));
    const settled = answer.then(
      () => undefined,
      () => undefined,
    );
    tails.set(key, settled);
    void settled.then(() => {
      if (tails.get(key) === settled) tails.delete(key);
    });
    return answer;
  };
}

let kept = false;

/**
 * Puts each command of {@link LANES} in its lane, once, for the life of the window: each is
 * asked through the same function as before. Before `startTheSwitchClock`, so a switch's
 * clock times what the person waited, the lane included.
 */
export function keepTheirOrder(): void {
  if (kept) return;
  kept = true;
  const asks = commands as unknown as Record<string, Ask>;
  for (const [lane, names] of Object.entries(LANES))
    for (const name of names) asks[name] = inOrder(lane, asks[name].bind(commands));
}
