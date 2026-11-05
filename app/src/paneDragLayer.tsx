// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useLayoutEffect, useMemo, useSyncExternalStore } from "react";
import { HeldPanes, type PaneBase, PANES_ROOM } from "./paneHeld";
import {
  type Box,
  type DropContext,
  heldPane,
  judgeDrop,
  type Size,
  sourceName,
  type Source,
  type Target,
} from "./paneDrop";
import { gridIn, planOf, type TerminalCells, wouldBe } from "./panePreview";
import { layoutShown, type Layout, panes, type Tabs } from "./tabs";
import { zoomedLayout } from "./tiling";
import {
  PanePreviewArriving,
  PanePreviewHeld,
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
 * **A component of its own, on a store of its own**, so that a pointer moving draws this, and never
 * the whole window its hook is called in. A label following the pointer is written straight on its
 * element; a target changing draws this and writes the frames.
 */

/** Where the pointer's over, and what is known of the place. */
export type Over = {
  target: Target;
  /** The pane's size at the drag's start, for the floor the drop keeps; none on the strip. */
  size?: Size;
  /** On the strip, the line where the tab would go. */
  line?: Box;
};

type Dragging = { source: Source; over?: Over; base?: PaneBase };

/** What a tab draws, as the window asks for it (`PlaneView`'s `LayoutPanes`). */
function shownIn(tabs: Tabs): { front: number; layout: Layout } | undefined {
  const front = tabs.inFront;
  const layout = front === undefined ? undefined : layoutShown(tabs, front);
  return front === undefined || layout === undefined
    ? undefined
    : { front, layout: zoomedLayout(tabs, front, layout) };
}

/** How far from the pointer the label and the words sit. */
const heldAt = (x: number, y: number) => ({ left: x + 14, top: y + 12 });
const wordsAt = (x: number, y: number) => ({
  left: Math.min(x + 14, window.innerWidth - 350),
  top: y + 40,
});

/** **The drag in progress**, read by its layer and written by the pointer's half. */
export class PaneDragStore {
  private state: Dragging | undefined;
  private readonly heard = new Set<() => void>();
  private readonly held = new HeldPanes();
  private measuredFor: Layout | undefined;
  private at = { x: 0, y: 0 };
  /** The elements that follow the pointer, while they are drawn. */
  private heldLabel: HTMLElement | null = null;
  private wordsLabel: HTMLElement | null = null;
  readonly heldHere = (element: HTMLElement | null) => {
    this.heldLabel = element;
  };
  readonly wordsHere = (element: HTMLElement | null) => {
    this.wordsLabel = element;
  };

  readonly get = () => this.state;
  readonly subscribe = (listener: () => void) => {
    this.heard.add(listener);
    return () => {
      this.heard.delete(listener);
    };
  };

  private set(next: Dragging | undefined) {
    this.state = next;
    for (const listener of this.heard) listener();
  }

  /** Something is picked up: the window is read, and every terminal is held at its size. */
  start(source: Source, tabs: Tabs, x: number, y: number) {
    document.documentElement.dataset.paneDragging = "";
    this.at = { x, y };
    const shown = shownIn(tabs);
    this.measuredFor = shown?.layout;
    this.set({ source, base: shown === undefined ? undefined : this.held.measure(shown.front) });
  }

  /** The window as it is now, when the tab in front or the room has changed under the drag. */
  retake(tabs: Tabs) {
    const shown = shownIn(tabs);
    if (this.state === undefined || shown === undefined) return;
    this.measuredFor = shown.layout;
    this.set({ ...this.state, base: this.held.measure(shown.front) });
  }

  /** Whether `tabs` draws what was measured. */
  measuredIs(layout: Layout | undefined) {
    return this.measuredFor === layout;
  }

  /** The pointer moved: what follows it goes with it, and nothing else is drawn for it. */
  moveTo(x: number, y: number) {
    this.at = { x, y };
    const held = heldAt(x, y);
    if (this.heldLabel !== null) {
      this.heldLabel.style.left = `${held.left}px`;
      this.heldLabel.style.top = `${held.top}px`;
    }
    const words = wordsAt(x, y);
    if (this.wordsLabel !== null) {
      this.wordsLabel.style.left = `${words.left}px`;
      this.wordsLabel.style.top = `${words.top}px`;
    }
  }

  /** Where the pointer was last. */
  pointer() {
    return this.at;
  }

  /** The place it is over has changed. */
  overPlace(over: Over | undefined) {
    if (this.state !== undefined) this.set({ ...this.state, over });
  }

  /** Writes the preview on the panes. */
  draw(...args: Parameters<HeldPanes["draw"]>) {
    this.held.draw(...args);
  }

  /** **The drag is over**: everything it wrote is taken off, in this very event. */
  stop() {
    const root = document.documentElement.dataset;
    delete root.paneDragging;
    delete root.panePreviewing;
    delete root.paneRefused;
    this.held.letGo();
    this.measuredFor = undefined;
    if (this.state !== undefined) this.set(undefined);
  }
}

const sameBox = (a: Size, b: Size) => a.width === b.width && a.height === b.height;

/** The cells a pane with no terminal yet would have: those of a terminal on screen. */
const anyCells = (base: PaneBase): TerminalCells | undefined =>
  base.terminals.values().next().value;

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
    if (!dragging || store.measuredIs(layout)) return;
    store.retake(tabs);
  }, [dragging, layout, store, tabs]);
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
  const refused = verdict !== undefined && !verdict.ok && !overItself;
  const held = drag === undefined ? undefined : heldPane(tabs, drag.source)?.pane;

  useLayoutEffect(() => {
    if (!dragging || base === undefined || front === undefined) return;
    store.draw(front, base, plan, { held });
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
  return (
    <>
      {over?.line !== undefined && (
        <div
          className="pane-drop-preview"
          data-line=""
          data-refused={refused || undefined}
          aria-hidden="true"
          style={{
            left: over.line.left,
            top: over.line.top,
            width: over.line.width,
            height: over.line.height,
          }}
        />
      )}
      {refusedZone !== undefined && <PanePreviewRefused box={refusedZone} />}
      {plan?.arriving.map(({ pane, box, content }) => (
        <PanePreviewArriving key={pane} box={box} name={ctx.name(content)} />
      ))}
      {plan?.rails.map(({ box, as, panes: folded }) => (
        <PanePreviewArriving key={folded.join(" ")} box={box} as={as} />
      ))}
      {plan !== undefined &&
        base !== undefined &&
        [...plan.frames].map(([pane, box]) => {
          const cells = base.terminals.get(pane);
          const from = base.zones.get(pane);
          if (cells === undefined || from === undefined || sameBox(from, box)) return null;
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
        {verdict === undefined
          ? "Let go here to cancel. Esc also cancels."
          : verdict.ok
            ? verdict.says
            : verdict.why}
      </PanePreviewWords>
    </>
  );
}
