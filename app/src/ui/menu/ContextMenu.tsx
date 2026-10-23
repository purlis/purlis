// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A context menu** (#2909): the menu family opened on a thing by a right-click, at the
 * pointer, or from the keyboard by Shift+F10 or the menu key on the thing itself. Its rows are a
 * `Menu`'s (`Menu.tsx`), drawn and walked as every other menu's.
 *
 * **Its own trigger, on React Aria's menu state.** React Aria's `MenuTrigger` opens a context menu
 * only through a pressable child, which would hand a terminal and a dragged tab React Aria's
 * press handling (it keeps the pointer's own `mousedown` from them). So the thing keeps its own
 * handlers, and this adds the two ways in: a `contextmenu` event, and the two keys.
 *
 * **The innermost thing's menu opens, and the ones around it stand back**: a `contextmenu` that
 * one menu has answered (its default prevented) opens no other, as the panes' menu stands back
 * for a terminal's.
 */
import {
  cloneElement,
  isValidElement,
  useEffect,
  useMemo,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import { useMenuTriggerState } from "react-stately/useMenuTriggerState";
import { OpenMenu } from "./Menu";

/** The `contextmenu` events sent for Shift+F10 and the menu key, which open on the first row. */
const FROM_THE_KEYS = new WeakSet<Event>();

/** Whether a key opens a context menu: Shift+F10 or the menu key. */
export function asksForAMenu(event: { key: string; shiftKey: boolean }): boolean {
  return event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
}

type Handlers = {
  /** Said on the element while its menu is open, as React Aria says an open trigger's state. */
  "data-open"?: string;
  onContextMenu?: (event: MouseEvent<HTMLElement>) => void;
  onKeyDown?: (event: KeyboardEvent<HTMLElement>) => void;
};

/**
 * **A context menu on `children`**, one element, which keeps its own handlers and layout: the
 * menu is `menu`, a `Menu`, drawn only while it is open, so a menu whose rows are worked out
 * from the window's state works them out only then.
 */
export function ContextMenu({
  children,
  menu,
  isDisabled,
  onAsked,
  onOpenChange,
  onClosed,
  opensFromWithin = false,
}: {
  /** Whether Shift+F10 and the menu key open it from anything inside the element that has the
   *  keyboard (a bar's items), at that thing, and not only from the element itself. */
  opensFromWithin?: boolean;
  /** The element the menu belongs to. One element: it is the trigger itself. */
  children: ReactNode;
  /** The menu, drawn while it is open. */
  menu: () => ReactNode;
  /** Opens nothing: the element is drawn as it is. */
  isDisabled?: boolean;
  /** The `contextmenu` event, before the menu reads it: preventing its default opens no menu
   *  here or around it, which is how a click a program asked for stays the program's. */
  onAsked?: (event: MouseEvent<HTMLElement>) => void;
  onOpenChange?: (isOpen: boolean) => void;
  /** Called once it has closed: where the keyboard goes instead of back where it was. */
  onClosed?: () => void;
}) {
  // Opened by a press of the pointer: the release of that press is not a row's.
  const [pressed, setPressed] = useState(false);
  const state = useMenuTriggerState({
    onOpenChange: (isOpen) => {
      // A menu closed while the button that opened it is still held owes that release nothing.
      if (!isOpen) setPressed(false);
      onOpenChange?.(isOpen);
    },
  });
  // The element it was opened on, as the event that opened it found it.
  const [on, setOn] = useState<Element | null>(null);
  const trigger = useMemo(() => ({ current: on }), [on]);
  useRightClickElsewhere(state.isOpen, state.close);
  useNotTheOpeningRelease(state.isOpen && pressed, () => setPressed(false));
  if (isDisabled || !isValidElement(children)) return <>{children}</>;
  const child = children as ReactElement<Handlers>;
  const own = child.props;
  // The element the menu was opened on is found by the event that opened it: the element keeps
  // its own `ref`, and no wrapper is added to a strip whose layout is measured.
  const handlers: Handlers = {
    "data-open": state.isOpen ? "" : undefined,
    onContextMenu: (event) => {
      own.onContextMenu?.(event);
      onAsked?.(event);
      // Answered by a menu inside it, or kept for a program: this one stands back.
      if (event.nativeEvent.defaultPrevented) return;
      event.preventDefault();
      setOn(event.currentTarget);
      setPressed(!FROM_THE_KEYS.has(event.nativeEvent));
      state.setPoint({ x: event.clientX, y: event.clientY });
      // From the keys, on its first row; from the pointer (or a program's own way in, as a
      // terminal's), the menu itself takes the keyboard and no row is chosen yet.
      state.open(FROM_THE_KEYS.has(event.nativeEvent) ? "first" : null);
    },
    onKeyDown: (event) => {
      own.onKeyDown?.(event);
      // Only when the element ITSELF has the keyboard: a key pressed inside it (a program in a
      // terminal) belongs to what it was pressed in.
      if (event.defaultPrevented) return;
      if (!opensFromWithin && event.target !== event.currentTarget) return;
      if (!asksForAMenu(event)) return;
      event.preventDefault();
      // **The event a right-click sends**, at the element's lower-left corner, so the menu a
      // pointer would open opens, and whatever notes what a menu was opened on (the Brief
      // panel's way back) notes it as it notes a right-click. macOS has no keyboard convention
      // for a context menu and its WebView raises no `contextmenu` for either key.
      const at = (event.target as Element).getBoundingClientRect();
      const asked = new globalThis.MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        clientX: at.left,
        clientY: at.bottom,
      });
      FROM_THE_KEYS.add(asked);
      event.target.dispatchEvent(asked);
    },
  };
  return (
    <>
      {cloneElement(child, handlers)}
      <OpenMenu
        state={state}
        triggerRef={trigger}
        // From the pointer, the menu itself takes the keyboard and no row is chosen yet.
        autoFocus={state.focusStrategy ?? true}
        onClosed={onClosed}
        placedByItsOpener
      >
        {state.isOpen && menu()}
      </OpenMenu>
    </>
  );
}

/**
 * A right-click anywhere outside an open context menu closes it, as every platform's does: the
 * menu's underlay takes the click, so it would otherwise stay up under a second one.
 */
function useRightClickElsewhere(isOpen: boolean, close: () => void) {
  useEffect(() => {
    if (!isOpen) return;
    const elsewhere = (event: globalThis.MouseEvent) => {
      const on = event.target;
      if (on instanceof Element && on.closest('[data-scope="popover"]')) return;
      close();
    };
    document.addEventListener("contextmenu", elsewhere, true);
    return () => document.removeEventListener("contextmenu", elsewhere, true);
  }, [isOpen, close]);
}

/**
 * **The release of the press that opened a menu runs no row.** A Mac's Ctrl+click opens a context
 * menu as the button goes down; near the window's edge the menu is placed under the pointer, and
 * the release of that same left button would land on a row and run it. So the first release
 * after it opens goes no further, until the pointer is pressed again.
 */
function useNotTheOpeningRelease(armed: boolean, disarm: () => void) {
  useEffect(() => {
    if (!armed) return;
    const release = (event: Event) => {
      event.stopPropagation();
      event.preventDefault();
      disarm();
    };
    const press = () => disarm();
    document.addEventListener("pointerup", release, true);
    document.addEventListener("mouseup", release, true);
    document.addEventListener("pointerdown", press, true);
    return () => {
      document.removeEventListener("pointerup", release, true);
      document.removeEventListener("mouseup", release, true);
      document.removeEventListener("pointerdown", press, true);
    };
  }, [armed, disarm]);
}
