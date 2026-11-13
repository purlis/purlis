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
 * - **`label`**, **`closer`**, **`count`**, **`picture`** and **`indicator`** are inside a tab.
 *   The picture is a split tab's small picture of its layout's shape ({@link TabPicture}).
 * - **`toolbar`** (family `editor`) is the bar over a file, named by its file tab, which joins it
 *   while it is chosen ({@link editorToolbar}).
 *
 * The strip's tablist behaviour (the keyboard along it, its one Tab stop, the tablist that owns
 * the tabs) is React Aria's, in `TabStrip.tsx` (#3042); the drag along it is `dnd-kit`'s
 * (`sortable.tsx`). This layer is how it is drawn.
 */
import { useState, type ComponentPropsWithRef, type PointerEvent, type ReactNode } from "react";
import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";
import { Minus, X } from "lucide-react";
import clsx from "clsx";
import type { Picture } from "../../layoutPicture";
import { pressable, renderPressed, usePressTip } from "../pressable";
import { Tipped } from "../tooltip/Tooltip";

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

/** The attributes of a part inside a tab: its name, its ×, its counts and a split's picture. */
export function tabPart(part: "label" | "closer" | "count" | "picture") {
  return { "data-scope": "tabs", "data-part": part } as const;
}

/** Where a picture's box sits on the 16 its icon is drawn on, how wide it is, and the gap
 *  between its tiles. */
const BOX_AT = 2;
const BOX_SIDE = 12;
const GAP = 1.5;
/** A tile's edge, on the 16: the gap halved on each side that meets another tile. */
const placed = (at: number) => BOX_AT + BOX_SIDE * at;
const near = (at: number) => placed(at) + (at > 0 ? GAP / 2 : 0);
const far = (at: number) => placed(at) - (at < 1 ? GAP / 2 : 0);
/** A fraction of the box, to its few places: a share is a thousandth. */
const exact = (n: number) => Math.round(n * 1000) / 1000;

/**
 * **A split tab's small picture of its layout's shape** (#5342, spec #5339): a tile for each pane,
 * parted by a gap, in place of `+1` (#5577: tiles, so three panes never read as the quad). Drawn
 * at the size of the tab's other marks, in the quiet tier. A screen reader hears it as words: how
 * many panes it holds.
 */
export function TabPicture({ picture }: { picture: Picture }) {
  return (
    <>
      <svg {...tabPart("picture")} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
        {picture.tiles.map((tile) => {
          const [x, y] = [exact(tile.x), exact(tile.y)];
          const [right, bottom] = [exact(tile.x + tile.w), exact(tile.y + tile.h)];
          return (
            <rect
              key={`${x} ${y}`}
              x={near(x)}
              y={near(y)}
              width={far(right) - near(x)}
              height={far(bottom) - near(y)}
              rx="1"
            />
          );
        })}
      </svg>
      <span className="hidden-words">, split, {picture.panes} panes</span>
    </>
  );
}

/** The attributes of the bar over a file, which its file tab joins. */
export const editorToolbar = {
  "data-scope": "editor",
  "data-part": "toolbar",
  "data-variant": "default",
} as const;

/**
 * **What the editor's toolbar names**: the file's name, strong, then its path, as the switcher
 * and tab-strip prototypes draw it (R146). Where the toolbar's tab already says the file's name
 * (a file's own tab, a comparison's), it says the path alone, in the quiet tier (`named`
 * false, #3147): the tab is the name, and the view repeats it nowhere.
 */
export function ToolbarFile({ path, named = true }: { path: string; named?: boolean }) {
  return (
    <>
      {named && <b className="ui-text-strong">{path.slice(path.lastIndexOf("/") + 1)}</b>}
      <code data-path-only={named ? undefined : ""}>{path}</code>
    </>
  );
}

/**
 * **One tab's cell**: the element the strip lays out and drags, holding the tab's own button,
 * its marks and its `×`. It is drawn by its attributes: its variant, whether it is the chosen tab
 * (`data-selected`), whether the pointer is over it (`data-hovered`: a mouse's, from the cell's
 * own enter and leave), and whether it is the first on the strip, on the panel's edge
 * (`data-first`).
 *
 * Not React Aria's `useHover` (D-3042-7): it also ends a hover when a `pointerover` lands outside
 * the cell with no `pointerleave`, which is how it handles an element removed under the pointer,
 * and a story's hover then read as no hover in one cell of the matrix. The cell's own enter and
 * leave are what the strip has always drawn by.
 *
 * `className` places it, as the strip's own rules do (`.tab`, a chip); never a look.
 */
export function TabCell({
  variant,
  selected,
  first = false,
  children,
  className,
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
      // Its words are the body's (#1920), in the tier its state gives them (`tabs.css`).
      className={clsx("ui-text-body", className)}
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

/**
 * **A tab's close** (#2930): the `×` that closes a tab's layout and ends nothing (#4756), or a
 * task's own tab's `−`, which sends its talk back to its row (#1489). The tabs family's `closer`
 * part on React Aria's `Button`, through the press every pressable family shares
 * (`ui/pressable.tsx`); `tabs.css` draws it.
 *
 * - **Named by its row** (`label`, the catalogue's words), its glyph hidden; `note` is its
 *   tooltip's second line.
 * - **A Tab stop on the front tab only** (`reachable`, #5341): the strip is one stop, so a `×` per
 *   tab in the sequence would be fifty stops again.
 * - **A background tab's never takes the keyboard** (#5445): pressed, it closes a tab behind the
 *   one in front, and the keyboard stays where it was. The front tab's takes it, and its close
 *   puts the keyboard in what comes to the front (#5392).
 */
export function TabCloser({
  label,
  note,
  minimise = false,
  reachable = false,
  onPress,
}: {
  label: string;
  note?: string;
  /** A task's own tab's `−` rather than a close's `×`. */
  minimise?: boolean;
  /** The front tab's: the one close in the keyboard's order. */
  reachable?: boolean;
  onPress: () => void;
}) {
  const tip = usePressTip(label, note, true, false);
  const { native, aria, own } = pressable(
    {
      "aria-label": label,
      tabIndex: reachable ? 0 : -1,
      preventFocusOnPress: !reachable,
      onPress,
    },
    tip,
  );
  const Glyph = minimise ? Minus : X;
  return (
    <Tipped surface={tip.surface}>
      <AriaButton
        {...(aria as AriaButtonProps)}
        {...tabPart("closer")}
        data-minimise={minimise ? "" : undefined}
        render={renderPressed(native, tip, own)}
      >
        <Glyph aria-hidden="true" />
      </AriaButton>
    </Tipped>
  );
}
