// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useCallback,
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
  /** Present while the list is held (#4020): what a story or a test waits on, as React Aria's
   *  `data-entering` says an overlay is opening. It draws nothing. */
  "data-holding": "" | undefined;
  onPointerOverCapture: PointerEventHandler<E>;
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
 * **What React hands the list is the list's own** ({@link heardIn}, #3594): React hands the
 * element around the list what happens in every element drawn inside it in React's tree, a
 * portal's too, so a row's menu and a row's card, each drawn in a layer of its own, hold it
 * as its rows do. The one element of its tree that does not is its filter, drawn into the
 * view's top, which would hold the list while it is typed in. The Chats list's filter stands
 * outside its list and never holds it; this one does not either.
 *
 * **A menu that goes from under the pointer sends no leave** (#3594): the element the pointer is
 * over next says it came from nowhere, so React tells the list nothing. So the pointer's hold
 * is let go too when the window hears the pointer come over an element React did not hand the
 * list, and the keyboard's when the focus lands on one: a menu of the window's elsewhere is
 * not the list's for being a menu.
 *
 * `holding` says whether it is held; `held` goes on the element around the list. `drawn` is what
 * the list draws its rows by: the focus is looked for after each draw it is a new value in,
 * since a menu or row that held the focus can go with no blur. A list that passes a new array
 * each time (the Personas view, the Chats list) has it looked for on every redraw of its panel,
 * which costs two comparisons. A hold that is not `on` is never taken, listens to nothing, and
 * lets go of what it held: the Chats list turns it off while it has no row to draw, where
 * nothing would say the pointer left.
 *
 * **The pointer's hold can wait as the pointer leaves** (`leaveAfter`, #3773): how long, asked
 * as it leaves, and nothing for a hold let go at once. Something the list draws a step away from
 * it, which the pointer crosses the gap to (a Chats row's card, over the view's inset), asks the
 * wait while it is up: the pointer coming over anything of the list's in that time keeps the
 * hold, and over nothing of it the hold goes once the wait is over. The window stopping being
 * the one in use lets go at once, waiting or not.
 */
export function useHeldStill<E extends HTMLElement>(
  drawn: unknown,
  on = true,
  leaveAfter: () => number = NO_WAIT,
): { holding: boolean; held: HeldBy<E> } {
  const ref = useRef<E>(null);
  /** The element the hold last saw the keyboard on: the one that must still have it. */
  const focused = useRef<Element | null>(null);
  /** The element a blur from the list said the focus went to, outside the list in the page:
   *  the list's own only if the list hears the focus arrive there (a row's menu). */
  const goingTo = useRef<Element | null>(null);
  /** The last `pointerover` React handed the list, which the window hears after it. */
  const seen = useRef<Event | null>(null);
  const [over, setOver] = useState(false);
  const [inside, setInside] = useState(false);
  /** The wait the pointer's hold is let go after, while the pointer is off the list. */
  const leaving = useRef<number | undefined>(undefined);
  /** How long the list asks it to wait, as it is now. */
  const asks = useRef(leaveAfter);
  useLayoutEffect(() => {
    asks.current = leaveAfter;
  });
  /** What the pointer coming over the list's own does: the wait, if any, is over. */
  const stay = useCallback(() => {
    window.clearTimeout(leaving.current);
    leaving.current = undefined;
  }, []);
  /** The pointer is off the list: let go, at once or after the wait the list asks. */
  const leave = useCallback(() => {
    const wait = asks.current();
    if (wait <= 0) {
      stay();
      setOver(false);
    } else if (leaving.current === undefined) {
      leaving.current = window.setTimeout(() => {
        leaving.current = undefined;
        setOver(false);
      }, wait);
    }
  }, [stay]);
  if (!on && (over || inside)) {
    setOver(false);
    setInside(false);
  }
  useEffect(() => {
    if (!on) return;
    const away = () => {
      stay();
      setOver(false);
      setInside(false);
    };
    // Heard by the window after the list's own handlers: an element the focus went to that the
    // list did not hear it arrive in is not one of its own.
    const landed = (event: Event) => {
      const to = goingTo.current;
      goingTo.current = null;
      if (to !== null && event.target === to) setInside(false);
    };
    // And the pointer over an element React did not hand the list is off it.
    const pointedAt = (event: Event) => {
      if (seen.current !== event) leave();
      seen.current = null;
    };
    window.addEventListener("blur", away);
    window.addEventListener("focusin", landed);
    window.addEventListener("pointerover", pointedAt);
    return () => {
      stay();
      window.removeEventListener("blur", away);
      window.removeEventListener("focusin", landed);
      window.removeEventListener("pointerover", pointedAt);
    };
  }, [on, stay, leave]);
  // After each draw of new rows: a row the keyboard was on, taken out of the page, took the
  // focus with it. The element the hold saw is asked, never what the list holds in the page:
  // a row's menu is drawn outside it.
  useLayoutEffect(() => {
    if (!inside) return;
    const at = focused.current;
    if (at === null || !at.isConnected || at !== document.activeElement) setInside(false);
  }, [inside, drawn]);
  const holding = on && (over || inside);
  return {
    holding,
    held: {
      ref,
      "data-holding": holding ? "" : undefined,
      onPointerOverCapture: (event) => {
        seen.current = event.nativeEvent;
        if (heardIn(event.target)) stay();
      },
      onPointerEnter: (event) => {
        if (heardIn(event.target)) setOver(true);
      },
      // Taken again by the pointer's next move, after the window was left and come back to; and
      // let go on a move over the filter, which React counts as inside.
      onPointerMove: (event) => {
        const here = heardIn(event.target);
        if (here) stay();
        if (here !== over) setOver(here);
      },
      onPointerLeave: () => leave(),
      // A focus by the keyboard takes the hold; any focus in the list while it holds moves what
      // the hold follows (an arrow's move, a menu's item, the row a menu gave the focus back to).
      onFocus: (event) => {
        if (!heardIn(event.target)) return;
        if (event.target === goingTo.current) goingTo.current = null;
        if (!(inside || byKeyboard(event.target))) return;
        focused.current = event.target;
        setInside(true);
      },
      // Kept for a focus that stays in the list. One that goes elsewhere in the page is kept
      // until the window hears where it landed: the list's `onFocus` has heard it by then if
      // that is its own (a row's menu), and it is let go if not. Never let go in between, which
      // would draw the rows as they are now under the menu as it opens.
      onBlur: (event: FocusEvent<E>) => {
        const to = event.relatedTarget;
        if (to instanceof Node && ref.current?.contains(to)) return;
        if (to instanceof Element) {
          goingTo.current = to;
          return;
        }
        setInside(false);
      },
      // A key in the list is the keyboard being here.
      onKeyDownCapture: (event) => {
        if (!heardIn(event.target) || !(event.target instanceof Element)) return;
        focused.current = event.target;
        if (!inside) setInside(true);
      },
    },
  };
}

/** No wait: the pointer's hold is let go as the pointer leaves. */
const NO_WAIT = () => 0;

/** The view's top, where a list in a view draws its filter (`ui/panel/Panel`'s `InPanelTop`). */
const IN_THE_TOP = "[data-top-slot]";

/**
 * Whether an event React handed the list, from `target`, is one a hold is taken for: any
 * element React draws inside the list, its rows' menus and cards too, but the filter it draws
 * into the view's top. Asked only of what React hands the list, which is its own.
 */
export function heardIn(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(IN_THE_TOP) === null;
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
