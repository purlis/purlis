// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createElement, type ElementType, type ReactNode } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import * as ContextMenu from "@radix-ui/react-context-menu";
import * as Dialog from "@radix-ui/react-dialog";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import * as Popover from "@radix-ui/react-popover";
import * as Select from "@radix-ui/react-select";
import * as OurAlertDialog from "./overlays/alert-dialog";
import * as OurContextMenu from "./overlays/context-menu";
import * as OurDialog from "./overlays/dialog";
import * as OurDropdownMenu from "./overlays/dropdown-menu";
import * as OurPopover from "./overlays/popover";
import * as OurSelect from "./overlays/select";
import { ProjectShownProvider } from "./projectShown";

/**
 * **Whether a file of `overlays/` hands on every part that draws over the window held back**
 * (#1979), decided by what each part is and what it draws, and not by its name.
 *
 * Every part Radix's package exports is listed, by Radix's short name, as one that draws over
 * the window or one that does not. A part the package has that is not listed (a new one, after
 * an upgrade) is a kind of drawing nobody has decided about, and fails. So does every name the
 * file hands on that is not listed, whatever it is called: `Popup`, or Radix's long name for a
 * listed part.
 *
 * A part that does not draw is handed on as Radix's own. A part that draws is handed on as
 * something that draws nothing of what it is given while its project is behind
 * (`onlyWhileShown`, `holdingOnlyWhileShown`): it is drawn here, in an open overlay of its
 * kind, under a project that is not in front, and nothing given it may reach the document.
 */

/** Each part a Radix package exports, by its short name: whether what it draws is over the window. */
export interface Parts {
  /** What puts something over the window: the portal, the content, the overlay, a sub-menu's content. */
  over: readonly string[];
  /** Everything else the package exports, its scope's maker among them. */
  notOver: readonly string[];
}

/** One Radix package, its parts, and an open overlay of its kind to draw a part in. */
export interface Package {
  /** The package as Radix exports it. */
  real: Record<string, unknown>;
  parts: Parts;
  /** Draws its children in an overlay of the kind, as the window's own file writes it. */
  Within: (props: { children: ReactNode }) => ReactNode;
  /** Opens that overlay once it is drawn, for a kind no prop opens. */
  open?: () => void;
}

/** Draws its children in `Root`, opened by its prop. */
const openIn =
  (Root: ElementType) =>
  ({ children }: { children: ReactNode }) =>
    createElement(Root, { open: true }, children);

/** What the context menu's trigger says, to right-click it by. */
const RIGHT_CLICKED = "the place a context menu opens from";

/**
 * Each of those packages as Radix exports it, every part it has by Radix's short name, said to
 * draw over the window or not, and an open overlay of the kind to draw a part in. A part the
 * package gains in an upgrade is not listed, and the guard fails until it is (#1979).
 */
const MENU: Parts = {
  over: ["Portal", "Content", "SubContent"],
  notOver: [
    "Root",
    "Trigger",
    "Group",
    "Label",
    "Item",
    "CheckboxItem",
    "RadioGroup",
    "RadioItem",
    "ItemIndicator",
    "Separator",
    "Arrow",
    "Sub",
    "SubTrigger",
  ],
};
export const PACKAGES: Record<string, Package> = {
  "alert-dialog": {
    real: AlertDialog,
    parts: {
      over: ["Portal", "Overlay", "Content"],
      notOver: [
        "Root",
        "Trigger",
        "Title",
        "Description",
        "Action",
        "Cancel",
        "createAlertDialogScope",
      ],
    },
    Within: openIn(OurAlertDialog.Root),
  },
  "context-menu": {
    real: ContextMenu,
    parts: { over: MENU.over, notOver: [...MENU.notOver, "createContextMenuScope"] },
    // Opened by a right-click on its trigger, not by a prop.
    Within: ({ children }) =>
      createElement(
        OurContextMenu.Root,
        null,
        createElement(OurContextMenu.Trigger, null, RIGHT_CLICKED),
        children,
      ),
    open: () => fireEvent.contextMenu(screen.getByText(RIGHT_CLICKED)),
  },
  dialog: {
    real: Dialog,
    parts: {
      over: ["Portal", "Overlay", "Content"],
      notOver: [
        "Root",
        "Trigger",
        "Title",
        "Description",
        "Close",
        "createDialogScope",
        "WarningProvider",
      ],
    },
    Within: openIn(OurDialog.Root),
  },
  "dropdown-menu": {
    real: DropdownMenu,
    parts: { over: MENU.over, notOver: [...MENU.notOver, "createDropdownMenuScope"] },
    Within: openIn(OurDropdownMenu.Root),
  },
  popover: {
    real: Popover,
    parts: {
      over: ["Portal", "Content"],
      notOver: [
        "Root",
        "Anchor",
        "Trigger",
        "Close",
        "Arrow",
        "Title",
        "Description",
        "createPopoverScope",
      ],
    },
    Within: openIn(OurPopover.Root),
  },
  select: {
    real: Select,
    parts: {
      over: ["Portal", "Content"],
      notOver: [
        "Root",
        "Trigger",
        "Value",
        "Icon",
        "Viewport",
        "Group",
        "Label",
        "Item",
        "ItemText",
        "ItemIndicator",
        "ScrollUpButton",
        "ScrollDownButton",
        "Separator",
        "Arrow",
        "createSelectScope",
        // The hidden input a form reads and the context it is drawn with: in place.
        "unstable_BubbleInput",
        "unstable_Provider",
      ],
    },
    Within: openIn(OurSelect.Root),
  },
};
/** What a part is given to draw, in one element as a portal takes it, to look for in the document. */
const GIVEN = "drawn by a project behind the one in front";

/**
 * What is wrong with `ours`, the window's file for one kind of overlay, against the package
 * it hands on: one line for each part, none when every part that draws is held back.
 */
export function partsNotHeld(
  { real, parts, Within, open }: Package,
  ours: Record<string, unknown>,
): string[] {
  const wrong: string[] = [];
  const listed = new Set([...parts.over, ...parts.notOver]);
  for (const name of listed)
    if (!(name in real)) wrong.push(`${name}: listed, and not a part of the package`);

  // Radix exports each part twice, by two names; the parts are the values.
  const known = new Set([...listed].map((name) => real[name]));
  const unknown = new Map<unknown, string[]>();
  for (const [name, part] of Object.entries(real))
    if (!known.has(part)) unknown.set(part, [...(unknown.get(part) ?? []), name]);
  for (const names of unknown.values())
    wrong.push(
      `${names.join(" / ")}: a part of the package nobody has said draws over the window or not`,
    );

  const radix = new Set(Object.values(real));
  for (const [name, part] of Object.entries(ours)) {
    if (!listed.has(name)) wrong.push(`${name}: handed on, and not a part listed for the package`);
    else if (parts.notOver.includes(name)) {
      if (part !== real[name]) wrong.push(`${name}: does not draw, and is not Radix's own`);
    } else if (radix.has(part)) wrong.push(`${name}: draws, and is handed on as Radix made it`);
    else {
      const drawn = drawnWhileBehind(part as ElementType, Within, open);
      if (drawn !== null) wrong.push(`${name}: draws, and ${drawn} while its project is behind`);
    }
  }
  return wrong;
}

/**
 * What `Part` does when drawn in an open overlay under a project that is not in front: `null`
 * when it draws nothing of what it is given, and otherwise what it did, drawing it or failing to
 * be drawn there at all (with the error it threw), either way not held back.
 */
function drawnWhileBehind(
  Part: ElementType,
  Within: Package["Within"],
  open: Package["open"],
): string | null {
  try {
    render(
      createElement(
        ProjectShownProvider,
        { value: false },
        createElement(Within, null, createElement(Part, null, createElement("span", null, GIVEN))),
      ),
    );
    open?.();
    return document.body.textContent?.includes(GIVEN) ? "draws what it is given" : null;
  } catch (error) {
    return `throws (${error instanceof Error ? error.message : String(error)})`;
  } finally {
    cleanup();
  }
}
