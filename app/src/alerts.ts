import { useCallback, useEffect, useState } from "react";
import { commands, type PlaneAlerts, type PlaneId } from "./bindings";
import { GIT, usePlaneChanged } from "./planeChanged";
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
 */
export function useAlerts(planes: readonly PlaneId[]): {
  reading: AlertsReading;
  reread: () => void;
} {
  const [reading, setReading] = useState<AlertsReading>({ at: "reading" });
  const [asked, setAsked] = useState(0);
  const holding = planes.join("\n");
  const changesOnDisk = usePlaneChanged(planes, GIT);

  // Written as `Extensions`'s first read is: the command's own promise, a `gone` flag, and the
  // state set inside the callback — so an answer that lands after a newer ask began, or after
  // the window let go, sets nothing.
  useEffect(() => {
    let gone = false;
    void commands
      .alertsEverywhere()
      .then((answer) => {
        if (gone) return;
        if (answer.status === "error") setReading({ at: "failed", why: answer.error });
        else if (!Array.isArray(answer.data))
          setReading({ at: "failed", why: "purlis did not answer with a reading" });
        else setReading({ at: "read", planes: answer.data });
      })
      .catch((err: unknown) => {
        if (!gone) setReading({ at: "failed", why: String(err) });
      });
    return () => {
      gone = true;
    };
  }, [holding, asked, changesOnDisk]);

  useEffect(() => {
    const again = () => setAsked((n) => n + 1);
    window.addEventListener("focus", again);
    const stop = everyWhileShown(REREAD_EVERY_MS, again);
    return () => {
      window.removeEventListener("focus", again);
      stop();
    };
  }, []);

  const reread = useCallback(() => setAsked((n) => n + 1), []);
  return { reading, reread };
}
