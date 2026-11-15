// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Quiet at the top of Asks** (#6017): the "Quiet now" toggle, and the summary of a Quiet that
 * ended, "While you were quiet: 2 asks, 1 finished, 1 failed", each part a link, with Dismiss.
 * Drawn in every cell of the theme matrix. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { InboxUpdate, QuietNow, Shown } from "./bindings";
import { finishUpdates } from "./finishRows";
import { AsksQuiet } from "./QuietSummary";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/home/op/project";

const ask = (session: number, key: string): Shown => ({
  session,
  ask: key,
  says: key,
  options: [],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "in-its-pane" },
});

const update = (key: string, kind: InboxUpdate["kind"]): InboxUpdate => ({
  key,
  kind,
  at: 1_500,
  session: 3,
  chain: ["steward 3"],
  says: key,
  read: false,
});

/** A turn of chat 3's that ended after a block still unresolved (T1-31), as its row is noted. */
const STOPPED_AFTER_A_BLOCK: InboxUpdate = {
  ...finishUpdates(
    [{ session: 3, key: "01TURN", at: 1_500, failed: false, blocked: 1, blocks: [] }],
    () => "steward 3",
  )[0],
  read: false,
};

const ENDED: QuietNow = {
  quiet: false,
  by_hand: false,
  ended: {
    from: 1_000,
    to: 2_000,
    asks: [
      { plane: PLANE, key: "p1" },
      { plane: PLANE, key: "p2" },
    ],
  },
};

function Asks({ more = [] }: { more?: readonly InboxUpdate[] }) {
  return (
    <div style={{ inlineSize: 360 }}>
      <AsksQuiet
        plane={PLANE}
        asks={[ask(3, "p1"), ask(4, "p2")]}
        updates={[update("done", "task-done"), update("failed", "task-failed"), ...more]}
        isNew={() => true}
        onGoToAsk={() => undefined}
        onShowUpdates={() => undefined}
      />
    </div>
  );
}

const meta = {
  component: Asks,
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "quiet_now" ? ENDED : null));
    return () => clearMocks();
  },
} satisfies Meta<typeof Asks>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **After a Quiet**: Quiet now not pressed, the summary's parts each a link, then Dismiss. */
export const AfterQuiet: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/While you were quiet/);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Quiet now" }),
      "Tab reaches Quiet now",
    );
    expectFocusInSight();
    await expect(canvas.getByRole("button", { name: "Quiet now" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "2 asks: go to them" }),
      "Tab reaches the asks",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "1 finished: open Updates" }),
      "Tab reaches what finished",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "1 failed: open Updates" }),
      "Tab reaches what failed",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Dismiss" }),
      "Tab reaches Dismiss",
    );
    expectFocusInSight();
  },
};

/** **Quiet now, turned on** from the keyboard. */
export const QuietNowOn: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/While you were quiet/);
    await userEvent.tab();
    const turn = canvas.getByRole("button", { name: "Quiet now" });
    await untilFocused(() => turn, "Tab reaches Quiet now");
    await userEvent.keyboard(" ");
    await expect(turn).toHaveAttribute("aria-pressed", "true");
    expectFocusInSight();
    // Off again: the window holds one Quiet, and the next story starts with it off.
    await userEvent.keyboard(" ");
    await expect(turn).toHaveAttribute("aria-pressed", "false");
  },
};

/** **Quiet summary has a stopped after a block bucket** (T1-31, R-182): its count, a link to
 *  Updates, reached by Tab after what failed. */
export const QuietSummaryHasAStoppedAfterABlockBucket: Story = {
  args: { more: [STOPPED_AFTER_A_BLOCK] },
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/While you were quiet/);
    for (const name of ["Quiet now", "2 asks: go to them", "1 finished: open Updates"]) {
      await userEvent.tab();
      await untilFocused(() => canvas.getByRole("button", { name }), `Tab reaches ${name}`);
    }
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "1 failed: open Updates" }),
      "Tab reaches what failed",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "1 stopped after a block: open Updates" }),
      "Tab reaches what stopped after a block",
    );
    expectFocusInSight();
  },
};
