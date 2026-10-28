// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FocusEvent,
  type PointerEventHandler,
  type FocusEventHandler,
  type KeyboardEventHandler,
  type RefObject,
} from "react";

/**
 * Whether the element that took the focus took it from the keyboard. A pointer's press focuses
 * a row too, on the WebViews that focus a button on a click, and a list held for that would
 * stay held after the pointer left. Where the engine cannot say, it is taken as the keyboard's.
 */
export function byKeyboard(target: Element): boolean {
  try {
    return target.matches(":focus-visible");
  } catch {
    return true;
  }
}

/** What the element a list is held by listens to, spread on it. */
export type HeldBy<E extends HTMLElement> = {
  ref: RefObject<E | null>;
  onPointerEnter: PointerEventHandler<E>;
  onPointerMove: PointerEventHandler<E>;
  onPointerLeave: PointerEventHandler<E>;
  onFocus: FocusEventHandler<E>;
  onBlur: FocusEventHandler<E>;
  onKeyDownCapture: KeyboardEventHandler<E>;
};

/**
 * **A list held still while the pointer is over it or the keyboard is in it** (V100-47, the
 * Chats list's rule, #3566): no row moves under a click or a key. Taken as the pointer or the
 * keyboard comes in, and let go when both have left. Neither outlives the window being the one
 * in use, since a pointer parked over a list while the person works elsewhere sends no leave,
 * nor a focused row taken out of the page, which sends no blur.
 *
 * **Only the list and its rows' menus hold it** ({@link heardIn}). React hands the element
 * around the list what happens in every element drawn inside it in React's tree, a portal's
 * too, so the list's filter, drawn into the view's top, would hold the list while it is typed
 * in. The Chats list's filter stands outside its list and never holds it; this one does not
 * either.
 *
 * `holding` says whether it is held; `held` goes on the element around the list. `drawn` is what
 * the list draws its rows by (a new value each draw that may move one): after each, the focus is
 * looked for, since a menu or row that held it can go with no blur. A
 * hold that is not `on` is never taken, and listens to nothing.
 */
export function useHeldStill<E extends HTMLElement>(
  drawn: unknown,
  on = true,
): { holding: boolean; held: HeldBy<E> } {
  const ref = useRef<E>(null);
  /** The element the hold last saw the keyboard on: the one that must still have it. */
  const focused = useRef<Element | null>(null);
  const [over, setOver] = useState(false);
  const [inside, setInside] = useState(false);
  useEffect(() => {
    if (!on) return;
    const away = () => {
      setOver(false);
      setInside(false);
    };
    window.addEventListener("blur", away);
    return () => window.removeEventListener("blur", away);
  }, [on]);
  // After each draw of new rows: a row the keyboard was on, taken out of the page, took the
  // focus with it. The element the hold saw is asked, never what the list holds in the page:
  // a row's menu is drawn outside it.
  useLayoutEffect(() => {
    if (!inside) return;
    const at = focused.current;
    if (at === null || !at.isConnected || at !== document.activeElement) setInside(false);
  }, [inside, drawn]);
  const heard = (target: EventTarget | null) => heardIn(ref.current, target);
  return {
    holding: on && (over || inside),
    held: {
      ref,
      onPointerEnter: (event) => {
        if (heard(event.target)) setOver(true);
      },
      // Taken again by the pointer's next move, after the window was left and come back to; and
      // let go on a move over the filter, which React counts as inside.
      onPointerMove: (event) => {
        const here = heard(event.target);
        if (here !== over) setOver(here);
      },
      onPointerLeave: () => setOver(false),
      // A focus by the keyboard takes the hold; any focus in the list while it holds moves what
      // the hold follows (an arrow's move, a menu's item, the row a menu gave the focus back to).
      onFocus: (event) => {
        if (!heard(event.target) || !(inside || byKeyboard(event.target))) return;
        focused.current = event.target;
        setInside(true);
      },
      onBlur: (event: FocusEvent<E>) => {
        if (heard(event.relatedTarget)) return;
        setInside(false);
      },
      // A key in the list is the keyboard being here.
      onKeyDownCapture: (event) => {
        if (!heard(event.target) || !(event.target instanceof Element)) return;
        focused.current = event.target;
        if (!inside) setInside(true);
      },
    },
  };
}

/**
 * Whether `target` is in the list `list` holds, or in one of its rows' menus, which are drawn
 * in a layer of their own: what a hold is taken for. The view's top, where the list's filter
 * is drawn, is neither.
 */
export function heardIn(list: Element | null, target: EventTarget | null): boolean {
  if (!(target instanceof Element) || list === null) return false;
  return list.contains(target) || target.closest('[role="menu"]') !== null;
}

/**
 * **`value` as it was when `holding` began**, for as long as it holds, and `value` itself once
 * it lets go: what a held list draws its rows by. **Held for one `of`** (a workspace): another
 * is the person's own doing, as a scope picked in the Chats list is (#1655), so its value is
 * theirs to see at once, and held from there while the hold lasts.
 */
export function useHeldWhile<T>(holding: boolean, value: T, of?: string): T {
  const [held, setHeld] = useState<{ value: T; of: string | undefined } | null>(null);
  if (holding && (held === null || held.of !== of)) setHeld({ value, of });
  if (!holding && held !== null) setHeld(null);
  return holding && held !== null && held.of === of ? held.value : value;
}
