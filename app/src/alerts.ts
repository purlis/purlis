// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { dequal } from "dequal/lite";
import { commands, type PlaneAlerts, type PlaneId } from "./bindings";
import { AFTER_GIT_MOVED_MS } from "./afterGitMoved";
import { GIT, usePlaneChanged } from "./planeChanged";
import { STILL_LOADING_AFTER } from "./projectLoads";
import { RETURN_FOLD_MS } from "./saving";
import { everyWhileShown } from "./whileShown";

/**
 * **purlis's alerts, for every project this window holds** — the reading each project's Inbox
 * lists its alerts from, as Notices (#1695, `InboxAlerts.tsx`).
 *
 * Alerts are about a PLANE: its pin, its front door, its workspaces' layout, its root. The
 * window reads them once for every project it holds, because the project that has one is often
 * not the one on screen: each Inbox names the other projects that have some. The core decides
 * them — the same `purlis_core::alerts` that draws `purlis statusline`'s rows — and this only
 * asks.
 */
export type AlertsReading =
  /** Nothing has come back yet. */
  | { at: "reading" }
  /** The core's answer, one entry per project it holds. */
  | { at: "read"; planes: PlaneAlerts[] }
  /** The ask itself failed, with the core's words. */
  | { at: "failed"; why: string };

/** How often the reading is refreshed while nothing else asks for it. Alerts move when a
 *  plane's files move — a pin edited, a workspace reinitialised — which is minutes, not
 *  frames; and each reading asks git for one status per project. It is read on the window's
 *  one beat (`whileShown.ts`): not while the window is hidden, and once when it is shown again. */
export const REREAD_EVERY_MS = 60_000;

/**
 * The reading, kept fresh: asked when the projects change, when one of them changes on disk
 * (`planeChanged.ts`, charter-app#264), when the window comes back into focus, when
 * {@link reread} is called (after a fix, or Read again), and once a minute while the window is
 * shown.
 *
 * **A reading is never replaced by "reading…"** once there has been one: the Inbox keeps
 * drawing the last answer while the next is on its way, rather than blanking every minute.
 *
 * **Asked once for a restore, not once per project that joins** (#5325): while `settling`,
 * nothing is asked, since each ask walks every project held. A project that never answers does
 * not keep the others' alerts away: after {@link STILL_LOADING_AFTER} the reading is asked anyway.
 */
export function useAlerts(
  planes: readonly PlaneId[],
  settling = false,
): {
  reading: AlertsReading;
  reread: () => void;
} {
  const [reading, setReading] = useState<AlertsReading>({ at: "reading" });
  /** What is drawn, for a reading to be compared with before anything is set: an update that
   *  changes nothing still draws the hook's owner once before React lets it go. */
  const drawn = useRef(reading);
  const [asked, setAsked] = useState(0);
  const [waited, setWaited] = useState(false);
  useEffect(() => {
    if (!settling) return;
    const enough = setTimeout(() => setWaited(true), STILL_LOADING_AFTER);
    return () => clearTimeout(enough);
  }, [settling]);
  const held = settling && !waited;
  const holding = planes.join("\n");
  // At its own turn after a move (`afterGitMoved.ts`, H8).
  const changesOnDisk = usePlaneChanged(planes, GIT, AFTER_GIT_MOVED_MS.alerts);

  // Written as `Extensions`'s first read is: the command's own promise, a `gone` flag, and the
  // state set inside the callback — so an answer that lands after a newer ask began, or after
  // the window let go, sets nothing.
  //
  // **A reading equal to the one held is not a new one**: it is kept, so nothing drawn from it
  // is drawn again for the same answer (each project's view is held, `memo`).
  /** Asks again, outside a render: set by the effect that asks. */
  const askAgain = useRef<() => void>(() => {});
  useEffect(() => {
    if (held) return;
    let gone = false;
    const read = (now: AlertsReading) => {
      if (dequal(drawn.current, now)) return;
      drawn.current = now;
      setReading(now);
    };
    // Only the newest ask's answer is read: one that lands after a newer ask began says less.
    let newest = 0;
    const ask = () => {
      const mine = (newest += 1);
      void commands
        .alertsEverywhere()
        .then((answer) => {
          if (gone || mine !== newest) return;
          if (answer.status === "error") read({ at: "failed", why: answer.error });
          else if (!Array.isArray(answer.data))
            read({ at: "failed", why: "purlis did not answer with a reading" });
          else read({ at: "read", planes: answer.data });
        })
        .catch((err: unknown) => {
          if (!gone && mine === newest) read({ at: "failed", why: String(err) });
        });
    };
    askAgain.current = ask;
    ask();
    return () => {
      gone = true;
      askAgain.current = () => {};
    };
  }, [holding, asked, changesOnDisk, held]);

  // One read on return (#1392, D-1392-6), as `useRereads` in `saving.ts`: coming back to the
  // window both shows it and focuses it, and a focus or a beat within RETURN_FOLD_MS of the
  // last read asked here is the same return. The monotonic clock, as there.
  // **Neither a focus nor the beat draws the window by itself** (felt speed, round 3, H7): each
  // asks where it is heard, and only a different reading sets anything. Counted in state, each
  // focus and each beat drew the whole window before the answer was even asked for.
  useEffect(() => {
    let last: number | undefined;
    const returned = () => {
      if (last !== undefined && performance.now() - last < RETURN_FOLD_MS) return;
      last = performance.now();
      askAgain.current();
    };
    window.addEventListener("focus", returned);
    const stop = everyWhileShown(REREAD_EVERY_MS, returned);
    return () => {
      window.removeEventListener("focus", returned);
      stop();
    };
  }, []);

  const reread = useCallback(() => setAsked((n) => n + 1), []);
  return { reading, reread };
}
