import { useState } from "react";
import type { Said } from "./actions";

/**
 * **The window's line says the newest answer** (#1669): the project in front's last answer or
 * the window's own, whichever came last.
 *
 * The line once preferred the project's answer whenever it had one, so a project that had said
 * anything earlier (a copied path, a pinned workspace) hid every later sentence of the window's:
 * what opening a repo did to the project's forges, a refused project drag, an away answer. Now
 * a new window answer is drawn over an older project one, and a new project answer over an
 * older window one. Bringing another project to the front draws that project's answer, or the
 * window's where it has none, as before.
 */

/** What the line was last drawn from, and what it drew. */
export type SaidSeen = {
  front: string | undefined;
  project: Said | undefined;
  window: Said | undefined;
  latest: Said | undefined;
};

/** What the line draws now, given what it drew before (`was`) and the answers as they are. */
export function nextSaid(
  was: SaidSeen | undefined,
  front: string | undefined,
  project: Said | undefined,
  window: Said | undefined,
): SaidSeen {
  let latest: Said | undefined;
  if (was === undefined) latest = project ?? window;
  // A new answer of the window's is the newest thing said, whichever project is in front.
  else if (window !== was.window && window !== undefined) latest = window;
  // Another project in front, or a new answer from the one in front: its answer, or the
  // window's where it has none.
  else if (front !== was.front || project !== was.project) latest = project ?? window;
  // The window's answer was taken back: the project's, if it has one.
  else if (window !== was.window) latest = project;
  else latest = was.latest;
  return { front, project, window, latest };
}

/** The answer the window's line draws: the newer of the project in front's and the window's. */
export function useLatestSaid(
  front: string | undefined,
  project: Said | undefined,
  window: Said | undefined,
): Said | undefined {
  const [seen, setSeen] = useState(() => nextSaid(undefined, front, project, window));
  if (seen.front === front && seen.project === project && seen.window === window)
    return seen.latest;
  // Worked out during the render, as React's "information from previous renders" pattern does,
  // so no frame draws the older answer first.
  const next = nextSaid(seen, front, project, window);
  setSeen(next);
  return next.latest;
}
