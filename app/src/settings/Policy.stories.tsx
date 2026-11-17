// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Settings › Project › Policy** (Q308a, instruction 45): the project's template, the families
 * set apart from it, a loosening waiting for Accept, the "would have blocked" count and the
 * enforcement log it opens. Drawn in every cell of the theme matrix. The core is faked at its
 * command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PolicyState } from "../bindings";
import { forgetPolicies } from "../projectPolicy";
import { TWO_LINES, WATCH } from "../projectPolicy.fixture";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { PolicyLogList, PolicyStanding, WouldHaveBlocked } from "./policy";

const PLANE = "/home/op/project";
const IDS = { id: "policy", labelledBy: "policy-label", describedBy: "policy-help" };

const STATE: PolicyState = {
  ...WATCH,
  waiting: [{ family: "guards.merge", accepted: "enforce", committed: "log" }],
  template: "hold",
  template_default: false,
  overrides: [
    { family: "guards.merge", said: "log", runs: "enforce" },
    { family: "network", said: "off", runs: "enforce" },
  ],
  locked: ["dispatch"],
};

function Page() {
  return (
    <div style={{ inlineSize: 720 }}>
      <span id="policy-label">Template and families</span>
      <PolicyStanding plane={PLANE} ids={IDS} canAccept />
      <WouldHaveBlocked plane={PLANE} ids={{ ...IDS, id: "blocked" }} />
      <PolicyLogList plane={PLANE} ids={{ ...IDS, id: "log" }} />
    </div>
  );
}

const meta = {
  component: Page,
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "project_policy_state") return STATE;
      if (cmd === "policy_log") return TWO_LINES;
      return null;
    });
    return () => {
      forgetPolicies();
      clearMocks();
    };
  },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A loosening waits**: the template, two families set apart, a lock, Accept and the log. */
export const LooseningWaits: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText("Would have blocked: 2");
    await canvas.findByRole("button", { name: "Accept" });
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Accept" }), "Tab reaches Accept");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Open the enforcement log" }),
      "Tab reaches Open the enforcement log",
    );
    expectFocusInSight();
    await expect(canvas.getAllByRole("listitem").length).toBeGreaterThan(2);
  },
};
