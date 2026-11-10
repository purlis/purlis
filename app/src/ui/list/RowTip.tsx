// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A row's tooltip** (#2911): what a row of a list, a tree or a table says that its words do
 * not (the whole of words cut to a line, the date a memory was written), beside the row, as the
 * window's shared tooltip (`ui/tooltip`, #2908). A row is drawn by its collection, so no
 * `Tooltip` can wrap it; the row shows its own instead:
 *
 * - **under the pointer**, after the window's tooltip delay, until the pointer has left the row
 *   and the tooltip a moment ({@link TOOLTIP_CLOSE_DELAY}): the pointer can cross onto it and
 *   read it (WCAG 1.4.13, hoverable, #5038);
 * - **at once while the keyboard is on the row** (its focus ring is showing), until it leaves;
 * - **Esc takes it down** without moving the keyboard, and is the tooltip's alone (WCAG 1.4.13);
 *   it is back on the next rest of the pointer or visit of the keyboard (#5038).
 *
 * It is anchored by a part that draws nothing and takes no pointer, and stands **beside the
 * pointer or the row's words, not the whole row** (#3328): a row across the window has no room
 * at either side of it, where a tooltip came out a column of single words.
 *
 * - The pointer opened it: it stands by the pointer, where it rested as the delay ran out.
 * - The keyboard opened it: it stands by the row's words, as far as they are drawn.
 * - Neither can be measured (no layout): by the whole row.
 */
import {
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { flushSync } from "react-dom";
import {
  AnchoredTooltip,
  onTooltipEscape,
  says,
  TOOLTIP_CLOSE_DELAY,
  TOOLTIP_DELAY,
  type Tip,
} from "../tooltip/Tooltip";
import { drawnCut } from "../drawnCut";
import { useWindowLeft } from "../windowLeft";
import { RowCutWords, RowHeard, RowScope } from "./Row";

/** What a row with a tooltip hands its element, and draws inside it. */
export interface RowTipped {
  /** The pointer's handlers for the row: its own, so a row that runs nothing shows one too. */
  hover: {
    onPointerEnter?: (event: React.PointerEvent) => void;
    onPointerMove?: (event: React.PointerEvent) => void;
    onPointerLeave?: (event: React.PointerEvent) => void;
  };
  /** What the row draws inside itself: the anchor, and the tooltip while it is up. */
  drawn: (keyboardOn: boolean) => ReactNode;
}

/** The handlers and the drawing of a row's tooltip; nothing at all for a row without one. */
export function useRowTip(
  tip: Tip | undefined,
  /** Shown to the pointer alone: a tip that only repeats the row (a file's path) would be heard
   *  on every arrow step. Nothing shows it on the keyboard, and so no Esc is spent on it. */
  pointerOnly = false,
  /** Told as the pointer's delay runs out on the row and as the keyboard comes to it, and as
   *  each goes, for a row whose tip is known only then (`useListRowTip`): the row listens for
   *  both whether or not it has a tip yet. */
  attend?: Attend,
): RowTipped {
  // Where the pointer rested as the delay ran out (its x in the window), while it is shown.
  const [pointed, setPointed] = useState<number | null>(null);
  // Esc took it down: until the pointer rests again or the keyboard comes again (#5038).
  const [dismissed, setDismissed] = useState(false);
  const soon = useRef(0);
  const later = useRef(0);
  const pointer = useRef(0);
  useEffect(
    () => () => {
      window.clearTimeout(soon.current);
      window.clearTimeout(later.current);
    },
    [],
  );
  // The last row told of attention, told when the pointer goes even once it no longer asks.
  const attended = useRef(attend);
  useLayoutEffect(() => {
    if (attend !== undefined) attended.current = attend;
  });
  /** The pointer has gone from the row and its tooltip: the pointer's tooltip goes. */
  const gone = useCallback(() => {
    window.clearTimeout(soon.current);
    window.clearTimeout(later.current);
    attended.current?.("pointer", false);
    setPointed(null);
  }, []);
  /** The pointer is on the row or its tooltip: what was to take the tooltip down does not. */
  const stay = useCallback(() => window.clearTimeout(later.current), []);
  /** The pointer has left the row or its tooltip: the tooltip goes unless it comes back. */
  const leaving = useCallback(() => {
    window.clearTimeout(soon.current);
    window.clearTimeout(later.current);
    later.current = window.setTimeout(gone, TOOLTIP_CLOSE_DELAY);
  }, [gone]);
  const onPointerEnter = useCallback(
    (event: React.PointerEvent) => {
      if (event.pointerType === "touch" || !onRow(event)) return;
      pointer.current = event.clientX;
      stay();
      window.clearTimeout(soon.current);
      soon.current = window.setTimeout(() => {
        // Asked only once the pointer has rested: a pointer passing over measures nothing.
        attend?.("pointer", true);
        setDismissed(false);
        setPointed((was) => was ?? pointer.current);
      }, TOOLTIP_DELAY);
    },
    [attend, stay],
  );
  const onPointerMove = useCallback((event: React.PointerEvent) => {
    if (onRow(event)) pointer.current = event.clientX;
  }, []);
  const onPointerLeave = useCallback(
    (event: React.PointerEvent) => {
      if (onRow(event)) leaving();
    },
    [leaving],
  );
  // **Esc takes it down until the next rest or visit** (#5038): the pointer's tooltip with it, and
  // what was to take it down later, so a pointer that was on the tooltip as it went (and so never
  // leaves the row) leaves nothing to come back with the keyboard.
  const dismiss = useCallback(() => {
    gone();
    setDismissed(true);
  }, [gone]);
  const has = tip !== undefined && tip !== null && tip !== "";
  const listens = has || attend !== undefined;
  // Gone from the window, which an engine may not say on the row (#3095).
  useWindowLeft((why) => {
    if (why === "pointer") gone();
  }, listens);
  if (!listens) return { hover: {}, drawn: () => null };
  return {
    hover: { onPointerEnter, onPointerMove, onPointerLeave },
    drawn: (keyboardOn) => {
      // The keyboard on the row shows its tooltip, unless it is shown to the pointer alone.
      const byKeyboard = keyboardOn && !pointerOnly;
      return (
        <>
          {attend !== undefined && <KeyboardAttends on={byKeyboard} attend={attend} />}
          {/* The keyboard coming to the row brings back a tooltip Esc took down (#5038). */}
          <KeyboardComes on={byKeyboard} onCome={() => setDismissed(false)} />
          {has && (pointed !== null || byKeyboard) && !dismissed && (
            <Shown openedAt={pointed} stay={stay} leaving={leaving} onEscape={dismiss}>
              {tip}
            </Shown>
          )}
        </>
      );
    },
  };
}

/**
 * **Whether a pointer's event is the row's own element's**: the row's tooltip is drawn inside the
 * row, so React hands the row the pointer's events on the tooltip too, which the tooltip answers
 * itself (`Shown`).
 */
const onRow = (event: React.PointerEvent) =>
  event.currentTarget instanceof Node &&
  event.target instanceof Node &&
  event.currentTarget.contains(event.target);

/** Tells the row as the keyboard comes to it. */
function KeyboardComes({ on, onCome }: { on: boolean; onCome: () => void }) {
  const came = useRef(onCome);
  useLayoutEffect(() => {
    came.current = onCome;
  });
  useLayoutEffect(() => {
    if (on) came.current();
  }, [on]);
  return null;
}

/** What the pointer or the keyboard says of its attention to a row. */
type Attend = (by: "pointer" | "keyboard", on: boolean) => void;

/** Tells the row whether the keyboard is on it, before the frame shows that: so the tooltip it
 *  then has stands in that frame. */
function KeyboardAttends({ on, attend }: { on: boolean; attend: Attend }) {
  useLayoutEffect(() => {
    attend("keyboard", on);
    return () => attend("keyboard", false);
  }, [on, attend]);
  return null;
}

/**
 * **A list's row's tooltip, or the whole of its line cut short** (#4711): the row's own `tip`
 * where it has one; else, while a `RowBelow` that asks (`tipWhenCut`) is cut by its clamp, that
 * line's whole words, which are its row's description while they are shown (a row is named by its
 * `textValue` alone, so a reader hears the line only so). `hold` draws the row's content where its
 * `RowBelow` can say it is cut.
 *
 * **Measured only while the row has someone's attention** (#4351): once the pointer has rested
 * on it, and while the keyboard is on it, then again as the line's size or words change, until
 * both have gone. Drawing a list measures no line, so it forces no layout before its frame.
 */
export function useListRowTip(tip: Tip | undefined): {
  tipped: RowTipped;
  hold: (content: ReactNode) => ReactNode;
} {
  const own = tip !== undefined && says(tip);
  const [cut, setCut] = useState<string>();
  // Whether a `RowBelow` asks: only then does the row listen for attention at all.
  const [lined, setLined] = useState(false);
  const [attention, setAttention] = useState(NOBODY);
  const attentionNow = useRef(NOBODY);
  const attended = !own && lined && (attention.pointer || attention.keyboard);
  const attend = useCallback<Attend>((by, on) => {
    const was = attentionNow.current;
    if (was[by] === on) return;
    const now = { ...was, [by]: on };
    attentionNow.current = now;
    setAttention(now);
    // Nobody left: what was measured is forgotten, so attention coming back measures afresh.
    if (!now.pointer && !now.keyboard) setCut(undefined);
  }, []);
  // The line its `RowBelow` hands it; while it is attended, measured and watched as it changes.
  const line = useRef<HTMLElement | null>(null);
  const watching = useRef<{ stop: () => void } | null>(null);
  const measure = useCallback(() => {
    const element = line.current;
    setCut(element !== null && drawnCut(element) ? element.textContent || undefined : undefined);
  }, []);
  const watch = useCallback(() => {
    watching.current?.stop();
    measure();
    const element = line.current;
    if (element === null || typeof ResizeObserver !== "function") {
      watching.current = { stop: () => {} };
      return;
    }
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    watching.current = { stop: () => observer.disconnect() };
  }, [measure]);
  const told = useCallback(
    (element: HTMLElement | null) => {
      line.current = element;
      setLined(element !== null);
      // A line handed, or handed again with new words, while it is attended is measured now.
      if (watching.current !== null) watch();
    },
    [watch],
  );
  useLayoutEffect(() => {
    if (!attended) return;
    watch();
    return () => {
      watching.current?.stop();
      watching.current = null;
    };
  }, [attended, watch]);
  const tipped = useRowTip(own ? tip : cut, false, own || !lined ? undefined : attend);
  return {
    tipped,
    hold: (content) => <RowCutWords.Provider value={told}>{content}</RowCutWords.Provider>,
  };
}

/** Nobody's attention on a row. */
const NOBODY = { pointer: false, keyboard: false } as const;

/** Where a row's tooltip is anchored: across the row from `left`, `width` wide, in px. */
interface Place {
  /** `null` where nothing could be measured: the whole row. */
  at: { left: number; width: number } | null;
}

/**
 * The tooltip while it is up, which the first Esc takes down.
 *
 * **Anchored once, where it opened** (#3364): by the pointer, or by the row's words. The pointer
 * coming to rest on a row the keyboard opened it on moves nothing, so the tooltip is never drawn
 * afresh while it is up, and a screen reader is handed it once.
 */
function Shown({
  openedAt,
  stay,
  leaving,
  onEscape,
  children,
}: {
  /** The pointer's x in the window, where the pointer opened it; read as it opens. */
  openedAt: number | null;
  /** The pointer is on the tooltip, or back on its row: it stays (WCAG 1.4.13, hoverable). */
  stay: () => void;
  /** The pointer has left the tooltip for anything but its row: it goes unless it comes back. */
  leaving: () => void;
  onEscape: () => void;
  children: Tip;
}) {
  const scope = useContext(RowScope);
  const anchor = useRef<HTMLSpanElement>(null);
  // The anchor is set by the pointer or the words before the tooltip is drawn, which measures it.
  const [place, setPlace] = useState<Place | null>(null);
  const opened = useRef(openedAt);
  useLayoutEffect(() => {
    setPlace(placeOf(anchor.current, scope, opened.current));
  }, [scope]);
  useEffect(() => onTooltipEscape(() => flushSync(onEscape)), [onEscape]);
  // **The tooltip is the row's description while it is shown**, as every tooltip is its
  // target's: the row is drawn by its collection, so its row hears it (below).
  const fallback = useId();
  const tooltip = useRef<HTMLDivElement>(null);
  const placed = place !== null;
  const rowOf = useCallback(() => rowAround(anchor.current, scope), [scope]);
  // **Its row builds its description from its parts' ids**, the tooltip's among them (#6101,
  // #6123): every row with a tooltip, a list's and a tree's, hands its own (`RowHeard`). The
  // tooltip never writes the row's `aria-describedby`. A list's row draws it through React
  // Aria's `render`; a tree's row writes it on its element from its root (`useTreeRowHeard`),
  // the one hand write in the window (`describedBy.guard.test.ts`), because React Aria's
  // `TreeItem` takes no `render` and drops an `aria-describedby` it is handed.
  const heard = useContext(RowHeard);
  useEffect(() => {
    const shown = tooltip.current;
    if (heard === null || !shown) return;
    if (shown.id === "") shown.id = fallback;
    return heard.add("tip", shown.id);
  }, [fallback, placed, heard]);
  const at = place?.at ?? null;
  return (
    <>
      <span
        ref={anchor}
        data-scope={scope}
        data-part="tip-anchor"
        aria-hidden="true"
        style={at === null ? undefined : { left: at.left, right: "auto", width: at.width }}
      />
      {placed && (
        <AnchoredTooltip
          anchor={anchor}
          tooltipRef={tooltip}
          onPointerEnter={stay}
          onPointerLeave={(event) => {
            const to = event.relatedTarget;
            if (to instanceof Node && rowOf()?.contains(to) === true) stay();
            else leaving();
          }}
        >
          {children}
        </AnchoredTooltip>
      )}
    </>
  );
}

/** The row's element `inside` is drawn in: the family's root of the row's scope. */
const rowAround = (inside: Element | null | undefined, scope: string): Element | null =>
  inside?.closest(`[data-scope="${scope}"][data-part="root"]`) ?? null;

/**
 * Where the anchor `span` (across its whole row, by its stylesheet) is set: on the pointer's x,
 * a line with no width, or on the row's words, as far as they are drawn. Where neither can be
 * measured it stays on the whole row.
 */
function placeOf(span: HTMLSpanElement | null, scope: string, pointer: number | null): Place {
  // The row's element, which the anchor is set across (`list.css`).
  const row = rowAround(span, scope);
  if (row === null) return { at: null };
  const box = row.getBoundingClientRect();
  if (pointer !== null) return { at: { left: pointer - box.left, width: 0 } };
  const words = wordsOf(row);
  if (words === null) return { at: null };
  const [from, to] = words;
  return { at: { left: from - box.left, width: Math.max(0, to - from) } };
}

/** Where a row's words are drawn, across the window: their label's text, cut where it is cut. */
function wordsOf(row: Element): [number, number] | null {
  const label = row.querySelector('[data-part="label"]');
  if (label === null || typeof document.createRange !== "function") return null;
  const range = document.createRange();
  range.selectNodeContents(label);
  if (typeof range.getBoundingClientRect !== "function") return null;
  const text = range.getBoundingClientRect();
  if (text.width === 0) return null;
  const shown = label.getBoundingClientRect();
  return [Math.max(text.left, shown.left), Math.min(text.right, shown.right)];
}
