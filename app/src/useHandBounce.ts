// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { type BounceEvent, type Holds, reduceBounce, STILL } from "./handBounce";
import { quietNow } from "./quiet";
import { motionReduced } from "./theme/motion";

/**
 * **The ✋'s bounce in one project's view** (#6343, ADR 0121 amended 2026-11-12): the schedule of
 * `handBounce.ts`, fed by its timer and by what the view is told.
 *
 * - `waiting`: whether a Blocking ask waits in this project (the ✋'s count is above 0).
 * - `arrived`: a number that goes up as asks arrive here (`useAsksArrival`'s `here`).
 * - `answered`: the newest answer noted in this project (`useAnswered`); a new one is an answer.
 * - `opened`: whether Asks is open in this project, in front; its opening resets the backoff.
 * - `inFront`: whether this project is the one in front; behind it, nothing bounces.
 *
 * The holds are read as a bounce would play, never drawn from: Quiet as the core said it last,
 * reduced motion, and whether this window has the focus. `bouncing` is true from a bounce until
 * `bounced` is told its animation ended.
 */
export function useHandBounce({
  waiting,
  arrived,
  answered,
  opened,
  inFront,
}: {
  waiting: boolean;
  arrived: number;
  answered: unknown;
  opened: boolean;
  inFront: boolean;
}): { bouncing: boolean; bounced: () => void } {
  const [schedule, dispatch] = useReducer(reduceBounce, STILL);
  const front = useRef(inFront);
  useEffect(() => {
    front.current = inFront;
  }, [inFront]);
  const holds = useCallback(
    (): Holds => ({
      quiet: quietNow().quiet,
      reduced: motionReduced(),
      behind: !front.current || !document.hasFocus(),
    }),
    [],
  );
  const tell = useCallback((event: BounceEvent) => dispatch(event), []);

  useEffect(() => tell({ kind: "waiting", waiting, at: Date.now() }), [waiting, tell]);

  // Each of these is told on a change, never as it is first read.
  const seen = useRef({ arrived, answered, opened });
  useEffect(() => {
    const was = seen.current;
    seen.current = { arrived, answered, opened };
    const at = Date.now();
    if (arrived !== was.arrived) tell({ kind: "arrived", at, holds: holds() });
    if (answered !== was.answered) tell({ kind: "answered", at });
    if (opened && !was.opened) tell({ kind: "opened", at });
  }, [arrived, answered, opened, tell, holds]);

  const due = schedule.due;
  useEffect(() => {
    if (due === null) return;
    // Told no earlier than it was due, so a timer that fires a hair early still moves it on.
    const timer = setTimeout(
      () => tell({ kind: "tick", at: Math.max(Date.now(), due), holds: holds() }),
      Math.max(0, due - Date.now()),
    );
    return () => clearTimeout(timer);
  }, [due, tell, holds]);

  const [ended, setEnded] = useState(0);
  const bounces = useRef(schedule.bounces);
  useEffect(() => {
    bounces.current = schedule.bounces;
  }, [schedule.bounces]);
  const bounced = useCallback(() => setEnded(bounces.current), []);
  return { bouncing: schedule.bounces > ended, bounced };
}
