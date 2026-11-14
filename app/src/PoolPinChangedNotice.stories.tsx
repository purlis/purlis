// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pool on another machine whose pin no longer holds** (#6270, ADR 0123 §3): the window's
 * Notice for each of the three changes, a host key, a CA and an engine, in the core's words, with
 * Check and trust again and Remove pool…, and the question Remove pool… asks. Drawn in every cell
 * of the theme matrix, held to axe and walked by the keyboard. The core is faked at its command
 * boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolPinChanged } from "./bindings";
import { PoolPinChangedNotice } from "./PoolPinChangedNotice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const KEY = "SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA";
const KEY_NOW = "SHA256:hX0mBv7pE9yS8Wd6Jf1Lq2Rk3Tn4Uo5Vp6Wq7Xr8Ys";
const CA =
  "7A:3F:91:0C:5E:D2:84:B6:19:EE:47:2A:C0:6D:F3:58:91:0B:7C:AA:E4:32:6F:1D:88:C5:09:B7:2E:64:D1:3F";
const CA_NOW =
  "C1:08:5B:7E:22:A9:F0:43:6D:91:BC:0E:57:34:A8:F2:6C:1D:E9:80:3B:47:A5:12:D6:9F:0C:68:B3:E1:24:7A";
const HELD = "purlis starts no runner, opens no new link and sends no vault value there until you";

const KEY_CHANGED: PoolPinChanged = {
  pool: "builds",
  host: "build-box",
  what: "key",
  pinned: KEY,
  now: KEY_NOW,
  title: "Pool builds: build-box shows a different key",
  said: `Pool builds: build-box shows a different key than the one you trusted (${KEY_NOW} now, ${KEY} trusted). This happens when the machine is reinstalled, and can mean something else is answering in its place. ${HELD} check its key and trust it again. Links already open stay open.`,
};

const CA_CHANGED: PoolPinChanged = {
  pool: "lab",
  host: "lab.example.internal",
  what: "ca",
  pinned: CA,
  now: CA_NOW,
  title: "Pool lab: lab.example.internal's CA changed",
  said: `Pool lab: /Users/sam/.docker/lab/ca.pem no longer holds the CA you trusted for lab.example.internal (${CA_NOW} now, ${CA} trusted). This happens when the CA is replaced, and can mean something else is answering in its place. ${HELD} check the CA and trust it again. Links already open stay open.`,
};

const ENGINE_CHANGED: PoolPinChanged = {
  pool: "builds",
  host: "build-box",
  what: "engine",
  pinned: "3TFZ:K4QX:LMNP:7RST",
  now: "9ABC:DEFG:HIJK:LMNO",
  title: "Pool builds: Docker on build-box isn't the one you trusted",
  said: `Pool builds: Docker on build-box isn't the one you trusted (9ABC:DEFG:HIJK:LMNO now, 3TFZ:K4QX:LMNP:7RST trusted). This happens when Docker there is reset or reinstalled. ${HELD} check it and trust it again. Links already open stay open. The pool's runners are on another engine, the one you trusted. Once you trust this one, you can make the runner again.`,
};

/** What the faked core has found changed. */
let found: PoolPinChanged[] = [KEY_CHANGED];

function Notices() {
  return (
    <div style={{ inlineSize: 720 }}>
      <PoolPinChangedNotice onCheckAndTrust={() => undefined} />
    </div>
  );
}

const meta = {
  component: Notices,
  beforeEach: () => {
    found = [KEY_CHANGED];
    mockIPC((cmd) => {
      if (cmd === "pools_pin_changed") return found;
      if (cmd === "pool_named_by") return [];
      if (cmd === "pool_held_runners") return [];
      if (cmd === "remove_docker_pool") return new Promise(() => undefined);
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Notices>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **The host key changed**: both fingerprints, and the two ways out, each reached by Tab. */
export const KeyChanged: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(KEY_CHANGED.title);
    await expect(canvas.getByText(/shows a different key than the one you trusted/)).toBeVisible();
    const again = canvas.getByRole("button", { name: "Check and trust again" });
    const remove = canvas.getByRole("button", { name: "Remove pool…" });
    await userEvent.tab();
    await untilFocused(() => again, "Check and trust again");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => remove, "Remove pool…");
    expectFocusInSight();
  },
};

/** **The CA changed**, over Docker's TLS API. */
export const AuthorityChanged: Story = {
  beforeEach: () => {
    found = [CA_CHANGED];
  },
  play: async ({ canvas }) => {
    await canvas.findByText(CA_CHANGED.title);
    await expect(canvas.getByText(/no longer holds the CA you trusted/)).toBeVisible();
    const again = canvas.getByRole("button", { name: "Check and trust again" });
    again.focus();
    await expect(again).toHaveFocus();
    expectFocusInSight();
  },
};

/** **The engine changed** under the same key: its runners are on another engine. */
export const EngineChanged: Story = {
  beforeEach: () => {
    found = [ENGINE_CHANGED];
  },
  play: async ({ canvas }) => {
    await canvas.findByText(ENGINE_CHANGED.title);
    await expect(canvas.getByText(/runners are on another engine/)).toBeVisible();
    const again = canvas.getByRole("button", { name: "Check and trust again" });
    again.focus();
    await expect(again).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Remove pool… pressed**: the question, saying nothing is sent there; Esc goes back. */
export const RemoveAsksFirst: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const remove = await canvas.findByRole("button", { name: "Remove pool…" });
    remove.focus();
    await userEvent.keyboard("{Enter}");
    const page = within(canvasElement.ownerDocument.body);
    const asking = await page.findByRole("alertdialog", { name: "Remove pool builds?" });
    await expect(asking.textContent).toMatch(/purlis sends nothing to build-box/);
    await untilFocused(
      () => within(asking).getByRole("button", { name: "Cancel" }),
      "the question's Cancel",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await expect(page.queryByRole("alertdialog")).toBeNull();
  },
};
