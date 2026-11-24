// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pool's runner whose link stays down past a blip** (#7134, ADR 0078 as amended 2026-11-18):
 * the window's Notice that says for how long and why, in the host's own words, that purlis keeps
 * linking, and offers Try now, Check pool and Dismiss. Drawn in every cell of the theme matrix,
 * light and dark among them, each held to axe and walked by the keyboard. The core is faked at
 * its command boundary, with a link down six minutes, as the person's Docker pool was.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { RunnerLinkDown } from "./bindings";
import { RunnerLinkDownNotice } from "./RunnerLinkDownNotice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const WHY =
  "the connector did not connect: Docker on 45.140.203.77 didn't answer through SSH (it said: " +
  "docker info failed: Cannot connect to the Docker daemon). Check that Docker is running " +
  "there, then check again.";

/** What the faked core was asked to try now, by pool. */
let triedNow: string[] = [];

function Window() {
  return (
    <div style={{ inlineSize: 720 }}>
      <RunnerLinkDownNotice plane="/planes/one" onCheckPool={() => undefined} />
    </div>
  );
}

const meta = {
  component: Window,
  beforeEach: () => {
    triedNow = [];
    const down: RunnerLinkDown[] = [
      {
        pool: "docker",
        why: WHY,
        since: Math.floor(Date.now() / 1000) - 6 * 60,
        state: "unreachable",
      },
    ];
    mockIPC((cmd, args) => {
      if (cmd === "runner_links_down") return down;
      if (cmd === "runner_link_now") triedNow.push((args as { pool: string }).pool);
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Window>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Down six minutes**: why, and its three ways out, each reached by Tab; Try now asks the core. */
export const DownSixMinutes: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/Its chats have waited 6 minutes/);
    await expect(canvas.getByText(/purlis keeps linking to it again/)).toBeVisible();
    const tryNow = canvas.getByRole("button", { name: "Try now" });
    const check = canvas.getByRole("button", { name: "Check pool docker" });
    await userEvent.tab();
    await untilFocused(() => tryNow, "Try now");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => check, "Check pool docker");
    expectFocusInSight();
    await userEvent.click(tryNow);
    await expect(triedNow).toEqual(["docker"]);
  },
};
