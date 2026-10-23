// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import * as ContextMenu from "@radix-ui/react-context-menu";
import * as Dialog from "@radix-ui/react-dialog";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import * as Popover from "@radix-ui/react-popover";
import * as Select from "@radix-ui/react-select";
import { isView, RADIX_FROM, sourceFiles, SRC, withoutComments } from "./guards.testkit";
import { PACKAGES, partsNotHeld } from "./overlaysHeld.testkit";
import { drawnOverTheWindow, InTheWindow, useProjectHost } from "./projectShown";

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

const OURS = import.meta.glob("./overlays/*.tsx", { eager: true });

/** The packages whose parts are drawn over the window, by the name after `react-`. */
const OVER = Object.entries(KINDS)
  .filter(([, what]) => what !== "in place")
  .map(([kind]) => kind.slice("@radix-ui/react-".length));

describe("every way the window draws something over itself", () => {
  it("is a kind the rule was written for", () => {
    const used = new Set<string>();
    for (const { text } of sources)
      for (const found of text.matchAll(new RegExp(`from "${RADIX_FROM}([a-z-]+)"`, "g")))
        used.add(`@radix-ui/react-${found[1]}`);
    // A package not listed is a kind nobody has decided about: say what it draws in `KINDS`,
    // give it a file in `overlays/`, and where it is drawn over the window, teach
    // `drawnOverTheWindow` its role below.
    expect([...used].filter((kind) => !(kind in KINDS))).toEqual([]);
  });

  it("is written through `overlays/`, where its portal asks whether its project is shown", () => {
    // From the package itself, an overlay's portal is Radix's own, and a project kept behind
    // another could open it over the one in front.
    const direct = sources
      .filter(({ file }) => !file.startsWith("overlays/"))
      .filter(({ text }) => new RegExp(`from "@radix-ui/react-(?:${OVER.join("|")})"`).test(text))
      .map(({ file }) => file);
    expect(direct).toEqual([]);
  });

  it("has a file there for each kind, and no other", () => {
    expect(Object.keys(OURS).sort()).toEqual(OVER.map((kind) => `./overlays/${kind}.tsx`).sort());
    expect(Object.keys(PACKAGES).sort()).toEqual([...OVER].sort());
  });

  it.each(OVER)(
    "hands on every part of %s that draws held back, by what it is and not by its name",
    (kind) => {
      // Radix exports every part twice, `Content` and `DialogContent`: the same part. Handed on
      // whole (`export *`), the long name was a way to write an overlay that asks nothing, and a
      // project behind could draw it over the window. So each file lists the parts it hands on:
      // Radix's own for a part that does not draw, and for one that does, a part that draws
      // nothing while its project is behind, which is looked at by drawing it there
      // (`overlaysHeld.testkit.ts`). A name nobody listed fails, whatever it is called.
      const ours = OURS[`./overlays/${kind}.tsx`] as Record<string, unknown>;
      expect(partsNotHeld(PACKAGES[kind], ours)).toEqual([]);
      // The portal everywhere; the content too, for an overlay written without its portal.
      expect(Object.keys(ours)).toEqual(expect.arrayContaining(["Portal", "Content"]));
    },
  );

  it("opens each one through its portal", () => {
    // `<Dialog.Content>` with no `<Dialog.Portal>` around it is drawn in place; held back all
    // the same (above), and never seen here, so the rule stays one sentence.
    const without = sources
      .filter(({ file }) => !file.startsWith("overlays/"))
      .flatMap(({ file, text }) =>
        [
          ...text.matchAll(
            new RegExp(`import \\* as (\\w+) from "${RADIX_FROM}(?:${OVER.join("|")})"`, "g"),
          ),
        ]
          .filter(
            ([, name]) => text.includes(`<${name}.Content`) && !text.includes(`<${name}.Portal`),
          )
          .map(([, name]) => `${file}: ${name}`),
      );
    expect(without).toEqual([]);
  });

  it("is drawn by one of those, and by no portal of the window's own", () => {
    const portals = sources
      .filter(({ text }) => /\bcreatePortal\(/.test(text))
      .map(({ file }) => file);
    // A new portal puts something outside where it is drawn: into its own project's element
    // (list it), or over the window, where it must go through `overlays/` as these do.
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

describe("a switch made from a row of a menu", () => {
  /** A project's place in the window, leaving the front when the menu's row is pressed. */
  function Switching({ slot, onLeftCovered }: { slot: HTMLElement; onLeftCovered: () => void }) {
    const [shown, setShown] = useState(true);
    const host = useProjectHost();
    return (
      <>
        <InTheWindow host={host} slot={slot} shown={shown} onLeftCovered={onLeftCovered} />
        <DropdownMenu.Root defaultOpen>
          <DropdownMenu.Trigger>More projects</DropdownMenu.Trigger>
          <DropdownMenu.Portal>
            <DropdownMenu.Content>
              <DropdownMenu.Item onSelect={() => setShown(false)}>
                Switch to project two
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
      </>
    );
  }

  it("leaves no menu over the window as the project goes, so the project is kept", () => {
    // The window's own menus are over the window too (the strip's show-more lists projects). A
    // menu that outlived the press of its row would cost every project left from one its view,
    // and nobody would see why: Radix closes it in the render the row's press makes, and no
    // menu here is kept for a closing animation.
    const own = document.body.appendChild(document.createElement("main"));
    const slot = own.appendChild(document.createElement("div"));
    const covered = vi.fn();
    render(<Switching slot={slot} onLeftCovered={covered} />);

    fireEvent.click(screen.getByRole("menuitem", { name: "Switch to project two" }));

    expect(screen.queryByRole("menu")).toBeNull();
    expect(covered).not.toHaveBeenCalled();
    own.remove();
  });
});
