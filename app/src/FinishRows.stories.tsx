// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The turn's finish row in Updates › Mine** (T1-31, R-241, R-178, R-181): a finished turn and
 * a failed chat each a row; "· 1 blocked this turn" where a block was still unresolved at the
 * turn's end; and identical rows of two chats grouped into one. Drawn in every cell of the theme
 * matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { FinishedTurn } from "./bindings";
import { chatLogAnswer, finishUpdates, groupedFinishes, stoppedAfterBlock } from "./finishRows";
import { Inbox } from "./Inbox";
import { listed } from "./inboxUpdates";
import { expectFocusInSight, tabInto } from "./stories.testkit";

const PLANE = "/home/op/project";
const AT = 1_763_031_600;

const NAMES: Record<number, string> = { 3: "steward 3", 5: "reviewer 5" };

const turn = (session: number, key: string, blocked: number, failed = false): FinishedTurn => ({
  session,
  key,
  at: AT - session,
  failed,
  blocked,
  blocks: [],
});

function Mine({
  turns,
  open,
}: {
  turns: readonly FinishedTurn[];
  /** Opens chat `session`'s Chat log, as the window's block result does (R-226). */
  open?: (session: number) => void;
}) {
  const updates = listed(
    finishUpdates(turns, (session) => NAMES[session] ?? String(session)).map((one) => ({
      ...one,
      read: false,
    })),
    [],
  );
  const rows = groupedFinishes(updates).map(({ update }) => ({
    update,
    dismiss: () => undefined,
    answers:
      open !== undefined && stoppedAfterBlock(update) && update.session !== null
        ? [chatLogAnswer(NAMES[update.session] ?? "", () => open(update.session ?? 0))]
        : undefined,
  }));
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        part="updates"
        plane={PLANE}
        asks={[]}
        onGo={() => undefined}
        onLeave={() => undefined}
        updates={{ rows, onMarkAllRead: () => undefined, onDismissAll: () => undefined }}
      />
    </div>
  );
}

const meta = {
  component: Mine,
  args: { turns: [] },
  beforeEach: () => {
    mockIPC(() => null);
    return () => clearMocks();
  },
} satisfies Meta<typeof Mine>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A finished turn and a failed chat show in Updates › Mine** (R-241). */
export const AFinishedTurnAndAFailedChat: Story = {
  args: { turns: [turn(3, "01TURN", 0), turn(5, "01RUN", 0, true)] },
  play: async ({ canvas }) => {
    const turned = await canvas.findByRole("row", { name: "steward 3: Finished its turn" });
    await expect(turned).toHaveAttribute("data-kind", "turn-finished");
    const failed = canvas.getByRole("row", { name: "reviewer 5: Failed" });
    await expect(failed).toHaveAttribute("data-kind", "chat-failed");
    await tabInto(() => turned.closest('[role="grid"]'));
    expectFocusInSight();
  },
};

/** **Identical finish rows group across chats** (R-181): one row names both chats. */
export const IdenticalFinishRowsGroupAcrossChats: Story = {
  args: { turns: [turn(3, "01A", 1), turn(5, "01B", 1), turn(5, "01C", 0)] },
  play: async ({ canvas }) => {
    const grouped = await canvas.findByRole("row", {
      name: "steward 3, reviewer 5: Finished its turn · 1 blocked this turn",
    });
    await expect(canvas.getAllByText(/· 1 blocked this turn/)).toHaveLength(1);
    await expect(canvas.getByRole("row", { name: "reviewer 5: Finished its turn" })).toBeVisible();
    await tabInto(() => grouped.closest('[role="grid"]'));
    expectFocusInSight();
  },
};

/**
 * **A block result opens its chat's Chat log** (R-226, T1-37): a turn that ended after a block
 * offers "Open the chat log", pressed from the keyboard; a turn without one offers nothing.
 */
export const ABlockResultOpensItsChatLog: Story = {
  args: { turns: [turn(3, "01A", 1), turn(5, "01B", 0)], open: fn() },
  play: async ({ canvas, args, userEvent }) => {
    const blocked = await canvas.findByRole("row", {
      name: "steward 3: Finished its turn · 1 blocked this turn",
    });
    const way = canvas.getByRole("button", { name: "Open the chat log of steward 3" });
    await expect(blocked).toContainElement(way);
    await expect(canvas.getAllByRole("button", { name: /^Open the chat log/ })).toHaveLength(1);
    way.focus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.open).toHaveBeenCalledWith(3);
  },
};
