// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › This machine › Pools, and Add a pool** (#2679; the person's ruling of 2026-10-30): the
 * group with no pool yet, saying what a pool is, with Add a pool…; the group with a pool; and the
 * Add a pool dialog with its Image box showing the runner image it is offered.
 * Each is drawn, held to axe, walked by the keyboard, and pictured in every pictured theme cell
 * (`docs/component-stories.md`). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AddDockerPoolHost } from "../AddDockerPool";
import type { PoolRow, PoolsShown } from "../bindings";
import { startAddingAPool, stopAddingAPool } from "../pools";
import { expectFocusInSight, pictureNow, untilFocused } from "../stories.testkit";
import { SettingGroup, SettingRow } from "./components";
import type { LiveSetting } from "./groups";
import { poolsGroup } from "./pools";

const IMAGE = `ghcr.io/purlis/purlis-runner@sha256:${"4f".repeat(32)}`;
const DEFAULTS = { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 };

const BUILDS: PoolRow = {
  name: "builds",
  image: IMAGE,
  limits: { ...DEFAULTS },
  suspend_after: "24h",
};

/** The core, faked: this machine keeps `pools`, and its release names {@link IMAGE}. */
function theCore(pools: PoolRow[]) {
  mockIPC(
    (cmd) => {
      if (cmd === "docker_pools")
        return {
          pools,
          defaults: { ...DEFAULTS },
          suspend_after: "24h",
          frames: 15,
          width: 1280,
          clusters: [],
        } satisfies PoolsShown;
      if (cmd === "docker_pool_link") return { state: "linked" };
      if (cmd === "docker_pool_version")
        return {
          runner: "0.4.2",
          why: null,
          desktop: "0.4.2",
          follows: true,
          draining: 0,
          named: true,
          waiting: 0,
        };
      if (cmd === "runner_image_of_this_version") return { state: "ours", image: IMAGE };
      if (cmd === "pool_profiles")
        return {
          kept: [
            {
              name: "work",
              kind: "claude",
              harness: "Claude Code",
              standing: "signed-in",
              not_kept: null,
              plans: [],
            },
          ],
          addable: [],
        };
      return [];
    },
    { shouldMockEvents: true },
  );
}

/** The group's one row, as the Settings tab draws a live setting. */
function Row({ setting }: { setting: LiveSetting }) {
  const { control, grouped, error } = setting.useControl();
  return (
    <SettingRow
      setting={setting.id}
      label={setting.label}
      help={setting.help}
      grouped={grouped}
      error={error}
      control={control}
    />
  );
}

/** The Pools group as Settings › You draws it, with the dialog its Add a pool… opens. */
function Pools() {
  const group = poolsGroup();
  return (
    <>
      <SettingGroup label={group.label} help={group.help}>
        {group.settings.map((one) => (
          <Row key={one.id} setting={one as LiveSetting} />
        ))}
      </SettingGroup>
      <AddDockerPoolHost />
    </>
  );
}

const meta = {
  component: Pools,
  // The pools the faked core keeps: a story's `parameters.pools`, none by default.
  beforeEach: ({ parameters }) => {
    theCore((parameters.pools as PoolRow[] | undefined) ?? []);
    return () => {
      stopAddingAPool();
      clearMocks();
    };
  },
} satisfies Meta<typeof Pools>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **No pool yet**: the group says what a pool is, and Tab reaches its Add a pool…. */
export const NoPoolYet: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    await within(group).findByText(/No pool yet\. A pool is a runner on your Docker/);
    const add = within(group).getByRole("button", { name: "Add a pool…" });
    await userEvent.tab();
    await untilFocused(() => add, "Tab reaches Add a pool…");
    expectFocusInSight();
  },
};

/** **A pool kept**: its row, and Add a pool… after the list. */
export const WithAPool: Story = {
  parameters: { pools: [BUILDS] },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    await within(group).findByRole("button", { name: "Remove pool builds" });
    await within(group).findByText("linked");
    await expect(within(group).queryByText(/No pool yet/)).toBeNull();
    await expect(within(group).getByRole("button", { name: "Add a pool…" })).toBeVisible();
  },
};

/** **Add a pool, its Image filled**: Kind first, and the Image box showing the runner image
 *  this version's release names, the default when it is left empty. Pictured open; Esc closes
 *  it. */
export const AddAPool: Story = {
  parameters: { screenshot: "page" },
  play: async ({ userEvent }) => {
    startAddingAPool();
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Add a pool" });
    const image = within(dialog).getByRole("textbox", { name: "Image" });
    await waitFor(() => expect(image).toHaveAttribute("placeholder", IMAGE));
    await expect(image).toHaveAccessibleDescription(/Empty is the default/);
    await expect(within(dialog).getByRole("radio", { name: /Docker/ })).toBeChecked();
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await untilFocused(() => name, "Add a pool: the keyboard starts in Name");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => image, "Tab goes on to Image");
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("dialog", { name: "Add a pool" })).toBeNull());
  },
};
