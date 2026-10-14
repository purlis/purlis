// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";

/**
 * **The core's answers come back in the order the window asked, where that order matters**
 * (#1949).
 *
 * The core answers a command that touches the disk, git or a process off the window's thread
 * (`off_the_main_thread.rs`), so the window keeps drawing while it works. Such commands no
 * longer answer in the order they were asked: a read asked just before a change can answer just
 * after it, and draw what the change had already undone. A window regaining focus as a notice's
 * button is pressed does exactly that: the focus reads the sandbox's state again, the press
 * changes it.
 *
 * So the commands that read and change the same files are one lane each, per project, and a
 * lane keeps the order a synchronous core kept **between a change and everything else**:
 *
 * - a **read** is sent once the last change asked before it in its lane has answered, and runs
 *   beside every other read: the forty panes a switch draws ask theirs together;
 * - a **change** is sent once everything asked before it in its lane has answered, reads
 *   included.
 *
 * One lane per set of files, never a file in two lanes: a read in one lane would not wait for a
 * change in another. Commands in no lane are sent at once, as before. A lane's first argument is
 * always the project (a path, for this machine's pins).
 */
export const LANES = {
  /**
   * The project's sandbox: its manifest and local settings, this machine's answers and grants
   * (`sandbox-grants`), the persona hosts allowed here, the network record, and a chat's grants
   * and blocks. One lane, since an Allow on a block or of a persona's hosts is read back by the
   * sandbox's state and the persona grants alike.
   */
  sandbox: {
    reads: [
      "sandboxState",
      "personaGrantsHeld",
      "sandboxNetwork",
      "chatNetwork",
      "sandboxKeptBlocked",
      "sandboxGrants",
    ],
    changes: [
      "answerSandboxOffer",
      "acknowledgeProjectHosts",
      "acknowledgeProjectPresets",
      "allowProjectExpects",
      "declineProjectExpects",
      "allowPersonaHosts",
      "allowPersonaGrants",
      // It drops a listed folder that resolves elsewhere now, so it writes as it reads.
      "grantableFolders",
      "listGrantableFolder",
      "unlistGrantableFolder",
      "allowBlockedHost",
      "allowSandboxBlock",
      "keepSandboxBlock",
      "forgetSandboxBlock",
      "allowSandboxBlockForTasks",
      "keepSandboxBlockForTasks",
      "revokeSandboxGrant",
    ],
  },
  /**
   * The dispatch grants: the project's committed grants, this machine's acceptances and nevers,
   * the dispatches waiting on the person, and what was refused while nobody was there. One lane,
   * since an Allow or a Never in any one of them is read back by all.
   */
  dispatch: {
    reads: [
      "dispatchGrantsNeeded",
      "dispatchStanding",
      "dispatchAway",
      "dispatchGrants",
      "dispatchArrival",
    ],
    changes: [
      "allowDispatch",
      "allowDispatchAnywhere",
      "neverDispatch",
      "keepDispatchBlocked",
      "liftDispatchNever",
      "revokeDispatchToAny",
      "declineProjectDispatch",
      "declineProjectDispatchIn",
      "removeDormantDispatch",
      "allowDispatchAway",
      "neverDispatchAway",
      "dismissDispatchAway",
      "revokeDispatchGrant",
      "addDispatchGrant",
      "acceptProjectDispatch",
      "acceptProjectDispatchIn",
      "allowDispatchToAny",
      "setDispatchWorkspace",
      "giveBackDispatch",
      "answerDispatchArrival",
      "dispatchGoneTold",
    ],
  },
  /** The project's work log: the item a chat is linked to. */
  workLink: { reads: ["chatWorkItem"], changes: ["chatWorkLink", "chatWorkUnlink"] },
  /** This machine's store: the person's pins in a project, by its path. */
  pins: {
    reads: ["planePins"],
    changes: ["pinProject", "pinWorkspace", "arrangeWorkspacePins", "pinOnThisMachine"],
  },
} as const satisfies Record<
  string,
  { reads: readonly (keyof typeof commands)[]; changes: readonly (keyof typeof commands)[] }
>;

type Ask = (...args: unknown[]) => Promise<unknown>;

/** What one lane of one project waits on: its last change, and every read asked since. */
type Lane = { change: Promise<unknown> | undefined; reads: Set<Promise<unknown>> };

const lanes = new Map<string, Lane>();

/** `answer`, settled either way, for what waits on it. */
const settledOf = (answer: Promise<unknown>) =>
  answer.then(
    () => undefined,
    () => undefined,
  );

/**
 * `ask` in `lane`, as a read or a change: see {@link LANES}. Answered as `ask` answers, a
 * refusal or an error included. Sent in the same tick where it waits for nothing.
 */
export function inOrder(lane: string, kind: "read" | "change", ask: Ask): Ask {
  return (...args: unknown[]) => {
    const key = `${lane}\u0000${String(args[0])}`;
    const now = lanes.get(key) ?? { change: undefined, reads: new Set() };
    lanes.set(key, now);
    const before = kind === "read" ? [now.change] : [now.change, ...now.reads];
    const waits = before.filter((one): one is Promise<unknown> => one !== undefined);
    const answer = waits.length === 0 ? ask(...args) : Promise.all(waits).then(() => ask(...args));
    const settled = settledOf(answer);
    if (kind === "read") {
      now.reads.add(settled);
      void settled.then(() => now.reads.delete(settled));
    } else {
      now.change = settled;
      now.reads = new Set();
      void settled.then(() => {
        if (now.change === settled) now.change = undefined;
      });
    }
    void settled.then(() => {
      if (lanes.get(key) === now && now.change === undefined && now.reads.size === 0)
        lanes.delete(key);
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
  for (const [lane, { reads, changes }] of Object.entries(LANES)) {
    for (const name of reads) asks[name] = inOrder(lane, "read", asks[name].bind(commands));
    for (const name of changes) asks[name] = inOrder(lane, "change", asks[name].bind(commands));
  }
}
