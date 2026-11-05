// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import {
  ArrowDown,
  ArrowLeft,
  ArrowLeftRight,
  ArrowRight,
  ArrowUp,
  GripVertical,
  SquareArrowOutUpRight,
} from "lucide-react";
import { keySaid, pressed } from "./keys/registry";
import { Notice } from "./Notice";
import { REGION_CHROME } from "./RegionFrame";
import {
  type Box,
  type DropContext,
  dropOn,
  judgeDrop,
  type Place,
  placeAt,
  previewOf,
  type Size,
  type Source,
  sourceName,
  type Target,
  type Verdict,
} from "./paneDrop";
import {
  firstTarget,
  groupsOf,
  pickedUpSaid,
  placingDrop,
  PUT_BACK,
  sameTarget,
  isArrow,
  type Step,
  stepFrom,
  stillHeld,
  targetSaid,
  targetsOf,
} from "./panePlacement";
import { canUndoLayout, recorded, undoLayout } from "./layoutUndo";
import { UNDO_MS } from "./MemoryEdits";
import { contentsOf, type LayoutUndo, type Tabs } from "./tabs";
import { Button, IconButton } from "./ui/button/Button";
import { onTooltipEscape } from "./ui/tooltip/Tooltip";

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
 *
 * **Placement mode** (#4764, `panePlacement.ts`) is every drag's route by keys and single presses
 * (WCAG 2.5.7): a press of a pane's grip ({@link PaneGrip}), Enter or Space on it, or ⌘⇧M on what
 * has the keyboard picks it up, and its targets are real buttons judged by the same rules.
 *
 * **The window's one Undo** is drawn and heard here too (`layoutUndo.ts`): the line after a drop,
 * a split or Tile all, and ⌘Z, which put back the same last change as the palette's row.
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

/** A pane's grip as a control (#4764): a press, Enter or Space picks the pane up for placement
 *  mode, and a drag from it moves the pane as a drag from the rest of its head does. */
export const GRIP_HANDLE = "data-pane-grip-handle";
/** One of placement mode's targets, by {@link targetKey}. */
const PLACE_TARGET = "data-place-target";

/** **Picking something up for placement mode**, for the grips and rows drawn inside the window. */
export type Placing = {
  /** Picks `source` up. `from` had the keyboard, and has it again when Esc puts it back (a
   *  pane's grip has it, where the pane's grip is drawn). Never changes. */
  pickUp: PickUp;
  /** What pane `pane`'s grip is called: "Move" and what the pane shows. Changes with the tabs. */
  gripName: (tab: number, pane: number) => string;
};

type PickUp = (source: Source, from?: Element | null) => void;

/** Placement mode's pick-up, which never changes, so a sidebar row reading it is never drawn
 *  again for it; none where nothing can be placed (a story). */
export const PickUpHere = createContext<PickUp | undefined>(undefined);
/** What each pane's grip is called, which changes with the tabs: read by the grips alone. */
const GripNamesHere = createContext<Placing["gripName"] | undefined>(undefined);

/** **Placement mode for what the window draws** (#4764): its pick-up, and its grips' names. */
export function PlacingHere({ placing, children }: { placing: Placing; children: ReactNode }) {
  return (
    <PickUpHere.Provider value={placing.pickUp}>
      <GripNamesHere.Provider value={placing.gripName}>{children}</GripNamesHere.Provider>
    </PickUpHere.Provider>
  );
}

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

export type PaneDragging = {
  /** The preview, the sentence by the pointer, and what a reader hears. */
  layer: ReactNode;
  /** The Undo line after a layout change. */
  undoLine: ReactNode;
  /** Puts the last layout change back, and says so: the line's Undo, ⌘Z and the palette's row. */
  undo: () => void;
  /** Placement mode's pick-up and its grips' names, for {@link PlacingHere}. */
  placing: Placing;
};

/** What placement mode holds: what was picked up, the target the keyboard is on, and what had
 *  the keyboard before. */
type Held = { source: Source; at: Target; from?: HTMLElement };

/** Where the panes of the tab in front and the strip are on screen, while something is held. */
type Boxes = { panes: Map<string, Box>; strip?: Box };

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
  active = true,
}: {
  tabs: Tabs;
  change: (how: (tabs: Tabs) => Tabs) => Tabs;
  ctx: DropContext;
  onHoldTab: (id: number) => void;
  /** A chat's own name, as the core was told it (`Content.chat`), for one dragged from a row. */
  chatNameOf: (session: number) => string;
  /** Whether this window is the one in front, whose ⌘⇧M picks up what has the keyboard. */
  active?: boolean;
}): PaneDragging {
  const [dragging, setDragging] = useState<Dragging>();
  // What a reader is told, with a count, so the same words said twice are a new node, heard again
  // (a second refusal for the same reason).
  const [told, setSaid] = useReducer(
    (was: { said: string; n: number }, said: string) => ({ said, n: was.n + 1 }),
    { said: "", n: 0 },
  );
  const [held, setHeld] = useState<Held>();
  const [boxes, setBoxes] = useState<Boxes>();
  /** A pointer drag ended in this very event: the press that a grip hears as it comes up is the
   *  drag's, never a pick-up. */
  const dragEnded = useRef(false);
  /** After a drop with the keys, the grip of the pane that moved takes the keyboard. */
  const followTheDrop = useRef(false);
  /** The last change whose Undo line went, by its time ({@link UNDO_MS}) or its Dismiss. ⌘Z and
   *  the palette's row work on while the tabs are as it left them. */
  const [lineGone, setLineGone] = useState<LayoutUndo>();
  const last = tabs.undo;
  useEffect(() => {
    if (last === undefined) return;
    const timer = window.setTimeout(() => setLineGone(last), UNDO_MS);
    return () => window.clearTimeout(timer);
  }, [last]);
  const now = useRef({ tabs, ctx, change, onHoldTab, chatNameOf, held, boxes });
  useEffect(() => {
    now.current = { tabs, ctx, change, onHoldTab, chatNameOf, held, boxes };
  }, [tabs, ctx, change, onHoldTab, chatNameOf, held, boxes]);

  // The Undo goes once the layout moves on without it: it is drawn and heard only while it can
  // still put the last change back.
  const undoable = canUndoLayout(tabs);

  const takeBack = useCallback(() => {
    let back = false;
    now.current.change((current) => {
      const was = undoLayout(current);
      back = was !== current;
      return was;
    });
    if (back) setSaid("Undone. The layout is back as it was.");
  }, []);

  // ⌘Z, while there is a change to undo: a field's own Undo comes first.
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
        dragEnded.current = true;
        window.setTimeout(() => (dragEnded.current = false), 0);
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
        now.current.change((current) =>
          recorded(current, dropOn(current, source, over.target, now.current.ctx, over.size), did),
        );
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

  // --- Placement mode (#4764) ------------------------------------------------------------------

  const pickUp = useCallback((picked: Source, from?: Element | null) => {
    if (dragEnded.current) return;
    const { tabs, ctx, chatNameOf } = now.current;
    const source: Source =
      picked.kind === "chat" && picked.chat === ""
        ? { ...picked, chat: chatNameOf(picked.session) }
        : picked;
    if (
      source.kind === "pane" &&
      contentsOf(tabs, source.tab).find(({ pane }) => pane === source.pane)?.content.kind ===
        "empty"
    ) {
      setSaid("Not picked up: an empty pane has nothing to move. Close it instead.");
      return;
    }
    setHeld({
      source,
      at: firstTarget(groupsOf(tabs, source, paneShown)),
      from: from instanceof HTMLElement ? from : undefined,
    });
    setSaid(pickedUpSaid(sourceName(tabs, source, ctx)));
  }, []);

  const gripName = useCallback(
    (tab: number, pane: number) => {
      const name = sourceName(tabs, { kind: "pane", tab, pane }, ctx);
      return name === "" ? "Move this pane" : `Move ${name}`;
    },
    [tabs, ctx],
  );

  /** Ends placement mode. `back` says it was put back, and gives the keyboard to where it was. */
  const putDown = useCallback((back: "said" | "focused" | "none") => {
    const was = now.current.held;
    setHeld(undefined);
    setBoxes(undefined);
    if (was === undefined || back === "none") return;
    setSaid(PUT_BACK);
    if (back !== "focused") return;
    // A pane's grip; a chat's or a tab's own control where it was picked up, or, picked up from a
    // menu that has gone since, its row or its tab on the strip.
    const from = was.from?.isConnected === true ? was.from : null;
    const to =
      was.source.kind === "pane" ? (homeOf(was.source) ?? from) : (from ?? homeOf(was.source));
    to?.focus();
  }, []);

  /** Drops what is held on `to`, by the pointer's own rules; a refused target says why, and what
   *  is held stays held. */
  const dropOnTarget = useCallback((to: Target) => {
    const { tabs, ctx, held, boxes } = now.current;
    if (held === undefined) return;
    const size = to.kind === "pane" ? boxes?.panes.get(paneKey(to.tab, to.pane)) : undefined;
    const tried = placingDrop(tabs, held.source, to, ctx, size);
    setSaid(tried.said);
    if (!tried.dropped) return;
    now.current.change((current) => placingDrop(current, held.source, to, ctx, size).tabs);
    followTheDrop.current = true;
    setHeld(undefined);
    setBoxes(undefined);
  }, []);

  const placing = useMemo<Placing>(() => ({ pickUp, gripName }), [pickUp, gripName]);

  // ⌘⇧M picks up what has the keyboard: a sidebar chat, a tab on the strip, or a pane, the
  // focused one where the keyboard is on none of them.
  useEffect(() => {
    if (!active) return;
    const heard = (event: KeyboardEvent) => {
      if (pressed(event, ["pane.place"]) === undefined) return;
      event.preventDefault();
      event.stopPropagation();
      if (event.repeat || now.current.held !== undefined) return;
      const on = document.activeElement;
      const source = focusedSource(on, now.current.tabs);
      if (source === undefined) {
        setSaid("Nothing to pick up. Focus a pane, a chat in the sidebar or a tab.");
        return;
      }
      pickUp(source, on);
    };
    window.addEventListener("keydown", heard, true);
    return () => window.removeEventListener("keydown", heard, true);
  }, [active, pickUp]);

  const holding = held !== undefined;

  // While something is held: arrows, Tab, Enter, Space and Esc are placement mode's, and a
  // press anywhere but a target puts it back.
  useEffect(() => {
    if (!holding) return;
    const key = (event: KeyboardEvent) => {
      const held = now.current.held;
      if (held === undefined) return;
      if (event.altKey || event.metaKey || event.ctrlKey) return;
      const step: Step | undefined =
        event.key === "Tab"
          ? event.shiftKey
            ? "ShiftTab"
            : "Tab"
          : !event.shiftKey && isArrow(event.key)
            ? (event.key as Step)
            : undefined;
      if (step !== undefined) {
        event.preventDefault();
        event.stopPropagation();
        const at = stepFrom(groupsOf(now.current.tabs, held.source, paneShown), held.at, step);
        setHeld({ ...held, at });
        return;
      }
      if ((event.key === "Enter" || event.key === " ") && !event.shiftKey) {
        event.preventDefault();
        event.stopPropagation();
        if (!event.repeat) dropOnTarget(held.at);
      }
    };
    // A press on a target drops; on a grip, it picks that pane up instead; on a tab on the strip,
    // it brings that tab forward, as holding a drag over it does. Anywhere else it puts back.
    const press = (event: PointerEvent) => {
      const on = event.target instanceof Element ? event.target : null;
      if (on?.closest(`[${PLACE_TARGET}], [${GRIP_HANDLE}]`) != null) return;
      if (on?.closest(`[${TAB}]:not([${PANE}])`)?.closest(`[${STRIP}]`) != null) return;
      putDown("said");
    };
    // Esc puts it back. It is heard where the first Esc goes while a tooltip is up (the target
    // the keyboard is on shows one), so one Esc takes both down.
    const release = onTooltipEscape(() => putDown("focused"));
    window.addEventListener("keydown", key, true);
    window.addEventListener("pointerdown", press, true);
    return () => {
      release();
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("pointerdown", press, true);
    };
  }, [holding, putDown, dropOnTarget]);

  // Where the targets are drawn: measured as something is picked up, as the tabs change under
  // it (a tab brought forward), and as the window is resized.
  useLayoutEffect(() => {
    if (!holding) return;
    const measure = () => setBoxes(measured());
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [holding, tabs]);

  // As the tabs change under what is held: the keyboard stays on a target that is still offered,
  // and what has closed is put down.
  useEffect(() => {
    const held = now.current.held;
    if (held === undefined) return;
    const still = stillHeld(tabs, held.source, held.at, paneShown);
    if ("gone" in still) {
      setHeld(undefined);
      setBoxes(undefined);
      setSaid(still.gone);
      // The keyboard was on a target, which has gone with it: the focused pane's grip has it.
      const focused = focusedPane(tabs);
      if (focused !== undefined) gripOf(focused.tab, focused.pane)?.focus();
      return;
    }
    if (still.at !== held.at) setHeld({ ...held, at: still.at });
  }, [tabs, setSaid]);

  // The keyboard is on the target placement mode is on.
  const at = held?.at;
  useEffect(() => {
    if (at === undefined || boxes === undefined) return;
    const target = document.querySelector<HTMLElement>(`[${PLACE_TARGET}="${targetKey(at)}"]`);
    if (target !== null && document.activeElement !== target) target.focus();
  }, [at, boxes]);

  // After a drop with the keys, the grip of the pane that moved has the keyboard.
  useEffect(() => {
    if (!followTheDrop.current) return;
    followTheDrop.current = false;
    const at = focusedPane(tabs);
    if (at !== undefined) gripOf(at.tab, at.pane)?.focus();
  }, [tabs]);

  const targets =
    held === undefined || boxes === undefined ? null : (
      <PlacementTargets tabs={tabs} ctx={ctx} held={held} boxes={boxes} onPlace={dropOnTarget} />
    );

  const layer = (
    <>
      {targets}
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
        {told.said === "" ? null : <span key={told.n}>{told.said}</span>}
      </div>
    </>
  );

  const undoLine =
    undoable && last !== undefined && lineGone !== last ? (
      <Notice
        cause="layout-changed"
        tone="news"
        fixes={[{ label: "Undo", onPress: takeBack }]}
        onDismiss={() => setLineGone(last)}
      >
        {last.did}
      </Notice>
    ) : null;

  return { layer, undoLine, undo: takeBack, placing };
}

const paneKey = (tab: number, pane: number) => `${tab}:${pane}`;

/** Pane `pane` of tab `tab`, where it is drawn as a pane: not a side folded to a rail (#4766). */
function paneOf(tab: number, pane: number): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[${PANE}="${pane}"][${TAB}="${tab}"]`);
}

const paneShown = (tab: number, pane: number) => paneOf(tab, pane) !== null;

/** What a pane gives the keyboard to when its grip is not drawn. */
const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

const drawn = (el: HTMLElement) => getComputedStyle(el).display !== "none";

/**
 * Where pane `pane` of tab `tab` takes the keyboard: its grip; or, where the grip is not drawn
 * (a narrow task pane, D15), the pane's first control that is, so the keyboard stays in the
 * pane rather than falling to the page.
 */
function gripOf(tab: number, pane: number): HTMLElement | null {
  const at = paneOf(tab, pane);
  if (at === null) return null;
  const grip = at.querySelector<HTMLElement>(`[${GRIP_HANDLE}]`);
  if (grip !== null && drawn(grip)) return grip;
  return [...at.querySelectorAll<HTMLElement>(FOCUSABLE)].find(drawn) ?? null;
}

/** Where `source` is drawn and takes the keyboard: a pane's grip, a chat's sidebar row, or a
 *  tab on the strip. */
function homeOf(source: Source): HTMLElement | null {
  if (source.kind === "pane") return gripOf(source.tab, source.pane);
  if (source.kind === "chat")
    return document.querySelector<HTMLElement>(`[${CHAT_ROW}="${source.session}"]`);
  const cell = document.querySelector<HTMLElement>(
    `[${STRIP}] [${TAB}="${source.tab}"]:not([${PANE}])`,
  );
  return cell?.matches('[role="tab"]') === true
    ? cell
    : (cell?.querySelector<HTMLElement>('[role="tab"]') ?? cell);
}

/** The tab in front's focused pane, if a tab is in front. */
function focusedPane(tabs: Tabs): { tab: number; pane: number } | undefined {
  const front = tabs.inFront;
  const pane = front === undefined ? undefined : tabs.byId[front]?.focused;
  return front === undefined || pane === undefined ? undefined : { tab: front, pane };
}

const boxOfRect = (r: DOMRect): Box => ({
  left: r.left,
  top: r.top,
  width: r.width,
  height: r.height,
});

/** A target's key, as its button is marked: its pane and place, or the strip. */
function targetKey(target: Target): string {
  return target.kind === "strip" ? "strip" : `${paneKey(target.tab, target.pane)}:${target.place}`;
}

/** Where every pane and the strip are on screen now. */
function measured(): Boxes {
  const panes = new Map<string, Box>();
  for (const pane of document.querySelectorAll(`[${PANE}]`)) {
    const r = pane.getBoundingClientRect();
    const key = paneKey(Number(pane.getAttribute(TAB)), Number(pane.getAttribute(PANE)));
    panes.set(key, boxOfRect(r));
  }
  const strip = document.querySelector(`[${STRIP}]`)?.getBoundingClientRect();
  return { panes, strip: strip === undefined ? undefined : boxOfRect(strip) };
}

/**
 * **What placement mode picks up from what has the keyboard** (⌘⇧M): a sidebar row's chat, a
 * tab on the strip, the pane the keyboard is in, or else the tab in front's focused pane.
 */
function focusedSource(on: Element | null, tabs: Tabs): Source | undefined {
  const row = on?.closest(`[${CHAT_ROW}]`);
  if (row != null) return { kind: "chat", session: Number(row.getAttribute(CHAT_ROW)), chat: "" };
  // A pane carries its tab's number too, and the strip may be drawn in a pane's top: a tab is
  // one on the strip that is not a pane.
  const tab = on?.closest(`[${TAB}]:not([${PANE}])`);
  if (tab != null && tab.closest(`[${STRIP}]`) !== null)
    return { kind: "tab", tab: Number(tab.getAttribute(TAB)) };
  const pane = on?.closest(`[${PANE}]`);
  if (pane != null)
    return {
      kind: "pane",
      tab: Number(pane.getAttribute(TAB)),
      pane: Number(pane.getAttribute(PANE)),
    };
  const at = focusedPane(tabs);
  return at === undefined ? undefined : { kind: "pane", ...at };
}

const PLACE_ICON: Record<Place, ReactNode> = {
  top: <ArrowUp aria-hidden="true" />,
  left: <ArrowLeft aria-hidden="true" />,
  middle: <ArrowLeftRight aria-hidden="true" />,
  right: <ArrowRight aria-hidden="true" />,
  bottom: <ArrowDown aria-hidden="true" />,
};

/**
 * **Placement mode's targets** (#4764): five on every other pane of the tab in front, in a cross
 * in its middle, and a New tab at the strip's end, each a real button named by what dropping
 * there would do, or by its place with why it can't be dropped on. Only the one the keyboard is
 * on is in the tab sequence; Tab and the arrows move it (`stepFrom`). The result of the one it is
 * on is drawn before the drop, as a pointer's is.
 */
function PlacementTargets({
  tabs,
  ctx,
  held,
  boxes,
  onPlace,
}: {
  tabs: Tabs;
  ctx: DropContext;
  held: Held;
  boxes: Boxes;
  onPlace: (to: Target) => void;
}) {
  const { source, at } = held;
  const verdictOf = (to: Target): Verdict =>
    judgeDrop(
      tabs,
      source,
      to,
      ctx,
      to.kind === "pane" ? boxes.panes.get(paneKey(to.tab, to.pane)) : undefined,
    );
  const button = (to: Target, there: string) => {
    const verdict = verdictOf(to);
    const current = sameTarget(to, at);
    const marks = {
      [PLACE_TARGET]: targetKey(to),
      tabIndex: current ? 0 : -1,
      isDisabled: !verdict.ok,
      tooltip: verdict.ok ? undefined : verdict.why,
      "aria-label": verdict.ok ? verdict.says : targetSaid(to, there),
      onPress: () => onPlace(to),
    };
    return to.kind === "strip" ? (
      <Button {...marks} size="sm" icon={<SquareArrowOutUpRight aria-hidden="true" />}>
        New tab
      </Button>
    ) : (
      <IconButton {...marks} size="sm" icon={PLACE_ICON[to.place]} />
    );
  };
  const heldBox =
    source.kind === "pane" ? boxes.panes.get(paneKey(source.tab, source.pane)) : undefined;
  const atBox = at.kind === "pane" ? boxes.panes.get(paneKey(at.tab, at.pane)) : undefined;
  const atVerdict = verdictOf(at);
  return (
    <>
      {heldBox !== undefined && tabs.inFront === (source.kind === "pane" ? source.tab : -1) && (
        <div className="pane-placing-held" aria-hidden="true" style={boxStyle(heldBox)} />
      )}
      {atBox !== undefined && at.kind === "pane" && (
        <div
          className="pane-drop-preview"
          data-refused={!atVerdict.ok || undefined}
          aria-hidden="true"
          style={boxStyle(previewOf(atBox, at.place))}
        />
      )}
      {groupsOf(tabs, source, paneShown).map((group) => {
        if (group.kind === "strip") {
          if (boxes.strip === undefined) return null;
          return (
            <div
              key="strip"
              className="pane-placing-strip"
              style={{
                top: boxes.strip.top,
                left: boxes.strip.left + boxes.strip.width,
                height: boxes.strip.height,
              }}
            >
              {button(group, "")}
            </div>
          );
        }
        const box = boxes.panes.get(paneKey(group.tab, group.pane));
        if (box === undefined) return null;
        const there =
          sourceName(tabs, { kind: "pane", tab: group.tab, pane: group.pane }, ctx) || "this pane";
        return (
          <div
            key={paneKey(group.tab, group.pane)}
            role="group"
            aria-label={`Places by ${there}`}
            className="pane-placing"
            style={boxStyle(box)}
          >
            {targetsOf(group).map((to) => (
              <span
                key={targetKey(to)}
                className="pane-placing-at"
                data-place={to.kind === "pane" ? to.place : undefined}
              >
                {button(to, there)}
              </span>
            ))}
          </div>
        );
      })}
    </>
  );
}

const boxStyle = (box: Box) => ({
  left: box.left,
  top: box.top,
  width: box.width,
  height: box.height,
});

/**
 * **A pane's grip, as a control** (#4764): its press, or Enter or Space on it, picks the pane up
 * for placement mode, and a drag from it moves the pane as a drag from its head does (#4763).
 * Drawn only where the window can place a pane ({@link PlacingHere}).
 */
export function PaneGrip({ tab, pane }: { tab: number; pane: number }) {
  const pickUp = useContext(PickUpHere);
  const gripName = useContext(GripNamesHere);
  const self = useRef<HTMLButtonElement>(null);
  if (pickUp === undefined || gripName === undefined) return null;
  return (
    <IconButton
      ref={self}
      size="sm"
      variant="ghost"
      icon={<GripVertical aria-hidden="true" />}
      aria-label={gripName(tab, pane)}
      tooltip={`Drag it, or press to place it with the keys (${keySaid("pane.place")})`}
      // F6 into the editor passes over it to the pane's own controls, as over the strip; Tab
      // still stops on it.
      {...{ [GRIP_HANDLE]: "", [REGION_CHROME]: "" }}
      onPress={() => pickUp({ kind: "pane", tab, pane }, self.current)}
    />
  );
}

/** What a press on `target` picks up, if anything: a pane by its grip, a sidebar row's chat, or
 *  a tab on the strip (with the strip's box, which it must be pulled down off first). */
function pickedUp(target: Element): { source: Source; strip?: DOMRect } | undefined {
  // A press on a control inside a head or a row is that control's, never a drag; but the grip's
  // own control is the grip (#4764).
  const control = target.closest(`[${GRIP_HANDLE}]`) !== null ? null : target.closest(CONTROLS);
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
