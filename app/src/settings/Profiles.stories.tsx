// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Profiles** (#2034, #6301): this machine's profiles, a table of controls of the table
 * family on the aligned page (#2919, R65, R67): one signed in
 * and pinned, one whose pin can move to the major version installed, and one adopted, each with
 * what it says and its actions; and the page with none yet. Drawn, held to axe, walked by the
 * keyboard and pictured in every pictured theme cell (`docs/component-stories.md`). The core is
 * faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { MachineProfileRow } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { profilesGroup } from "./profiles";
import { OnItsPage } from "./settingsStory.testkit";

function row(name: string, more: Partial<MachineProfileRow> = {}): MachineProfileRow {
  return {
    name,
    kind: "claude",
    harness: "Claude Code",
    origin: "made",
    folder: `/Users/you/Library/Application Support/purlis/profiles/${name}/config`,
    // A fixed day, so the picture says the same on every run.
    imported_at: 1_760_000_000,
    imported_from: "/Users/you/.claude",
    signs_in: "through Claude Code's own sign-in, on this Mac",
    standing: "signed-in",
    plans: [],
    pinned: "2.1.300",
    installed: "2.1.300",
    offered: null,
    sandbox_only: false,
    commands_can_read_sign_in: null,
    icon: { glyph: null, own: null, monogram: "C" },
    ...more,
  };
}

const PROFILES: MachineProfileRow[] = [
  row("work"),
  row("personal", { pinned: "2.1.283", installed: "3.0.0", offered: "3.0.0", standing: "none" }),
  row("codex", {
    kind: "codex",
    harness: "Codex",
    origin: "adopted",
    folder: "/Users/you/.codex",
    imported_at: null,
    signs_in: "through Codex's own sign-in, on this Mac",
    icon: { glyph: null, own: null, monogram: "X" },
  }),
];

/** The core, faked: this machine keeps `profiles`, and no pool. */
function theCore(profiles: MachineProfileRow[]) {
  mockIPC(
    (cmd) => {
      if (cmd === "machine_profiles") return profiles;
      if (cmd === "docker_pools")
        return {
          pools: [],
          defaults: { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 },
          suspend_after: "24h",
          frames: 15,
          width: 1280,
          clusters: [],
        };
      return null;
    },
    { shouldMockEvents: true },
  );
}

function Page() {
  return <OnItsPage group={profilesGroup("/Users/you/projects/charter")} />;
}

const meta = {
  component: Page,
  beforeEach: ({ parameters }) => {
    theCore((parameters.profiles as MachineProfileRow[] | undefined) ?? PROFILES);
    return () => clearMocks();
  },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **Three profiles**: each one's name, harness and folder, how it signs in, its pin, and its
 * actions; Update the pin only on the one a newer major version is installed for.
 */
export const ThreeProfiles: Story = {
  play: async ({ canvas, userEvent }) => {
    const update = await canvas.findByRole("button", {
      name: "Update the pin of personal to 3.0.0",
    });
    await expect(canvas.getAllByRole("button", { name: /^Update the pin of / })).toHaveLength(1);
    // A table of controls (#6301): a row a profile, each of its buttons a Tab stop of its own.
    const table = canvas.getByRole("table", { name: "This machine's profiles" });
    await expect(within(table).getAllByRole("rowheader")).toHaveLength(3);
    await expect(canvas.getByRole("button", { name: "Remove codex" })).toBeVisible();
    update.focus();
    await untilFocused(() => update, "Update the pin");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Import again: personal" }),
      "Tab goes on to the row's next action",
    );
    expectFocusInSight();
  },
};

/** **None yet**: the list says so. Words only. */
export const NoneYet: Story = {
  parameters: { profiles: [] },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Profiles" });
    await within(group).findByText("This machine keeps no profile of its own yet.");
  },
};
