// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import { changesAskedBefore } from "./commandOrder";

/**
 * **An identical read already in flight is asked once, and every caller gets its answer** (felt
 * speed round 2, H6).
 *
 * Every command the window sends enters the core through one queue on its main thread, so each
 * ask waits behind every ask before it, typing included. The window asked the same reads many
 * times in the same moment: `signingWaiting` seventeen times at once, each answered after about
 * 17 s of queueing, `chatSkills` four times, and one of each per pane for the rest of
 * {@link SHARED_READS}. An ask of one of those, with the same arguments as one not yet answered,
 * is handed that one's answer, an error included, instead of being sent again.
 *
 * Nothing is kept past the answer: an ask after it is sent afresh. A read in a lane
 * (`commandOrder.ts`) asked after a change in that lane is never handed the answer of one asked
 * before the change, which may not show it.
 */

/**
 * The reads that are shared: each changes nothing the window could see, so one answer serves
 * every caller asking at once. Never a change, typing, a resize or anything else with an effect
 * of its own (the test holds none is a lane's change, writes a lane's file or waits by design).
 */
export const SHARED_READS: ReadonlySet<keyof typeof commands> = new Set<keyof typeof commands>([
  // It forgets a branch folder that is gone as it reads: a tidy-up that one ask does as well as
  // seventeen.
  "signingWaiting",
  "chatSkills",
  "chatSkillsMissing",
  "tasksSharingAFolder",
  "personaGrantsHeld",
  "skillDrafts",
]);

type Ask = (...args: unknown[]) => Promise<unknown>;

/** `ask`, its answer shared by every identical ask made while it is in flight. */
export function shared(command: string, ask: Ask): Ask {
  const inFlight = new Map<string, Promise<unknown>>();
  return (...args: unknown[]) => {
    const key = `${changesAskedBefore(command, args)}\u0000${JSON.stringify(args)}`;
    const asked = inFlight.get(key);
    if (asked !== undefined) return asked;
    const answer = ask(...args);
    inFlight.set(key, answer);
    const done = () => {
      if (inFlight.get(key) === answer) inFlight.delete(key);
    };
    answer.then(done, done);
    return answer;
  };
}

let sharing = false;

/**
 * Shares each of {@link SHARED_READS}, once, for the life of the window. After the answers are
 * timed (`answerWatch.ts`), so one shared answer is timed once, as the one ask the core was sent.
 */
export function shareTheReads(): void {
  if (sharing) return;
  sharing = true;
  const asks = commands as unknown as Record<string, Ask>;
  for (const name of SHARED_READS) asks[name] = shared(name, asks[name].bind(commands));
}
