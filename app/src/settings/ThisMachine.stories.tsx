// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › This machine** (ST-2, #6301): the recents, one of them gone with its Locate…, the pins
 * with a dormant one, and the approvals, each a table of the table family on the aligned page
 * (#2919, R65, R67), and the update channel. Drawn, held to axe, walked by the keyboard and
 * pictured in every pictured theme cell (`docs/component-stories.md`). The core is faked at its
 * command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
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
 * **Each list a table** (#6301, after Your hosts, #2919 R65, R67): a row a project, its name
 * heading the row, where it is in the quiet words and its status, then Locate… (on the gone one
 * only) and Forget, each in one place on every row. The table is one Tab stop: ↓ moves a row and
 * → walks its cells to its buttons.
 */
export const Lists: Story = {
  play: async ({ canvas, userEvent }) => {
    const recents = await canvas.findByRole("grid", { name: "Recent projects" });
    await expect(
      within(recents)
        .getAllByRole("rowheader")
        .map((one) => one.textContent),
    ).toEqual(["charter", "elasticpress", "old-site"]);
    await expect(within(recents).getByText("gone")).toBeVisible();
    await expect(canvas.getByRole("grid", { name: "Pins" })).toBeVisible();
    await expect(canvas.getByRole("grid", { name: "Approved projects" })).toBeVisible();
    const rows = () => within(recents).getAllByRole("row").slice(1);
    (rows()[0] as HTMLElement).focus();
    await untilFocused(() => rows()[0] as HTMLElement, "the first project's row");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    await untilFocused(() => rows()[2] as HTMLElement, "↓ reaches the gone project's row");
    await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}{ArrowRight}");
    const locate = within(rows()[2] as HTMLElement).getByRole("button", {
      name: "Locate old-site",
    });
    await untilFocused(() => locate, "→ walks the row's cells to its Locate…");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(rows()[2] as HTMLElement).getByRole("button", { name: "Forget old-site" }),
      "→ goes on to its Forget",
    );
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
