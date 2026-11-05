// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The pane foot** (#5202, spec #5191, look A): the quiet line under a chat's terminal, on the
 * terminal's fill, as a chat's pane draws it. Harness · workspace at its start; `ctx`, its fill a
 * thin bar under its number in the core's tone, then `cache` at its end. In every cell of the
 * theme matrix, the compact ones included, where the foot is the compact tab strip's height.
 * Nothing on it is pressed by the family itself: the harness's button is a screen's
 * (`PaneLine.stories.tsx` walks it).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import type { ReactNode } from "react";
import {
  PaneFoot,
  PaneFootEnd,
  PaneFootGauge,
  PaneFootSeparator,
  PaneFootStart,
  type PaneFootTone,
} from "./PaneFoot";

/** A chat's pane as the window lays it out: the terminal's fill, a body, and the foot. */
function Pane({ children, width = 640 }: { children: ReactNode; width?: number }) {
  return (
    // A pane's size is its layout's (`container-type: size`), so the story gives it one.
    <div className="pane-frame pane-headed" style={{ inlineSize: width, blockSize: 96 }}>
      <div className="pane-body" />
      {children}
    </div>
  );
}

/** A foot with harness · workspace and a gauge whose `ctx` is `ctx`% full in `tone`. */
function Foot({
  ctx,
  tone,
  cacheTone = "ok",
}: {
  ctx: number | null;
  tone: PaneFootTone;
  cacheTone?: PaneFootTone;
}) {
  return (
    <Pane>
      <PaneFoot>
        <PaneFootStart>
          <span>Claude Code</span>
          <PaneFootSeparator />
          <span>smart-ide</span>
        </PaneFootStart>
        <PaneFootEnd>
          {ctx !== null && (
            <>
              <PaneFootGauge label="ctx" value={`${ctx}%`} fill={{ percent: ctx, tone }} />
              <PaneFootGauge
                label="cache"
                value={cacheTone === "ok" ? "91%" : "12%"}
                tone={cacheTone}
              />
            </>
          )}
        </PaneFootEnd>
      </PaneFoot>
    </Pane>
  );
}

const meta = {
  component: Foot,
  args: { ctx: 62, tone: "ok" },
} satisfies Meta<typeof Foot>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A chat's foot**: where it runs, and its gauge, the bar under `ctx` in the ok tone. */
export const AChatsFoot: Story = {
  play: async ({ canvasElement }) => {
    const foot = canvasElement.querySelector('[data-scope="pane-foot"][data-part="root"]');
    await expect(foot?.textContent?.replace(/\s+/g, " ")).toContain("ctx 62%");
    // The bar is a mark, never what a reader is told.
    const track = canvasElement.querySelector('[data-scope="pane-foot"][data-part="track"]');
    await expect(track).toHaveAttribute("aria-hidden", "true");
  },
};

/** **Nearly full**: the bar in the waiting tone, its number saying the value too. */
export const NearlyFull: Story = { args: { ctx: 87, tone: "warn" } };

/** **Full**: the bar in the failed tone. */
export const Full: Story = { args: { ctx: 96, tone: "bad" } };

/** **A cold cache**: its number says the warning itself, in each tone, as it has no bar. */
export const ACacheLow: Story = { args: { cacheTone: "warn" } };
export const ACacheVeryLow: Story = { args: { cacheTone: "bad" } };

/** **Before the first usage**: the foot is there with no gauge, so the terminal never moves for
 *  it when one arrives. */
export const BeforeAnyUsage: Story = { args: { ctx: null } };
