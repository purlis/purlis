// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The merge row** (T1-40; R-120, R-121, R-126, R-147, R-151, R-303): a merge a chat's hook
 * held, on its row in Asks. It names the repo and the head, says there is no undo, shows the
 * ruleset state purlis read with the gap beside it, says a refusal of the forge's on the row,
 * and its word for no is Don't merge. Don't push is phase 2's, on a push row (R-116).
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { MergeShown, RulesetShown, Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const HEAD = "0123456789abcdef0123456789abcdef01234567";

/** What the forge refused a press with, as the core passes it on. */
const REFUSED =
  "the forge did not merge it: Repository rule violations found for refs/heads/main. 2 of 2 required status checks are expected.";

const facts = (ruleset: RulesetShown, shown: Partial<MergeShown> = {}): MergeShown => ({
  title: "Add the parser",
  author: "octo",
  base: "main",
  checks: "PASSED",
  read_at: 1_763_000_000,
  ruleset,
  look: "not-viewed",
  approved_by: null,
  since_viewed: null,
  pushed_here: true,
  digest: "d-passed",
  ...shown,
});

/** The merge chat 3's hook held: request 12 of `o/r` at one head, into main. */
const hold = (ruleset: RulesetShown, shown: Partial<MergeShown> = {}): Shown => ({
  session: 3,
  ask: `merge:3:o/r#12@${HEAD}`,
  says: "Merge request 12 of o/r at 0123456789ab? There is no undo.",
  options: [
    { id: "merge", label: "Merge", allows: true },
    { id: "dont-merge", label: "Don't merge", allows: false },
  ],
  source: "merge-hold",
  chain: ["steward 3"],
  answer: { via: "merge-hold", repo: "o/r", number: 12, head: HEAD, facts: facts(ruleset, shown) },
  risk: {
    class: "merge",
    why: "Merges request 12 of o/r at 0123456 into main, with no undo",
    confirm: "Merge request 12 of o/r at 0123456 into main?",
  },
  since: 1_763_000_000,
});

function MergeRow({ ruleset, shown }: { ruleset: RulesetShown; shown?: Partial<MergeShown> }) {
  const [asks] = useState(() => [hold(ruleset, shown)]);
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox plane="/plane" asks={asks} onGo={() => {}} onLeave={() => {}} />
    </div>
  );
}

const meta = {
  component: MergeRow,
  beforeEach: () => {
    mockIPC((cmd, args) => {
      if (cmd === "reread_held_merge")
        return { head: (args as { head: string }).head, facts: facts("protects") };
      if (cmd === "merge_held_request") throw new Error(REFUSED);
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof MergeRow>;

export default meta;
type Story = StoryObj<typeof meta>;

const rowOf = (canvasElement: HTMLElement) =>
  canvasElement.querySelector<HTMLElement>('[role="row"][data-source="merge-hold"]') as HTMLElement;

/** Tab onto the row, → to its Answer, and Return: its answers, drawn in the row. */
const answersOf = async (row: HTMLElement) => {
  await userEvent.tab();
  await untilFocused(() => row, "Tab lands on the merge row");
  expectFocusInSight();
  const answer = within(row).getByRole("button", { name: /^Answer/ });
  for (let step = 0; step < 6 && document.activeElement !== answer; step++)
    await userEvent.keyboard("{ArrowRight}");
  await untilFocused(() => answer, "→ reaches its Answer");
  await userEvent.keyboard("{Enter}");
  await waitFor(() =>
    expect(within(row).getByRole("button", { name: "Don't merge" })).toBeVisible(),
  );
};

/** The person reads the row for longer than a press waits for it to settle. */
const read = () => new Promise((done) => setTimeout(done, SETTLE_MS + 50));

/** **The merge row names the repo and the head SHA** (R-120), and **says There is no undo**
 *  (R-121): `the merge row names the repo and the head SHA`, `the merge row says There is no
 *  undo`. */
export const NamesTheRepoAndTheHead: Story = {
  args: { ruleset: "protects" },
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement);
    await expect(row).toHaveTextContent("o/r");
    await expect(row).toHaveTextContent("0123456789ab");
    await expect(row).toHaveTextContent("There is no undo.");
    await answersOf(row);
  },
};

/** **The merge row shows the ruleset state** (R-147), protected: `the merge row shows the
 *  ruleset state`. */
export const AProtectedBase: Story = {
  args: { ruleset: "protects" },
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement);
    await expect(row.querySelector(".merge-protection")).toHaveTextContent(
      "A ruleset protects main.",
    );
    await answersOf(row);
  },
};

/** **The merge row shows the ruleset state** (R-147), unprotected, with the gap beside it
 *  (R-151): `the merge row states that a script merging with the chat's token gets past
 *  purlis`. */
export const AnUnprotectedBaseAndTheGap: Story = {
  args: { ruleset: "none" },
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement);
    await expect(row.querySelector(".merge-protection")).toHaveTextContent(
      "No ruleset on main. That's fine: the merge still waits for you. A script that merges with the chat's token gets past purlis.",
    );
    await answersOf(row);
  },
};

/** **The word for no on a merge row is Don't merge** (R-303): `the word for no on a merge row is
 *  Don't merge`. Don't push is a push row's, in phase 2. */
export const TheWordForNoIsDontMerge: Story = {
  args: { ruleset: "protects" },
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement);
    await answersOf(row);
    const words = within(row)
      .getAllByRole("button")
      .map((one) => one.textContent ?? "");
    await expect(words).toContain("Don't merge");
    await expect(words.filter((one) => /^(No|Deny|Don't push|Keep blocked)$/.test(one))).toEqual(
      [],
    );
  },
};

/** **A ruleset or queue refusal shows on the merge row** (R-126): the forge refused the merge,
 *  and the row says so in the forge's words; nothing fails silently. */
export const AForgeRefusalShowsOnTheRow: Story = {
  args: { ruleset: "protects" },
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement);
    await answersOf(row);
    await read();
    await userEvent.click(within(row).getByRole("button", { name: "Merge" }));
    const again = await within(row).findByRole("group", {
      name: "Merge request 12 of o/r at 0123456 into main?",
    });
    await read();
    await userEvent.click(within(again).getByRole("button", { name: "Merge" }));
    await waitFor(() => expect(within(row).getByRole("status")).toHaveTextContent(REFUSED));
  },
};

/** **A moved head needs the once-more press, with the commit count** (T1-41; R-123, Q268): the
 *  person viewed the request two commits ago, so the row says so, and Merge asks once more
 *  before anything merges: `a moved head needs the once-more press with the commit count`. */
export const AMovedHeadNeedsTheOnceMorePress: Story = {
  args: { ruleset: "protects", shown: { look: "not-viewed", since_viewed: 2 } },
  play: async ({ canvasElement }) => {
    const row = rowOf(canvasElement);
    await expect(row).toHaveTextContent("2 commits since you looked.");
    await answersOf(row);
    await read();
    await userEvent.click(within(row).getByRole("button", { name: "Merge" }));
    await expect(
      await within(row).findByRole("group", {
        name: "Merge request 12 of o/r at 0123456 into main?",
      }),
    ).toBeVisible();
  },
};
