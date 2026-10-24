// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A list a field aims at** (#2911): the listbox of a combobox drawn in place, the palette's
 * actions, projects and files. The keyboard stays in the field the person types in, and the
 * field moves the aim (the WAI-ARIA combobox pattern, `aria-activedescendant` on the field), so
 * the list takes no keyboard of its own: its owner says which row is aimed at, and a press runs
 * a row. Drawn as every list's row is (`list.css`): the aimed row carries the keyboard's ring
 * inside it and the aim's own fill (`control.aimed`, quieter than a chosen row), so nothing moves
 * as the aim does.
 */
import { createContext, useContext, type ReactNode } from "react";
import { RowScope, type RowSize } from "./Row";

const AimedSize = createContext<RowSize>("md");

/** **The list**: one `listbox`, named, that its field controls by its `id`. */
export function AimedList({
  id,
  "aria-label": label,
  size = "md",
  className,
  children,
}: {
  /** What the field's `aria-controls` names. */
  id: string;
  "aria-label": string;
  size?: RowSize;
  /** A place for it in its view: its height, its scroll. Never a look. */
  className?: string;
  children: ReactNode;
}) {
  return (
    <RowScope.Provider value="list">
      <AimedSize.Provider value={size}>
        <ul
          id={id}
          role="listbox"
          aria-label={label}
          className={className}
          data-scope="list"
          data-part="body"
          data-size={size}
          data-aimed-by="field"
        >
          {children}
        </ul>
      </AimedSize.Provider>
    </RowScope.Provider>
  );
}

/**
 * **One option**: aimed at or not, by its owner. A press is its owner's: an option that cannot
 * run is drawn in the faint tier and its owner says why when it is pressed, rather than run it.
 */
export function AimedOption({
  id,
  aimed,
  unavailable = false,
  onPress,
  className,
  children,
}: {
  /** What the field's `aria-activedescendant` names while it is aimed at. */
  id: string;
  aimed: boolean;
  /** It cannot run now: said to a reader, drawn in the faint tier, its reason beside it. */
  unavailable?: boolean;
  onPress: () => void;
  className?: string;
  children: ReactNode;
}) {
  const size = useContext(AimedSize);
  return (
    // The pointer presses it; the keyboard is the field's, so it takes none.
    <li
      id={id}
      role="option"
      aria-selected={aimed}
      aria-disabled={unavailable || undefined}
      className={className}
      data-scope="list"
      data-part="root"
      data-size={size}
      data-aimed={aimed || undefined}
      data-focus-visible={aimed || undefined}
      data-disabled={unavailable || undefined}
      onClick={onPress}
    >
      {children}
    </li>
  );
}
