// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";

/**
 * **Text that changes with a choice keeps its room** (#2910, the dialog rules of #2798, rule 3):
 * every text this place can show is laid out in one cell, and only the one it shows now is seen
 * and heard. The cell is as wide and as tall as the largest, so choosing, or a button's words
 * turning to "Creating…", never moves or resizes what is around it.
 *
 * `texts` lists every text the place can show, the one shown now among them or not; `children`
 * is the one shown now. `as="div"` keeps a block's room (a help line under a choice), the default
 * `span` a line's (a button's words).
 *
 * **The texts kept for their room are drawn by the stylesheet, not written in the page**
 * (`dialog.css`, from `data-text`): they take their room and are never read, so a button's
 * words are only the ones it shows, to a screen reader, to `textContent` and to a test that finds
 * it by its text.
 */
export function Reserved({
  texts,
  as: Tag = "span",
  children,
}: {
  texts: readonly string[];
  as?: "span" | "div";
  children: ReactNode;
}) {
  return (
    <Tag data-scope="reserved" data-part="root">
      <Tag data-scope="reserved" data-part="shown">
        {children}
      </Tag>
      {texts.map((text, at) => (
        <Tag key={at} data-scope="reserved" data-part="kept" data-text={text} aria-hidden="true" />
      ))}
    </Tag>
  );
}
