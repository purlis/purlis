// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import * as ContextMenu from "@radix-ui/react-context-menu";
import * as Dialog from "@radix-ui/react-dialog";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import * as Popover from "@radix-ui/react-popover";
import * as Select from "@radix-ui/react-select";
import { isView, sourceFiles, SRC, withoutComments } from "./guards.testkit";
import { drawnOverTheWindow } from "./projectShown";

/**
 * **Nothing a project's view opens over the window can stay up over another project**
 * (`projectShown.tsx`, `drawnOverTheWindow`).
 *
 * A project kept behind the front one keeps its view, and a dialog, a menu or a select's list is
 * drawn outside that view, over the whole window. One left open as its project leaves the front
 * is why that project is not kept. The rule finds them by the role each kind is drawn with, so
 * what it has to be held to is the kinds: every way this window draws something over itself is
 * one the rule knows, and a new way fails here until it does.
 */

afterEach(cleanup);

/** Each Radix package the window draws with, and whether what it draws is over the window. */
const KINDS: Record<string, "over" | "says nothing to answer" | "in place"> = {
  "@radix-ui/react-alert-dialog": "over",
  "@radix-ui/react-context-menu": "over",
  "@radix-ui/react-dialog": "over",
  "@radix-ui/react-dropdown-menu": "over",
  "@radix-ui/react-popover": "over",
  "@radix-ui/react-select": "over",
  // A tooltip asks nothing and answers nothing: it is not why a project is taken down.
  "@radix-ui/react-tooltip": "says nothing to answer",
  "@radix-ui/react-checkbox": "in place",
  "@radix-ui/react-radio-group": "in place",
  "@radix-ui/react-roving-focus": "in place",
  "@radix-ui/react-switch": "in place",
};

/** The files that put something outside where they are drawn, by React's own portal. */
const PORTALS = [
  // Into the pane's own stack and the Inbox's place for the window's lines.
  "Notice.tsx",
  // Into the Panel's half the section is in.
  "PanelSection.tsx",
  // The project's view, into its own element.
  "PlaneView.tsx",
  // A view's element, into the region it is in.
  "RegionFrame.tsx",
];

const sources = sourceFiles(isView).map((file) => ({
  file,
  text: withoutComments(readFileSync(join(SRC, file), "utf8"), "ts"),
}));

describe("every way the window draws something over itself", () => {
  it("is a kind the rule was written for", () => {
    const used = new Set<string>();
    for (const { text } of sources)
      for (const found of text.matchAll(/from "(@radix-ui\/[a-z-]+)"/g)) used.add(found[1]);
    // A package not listed is a kind nobody has decided about: say what it draws in `KINDS`,
    // and where it is drawn over the window, teach `drawnOverTheWindow` its role below.
    expect([...used].filter((kind) => !(kind in KINDS))).toEqual([]);
  });

  it("is drawn by one of those, and by no portal of the window's own", () => {
    const portals = sources
      .filter(({ text }) => /\bcreatePortal\(/.test(text))
      .map(({ file }) => file);
    // A new portal puts something outside its project's element: into the project (list it), or
    // over the window, where it must be found by `drawnOverTheWindow` as these are.
    expect(portals).toEqual(PORTALS);
  });
});

describe("each kind drawn over the window is found as its project leaves the front", () => {
  /** The window's own element, with the place for the project in front. */
  function slot(): HTMLElement {
    const own = document.body.appendChild(document.createElement("main"));
    return own.appendChild(document.createElement("div"));
  }

  const kinds: [string, React.ReactElement][] = [
    [
      "@radix-ui/react-dialog",
      <Dialog.Root open>
        <Dialog.Portal>
          <Dialog.Content aria-describedby={undefined}>
            <Dialog.Title>Doctor</Dialog.Title>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>,
    ],
    [
      "@radix-ui/react-alert-dialog",
      <AlertDialog.Root open>
        <AlertDialog.Portal>
          <AlertDialog.Content aria-describedby={undefined}>
            <AlertDialog.Title>Delete persona steward?</AlertDialog.Title>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>,
    ],
    [
      "@radix-ui/react-popover",
      <Popover.Root open>
        <Popover.Trigger>Scope</Popover.Trigger>
        <Popover.Portal>
          <Popover.Content>This tab</Popover.Content>
        </Popover.Portal>
      </Popover.Root>,
    ],
    [
      "@radix-ui/react-dropdown-menu",
      <DropdownMenu.Root open>
        <DropdownMenu.Trigger>More</DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content>
            <DropdownMenu.Item>End chat</DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>,
    ],
    [
      "@radix-ui/react-select",
      <Select.Root open value="a">
        <Select.Trigger aria-label="Profile">
          <Select.Value />
        </Select.Trigger>
        <Select.Portal>
          <Select.Content>
            <Select.Viewport>
              <Select.Item value="a">
                <Select.ItemText>a</Select.ItemText>
              </Select.Item>
            </Select.Viewport>
          </Select.Content>
        </Select.Portal>
      </Select.Root>,
    ],
  ];

  it.each(kinds)("%s", (_, drawn) => {
    const place = slot();
    expect(drawnOverTheWindow(place)).toBe(false);
    render(drawn);
    expect(drawnOverTheWindow(place)).toBe(true);
    place.parentElement?.remove();
  });

  it("and every kind listed as drawn over the window is one of those, or a menu like them", () => {
    const over = Object.entries(KINDS)
      .filter(([, what]) => what === "over")
      .map(([kind]) => kind);
    // The context menu opens on a right-click and not on a prop; it draws the dropdown's
    // `role="menu"`, which is the one held above.
    expect(ContextMenu.Content).toBeDefined();
    expect(over.sort()).toEqual(
      [...kinds.map(([kind]) => kind), "@radix-ui/react-context-menu"].sort(),
    );
  });
});
