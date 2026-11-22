// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A held forge act in Asks** (one login F2; ADR 0128 D8): the four kinds a chat can ask for, each
 * with every field its digest covers, the press that asks once more, and a Don't beside it. Walked
 * by the keyboard and held to axe, like every component (`docs/component-stories.md`).
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import type { HeldActShown } from "./bindings";
import { HeldActRow, onceMoreSaid, type FirstPress } from "./HeldActAsks";
import { CI_PUSH, FORK, RELEASE, TAG } from "./HeldActAsks.fixtures";
import { expectFocusInSight } from "./stories.testkit";

/** One ask, as Asks draws it, with the two presses walked: what it said last is written beside it. */
function OneAsk({ act }: { act: HeldActShown }) {
  const [first, setFirst] = useState<FirstPress>();
  const [said, setSaid] = useState("Nothing pressed yet");
  return (
    <div style={{ inlineSize: "32rem", maxInlineSize: "100%" }}>
      <ul style={{ margin: 0, padding: 0, listStyle: "none" }}>
        <HeldActRow
          act={act}
          first={first}
          onPress={() => {
            if (first === undefined) setFirst({ nonce: "nonce-1", digest: act.digest });
            else {
              setFirst(undefined);
              setSaid("Made, with the nonce the first press returned");
            }
          }}
          onCancel={() => setFirst(undefined)}
          onDont={() => {
            setFirst(undefined);
            setSaid("Put away; nothing was made");
          }}
        />
      </ul>
      <p role="status">{said}</p>
    </div>
  );
}

const meta = {
  component: OneAsk,
  args: { act: TAG },
} satisfies Meta<typeof OneAsk>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A tag: the commit and whether it is on the default branch. Tab reaches the press, then Don't. */
export const Tag: Story = {
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByText(TAG.rows[3].value)).toBeInTheDocument();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Make the tag" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Don't" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByText("Put away; nothing was made")).toBeInTheDocument());
  },
};

/** A release: the notes shown whole under its facts, then the press. */
export const Release: Story = {
  args: { act: RELEASE },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("figure", { name: "Notes" })).toHaveTextContent(
      "Fixes the crash on first launch.",
    );
    await expect(canvas.getByRole("button", { name: "Make the release" })).toBeInTheDocument();
  },
};

/** A CI-file push: the changed lines with their paths, which a person reads before pressing. */
export const CiFilePush: Story = {
  args: { act: CI_PUSH },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("figure", { name: "Changed lines" })).toHaveTextContent(
      "+++ b/.github/workflows/ci.yml",
    );
    await expect(canvas.getByRole("button", { name: "Push the branch" })).toBeInTheDocument();
  },
};

/** A fork: the source and the person's own account, the only place it goes. */
export const Fork: Story = {
  args: { act: FORK },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("your own account, aaron")).toBeInTheDocument();
    await expect(canvas.getByRole("button", { name: "Fork it" })).toBeInTheDocument();
  },
};

/** The first press asks once more, and Enter on the second makes it: Cancel takes it back. */
export const AsksOnceMore: Story = {
  args: { act: TAG },
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByText(onceMoreSaid(TAG))).toBeInTheDocument());
    await expect(canvas.getByRole("button", { name: "Yes, make the tag" })).toBeInTheDocument();
    const cancel = canvas.getByRole("button", { name: "Cancel" });
    await userEvent.click(cancel);
    await waitFor(() => expect(canvas.queryByText(onceMoreSaid(TAG))).toBeNull());
    await userEvent.tab();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByText("Made, with the nonce the first press returned")).toBeInTheDocument(),
    );
  },
};
