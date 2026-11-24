// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Allowed list of answers given at an ask** (M75-21, spec §8.8): a timed grant with its
 * reason and countdown beside a grant that lasts, each with a Remove. The core is faked at its
 * command boundary. Slice 38 mounts the list in Settings.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { AllowedAnswer } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { AllowedList } from "./GrantedList";

/** Far enough ahead that the story never sees it end, and the same picture every run. */
const FAR = 4_102_444_800;

const TIMED: AllowedAnswer = {
  id: "aaaaaaaaaaaaaaaa",
  scope: "timed",
  rule: "processes-curl",
  action: "exec",
  resource: "curl",
  chat: "01M85WM43DT07ANNAQ6NQEJTXD",
  workspace: null,
  reason: "Run the release build",
  until: FAR,
  left: 2520,
};
const LASTING: AllowedAnswer = {
  id: "bbbbbbbbbbbbbbbb",
  scope: "this workspace",
  rule: "processes-curl",
  action: "exec",
  resource: "ls",
  chat: null,
  workspace: "shop",
  reason: null,
  until: null,
  left: null,
};

/** Which answers the core was told to remove, for the story to read. */
let removed: unknown[] = [];

const meta = {
  component: AllowedList,
  args: { plane: "/Users/you/work/shop", ids: { id: "allowed-at-an-ask", labelledBy: undefined } },
  parameters: { screenshot: "page" },
  beforeEach: () => {
    removed = [];
    let held = [TIMED, LASTING];
    mockIPC((cmd, args) => {
      if (cmd === "allowed_answers") return structuredClone(held);
      if (cmd === "remove_allowed_answer") {
        const { id } = args as { id: string };
        removed.push(id);
        held = held.filter((one) => one.id !== id);
        return null;
      }
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof AllowedList>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A timed grant says it is allowed for a while, with its reason; Tab walks the Remove buttons. */
export const TimedBesideAGrant: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const list = await canvas.findByRole("list", { name: "Allowed at an ask" });
    await expect(list).toHaveTextContent(/Allowed for \d+ min · exec curl · Run the release build/);
    await expect(list).toHaveTextContent("Allowed · exec ls · This workspace");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Remove the timed grant for exec curl" }),
      "the keyboard reaches the timed grant's Remove",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Remove the grant for exec ls" }),
      "Tab goes on to the next",
    );
    await expect(removed).toEqual([]);
  },
};

/** Remove sends the core that answer's id alone, and the list draws what is left. */
export const RemoveTakesATimedGrantBack: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("list", { name: "Allowed at an ask" });
    await userEvent.tab();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(removed).toEqual([TIMED.id]));
    await waitFor(() =>
      expect(canvas.getByRole("list", { name: "Allowed at an ask" })).not.toHaveTextContent(
        "Run the release build",
      ),
    );
  },
};
