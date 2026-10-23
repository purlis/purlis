// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The session tabs, docked on the content panel** (#2914, spec #2897, ADR 0103, ADR 0104): the
 * parts the strip of a workspace's tabs is drawn by, and the editor toolbar a file's tab joins.
 * `tabs.css` draws the family default from these attributes and a theme's parts reach the same
 * ones (`purlis theme parts` lists them).
 *
 * - **`layout`** is the column: the strip, then the content panel the tabs attach to (`ui/panel`,
 *   `panelRoot("content")`). The
 *   theme's `tabs.layout.attach` says what they attach to: the panel (Purlis: the first tab on the
 *   panel's edge, the chosen tab joining it with inverse-corner flares) or a strip of their own
 *   (Purlis Classic).
 * - **`strip`** is the row the tabs sit in. Its tabs shrink, each cut short with an ellipsis,
 *   down to the strip's floor; past it they collapse into its show-more menu (ADR 0039). The `+`
 *   that opens a chat ends it.
 * - **`tab`** is one tab's cell ({@link TabCell}): the tab's own button, its marks and its `×`.
 *   Choosing a tab changes its colour and its fill and nothing else, so no tab moves. An unchosen
 *   tab under the pointer draws a pill inset from its cell, which never touches the panel.
 * - **`label`**, **`closer`**, **`count`** and **`indicator`** are inside a tab.
 * - **`toolbar`** (family `editor`) is the bar over a file, named by its file tab, which joins it
 *   while it is chosen ({@link editorToolbar}).
 *
 * The strip keeps its behaviour where it was: the roving focus, the drag along the strip and the
 * tablist that owns the tabs (`ChatStrip.tsx`, `StripTablist.tsx`). This layer is how it is drawn.
 */
import { useState, type ComponentPropsWithRef, type PointerEvent, type ReactNode } from "react";

/** The tab's variants, in the order the manifest names them. */
export const TAB_VARIANTS = ["chat", "view", "file", "background"] as const;
export type TabVariant = (typeof TAB_VARIANTS)[number];

/** The parts drawn around the tabs, each reached by its own attributes. */
export type AroundPart = "layout" | "strip";

/**
 * The attributes of a part drawn around the tabs, for an element that is placed by another
 * library or view (a region's column, a header the strip's roving focus is on).
 */
export function tabsPart(part: AroundPart) {
  return { "data-scope": "tabs", "data-part": part } as const;
}

/**
 * **The panel the chosen tab names** (WAI-ARIA Tabs): the content under the strip, while the
 * chosen tab is drawn on it; nothing while it is not.
 */
export function tabPanelOf(namedBy: string | undefined) {
  return namedBy === undefined ? {} : ({ role: "tabpanel", "aria-labelledby": namedBy } as const);
}

/** The attributes of a part inside a tab: its name, its × and its counts. */
export function tabPart(part: "label" | "closer" | "count") {
  return { "data-scope": "tabs", "data-part": part } as const;
}

/** The attributes of the bar over a file, which its file tab joins. */
export const editorToolbar = {
  "data-scope": "editor",
  "data-part": "toolbar",
  "data-variant": "default",
} as const;

/**
 * **What the editor's toolbar names**: the file's name, strong, then its path, as the switcher
 * and tab-strip prototypes draw it (R146).
 */
export function ToolbarFile({ path }: { path: string }) {
  return (
    <>
      <b className="ui-text-strong">{path.slice(path.lastIndexOf("/") + 1)}</b>
      <code>{path}</code>
    </>
  );
}

/**
 * **One tab's cell**: the element the strip lays out and drags, holding the tab's own button,
 * its marks and its `×`. It is drawn by its attributes: its variant, whether it is the chosen tab
 * (`data-selected`), whether the pointer is over it (`data-hovered`, a mouse's only, as React
 * Aria has it), and whether it is the first on the strip, on the panel's edge (`data-first`).
 *
 * `className` places it, as the strip's own rules do (`.tab`, a chip); never a look.
 */
export function TabCell({
  variant,
  selected,
  first = false,
  children,
  onPointerEnter,
  onPointerLeave,
  ...rest
}: Omit<ComponentPropsWithRef<"span">, "children"> & {
  variant: TabVariant;
  selected: boolean;
  /** The first tab on the strip, which starts at the panel's edge. */
  first?: boolean;
  children?: ReactNode;
  [data: `data-${string}`]: string | number | boolean | undefined;
}) {
  const [hovered, setHovered] = useState(false);
  const hover = (on: boolean) => (event: PointerEvent<HTMLSpanElement>) => {
    if (event.pointerType === "mouse") setHovered(on);
  };
  return (
    <span
      {...rest}
      data-scope="tabs"
      data-part="tab"
      data-variant={variant}
      data-selected={selected || undefined}
      data-hovered={hovered || undefined}
      data-first={first || undefined}
      onPointerEnter={(event) => {
        hover(true)(event);
        onPointerEnter?.(event);
      }}
      onPointerLeave={(event) => {
        hover(false)(event);
        onPointerLeave?.(event);
      }}
    >
      {children}
      {/* The theme's indicator, if it draws one: a mark on the chosen tab's edge, none by
          default. Out of the row, so it takes no room. */}
      <span data-scope="tabs" data-part="indicator" aria-hidden="true" />
    </span>
  );
}
