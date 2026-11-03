// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { pressed } from "./keys/registry";
import { Notice } from "./Notice";
import {
  type Box,
  type DropContext,
  dropOn,
  judgeDrop,
  placeAt,
  previewOf,
  type Size,
  type Source,
  sourceName,
  type Target,
  undone,
  type Verdict,
} from "./paneDrop";
import { UNDO_MS } from "./MemoryEdits";
import type { Tabs } from "./tabs";

/**
 * **Dragging panes in the window** (#4763): the pointer half of `paneDrop.ts`. Everything a drop
 * does and refuses is decided there; this measures where the pointer is, draws the answer before
 * the drop, says it, and offers Undo after it.
 *
 * What can be picked up is marked in the markup, so the panes, the sidebar and the strip each
 * keep their own drawing and add a mark (`data-*` below), and no drag library sits between a
 * pointer and a terminal that eats its events:
 *
 * - a pane by its head, {@link GRIP} inside a pane marked {@link PANE} (an empty pane has none);
 * - a sidebar row's chat, {@link CHAT_ROW};
 * - a tab on the strip, {@link TAB}, once it is pulled down off the strip ({@link OFF_THE_STRIP}).
 *   Along the strip it is the strip's own drag, which reorders (SI-6).
 *
 * Esc cancels a drag. Holding over a tab for {@link HOLD_MS} brings it forward, so a pane can
 * land in another tab's layout. A refused place is drawn dashed, with its reason.
 */

/** The pane a drop lands in: `data-drop-pane` holds its number, `data-drop-tab` its tab's. */
export const PANE = "data-drop-pane";
/** A pane's grip: its head, pressed outside its buttons. */
export const GRIP = "data-pane-grip";
/** A sidebar row a chat is dragged from, by its session. */
export const CHAT_ROW = "data-drag-chat";
/** The strip: a drop on it makes a tab. Its tabs carry {@link TAB}. */
export const STRIP = "data-drop-strip";
/** A tab on the strip, by its id. */
export const TAB = "data-drop-tab";

/** The marks a pane is drawn with so a drag can land in it: its number and its tab's. */
export function dropPane(tab: number, pane: number): Record<string, string> {
  return { [PANE]: String(pane), [TAB]: String(tab) };
}

/** How far a pointer moves, in pixels, before a press is a drag. */
const DRAG_FROM = 5;
/** How far below the strip a tab is pulled before it leaves it for a split. */
const OFF_THE_STRIP = 24;
/** How long a drag holds over a tab before the tab comes forward (#4583). */
export const HOLD_MS = 500;

/** What the drag is over now, and what dropping there would do. */
type Over = {
  target: Target;
  verdict: Verdict;
  /** Where the result is drawn: the half the source would take, or a line on the strip. */
  box: Box;
  /** The pane's size, for the floor the drop keeps; none on the strip. */
  size?: Size;
};

type Dragging = { source: Source; x: number; y: number; over?: Over };

/** A drop that can still be undone: the tabs before it and after it, and what it did. */
type Undoable = { before: Tabs; after: Tabs; did: string };

export type PaneDragging = {
  /** The preview, the sentence by the pointer, and what a reader hears. */
  layer: ReactNode;
  /** The Undo line after a drop. */
  undoLine: ReactNode;
};

/**
 * **Pane dragging for a project's window.** `change` is the window's one way to change its tabs;
 * `onHoldTab` brings a tab forward. `ctx` names what is dragged and where chats are filed.
 */
export function usePaneDrag({
  tabs,
  change,
  ctx,
  onHoldTab,
  chatNameOf,
}: {
  tabs: Tabs;
  change: (how: (tabs: Tabs) => Tabs) => Tabs;
  ctx: DropContext;
  onHoldTab: (id: number) => void;
  /** A chat's own name, as the core was told it (`Content.chat`), for one dragged from a row. */
  chatNameOf: (session: number) => string;
}): PaneDragging {
  const [dragging, setDragging] = useState<Dragging>();
  const [said, setSaid] = useState("");
  const [undo, setUndo] = useState<Undoable>();
  /** Whether the Undo line is drawn: for {@link UNDO_MS} after a drop. ⌘Z works on while the
   *  layout is still what the drop left. */
  const [lineUp, setLineUp] = useState(false);
  useEffect(() => {
    if (undo === undefined) return;
    const timer = window.setTimeout(() => setLineUp(false), UNDO_MS);
    return () => window.clearTimeout(timer);
  }, [undo]);
  const now = useRef({ tabs, ctx, change, onHoldTab, chatNameOf });
  useEffect(() => {
    now.current = { tabs, ctx, change, onHoldTab, chatNameOf };
  }, [tabs, ctx, change, onHoldTab, chatNameOf]);

  // The Undo goes once the layout moves on without it (`undone`): it is drawn and heard only
  // while it can still put the drop's layout back.
  const undoable = undo !== undefined && undone(tabs, undo.before, undo.after) !== undefined;

  const takeBack = useCallback(() => {
    if (undo === undefined) return;
    const was = undo;
    now.current.change((current) => undone(current, was.before, was.after) ?? current);
    setUndo(undefined);
    setSaid("Undone. The layout is back as it was.");
  }, [undo]);

  // ⌘Z, while there is a drop to undo: a field's own Undo comes first.
  useEffect(() => {
    if (!undoable) return;
    const heard = (event: KeyboardEvent) => {
      if (pressed(event, ["layout.undo"], { runnable: false }) === undefined) return;
      event.preventDefault();
      event.stopPropagation();
      takeBack();
    };
    window.addEventListener("keydown", heard, true);
    return () => window.removeEventListener("keydown", heard, true);
  }, [takeBack, undoable]);

  useEffect(() => {
    const down = (press: PointerEvent) => {
      if (press.button !== 0 || !(press.target instanceof Element)) return;
      const picked = pickedUp(press.target);
      if (picked === undefined) return;
      const { source } = picked;
      if (source.kind === "chat") source.chat = now.current.chatNameOf(source.session);
      follow(press, picked);
    };

    let leavingTheStrip = false;
    const follow = (press: PointerEvent, picked: { source: Source; strip?: DOMRect }) => {
      const { source, strip } = picked;
      let live = false;
      let held: { tab: number; timer: number } | undefined;
      const stopHolding = () => {
        if (held !== undefined) window.clearTimeout(held.timer);
        held = undefined;
      };
      const end = () => {
        window.removeEventListener("pointermove", move, true);
        window.removeEventListener("pointerup", up, true);
        window.removeEventListener("keydown", esc, true);
        stopHolding();
        delete document.documentElement.dataset.paneDragging;
        setDragging(undefined);
      };
      const move = (at: PointerEvent) => {
        if (!live) {
          const far =
            strip !== undefined
              ? at.clientY > strip.bottom + OFF_THE_STRIP
              : Math.hypot(at.clientX - press.clientX, at.clientY - press.clientY) >= DRAG_FROM;
          if (!far) return;
          live = true;
          // A tab pulled off the strip leaves the strip's own drag, which an Escape on the
          // document ends (dnd-kit's pointer sensor).
          if (strip !== undefined) {
            // Kept to dnd-kit's listener on the document as far as it can be: it is heard there
            // first, and goes no further up to the window's own Escape handlers.
            leavingTheStrip = true;
            document.addEventListener("keydown", stopHere, { once: true });
            document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape" }));
            document.removeEventListener("keydown", stopHere);
            leavingTheStrip = false;
          }
          document.documentElement.dataset.paneDragging = "";
          setSaid(`Dragging ${sourceName(now.current.tabs, source, now.current.ctx)}.`);
        }
        const over = hit(at.clientX, at.clientY, source);
        const tab = tabUnder(at.clientX, at.clientY);
        if (tab !== held?.tab) {
          stopHolding();
          if (tab !== undefined)
            held = { tab, timer: window.setTimeout(() => now.current.onHoldTab(tab), HOLD_MS) };
        }
        setDragging({ source, x: at.clientX, y: at.clientY, over });
      };
      const up = (at: PointerEvent) => {
        end();
        if (!live) return;
        // The click that ends a drag is the drag's, never a row's press.
        window.addEventListener("click", swallow, { capture: true, once: true });
        window.setTimeout(() => window.removeEventListener("click", swallow, true), 0);
        const over = hit(at.clientX, at.clientY, source);
        if (over === undefined) {
          setSaid("Not dropped: drop it on a pane or on the tab strip.");
          return;
        }
        if (!over.verdict.ok) {
          setSaid(`Not dropped: ${over.verdict.why}`);
          return;
        }
        const did = over.verdict.did;
        const before = now.current.tabs;
        const after = now.current.change((current) =>
          dropOn(current, source, over.target, now.current.ctx, over.size),
        );
        if (after !== before) {
          setUndo({ before, after, did });
          setLineUp(true);
        }
        setSaid(did);
      };
      const esc = (key: KeyboardEvent) => {
        // Not the Escape that ends the strip's own drag as a tab leaves it.
        if (key.key !== "Escape" || leavingTheStrip) return;
        key.preventDefault();
        key.stopPropagation();
        end();
        if (live) setSaid("Put back. Nothing moved.");
      };
      window.addEventListener("pointermove", move, true);
      window.addEventListener("pointerup", up, true);
      window.addEventListener("keydown", esc, true);
    };

    /** What dropping `source` at the point would do: in a pane, or on the strip. */
    const hit = (x: number, y: number, source: Source): Over | undefined => {
      const { tabs, ctx } = now.current;
      const under = document.elementFromPoint(x, y);
      if (!(under instanceof Element)) return undefined;
      const strip = under.closest(`[${STRIP}]`);
      if (strip !== null) {
        const placed = stripPlace(strip, x, tabs);
        if (placed === undefined) return undefined;
        const target: Target = { kind: "strip", at: placed.at };
        return { target, verdict: judgeDrop(tabs, source, target, ctx), box: placed.line };
      }
      const pane = under.closest(`[${PANE}]`);
      if (!(pane instanceof HTMLElement)) return undefined;
      const tab = Number(pane.getAttribute(TAB));
      const number = Number(pane.getAttribute(PANE));
      const r = pane.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) return undefined;
      const place = placeAt((x - r.left) / r.width, (y - r.top) / r.height);
      const target: Target = { kind: "pane", tab, pane: number, place };
      const size = { width: r.width, height: r.height };
      return {
        target,
        verdict: judgeDrop(tabs, source, target, ctx, size),
        box: previewOf(r, place),
        size,
      };
    };

    window.addEventListener("pointerdown", down, true);
    return () => window.removeEventListener("pointerdown", down, true);
  }, []);

  const layer = (
    <>
      {dragging?.over !== undefined && (
        <div
          className="pane-drop-preview"
          data-refused={!dragging.over.verdict.ok || undefined}
          data-line={dragging.over.target.kind === "strip" || undefined}
          aria-hidden="true"
          style={{
            left: dragging.over.box.left,
            top: dragging.over.box.top,
            width: dragging.over.box.width,
            height: dragging.over.box.height,
          }}
        />
      )}
      {dragging?.over !== undefined && (
        <div
          className="pane-drop-says"
          data-refused={!dragging.over.verdict.ok || undefined}
          aria-hidden="true"
          style={{ left: dragging.x + 14, top: dragging.y + 14 }}
        >
          {dragging.over.verdict.ok ? dragging.over.verdict.says : dragging.over.verdict.why}
        </div>
      )}
      <div role="status" className="sr-only" data-pane-drag-said="">
        {said}
      </div>
    </>
  );

  const undoLine =
    undoable && lineUp ? (
      <Notice
        cause="layout-changed"
        tone="news"
        fixes={[{ label: "Undo", onPress: takeBack }]}
        onDismiss={() => setLineUp(false)}
      >
        {undo.did}
      </Notice>
    ) : null;

  return { layer, undoLine };
}

/** What a press on `target` picks up, if anything: a pane by its grip, a sidebar row's chat, or
 *  a tab on the strip (with the strip's box, which it must be pulled down off first). */
function pickedUp(target: Element): { source: Source; strip?: DOMRect } | undefined {
  // A press on a control inside a head or a row is that control's, never a drag.
  const control = target.closest(CONTROLS);
  const inside = (holder: Element) =>
    control !== null && control !== holder && holder.contains(control);
  const grip = target.closest(`[${GRIP}]`);
  const pane = grip?.closest(`[${PANE}]`);
  if (grip != null && pane != null) {
    if (inside(grip) || control === grip) return undefined;
    return {
      source: {
        kind: "pane",
        tab: Number(pane.getAttribute(TAB)),
        pane: Number(pane.getAttribute(PANE)),
      },
    };
  }
  // A sidebar row is pressable itself: its press stays a press, and a drag takes it.
  const row = target.closest(`[${CHAT_ROW}]`);
  if (row !== null) {
    if (inside(row)) return undefined;
    return {
      source: {
        kind: "chat",
        session: Number(row.getAttribute(CHAT_ROW)),
        // Its own name is the window's to say (`chatNameOf`), not the row's markup.
        chat: "",
      },
    };
  }
  const tab = target.closest(`[${TAB}]`);
  const strip = tab?.closest(`[${STRIP}]`);
  // A tab is pressed by its own tab element; a control beside it in its cell (its close, its
  // name box) keeps its press.
  if (
    control !== null &&
    tab?.contains(control) === true &&
    control.closest("[role='tab']") === null
  )
    return undefined;
  if (tab != null && strip != null)
    return {
      source: { kind: "tab", tab: Number(tab.getAttribute(TAB)) },
      strip: strip.getBoundingClientRect(),
    };
  return undefined;
}

/** What a press is a control's press on, rather than a drag's start. */
const CONTROLS = "button, a, input, textarea, select, [role='menuitem'], [role='button']";

/** Where on the strip a drop at `x` goes: before the first tab whose middle is past it. */
function stripPlace(strip: Element, x: number, tabs: Tabs): { at: number; line: Box } | undefined {
  const shown = [...strip.querySelectorAll(`[${TAB}]`)];
  const box = strip.getBoundingClientRect();
  const line = (left: number): Box => ({
    left: left - 1,
    top: box.top,
    width: 2,
    height: box.height,
  });
  for (const one of shown) {
    const r = one.getBoundingClientRect();
    if (x < r.left + r.width / 2) {
      const at = tabs.order.indexOf(Number(one.getAttribute(TAB)));
      return at < 0 ? undefined : { at, line: line(r.left) };
    }
  }
  const last = shown[shown.length - 1];
  if (last === undefined) return { at: tabs.order.length, line: line(box.left) };
  const at = tabs.order.indexOf(Number(last.getAttribute(TAB)));
  return at < 0 ? undefined : { at: at + 1, line: line(last.getBoundingClientRect().right) };
}

/** The tab on the strip under the point, if any: a drag held there brings it forward. */
function tabUnder(x: number, y: number): number | undefined {
  const tab = document.elementFromPoint(x, y)?.closest(`[${TAB}]`);
  return tab?.closest(`[${STRIP}]`) == null ? undefined : Number(tab.getAttribute(TAB));
}

/** Ends an event where it is heard, so nothing further up hears it. */
function stopHere(event: Event) {
  event.stopPropagation();
}

function swallow(event: MouseEvent) {
  event.stopPropagation();
  event.preventDefault();
}
