// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › This machine** (ST-2, #6301): the recents, one of them gone with its Locate…, the pins
 * with a dormant one, and the approvals, on the aligned page, and the update channel. Drawn, held to axe, walked by the keyboard and
 * pictured in every pictured theme cell (`docs/component-stories.md`). The core is faked at its
 * command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { MachineProject, ThisMachine } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { OnItsPage } from "./settingsStory.testkit";
import { thisMachineGroup } from "./thisMachine";

function project(path: string, more: Partial<MachineProject> = {}): MachineProject {
  return {
    path,
    name: path.split("/").pop() ?? path,
    gone: null,
    approved: false,
    pinned: false,
    workspace_pins: [],
    ...more,
  };
}

const PROJECTS: MachineProject[] = [
  project("/Users/you/projects/charter", {
    approved: true,
    pinned: true,
    workspace_pins: [
      { name: "smart-ide", gone: false },
      { name: "issues", gone: true },
    ],
  }),
  project("/Users/you/projects/elasticpress", { approved: true }),
  project("/Volumes/usb/old-site", { gone: "is not there any more" }),
];

/** The core, faked: this machine's store holds `projects`, on the stable channel. */
function theCore(projects: MachineProject[]) {
  mockIPC(
    (cmd) => {
      if (cmd === "this_machine")
        return { projects, dropped: [], forgetful: null } satisfies ThisMachine;
      if (cmd === "update_channel") return "stable";
      return null;
    },
    { shouldMockEvents: true },
  );
}

function Page() {
  return <OnItsPage group={thisMachineGroup()} />;
}

const meta = {
  component: Page,
  beforeEach: ({ parameters }) => {
    theCore((parameters.projects as MachineProject[] | undefined) ?? PROJECTS);
    return () => clearMocks();
  },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **Three lists**: the recents, one of them gone with Locate… before its Forget; the pins, a
 * dormant one among them; and the approvals.
 */
export const Lists: Story = {
  play: async ({ canvas }) => {
    const locate = await canvas.findByRole("button", { name: "Locate old-site" });
    await expect(canvas.getByRole("button", { name: "Forget old-site" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Forget issues in charter" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Revoke elasticpress" })).toBeVisible();
    locate.focus();
    await untilFocused(() => locate, "Locate…");
    expectFocusInSight();
  },
};

/** **Nothing kept**: each list says so in its row, and there is no table. Words only. */
export const NothingKept: Story = {
  parameters: { projects: [] },
  play: async ({ canvas }) => {
    await canvas.findByText("No recent projects.");
    await expect(canvas.getByText("Nothing is pinned.")).toBeVisible();
    await expect(canvas.queryByRole("grid")).toBeNull();
  },
};
