// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A native page, and the surfaces kept clear of it** (#6226): an artifact's page is a native
 * webview, drawn above the window's own page, so nothing the window draws can stand over it. Its
 * frame wears {@link NATIVE_PAGE}; the page is put away while a surface crosses that frame
 * (`coveredByAnOverlay`), and a card beside a row is placed so it does not (`AnchoredPopover`).
 */
import type { Side } from "./Popover";

/** The attribute a native page's frame wears: where its webview is laid. */
export const NATIVE_PAGE = "data-native-page";

/** Whether boxes `one` and `other` share any room: touching edges share none. */
export function crosses(one: DOMRect, other: DOMRect): boolean {
  return !(
    one.right <= other.left ||
    one.left >= other.right ||
    one.bottom <= other.top ||
    one.top >= other.bottom
  );
}

/**
 * **The region a surface beside `trigger` is kept in, so it does not cross a native page**: the
 * widest element around the trigger clear of every page on screen, when a page lies across the
 * side the surface opens on; or nothing, where it stands as it asked. Only a surface beside its
 * trigger (`left`, `right`) is moved: one under or over it hangs where it was opened from.
 */
export function keptOffPages(
  trigger: Element | null,
  side: Side,
  root: ParentNode = document,
): Element | null {
  if (trigger === null || (side !== "left" && side !== "right")) return null;
  const at = trigger.getBoundingClientRect();
  const pages = [...root.querySelectorAll(`[${NATIVE_PAGE}]`)]
    .filter((page) => page.closest("[hidden]") === null)
    .map((page) => page.getBoundingClientRect())
    .filter((page) => page.width > 0 && page.height > 0);
  const across = pages.some(
    (page) =>
      page.top < at.bottom &&
      page.bottom > at.top &&
      (side === "right" ? page.right > at.right : page.left < at.left),
  );
  if (!across || pages.some((page) => crosses(at, page))) return null;
  let region: Element | null = null;
  for (let one = trigger.parentElement; one !== null; one = one.parentElement) {
    const box = one.getBoundingClientRect();
    if (pages.some((page) => crosses(box, page))) break;
    region = one;
  }
  return region;
}
