// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createElement, type ComponentPropsWithRef, type ElementType } from "react";

/**
 * **The window's top layer** (#2910): the one element every menu, select list, popover and
 * tooltip is drawn into, over everything else the window draws, a dialog of the dialog family
 * included.
 *
 * A dialog of the family (`ui/dialog/Dialog.tsx`, on React Aria) makes the rest of the window
 * unreachable while it is up and holds the keyboard in itself; what a control inside it opens
 * (a select's list, a field's menu) is drawn outside it, in a portal, and would be made
 * unreachable with the rest. React Aria keeps an element marked `data-react-aria-top-layer`, and
 * what is drawn in it, reachable and visible, and lets the keyboard go into it: so the overlays
 * are drawn here. It is the window's own, added to the page once, and stacked over the window's
 * rows and its dialogs.
 */
export function topLayer(): HTMLElement {
  // Asked on every draw of an open list, so it looks only among the body's own children: a
  // `body > [...]` selector walks the whole window's tree to answer (#3136).
  const found = Array.from(document.body.children).find(
    (child): child is HTMLElement =>
      child instanceof HTMLElement && child.hasAttribute("data-purlis-top-layer"),
  );
  if (found) return found;
  const made = document.createElement("div");
  made.setAttribute("data-purlis-top-layer", "");
  made.setAttribute("data-react-aria-top-layer", "");
  made.style.position = "relative";
  made.style.zIndex = "1";
  document.body.append(made);
  return made;
}

/** An overlay's portal, drawn into the window's top layer unless it is given a place of its own. */
export function inTheTopLayer<T extends ElementType>(Portal: T): T {
  function InTheTopLayer(props: ComponentPropsWithRef<T>) {
    return createElement(Portal, { container: topLayer(), ...props });
  }
  return InTheTopLayer as unknown as T;
}
