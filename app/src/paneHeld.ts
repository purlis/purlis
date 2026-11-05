// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Box } from "./paneDrop";
import { type Gaps, nearly, type Plan, sameSize, type TerminalCells } from "./panePreview";

/**
 * **The real panes while a drag holds them** (#5291): measured once as it starts, moved to where
 * the drop would leave them, and let go of exactly. Everything here is written on the elements
 * the window already draws, and taken off again, so a cancel has nothing to put back.
 *
 * - **A pane moves by a transform and a size of its own**, on its frame. No tree is drawn again,
 *   so no terminal is taken out of its pane, and what prints goes on printing where it is.
 * - **A terminal's holder is pinned to the pixels it had** ({@link PINNED}), against the bottom of
 *   its pane's body, as it always grows from the top. The holder is what `SessionPane` watches, so
 *   however its frame moves the observer hears nothing, the terminal isn't fitted, and its program
 *   is told nothing. Its text keeps its width and is clipped or spaced by the frame.
 *   Letting go of the pin is the one change the observer hears, so a terminal is fitted once.
 * - **The top of a pane is pinned too**: a Notice that arrives during a drag waits on the bell and
 *   in the Inbox rather than taking rows under the pointer.
 */

/** The element a window's panes are laid out in, whose box is the room they have. */
export const PANES_ROOM = "data-panes-room";
/** What holds a terminal in its pane (`SessionPane`): the element its observer watches. */
export const TERMINAL_HOLDER = "data-pane-terminal";
/** What a drag holds at its size until it ends. */
const PINNED = "data-pane-pinned";
/** What a pane is doing in the preview: `moved`, `lifted` or `gone` (`paneFrame.css`). */
const PREVIEW = "data-pane-preview";
/** The pane a drop lands in: `data-drop-pane` holds its number, `data-drop-tab` its tab's. */
export const PANE = "data-drop-pane";
/** A tab on the strip, by its id; on a pane, the tab it is a pane of. */
export const TAB = "data-drop-tab";

/** What a terminal says of its own grid, to reckon what would fit in a box. */
export type OwnCells = {
  width: number;
  height: number;
  /** Its scrollbar's width and its own padding across. */
  extraWidth: number;
  /** Its own padding down. */
  extraHeight: number;
};

const readers = new WeakMap<Element, () => OwnCells | undefined>();

/** A terminal says how to read its cells, on the element that holds it. Gives back its leaving. */
export function terminalInPane(holder: Element, read: () => OwnCells | undefined): () => void {
  readers.set(holder, read);
  return () => readers.delete(holder);
}

/** What a drag reads of the window as it starts, and again when another tab comes to the front. */
export type PaneBase = {
  /** The room the panes have; none where the window draws no room to lay them out in. */
  room?: Box;
  gaps: Gaps;
  /** Where each pane is drawn, by its number: the drop zones, which stay on this layout. */
  zones: Map<number, Box>;
  /** How each pane's terminal sits in it, by the pane's number. */
  terminals: Map<number, TerminalSeat>;
};

/** A pane's terminal: where it says its cells, and all the pane draws around what it has. */
export type TerminalSeat = { holder: Element; aroundWidth: number; aroundHeight: number };

/**
 * **How a terminal sits in its pane, as it says now.** Read when asked for and not as the window is
 * measured: a pane just drawn, on a tab brought forward, has its terminal a moment after.
 */
export function cellsIn(seat: TerminalSeat | undefined): TerminalCells | undefined {
  const own = seat === undefined ? undefined : readers.get(seat.holder)?.();
  return seat === undefined || own === undefined
    ? undefined
    : {
        width: own.width,
        height: own.height,
        aroundWidth: seat.aroundWidth + own.extraWidth,
        aroundHeight: seat.aroundHeight + own.extraHeight,
      };
}

/** Its sides as plain values: a `DOMRect` keeps them on its prototype, so a spread of one has no
 *  box (#5284). */
export const boxOfRect = (r: DOMRect): Box => ({
  left: r.left,
  top: r.top,
  width: r.width,
  height: r.height,
});

const framesOf = (tab: number) => [
  ...document.querySelectorAll<HTMLElement>(`[${PANE}][${TAB}="${tab}"]`),
];

/** The pixels a length in the theme comes to here. Its own element, since nothing may be drawn. */
function pixelsOf(length: string): number {
  const probe = document.createElement("div");
  probe.style.cssText = `position:absolute;visibility:hidden;inline-size:${length}`;
  document.body.append(probe);
  const { width } = probe.getBoundingClientRect();
  probe.remove();
  return width;
}

/** Leaves a frame where the layout has it. */
function unmove(frame: HTMLElement) {
  frame.style.removeProperty("transform");
  frame.style.removeProperty("width");
  frame.style.removeProperty("height");
}

function pin(element: HTMLElement, sides: "both" | "height", pinned: Set<HTMLElement>) {
  if (element.hasAttribute(PINNED)) return;
  const { width, height } = element.getBoundingClientRect();
  if (width === 0 || height === 0) return;
  if (sides === "both") element.style.width = `${width}px`;
  element.style.height = `${height}px`;
  element.setAttribute(PINNED, "");
  pinned.add(element);
}

/** **The panes a drag holds**, for as long as it lasts. */
export class HeldPanes {
  private readonly pinned = new Set<HTMLElement>();
  private readonly written = new Set<HTMLElement>();

  /**
   * **Reads the window for tab `front`**, with whatever a preview wrote taken off first, and pins
   * what is not pinned yet. What was pinned stays: nothing is fitted until the drag ends.
   */
  measure(front: number): PaneBase {
    this.clearWritten(true);
    const zones = new Map<number, Box>();
    const terminals = new Map<number, TerminalSeat>();
    const frames = framesOf(front);
    for (const frame of frames) {
      const pane = Number(frame.getAttribute(PANE));
      const box = boxOfRect(frame.getBoundingClientRect());
      if (box.width === 0 || box.height === 0) continue;
      zones.set(pane, box);
      const holder = frame.querySelector<HTMLElement>(`[${TERMINAL_HOLDER}]`);
      if (holder !== null) {
        // The body is what a terminal has: its holder may be pinned at another size.
        const room = (frame.querySelector(".pane-body") ?? holder).getBoundingClientRect();
        terminals.set(pane, {
          holder,
          aroundWidth: box.width - room.width,
          aroundHeight: box.height - room.height,
        });
      }
    }
    for (const frame of frames) {
      const holder = frame.querySelector<HTMLElement>(`[${TERMINAL_HOLDER}]`);
      if (holder !== null) pin(holder, "both", this.pinned);
      const top = frame.querySelector<HTMLElement>(".pane-top");
      if (top !== null) pin(top, "height", this.pinned);
    }
    const room = document.querySelector(`[${PANES_ROOM}]`)?.getBoundingClientRect();
    return {
      room: room === undefined || room.width === 0 ? undefined : boxOfRect(room),
      gaps: {
        separator: pixelsOf("max(var(--window-gutter), var(--panel-border-width))"),
        fold: pixelsOf("var(--space-1)"),
      },
      zones,
      terminals,
    };
  }

  /**
   * **Draws the preview on the frames of tab `front`**: each moved to its box, or, with no plan,
   * where it is. `held` is the pane being moved, which dims where it is until it has a place.
   */
  draw(front: number, base: PaneBase, plan: Plan | undefined, held: number | undefined) {
    for (const frame of framesOf(front)) {
      const pane = Number(frame.getAttribute(PANE));
      const from = base.zones.get(pane);
      const box = plan?.frames.get(pane);
      this.written.add(frame);
      if (
        box === undefined ||
        from === undefined ||
        (nearly(box.left, from.left) && nearly(box.top, from.top) && sameSize(box, from))
      ) {
        unmove(frame);
      } else {
        frame.style.transform = `translate(${box.left - from.left}px, ${box.top - from.top}px)`;
        frame.style.width = `${box.width}px`;
        frame.style.height = `${box.height}px`;
      }
      const doing =
        plan === undefined
          ? pane === held
            ? "lifted"
            : undefined
          : plan.gone.includes(pane)
            ? "gone"
            : pane === held
              ? "moved"
              : undefined;
      if (doing === undefined) frame.removeAttribute(PREVIEW);
      else frame.setAttribute(PREVIEW, doing);
    }
  }

  /** **Lets go of everything**: every frame, terminal and top as the window drew them. */
  letGo() {
    this.clearWritten(false);
    for (const element of this.pinned) {
      element.style.removeProperty("width");
      element.style.removeProperty("height");
      element.removeAttribute(PINNED);
    }
    this.pinned.clear();
  }

  /** `at once`: with no motion, so what is measured next is where the frame belongs. */
  private clearWritten(atOnce: boolean) {
    for (const frame of this.written) {
      if (atOnce) frame.style.transition = "none";
      unmove(frame);
      frame.removeAttribute(PREVIEW);
    }
    if (atOnce) {
      // Read so the frames are laid out where they belong before their motion is theirs again.
      for (const frame of this.written) frame.getBoundingClientRect();
      for (const frame of this.written) frame.style.removeProperty("transition");
    }
    this.written.clear();
  }
}
