// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Phase 1a's Asks** (T1a-61; T1a-27, T1a-39 and T1a-53 add theirs here). Your keys locked at a
 * start (R-510, R-592): W10 with *Unlock*, in place in New chat while the person is there, and in
 * Needs you, its chat's tab marked, when the chat is unattended. While the OS's unlock runs,
 * *Unlock* is busy; a cancel leaves W10 as it was; a failure puts W84 under it.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown, Unlock } from "./bindings";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { Inbox } from "./Inbox";
import { QuestionsHere } from "./QuestionsHere";
import { untilSettled } from "./stories.testkit";
import { panelRoot } from "./ui/panel/Panel";
import { UnlockAsk } from "./UnlockAsk";
import { windowCopy } from "./windowCopy";

/** Your keys' entry the start waits on. */
const ENTRY = "01JA0000000000000000000000";

/** How the OS's unlock answers in a story: as said, or never while the story is looked at. */
type Answer = Unlock | "never";

/** **W10 in place in New chat**, the OS's unlock answering `answer`. */
function InNewChat({ answer }: { answer: Answer }) {
  return (
    <div style={{ inlineSize: 480 }}>
      <UnlockAsk
        entry={ENTRY}
        unlock={() =>
          answer === "never"
            ? new Promise<Unlock>(() => {})
            : Promise.resolve<Unlock>(answer)
        }
        onUnlocked={() => {}}
      />
    </div>
  );
}

/** The unattended chat whose start the keychain's lock stopped. */
const CHAT: ListedChat = {
  session: 7,
  name: "unattended",
  persona: "steward",
  workspace: "smart-ide",
  shell: false,
  parent: null,
  mode: null,
  from: null,
  tab: true,
  branch: null,
  report: null,
  outcome: null,
  asking: null,
  harness: "Claude Code",
};

/** Its Ask, as the app raises it (`asking::model_key_locked`). */
const LOCKED: Shown = {
  session: 7,
  ask: `model-key-locked:${ENTRY}`,
  says: windowCopy.W10,
  options: [{ id: "unlock", label: "Unlock", allows: false }],
  source: "model-key-locked",
  chain: ["steward 7"],
  answer: { via: "unlock-your-keys", entry: ENTRY },
};

/** The board: the chat waits on the person, so it is in Needs you. */
const STATES: ChatStates = { ...nothingKnown, bySession: { 7: "waiting" }, needsYou: [7] };

/** **W10 in Needs you**, beside the chat's row, whose mark says it needs the person. */
function InNeedsYou() {
  return (
    <ChatsHere.Provider value={fixedChats(STATES)}>
      <QuestionsHere.Provider value={{ asks: [LOCKED], with_asking_chat: [] }}>
        <div style={{ display: "flex" }}>
          <div
            {...panelRoot("side")}
            aria-label="Navigation"
            role="region"
            style={{ inlineSize: 240, blockSize: 200 }}
          >
            <ChatsSection rows={chatsTree([CHAT])} front={7} onOpen={() => {}} />
          </div>
          <div style={{ inlineSize: 420 }}>
            <Inbox
              part="asks"
              plane="/plane"
              asks={[LOCKED]}
              onGo={() => {}}
              onLeave={() => {}}
            />
          </div>
        </div>
      </QuestionsHere.Provider>
    </ChatsHere.Provider>
  );
}

function Stage({ place, answer }: { place: "new-chat" | "needs-you"; answer: Answer }) {
  return place === "new-chat" ? <InNewChat answer={answer} /> : <InNeedsYou />;
}

const meta = { component: Stage } satisfies Meta<typeof Stage>;

export default meta;
type Story = StoryObj<typeof meta>;

const unlockOf = (canvasElement: HTMLElement) =>
  within(canvasElement).getByRole("button", { name: "Unlock" });

/** **Attended**: W10 stands in place in New chat, with Unlock. */
export const UnlockAskInNewChat: Story = {
  args: { place: "new-chat", answer: "never" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(windowCopy.W10)).toBeVisible();
    await expect(unlockOf(canvasElement)).toBeVisible();
    await untilSettled(canvasElement);
  },
};

/** **Unattended**: W10 in Needs you, with Unlock on its row, and the chat's tab marked. */
export const UnlockAskInNeedsYouUnattended: Story = {
  args: { place: "needs-you", answer: "never" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(windowCopy.W10)).toBeVisible();
    await expect(unlockOf(canvasElement)).toBeVisible();
    const mark = canvasElement.querySelector('[data-scope="chat-mark"][data-part="root"]');
    await expect(mark).toHaveAttribute("data-variant", "needs");
    await untilSettled(canvasElement);
  },
};

/** **Unlocking**: Unlock is busy while the OS's unlock runs. */
export const UnlockAskBusy: Story = {
  args: { place: "new-chat", answer: "never" },
  play: async ({ canvasElement }) => {
    await userEvent.click(unlockOf(canvasElement));
    await waitFor(() => expect(unlockOf(canvasElement)).toHaveAttribute("aria-disabled", "true"));
    await untilSettled(canvasElement);
  },
};

/** **Cancelled**: W10 as it was, and Unlock ready again. */
export const UnlockAskCancelled: Story = {
  args: { place: "new-chat", answer: "cancelled" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(unlockOf(canvasElement));
    await waitFor(() =>
      expect(unlockOf(canvasElement)).not.toHaveAttribute("aria-disabled", "true"),
    );
    await expect(canvas.getByText(windowCopy.W10)).toBeVisible();
    await expect(canvas.queryByText(windowCopy.W84)).toBeNull();
    await untilSettled(canvasElement);
  },
};

/** **Failed**: W84 under W10, and Unlock tries again. */
export const UnlockAskFailed: Story = {
  args: { place: "new-chat", answer: "failed" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(unlockOf(canvasElement));
    await waitFor(() => expect(canvas.getByText(windowCopy.W84)).toBeVisible());
    await expect(canvas.getByText(windowCopy.W10)).toBeVisible();
    await untilSettled(canvasElement);
  },
};
