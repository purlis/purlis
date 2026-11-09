// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The message family** (#5649, spec #5638, ADR 0119 decision 6): one message of the Inbox's
 * views, an ask, a line of Mine or Project, a Notice, drawn as a row of its view rather than a
 * boxed card. The look is the variants prototype's variant A
 * (`app/src/inbox-variants.prototype.html`, which lives on the branch `prototype/inbox` and not in
 * this tree): borderless rows on the view's own ground, a hairline
 * between two of them, the row's own fill only under the pointer or the keyboard.
 *
 * A row is, left to right:
 *
 * - **its mark** (`mark`): a round icon tinted in its state's colour, its variant: `waiting` (it
 *   waits on the person), `done` (it finished), `failed` (it failed, or needs a fix), `report`
 *   (something came back) or `plain`. Decoration: the row's meta says its state in words, so the
 *   mark is hidden from a reader and nothing is told by its colour alone (WCAG 1.4.1).
 * - **its main column** (`main`), in the order a screen gives it:
 *   {@link MessageTop} (its title, the chat or the thing, and its time at the right),
 *   {@link MessageMeta} (what it is, and where, led by {@link MessageState}, what it waits on),
 *   {@link MessageBody} (what it says),
 *   {@link MessageLinks} (its ways out, as text links: Go to chat, a Notice's fix, Show all) and
 *   {@link MessageForm} (what it is answered with, in the row: a reply box, an ask's answers).
 * - **its ×** (`dismiss`), where it has one: at the top right, over its time, shown while the row
 *   is under the pointer or holds the keyboard, and the time hidden then (`data-dismiss`).
 *
 * A screen gives every word; the family gives the frame. No state changes a size (#2800): the
 * pointer and the keyboard change a fill and the time's visibility, never a line.
 */
import {
  createElement,
  useCallback,
  useMemo,
  useState,
  type ComponentPropsWithRef,
  type ElementType,
  type ReactNode,
  type Ref,
} from "react";
import { mergeProps, mergeRefs, useFocusWithin, useHover } from "react-aria";
import { ButtonRow } from "../button/Button";
import { ListRowDrawn } from "../list/List";
import { parted } from "../partClasses";

/** The family's variants, in the order the manifest names them: a row's state. */
export const MESSAGE_TONES = ["waiting", "done", "failed", "report", "plain"] as const;
export type MessageTone = (typeof MESSAGE_TONES)[number];

/** The elements a row is drawn as: an item of a list, or a box a list moves itself; or `row`,
 *  a row of the list family's list of rows that act (`List`), which it is the look of (#3123). */
type MessageElement = "li" | "div" | "article" | "row";

/**
 * **Whether the keyboard is on a row or inside it**, for a row the list family draws: React Aria
 * hands a row's own focus handlers to nothing a screen gives it, so it is heard where it lands.
 */
function useFocusInside(given: Ref<HTMLElement> | undefined) {
  const [inside, setInside] = useState(false);
  // A ref's cleanup takes the listeners off again (React 19), so a ref handed anew never adds a
  // second pair.
  const heard = useCallback((element: HTMLElement | null) => {
    if (element === null) return;
    const came = () => setInside(true);
    const went = (event: FocusEvent) => {
      const to = event.relatedTarget;
      setInside(to instanceof Node && element.contains(to));
    };
    element.addEventListener("focusin", came);
    element.addEventListener("focusout", went);
    return () => {
      element.removeEventListener("focusin", came);
      element.removeEventListener("focusout", went);
    };
  }, []);
  const attach = useMemo(() => mergeRefs<HTMLElement>(heard, given ?? null), [heard, given]);
  return { inside, attach };
}

/**
 * **One message, as a row of its view.** `icon` is its mark's glyph (a `lucide-react` icon);
 * `dismiss` its ×, drawn at its top right (an `IconButton`, which a roving list may wrap in its
 * stop). `read`: a line the person has read, whose title steps back. `className` places it, and
 * never draws it.
 */
export function MessageRow({
  as = "li",
  tone,
  icon,
  dismiss,
  read = false,
  consent = false,
  children,
  textValue,
  onDrawn,
  ...rest
}: Omit<ComponentPropsWithRef<"li">, "style" | "ref"> & {
  ref?: Ref<HTMLElement>;
  as?: MessageElement;
  /** As a list's row (`as="row"`): what it is called where nothing names it. */
  textValue?: string;
  /** As a list's row: heard after each drawing of it, once it is in the document
   *  (`ListRowDrawn`). */
  onDrawn?: () => void;
  tone: MessageTone;
  icon: ReactNode;
  dismiss?: ReactNode;
  read?: boolean;
  /** **It waits on the person's consent** (ADR 0097 §6, the ruling on #5222): among rows that do
   *  not, it keeps a waiting edge at its start, which no theme takes away. */
  consent?: boolean;
  children: ReactNode;
}) {
  // Under the pointer and holding the keyboard, as React Aria marks them: what a theme's
  // `message.root.hover` and `message.root.focused` are keyed on.
  const { hoverProps, isHovered } = useHover({});
  const [focusedWithin, setFocusedWithin] = useState(false);
  const { focusWithinProps } = useFocusWithin({ onFocusWithinChange: setFocusedWithin });
  const row = as === "row";
  const inside = useFocusInside(rest.ref);
  // `createElement` passes JSX's runtime by, so the part's classes are drawn here (#4351). As a
  // list's row, React Aria marks it under the pointer itself, and the keyboard is heard on it.
  const element: ElementType = row ? ListRowDrawn : as;
  return createElement(
    element,
    parted({
      ...(row
        ? { ...rest, textValue, onDrawn, ref: inside.attach }
        : mergeProps(rest, hoverProps, focusWithinProps)),
      "data-hovered": isHovered ? "" : undefined,
      "data-focus-within": (row ? inside.inside : focusedWithin) ? "" : undefined,
      "data-scope": "message",
      "data-part": "root",
      "data-variant": tone,
      "data-read": read ? "" : undefined,
      "data-consent": consent ? "" : undefined,
      "data-dismiss": dismiss === undefined || dismiss === null ? undefined : "",
    }),
    <span data-scope="message" data-part="mark" aria-hidden="true">
      {icon}
    </span>,
    <div data-scope="message" data-part="main">
      {children}
    </div>,
    dismiss === undefined || dismiss === null ? null : (
      <span data-scope="message" data-part="dismiss">
        {dismiss}
      </span>
    ),
  );
}

/** **A row's first line**: its title, cut short with an ellipsis, and its time at the right. */
export function MessageTop({
  title,
  time,
  titleId,
}: {
  /** The chat or the thing it is about. */
  title: ReactNode;
  /** When it happened, or how long it has waited: a `<time>`, as short as "5m". */
  time?: ReactNode;
  /** The title's id, for a row's `aria-labelledby`. */
  titleId?: string;
}) {
  return (
    <div data-scope="message" data-part="top">
      <span id={titleId} data-scope="message" data-part="title">
        {title}
      </span>
      {time !== undefined && time !== null && (
        <span data-scope="message" data-part="time">
          {time}
        </span>
      )}
    </div>
  );
}

/** **What it is, and where**, quiet under its title: "Waiting on your reply · smart-ide". */
export function MessageMeta({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <div id={id} data-scope="message" data-part="meta">
      {children}
    </div>
  );
}

/** **What it waits on, in words** (`state`), at the head of its meta: in the waiting colour on a
 *  row that waits, so its state is said by more than its mark. */
export function MessageState({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <span id={id} data-scope="message" data-part="state">
      {children}
    </span>
  );
}

/** **What it says**, at the row's size. A screen's own words, a chat's markdown, or a list. */
export function MessageBody({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <div id={id} data-scope="message" data-part="body">
      {children}
    </div>
  );
}

/** **Its ways out, as text links on one line that wraps**: Answer, Go to chat, a fix, Show all. */
export function MessageLinks({ children }: { children: ReactNode }) {
  // The shared small row (`ButtonRow`), so a long label wraps rather than widening a narrow side.
  return (
    <div data-scope="message" data-part="links">
      <ButtonRow size="sm">{children}</ButtonRow>
    </div>
  );
}

/**
 * **What it is answered with, in the row** (#5651): a reply box, an ask's answers, what a way out
 * opened, drawn while its Answer link is open. Under its links, at the main column's width; `id`
 * is what that link names as what it opens.
 */
export function MessageForm({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <div id={id} data-scope="message" data-part="form">
      {children}
    </div>
  );
}
