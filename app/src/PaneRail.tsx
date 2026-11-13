// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Hand } from "lucide-react";
import type { ReactNode } from "react";
import { paneLinesOf } from "./paneHead";
import type { BandHolds } from "./paneNotices";
import { Button } from "./ui/button/Button";
import { RAIL } from "./paneHeld";

/**
 * **A folded side of a split** (#4766, spec #4750 stories 81 to 83; `prototype/narrow-window`
 * Q3 A): a 40px rail beside the pane that kept the room, or a 32px bar above or below it. One
 * button per folded pane, each with the pane's icon (and its state, where the icon wears it),
 * its notes count and ✋ while it asks, so no pane goes off screen and no ask hides: a rail too
 * short for its panes scrolls. A press opens that pane: it takes the focus, and the side that
 * held it folds instead (`paneFold.ts`). A bar has room for each pane's name; a rail says it in
 * its tooltip.
 *
 * **Drawn from the pane's model** (#5366, `paneHead.paneLinesOf`): a folded pane's head is who it
 * runs as and its name, and its one action its bell, so the rail's button wears the mark and says
 * the name, and the bell's count stands beside it, each where the model keeps it. Its ✋ is no
 * part of a line: an ask never hides. Where the mark wears the chat's state, its badge is that ✋,
 * and the rail draws no second (#5447).
 */

/** One folded pane, as its rail draws it. */
export type RailPane = {
  pane: number;
  /** What the pane holds, by name: its chat, its view, or what an empty pane would do. */
  name: string;
  /** Its icon: a chat's persona and state, a view's mark. */
  mark: ReactNode;
  /** The notes its band held when it was last drawn (`keptHolds`). */
  holds: BandHolds;
  /** The chat it shows asks the person something now. */
  asks: boolean;
  /** **Its mark wears its chat's one state** (#5340), the needs badge among them: then the
   *  rail draws no ✋ of its own beside it (#5447). */
  stated?: boolean;
};

/**
 * **A folded pane: its button, then its count and ✋ drawn beside it.** The button is named by
 * its own words, never an `aria-label` over them, so what is seen is what is heard (WCAG 2.5.3):
 * its name (drawn on a bar, heard on a rail), then "N notices", "folded" and "asks you", heard.
 * The count and ✋ are the rail's own marks, beside the button rather than drawn in it, so the
 * button stays the family's.
 *
 * Its words are one run: each comma is inside the word it follows and each space is text
 * between words, so the button's flex box makes no block (which a name computation spaces
 * either side of) of any one of them.
 */
export function PaneRail({
  as,
  panes,
  onOpen,
}: {
  as: "rail" | "bar";
  panes: readonly RailPane[];
  onOpen: (pane: number) => void;
}) {
  const lines = paneLinesOf("folded", undefined, {});
  const named = lines.head.includes("name");
  const marked = lines.head.includes("persona");
  const belled = lines.actions.includes("bell");
  return (
    <div
      className="pane-rail"
      data-fold={as}
      {...{ [RAIL]: "" }}
      role="group"
      aria-label="Folded panes"
    >
      {panes.map((one) => {
        const notes = belled ? one.holds.notes : 0;
        return (
          <div key={one.pane} className="pane-rail-item" data-pane={one.pane}>
            <Button
              variant="quiet"
              size="sm"
              className="pane-rail-pane"
              tooltip={`Open ${one.name}`}
              icon={marked ? one.mark : undefined}
              onPress={() => onOpen(one.pane)}
            >
              <span className="pane-rail-words">
                {named && (
                  <>
                    <span className={as === "bar" ? "pane-rail-name" : "hidden-words"}>
                      {one.name}
                      <span className="pane-rail-pause">,</span>
                    </span>{" "}
                  </>
                )}
                {notes > 0 && (
                  <>
                    <span className="hidden-words">
                      {notes} {notes === 1 ? "notice," : "notices,"}
                    </span>{" "}
                  </>
                )}
                <span className="hidden-words">{one.asks ? "folded, asks you" : "folded"}</span>
              </span>
            </Button>
            {notes > 0 && (
              <span
                className="pane-rail-count ui-text-meta ui-text-strong"
                data-trouble={one.holds.trouble > 0 ? "" : undefined}
                aria-hidden="true"
              >
                {notes}
              </span>
            )}
            {/* **One mark says it** (#5447, the one-mark rule of #5340): a mark that wears its
                chat's state shows ✋ on its badge already, so the rail's own is drawn only beside
                a mark that cannot. */}
            {one.asks && one.stated !== true && (
              <Hand className="pane-rail-asks" data-mark="needs-you" aria-hidden="true" />
            )}
          </div>
        );
      })}
    </div>
  );
}
