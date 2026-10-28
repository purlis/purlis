// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The popover family** (#2909): a card opened from its trigger, a list laid out as R165 has
 * it (a filter row with its rule, the scrolling list, the footer that says the keys), and a
 * refusal held up beside the field it is about, each drawn, held to axe, walked by the keyboard
 * and compared with its baseline in Chromium and WebKit (`docs/component-stories.md`). A popover
 * draws outside its frame, so each picture is the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useRef, useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocusWithin, untilFocused } from "../../stories.testkit";
import { Button } from "../button/Button";
import { InlineMenu, MenuItem } from "../menu/Menu";
import {
  AnchoredPopover,
  PopoverDialog,
  PopoverFilter,
  PopoverFooter,
  PopoverList,
  PopoverTrigger,
} from "./Popover";

/** A row's card: its facts, beside what opened it, with the point it takes from it. */
function RowCard() {
  return (
    <PopoverTrigger>
      <Button>About steward 4</Button>
      <PopoverDialog aria-label="steward 4" className="row-card" arrow>
        <p style={{ margin: 0 }}>Running in alpha, on claude.</p>
        <p style={{ margin: 0 }}>Started 4 minutes ago.</p>
      </PopoverDialog>
    </PopoverTrigger>
  );
}

const meta = {
  component: RowCard,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof RowCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A card from its trigger**: Enter opens it with the keyboard in it, Escape hands it back. */
export const ACard: Story = {
  play: async ({ canvas, userEvent }) => {
    const trigger = canvas.getByRole("button", { name: "About steward 4" });
    await userEvent.tab();
    await expect(trigger).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    const card = await screen.findByRole("dialog", { name: "steward 4" });
    await untilFocusWithin(() => card, "Popover: Enter opens the card with the keyboard in it");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    // The keyboard comes back to the trigger after the popover has gone, a frame or more later.
    await untilFocused(() => trigger, "Popover: Escape gives the keyboard back to its trigger");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("dialog", { name: "steward 4" });
  },
};

const WORKSPACES = ["alpha", "beta", "docs", "ops", "web"];

/**
 * **A list in a popover** (R165): the filter row and the rule under it, the list that scrolls
 * while they stay in view, and the footer that says its keys in mono caps. The keyboard starts
 * in the filter; Down goes into the list, the arrows walk it, Enter goes, Escape closes.
 */
function WorkspacePicker() {
  const [said, setSaid] = useState("Nothing chosen");
  const [typed, setTyped] = useState("");
  const list = useRef<HTMLDivElement>(null);
  const shown = WORKSPACES.filter((one) => one.includes(typed.trim()));
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <PopoverTrigger>
        <Button>All workspaces</Button>
        <PopoverDialog aria-label="Workspaces" layout="list" style={{ inlineSize: "20rem" }}>
          {({ close }: { close: () => void }) => (
            <>
              <PopoverFilter>
                <input
                  className="ui-input"
                  aria-label="Go to a workspace in purlis"
                  placeholder="Go to a workspace in purlis…"
                  value={typed}
                  autoFocus
                  onChange={(event) => setTyped(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key !== "ArrowDown") return;
                    event.preventDefault();
                    list.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
                  }}
                  style={{ inlineSize: "100%" }}
                />
              </PopoverFilter>
              <PopoverList>
                <InlineMenu
                  aria-label="Workspaces in purlis"
                  ref={list}
                  onAction={(key) => {
                    setSaid(`Went to ${String(key)}`);
                    close();
                  }}
                >
                  {shown.map((one) => (
                    <MenuItem key={one} id={one} textValue={one}>
                      {one}
                    </MenuItem>
                  ))}
                </InlineMenu>
              </PopoverList>
              <PopoverFooter />
            </>
          )}
        </PopoverDialog>
      </PopoverTrigger>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

export const AListWithItsFilter: Story = {
  render: () => <WorkspacePicker />,
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await userEvent.keyboard("{Enter}");
    const popover = await screen.findByRole("dialog", { name: "Workspaces" });
    const filter = within(popover).getByRole("textbox", { name: "Go to a workspace in purlis" });
    await untilFocused(() => filter, "Workspaces: the keyboard starts in the filter");
    expectFocusInSight();
    // The anatomy: the filter row, its rule, the list and the footer, in that order.
    const parts = [...popover.querySelectorAll("[data-part]")]
      .map((one) => one.getAttribute("data-part"))
      .filter((one) => ["filter", "rule", "list", "footer"].includes(one ?? ""));
    await expect(parts.slice(0, 4)).toEqual(["filter", "rule", "list", "footer"]);
    // Seen once the popover has arrived (it fades in as it appears).
    await waitFor(() => expect(within(popover).getByText("move")).toBeVisible());
    await userEvent.keyboard("o");
    await expect(
      within(popover)
        .getAllByRole("menuitem")
        .map((one) => one.textContent),
    ).toEqual(["docs", "ops"]);
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(popover).getByRole("menuitem", { name: "docs" }),
      "Workspaces: Down reaches docs",
    );
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    // The arrows stop at the list's end.
    await expect(within(popover).getByRole("menuitem", { name: "ops" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Went to ops"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    // The keyboard is back on the trigger; open again for the picture.
    await untilFocused(
      () => canvas.getByRole("button", { name: "All workspaces" }),
      "Workspaces: the keyboard is back on the trigger",
    );
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("dialog", { name: "Workspaces" });
  },
};

/** **A refusal beside its field**: said under the box it is about, the keyboard left there. */
function Refused() {
  const box = useRef<HTMLInputElement>(null);
  const [typed, setTyped] = useState("web");
  const taken = typed.trim() === "web";
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <input
        ref={box}
        className="ui-input"
        aria-label="Rename chat claude 4"
        aria-invalid={taken}
        aria-describedby={taken ? "refused" : undefined}
        value={typed}
        onChange={(event) => setTyped(event.target.value)}
      />
      <AnchoredPopover triggerRef={box} isOpen={taken} className="row-card">
        <div id="refused" role="alert">
          That name is taken.
        </div>
      </AnchoredPopover>
    </div>
  );
}

export const ARefusalBesideItsField: Story = {
  render: () => <Refused />,
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("textbox", { name: "Rename chat claude 4" });
    await userEvent.tab();
    await expect(box).toHaveFocus();
    await expect(box).toHaveAccessibleDescription("That name is taken.");
    // At the end of the name, as a person fixing it is.
    await userEvent.keyboard("{End}s");
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    await expect(box).toHaveFocus();
    await userEvent.keyboard("{Backspace}");
    await screen.findByRole("alert");
    // It took no keyboard.
    await expect(box).toHaveFocus();
    expectFocusInSight();
  },
};
