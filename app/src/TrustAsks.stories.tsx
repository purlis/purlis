// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The trust asks in Asks** (T1-42; R-330, R-337, Q181, Q184): a GitHub token refused for a
 * clone purlis has no record of is "Trust this clone so chats can use GitHub", its Trust this
 * clone on its row; the clones purlis has no record of are one ask, "Trust these clones", each
 * clone with its remote and when it was made, with Trust all and Choose….
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { TRUST_ALL_ASK, TRUST_CLONE_ASK } from "./trustAsks.testkit";

function TrustAsks({ ask }: { ask: Shown }) {
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox plane="/plane" asks={[ask]} onGo={() => {}} onLeave={() => {}} />
    </div>
  );
}

const meta = {
  component: TrustAsks,
} satisfies Meta<typeof TrustAsks>;

export default meta;
type Story = StoryObj<typeof meta>;

const rowOf = (canvasElement: HTMLElement, source: string) =>
  canvasElement.querySelector<HTMLElement>(`[role="row"][data-source="${source}"]`) as HTMLElement;

/** **The Trust this clone row carries its control** (R-330): the ask's words, and Trust this
 *  clone on the row, reached by the keyboard. */
export const TheTrustThisCloneRowCarriesItsControl: Story = {
  args: { ask: TRUST_CLONE_ASK },
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement, "trust-clone");
    await expect(row).toHaveTextContent("Trust this clone so chats can use GitHub");
    await expect(within(row).queryByRole("button", { name: /^Answer/ })).toBeNull();
    const trust = within(row).getByRole("button", { name: "Trust this clone" });
    await userEvent.tab();
    await untilFocused(() => row, "Tab lands on the ask's row");
    for (let step = 0; step < 6 && document.activeElement !== trust; step++)
      await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => trust, "→ reaches Trust this clone");
    expectFocusInSight();
  },
};

/** **The clones ask offers Trust all and Choose…, and lists each remote and time** (R-337):
 *  Choose… opens a box for each clone under the row. */
export const TheClonesAskOffersTrustAllAndChooseAndListsEachRemoteAndTime: Story = {
  args: { ask: TRUST_ALL_ASK },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const row = rowOf(canvasElement, "trust-clones");
    const clones = within(row).getByRole("list", { name: "Clones purlis did not clone" });
    const each = within(clones).getAllByRole("listitem");
    await expect(each).toHaveLength(2);
    await expect(each[0]).toHaveTextContent("github.com/acme/api");
    await expect(each[1]).toHaveTextContent("github.com/acme/web");
    await expect(each[0].querySelector("time")).not.toBeNull();
    await expect(each[1].querySelector("time")).not.toBeNull();
    await expect(within(row).getByRole("button", { name: "Trust all" })).toBeInTheDocument();
    const choose = within(row).getByRole("button", { name: "Choose…" });
    await userEvent.tab();
    await untilFocused(() => row, "Tab lands on the ask's row");
    for (let step = 0; step < 6 && document.activeElement !== choose; step++)
      await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => choose, "→ reaches Choose…");
    expectFocusInSight();
    // The person reads the row before choosing.
    await new Promise((done) => setTimeout(done, 1600));
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("group", { name: "Choose the clones to trust" })).toBeInTheDocument(),
    );
    await expect(canvas.getAllByRole("checkbox")).toHaveLength(2);
  },
};
