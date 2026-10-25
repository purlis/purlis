// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tooltip family** (#2908, spec #2897, ADR 0104): every hint in the window, on React Aria's
 * `TooltipTrigger` and `Tooltip`, wrapped here once. The native `title` tooltip is refused by
 * lint (`purlis/tooltip-family`); a screen draws a hint with these and never imports the
 * primitives library.
 *
 * - **Drawn by its parts.** The surface carries `data-scope="tooltip"`, `data-part="root"` and
 *   `tooltip.css` draws the family default from them and a theme's parts reach the same
 *   attributes (ADR 0103).
 * - **One placement and one delay** ({@link TOOLTIP_PLACEMENT}, {@link TOOLTIP_OFFSET},
 *   {@link TOOLTIP_DELAY}, {@link TOOLTIP_CLOSE_DELAY}), which no call site can change: beside
 *   its target at the target's end, 10 px from it, flipped to its start where the window has no
 *   room (R166).
 * - **WCAG 1.4.13** (content on hover or focus): it opens under the pointer after the delay and
 *   at once on the keyboard's focus; **Esc dismisses it** without moving the focus, and is the
 *   tooltip's alone: a dialog under it, or a terminal with the focus, hears nothing of that Esc,
 *   and the next one is theirs; the pointer can move onto a tooltip it opened without it closing
 *   (**hoverable**; one over the title bar moves the window when pressed, as the bar does), while
 *   one the keyboard opened takes no pointer, so it never stands in the way of what is under it
 *   (the title bar's drag gap); and it stays until the pointer or the
 *   focus leaves, or Esc (**persistent**).
 * - **Reachable by the keyboard.** A hint's target takes the keyboard: a control already does,
 *   and words with a hint become a Tab stop, as a settings row's badge always was (DS-8), so a
 *   hint is never only a hover's. A hint is the target's description while it is shown.
 * - **Only while its project is in front** (`projectShown.tsx`): a project kept behind another
 *   opens no tooltip over the window.
 *
 * The button family takes its hint as a prop (`tooltip` on `Button` and `ToggleButton`; an
 * `IconButton` always shows its name), so `Tooltip` here is for every other target.
 */
import {
  Children,
  cloneElement,
  isValidElement,
  type ReactElement,
  type ReactNode,
  type Ref,
  type RefObject,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import {
  Tooltip as AriaTooltip,
  TooltipContext,
  TooltipTrigger,
  TooltipTriggerStateContext,
  type TooltipProps as AriaTooltipProps,
} from "react-aria-components";
import {
  mergeProps,
  mergeRefs,
  useFocusable,
  useFocusVisible,
  useId,
  useObjectRef,
} from "react-aria";
import { unavailable } from "../unavailable";
import { useProjectShown } from "../../projectShown";

/** **Where a tooltip stands**: beside its target, at the target's end (R166). */
export const TOOLTIP_PLACEMENT = "end" satisfies AriaTooltipProps["placement"];

/** How far from its target a tooltip stands, in px (R166). */
export const TOOLTIP_OFFSET = 10;

/**
 * **How long the pointer rests on a target before its tooltip opens**, in ms. Once one tooltip
 * has opened, the next opens at once until none has been open for a moment (React Aria's
 * warm-up), so a pointer that moves along a strip reads each hint without waiting again. The
 * keyboard's focus opens a tooltip at once.
 */
export const TOOLTIP_DELAY = 500;

/**
 * **How long a tooltip stays after the pointer leaves its target**, in ms: time for the pointer
 * to cross the gap onto the tooltip, which keeps it open (WCAG 1.4.13, hoverable).
 */
export const TOOLTIP_CLOSE_DELAY = 300;

/** The controls a hint's element may be inside, which take the keyboard for it. */
const CONTROLS = [
  "button",
  "a[href]",
  "summary",
  "label",
  ...["button", "link", "tab", "treeitem", "option", "menuitem", "row", "gridcell", "switch"].map(
    (role) => `[role="${role}"]`,
  ),
  // A list's rows that take the keyboard, one stop for the list (the Inbox's asks).
  '[role="listitem"][tabindex]',
  "li[tabindex]",
].join(", ");

/** What each tooltip that is up does on Esc: takes itself down. */
const dismissers = new Set<() => void>();

// **The first Esc is the tooltips' alone** (WCAG 1.4.13, dismissible without moving the focus):
// heard on the window's capture phase by a listener added as this module loads, so before any a
// dialog, a menu, the palette or a terminal adds, and stopped there, the window's later
// listeners included. The next Esc is theirs.
if (typeof window !== "undefined")
  window.addEventListener(
    "keydown",
    (event) => {
      if (event.key !== "Escape" || dismissers.size === 0) return;
      event.stopImmediatePropagation();
      for (const dismiss of [...dismissers]) dismiss();
    },
    true,
  );

/** Takes `dismiss` the first Esc while it is held; hands back what lets it go. */
export function onTooltipEscape(dismiss: () => void): () => void {
  dismissers.add(dismiss);
  return () => {
    dismissers.delete(dismiss);
  };
}

/** The elements that take the keyboard of their own. */
const NATIVELY_FOCUSED = new Set(["button", "a", "input", "select", "textarea", "summary"]);

/** A hint: words, or what a hint must show beyond words (a path set as code). */
export type Tip = ReactNode;

/** Whether `tip` says anything: an empty or absent hint draws no tooltip. */
export const says = (tip: Tip): boolean =>
  tip !== undefined && tip !== null && tip !== false && tip !== "";

/**
 * **The surface**: what a tooltip shows, in the family's parts. It is always named by its words:
 * a tooltip that only says its target's name (an icon button's) describes nothing instead, so it
 * is not heard twice.
 *
 * **It takes the pointer only when the pointer opened it** (`data-pointer`): one the keyboard
 * opened lets the pointer through to what is under it.
 */
export function TooltipSurface({ children }: { children: Tip }) {
  // A tooltip open as its project leaves the front is taken down: it is drawn over the window,
  // and the project is not. Read here, and the surface stays mounted, so a switch between
  // projects mounts nothing and draws none of the targets again.
  const shown = useProjectShown();
  const state = useContext(TooltipTriggerStateContext);
  useEffect(() => {
    if (!shown && state?.isOpen) state.close(true);
  }, [shown, state]);
  // The first Esc while it is up is its own (`onTooltipEscape`).
  const open = state?.isOpen === true;
  useEffect(() => {
    if (!open || state === null) return;
    return onTooltipEscape(() => state.close(true));
  }, [open, state]);
  const { isFocusVisible } = useFocusVisible();
  // A tooltip of the title bar is part of the bar: it takes the pointer like any other (hoverable),
  // and a press on it moves the window as a press on the bar's drag gap under it would.
  const context = useContext(TooltipContext) as { triggerRef?: RefObject<Element | null> } | null;
  const trigger = context?.triggerRef;
  const inTheBar = useCallback(
    () => trigger?.current?.closest("[data-tauri-drag-region]") != null,
    [trigger],
  );
  return (
    <AriaTooltip
      placement={TOOLTIP_PLACEMENT}
      offset={TOOLTIP_OFFSET}
      data-scope="tooltip"
      data-part="root"
      data-pointer={isFocusVisible ? undefined : ""}
      data-tauri-drag-region={open && inTheBar() ? "deep" : undefined}
    >
      {children}
    </AriaTooltip>
  );
}

/** A second line under a tooltip's first: the `detail` part. */
export function TooltipDetail({ id, children }: { id?: string; children: Tip }) {
  return (
    <span data-scope="tooltip" data-part="detail" id={id}>
      {children}
    </span>
  );
}

/**
 * **A target and its tooltip**, for a target that takes React Aria's focusable context itself
 * (the button family). The trigger is always drawn, with or without a hint, so a hint that comes
 * and goes never draws the target afresh and never takes its focus away.
 */
export function Tipped({ surface, children }: { surface: ReactNode; children: ReactNode }) {
  const has = surface !== null && surface !== undefined;
  return (
    <TooltipTrigger delay={TOOLTIP_DELAY} closeDelay={TOOLTIP_CLOSE_DELAY} isDisabled={!has}>
      {children}
      {has ? surface : null}
    </TooltipTrigger>
  );
}

/** How a target is described while its tooltip says what it says. */
type Heard =
  /** By the tooltip, while it is shown: React Aria's own. */
  | { kind: "tooltip" }
  /** By nothing more: the tooltip says the target's own name or description. */
  | { kind: "named" }
  /** By its second line, while it is shown: an icon-only button's hint under its name. */
  | { kind: "detail"; id: string }
  /** By its reason, always: an unavailable button's, which a screen reader hears at once. */
  | { kind: "reason"; id: string };

/**
 * The element a hint is on, handed the trigger's focus, hover and description. Words that took
 * no keyboard become a Tab stop (`tabIndex` 0) while they have a hint; an element that says its
 * own place in the Tab order (a roving tab's -1) keeps it. A mark hidden from a screen reader
 * (`aria-hidden`), one inside a control (a row's mark, a tab's), a label and a mark whose hint
 * is its own name take none: the control takes the keyboard, and what the mark means is said.
 *
 * **What a parent hands the tooltip reaches the element** (`handed`, `ref`): a Radix trigger
 * drawn `asChild` around a tooltip (a context menu's, a popover's) hands its handlers and its
 * ref to its child, which is this, and they are the element's, as they were before the hint was
 * a tooltip.
 *
 * **A disabled `<button>` with a hint keeps the focus** and does nothing (`ui/unavailable.ts`),
 * what its parents handed it included; its reason is its description for good.
 */
function Target({
  has,
  heard,
  pointerOnly: theirs = false,
  unavailableNow,
  handed,
  handedRef,
  children,
}: {
  has: boolean;
  heard: Heard;
  /** Its hint only repeats its words: the keyboard's focus never opens it. */
  pointerOnly?: boolean;
  unavailableNow: boolean;
  handed: Record<string, unknown>;
  handedRef: Ref<HTMLElement> | undefined;
  children: ReactElement;
}) {
  const props = children.props as {
    ref?: Ref<HTMLElement>;
    "aria-hidden"?: unknown;
    "aria-describedby"?: string;
  };
  // Whether it is drawn inside a control (a row, a tab), read once it is in the document.
  const [inside, setInside] = useState(false);
  const placed = useCallback((element: HTMLElement | null) => {
    if (element !== null) setInside(element.parentElement?.closest(CONTROLS) != null);
  }, []);
  const own = props.ref;
  const merged = useMemo(
    () => mergeRefs(own ?? null, handedRef ?? null, placed),
    [own, handedRef, placed],
  );
  const ref = useObjectRef<HTMLElement>(merged);
  const { focusableProps } = useFocusable({ isDisabled: !has }, ref);
  const state = useContext(TooltipTriggerStateContext);
  // A mark a screen reader does not hear, one inside a control, a label (its field takes the
  // keyboard) and a mark whose hint is its own name or description take no keyboard: what each
  // means is said by what holds it or by itself, and its hint is the pointer's.
  const pointerOnly =
    inside ||
    heard.kind === "named" ||
    children.type === "label" ||
    String(props["aria-hidden"]) === "true";
  // Only words are made a Tab stop: a control takes the keyboard already, and a component (a
  // Radix radio, a menu's row) keeps the place in the Tab order its own focus gives it.
  const words = typeof children.type === "string" && !NATIVELY_FOCUSED.has(children.type);
  const { tabIndex, onFocus, onBlur, ...rest } = focusableProps;
  // The keyboard's focus opens no tooltip that only repeats what the element says.
  const focusable = theirs ? rest : { ...rest, onFocus, onBlur };
  const slotted = asSlot(handed, props as Record<string, unknown>);
  const drawn = mergeProps(
    focusable,
    words && !pointerOnly ? { tabIndex } : {},
    unavailableNow ? unavailable(slotted) : slotted,
  );
  const describedBy =
    heard.kind === "reason"
      ? [props["aria-describedby"], heard.id].filter(Boolean).join(" ")
      : heard.kind === "named"
        ? props["aria-describedby"]
        : heard.kind === "detail"
          ? [props["aria-describedby"], state?.isOpen ? heard.id : null]
              .filter(Boolean)
              .join(" ") || undefined
          : (drawn as { "aria-describedby"?: string })["aria-describedby"];
  return cloneElement(children, {
    ...drawn,
    "aria-describedby": describedBy,
    "data-tooltip-target": has ? "" : undefined,
    ref,
  } as object);
}

/**
 * What a parent hands the element, merged as a Radix `Slot` merges it, which is how the element
 * was handed it before it had a tooltip: its own handler for an event runs first and the
 * parent's after it (a strip's tab keeps the focus it is dragged with before the roving focus
 * moves it), and its own value of anything else wins; class names and styles join.
 */
function asSlot(handed: Record<string, unknown>, own: Record<string, unknown>) {
  const merged: Record<string, unknown> = { ...handed, ...own };
  for (const [key, given] of Object.entries(handed)) {
    const mine = own[key];
    if (/^on[A-Z]/.test(key) && typeof given === "function" && typeof mine === "function")
      merged[key] = (...args: unknown[]) => {
        const result = (mine as (...a: unknown[]) => unknown)(...args);
        (given as (...a: unknown[]) => unknown)(...args);
        return result;
      };
    else if (key === "className" && typeof given === "string" && typeof mine === "string")
      merged[key] = `${given} ${mine}`;
    else if (key === "style" && typeof given === "object" && typeof mine === "object")
      merged[key] = { ...given, ...mine };
    else if (mine === undefined) merged[key] = given;
  }
  return merged;
}

/** The words an element shows, all of them, as one string. */
const textOf = (children: unknown): string =>
  Children.toArray(children as ReactNode)
    .map((one) =>
      typeof one === "string" || typeof one === "number"
        ? String(one)
        : isValidElement(one)
          ? textOf((one.props as { children?: unknown }).children)
          : "",
    )
    .join("");

/** The words an element shows, shallowly: an icon-only button shows none. A part that says it
 *  shows words (`data-shows-words`: a select's value, drawn by a function of what it holds,
 *  `ui/select/Select.tsx`) shows them. */
const showsWords = (children: unknown): boolean =>
  Children.toArray(children as ReactNode).some(
    (one) =>
      typeof one === "string" ||
      typeof one === "number" ||
      (isValidElement(one) &&
        String((one.props as { "aria-hidden"?: unknown })["aria-hidden"]) !== "true" &&
        ((one.props as Record<string, unknown>)["data-shows-words"] !== undefined ||
          showsWords((one.props as { children?: unknown }).children))),
  );

/**
 * **A hint on any element but a family button** (which takes `tooltip`): `tip` shown beside
 * `children`, which must be one element that hands its props and its ref to the DOM. Any other
 * prop, and a ref, go on to that element, so a tooltip can stand where its element stood as a
 * Radix trigger's child.
 *
 * **An icon-only `<button>` is its name first** (its `aria-label`), as an `IconButton` is, with
 * the hint as a second line under it, so what the tooltip shows and what a screen reader hears
 * agree.
 */
export function Tooltip({
  tip,
  children,
  ref,
  ...handed
}: {
  tip: Tip;
  children: ReactElement;
  ref?: Ref<HTMLElement>;
  [prop: string]: unknown;
}) {
  const detailId = useId();
  const reasonId = useId();
  const has = says(tip);
  const only = Children.only(children);
  if (!isValidElement(only)) return only;
  const own = only.props as {
    "aria-label"?: unknown;
    "aria-description"?: unknown;
    disabled?: unknown;
    children?: unknown;
  };
  // A hint that only repeats the element's own words (a name cut short where it is narrow) is
  // the pointer's: the keyboard and a screen reader have those words already.
  const repeats = typeof tip === "string" && tip !== "" && textOf(own.children).includes(tip);
  const named = own["aria-label"] === tip || own["aria-description"] === tip || repeats;
  const button = only.type === "button";
  const label = typeof own["aria-label"] === "string" ? own["aria-label"] : "";
  const iconOnly = button && label !== "" && !showsWords(own.children);
  const unavailableNow = has && button && own.disabled === true;
  const line = iconOnly ? (tip === label ? undefined : tip) : named ? undefined : tip;
  const heard: Heard =
    unavailableNow && says(line)
      ? { kind: "reason", id: reasonId }
      : named || (iconOnly && line === undefined)
        ? { kind: "named" }
        : iconOnly
          ? { kind: "detail", id: detailId }
          : { kind: "tooltip" };
  const surface = iconOnly ? (
    <TooltipSurface>
      {label}
      {line !== undefined ? <TooltipDetail id={detailId}>{line}</TooltipDetail> : null}
    </TooltipSurface>
  ) : (
    <TooltipSurface>{tip}</TooltipSurface>
  );
  return (
    <TooltipTrigger delay={TOOLTIP_DELAY} closeDelay={TOOLTIP_CLOSE_DELAY} isDisabled={!has}>
      <Target
        has={has}
        heard={heard}
        pointerOnly={repeats}
        unavailableNow={unavailableNow}
        handed={handed}
        handedRef={ref}
      >
        {only}
      </Target>
      {has ? surface : null}
      {heard.kind === "reason" ? (
        <span id={reasonId} hidden>
          {line}
        </span>
      ) : null}
    </TooltipTrigger>
  );
}

/** The state of a tooltip whose drawer opens and closes it. */
const ALWAYS_OPEN = { isOpen: true, shouldSkipAnimation: false, open: () => {}, close: () => {} };

/**
 * **A tooltip opened by what it stands by, not by a pointer on it**: a menu row's, which shows
 * while the keyboard or the pointer has rested on its row (`ui/menu/MenuTip.tsx`). It stands
 * beside `anchor` as every tooltip does, at the end, or at the start where the end is taken (a
 * submenu opens there). Whoever opens it takes it down, Esc among the ways.
 */
export function AnchoredTooltip({
  anchor,
  side = "end",
  onPointerEnter,
  onPointerLeave,
  tooltipRef,
  children,
}: {
  anchor: RefObject<Element | null>;
  /** Its element, which what it describes names (a row's, `ui/list/RowTip.tsx`). */
  tooltipRef?: Ref<HTMLDivElement>;
  side?: "end" | "start";
  /** The pointer is on it, or has left it (WCAG 1.4.13, hoverable): its drawer keeps it then. */
  onPointerEnter?: () => void;
  onPointerLeave?: () => void;
  children: Tip;
}) {
  const shown = useProjectShown();
  if (!shown) return null;
  return (
    // Open for as long as it is drawn: whoever draws it takes it down.
    <TooltipTriggerStateContext.Provider value={ALWAYS_OPEN}>
      <AriaTooltip
        ref={tooltipRef}
        triggerRef={anchor}
        isOpen
        placement={side === "end" ? TOOLTIP_PLACEMENT : "start"}
        offset={TOOLTIP_OFFSET}
        data-scope="tooltip"
        data-part="root"
        data-pointer=""
        onPointerEnter={onPointerEnter}
        onPointerLeave={onPointerLeave}
      >
        {children}
      </AriaTooltip>
    </TooltipTriggerStateContext.Provider>
  );
}
