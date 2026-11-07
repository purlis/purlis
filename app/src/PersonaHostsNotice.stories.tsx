// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What stays up after an Always allow** (ADR 0112, #5245). Where a teammate's host was already
 * waiting, the person's Always allow approves nothing: the line goes into the committed file, the
 * chat that asked runs on a Run grant of its own, and this Notice stays up for all of the persona's
 * list, the new line and the teammate's host together. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PersonaHosts, SandboxState } from "./bindings";
import { PersonaHostsNotice } from "./PersonaHostsNotice";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

const PLANE = "/Users/you/work/shop";

/** The list as it stands after the click: the teammate's host, and the line Always allow wrote. */
const AFTER_ALWAYS_ALLOW: PersonaHosts = {
  persona: "devops",
  hosts: ["registry.internal.example"],
  reached: ["registry.internal.example"],
  tools: ["curl with TOKEN from vault team as TOKEN env"],
  digest: "d1",
  allowed: false,
  default: false,
  waiting: true,
  cautions: ["curl can send the values it is handed anywhere this chat can reach."],
};

const STATE: SandboxState = {
  on: true,
  offer: false,
  said: null,
  never: [],
  hosts_changed: null,
  presets_changed: null,
  presets: [],
  persona_hosts: [AFTER_ALWAYS_ALLOW],
  besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
  policy: null,
};

/** What the core was asked to allow, for the story to read. */
let allowed: unknown[] = [];

const meta = {
  component: PersonaHostsNotice,
  args: { plane: PLANE, canAllow: true },
  parameters: { screenshot: "page" },
  beforeEach: () => {
    allowed = [];
    mockIPC((cmd, args) => {
      if (cmd === "sandbox_state") return structuredClone(STATE);
      if (cmd === "allow_persona_hosts") allowed.push(args);
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof PersonaHostsNotice>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * A teammate's host and the new line wait as one list, under one Allow, and nothing went through
 * by itself. The keyboard reaches Allow, then Not now, which hides it and allows nothing.
 */
export const ATeammatesHostWaitsBesideTheNewLine: Story = {
  play: async ({ canvasElement }) => {
    const notice = await within(canvasElement).findByRole("status", {
      name: "devops's hosts and tools wait for you",
    });
    await expect(notice).toHaveTextContent("registry.internal.example");
    await expect(notice).toHaveTextContent("curl with TOKEN from vault team as TOKEN env");
    await expect(notice).toHaveTextContent("they have changed since");

    await userEvent.tab();
    await untilFocused(
      () => within(notice).getByRole("button", { name: "Allow for devops chats" }),
      "the keyboard reaches Allow",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => within(notice).getByRole("button", { name: "Not now" }),
      "Tab goes on to Not now",
    );
    expectFocusInSight();
    // Not now hides it, and the picture is of what stays up.
    await pictureNow();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(
        within(canvasElement).queryByRole("status", {
          name: "devops's hosts and tools wait for you",
        }),
      ).toBeNull(),
    );
    await expect(allowed).toEqual([]);
  },
};
