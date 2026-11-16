// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Your keys** (T1a-11, R-171, R-578): the model keys this machine keeps, a table of
 * controls on the aligned page, in each of its states: one key, none yet, reading, the list
 * that can't be read, a row whose keychain is locked, an orphaned line, *not checked yet*,
 * *refused at save*, a key changed outside purlis, a key sent to two pools, and each act busy and
 * failed, with Remove's and Stop sending's questions in their three forms, and a long product
 * name in the narrowest pane. Drawn, held to axe, walked by the keyboard and pictured in every
 * pictured theme cell (`docs/component-stories.md`). Core is faked at its command boundary, and
 * an act shown busy is answered never.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, screen, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { YourKeyRow } from "../bindings";
import { expectFocusInSight, inAWindow, untilFocused, untilSettled } from "../stories.testkit";
import { windowCopy } from "../windowCopy";
import { OnItsPage } from "./settingsStory.testkit";
import { yourKeysGroup } from "./yourKeys";

function key(more: Partial<YourKeyRow> = {}): YourKeyRow {
  return {
    id: "01JD6Y8ZQK3V4T5N6P7R8S9TAB",
    product: "Anthropic",
    origins: ["https://api.anthropic.com"],
    approval: "off",
    checked: "passed",
    state: "marked",
    profiles: ["claude-anthropic"],
    chats: ["Fix the parser"],
    pools: [],
    ...more,
  };
}

/** A command core never answers, so the act stays busy for the picture. */
const never = () => new Promise<never>(() => undefined);

/** What a failing command answers. */
const fails = (error: string) => () => Promise.reject(error);

type Core = Record<string, (args: Record<string, unknown>) => unknown>;

function theCore(answers: Core) {
  mockIPC(
    (cmd, args) => {
      const answer = answers[cmd];
      return answer === undefined ? null : answer((args ?? {}) as Record<string, unknown>);
    },
    { shouldMockEvents: true },
  );
}

function Page({ inlineSize }: { inlineSize?: number }) {
  return <OnItsPage group={yourKeysGroup()} inlineSize={inlineSize} />;
}

const meta = {
  component: Page,
  beforeEach: ({ parameters }) => {
    // A story's core is its answers, or a function making them afresh for each drawing, where
    // they change as they are asked (a count that differs each time).
    const given = parameters.core as Core | (() => Core) | undefined;
    const answers = typeof given === "function" ? given() : (given ?? {});
    theCore({ your_keys: () => [key()], ...answers });
    return () => clearMocks();
  },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The page's table, once core has answered. */
const theTable = (canvas: { findByRole: typeof screen.findByRole }) =>
  canvas.findByRole("table", { name: "Your keys" });

/** Presses `button` from the keyboard: focus, then Enter. */
async function press(
  button: HTMLElement,
  step: string,
  userEvent: { keyboard: (keys: string) => Promise<void> },
) {
  button.focus();
  await untilFocused(() => button, step);
  expectFocusInSight();
  await userEvent.keyboard("{Enter}");
}

// ---------------------------------------------------------------------------------------------
// The list (R-171)

/**
 * **One key**: its product, the origin it is marked for, its approval setting, the profile and
 * chat using it, and Replace the key… and Remove…, each a Tab stop of its own.
 */
export const YourKeysPage: Story = {
  play: async ({ canvas, userEvent }) => {
    const table = await theTable(canvas);
    await expect(within(table).getAllByRole("rowheader")).toHaveLength(1);
    await expect(within(table).getByText("Sent to https://api.anthropic.com")).toBeVisible();
    await expect(within(table).getByText("Asks before each use: off")).toBeVisible();
    const replace = canvas.getByRole("button", { name: "Replace the key… for Anthropic" });
    replace.focus();
    await untilFocused(() => replace, "Replace the key…");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Remove the key for Anthropic" }),
      "Tab goes on to Remove…",
    );
    expectFocusInSight();
  },
};

/** **A key sent to two pools**: each pool's line with its own Stop sending. */
export const YourKeysSentToPools: Story = {
  parameters: { core: { your_keys: () => [key({ pools: ["gpu", "lab"] })] } },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    const gpu = canvas.getByRole("button", { name: "Stop sending to gpu" });
    gpu.focus();
    await untilFocused(() => gpu, "Stop sending to gpu");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Stop sending to lab" }),
      "Tab goes on to the next pool's Stop sending",
    );
    expectFocusInSight();
  },
};

/**
 * **Stop sending asks first**, in W123's three forms: two chats there, one, and none. Each is
 * cancelled from the keyboard; the last stays up for the picture.
 */
export const YourKeysStopSendingConfirms: Story = {
  parameters: {
    screenshot: "page",
    core: () => {
      const counts = [2, 1, 0];
      return {
        your_keys: () => [key({ pools: ["gpu"] })],
        your_key_pool_chats: () => counts.shift() ?? 0,
      };
    },
  },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    for (const count of [2, 1, 0]) {
      await press(
        canvas.getByRole("button", { name: "Stop sending to gpu" }),
        "Stop sending",
        userEvent,
      );
      const asked = await screen.findByRole("alertdialog");
      await untilSettled(asked);
      await expect(within(asked).getByText(windowCopy.W123({ count, pool: "gpu" }))).toBeVisible();
      if (count > 0) {
        await userEvent.keyboard("{Escape}");
        await expect(screen.queryByRole("alertdialog")).toBeNull();
      }
    }
  },
};

/** **Stop sending, busy** while core deletes the row. */
export const YourKeysStopSendingBusy: Story = {
  parameters: {
    core: {
      your_keys: () => [key({ pools: ["gpu"] })],
      your_key_pool_chats: () => 1,
      your_key_stop_sending: never,
    },
  },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: "Stop sending to gpu" }),
      "Stop sending",
      userEvent,
    );
    const asked = await screen.findByRole("alertdialog");
    await untilSettled(asked);
    await userEvent.click(within(asked).getByRole("button", { name: windowCopy.W125 }));
    await expect(await canvas.findByText("Stopping…")).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Stop sending to gpu" })).toBeDisabled();
  },
};

/** **Stop sending, failed**: the pool's line stays, with W129 under it. */
export const YourKeysStopSendingFailed: Story = {
  parameters: {
    core: {
      your_keys: () => [key({ pools: ["gpu"] })],
      your_key_pool_chats: () => 1,
      your_key_stop_sending: fails("the allowance store is locked"),
    },
  },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: "Stop sending to gpu" }),
      "Stop sending",
      userEvent,
    );
    const asked = await screen.findByRole("alertdialog");
    await untilSettled(asked);
    await userEvent.click(within(asked).getByRole("button", { name: windowCopy.W125 }));
    await expect(
      await canvas.findByText(
        windowCopy.W129({ pool: "gpu", error: "the allowance store is locked" }),
      ),
    ).toBeVisible();
  },
};

// ---------------------------------------------------------------------------------------------
// The page's states (R-578)

/** **None yet**: W4, with Add a key…, which opens the key form in place under the head. */
export const YourKeysEmpty: Story = {
  parameters: { core: { your_keys: () => [] } },
  play: async ({ canvas, userEvent }) => {
    await expect(await canvas.findByText(windowCopy.W4)).toBeVisible();
    await press(canvas.getByRole("button", { name: "Add a key…" }), "Add a key…", userEvent);
    await expect(await canvas.findByLabelText(windowCopy.W1)).toBeVisible();
    await expect(screen.queryByRole("dialog")).toBeNull();
  },
};

/** **Reading**: while core answers the list. Words only. */
export const YourKeysReading: Story = {
  parameters: { core: { your_keys: never } },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText("Reading your keys…")).toBeVisible();
  },
};

/** **The list can't be read**: W115, with Try again, busy while core reads again. */
export const YourKeysListUnreadable: Story = {
  parameters: {
    core: () => {
      let asked = 0;
      return {
        your_keys: () =>
          asked++ === 0 ? Promise.reject("the index is not your keys' index") : never(),
      };
    },
  },
  play: async ({ canvas, userEvent }) => {
    await expect(
      await canvas.findByText(windowCopy.W115({ error: "the index is not your keys' index" })),
    ).toBeVisible();
    await press(canvas.getByRole("button", { name: "Try again" }), "Try again", userEvent);
    await expect(await canvas.findByText("Reading…")).toBeVisible();
  },
};

const LOCKED = { your_keys: () => [key({ state: "locked" })] };

/** **The keychain locked**: W6, with Unlock. */
export const YourKeysUnreadable: Story = {
  parameters: { core: LOCKED },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText(windowCopy.W6)).toBeVisible();
    const unlock = canvas.getByRole("button", { name: "Unlock the key for Anthropic" });
    unlock.focus();
    await untilFocused(() => unlock, "Unlock");
    expectFocusInSight();
  },
};

/** **Unlock, busy** while the system asks. */
export const YourKeysUnlockBusy: Story = {
  parameters: { core: { ...LOCKED, your_key_unlock: never } },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: "Unlock the key for Anthropic" }),
      "Unlock",
      userEvent,
    );
    await expect(await canvas.findByText("Unlocking…")).toBeVisible();
  },
};

/** **Unlock, cancelled**: back to W6, with nothing under it. */
export const YourKeysUnlockCancelled: Story = {
  parameters: { core: { ...LOCKED, your_key_unlock: () => false } },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: "Unlock the key for Anthropic" }),
      "Unlock",
      userEvent,
    );
    await expect(
      await canvas.findByRole("button", { name: "Unlock the key for Anthropic" }),
    ).toBeEnabled();
    await expect(canvas.getByText(windowCopy.W6)).toBeVisible();
    await expect(canvas.queryByText(windowCopy.W84)).toBeNull();
  },
};

/** **Unlock, failed**: W84 under W6. */
export const YourKeysUnlockFailed: Story = {
  parameters: { core: { ...LOCKED, your_key_unlock: fails("the keychain refused") } },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: "Unlock the key for Anthropic" }),
      "Unlock",
      userEvent,
    );
    await expect(await canvas.findByText(windowCopy.W84)).toBeVisible();
  },
};

/** **An orphaned line**: W55, with Remove… only. */
export const YourKeysOrphanedLine: Story = {
  parameters: { core: { your_keys: () => [key({ state: "missing", origins: [] })] } },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText(windowCopy.W55)).toBeVisible();
    await expect(canvas.queryByRole("button", { name: /^Replace the key…/ })).toBeNull();
    const remove = canvas.getByRole("button", { name: "Remove the key for Anthropic" });
    remove.focus();
    await untilFocused(() => remove, "Remove…");
    expectFocusInSight();
  },
};

const NOT_CHECKED = { your_keys: () => [key({ checked: "not checked yet" })] };

/** ***Not checked yet***: W7's row, with its Try again on the row. */
export const YourKeysNotCheckedYet: Story = {
  parameters: { core: NOT_CHECKED },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText(windowCopy.W7)).toBeVisible();
    const again = canvas.getByRole("button", { name: "Try again: check the key for Anthropic" });
    again.focus();
    await untilFocused(() => again, "Try again");
    expectFocusInSight();
  },
};

/** **The row's Try again** (F-6), busy while the check runs. */
export const YourKeysNotCheckedYetTryAgain: Story = {
  parameters: { core: { ...NOT_CHECKED, your_key_check_again: never } },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: "Try again: check the key for Anthropic" }),
      "Try again",
      userEvent,
    );
    await expect(await canvas.findByText("Checking…")).toBeVisible();
  },
};

/** ***Refused at save***: W46, with Replace the key…. Words only. */
export const YourKeysRefusedAtSave: Story = {
  parameters: { core: { your_keys: () => [key({ checked: "refused at save" })] } },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText(windowCopy.W46)).toBeVisible();
    await expect(
      canvas.getByRole("button", { name: "Replace the key… for Anthropic" }),
    ).toBeVisible();
  },
};

const UNMARKED = { your_keys: () => [key({ state: "unmarked", origins: [] })] };

/** **Changed outside purlis**: W60, with Mark again, and no Unmark anywhere. */
export const YourKeysUnmarked: Story = {
  parameters: { core: UNMARKED },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText(windowCopy.W60)).toBeVisible();
    await expect(canvas.queryByRole("button", { name: /unmark/i })).toBeNull();
    const again = canvas.getByRole("button", { name: `${windowCopy.W126}: the key for Anthropic` });
    again.focus();
    await untilFocused(() => again, "Mark again");
    expectFocusInSight();
  },
};

/** **Mark again, busy** while T1a-10's command runs. */
export const YourKeysMarkAgainBusy: Story = {
  parameters: { core: { ...UNMARKED, your_key_mark_again: never } },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: `${windowCopy.W126}: the key for Anthropic` }),
      "Mark again",
      userEvent,
    );
    await expect(await canvas.findByText("Marking…")).toBeVisible();
  },
};

/** **Mark again, failed**: the row stays unmarked, with W128 under it. */
export const YourKeysMarkAgainFailed: Story = {
  parameters: {
    core: { ...UNMARKED, your_key_mark_again: fails("the marks' device key couldn't be kept") },
  },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: `${windowCopy.W126}: the key for Anthropic` }),
      "Mark again",
      userEvent,
    );
    await expect(
      await canvas.findByText(windowCopy.W128({ error: "the marks' device key couldn't be kept" })),
    ).toBeVisible();
    await expect(canvas.getByText(windowCopy.W60)).toBeVisible();
  },
};

/**
 * **Remove… asks first**, in W5's three forms, and offers no undo: two chats, one, and none.
 * Each is cancelled from the keyboard; the last stays up for the picture.
 */
export const YourKeysRemoveConfirms: Story = {
  parameters: {
    screenshot: "page",
    core: () => {
      const counts = [2, 1, 0];
      return { your_keys: () => [key()], your_key_chats: () => counts.shift() ?? 0 };
    },
  },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    for (const count of [2, 1, 0]) {
      await press(
        canvas.getByRole("button", { name: "Remove the key for Anthropic" }),
        "Remove…",
        userEvent,
      );
      const asked = await screen.findByRole("alertdialog");
      await untilSettled(asked);
      await expect(
        within(asked).getByText(`${windowCopy.W5({ count })} There is no undo.`),
      ).toBeVisible();
      await expect(within(asked).queryByRole("button", { name: /undo/i })).toBeNull();
      if (count > 0) {
        await userEvent.keyboard("{Escape}");
        await expect(screen.queryByRole("alertdialog")).toBeNull();
      }
    }
  },
};

/** **Remove, failed**: the row stays, with W114 under it. */
export const YourKeysRemoveFailed: Story = {
  parameters: {
    core: {
      your_keys: () => [key()],
      your_key_chats: () => 1,
      your_key_remove: fails("your keychain didn't delete it"),
    },
  },
  play: async ({ canvas, userEvent }) => {
    await theTable(canvas);
    await press(
      canvas.getByRole("button", { name: "Remove the key for Anthropic" }),
      "Remove…",
      userEvent,
    );
    const asked = await screen.findByRole("alertdialog");
    await untilSettled(asked);
    await userEvent.click(within(asked).getByRole("button", { name: "Remove" }));
    await expect(
      await canvas.findByText(windowCopy.W114({ error: "your keychain didn't delete it" })),
    ).toBeVisible();
    await expect(canvas.getByText("Anthropic")).toBeVisible();
  },
};

/** Opens the row's Replace the key…, pastes a synthetic value and saves. */
async function replaceWith(
  canvas: Parameters<NonNullable<Story["play"]>>[0]["canvas"],
  userEvent: Parameters<NonNullable<Story["play"]>>[0]["userEvent"],
) {
  await theTable(canvas);
  await press(
    canvas.getByRole("button", { name: "Replace the key… for Anthropic" }),
    "Replace the key…",
    userEvent,
  );
  const box = await canvas.findByLabelText(windowCopy.W1);
  await userEvent.type(box, "synthetic-rotated-value");
  await userEvent.click(canvas.getByRole("button", { name: "Save" }));
}

/** **Replace, busy** while core writes the value and seals the mark and pools again. */
export const YourKeysReplaceBusy: Story = {
  parameters: { core: { your_keys: () => [key()], your_key_save: never } },
  play: async ({ canvas, userEvent }) => {
    await replaceWith(canvas, userEvent);
    await expect(await canvas.findByText("Saving…")).toBeVisible();
    await expect(screen.queryByRole("dialog")).toBeNull();
  },
};

/** **Replace, failed**: W88 under the box, with Try again. */
export const YourKeysReplaceFailed: Story = {
  parameters: {
    core: { your_keys: () => [key()], your_key_save: fails("your keychain refused the write") },
  },
  play: async ({ canvas, userEvent }) => {
    await replaceWith(canvas, userEvent);
    await expect(
      await canvas.findByText(windowCopy.W88({ error: "your keychain refused the write" })),
    ).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Try again" })).toBeVisible();
  },
};

/**
 * **A long product name, in the narrowest pane**: the row wraps, and Replace the key… and
 * Remove… stay in sight, in the narrowest window purlis opens (1024 px) with Settings in a pane
 * of the width Settings goes narrow at (R75).
 */
export const YourKeysLongProductNarrowPane: Story = {
  args: { inlineSize: 640 },
  parameters: {
    core: {
      your_keys: () => [
        key({
          product:
            "Anthropic on the team's Bedrock gateway in the European region, for long contexts",
          origins: ["https://bedrock-runtime.eu-central-1.amazonaws.com"],
        }),
      ],
    },
  },
  play: async ({ canvas }) => {
    await inAWindow(1024, 720);
    await theTable(canvas);
    for (const name of [/^Replace the key… for Anthropic on/, /^Remove the key for Anthropic on/]) {
      const button = canvas.getByRole("button", { name });
      await expect(button).toBeVisible();
      button.focus();
      await untilFocused(() => button, String(name));
      expectFocusInSight();
    }
  },
};
