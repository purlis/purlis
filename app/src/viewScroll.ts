import { useLayoutEffect, type RefObject } from "react";

/**
 * **A moved view keeps where it was scrolled** (#1752, P-3). "Move to panel" and "Move to
 * sidebar" draw a view in another region, so it is mounted again there and every scroller in it
 * starts at the top. Each view's scrollers are remembered as they scroll, by their place under
 * the view's own element, and a view drawn in another region than it was is scrolled back.
 *
 * **Only a move**: what is kept is the region it was in and the frame that drew it. A view drawn
 * again in the same region (a project brought back to the front draws a new frame) starts as it
 * always did, and a frame's memory dies with it. In memory only, never written: a layout file
 * holds where a view is, not how far down it was read.
 */

/** Where a view's scrollers were, by each one's path of child places under the view. */
type Kept = { region: string; at: Map<string, { top: number; left: number }> };

const kept = new Map<string, Kept>();

/** The child places from `root` down to `el`: `"0/2/1"`, or `undefined` when it is not inside. */
function pathOf(root: Element, el: Element): string | undefined {
  const places: number[] = [];
  let at: Element | null = el;
  while (at !== null && at !== root) {
    const parent: Element | null = at.parentElement;
    if (parent === null) return undefined;
    places.unshift(Array.prototype.indexOf.call(parent.children, at) as number);
    at = parent;
  }
  return at === root ? places.join("/") : undefined;
}

/** The element at `path` under `root`, if it is still there. */
function atPath(root: Element, path: string): Element | undefined {
  let at: Element | undefined = root;
  for (const place of path === "" ? [] : path.split("/")) {
    at = at?.children[Number(place)];
    if (at === undefined) return undefined;
  }
  return at;
}

/**
 * Remembers the scrollers of `view`, drawn in `region` by the frame `frame`, and scrolls them
 * back when the view is drawn in another region than it was last.
 */
export function useViewScroll(
  frame: string,
  view: string,
  region: string,
  element: RefObject<HTMLElement | null>,
): void {
  useLayoutEffect(() => {
    const root = element.current;
    if (root === null) return;
    const key = `${frame}\u0000${view}`;
    const was = kept.get(key);
    const mine: Kept = { region, at: was?.region === region ? was.at : new Map() };
    kept.set(key, mine);
    let frames = 0;
    let asked = 0;
    if (was !== undefined && was.region !== region && was.at.size > 0) {
      // Its content may draw a frame or two after it is mounted (a list read again), so the
      // scroll is put back until each scroller is there, a few frames at most.
      const restore = () => {
        let missing = 0;
        for (const [path, to] of was.at) {
          const el = atPath(root, path);
          if (el === undefined) {
            missing++;
            continue;
          }
          el.scrollTop = to.top;
          el.scrollLeft = to.left;
          mine.at.set(path, to);
          // A scroller whose content is not that tall yet stops short: tried again next frame.
          if (el.scrollTop < to.top) missing++;
        }
        if (missing > 0 && ++frames < 5) asked = requestAnimationFrame(restore);
      };
      restore();
    }
    const scrolled = (event: Event) => {
      const el = event.target;
      if (!(el instanceof Element)) return;
      const path = pathOf(root, el);
      if (path !== undefined) mine.at.set(path, { top: el.scrollTop, left: el.scrollLeft });
    };
    // Capture: a scroll event does not bubble, and the view's scrollers are inside it.
    root.addEventListener("scroll", scrolled, true);
    return () => {
      cancelAnimationFrame(asked);
      root.removeEventListener("scroll", scrolled, true);
    };
  }, [frame, view, region, element]);
}

/** Forgets what the frame `frame` kept, once it is gone: a project put behind draws a new one. */
export function forgetFrameScroll(frame: string): void {
  for (const key of kept.keys()) if (key.startsWith(`${frame}\u0000`)) kept.delete(key);
}
