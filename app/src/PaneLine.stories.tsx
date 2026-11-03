// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The head of a pane showing a task, drawn by the button family** (#3129, #4760): at its start
 * the task's path and "harness · workspace", the harness its card's button; at its end the one
 * state button (Brief, on a task), the chat's ⋯ and the pane's splits. Each is the window's own
 * component in the window's own header (`PaneHead`, `.pane-chips`, `PaneDoing`), so what is
 * drawn is what a pane draws: no control on the head takes a border in Purlis, and each is
 * reached by Tab with its ring in sight. A task's two ways to end it are rows of its ⋯ since
 * #4760, one of them unavailable here, with its reason.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { catalogued, type Offer } from "./actions";
import { BriefButton, BriefOpener, type BriefAsk } from "./Brief";
import { HarnessChip } from "./HarnessCard";
import { PaneDoing, PaneHead } from "./LayoutPanes";
import { ChatMenuButton } from "./Menus";
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

/** The pane's two splits, as the catalogue offers them. */
const SPLITS: Offer[] = [
  {
    id: "pane.split.right",
    title: "Split right",
    available: true,
    reason: "",
    does: { verb: "split", direction: "row" },
  },
  {
    id: "pane.split.down",
    title: "Split down",
    available: true,
    reason: "",
    does: { verb: "split", direction: "column" },
  },
];

/** The task's list, which its ⋯ opens. */
const LIST = catalogued([STOP, CLOSE]);

/** The head of a pane showing a task, with what was last pressed on it said under it. */
function PaneLine() {
  const [said, setSaid] = useState("Nothing pressed");
  const pressed = (what: string) => () => setSaid(what);
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: 760 }}>
      {/* A pane's size is its layout's (`container-type: size`), so the story gives it one. */}
      <div className="pane-frame pane-headed" style={{ blockSize: 160 }}>
        <div className="pane-top">
          <BriefOpener value={(of: BriefAsk) => setSaid(`Brief of ${of.name}`)}>
            <PaneHead
              doing={
                <div className="pane-actions">
                  <BriefButton of={{ name: "fix-login", chat: SESSION }} word="Brief" />
                  <ChatMenuButton
                    session={SESSION}
                    offers={LIST}
                    onPress={(offer) => setSaid(offer.title)}
                  />
                  <PaneDoing
                    pane={1}
                    offerFor={(id) => SPLITS.find((offer) => offer.id === id)}
                    closeOf={() => undefined}
                    onPaneDoes={(_pane, offer) => setSaid(offer?.title ?? "")}
                    task
                  />
                </div>
              }
            >
              <div className="pane-chips">
                <span className="pane-crumbs">steward 2 › fix-login</span>
                <span className="pane-where">
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
                  {" · alpha"}
                </span>
              </div>
            </PaneHead>
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
 * **The task's head**: Tab reaches the harness, Brief, ⋯ and the two splits, in that order, each
 * with its ring in sight; Enter presses each. ⋯ lists the task's two ways to end it, the
 * unavailable Close now saying why. The picture is the head at rest.
 */
export const ATasksLine: Story = {
  play: async ({ canvas, userEvent }) => {
    const status = canvas.getByRole("status");
    const harness = canvas.getByRole("button", { name: "What Claude Code can do here" });
    const brief = canvas.getByRole("button", { name: "Brief of fix-login" });
    const more = canvas.getByRole("button", { name: /^More for / });
    const right = canvas.getByRole("button", { name: "Split right" });
    const down = canvas.getByRole("button", { name: "Split down" });

    await userEvent.tab();
    await expect(harness).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Harness card"));

    await userEvent.tab();
    await expect(brief).toHaveFocus();
    await expect(brief).toHaveTextContent("Brief");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Brief of fix-login"));

    await userEvent.tab();
    await expect(more).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const menu = await within(document.body).findByRole("menu");
    await expect(within(menu).getByRole("menuitem", { name: /^Close now/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await userEvent.click(within(menu).getByRole("menuitem", { name: /^Stop and get its report/ }));
    await waitFor(() => expect(status).toHaveTextContent("Stop and get its report"));

    right.focus();
    await expect(right).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Split right"));

    await userEvent.tab();
    await expect(down).toHaveFocus();
    expectFocusInSight();

    (document.activeElement as HTMLElement | null)?.blur();
  },
};
