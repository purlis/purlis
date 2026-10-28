// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a view chooses for a column's cells** (#3671): whether they keep one line, how many
 * lines they wrap to, and how wide the column is. Both members of the table family take the same
 * choices on a column (`TableColumn`, `ControlColumn`), and each of their cells draws its words
 * by them here.
 *
 * - **`cut`**: its cells keep one line. Where the column is narrower than a cell's words, they are
 *   cut with an ellipsis and the whole words are the cell's tooltip.
 * - **`clamp`**: its cells wrap to at most that many lines and are cut after them, the whole words
 *   in the tooltip (a description column, "What it does", `clamp={2}`).
 * - **`size`**: `fit`, as narrow as its widest cell on one line; `fill`, the room the other
 *   columns leave, down to its `minWidth`. A table has at most one column that fills.
 * - **`width`, `minWidth`, `maxWidth`**: a length in px (a number) or any CSS length a view
 *   places a table with. `width` is the column's own width; `minWidth` and `maxWidth` hold its
 *   cells' words between them, so a `cut` column with a `maxWidth` is cut there.
 *
 * A column that chooses none of this draws exactly as it did: its cells' words are handed on as
 * they are, wrapped as the table wraps them.
 *
 * **The tooltip of a cut cell** says its whole words, and only while they are cut: under the
 * pointer after the window's tooltip delay, and at once while the keyboard is on the cell (in a
 * table of data, the grid's arrows) or on the words (in a table of controls, where words cut
 * short are a Tab stop of their own, as a column's shortened name is). Esc takes it down without
 * moving the keyboard (WCAG 1.4.13). The words are still the cell's, all of them, so a screen
 * reader hears them whole and the tooltip describes nothing more.
 */
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { useFocusRing } from "react-aria";
import {
  AnchoredTooltip,
  onTooltipEscape,
  TOOLTIP_CLOSE_DELAY,
  TOOLTIP_DELAY,
} from "../tooltip/Tooltip";

/** How a column shares the table's width: as narrow as its cells, or the room left. */
export type ColumnSize = "fit" | "fill";

/** A column's length: px as a number, or a CSS length (`"12rem"`, `"30%"`). */
export type ColumnLength = number | string;

/** **The choices a view makes for a column** (#3671), on `TableColumn` and `ControlColumn`. */
export interface ColumnChoices {
  /** Its cells keep one line, cut with an ellipsis where it is narrower, whole in the tooltip. */
  cut?: boolean;
  /** Its cells wrap to at most this many lines, cut after them, whole in the tooltip. */
  clamp?: number;
  /** `fit`: as narrow as its widest cell; `fill`: the room the other columns leave. */
  size?: ColumnSize;
  /** The column's own width. */
  width?: ColumnLength;
  /** The least its cells' words are drawn in. */
  minWidth?: ColumnLength;
  /** The most its cells' words are drawn in: a `cut` column is cut there. */
  maxWidth?: ColumnLength;
}

/** What a column's choices make of its cells' words, read back from its name's element. */
export interface Choices {
  cut: boolean;
  clamp: number | null;
  size: ColumnSize | null;
  least: string | null;
  most: string | null;
}

/** A length as CSS writes it: a number is px. */
const length = (one: ColumnLength | undefined): string | undefined =>
  one === undefined ? undefined : typeof one === "number" ? `${one}px` : one;

/** The lines a clamp keeps: a whole number of at least one, or none. */
const lines = (clamp: number | undefined): number | undefined =>
  clamp === undefined || !Number.isFinite(clamp) || clamp < 1 ? undefined : Math.floor(clamp);

/**
 * **A column's choices as its name's element carries them**: the attributes its cells read back
 * (`data-cut`, `data-clamp`, `data-size`, `data-least`, `data-most`) and its own width.
 */
export function columnAttributes(choices: ColumnChoices): {
  attributes: Record<`data-${string}`, string | undefined>;
  style: CSSProperties | undefined;
} {
  const clamp = lines(choices.clamp);
  const width = length(choices.width);
  return {
    attributes: {
      "data-cut": choices.cut === true && clamp === undefined ? "" : undefined,
      "data-clamp": clamp === undefined ? undefined : String(clamp),
      "data-size": choices.size,
      "data-least": length(choices.minWidth),
      "data-most": length(choices.maxWidth),
    },
    style: width === undefined ? undefined : { inlineSize: width },
  };
}

/** A string attribute's value, or `null`. */
const text = (value: unknown): string | null => (typeof value === "string" ? value : null);

/**
 * **What a column chose, read from its attributes**: a data table's column's props (React Aria
 * keeps them on the column's node) or a table of controls' column's element. `null` for a
 * column that chose nothing, whose cells draw as they always have.
 */
export function choicesOf(attributes: Record<string, unknown> | null | undefined): Choices | null {
  if (attributes === null || attributes === undefined) return null;
  const clamp = Number(text(attributes["data-clamp"]) ?? NaN);
  const size = text(attributes["data-size"]);
  const choices: Choices = {
    cut: attributes["data-cut"] !== undefined && attributes["data-cut"] !== null,
    clamp: Number.isFinite(clamp) && clamp >= 1 ? clamp : null,
    size: size === "fit" || size === "fill" ? size : null,
    least: text(attributes["data-least"]),
    most: text(attributes["data-most"]),
  };
  const chose =
    choices.cut ||
    choices.clamp !== null ||
    choices.size !== null ||
    choices.least !== null ||
    choices.most !== null;
  return chose ? choices : null;
}

/** The same, from a table of controls' column's element (`dataset` names them in camel case). */
export function choicesOfElement(head: HTMLElement | null | undefined): Choices | null {
  if (head === null || head === undefined) return null;
  const attributes: Record<string, unknown> = {};
  for (const name of ["data-cut", "data-clamp", "data-size", "data-least", "data-most"]) {
    const value = head.getAttribute(name);
    if (value !== null) attributes[name] = value;
  }
  return choicesOf(attributes);
}

/** Whether `element`'s words are drawn cut: wider or taller than its box shows. */
function drawnCut(element: HTMLElement): boolean {
  return (
    element.scrollWidth > element.clientWidth + 0.5 ||
    element.scrollHeight > element.clientHeight + 0.5
  );
}

/**
 * **A cell's words, as its column chose** (#3671). `keyboardOn` is a table of data's: the grid
 * has the keyboard on the cell. `ownStop` is a table of controls': its words cut short are a Tab
 * stop of their own, since its cells are no stops.
 */
export function CellWords({
  choices,
  keyboardOn = false,
  ownStop = false,
  children,
}: {
  choices: Choices | null;
  keyboardOn?: boolean;
  ownStop?: boolean;
  children?: ReactNode;
}) {
  if (choices === null) return <>{children}</>;
  return (
    <Chosen choices={choices} keyboardOn={keyboardOn} ownStop={ownStop}>
      {children}
    </Chosen>
  );
}

function Chosen({
  choices,
  keyboardOn,
  ownStop,
  children,
}: {
  choices: Choices;
  keyboardOn: boolean;
  ownStop: boolean;
  children?: ReactNode;
}) {
  const words = useRef<HTMLSpanElement>(null);
  const cuts = choices.cut || choices.clamp !== null;
  const [cut, setCut] = useState(false);
  // The whole words, as they were last measured: what the tooltip says.
  const [whole, setWhole] = useState("");
  // Measured as it is drawn and again whenever its box changes: a column narrowed by a split, a
  // window made smaller, words that changed.
  useLayoutEffect(() => {
    const element = words.current;
    if (!cuts || element === null) return;
    const measure = () => {
      setCut(drawnCut(element));
      setWhole(element.textContent ?? "");
    };
    measure();
    if (typeof ResizeObserver !== "function") return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [cuts, children]);

  // The target a test and the tooltip family read the hint on (`data-tooltip-target`): a data
  // table's cell, which the grid's arrows reach, or the words themselves in a table of controls.
  useEffect(() => {
    const element = words.current;
    const target = ownStop ? element : element?.closest<HTMLElement>('[data-part="cell"]');
    if (target === null || target === undefined) return;
    if (cut) target.setAttribute("data-tooltip-target", "");
    else target.removeAttribute("data-tooltip-target");
    return () => target.removeAttribute("data-tooltip-target");
  }, [cut, ownStop]);

  const { focusProps, isFocusVisible } = useFocusRing();
  const [pointed, setPointed] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const soon = useRef(0);
  const leaving = useRef(0);
  useEffect(
    () => () => {
      window.clearTimeout(soon.current);
      window.clearTimeout(leaving.current);
    },
    [],
  );
  const onPointerEnter = useCallback((event: React.PointerEvent) => {
    if (event.pointerType === "touch") return;
    window.clearTimeout(leaving.current);
    window.clearTimeout(soon.current);
    soon.current = window.setTimeout(() => setPointed(true), TOOLTIP_DELAY);
  }, []);
  const onPointerLeave = useCallback(() => {
    window.clearTimeout(soon.current);
    // Time for the pointer to cross onto the tooltip, which keeps it (WCAG 1.4.13, hoverable).
    leaving.current = window.setTimeout(() => setPointed(false), TOOLTIP_CLOSE_DELAY);
  }, []);
  const onTip = useCallback(() => window.clearTimeout(leaving.current), []);

  const keyboard = ownStop ? isFocusVisible : keyboardOn;
  // A dismissed tooltip comes back only once the pointer and the keyboard have both left.
  const away = !pointed && !keyboard;
  const [wasAway, setWasAway] = useState(away);
  if (away !== wasAway) {
    setWasAway(away);
    if (away) setDismissed(false);
  }
  const shown = cut && (pointed || keyboard) && !dismissed;
  useEffect(() => {
    if (!shown) return;
    return onTooltipEscape(() => setDismissed(true));
  }, [shown]);

  // The view's lengths, on the words: a column that fills is at least its whole cell, and at
  // least its own least where it chose one.
  const fills = choices.size === "fill";
  const style: CSSProperties = {
    minInlineSize:
      choices.least === null ? undefined : fills ? `max(100%, ${choices.least})` : choices.least,
    maxInlineSize: choices.most ?? undefined,
    WebkitLineClamp: choices.clamp ?? undefined,
  };
  const stop = ownStop && cut;
  return (
    <>
      <span
        ref={words}
        data-scope="table"
        data-part="words"
        data-cut={choices.cut && choices.clamp === null ? "" : undefined}
        data-clamp={choices.clamp ?? undefined}
        data-size={choices.size ?? undefined}
        style={style}
        tabIndex={stop ? 0 : undefined}
        {...(stop ? focusProps : {})}
        onPointerEnter={cuts ? onPointerEnter : undefined}
        onPointerLeave={cuts ? onPointerLeave : undefined}
      >
        {children}
      </span>
      {shown && (
        <AnchoredTooltip anchor={words} onPointerEnter={onTip} onPointerLeave={onPointerLeave}>
          {whole}
        </AnchoredTooltip>
      )}
    </>
  );
}
