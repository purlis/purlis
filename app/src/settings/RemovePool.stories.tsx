// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Remove pool {name}?** (#4678, the person's ruling of 2026-11-02): the one question Pools
 * settings and the Devices view ask before a pool goes. With a project and a workspace naming the
 * pool, it names both, since removing the pool sets them back to this machine; with neither, it
 * says nothing of them. Each is drawn, held to axe, walked by the keyboard, and pictured in every
 * pictured theme cell (`docs/component-stories.md`). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolNamedBy } from "../bindings";
import { expectFocusInSight, pictureNow, untilFocused } from "../stories.testkit";
import { RemovePool } from "./pools";

/** Pool builds's question, until it is answered. */
function Asking() {
  const [open, setOpen] = useState(true);
  return open ? (
    <RemovePool name="builds" onRemove={() => setOpen(false)} onCancel={() => setOpen(false)} />
  ) : null;
}

const meta = {
  component: Asking,
  parameters: { screenshot: "page" },
  // What names the pool, as the faked core answers: a story's `parameters.namedBy`, none by
  // default.
  beforeEach: ({ parameters }) => {
    const named = (parameters.namedBy as PoolNamedBy[] | undefined) ?? [];
    mockIPC((cmd) => (cmd === "pool_named_by" ? structuredClone(named) : null));
    return () => clearMocks();
  },
} satisfies Meta<typeof Asking>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The keyboard starts on Cancel, and Tab goes on to Remove pool. */
async function walked(
  alert: HTMLElement,
  userEvent: { tab: (options?: { shift?: boolean }) => Promise<void> },
) {
  await untilFocused(
    () => within(alert).getByRole("button", { name: "Cancel" }),
    "Remove pool: the keyboard starts on Cancel",
  );
  expectFocusInSight();
  await userEvent.tab();
  await untilFocused(
    () => within(alert).getByRole("button", { name: "Remove pool" }),
    "Tab goes on to Remove pool",
  );
  expectFocusInSight();
  await userEvent.tab({ shift: true });
  await untilFocused(
    () => within(alert).getByRole("button", { name: "Cancel" }),
    "Shift+Tab goes back to Cancel",
  );
}

/** Esc answers Cancel, and the question goes. */
async function dismissed(userEvent: { keyboard: (keys: string) => Promise<void> }) {
  await userEvent.keyboard("{Escape}");
  await waitFor(() =>
    expect(
      within(document.body).queryByRole("alertdialog", { name: "Remove pool builds?" }),
    ).toBeNull(),
  );
}

/** **A project and a workspace name the pool**: the question names both before anything goes. */
export const NamesWhatGoesBack: Story = {
  parameters: {
    namedBy: [
      { path: "/home/op/shop", name: "shop", project: true, workspaces: [] },
      { path: "/home/op/docs", name: "docs", project: false, workspaces: ["site"] },
    ] satisfies PoolNamedBy[],
  },
  play: async ({ userEvent }) => {
    const page = within(document.body);
    const alert = await page.findByRole("alertdialog", { name: "Remove pool builds?" });
    const list = await within(alert).findByRole("list", {
      name: "These go back to this machine:",
    });
    await expect(
      within(list)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual(["Project shop, and the workspaces that follow it", "Workspace site in docs"]);
    await expect(alert).toHaveAccessibleDescription(/Workspace site in docs/);
    await walked(alert, userEvent);
    await pictureNow();
    await dismissed(userEvent);
  },
};

/** **Nothing names the pool**: the question says nothing of workspaces or projects. */
export const NothingNamesIt: Story = {
  play: async ({ userEvent }) => {
    const page = within(document.body);
    const alert = await page.findByRole("alertdialog", { name: "Remove pool builds?" });
    await waitFor(() =>
      expect(within(alert).getByRole("button", { name: "Remove pool" })).toBeEnabled(),
    );
    await expect(within(alert).queryByText(/go back to this machine/)).toBeNull();
    await expect(within(alert).queryByRole("list")).toBeNull();
    await walked(alert, userEvent);
    await pictureNow();
    await dismissed(userEvent);
  },
};
