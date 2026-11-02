// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import writeSites from "./writeSites.json";

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
      "addSandboxHost",
      "removeSandboxHost",
      "confirmSandboxHost",
      "saveProjectSettings",
      "moveProjectSettings",
      "acknowledgeProjectHosts",
      "acknowledgeProjectBrowserSites",
      "acknowledgeProjectPresets",
      "allowProjectExpects",
      "declineProjectExpects",
      "answerTeamSandboxOff",
      "allowPersonaHosts",
      "allowPersonaGrants",
      // A proposed change applied to the project's manifest (#1975).
      "applyProposedChange",
      // It drops a listed folder that resolves elsewhere now, so it writes as it reads.
      "grantableFolders",
      "listGrantableFolder",
      "unlistGrantableFolder",
      "allowBlockedHost",
      // A service on this Mac added by its port on Settings' Network page (#1970).
      "allowSandboxPort",
      "allowSandboxBlock",
      "forgetSandboxBlock",
      "allowSandboxBlockForTasks",
      "revokeSandboxGrant",
      "allowRefusedVault",
      // Always for a persona's browser is kept in this machine's answers (#2241).
      "allowChatBrowser",
      // A browser site for every chat here is kept in this machine's answers (#2018).
      "allowBrowserSites",
      // And one for a browser the person opened (#2176).
      "allowPersonBrowserSites",
    ],
  },
  /**
   * The dispatch grants: the project's committed grants, this machine's acceptances and nevers,
   * the dispatches waiting on the person, and what was refused while nobody was there. One lane,
   * since an Allow or a Never in any one of them is read back by all.
   */
  dispatch: {
    reads: ["dispatchGrantsNeeded", "dispatchAway", "dispatchGrants", "dispatchArrival"],
    changes: [
      // It marks a persona that is gone as it reads, so it writes as it reads.
      "dispatchStanding",
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
    reads: ["planePins", "chatsRunOn"],
    changes: [
      "pinProject",
      "pinWorkspace",
      "arrangeWorkspacePins",
      "pinOnThisMachine",
      // Where a workspace's chats run (#2046): kept beside the pins, in the same store.
      "setChatsRunOn",
    ],
  },
} as const satisfies Record<
  string,
  { reads: readonly (keyof typeof commands)[]; changes: readonly (keyof typeof commands)[] }
>;

/**
 * **Every place in the core that writes a file one of the lanes reads, and the window commands
 * that reach it** (#1949): `writeSites.json`, one table for both sides.
 *
 * - The core's test (`off_the_main_thread/tests/lane_writes.rs`) reads the app crate's own code, its test
 *   modules left out, and fails where the table does not hold what it finds:
 *   - a call that writes one of these files, by the writers it names (and, for the sandbox's
 *     local file, any function of it that is not a known read) and the function it is in;
 *   - a window command that reaches such a call within two calls, by a qualified call
 *     (`module::name(`) or a plain one in the same file, and is not in that site's `by`.
 *
 *   What it does not follow: a command reaching a write through a method (`held.x().y(`), or
 *   deeper than two calls. There the table's `by` is the word, kept by hand.
 * - This module's test holds each command the table names as a writer of a lane's file to that
 *   lane, as a change, or to {@link OUTSIDE_THE_LANES} with why; and each change of a lane to a
 *   file of it.
 *
 * A call graph by name across the whole crate was tried first: it found every command reaching
 * every file through names many modules share (`open`, `held`, `chats`), and so told nothing.
 */
export const WRITE_SITES = writeSites as {
  sections: Record<string, keyof typeof LANES>;
  sites: { at: string; writes: string | null; by?: string[]; why?: string }[];
};

/** Each lane's files, and the window commands that write each, from {@link WRITE_SITES}. */
export const WRITERS: Record<string, Record<string, readonly string[]>> = (() => {
  const out: Record<string, Record<string, string[]>> = {};
  for (const site of WRITE_SITES.sites) {
    if (site.writes === null) continue;
    const lane = WRITE_SITES.sections[site.writes];
    const files = (out[lane] ??= {});
    const writers = (files[site.writes] ??= []);
    for (const command of site.by ?? []) if (!writers.includes(command)) writers.push(command);
  }
  return out;
})();

/** The writers of a lane's files that stay out of it, each with why. */
export const OUTSIDE_THE_LANES: Partial<Record<keyof typeof commands, string>> = {
  keepSandboxBlock:
    "It tells the chat, in fixed words typed into its pane, not to try again: that lands in the " +
    "order of the person's own typing, which a lane behind a slow read would break. Its line in " +
    "the network record is a look back, read again at the next look.",
  keepSandboxBlockForTasks: "As keepSandboxBlock, for each task held on the block.",
  agentIdentityFinish:
    "It asks GitHub for seconds before it writes, which would hold every read of the sandbox's " +
    "lane behind it. Its Allow is one the person made by pressing Finish, and Settings reads the " +
    "standing again after it.",
  addProjectForge:
    "It writes the manifest's forges, which no read of the sandbox's lane draws; Settings reads " +
    "the file again after it. The read it answers with drops the confirmed hosts the file no " +
    "longer lists, as projectSettings does: a tidy-up of what no read of the sandbox's lane " +
    "would count anyway.",
  removeProjectForge: "As addProjectForge.",
  addProjectProfile:
    "It writes the manifest's profiles, which no read of the sandbox's lane draws; Settings reads " +
    "the file again after it. The read it answers with drops the confirmed hosts the file no " +
    "longer lists, as projectSettings does: a tidy-up of what no read of the sandbox's lane " +
    "would count anyway.",
  removeProjectProfile: "As addProjectProfile.",
  renameProjectProfile: "As addProjectProfile.",
  choosePlaneMode:
    "It writes the manifest's `[plane] mode`, which no read of the sandbox's lane draws; the " +
    "saving view draws its own answer.",
  projectSettings:
    "Settings' read of a settings file drops the confirmed hosts the file no longer lists, a " +
    "tidy-up of what no read of the sandbox's lane would count anyway.",
  forgetProject:
    "Settings › This machine names a project by its path, not as the window holds it, waits for " +
    "the answer and reads the store again.",
  locateProject: "As forgetProject.",
};

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
