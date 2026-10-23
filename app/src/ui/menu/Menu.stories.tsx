// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The menu family** (#2909): a dropdown, a row's ⋯ menu, groups with a toggle, a shortcut, a
 * row that cannot be undone and a row that cannot run, a submenu, and a context menu, each
 * drawn, held to axe, walked by the keyboard and compared with its baseline in Chromium and
 * WebKit (`docs/component-stories.md`). A menu draws outside its frame, so each picture is the
 * page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { Ellipsis } from "lucide-react";
import { expectFocusInSight } from "../../stories.testkit";
import { Button, IconButton } from "../button/Button";
import { ContextMenu } from "./ContextMenu";
import {
  Menu,
  MenuItem,
  MenuSection,
  MenuSeparator,
  MenuTrigger,
  MenuWords,
  Submenu,
  type Key,
} from "./Menu";

/** What was chosen last, under the trigger, as a person would read it. */
function Chosen({ said }: { said: string }) {
  return (
    <p role="status" style={{ margin: 0 }}>
      {said}
    </p>
  );
}

/** A tab's menu: groups, a toggle, shortcuts, a row that cannot run, and what cannot be undone. */
function TabMenu() {
  const [said, setSaid] = useState("Nothing chosen");
  const [pinned, setPinned] = useState(false);
  const chose = (key: Key) => setSaid(`Chose ${String(key)}`);
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <MenuTrigger>
        <Button>Tab steward 4</Button>
        <Menu aria-label="Tab steward 4" onAction={chose}>
          <MenuSection title="This chat">
            <MenuItem id="rename" textValue="Rename…" keys="F2">
              <MenuWords said="Rename…" />
            </MenuItem>
          </MenuSection>
          <MenuSeparator />
          <MenuSection
            selectionMode="multiple"
            selectedKeys={pinned ? ["pinned"] : []}
            onSelectionChange={() => setPinned((was) => !was)}
            shouldCloseOnSelect
          >
            <MenuItem id="pinned" textValue="Pinned">
              <MenuWords said="Pinned" />
            </MenuItem>
          </MenuSection>
          <MenuSeparator />
          <MenuSection>
            <MenuItem id="restart" textValue="Restart" isDisabled>
              <MenuWords said="Restart" under="It is starting already." />
            </MenuItem>
            <MenuItem id="end" textValue="End" variant="danger" keys="⌫">
              <MenuWords said="End" under="Ends the program it runs. There is no undo." />
            </MenuItem>
          </MenuSection>
        </Menu>
      </MenuTrigger>
      <Chosen said={said} />
    </div>
  );
}

const meta = {
  component: TabMenu,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof TabMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The innermost menu open on the page: a submenu is drawn after the menu it opened from. */
/** How long a menu may take to go: WebKit draws the next frame late in a busy run. */
const CLOSES = { timeout: 3_000 };

const openMenu = () => {
  const menus = screen.getAllByRole("menu");
  return menus[menus.length - 1];
};

/**
 * **A menu from its trigger**: Enter opens it on its first row, the arrows walk it and stop at
 * its ends, a row that cannot run is passed over, Escape closes it and the keyboard is back on
 * the trigger; the baseline shows it open on its last row.
 */
export const FromItsTrigger: Story = {
  play: async ({ canvas, userEvent }) => {
    const trigger = canvas.getByRole("button", { name: "Tab steward 4" });
    await userEvent.tab();
    await waitFor(() => expect(trigger).toHaveFocus());
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Tab steward 4" });
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: /Rename/ })).toHaveFocus(),
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    // Restart cannot run, and the arrows go past it to End.
    await waitFor(() => expect(within(menu).getByRole("menuitem", { name: /End/ })).toHaveFocus());
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(within(menu).getByRole("menuitem", { name: /End/ })).toHaveFocus());
    expectFocusInSight();
    // Typing a letter finds a row by its words (Restart cannot run, and is passed over).
    await userEvent.keyboard("r");
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: /Rename/ })).toHaveFocus(),
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await waitFor(() => expect(trigger).toHaveFocus());
    // Open again for the picture, on the row that cannot be undone.
    await userEvent.keyboard("{ArrowUp}");
    await waitFor(() =>
      expect(within(openMenu()).getByRole("menuitem", { name: /End/ })).toHaveFocus(),
    );
  },
};

/** **A toggle is one row with a tick**: Enter turns it on, and the menu closes as a row's does. */
export const AToggle: Story = {
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Tab steward 4" }));
    const pinned = within(await screen.findByRole("menu")).getByRole("menuitemcheckbox", {
      name: "Pinned",
    });
    await expect(pinned).toHaveAttribute("aria-checked", "false");
    await userEvent.click(pinned);
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    // The keyboard is back on the trigger once the menu has gone.
    await waitFor(() =>
      expect(canvas.getByRole("button", { name: "Tab steward 4" })).toHaveFocus(),
    );
    await userEvent.keyboard("{Enter}");
    const again = within(await screen.findByRole("menu")).getByRole("menuitemcheckbox", {
      name: "Pinned",
    });
    await expect(again).toHaveAttribute("aria-checked", "true");
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(again).toHaveFocus());
    expectFocusInSight();
  },
};

/** **A click outside closes it**, and reaches what it was on. No picture: what is left drawn is
 *  the closed trigger `FromItsTrigger` already draws open. */
export const ClosedByAClickOutside: Story = {
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Tab steward 4" }));
    await screen.findByRole("menu");
    await userEvent.click(canvas.getByRole("status"));
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
  },
  parameters: { screenshot: false },
};

/**
 * **A row's ⋯ menu** (R56, R59), as a setting's row has it: a quiet icon button at the row's
 * end, and its menu under it, lined up with its end, never nearer the window's edge than 8 px.
 */
function RowMenu() {
  const [said, setSaid] = useState("Nothing chosen");
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "12rem 1fr auto",
        alignItems: "center",
        gap: "var(--space-4)",
        inlineSize: "34rem",
      }}
    >
      <span>Default profile</span>
      <span>claude</span>
      <MenuTrigger>
        <IconButton
          variant="ghost"
          aria-label="Reset, move to another file"
          icon={<Ellipsis aria-hidden="true" />}
        />
        <Menu
          aria-label="Default profile"
          align="end"
          onAction={(key) => setSaid(`Chose ${String(key)}`)}
        >
          <MenuSection>
            <MenuItem id="reset" textValue="Reset to claude">
              <MenuWords said="Reset to claude" />
            </MenuItem>
          </MenuSection>
          <MenuSeparator />
          <MenuSection>
            <MenuItem id="local" textValue="Move to purlis.local.toml">
              <MenuWords said="Move to purlis.local.toml" />
            </MenuItem>
            <MenuItem id="yours" textValue="Move to your settings">
              <MenuWords said="Move to your settings" />
            </MenuItem>
          </MenuSection>
          <MenuSeparator />
          <MenuSection>
            <MenuItem id="copy" textValue="Copy key default_profile">
              <MenuWords said="Copy key default_profile" />
            </MenuItem>
          </MenuSection>
        </Menu>
      </MenuTrigger>
      <Chosen said={said} />
    </div>
  );
}

export const ARowsMenu: Story = {
  render: () => <RowMenu />,
  play: async ({ canvas, userEvent }) => {
    const trigger = canvas.getByRole("button", { name: "Reset, move to another file" });
    await userEvent.tab();
    await waitFor(() => expect(trigger).toHaveFocus());
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Default profile" });
    // Under its button, lined up with its end.
    const at = trigger.getBoundingClientRect();
    const surface = menu.closest('[data-part="root"]') as HTMLElement;
    const box = surface.getBoundingClientRect();
    await expect(box.top).toBeGreaterThanOrEqual(at.bottom);
    await expect(Math.abs(box.right - at.right)).toBeLessThanOrEqual(1);
    await expect(box.left).toBeGreaterThanOrEqual(8);
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: /Reset/ })).toHaveFocus(),
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Chose local"));
    await waitFor(() => expect(trigger).toHaveFocus());
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("menu", { name: "Default profile" });
  },
};

/** **A row that opens a submenu** (the menus' rule 3): Right opens it, Left closes it. */
function WithASubmenu() {
  const [said, setSaid] = useState("Nothing chosen");
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <MenuTrigger>
        <Button>Chat steward 4</Button>
        <Menu aria-label="Chat steward 4" onAction={(key) => setSaid(`Chose ${String(key)}`)}>
          <MenuItem id="switch" textValue="Switch to">
            <MenuWords said="Switch to" />
          </MenuItem>
          <Submenu
            aria-label="Ask"
            onAction={(key) => setSaid(`Asked ${String(key)}`)}
            row={
              <MenuItem id="ask" textValue="Ask">
                <MenuWords said="Ask" />
              </MenuItem>
            }
          >
            <MenuItem id="devops" textValue="devops…">
              <MenuWords said="devops…" />
            </MenuItem>
            <MenuItem id="qa" textValue="qa…">
              <MenuWords said="qa…" />
            </MenuItem>
          </Submenu>
        </Menu>
      </MenuTrigger>
      <Chosen said={said} />
    </div>
  );
}

export const ASubmenu: Story = {
  render: () => <WithASubmenu />,
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Chat steward 4" });
    await userEvent.keyboard("{ArrowDown}");
    const ask = within(menu).getByRole("menuitem", { name: /Ask/ });
    await expect(ask).toHaveAttribute("aria-haspopup", "menu");
    await waitFor(() => expect(ask).toHaveFocus());
    await userEvent.keyboard("{ArrowRight}");
    const sub = await screen.findByRole("menu", { name: "Ask" });
    await waitFor(() =>
      expect(within(sub).getByRole("menuitem", { name: /devops/ })).toHaveFocus(),
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(screen.queryByRole("menu", { name: "Ask" })).toBeNull());
    await waitFor(() => expect(ask).toHaveFocus());
    await userEvent.keyboard("{ArrowRight}{ArrowDown}");
    await waitFor(() =>
      expect(within(openMenu()).getByRole("menuitem", { name: /qa/ })).toHaveFocus(),
    );
    expect(canvas.getByRole("status")).toHaveTextContent("Nothing chosen");
  },
};

/** **A context menu on a row**: Shift+F10 opens it on its first row, Escape closes it. */
function OnARow() {
  const [said, setSaid] = useState("Nothing chosen");
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <ContextMenu
        menu={() => (
          <Menu aria-label="Row steward 4" onAction={(key) => setSaid(`Chose ${String(key)}`)}>
            <MenuItem id="open" textValue="Open">
              <MenuWords said="Open" />
            </MenuItem>
            <MenuItem id="rename" textValue="Rename…" keys="F2">
              <MenuWords said="Rename…" />
            </MenuItem>
          </Menu>
        )}
      >
        <div tabIndex={0} role="group" aria-label="steward 4" style={{ padding: "var(--space-3)" }}>
          steward 4
        </div>
      </ContextMenu>
      <Chosen said={said} />
    </div>
  );
}

export const AContextMenu: Story = {
  render: () => <OnARow />,
  play: async ({ canvas, userEvent }) => {
    const row = canvas.getByRole("group", { name: "steward 4" });
    await userEvent.tab();
    await waitFor(() => expect(row).toHaveFocus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "Row steward 4" });
    await waitFor(() => expect(within(menu).getByRole("menuitem", { name: /Open/ })).toHaveFocus());
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await waitFor(() => expect(row).toHaveFocus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() =>
      expect(within(openMenu()).getByRole("menuitem", { name: /Rename/ })).toHaveFocus(),
    );
  },
};
