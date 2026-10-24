// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The key registry on screen** (#2921, N25): the cheat sheet it generates, in its view tab's
 * room, and the key it gives a palette's row, a menu's row and a tooltip, each through its
 * family's `shortcut` part. Drawn with a Mac's keys, so the pictures are the same on every
 * runner; `OffAMac` walks the Ctrl ones. Each is held to axe, walked by the keyboard and compared
 * with its baseline in Chromium and WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { Button } from "../ui/button/Button";
import { AimedList, AimedOption } from "../ui/list/AimedList";
import { RowLabel, RowShortcut, RowTrail } from "../ui/list/Row";
import { Menu, MenuItem, MenuShortcuts, MenuTrigger, MenuWords } from "../ui/menu/Menu";
import { ActivityItem } from "../ui/activity/ActivityItem";
import { Files } from "lucide-react";
import { ShortcutsView } from "./ShortcutsView";
import { keySaid, rowAriaKeys, rowKey } from "./registry";

/** The cheat sheet in a view tab's room. */
function Sheet({ mac }: { mac: boolean }) {
  return (
    <div style={{ inlineSize: "40rem", blockSize: "30rem", display: "flex" }}>
      <ShortcutsView mac={mac} />
    </div>
  );
}

const meta = {
  component: Sheet,
  args: { mac: true },
} satisfies Meta<typeof Sheet>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The keyboard on the row with `name`, in the list. */
const row = (list: HTMLElement, name: RegExp) => within(list).getByRole("row", { name });

/**
 * **The cheat sheet**: the registry's groups, each act with where it applies and its key at the
 * row's end. The filter finds an act by its words or its key; Tab goes into the list, whose rows
 * the arrows walk; a filter that matches nothing says so.
 */
export const TheCheatSheet: Story = {
  play: async ({ canvas, userEvent }) => {
    const filter = canvas.getByRole("searchbox", { name: "Filter shortcuts" });
    const list = canvas.getByRole("grid", { name: "Keyboard shortcuts" });
    // Generated from the registry: its keys, in the shortcut part.
    for (const id of ["palette.open", "finder.open", "focus.toggle", "shortcuts.show"])
      expect(
        within(list).getAllByText(new RegExp(escape(keySaid(id, true)))).length,
      ).toBeGreaterThan(0);
    await userEvent.tab();
    await untilFocused(() => filter, "The cheat sheet: Tab onto its filter");
    expectFocusInSight();
    await userEvent.keyboard("work alone");
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(1));
    const focus = row(list, /Focus mode/);
    expect(within(focus).getByText("⌘⇧↩")).toHaveAttribute("data-part", "shortcut");
    await userEvent.clear(filter);
    // A key finds its act too.
    await userEvent.keyboard("⌘/");
    await waitFor(() => expect(row(list, /keyboard shortcuts/)).toBeVisible());
    await userEvent.clear(filter);
    await userEvent.tab();
    await untilFocused(() => row(list, /command palette/), "The cheat sheet: Tab into the list");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row(list, /Find a project/), "The cheat sheet: Down to the next act");
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    await untilFocused(() => filter, "The cheat sheet: Shift+Tab back to its filter");
    await userEvent.keyboard("nothing at all");
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent(
        "No shortcut is called “nothing at all”.",
      ),
    );
    await userEvent.clear(filter);
  },
};

/** **Off a Mac** Ctrl takes ⌘'s place, and the keys a chat keeps there say so. */
export const OffAMac: Story = {
  args: { mac: false },
  // The same rows with other words on them: `TheCheatSheet`'s picture is the sheet's.
  parameters: { screenshot: false },
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Keyboard shortcuts" });
    const filter = canvas.getByRole("searchbox", { name: "Filter shortcuts" });
    await userEvent.click(filter);
    await userEvent.keyboard("sidebar");
    const left = row(list, /left sidebar/);
    expect(left).toHaveTextContent("Ctrl+B");
    expect(left).toHaveTextContent("Not in a chat");
    await userEvent.clear(filter);
    await userEvent.keyboard("pinned");
    expect(row(list, /pinned workspaces/)).toHaveTextContent("Ctrl+1–Ctrl+9");
  },
};

/** A regular expression that finds `text` as it is written. */
function escape(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Palette rows as the catalogue lists them, by their ids. */
const PALETTE = [
  { id: "shell.new", title: "New shell" },
  { id: "view.toggle:navigation", title: "Put the Navigation region away" },
  { id: "focus.toggle", title: "Focus mode" },
  { id: "extensions.show", title: "Extensions…" },
];

/** A palette's rows, a tab's menu and an activity item: each says its key from the registry. */
function OnRows() {
  const [aimed, setAimed] = useState(0);
  const [said, setSaid] = useState("Nothing chosen");
  return (
    <MenuShortcuts.Provider value={(act) => rowKey(act, true)}>
      <div style={{ display: "grid", gap: "var(--space-4)", inlineSize: "26rem" }}>
        <AimedList id="rows" aria-label="Actions">
          {PALETTE.map((one, at) => {
            const key = rowKey(one.id, true);
            return (
              <AimedOption
                key={one.id}
                id={`row-${one.id}`}
                aimed={at === aimed}
                keys={rowAriaKeys(one.id, true)}
                onPress={() => setAimed(at)}
              >
                <RowLabel>{one.title}</RowLabel>
                {one.id === "extensions.show" && <RowTrail>What is in force</RowTrail>}
                {key !== undefined && <RowShortcut keys={key} />}
              </AimedOption>
            );
          })}
        </AimedList>
        <div style={{ display: "flex", gap: "var(--space-3)", alignItems: "center" }}>
          <MenuTrigger>
            <Button>Tab steward 4</Button>
            <Menu aria-label="Tab steward 4" onAction={(key) => setSaid(`Chose ${String(key)}`)}>
              <MenuItem id="rename" textValue="Rename…" shortcut="tab.rename:4">
                <MenuWords said="Rename…" />
              </MenuItem>
              <MenuItem id="shell" textValue="New shell" shortcut="shell.new">
                <MenuWords said="New shell" />
              </MenuItem>
              <MenuItem id="close" textValue="Close" shortcut="tab.close:4" variant="danger">
                <MenuWords said="Close" under="Ends the program it runs." />
              </MenuItem>
            </Menu>
          </MenuTrigger>
          <ActivityItem
            variant="bar"
            selected={false}
            aria-label="Explorer"
            icon={<Files />}
            tip="Show the Explorer view"
            shortcut={keySaid("view.explorer", true)}
          />
          <p role="status" style={{ margin: 0 }}>
            {said}
          </p>
        </div>
      </div>
    </MenuShortcuts.Provider>
  );
}

/**
 * **The key on a row** (N25): a palette row's at its end, a menu row's, and an activity item's
 * tooltip after its words, each from the registry by the act's id; a row with no key draws
 * none. Enter opens the menu, whose rows say their keys; Escape closes it and the keyboard is
 * back on its trigger.
 */
export const ShortcutsOnRows: Story = {
  render: () => <OnRows />,
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const rows = canvas.getByRole("listbox", { name: "Actions" });
    const shell = within(rows).getByRole("option", { name: /New shell/ });
    expect(shell).toHaveAttribute("aria-keyshortcuts", "Shift+Meta+T");
    expect(shell.querySelector('[data-part="shortcut"]')).toHaveTextContent("⌘⇧T");
    const none = within(rows).getByRole("option", { name: /Extensions/ });
    expect(none.querySelector('[data-part="shortcut"]')).toBeNull();
    const trigger = canvas.getByRole("button", { name: "Tab steward 4" });
    await userEvent.tab();
    await untilFocused(() => trigger, "On rows: Tab onto the menu's trigger");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Tab steward 4" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /Rename/ }),
      "On rows: Enter opens the menu",
    );
    expectFocusInSight();
    const keys = within(menu)
      .getAllByRole("menuitem")
      .map((item) => item.querySelector('[data-part="shortcut"]')?.textContent);
    expect(keys).toEqual(["F2", "⌘⇧T", "⌫"]);
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    await untilFocused(() => trigger, "On rows: Escape closes it, back on its trigger");
    // The activity item says its key after its tooltip's words.
    await userEvent.tab();
    const item = canvas.getByRole("button", { name: "Explorer" });
    await untilFocused(() => item, "On rows: Tab onto the activity item");
    const tip = await screen.findByRole("tooltip");
    expect(tip.querySelector('[data-part="shortcut"]')).toHaveTextContent("⌘⇧E");
    expectFocusInSight();
  },
};
