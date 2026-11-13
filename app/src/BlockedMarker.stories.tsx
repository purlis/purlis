// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A block shows as a marker on its chat: "2 new blocked"** (R-172, R-299; T1-30): on the
 * sidebar row's end and on the pane's band, a quiet button that opens the chat's Chat log
 * filtered to blocks; a muted dot where the only new reports are unverified, which never count
 * on the sidebar. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn } from "storybook/test";
import { BlockedMarker } from "./BlockedMarker";
import { BlockedMarkers, markerOf, type BlockedMarkersHeld } from "./blockedMarkers";
import type { ChatBlocks } from "./bindings";
import { expectFocusInSight } from "./stories.testkit";

/** The marker in both its places: a sidebar row's end and a pane's band. */
function Placed({ counted, open }: { counted: ChatBlocks; open: (session: number) => void }) {
  const held: BlockedMarkersHeld = { of: () => markerOf(counted, new Set()), open };
  return (
    <BlockedMarkers.Provider value={held}>
      <div style={{ display: "grid", gap: "var(--space-4)", inlineSize: "20rem" }}>
        <div className="chat" data-placement="sidebar" aria-label="Sidebar row" role="group">
          <span className="session ui-text-body">talk</span>{" "}
          <BlockedMarker session={counted.session} />
        </div>
        <div className="pane-chips" data-placement="band" aria-label="Pane band" role="group">
          <span className="pane-name">talk</span> <BlockedMarker session={counted.session} />
        </div>
      </div>
    </BlockedMarkers.Provider>
  );
}

const meta = {
  component: Placed,
  args: {
    counted: { session: 7, blocks: 2, unverified: 0, latest: "01B" },
    open: fn(),
  },
} satisfies Meta<typeof Placed>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **"2 new blocked", on the sidebar row and the pane band** (R-172): the word "new" in both,
 * pressed from the keyboard too, opening that chat's Chat log (R-301).
 */
export const TwoNewBlocked: Story = {
  play: async ({ canvas, args, userEvent }) => {
    const markers = canvas.getAllByRole("button", { name: "2 new blocked" });
    await expect(markers).toHaveLength(2);
    await expect(canvas.getByRole("group", { name: "Sidebar row" })).toContainElement(markers[0]);
    await expect(canvas.getByRole("group", { name: "Pane band" })).toContainElement(markers[1]);
    // Seen, never announced (R-176).
    await expect(canvas.queryByRole("status")).toBeNull();
    markers[1].focus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.open).toHaveBeenCalledWith(7);
  },
};

/**
 * **Unverified reports alone: a muted dot** (R-299): no count on the sidebar; the count is in
 * the Chat log and the detail.
 */
export const UnverifiedIsAMutedDot: Story = {
  args: { counted: { session: 7, blocks: 0, unverified: 3, latest: "01C" } },
  play: async ({ canvas }) => {
    await expect(
      canvas.getAllByRole("img", { name: "Unverified blocks, counted in its Chat log" }),
    ).toHaveLength(2);
    await expect(canvas.queryByText(/new blocked/)).toBeNull();
    await expect(canvas.queryByText(/\d/)).toBeNull();
  },
};
