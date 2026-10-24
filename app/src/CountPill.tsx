// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useRef } from "react";
import { fitPillOn } from "./wholeWays";
import { Count, CountGroup } from "./ui/badge/Badge";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **The one pill a tab's counts are drawn in** (qw337): how many chats are open there, in the
 * neutral style, and, where any of them needs you, how many, as a second segment of the same
 * pill in the needs-you colour. A workspace's tab, the plane root's and a project's all draw
 * this, and so do the rows of the show-more menus, which draw what their tabs draw.
 *
 * It was two pills side by side, a grey count and a red one, after a broadcast icon and a pin:
 * four marks on one tab (the operator's screenshot, 2026-10-13). One pill reads as one fact
 * with two parts.
 *
 * **One accessible name says both** ({@link countsSaid}), on the pill; the digits are its
 * drawing. It is inside its tab's button, so the tab's own name ends with it.
 *
 * **A chat that needs you is never left unsaid for want of a tab to count** (ADR 0054): where
 * no chat is open in a tab there and one still needs you, the needs-you segment is drawn alone.
 * `chats` is left out by a surface that counts none (a project's tab), which draws that segment
 * alone too. Nothing is drawn where both are nothing.
 *
 * **On a tab too narrow for it and its title, the pill goes and the title stays**
 * (`wholeWays.fitPill`, D-1867-1): measured, as the session tab's chip is.
 */
export function CountPill({ chats, needs }: { chats?: number; needs: number }) {
  const pill = useRef<HTMLSpanElement>(null);
  const drawn = (chats ?? 0) > 0 || needs > 0;
  useLayoutEffect(() => {
    if (!drawn || pill.current === null) return;
    return fitPillOn(pill.current);
  }, [drawn]);
  if (!drawn) return null;
  const said = countsSaid(chats, needs);
  return (
    <Tooltip tip={said}>
      <CountGroup ref={pill} role="img" aria-label={said} data-chats={chats} data-needs={needs}>
        {(chats ?? 0) > 0 && <Count aria-hidden="true">{chats}</Count>}
        {needs > 0 && (
          <Count variant="hot" aria-hidden="true">
            {needs}
          </Count>
        )}
      </CountGroup>
    </Tooltip>
  );
}

/**
 * What a pill is called: `3 chats`, `3 chats, 1 needs you`, and `1 chat needs you` where
 * there is no count of chats to say first. "Chats", the window's word for them everywhere
 * else (the Chats view, the title bar's "2 chats need you").
 */
export function countsSaid(chats: number | undefined, needs: number): string {
  const open = chats ?? 0;
  if (open === 0) return needs === 1 ? "1 chat needs you" : `${needs} chats need you`;
  const many = open === 1 ? "1 chat" : `${open} chats`;
  if (needs === 0) return many;
  return `${many}, ${needs} ${needs === 1 ? "needs" : "need"} you`;
}
