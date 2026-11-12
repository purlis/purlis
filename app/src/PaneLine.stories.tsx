// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The head of a pane showing a task, drawn by the button family** (#3129, #4760), **and its
 * foot** (#5202): at the head's start the task's path; at its end two groups (#5203, spec #5191):
 * the chat's actions, its one state button (Brief, on a task), the bell and ⋯ last, then, after a
 * thin divider, the pane's splits, and no close, as a pane showing a task of its session draws
 * none. Under the terminal, the pane foot says "harness · workspace", the harness its card's
 * button, and the task's gauge. The controls are the window's own (`PaneHead`,
 * `PaneActions`, `BriefButton`, `HarnessChip`); the path stands in as its words, since the real
 * one (`PaneCrumbs`) reads the chats' store, and `PaneCrumbs.test.tsx` holds its shape. No
 * control on the head takes a border in Purlis, and each is reached by Tab with its ring in
 * sight. A task's two ways to end it are rows of its ⋯ since
 * #4760, one of them unavailable here, with its reason.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { catalogued, type Offer } from "./actions";
import { BriefButton, BriefOpener, type BriefAsk } from "./Brief";
import { HarnessChip } from "./HarnessCard";
import { ChatGauge } from "./ChatGauge";
import { PaneActions, PaneActionsOf, PaneBellOf, PaneHead, PaneNumber } from "./LayoutPanes";
import { paneLinesOf } from "./paneHead";
import { PaneFoot, PaneFootEnd, PaneFootSeparator, PaneFootStart } from "./ui/pane-foot/PaneFoot";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

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
      <div className="pane-frame pane-headed" style={{ blockSize: 180 }}>
        <div className="pane-top">
          <BriefOpener value={(of: BriefAsk) => setSaid(`Brief of ${of.name}`)}>
            {/* A pane showing a task of its session draws no close (#1486). */}
            <PaneActionsOf.Provider
              value={paneLinesOf("task", undefined, { closes: false }).actions}
            >
              <PaneHead
                doing={
                  // The bell, with nothing to hold: a chat's pane always draws it (#4754).
                  <PaneBellOf.Provider value={{ notes: 0, trouble: 0, asks: 0 }}>
                    <PaneActions
                      pane={1}
                      // Its one state button: Brief, on a task (`PaneStateButton` draws this one).
                      state={<BriefButton of={{ name: "fix-login", chat: SESSION }} word="Brief" />}
                      chat={{ session: SESSION, offers: LIST }}
                      offerFor={(id) => SPLITS.find((offer) => offer.id === id)}
                      closeOf={() => undefined}
                      onPaneDoes={(_pane, offer) => setSaid(offer?.title ?? "")}
                    />
                  </PaneBellOf.Provider>
                }
              >
                <div className="pane-chips">
                  <span className="pane-crumbs">steward 2 › fix-login</span>
                  <PaneNumber session={SESSION} />
                </div>
              </PaneHead>
            </PaneActionsOf.Provider>
          </BriefOpener>
        </div>
        {/* **The focused pane**, as the window draws it: its own controls are always drawn, and
            in the keyboard's reach (#5318). */}
        <div className="pane-body">
          <div className="pane focused" />
        </div>
        {/* **The pane foot** (#5202, look A): where the task runs, and its gauge. */}
        <PaneFoot className="pane-foot" label="deep: where it runs and its context">
          <PaneFootStart>
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
              <PaneFootSeparator />
              alpha
            </span>
          </PaneFootStart>
          <PaneFootEnd>
            <ChatGauge
              usage={{
                context: { value: 62, tone: "ok" },
                cache: { value: 91, tone: "ok" },
                rebuilds: null,
                turns: [],
                cold: null,
              }}
            />
          </PaneFootEnd>
        </PaneFoot>
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
 * **The task's head and foot**: Tab reaches Brief, the bell, ⋯ and the two splits, in the order
 * they are drawn, then the foot's harness, each with its ring in sight; Enter presses each. ⋯ ends
 * the chat's actions and a thin divider parts them from the splits (#5203). ⋯ lists the task's two
 * ways to end it, the unavailable Close now saying why. The picture is the head and the foot at
 * rest.
 */
export const ATasksLine: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const status = canvas.getByRole("status");
    const harness = canvas.getByRole("button", { name: "What Claude Code can do here" });
    const brief = canvas.getByRole("button", { name: "Brief of fix-login" });
    const bell = canvas.getByRole("button", { name: /^Pane notices:/ });
    const more = canvas.getByRole("button", { name: /^More for / });
    const right = canvas.getByRole("button", { name: "Split right" });
    const down = canvas.getByRole("button", { name: "Split down" });

    await userEvent.tab();
    await expect(brief).toHaveFocus();
    await expect(brief).toHaveTextContent("Brief");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Brief of fix-login"));

    await userEvent.tab();
    await expect(bell).toHaveFocus();
    expectFocusInSight();

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
    // The press closes the list, as a person's does: the story ends on the head, not the menu.
    await waitFor(() => expect(document.querySelector('[role="menu"]')).toBeNull());
    // And the keyboard back on ⋯: the menu hands it back a frame later whenever nothing holds it,
    // which, left to race the blur below, could draw ⋯ focused with its tip in the picture.
    await untilFocused(() => more, "the closed menu hands the keyboard back to ⋯");

    right.focus();
    await expect(right).toHaveFocus();
    // Drawn left to right as Tab takes them: Brief, the bell, ⋯, the divider, then the splits.
    const box = (one: Element) => one.getBoundingClientRect();
    const line = canvasElement.querySelector('.pane-actions [data-scope="separator"]') as Element;
    await expect(line).not.toBeNull();
    await expect(box(brief).right).toBeLessThan(box(bell).left);
    await expect(box(bell).right).toBeLessThan(box(more).left);
    await expect(box(more).right).toBeLessThan(box(line).left);
    await expect(box(line).right).toBeLessThan(box(right).left);
    await expect(box(line).width).toBeGreaterThan(0);
    await expect(box(line).height).toBeLessThan(box(right).height);
    // Centred on the marks it parts, not at the top of their row.
    const middle = (one: Element) => box(one).top + box(one).height / 2;
    await expect(Math.abs(middle(line) - middle(right))).toBeLessThanOrEqual(1);
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Split right"));

    await userEvent.tab();
    await expect(down).toHaveFocus();
    expectFocusInSight();

    // The foot follows the terminal: the harness is the next stop after the head (#5202).
    await userEvent.tab();
    await expect(harness).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(status).toHaveTextContent("Harness card"));

    (document.activeElement as HTMLElement | null)?.blur();
  },
};
