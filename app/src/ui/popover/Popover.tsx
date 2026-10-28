// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The popover family** (#2909, spec #2897, ADR 0104): everything the window draws beside the
 * thing that opened it, over the rest of the window, on React Aria's `Popover` and `Dialog`,
 * wrapped here once. A menu is drawn on this surface too (`ui/menu/Menu.tsx`), so a card, a
 * refusal, a list and a menu share one ground, one edge, one corner and one lift (#1845).
 *
 * - **Drawn by its parts** (ADR 0103). The surface is `data-scope="popover" data-part="root"`;
 *   inside it the anatomy of R165: the `filter` row with its `rule` under it, the scrolling
 *   `list`, and the `footer` that says the keys, each `key` in a mono cap. A group's `header` and
 *   the `rule` between groups are parts of the surface too, since they stand in a menu's list.
 *   `popover.css` draws the family default from those and a theme's parts reach the same
 *   attributes. A screen gives a popover a place (`className` for width), never a look.
 * - **Placed under its trigger, aligned to it, and never nearer the window's edge than 8 px**
 *   (`EDGE`): React Aria flips it to the other side where there is no room.
 * - **Closed by a click outside and by Escape**, and it hands the keyboard back to where it was,
 *   the moment it goes (`HandsBack`, #3603), ahead of React Aria's `FocusScope`. A popover that
 *   only says something (a card under the pointer, a refusal beside a field) is `isNonModal`: it
 *   takes no keyboard and closes when its owner says so.
 * - **Drawn only while its project is in front** (`projectShown.tsx`): a project kept behind
 *   another never draws over the window.
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentPropsWithRef,
  type CSSProperties,
  type ReactNode,
  type RefObject,
} from "react";
import {
  Dialog as AriaDialog,
  DialogTrigger as AriaDialogTrigger,
  OverlayArrow,
  Popover as AriaPopover,
  type DialogTriggerProps,
  type PopoverProps as AriaPopoverProps,
} from "react-aria-components";
// React Aria's own careful focus, which its hand-back uses: it is not in the package's index.
import { focusSafely } from "react-aria/private/interactions/focusSafely";
import { useProjectShown } from "../../projectShown";
import { pressableTrigger } from "../trigger";

/** How near the window's edge a popover or a menu may come, in px (R59). */
export const EDGE = 8;

/** How far a popover stands off what opened it, in px. */
export const OFFSET = 4;

/** Where a popover stands beside what opened it: under it, and which of its edges it lines up. */
export type Align = "start" | "end";

export type Side = "bottom" | "top" | "left" | "right";

/** React Aria's placement for a side and an alignment. */
export function placementOf(side: Side, align: Align): AriaPopoverProps["placement"] {
  return `${side} ${align}` as AriaPopoverProps["placement"];
}

/** What a member of the family that draws the surface takes. */
export interface SurfaceProps {
  /** The side it opens on: `bottom`, under its trigger, by default. */
  side?: Side;
  /** Which edge of its trigger it lines up with: the start (the default) or the end. */
  align?: Align;
  /** How far it stands off its trigger, in px. */
  offset?: number;
  /** A popover that only says something: it takes no keyboard and no click outside closes it. */
  isNonModal?: boolean;
  /** Its place in its view: a width. Never a look. */
  className?: string;
  style?: CSSProperties;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  /** The pointer coming onto the surface and leaving it. */
  onPointerEnter?: () => void;
  onPointerLeave?: () => void;
  /** `data-*` attributes reach the surface as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
}

/**
 * **What opened a surface**, which a press outside stands back for: React Aria's trigger, whose
 * press while it is open closes it; an owner's element, whose own press says what a press on it
 * does; or nothing to stand back for (a context menu at the pointer).
 */
export type Opener = "trigger" | "owner" | "none";

/**
 * **A press outside closes a surface that is not modal**, as on every platform, and still
 * reaches what it was on: React Aria closes one only when the keyboard leaves it for something
 * else. A press on React Aria's own trigger while it is open closes it and opens nothing; an
 * owner's element is left to its own press. With `escape`, Escape closes it too, from wherever
 * the keyboard is, and goes no further: for a surface that took no keyboard.
 */
export function useClosedFromOutside({
  isOpen,
  close,
  triggerRef,
  opener,
  escape,
}: {
  isOpen: boolean;
  close: () => void;
  triggerRef: RefObject<Element | null> | undefined;
  opener: Opener;
  escape: boolean;
}): void {
  const latest = useRef(close);
  useLayoutEffect(() => {
    latest.current = close;
  });
  useEffect(() => {
    if (!isOpen) return;
    const key = (event: KeyboardEvent) => {
      if (!escape || event.key !== "Escape") return;
      event.stopPropagation();
      event.preventDefault();
      latest.current();
    };
    const press = (event: PointerEvent) => {
      const on = event.target;
      if (!(on instanceof Node)) return;
      if (on instanceof Element && on.closest('[data-scope="popover"]')) return;
      if (opener !== "none" && triggerRef?.current?.contains(on)) {
        if (opener === "owner") return;
        // React Aria's trigger opens on the press: this one closes it, and goes no further.
        event.stopPropagation();
      }
      latest.current();
    };
    window.addEventListener("keydown", key, true);
    document.addEventListener("pointerdown", press, true);
    return () => {
      window.removeEventListener("keydown", key, true);
      document.removeEventListener("pointerdown", press, true);
    };
  }, [isOpen, triggerRef, opener, escape]);
}

/**
 * **What a trigger is told once its surface has gone**: a surface stays drawn while it leaves,
 * and the keyboard is handed on only once it is gone, so React Aria's own hand-back finds it
 * already given (`onClosed`). A submenu's surface tells nobody.
 */
export const SurfaceGone = createContext<(() => void) | undefined>(undefined);

/** What React Aria asks before it hands the keyboard back to an element, so a collection that
 *  draws its rows again can send it to the row that now stands for it (`preventDefault`). */
const RESTORE_EVENT = "react-aria-focus-scope-restore";

/**
 * **The keyboard goes back the moment the surface goes** (#3603): to what had it as the surface
 * opened, when the keyboard was in the surface and nothing else has taken it, a microtask after
 * the commit that took the surface away, before anything else can run. React Aria's own
 * hand-back waits for the next frame, and a frame drawn late (a busy run) left the keyboard on
 * nothing long enough for something else to take it: React Aria's dialog puts the keyboard on
 * itself half a second after it opens when it finds it on nothing, so Esc on a menu in a dialog
 * lost the person's place. React Aria's hand-back, a frame later, finds the keyboard placed and
 * leaves it. An opener the window still holds unreachable (behind a modal surface that has not
 * let go yet) is left to React Aria's. Then says `gone`, for the trigger's `onClosed`.
 *
 * It focuses as React Aria's hand-back does (`focusSafely`): at once with a keyboard or a
 * pointer, and with a screen reader's virtual cursor only once the page's transitions are done,
 * if nothing took the keyboard meanwhile.
 *
 * - **A surface drawn inside another** (a submenu, drawn into its root menu's wrapper beside the
 *   root's surface, not in it) says so to the one around it as they go together, in one commit
 *   (#3642): it goes first, its own opener goes with them, and the keyboard goes back to the
 *   outer one's opener.
 * - **An opener that is a row of a React Aria collection** takes the hand-back itself (the
 *   collection stops the restore event) and would focus the row only on a render of its own, a
 *   task later (#3642). The row is focused here at once, while it still stands for the item it
 *   stood for as the surface opened (its `data-key`), and the collection's render finds it
 *   placed.
 */
function HandsBack({
  surface,
  gone,
  outer,
  inner,
}: {
  surface: RefObject<Element | null>;
  gone: (() => void) | undefined;
  /** Told when the keyboard was in this surface as it went: the surface around this one's. */
  outer: KeyboardWent | null;
  /** Told by a surface drawn inside this one when the keyboard was in it as it went. */
  inner: KeyboardWent;
}) {
  // What had the keyboard as the surface opened, told as it is first drawn, as React Aria's own
  // focus scope keeps it, and the item of a collection it stood for then.
  const [opener] = useState(() => {
    const had = document.activeElement;
    return had instanceof HTMLElement && had !== had.ownerDocument.body ? had : null;
  });
  const [item] = useState(() => opener?.getAttribute("data-key") ?? null);
  const latest = useRef(gone);
  useLayoutEffect(() => {
    latest.current = gone;
  });
  // The surface's element, read once it is drawn: React lets go of the ref before this goes.
  const drawn = useRef<Element | null>(null);
  useEffect(() => {
    drawn.current = surface.current;
  }, [surface]);
  useLayoutEffect(
    () => () => {
      latest.current?.();
      // Still in the document here: what is in a surface goes before the surface itself.
      const element = drawn.current;
      const doc = element?.ownerDocument;
      if (!element || !doc) return;
      // A surface inside this one goes first, and says whether the keyboard was in it.
      const had = inner.current || element.contains(doc.activeElement);
      inner.current = false;
      if (!had) return;
      if (outer !== null) {
        // Read in this same commit by the surface around this one, if it goes too.
        outer.current = true;
        queueMicrotask(() => {
          outer.current = false;
        });
      }
      if (opener === null) return;
      queueMicrotask(() => {
        const now = doc.activeElement;
        if (now !== null && now !== doc.body) return;
        if (!opener.isConnected || opener.closest("[inert]")) return;
        const restore = new CustomEvent(RESTORE_EVENT, { bubbles: true, cancelable: true });
        const asked = opener.dispatchEvent(restore);
        if (asked || opener.getAttribute("data-key") === item) focusSafely(opener);
      });
    },
    [opener, item, outer, inner],
  );
  return null;
}

/** Whether the keyboard was in a surface as it went, told to the surface around it. */
type KeyboardWent = { current: boolean };

/** The surface being drawn, for a surface drawn inside it to tell as they go (`HandsBack`). */
const SurfaceAround = createContext<KeyboardWent | null>(null);

/**
 * Calls `closed` once a surface that was open has closed and gone: where the keyboard goes
 * then, when it is not back to where it was. Answers what its trigger hands `SurfaceGone`.
 */
export function useGone(isOpen: boolean, closed: (() => void) | undefined): () => void {
  const open = useRef(isOpen);
  const latest = useRef(closed);
  useLayoutEffect(() => {
    open.current = isOpen;
    latest.current = closed;
  });
  // Gone while still open is a project leaving the front, which hides the surface and closes
  // nothing. Asked once the commit that took it away is done, when what it knows is current.
  return useCallback(() => {
    queueMicrotask(() => {
      if (!open.current) latest.current?.();
    });
  }, []);
}

/**
 * **The surface**: React Aria's `Popover`, wearing the family's root part, drawn only while its
 * project is in front. For the family's members, which hand it its trigger, its state and its
 * kind; a screen draws `PopoverTrigger` or `AnchoredPopover`.
 */
export function Surface({
  side = "bottom",
  align = "start",
  offset = OFFSET,
  isNonModal,
  className,
  style,
  kind = "popover",
  placed = true,
  children,
  ...rest
}: SurfaceProps & {
  kind?: "popover" | "menu";
  /** Whether this sets its own placement, or leaves it to the trigger (a submenu's). */
  placed?: boolean;
  triggerRef?: RefObject<Element | null>;
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  shouldFlip?: boolean;
  shouldCloseOnInteractOutside?: (element: Element) => boolean;
  children: ReactNode;
}) {
  const shown = useProjectShown();
  const gone = useContext(SurfaceGone);
  const outer = useContext(SurfaceAround);
  const inner = useRef(false);
  const surface = useRef<HTMLElement>(null);
  if (!shown) return null;
  const placement = placed ? { placement: placementOf(side, align), offset } : {};
  return (
    <AriaPopover
      {...rest}
      {...placement}
      containerPadding={EDGE}
      isNonModal={isNonModal}
      className={className}
      style={style}
      data-scope="popover"
      data-part="root"
      data-variant={kind}
      ref={surface}
    >
      <SurfaceAround.Provider value={inner}>{children}</SurfaceAround.Provider>
      <HandsBack surface={surface} gone={gone} outer={outer} inner={inner} />
    </AriaPopover>
  );
}

/**
 * **A popover opened by a press on its trigger** (React Aria's `DialogTrigger`): the trigger
 * first, then what it opens, a `PopoverDialog`. A trigger that is not the button family's (a
 * list's row) is made pressable here. `isOpen` and `onOpenChange` hold it where its owner
 * decides when it is up.
 */
export function PopoverTrigger({
  children,
  isOpen: held,
  defaultOpen = false,
  onOpenChange,
  onClosed,
}: Omit<DialogTriggerProps, "children"> & {
  children: [ReactNode, ReactNode];
  /** Called once it has closed and gone: where the keyboard goes instead of back. */
  onClosed?: () => void;
}) {
  const [own, setOwn] = useState(defaultOpen);
  const isOpen = held ?? own;
  const gone = useGone(isOpen, onClosed);
  const [trigger, popover] = children;
  return (
    <AriaDialogTrigger
      isOpen={isOpen}
      onOpenChange={(next) => {
        if (held === undefined) setOwn(next);
        onOpenChange?.(next);
      }}
    >
      {pressableTrigger(trigger)}
      <SurfaceGone.Provider value={gone}>{popover}</SurfaceGone.Provider>
    </AriaDialogTrigger>
  );
}

/**
 * **What a `PopoverTrigger` opens**: the surface, and in it a dialog named by `aria-label`,
 * which holds the keyboard while it is up and hands it back as it closes.
 */
export function PopoverDialog({
  children,
  "aria-label": label,
  "aria-labelledby": labelledBy,
  layout = "padded",
  arrow = false,
  ...surface
}: SurfaceProps & {
  /** What it holds, or a function of the way to close it (a list that closes on a pick). */
  children: ReactNode | ((dialog: { close: () => void }) => ReactNode);
  /** `list`: the anatomy of R165, whose parts pad themselves (`PopoverFilter`, `PopoverList`,
   *  `PopoverFooter`); `padded` (the default): words and controls, padded as a dialog is. */
  layout?: "padded" | "list";
  /** The point it takes from what opened it. */
  arrow?: boolean;
}) {
  return (
    <Surface {...surface} data-layout={layout}>
      {arrow && <PopoverArrow />}
      <AriaDialog
        aria-label={label}
        aria-labelledby={labelledBy}
        data-scope="popover"
        data-part="body"
      >
        {children}
      </AriaDialog>
    </Surface>
  );
}

/**
 * **A popover held up by its owner beside an element it does not open from** (a card beside a
 * row the pointer rests on, a refusal under the field it is about): its owner says when it is
 * open, and `triggerRef` is the element it stands beside. It takes no keyboard and says
 * something rather than asking: what it says carries its own role inside it.
 */
export function AnchoredPopover({
  triggerRef,
  isOpen,
  onOpenChange,
  children,
  ...surface
}: SurfaceProps & {
  triggerRef: RefObject<Element | null>;
  isOpen: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  children: ReactNode;
}) {
  // **Escape takes it down**, wherever the keyboard is, as a tooltip goes: it took no keyboard.
  useEffect(() => {
    if (!isOpen || onOpenChange === undefined) return;
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) onOpenChange(false);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [isOpen, onOpenChange]);
  return (
    <Surface
      isNonModal
      {...surface}
      triggerRef={triggerRef}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
    >
      {children}
    </Surface>
  );
}

/**
 * **A popover that asks, held open by its owner** beside an element that keeps its own presses
 * and keys (a list's row that is a roving item): its owner says when it is open, and says
 * `aria-haspopup="dialog"` and `aria-expanded` on the element. It takes the keyboard as it
 * opens, in a dialog named by `aria-label`, and hands it back as it closes. Not modal: Tab
 * leaves it, and a press outside or Escape closes it.
 */
export function HeldPopover({
  triggerRef,
  isOpen,
  onOpenChange,
  children,
  "aria-label": label,
  "aria-labelledby": labelledBy,
  layout = "padded",
  arrow = false,
  ...surface
}: SurfaceProps & {
  triggerRef: RefObject<Element | null>;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  layout?: "padded" | "list";
  arrow?: boolean;
  children: ReactNode;
}) {
  useClosedFromOutside({
    isOpen,
    close: () => onOpenChange(false),
    triggerRef,
    opener: "owner",
    escape: false,
  });
  return (
    <Surface
      isNonModal
      {...surface}
      triggerRef={triggerRef}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      shouldCloseOnInteractOutside={(element) => !(triggerRef.current?.contains(element) ?? false)}
      data-layout={layout}
    >
      {arrow && <PopoverArrow />}
      <AriaDialog
        aria-label={label}
        aria-labelledby={labelledBy}
        data-scope="popover"
        data-part="body"
      >
        {children}
      </AriaDialog>
    </Surface>
  );
}

/** The point a popover takes from what opened it, in the surface's own ground. */
function PopoverArrow() {
  return (
    <OverlayArrow data-scope="popover" data-part="arrow">
      <svg width={12} height={6} viewBox="0 0 12 6" aria-hidden="true">
        <path d="M0 0 L6 6 L12 0" />
      </svg>
    </OverlayArrow>
  );
}

// --- The anatomy (R165) ----------------------------------------------------------------------

/**
 * **The filter row**, first in a popover that lists more than a few things: the field that
 * narrows the list, with the `rule` under it. The field is the field family's, handed in.
 */
export function PopoverFilter({ children }: { children: ReactNode }) {
  return (
    <>
      <div data-scope="popover" data-part="filter">
        {children}
      </div>
      <PopoverRule />
    </>
  );
}

/** **A rule**: the line under the filter row, and between a menu's groups. */
export function PopoverRule() {
  return <div data-scope="popover" data-part="rule" role="presentation" />;
}

/**
 * **The list**, which scrolls inside the popover while the filter row and the footer stay in
 * view. What is in it is a list family's, or a menu's.
 */
export function PopoverList({
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "className" | "style">) {
  return (
    <div {...rest} data-scope="popover" data-part="list">
      <InPopoverList.Provider value>{children}</InPopoverList.Provider>
    </div>
  );
}

/** Whether what is drawn is inside a popover's list, whose rows a menu in it lays out. */
export const InPopoverList = createContext(false);

/** One key the footer names, and what it does there. */
export type KeyHint = { keys: readonly string[]; does: string };

/** The keys every list in a popover answers to (R165): move, go, close. */
export const LIST_KEYS: readonly KeyHint[] = [
  { keys: ["↑", "↓"], does: "move" },
  { keys: ["⏎"], does: "go" },
  { keys: ["esc"], does: "close" },
];

/**
 * **The footer**: the keys the list answers to, "↑↓ move · ⏎ go · esc close", each key in a
 * mono cap, in the quiet tier. Hidden from a screen reader, which hears the list's own keys.
 */
export function PopoverFooter({ hints = LIST_KEYS }: { hints?: readonly KeyHint[] }) {
  return (
    <div data-scope="popover" data-part="footer" data-of="keys" aria-hidden="true">
      {hints.map((hint, at) => (
        <span key={hint.does} data-scope="popover" data-part="hint">
          {at > 0 && (
            <span data-scope="popover" data-part="dot">
              ·
            </span>
          )}
          {hint.keys.map((key) => (
            <kbd key={key} data-scope="popover" data-part="key">
              {key}
            </kbd>
          ))}
          <span>{hint.does}</span>
        </span>
      ))}
    </div>
  );
}
