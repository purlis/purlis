// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A clone made outside purlis**: the project's Notice for a workspace clone purlis did not clone
 * and the person has not trusted, naming its remote, with Trust this clone and Dismiss; the one
 * whose remote names two addresses, with Dismiss alone; and what a refused Trust says. Drawn in
 * every cell of the theme matrix, each held to axe and walked by the keyboard. The core is faked
 * at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { CloneToVouch } from "./bindings";
import { CloneVouchNotices } from "./CloneVouchNotice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const MINE: CloneToVouch = {
  workspace: "shop",
  folder: "workspaces/shop/repos/storefront",
  place: "github.com/acme/storefront",
  places: ["github.com/acme/storefront"],
  made: null,
};

const SPLIT: CloneToVouch = {
  ...MINE,
  place: null,
  places: ["github.com/acme/storefront", "github.com/someone/storefront"],
};

/** What the faked core lists, and what it says to a Trust: nothing, or why it refused. */
let listed: CloneToVouch[] = [MINE];
let trustSays: string | undefined;

function Project() {
  return (
    <div style={{ inlineSize: 720 }}>
      <CloneVouchNotices
        plane="/Users/you/work/shop"
        dismissed={new Set()}
        dismiss={() => undefined}
        settle={() => undefined}
      />
    </div>
  );
}

const meta = {
  component: Project,
  beforeEach: () => {
    listed = [MINE];
    trustSays = undefined;
    mockIPC((cmd) => {
      if (cmd === "clones_to_vouch") return listed;
      if (cmd === "vouch_clone") {
        if (trustSays !== undefined) throw trustSays;
        return MINE.place;
      }
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Project>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Not trusted yet**: the line, with Trust this clone and Dismiss, each reached by Tab. */
export const NotTrustedYet: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/was not cloned by purlis/);
    const trust = canvas.getByRole("button", { name: "Trust this clone" });
    const dismiss = canvas.getByRole("button", { name: "Dismiss" });
    await userEvent.tab();
    await untilFocused(() => trust, "Trust this clone");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => dismiss, "Dismiss");
    expectFocusInSight();
  },
};

/** **Two addresses**: no Trust, the reason, and Dismiss reached by Tab. */
export const TwoAddresses: Story = {
  beforeEach: () => {
    listed = [SPLIT];
  },
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/purlis can trust it once origin has one address/);
    await expect(canvas.queryByRole("button", { name: "Trust this clone" })).toBeNull();
    const dismiss = canvas.getByRole("button", { name: "Dismiss" });
    await userEvent.tab();
    await untilFocused(() => dismiss, "Dismiss");
    expectFocusInSight();
  },
};

/** **A refused Trust**: the core's sentence on the line, which stays with its Trust. */
export const TrustRefused: Story = {
  play: async ({ canvas, userEvent }) => {
    trustSays =
      "The remote of workspaces/shop/repos/storefront changed since the window showed it, so purlis trusted nothing. Look at it again and choose";
    const trust = await canvas.findByRole("button", { name: "Trust this clone" });
    trust.focus();
    await userEvent.keyboard("{Enter}");
    await canvas.findByText(/changed since the window showed it/);
    await expect(canvas.getByRole("button", { name: "Trust this clone" })).toBeVisible();
  },
};
