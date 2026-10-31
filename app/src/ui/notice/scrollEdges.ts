// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, type RefObject } from "react";

/** Less than this past an edge is a scaled screen's rounding, never more to scroll. */
const ROUNDING = 1;

/**
 * **Marks the edges a scrolling box has more past** (#3168): `data-more-above` while it is
 * scrolled down from its top, `data-more-below` while more is under what it shows, neither once
 * everything fits. What it draws for them is its stylesheet's (`notice.css`, the band's fade).
 *
 * It looks again on a scroll, on a resize of the box or of anything it holds, and when what it
 * holds comes, goes or is hidden, so a warning arriving in a band at rest marks it too. The
 * attributes are set on the box itself, so nothing is drawn again for them.
 */
export function useScrollEdges(at: RefObject<HTMLElement | null>) {
  useLayoutEffect(() => {
    const box = at.current;
    if (box === null) return;
    const mark = () => {
      const past = box.scrollHeight - box.clientHeight - box.scrollTop;
      box.toggleAttribute("data-more-above", box.scrollTop > ROUNDING);
      box.toggleAttribute("data-more-below", past > ROUNDING);
    };
    mark();
    box.addEventListener("scroll", mark, { passive: true });
    const sized = new ResizeObserver(mark);
    const watchSizes = () => {
      sized.disconnect();
      sized.observe(box);
      for (const one of box.children) sized.observe(one);
    };
    watchSizes();
    // A warning that comes or goes, or is put behind "+N more" (`hidden`): watched for those
    // alone, so the marks it sets are not themselves heard.
    const changed = new MutationObserver(() => {
      watchSizes();
      mark();
    });
    changed.observe(box, { childList: true, subtree: true, attributeFilter: ["hidden"] });
    return () => {
      box.removeEventListener("scroll", mark);
      sized.disconnect();
      changed.disconnect();
    };
  }, [at]);
}
