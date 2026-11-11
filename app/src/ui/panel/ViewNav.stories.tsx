// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The page navigation, with groups** (the `view-nav` family, #3666; groups since #2919, R41–R46):
 * a page that heads pages of its own ends in its toggle, apart from its own button; a folded
 * group's pages are not drawn and its page carries their count. Drawn, held to axe, walked by
 * the keyboard and pictured in every pictured theme cell (`docs/component-stories.md`).
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { ViewNav, type ViewPage } from "./ViewNav";

/** Settings' pages, as the prototype draws them: Sandbox heads four of its own. */
function Grouped() {
  const [page, setPage] = useState("general");
  const [open, setOpen] = useState(true);
  const pages: ViewPage[] = [
    { id: "general", label: "General" },
    { id: "saving", label: "Saving" },
    {
      id: "sandbox",
      label: "Sandbox",
      expanded: open,
      count: open ? undefined : 18,
      waiting: true,
    },
    ...(open
      ? [
          { id: "hosts", label: "Your hosts", sub: true, count: 18, waiting: true },
          { id: "level", label: "Your level", sub: true },
          { id: "network", label: "Network", sub: true },
        ]
      : []),
    { id: "dispatch", label: "Dispatch" },
  ];
  return (
    <div style={{ inlineSize: 236 }}>
      <ViewNav
        label="Settings pages"
        pages={pages}
        page={page}
        onPage={setPage}
        onExpand={(_, to) => setOpen(to)}
      />
    </div>
  );
}

const meta = { component: Grouped } satisfies Meta<typeof Grouped>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **A group, open, then folded**: ↓ walks the pages, → reaches Sandbox's toggle, Enter folds it
 * and its count moves onto Sandbox, ← goes back to the page, which was never chosen.
 */
export const GroupFolded: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const general = canvas.getByRole("button", { name: "General" });
    general.focus();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    const sandbox = canvas.getByRole("button", { name: "Sandbox" });
    await untilFocused(() => sandbox, "Sandbox's page");
    await userEvent.keyboard("{ArrowRight}");
    const toggle = canvas.getByRole("button", { name: "Collapse Sandbox" });
    await untilFocused(() => toggle, "Sandbox's toggle");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("button", { name: "Expand Sandbox" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    await expect(canvas.queryByRole("button", { name: /^Your hosts/ })).toBeNull();
    await expect(canvas.getByRole("button", { name: /^Sandbox 18/ })).toBeVisible();
    await userEvent.keyboard("{ArrowLeft}");
    await untilFocused(() => canvas.getByRole("button", { name: /^Sandbox/ }), "back on Sandbox");
    await expect(general).toHaveAttribute("aria-current", "page");
    expectFocusInSight();
  },
};
