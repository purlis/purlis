// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Pool setup's Try it and its last screen** (#4483): every state, drawn, held to axe, walked by
 * the keyboard and pictured in every pictured theme cell. The core is faked at its command
 * boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { PoolSetupCheck, PoolSetupStep } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { TryStep } from "./TryStep";

/** A line of Try it, said of what its title names, as the core tags it. */
const check = (verdict: string, title: string, more: string | null = null): PoolSetupCheck => ({
  about: /identity|\bgh\b/.test(title) ? "identity" : /signed in/.test(title) ? "signins" : "chat",
  verdict,
  title,
  more,
});

const READY: PoolSetupStep = {
  id: "try",
  name: "Try it",
  state: "current",
  line: null,
  findings: [],
  next: {
    id: "start-test-chat",
    label: "Start a test chat",
    does: "Starts a real chat on dev-box, and closes it once it has started. Nothing is sent to it.",
    command: null,
    url: null,
  },
  skippable: false,
  see: null,
  checks: [
    check("todo", "A chat starts on dev-box"),
    check("ok", "work is signed in"),
    check(
      "todo",
      "The agent's identity is set",
      "The agent identity step sets it, for gh and git push in these chats.",
    ),
  ],
};

const meta = {
  component: TryStep,
  args: {
    step: READY,
    pool: "dev-box",
    plane: "p",
    changed: () => undefined,
    onGo: () => undefined,
    onOpenChat: () => undefined,
    onAddAnother: () => undefined,
  },
  decorators: [
    (Story) => (
      <div style={{ maxWidth: 720 }} className="pool-setup-page">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof TryStep>;

export default meta;
type Story = StoryObj<typeof meta>;

const faked = (answer: () => unknown) => () => {
  mockIPC((cmd) => (cmd === "pool_setup_try" ? answer() : null));
};

/** Its own button, and what it checks. */
export const Ready: Story = {
  beforeEach: faked(() => new Promise(() => undefined)),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const start = canvas.getByRole("button", { name: "Start a test chat" });
    start.focus();
    await expect(start).toHaveFocus();
    await expectFocusInSight();
  },
};

/** The chat is starting. */
export const Trying: Story = {
  beforeEach: faked(() => new Promise(() => undefined)),
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Start a test chat" }));
    await canvas.findByRole("heading", { name: "Trying it for real" });
    await expect(canvas.getByRole("status")).toHaveTextContent("Starting a chat on dev-box");
    // Cancel stands beside the busy button while the chat starts (#4976).
    const cancel = canvas.getByRole("button", { name: "Cancel" });
    cancel.focus();
    await untilFocused(() => cancel, "Cancel");
    await expectFocusInSight();
  },
};

/** The chat didn't start, with nothing else to fix: what happened, and Try again. */
export const DidntStartWithinTime: Story = {
  beforeEach: faked(() => ({
    failed: {
      why: "The chat on dev-box didn't start within 2 minutes. Its harness may be waiting on a question, so open a chat there yourself and answer it, then try again.",
      step: null,
    },
  })),
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Start a test chat" }));
    const again = await canvas.findByRole("button", { name: "Try again" });
    again.focus();
    await expect(again).toHaveFocus();
    await expectFocusInSight();
  },
};

/** The chat didn't start: what happened, and the one thing to try. */
export const TheChatDidntStart: Story = {
  beforeEach: faked(() => ({
    failed: {
      why: "work isn't signed in on dev-box's runner. Sign it in on the Sign-ins step, then try again.",
      step: "signins",
    },
  })),
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Start a test chat" }));
    // The step's heading takes the keyboard as it changes, and Tab walks on from there.
    const heading = await canvas.findByRole("heading", { name: "Try it didn't pass" });
    await untilFocused(() => heading, "the step's heading");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Go to Sign-ins" }),
      "Go to Sign-ins",
    );
    expectFocusInSight();
    // Try again stands beside it, the next stop of the keyboard.
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Try again" }), "Try again");
    expectFocusInSight();
  },
};

/** The last screen: everything passed. */
export const EverythingPassed: Story = {
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const open = canvas.getByRole("button", { name: "Open a chat on dev-box" });
    open.focus();
    await untilFocused(() => open, "Open a chat");
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    // The next control is Add another pool (#6256).
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Add another pool" })).toHaveFocus();
    await expectFocusInSight();
  },
  args: {
    step: {
      ...READY,
      state: "done",
      next: null,
      checks: [
        check("ok", "A chat started on dev-box"),
        check("ok", "work is signed in"),
        check("ok", "The agent's identity is set: gh through steward"),
      ],
    },
  },
};

/** The last screen with a step skipped: works, except for this. */
export const WorksExceptForThis: Story = {
  args: {
    step: {
      ...READY,
      state: "done",
      next: null,
      checks: [
        check("ok", "A chat started on dev-box"),
        check("ok", "work is signed in"),
        check(
          "skip",
          "gh and git push weren't checked: agent identity was skipped",
          "In these chats, gh and git push say they have no identity.",
        ),
      ],
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const open = canvas.getByRole("button", { name: "Open a chat on dev-box" });
    open.focus();
    await expect(open).toHaveFocus();
    await expectFocusInSight();
  },
};
