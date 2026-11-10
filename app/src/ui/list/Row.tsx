// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A row's anatomy** (#2911, spec #2897, ADR 0103): what a list's, a tree's and a table's row
 * holds, in the same places in all three. A row is the family's root; inside it its `lead` (a
 * mark, an icon, a persona's tile), its `label` (the words, which take the room and give way to
 * an ellipsis), the one `description` that may stand beside or under them, its `trail` (a
 * count, a state, the row's actions) at its end, and its `shortcut`, the key that does the same
 * (#2921), last.
 *
 * Each slot carries the `data-scope` of the family whose row it is in (`RowScope`), so a theme's
 * `list.label.text` reaches a list's words and never a tree's.
 */
import {
  createContext,
  useContext,
  useLayoutEffect,
  useRef,
  type CSSProperties,
  type ReactNode,
} from "react";

/** The families a row belongs to. */
export type RowFamily = "list" | "tree" | "table";

/** The family the row a slot is drawn in belongs to. */
export const RowScope = createContext<RowFamily>("list");

/**
 * **What a row's lines are heard by** (#6075, #6101): each part that says something about it (its
 * one line, a line under its words, a mark with words to say) hands its id here, and is read as
 * its description in `HEARD_ORDER`'s order. React Aria's `useOption` describes an option by its
 * `description` slot alone and takes no other id, so the list family describes it by these, and
 * then by what React Aria adds (its hint for dragging it).
 */
export interface PartsHeard {
  /** The ids its parts are drawn with: `${base}-${kind}`, one part of each kind to an option. */
  base: string;
  /** Hears the part `id` as `kind`; what it hands back stops hearing it. */
  add(kind: HeardKind, id: string): () => void;
}

/** The parts of a row heard as its description, in the order a reader hears them: an option's
 *  one line, its line under its words and its mark, then any row's tooltip while it is up
 *  (#6101). */
export const HEARD_ORDER = ["description", "below", "lead", "tip"] as const;
export type HeardKind = (typeof HEARD_ORDER)[number];

/** A part a row hears, by its kind and the id it is drawn with. */
export interface PartHeard {
  kind: HeardKind;
  id: string;
}

/** **The ids a row is described by**, from what its parts handed it, in `HEARD_ORDER`'s order;
 *  none where it was handed none. The list's rows and the tree's join them the same way. */
export function heardIds(parts: readonly PartHeard[]): string | undefined {
  const ids = HEARD_ORDER.flatMap((kind) =>
    parts.filter((one) => one.kind === kind).map((one) => one.id),
  );
  return ids.length === 0 ? undefined : ids.join(" ");
}

/**
 * **A row that builds its `aria-describedby` from what its parts hand it** (#6101, #6123): every
 * row with a tooltip, a list's option, a list's row and a tree's row. Its tooltip hands its id
 * here while it is up (`RowTip.tsx`), so the row's description is built in one place and a part
 * that comes or goes while the tooltip is up does not drop it. A list's row draws its element
 * with it through React Aria's `render` prop; a tree's, whose React Aria row takes no `render`,
 * writes it on its element from its root (`useTreeRowHeard`). `null` outside a row.
 */
export const RowHeard = createContext<HearsParts | null>(null);

/** What a row's registry is to its tooltip (`RowHeard`): only how to be heard. An option's
 *  parts also draw their ids from its `base` (`InOption`). */
export type HearsParts = Pick<PartsHeard, "add">;

/**
 * **Inside an option** (`ListOption`): its words are its name and its one line about it its
 * description, to a reader too, rather than the whole row's content run together. `null` in any
 * other row.
 *
 * **An option is named by its content, and its content is its words alone** (#6043): every
 * other part of it (its mark, its one line, its end, a line under it) is kept from a reader
 * (`aria-hidden`), and its one line is handed to it as its description, so it is heard once,
 * after the name. Named by `aria-labelledby` instead,
 * its visible words would be more than its name, which WCAG 2.5.3 refuses: axe's
 * `label-content-name-mismatch` counts every visible word, `aria-hidden` or not, as its label.
 *
 * **What else it draws that says something is heard too** (#6075): its line under its words
 * (`RowBelow`) and a mark given words to say (`RowLead`'s `said`) join its description after its
 * one line (`PartsHeard`). Kept from a reader as content, each is still read where the option's
 * `aria-describedby` points at it, so each is heard once.
 */
export const InOption = createContext<PartsHeard | null>(null);

/** What a part that is not an option's name carries inside an option: kept from a reader. */
function keptInOption(option: boolean): { "aria-hidden"?: true } {
  return option ? { "aria-hidden": true } : {};
}

/** **Heard in its option's description** (#6075): the id to give the part, `when` it says
 *  something, in an option; else none, and nothing is heard. */
function useHeardInOption(kind: HeardKind, when: boolean, given?: string): string | undefined {
  const option = useContext(InOption);
  const hears = option !== null && when;
  const id = hears ? (given ?? `${option.base}-${kind}`) : undefined;
  useLayoutEffect(
    () => (option !== null && id !== undefined ? option.add(kind, id) : undefined),
    [option, kind, id],
  );
  return id;
}

/** The sizes a row is drawn at: 24, 28 and 32 px at the default text size and density. */
export const ROW_SIZES = ["sm", "md", "lg"] as const;
export type RowSize = (typeof ROW_SIZES)[number];

/** What every slot takes: what it holds, and a place in its row (never a look). */
interface Slot {
  children?: ReactNode;
  /** A place for it in its row: margin, width, order. Never a look. */
  className?: string;
  style?: CSSProperties;
  id?: string;
  /** `data-*` attributes reach the element as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

function slot(part: string, named?: "label" | "description") {
  function RowSlot({ children, ...rest }: Slot) {
    const scope = useContext(RowScope);
    const option = useContext(InOption) !== null;
    const heard = useHeardInOption("description", named === "description", rest.id);
    return (
      <span
        {...rest}
        {...keptInOption(option && named !== "label")}
        id={heard ?? rest.id}
        data-scope={scope}
        data-part={part}
      >
        {children}
      </span>
    );
  }
  RowSlot.displayName = `Row.${part}`;
  return RowSlot;
}

const RowLeadDrawn = slot("lead");

/**
 * **Its marks**, before its words: a state, an icon, a persona's tile. Inside an option a mark
 * is kept from a reader; one that says something its words do not (a hit's line number) is
 * given that in words (`said`), which the option hears after the rest of its description
 * (#6075).
 */
export function RowLead({ said, children, ...rest }: Slot & { said?: string }) {
  const heard = useHeardInOption("lead", said !== undefined);
  return (
    <RowLeadDrawn {...rest}>
      {children}
      {heard !== undefined && (
        // Drawn nowhere: read only where the option's description points at it.
        <span id={heard} hidden>
          {said}
        </span>
      )}
    </RowLeadDrawn>
  );
}
RowLead.displayName = "Row.lead";

/** **Its words**: one line that takes the room and gives way to an ellipsis, never wraps. */
export const RowLabel = slot("label", "label");

/**
 * **The one line about it**, beside its words or under them, in the quiet tier: one to a row.
 * In an option it is the option's description, which a reader hears after its name.
 */
export const RowDescription = slot("description", "description");

/** **Its end**: a count, a state, the row's own actions. */
export const RowTrail = slot("trail");

/**
 * **Where a row asks a line under it for its whole words while its clamp cuts them** (#4711):
 * what `ListRow` and `ListOption` hand their `RowBelow`, which hands back its line's element, or
 * `null` as it goes. `null` in any other row, where `tipWhenCut` does nothing.
 *
 * **Asked only when the row could say them** (#4351): as the pointer comes to the row and as the
 * keyboard does, so a list's rows measure no line as they are drawn. Two hundred lines measured
 * as a list was drawn forced its layout before the frame could, and watched each line's size.
 */
export const RowCutWords = createContext<((line: HTMLElement | null) => void) | null>(null);

/**
 * **A line under the row's words, inside the row**: what a search matched in its body, why it
 * was refused, a skill's one-line description. The row grows to fit it and is still one row to
 * the keyboard (#3751). `clamp` cuts it at one line (an ellipsis at its end) or two; without
 * one it wraps.
 *
 * **`tipWhenCut`** (#4711): while its clamp cuts it, its whole words are its row's tooltip
 * (`RowTip.tsx`) and its description, under the pointer and while the keyboard is on the row;
 * while they fit, there is none. The row stays as it is drawn. One to a row, and a list's row's
 * alone (`ListRow`, `ListOption`).
 *
 * **In an option it is heard after the option's one line** (#6075): kept from a reader as
 * content, and read whole (its clamp is only how it is drawn) as part of the option's
 * description.
 */
export function RowBelow({
  clamp,
  tipWhenCut = false,
  children,
  ...rest
}: Slot & { clamp?: 1 | 2; tipWhenCut?: boolean }) {
  const scope = useContext(RowScope);
  const option = useContext(InOption) !== null;
  const heard = useHeardInOption("below", true, rest.id);
  const say = useContext(RowCutWords);
  const line = useRef<HTMLSpanElement>(null);
  const asks = tipWhenCut && clamp !== undefined ? say : null;
  useLayoutEffect(() => {
    const element = line.current;
    if (asks === null || element === null) return;
    asks(element);
    return () => asks(null);
  }, [asks]);
  // Handed again when its words change, as drawn (words drawn as marks are new on every draw):
  // a row measuring it now measures them.
  const wordsHanded = useRef<string | null>(null);
  useLayoutEffect(() => {
    const element = line.current;
    if (asks === null || element === null) return;
    const words = element.textContent;
    if (wordsHanded.current === null) wordsHanded.current = words;
    else if (words !== wordsHanded.current) {
      wordsHanded.current = words;
      asks(element);
    }
  });
  return (
    <span
      {...rest}
      {...keptInOption(option)}
      id={heard ?? rest.id}
      ref={line}
      data-scope={scope}
      data-part="below"
      data-clamp={clamp}
    >
      {children}
    </span>
  );
}

/** **What a card row's summary says it in** (#3669): its description in the muted tier, or a
 *  reason in its place, in the waiting colour (it needs the person) or the quiet tier (it is
 *  left out). */
export type RowSummaryTone = "description" | "warning" | "quiet";

/**
 * **A card row's two lines about it** (#3669), under its first line: its description, or a
 * reason in its place, cut cleanly after the second line. The card row says the whole of it in
 * its tooltip (`ListCardRow`).
 */
export function RowSummary({
  tone = "description",
  ...rest
}: Slot & {
  tone?: RowSummaryTone;
}) {
  const scope = useContext(RowScope);
  return (
    <span
      {...rest}
      data-scope={scope}
      data-part="summary"
      data-tone={tone === "description" ? undefined : tone}
    />
  );
}

/** **A card row's action line** (#3669), under its summary: its one fix. */
export const RowActions = slot("actions");

/**
 * **The key that does the same**, at the row's end and read last (#2921, N25): a palette row's,
 * from the key registry (`keys/registry.ts`). Drawn, and hidden from a reader, who hears the key
 * by its name from the row's `aria-keyshortcuts` instead of as symbols run onto its words. Where
 * the key is what the row is about (the cheat sheet's), it is `heard`: its symbols are drawn and
 * its `spoken` words ("Command Shift Return") are what a reader hears.
 */
export function RowShortcut({
  keys,
  heard = false,
  spoken,
  className,
}: {
  keys: string;
  heard?: boolean;
  /** The key in words, heard in place of the symbols drawn. */
  spoken?: string;
  className?: string;
}) {
  const scope = useContext(RowScope);
  return (
    <kbd
      className={className}
      data-scope={scope}
      data-part="shortcut"
      aria-hidden={heard ? undefined : "true"}
    >
      {heard && spoken !== undefined ? (
        <>
          <span aria-hidden="true">{keys}</span>
          <span data-heard-as="">{spoken}</span>
        </>
      ) : (
        keys
      )}
    </kbd>
  );
}
