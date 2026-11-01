// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pool's limits page** (#4285, variant B): every state a person meets it in, drawn, held to
 * axe, walked by the keyboard and pictured in every pictured theme cell
 * (`docs/component-stories.md`). The core is faked at its command boundary, and holds more limits
 * than the real list does yet, since the page draws whatever it is handed.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolLimitRow, PoolLimitsPage } from "../bindings";
import { askLimits } from "../pools";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { PoolLimits } from "./PoolLimits";

const row = (
  key: string,
  plain: string,
  unit: string,
  by: number,
  over: Partial<PoolLimitRow> = {},
): PoolLimitRow => ({
  key,
  plain,
  unit,
  default: by,
  default_said: `${by} ${unit}`,
  set: null,
  applies: null,
  ...over,
});

const NEXT = "Applies from the next attempt.";

/** Every group, with the ways a row can differ. */
const LIMITS = (
  set: Readonly<Record<string, number>>,
  waiting: readonly string[] = [],
): PoolLimitsPage => {
  const held = (one: PoolLimitRow) => ({ ...one, set: set[one.key] ?? null });
  return {
    groups: [
      {
        label: "Code",
        help: "What purlis sends to the runner.",
        applies: NEXT,
        limits: [row("codeSize", "Largest code sent to a runner", "MiB", 1024)].map(held),
      },
      {
        label: "Sign-in",
        help: "Profiles going to the runner and signing in there.",
        applies: NEXT,
        limits: [
          row("profileSize", "Most a profile may send: size", "MiB", 128),
          row("profileFiles", "Most a profile may send: files", "files", 8192),
        ].map(held),
      },
      {
        label: "Runner",
        help: "What the runner is made with.",
        applies: "Applies when this runner is next replaced.",
        limits: [row("memory", "Memory", "MiB", 4096)].map(held),
      },
      {
        // Its rows differ, so each says when it applies.
        label: "Mixed",
        help: "A group whose limits apply at different times.",
        applies: null,
        limits: [
          row("soon", "One that applies soon", "s", 60, { applies: NEXT }),
          row("later", "One that applies later", "MiB", 512, {
            applies: "Applies when this runner is next replaced.",
          }),
        ].map(held),
      },
    ],
    advanced: [
      row("answerWait", "Longest wait for a runner's answer", "s", 30, { applies: NEXT }),
    ].map(held),
    waiting: [...waiting],
  };
};

/** The core, faked: it holds `set`, and refuses what the real check does. */
function theCore(
  set: Record<string, number>,
  unread?: string,
  refuseSave?: string,
  waiting?: readonly string[],
  refuseReplace?: string,
) {
  mockIPC((cmd, args) => {
    const asked = (args ?? {}) as { key: string; typed: string; value: number | null };
    if (cmd === "pool_limits") {
      if (unread === "never") return new Promise(() => undefined);
      if (unread !== undefined) throw unread;
      return LIMITS(set, waiting);
    }
    if (cmd === "docker_pool_replace_runner" && refuseReplace !== undefined) throw refuseReplace;
    if (cmd === "pool_limit_check") {
      if (!/^\d+$/.test(asked.typed.trim())) throw "This limit is a whole number";
      if (Number(asked.typed) > 1024) throw "Most a profile may send: size is at most 1024 MiB";
      return Number(asked.typed);
    }
    if (cmd === "pool_limit_set") {
      if (refuseSave !== undefined) throw refuseSave;
      if (asked.value === null) Reflect.deleteProperty(set, asked.key);
      else set[asked.key] = asked.value;
      return null;
    }
    return null;
  });
}

const meta = {
  component: PoolLimits,
  args: { pool: "builds" },
  beforeEach: async ({ parameters }) => {
    theCore(
      { ...(parameters.set as Record<string, number> | undefined) },
      parameters.unread as string | undefined,
      parameters.refuseSave as string | undefined,
      parameters.waiting as readonly string[] | undefined,
      parameters.refuseReplace as string | undefined,
    );
    // The window's limits are held once for every story drawn in this page.
    if (parameters.unread !== "never") await askLimits("builds");
    return () => clearMocks();
  },
  decorators: [
    (Story) => (
      <div style={{ inlineSize: 720 }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PoolLimits>;

export default meta;
type Story = StoryObj<typeof meta>;

const open = async (canvas: Parameters<NonNullable<Story["play"]>>[0]["canvas"]) => {
  const trigger = await canvas.findByRole("button", { name: "Limits of pool builds" });
  trigger.click();
  await canvas.findByRole("region", { name: "Sign-in" });
};

/** **Folded, with values set**: the row says how many are the person's. */
export const Folded: Story = {
  parameters: { set: { profileSize: 256, memory: 8192 } },
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText("2 set by you");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Limits of pool builds" }),
      "Limits' trigger",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await canvas.findByRole("region", { name: "Sign-in" });
    await userEvent.keyboard("{Enter}");
  },
};

/** **Every limit left alone**: empty boxes showing their defaults, nothing to reset. */
export const AllDefaults: Story = {
  play: async ({ canvas, userEvent }) => {
    await open(canvas);
    const box = canvas.getByRole("textbox", { name: "Most a profile may send: size MiB" });
    await expect(box).toHaveValue("");
    await expect(box).toHaveAttribute("placeholder", "128");
    await expect(box).toHaveAccessibleDescription(/Default 128 MiB/);
    await expect(canvas.queryByText("You set this")).toBeNull();
    box.focus();
    await untilFocused(() => box, "a limit's box");
    expectFocusInSight();
    await userEvent.tab();
    expectFocusInSight();
  },
};

/** **Values the person set**: marked, with Reset on, and Reset returns one to the default. */
export const ValuesSet: Story = {
  parameters: { set: { profileSize: 256, memory: 8192 } },
  play: async ({ canvas, userEvent }) => {
    await open(canvas);
    const box = canvas.getByRole("textbox", { name: "Largest code sent to a runner MiB" });
    await expect(box).toHaveValue("");
    await expect(canvas.getAllByText("You set this")).toHaveLength(2);
    const size = canvas.getByRole("textbox", { name: "Most a profile may send: size MiB" });
    await expect(size).toHaveValue("256");
    size.focus();
    await untilFocused(() => size, "the set limit's box");
    await userEvent.tab();
    const reset = canvas.getByRole("button", {
      name: "Reset to 128 MiB: Most a profile may send: size on pool builds",
    });
    await untilFocused(() => reset, "Tab goes on to its Reset");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(size).toHaveValue(""));
    await expect(canvas.getAllByText("You set this")).toHaveLength(1);
    await untilFocused(() => size, "Reset hands the keyboard back to its box");
  },
};

/** **A save the core refused**: its words under the box, which keeps what was typed. */
export const SaveRefused: Story = {
  parameters: { refuseSave: "the limit was not saved: this machine's pools could not be read." },
  play: async ({ canvas, userEvent }) => {
    await open(canvas);
    const box = canvas.getByRole("textbox", { name: "Most a profile may send: size MiB" });
    await userEvent.type(box, "256{Enter}");
    await canvas.findByText(/the limit was not saved/);
    await expect(box).toHaveValue("256");
  },
};

/** **Still reading**: a pool of its own, whose core has not answered. */
export const Reading: Story = {
  args: { pool: "still-reading" },
  parameters: { unread: "never" },
  play: async ({ canvas }) => {
    (await canvas.findByRole("button", { name: "Limits of pool still-reading" })).click();
    await canvas.findByText("Reading…");
  },
};

/** **A bad value as it is typed**: why, in the core's words, with the box marked and unsaved. */
export const RefusedAsTyped: Story = {
  play: async ({ canvas, userEvent }) => {
    await open(canvas);
    const box = canvas.getByRole("textbox", { name: "Most a profile may send: size MiB" });
    await userEvent.type(box, "5000");
    await canvas.findByText(/is at most 1024 MiB/);
    await expect(box).toHaveAttribute("aria-invalid", "true");
    expectFocusInSight();
  },
};

/** **Advanced, open**: the uncommon limits, each saying when it applies. */
export const AdvancedOpen: Story = {
  play: async ({ canvas, userEvent }) => {
    await open(canvas);
    await userEvent.click(canvas.getByRole("button", { name: "Advanced" }));
    const box = await canvas.findByRole("textbox", {
      name: "Longest wait for a runner's answer s",
    });
    await waitFor(() => expect(box).toBeVisible());
  },
};

/** **The limits could not be read**: why, and Read again. */
export const CouldNotRead: Story = {
  parameters: { unread: "this machine's pools could not be read." },
  play: async ({ canvas }) => {
    (await canvas.findByRole("button", { name: "Limits of pool builds" })).click();
    const said = await canvas.findByRole("alert");
    await within(said).findByText(/could not be read/);
    await expect(canvas.getByRole("button", { name: "Read again" })).toBeVisible();
  },
};

/** **A change waits for the runner to be replaced** (#4286): what changed, and Replace. */
export const ChangeWaitingForReplacement: Story = {
  parameters: { set: { memory: 8192 }, waiting: ["Memory"] },
  play: async ({ canvas }) => {
    await open(canvas);
    await canvas.findByRole("button", { name: "Replace the runner…" });
  },
};

/** **Replace asks first**, and says running chats carry on on the old runner (#4286). */
export const ReplaceAsksFirst: Story = {
  parameters: { set: { memory: 8192 }, waiting: ["Memory"] },
  play: async ({ canvas, userEvent }) => {
    await open(canvas);
    await userEvent.click(await canvas.findByRole("button", { name: "Replace the runner…" }));
    const dialog = await within(document.body).findByRole("alertdialog");
    await expect(dialog.textContent).toContain("carry on on the old runner until they end");
  },
};

/** **A refused Replace says why**, with the change still waiting (#4286). */
export const ReplaceRefused: Story = {
  parameters: {
    set: { memory: 8192 },
    waiting: ["Memory"],
    refuseReplace: "Docker did not answer",
  },
  play: async ({ canvas, userEvent }) => {
    await open(canvas);
    await userEvent.click(await canvas.findByRole("button", { name: "Replace the runner…" }));
    const dialog = await within(document.body).findByRole("alertdialog");
    await userEvent.click(within(dialog).getByRole("button", { name: "Replace the runner" }));
    await canvas.findByText(/Docker did not answer/);
  },
};
