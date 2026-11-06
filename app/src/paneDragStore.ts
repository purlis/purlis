// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { HeldPanes, type PaneBase } from "./paneHeld";
import { holdKeyboard } from "./paneKeyboard";
import type { Box, Size, Source, Target } from "./paneDrop";
import { layoutShown, type Layout, type Tabs } from "./tabs";
import { zoomedLayout } from "./tiling";

/**
 * **The drag in progress**, read by its layer (`paneDragLayer.tsx`) and written by the pointer's
 * half (`paneDrag.tsx`). One interface over four jobs (#5573), each a part of its own:
 *
 * - {@link DragState}: what is held and where, and who hears it change;
 * - {@link Measured}: the window as the drag measured it, and when that is to be retaken;
 * - {@link PointerLabels}: the pointer, and the label and words that follow it, written straight
 *   on their elements so that a pointer moving draws nothing;
 * - {@link WindowHold}: what the window is held in while the drag lasts, the root's marks and the
 *   keyboard (#5523), all let go of as it stops.
 */

/** Where the pointer's over, and what is known of the place. */
export type Over = {
  target: Target;
  /** The pane's size at the drag's start, for the floor the drop keeps; none on the strip. */
  size?: Size;
  /** On the strip, the line where the tab would go. */
  line?: Box;
};

/**
 * How it is held: by a pointer, or by the keys and single presses of placement mode (#5293), which
 * draw the same preview and want no grabbing cursor.
 */
export type HeldBy = "pointer" | "keys";

export type Dragging = {
  source: Source;
  over?: Over;
  base?: PaneBase;
  /** The tab on the strip a drag rests on, until it comes forward (#5294): it wears a mark. */
  opening?: number;
  /** Over a side folded to a rail, which takes no drop and says why (#5415). */
  onRail?: boolean;
};

/** What a tab draws, as the window asks for it (`PlaneView`'s `LayoutPanes`). */
export function shownIn(tabs: Tabs): { front: number; layout: Layout } | undefined {
  const front = tabs.inFront;
  const layout = front === undefined ? undefined : layoutShown(tabs, front);
  return front === undefined || layout === undefined
    ? undefined
    : { front, layout: zoomedLayout(tabs, front, layout) };
}

const measuredAs = (shown: ReturnType<typeof shownIn>) =>
  shown === undefined ? "" : `${shown.front}:${placesOf(shown.layout)}`;

/** How far from the pointer the label and the words sit. */
export const heldAt = (x: number, y: number) => ({ left: x + 14, top: y + 12 });
/** How far from the window's end the words keep, so they stay in it (`pane-preview.css`). */
const WORDS_ROOM = 350;
export const wordsAt = (x: number, y: number) => ({
  left: Math.min(x + 14, window.innerWidth - WORDS_ROOM),
  top: y + 40,
});

/**
 * What a zone depends on in a tree: which panes, in which splits, at which shares. A chat shown in
 * a pane's place draws another tree of the same panes, which is no reason to measure anything.
 */
function placesOf(layout: Layout | undefined): string {
  if (layout === undefined) return "";
  if (layout.kind === "pane") return String(layout.pane);
  const [a, b] = layout.children;
  return `${layout.direction}${layout.share ?? ""}(${placesOf(a)} ${placesOf(b)})`;
}

/** **What is held and where**, and who hears it change. A change to what it already is tells
 *  nobody. */
class DragState {
  private state: Dragging | undefined;
  private readonly heard = new Set<() => void>();

  readonly get = () => this.state;
  readonly subscribe = (listener: () => void) => {
    this.heard.add(listener);
    return () => {
      this.heard.delete(listener);
    };
  };

  set(next: Dragging | undefined) {
    if (next === undefined && this.state === undefined) return;
    this.state = next;
    for (const listener of this.heard) listener();
  }

  /** Changes the drag in progress, if there is one. */
  change(patch: Partial<Dragging>) {
    if (this.state !== undefined) this.set({ ...this.state, ...patch });
  }
}

/** **The window as the drag measured it**: the tab in front and the places of its panes, so a
 *  drawing of another tree, and only that, is retaken. */
class Measured {
  private measuredFor = "";

  constructor(private readonly held: HeldPanes) {}

  /** The window's zones for what `tabs` draws, now; none where it draws no tab. */
  take(tabs: Tabs): PaneBase | undefined {
    const shown = shownIn(tabs);
    this.measuredFor = measuredAs(shown);
    return shown === undefined ? undefined : this.held.measure(shown.front);
  }

  /** Whether `tabs` draws what was measured. */
  is(tabs: Tabs) {
    return this.measuredFor === measuredAs(shownIn(tabs));
  }

  forget() {
    this.measuredFor = "";
  }
}

/** **The pointer, and what follows it**: written straight on the label's and the words' elements,
 *  while they are drawn. */
class PointerLabels {
  private at = { x: 0, y: 0 };
  private heldLabel: HTMLElement | null = null;
  private wordsLabel: HTMLElement | null = null;
  readonly heldHere = (element: HTMLElement | null) => {
    this.heldLabel = element;
  };
  readonly wordsHere = (element: HTMLElement | null) => {
    this.wordsLabel = element;
  };

  /** Where it starts: nothing is drawn yet to move. */
  startAt(x: number, y: number) {
    this.at = { x, y };
  }

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

  pointer() {
    return this.at;
  }
}

/** **What the window is held in while a drag lasts**: the root's marks (`paneFrame.css`), and the
 *  keyboard where it is (`holdKeyboard`, #5523). */
class WindowHold {
  /** Lets go of the keyboard, while it is held. */
  private keyboard: (() => void) | undefined;

  take(by: HeldBy) {
    document.documentElement.dataset.paneDragging = by;
    this.keyboard ??= holdKeyboard();
  }

  letGo() {
    const root = document.documentElement.dataset;
    delete root.paneDragging;
    delete root.panePreviewing;
    delete root.paneRefused;
    this.keyboard?.();
    this.keyboard = undefined;
  }
}

/** **The drag in progress**: one interface over its four parts. */
export class PaneDragStore {
  private readonly state = new DragState();
  readonly held = new HeldPanes();
  private readonly measured = new Measured(this.held);
  private readonly labels = new PointerLabels();
  private readonly hold = new WindowHold();
  readonly heldHere = this.labels.heldHere;
  readonly wordsHere = this.labels.wordsHere;
  readonly get = this.state.get;
  readonly subscribe = this.state.subscribe;

  /** Something is picked up: the window is read, every terminal is held at its size, and the
   *  keyboard where it is: no terminal of a tab brought forward takes it (`holdKeyboard`). */
  start(source: Source, tabs: Tabs, x: number, y: number, by: HeldBy = "pointer") {
    this.hold.take(by);
    this.labels.startAt(x, y);
    this.state.set({ source, base: this.measured.take(tabs) });
  }

  /** The window as it is now, when the tab in front or the room has changed under the drag. */
  retake(tabs: Tabs) {
    if (this.state.get() === undefined || shownIn(tabs) === undefined) return;
    this.state.change({ base: this.measured.take(tabs) });
  }

  /** Whether `tabs` draws what was measured. */
  measuredIs(tabs: Tabs) {
    return this.measured.is(tabs);
  }

  /** The pointer moved: what follows it goes with it, and nothing else is drawn for it. */
  moveTo(x: number, y: number) {
    this.labels.moveTo(x, y);
  }

  /** Where the pointer was last. */
  pointer() {
    return this.labels.pointer();
  }

  /** The place it is over has changed. */
  overPlace(over: Over | undefined) {
    this.state.change({ over });
  }

  /** Whether it is over a side folded to a rail (#5415): no place, and a refusal that says why. */
  overRail(on: boolean) {
    if ((this.state.get()?.onRail ?? false) !== on) this.state.change({ onRail: on });
  }

  /** The tab it rests on, about to come forward, or none (#5294). */
  restOn(tab: number | undefined) {
    if (this.state.get()?.opening !== tab) this.state.change({ opening: tab });
  }

  /** **The drag is over**: everything it wrote is taken off, in this very event. */
  stop() {
    this.hold.letGo();
    this.held.letGo();
    this.measured.forget();
    this.state.set(undefined);
  }
}
