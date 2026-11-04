// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A row's tooltip** (#2911): what a row of a list, a tree or a table says that its words do
 * not (the whole of words cut to a line, the date a memory was written), beside the row, as the
 * window's shared tooltip (`ui/tooltip`, #2908). A row is drawn by its collection, so no
 * `Tooltip` can wrap it; the row shows its own instead:
 *
 * - **under the pointer**, after the window's tooltip delay, until the pointer leaves;
 * - **at once while the keyboard is on the row** (its focus ring is showing), until it leaves;
 * - **Esc takes it down** without moving the keyboard, and is the tooltip's alone (WCAG 1.4.13).
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
  TOOLTIP_DELAY,
  type Tip,
} from "../tooltip/Tooltip";
import { drawnCut } from "../drawnCut";
import { useWindowLeft } from "../windowLeft";
import { RowCutWords, RowScope } from "./Row";

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
  const [dismissed, setDismissed] = useState(false);
  const soon = useRef(0);
  const pointer = useRef(0);
  useEffect(() => () => window.clearTimeout(soon.current), []);
  // The last row told of attention, told when the pointer goes even once it no longer asks.
  const attended = useRef(attend);
  useLayoutEffect(() => {
    if (attend !== undefined) attended.current = attend;
  });
  const onPointerEnter = useCallback(
    (event: React.PointerEvent) => {
      if (event.pointerType === "touch") return;
      pointer.current = event.clientX;
      window.clearTimeout(soon.current);
      soon.current = window.setTimeout(() => {
        // Asked only once the pointer has rested: a pointer passing over measures nothing.
        attend?.("pointer", true);
        setPointed(pointer.current);
      }, TOOLTIP_DELAY);
    },
    [attend],
  );
  const onPointerMove = useCallback((event: React.PointerEvent) => {
    pointer.current = event.clientX;
  }, []);
  const onPointerLeave = useCallback(() => {
    window.clearTimeout(soon.current);
    attended.current?.("pointer", false);
    setPointed(null);
    setDismissed(false);
  }, []);
  const has = tip !== undefined && tip !== null && tip !== "";
  const listens = has || attend !== undefined;
  // Gone from the window, which an engine may not say on the row (#3095).
  useWindowLeft((why) => {
    if (why === "pointer") onPointerLeave();
  }, listens);
  if (!listens) return { hover: {}, drawn: () => null };
  return {
    hover: { onPointerEnter, onPointerMove, onPointerLeave },
    drawn: (keyboardOn) => (
      <>
        {attend !== undefined && (
          <KeyboardAttends on={keyboardOn && !pointerOnly} attend={attend} />
        )}
        {has && (pointed !== null || (keyboardOn && !pointerOnly)) && !dismissed && (
          <Shown pointer={pointed} onEscape={() => setDismissed(true)}>
            {tip}
          </Shown>
        )}
      </>
    ),
  };
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
  /** The pointer's x it was measured for; `null` for the row's words. */
  by: number | null;
  /** `null` where nothing could be measured: the whole row. */
  at: { left: number; width: number } | null;
}

/** The tooltip while it is up, which the first Esc takes down. */
function Shown({
  pointer,
  onEscape,
  children,
}: {
  /** The pointer's x in the window, where the pointer opened it. */
  pointer: number | null;
  onEscape: () => void;
  children: Tip;
}) {
  const scope = useContext(RowScope);
  const anchor = useRef<HTMLSpanElement>(null);
  // The anchor is set by the pointer or the words before the tooltip is drawn, which measures it,
  // and again when the pointer comes to a row the keyboard opened it on.
  const [place, setPlace] = useState<Place | null>(null);
  useLayoutEffect(() => {
    setPlace(placeOf(anchor.current, scope, pointer));
  }, [scope, pointer]);
  const placed = place !== null && place.by === pointer;
  useEffect(() => onTooltipEscape(() => flushSync(onEscape)), [onEscape]);
  // **The tooltip is the row's description while it is shown**, as every tooltip is its
  // target's: the row is drawn by its collection, so it is said on the row's element here.
  const fallback = useId();
  const tooltip = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const row = anchor.current?.closest(`[data-scope="${scope}"][data-part="root"]`);
    const shown = tooltip.current;
    if (!row || !shown) return;
    if (shown.id === "") shown.id = fallback;
    const id = shown.id;
    const was = row.getAttribute("aria-describedby");
    row.setAttribute("aria-describedby", was ? `${was} ${id}` : id);
    return () => {
      const now = (row.getAttribute("aria-describedby") ?? "")
        .split(/\s+/)
        .filter((one) => one !== "" && one !== id);
      if (now.length === 0) row.removeAttribute("aria-describedby");
      else row.setAttribute("aria-describedby", now.join(" "));
    };
  }, [fallback, scope, placed]);
  const at = placed ? place.at : null;
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
        <AnchoredTooltip key={pointer ?? "words"} anchor={anchor} tooltipRef={tooltip}>
          {children}
        </AnchoredTooltip>
      )}
    </>
  );
}

/**
 * Where the anchor `span` (across its whole row, by its stylesheet) is set: on the pointer's x,
 * a line with no width, or on the row's words, as far as they are drawn. Where neither can be
 * measured it stays on the whole row.
 */
function placeOf(span: HTMLSpanElement | null, scope: string, pointer: number | null): Place {
  // The row's element, which the anchor is set across (`list.css`).
  const row = span?.closest(`[data-scope="${scope}"][data-part="root"]`);
  if (row === null || row === undefined) return { by: pointer, at: null };
  const box = row.getBoundingClientRect();
  if (pointer !== null) return { by: pointer, at: { left: pointer - box.left, width: 0 } };
  const words = wordsOf(row);
  if (words === null) return { by: pointer, at: null };
  const [from, to] = words;
  return { by: pointer, at: { left: from - box.left, width: Math.max(0, to - from) } };
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
