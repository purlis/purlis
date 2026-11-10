// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Asks when nothing waits** (ADR 0121 decision 23, #6020): "Nothing waits on you · 2 chats
 * working", each part a link to the chats. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { Inbox } from "./Inbox";
import type { AllChatsFilter } from "./InboxAsks";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

function NothingWaits({ onAllChats }: { onAllChats: (filter: AllChatsFilter) => void }) {
  return (
    <ChatsHere.Provider
      value={fixedChats({ ...nothingKnown, bySession: { 1: "running", 2: "running", 3: "done" } })}
    >
      <div style={{ inlineSize: 320 }}>
        <Inbox
          part="asks"
          plane="/plane"
          asks={[]}
          onGo={() => {}}
          onLeave={() => {}}
          onAllChats={onAllChats}
        />
      </div>
    </ChatsHere.Provider>
  );
}

const meta = {
  component: NothingWaits,
  args: { onAllChats: fn() },
} satisfies Meta<typeof NothingWaits>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Each part is a link**: Tab reaches each, and Enter opens the chats it names. */
export const TwoWorking: Story = {
  play: async ({ canvas, args, userEvent }) => {
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Nothing waits on you" }),
      "Tab to the first part",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onAllChats).toHaveBeenLastCalledWith("all");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "2 chats working" }),
      "Tab to the chats working",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onAllChats).toHaveBeenLastCalledWith("working");
  },
};
