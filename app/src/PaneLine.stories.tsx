// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The buttons on a pane's top line, drawn by the button family** (#3129): a task's Brief, the
 * two ways to end it (one that cannot run, with its reason), and the harness the chat runs. Each
 * is the window's own component in the window's own header (`PaneHead`, `.pane-chips`), so what
 * is drawn is what a pane draws: no control on the line takes a border in Purlis, and each is
 * reached by Tab with its ring in sight.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import type { Offer, TaskEndWay } from "./actions";
import { BriefButton, BriefOpener, type BriefAsk } from "./Brief";
import { HarnessChip } from "./HarnessCard";
import { PaneHead } from "./LayoutPanes";
import { TaskEndContext, TaskEnds } from "./TaskEnd";
import { expectFocusInSight } from "./stories.testkit";

const SESSION = 7;

/** A task's two rows, as the catalogue offers them: Stop, and Close now that cannot run. */
const STOP: Offer = {
  id: `task.stop:${SESSION}`,
  title: "Stop and get its report",
  available: true,
  reason: "",
  does: { verb: "endTask", session: SESSION, way: "report" },
};
const CLOSE: Offer = {
  id: `task.close:${SESSION}`,
  title: "Close now",
  available: false,
  reason: "It is ending already.",
  does: { verb: "endTask", session: SESSION, way: "now" },
};

/** The line of a pane showing a task, with what was last pressed on it said under it. */
function PaneLine() {
  const [said, setSaid] = useState("Nothing pressed");
  const pressed = (what: string) => () => setSaid(what);
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: 640 }}>
      {/* A pane's size is its layout's (`container-type: size`), so the story gives it one. */}
      <div className="pane-frame pane-headed" style={{ blockSize: 96 }}>
        <div className="pane-top">
          <BriefOpener value={(of: BriefAsk) => setSaid(`Brief of ${of.name}`)}>
            <TaskEndContext.Provider
              value={{
                onEnd: (_session: number, way: TaskEndWay) => setSaid(`Ended by ${way}`),
                onConfirm: () => undefined,
              }}
            >
              <PaneHead doing={null}>
                <div className="pane-chips">
                  <span className="pane-crumbs">steward 2 › fix-login</span>
                  <BriefButton of={{ name: "fix-login", chat: SESSION }} />
                  <TaskEnds session={SESSION} stop={STOP} close={CLOSE} />
                  <HarnessChip
                    glance={{
                      name: "claude",
                      title: "Claude Code",
                      label: "What Claude Code can do here",
                      lines: [],
                      cannot_type: null,
                    }}
                    onOpen={pressed("Harness card")}
                  />
                </div>
              </PaneHead>
            </TaskEndContext.Provider>
          </BriefOpener>
        </div>
      </div>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

const meta = {
  component: PaneLine,
} satisfies Meta<typeof PaneLine>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **The task's line**: Tab reaches Brief, Stop, the unavailable Close now (which keeps the focus
 * and says why) and the harness, in that order, each with its ring in sight; Enter presses the
 * ones that can run and nothing presses Close now. The picture is the line at rest.
 */
export const ATasksLine: Story = {
  play: async ({ canvas, userEvent }) => {
    const status = canvas.getByRole("status");
    const brief = canvas.getByRole("button", { name: "Brief of fix-login" });
    const stop = canvas.getByRole("button", { name: "Stop and get its report" });
    const close = canvas.getByRole("button", { name: "Close now" });
    const harness = canvas.getByRole("button", { name: "What Claude Code can do here" });

    await userEvent.tab();
    await expect(brief).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Brief of fix-login"));

    await userEvent.tab();
    await expect(stop).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Ended by report"));

    await userEvent.tab();
    await expect(close).toHaveFocus();
    await expect(close).toHaveAttribute("aria-disabled", "true");
    await expect(close).toHaveAccessibleDescription("It is ending already.");
    await userEvent.keyboard("{Enter}");
    await expect(status).toHaveTextContent("Ended by report");

    await userEvent.tab();
    await expect(harness).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Harness card"));

    (document.activeElement as HTMLElement | null)?.blur();
  },
};
