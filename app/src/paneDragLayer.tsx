// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useLayoutEffect, useMemo, useSyncExternalStore } from "react";
import { boxOfRect, cellsIn, type PaneBase, PANES_ROOM, stripTab } from "./paneHeld";
import { type Box, type DropContext, heldPane, judgeDrop, sourceName } from "./paneDrop";
import { heldAt, type PaneDragStore, shownIn, wordsAt } from "./paneDragStore";
import { gridIn, planOf, sameSize, type TerminalCells, wouldBe } from "./panePreview";
import { panes, type Tabs } from "./tabs";
import {
  PanePreviewArriving,
  PanePreviewHeld,
  PanePreviewLine,
  PanePreviewOpening,
  PanePreviewRefused,
  PanePreviewSize,
  PanePreviewWords,
} from "./ui/pane-preview/PanePreview";

/**
 * **What a drag draws, as it goes** (#5291, spec #5288): the panes where the drop would leave them,
 * what has no pane yet, the sizes, the mark on a refused place and the words by the pointer.
 *
 * **It holds a preview, not a change.** The tabs it is given are the real ones, and only ever read:
 * what a drop would do is `paneDrop.ts`'s answer for them. Nothing here reaches `change`, the core
 * or a terminal, so a cancel is letting go of what was written (`paneHeld.ts`).
 *
 * **A component of its own, on a store of its own** (`paneDragStore.ts`), so that a pointer moving
 * draws this, and never the whole window its hook is called in. A label following the pointer is
 * written straight on its element; a target changing draws this and writes the frames.
 */

/** The cells a pane with no terminal yet would have: those of a terminal on screen. */
function anyCells(base: PaneBase): TerminalCells | undefined {
  for (const seat of base.terminals.values()) {
    const cells = cellsIn(seat);
    if (cells !== undefined) return cells;
  }
  return undefined;
}

export function PaneDragLayer({
  store,
  tabs,
  ctx,
}: {
  store: PaneDragStore;
  tabs: Tabs;
  ctx: DropContext;
}) {
  const drag = useSyncExternalStore(store.subscribe, store.get);
  const heldHere = useCallback((label: HTMLElement | null) => store.heldHere(label), [store]);
  const wordsHere = useCallback((label: HTMLElement | null) => store.wordsHere(label), [store]);
  const dragging = drag !== undefined;
  const shown = useMemo(() => shownIn(tabs), [tabs]);
  const front = shown?.front;
  const layout = shown?.layout;

  // **The zones stay on the layout from the drag's start**: retaken only when what is drawn is
  // another tree, a tab brought forward or a pane closing, and when the room itself changes.
  useLayoutEffect(() => {
    if (dragging && !store.measuredIs(tabs)) store.retake(tabs);
  }, [dragging, store, tabs]);
  const room = dragging ? document.querySelector(`[${PANES_ROOM}]`) : null;
  useLayoutEffect(() => {
    if (room === null || typeof ResizeObserver === "undefined") return;
    const watching = new ResizeObserver(() => {
      const was = store.get()?.base?.room;
      const now = room.getBoundingClientRect();
      if (was !== undefined && (was.width !== now.width || was.height !== now.height))
        store.retake(tabs);
    });
    watching.observe(room);
    return () => watching.disconnect();
  }, [room, store, tabs]);

  const over = drag?.over;
  const base = drag?.base;
  const verdict = useMemo(
    () =>
      drag === undefined || over === undefined
        ? undefined
        : judgeDrop(tabs, drag.source, over.target, ctx, over.size),
    [ctx, drag, over, tabs],
  );
  const plan = useMemo(() => {
    if (drag === undefined || over === undefined || layout === undefined) return undefined;
    if (base?.room === undefined || verdict?.ok !== true) return undefined;
    const after = wouldBe(tabs, drag.source, over.target, ctx, over.size);
    if (after === undefined) return undefined;
    const onScreen = new Map(
      panes(layout)
        .filter(({ pane }) => base.zones.has(pane))
        .map(({ pane, content }) => [pane, content]),
    );
    return planOf(after, onScreen, base.room, base.gaps);
  }, [base, ctx, drag, layout, over, tabs, verdict]);
  const overItself = verdict !== undefined && !verdict.ok && verdict.overItself === true;
  // A rail is no place, so it has no verdict: it refuses all the same, and says why (#5415).
  const onRail = verdict === undefined && drag?.onRail === true;
  const refused = (verdict !== undefined && !verdict.ok && !overItself) || onRail;
  const held = drag === undefined ? undefined : heldPane(tabs, drag.source)?.pane;

  useLayoutEffect(() => {
    if (!dragging || base === undefined || front === undefined) return;
    store.held.draw(front, base, plan, held);
    const root = document.documentElement.dataset;
    if (plan === undefined) delete root.panePreviewing;
    else root.panePreviewing = "";
    if (refused) root.paneRefused = "";
    else delete root.paneRefused;
  }, [base, dragging, front, held, plan, refused, store]);

  if (drag === undefined) return null;

  const { x, y } = store.pointer();
  const name = sourceName(tabs, drag.source, ctx);
  const refusedZone =
    refused && over?.target.kind === "pane" ? base?.zones.get(over.target.pane) : undefined;
  const opening = drag.opening === undefined ? undefined : markUnder(drag.opening);
  return (
    <>
      {opening !== undefined && <PanePreviewOpening box={opening} />}
      {over?.line !== undefined && <PanePreviewLine box={over.line} refused={refused} />}
      {refusedZone !== undefined && <PanePreviewRefused box={refusedZone} />}
      {plan?.arriving.map(({ pane, box, content }) => (
        <PanePreviewArriving key={pane} box={box} name={ctx.name(content)} />
      ))}
      {plan?.rails.map(({ box, panes: folded }) => (
        <PanePreviewArriving key={folded.join(" ")} box={box} />
      ))}
      {plan !== undefined &&
        base !== undefined &&
        [...plan.frames].map(([pane, box]) => {
          const cells = cellsIn(base.terminals.get(pane));
          const from = base.zones.get(pane);
          if (cells === undefined || from === undefined || sameSize(from, box)) return null;
          return <PanePreviewSize key={pane} box={box} {...gridIn(box, cells)} />;
        })}
      {plan !== undefined &&
        base !== undefined &&
        plan.arriving.map(({ pane, box, content }) => {
          const cells = anyCells(base);
          if (cells === undefined || content.kind !== "session") return null;
          return <PanePreviewSize key={pane} box={box} {...gridIn(box, cells)} />;
        })}
      {name !== "" && (
        <PanePreviewHeld at={heldAt(x, y)} ref={heldHere}>
          {name}
        </PanePreviewHeld>
      )}
      <PanePreviewWords at={wordsAt(x, y)} refused={refused} ref={wordsHere}>
        {onRail
          ? ON_A_RAIL
          : verdict === undefined
            ? "Let go here to cancel. Esc also cancels."
            : verdict.ok
              ? verdict.says
              : verdict.why}
      </PanePreviewWords>
    </>
  );
}

/** What the words say over a side folded to a rail (spec #5288, story 58): why letting go there
 *  cancels. Its panes are a line of buttons too narrow to show a drop's place in. */
const ON_A_RAIL = "A folded pane takes no drop. Let go here to cancel. Esc also cancels.";

/** How thick the mark under a tab about to come forward is, in CSS pixels. */
const MARK = 2;

/** The mark along the bottom of tab `id` on the strip, where it is drawn. */
function markUnder(id: number): Box | undefined {
  const tab = document.querySelector(stripTab(id));
  if (tab === null) return undefined;
  const box = boxOfRect(tab.getBoundingClientRect());
  return { left: box.left, top: box.top + box.height - MARK, width: box.width, height: MARK };
}
