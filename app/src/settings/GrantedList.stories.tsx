// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Run grants on the Allowed list** (ADR 0112, #5241): each says what it runs, with which keys,
 * for which chat and at which scope, beside the Allows the list already held, and each has a
 * Remove. Remove applies from the next run and doesn't stop a running one. The core is faked at
 * its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SandboxGrant } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { AllowedRows } from "./GrantedList";

const grant = (over: Partial<SandboxGrant> & Pick<SandboxGrant, "id" | "what" | "target">) =>
  ({
    persona: null,
    level: "chat",
    by: null,
    at: null,
    chat: "Deploy",
    locked: null,
    waiting: null,
    for_no_persona: false,
    expected: false,
    ...over,
  }) satisfies SandboxGrant;

const FOR_THE_CHAT = grant({
  id: "chat\u0001run-grant\u00017",
  what: "run-grant",
  target: "/usr/bin/curl with TOKEN from vault team",
});
const ONCE = grant({
  id: "once\u0001run-grant\u00018",
  what: "run-grant",
  target: "/usr/local/bin/stripe with STRIPE_KEY from vault billing",
  level: "once",
  chat: "Refunds",
});
const A_HOST = grant({
  id: "you\u0001host\u0001registry.example.com",
  what: "host",
  target: "registry.example.com",
  level: "you",
  chat: null,
});

/** Which grants the core was told to take back, for the story to read. */
let removed: unknown[] = [];

const meta = {
  component: AllowedRows,
  args: {
    plane: "/Users/you/work/shop",
    file: "purlis.toml",
    ids: { id: "allowed", labelledBy: undefined },
  },
  parameters: { screenshot: "page" },
  beforeEach: () => {
    removed = [];
    let held = [FOR_THE_CHAT, ONCE, A_HOST];
    mockIPC((cmd, args) => {
      if (cmd === "sandbox_grants") return structuredClone(held);
      if (cmd === "revoke_sandbox_grant") {
        const { id } = args as { id: string };
        removed.push(id);
        held = held.filter((one) => one.id !== id);
        return structuredClone(held);
      }
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof AllowedRows>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Both scopes a Run grant has, beside a host: Tab walks their Remove buttons in order. */
export const RunGrantsBesideAHost: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const list = await canvas.findByRole("list", { name: "Allowed" });
    await expect(list).toHaveTextContent(
      "Run /usr/bin/curl with TOKEN from vault team, in the chat's own sandbox · This chat",
    );
    await expect(list).toHaveTextContent(
      "Run /usr/local/bin/stripe with STRIPE_KEY from vault billing, in the chat's own sandbox · This chat, once",
    );

    await userEvent.tab();
    await untilFocused(
      () =>
        canvas.getByRole("button", {
          name: "Remove running /usr/bin/curl with TOKEN from vault team",
        }),
      "the keyboard reaches the first Remove",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () =>
        canvas.getByRole("button", {
          name: "Remove running /usr/local/bin/stripe with STRIPE_KEY from vault billing",
        }),
      "Tab goes on to the second",
    );
    expectFocusInSight();
    await expect(removed).toEqual([]);
  },
};

/** Remove sends the core that grant's id alone, and the list draws what the core says is left. */
export const RemoveTakesARunGrantBack: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("list", { name: "Allowed" });

    await userEvent.tab();
    await untilFocused(
      () =>
        canvas.getByRole("button", {
          name: "Remove running /usr/bin/curl with TOKEN from vault team",
        }),
      "the keyboard reaches Remove",
    );
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(removed).toEqual([FOR_THE_CHAT.id]));
    await waitFor(() =>
      expect(canvas.getByRole("list", { name: "Allowed" })).not.toHaveTextContent("/usr/bin/curl"),
    );
    await expect(canvas.getByRole("list", { name: "Allowed" })).toHaveTextContent(
      "/usr/local/bin/stripe",
    );
  },
};
