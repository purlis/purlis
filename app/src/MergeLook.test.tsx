// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { MergeHoldNotice } from "./MergeHoldNotice";
import type { LookShown, MergeShown, Shown } from "./bindings";
import { SETTLE_MS } from "./noticeSettle";

/**
 * **Who looked at a merge's head, and who pushed it** (T1-41; R-122, R-124, R-125, R-302). A
 * merge always takes two presses (Q268, Q269): what the core says of the head (viewed in purlis,
 * approved by someone else on the forge, or neither, with the commits since the person looked)
 * is said beside it, and a head the chat didn't push is flagged. View changes shows the
 * request's changes and marks its head viewed. The press hands back the digest of every fact
 * its row showed (the follow-up Q269 named).
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const SESSION = 3;
const HEAD = "0123456789abcdef0123456789abcdef01234567";
const MOVED = "fedcba9876543210fedcba9876543210fedcba98";
const DIFF = "diff --git a/parser.rs b/parser.rs\n+fn parse() {}\n";

const facts = (shown: Partial<MergeShown> = {}): MergeShown => ({
  title: "Add the parser",
  author: "octo",
  base: "main",
  checks: "PASSED",
  read_at: 1_763_000_000,
  ruleset: "none",
  look: "not-viewed",
  since_viewed: null,
  pushed_here: true,
  digest: "d-passed",
  ...shown,
});

const hold = (shown: Partial<MergeShown> = {}, head = HEAD): Shown => ({
  session: SESSION,
  ask: `merge:${SESSION}:o/r#12@${head}`,
  says: `Merge request 12 of o/r at ${head.slice(0, 12)}? There is no undo.`,
  options: [
    { id: "merge", label: "Merge", allows: true },
    { id: "dont-merge", label: "Don't merge", allows: false },
  ],
  source: "merge-hold",
  chain: ["steward 3"],
  answer: { via: "merge-hold", repo: "o/r", number: 12, head, facts: facts(shown) },
  risk: {
    class: "merge",
    why: `Merges request 12 of o/r at ${head.slice(0, 7)} into main, with no undo`,
    confirm: `Merge request 12 of o/r at ${head.slice(0, 7)} into main?`,
  },
  since: 1_000,
});

/** A core that records what the window sends; its re-read gives the row's own facts back. */
function core() {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "reread_held_merge") {
      const { head } = args as { head: string };
      return { head, facts: facts() };
    }
    if (cmd === "view_held_merge") return DIFF;
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

const notice = () => screen.getByRole("status", { name: "Merge held" });
const read = () => new Promise((done) => setTimeout(done, SETTLE_MS + 50));
const buttons = () =>
  within(notice())
    .getAllByRole("button")
    .map((one) => one.textContent);
const confirming = async () => {
  await userEvent.click(within(notice()).getByRole("button", { name: "Merge" }));
  await within(notice()).findByRole("button", { name: "Cancel" });
};

describe("who looked at a held merge's head (T1-41)", () => {
  const cases: [string, LookShown, number | null, string][] = [
    ["viewed in purlis", "viewed", null, "You viewed these changes in purlis at this head."],
    [
      "approved by someone else",
      "approved",
      null,
      "Approved at this head by a reviewer who isn't its author, an app or a bot.",
    ],
    [
      "neither (the author's, an app's or a bot's approval, or one of an older head, reads so)",
      "not-viewed",
      null,
      "You haven't viewed these changes in purlis.",
    ],
    ["viewed at an older head", "not-viewed", 2, "2 commits since you looked."],
    ["forgotten by a restart", "not-viewed", null, "You haven't viewed"],
  ];

  it.each(cases)(
    "a merge takes two presses whether the head was %s, and says which (R-122, Q268)",
    async (_case, look, since, said) => {
      const sent = core();
      render(
        <MergeHoldNotice
          plane={PLANE}
          session={SESSION}
          asks={[hold({ look, since_viewed: since })]}
        />,
      );
      expect(notice().textContent).toContain(said);

      await confirming();
      expect(sent("merge_held_request")).toEqual([]);

      await read();
      await userEvent.click(within(notice()).getByRole("button", { name: "Merge" }));
      await vi.waitFor(() => expect(sent("merge_held_request")).toHaveLength(1));
    },
  );

  it("says 1 commit, not 1 commits", () => {
    core();
    render(
      <MergeHoldNotice
        plane={PLANE}
        session={SESSION}
        asks={[hold({ look: "not-viewed", since_viewed: 1 })]}
      />,
    );
    expect(notice().textContent).toContain("1 commit since you looked.");
  });

  it("a PR this chat did not push is flagged (R-124)", () => {
    core();
    const { rerender } = render(
      <MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold({ pushed_here: true })]} />,
    );
    expect(notice().textContent).not.toContain("didn't push");

    rerender(
      <MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold({ pushed_here: false })]} />,
    );
    expect(notice().textContent).toContain(
      "This chat didn't push this head, by the pushes purlis recorded.",
    );
  });

  it("a head that moves redraws the merge row: its SHA, what was looked at, and the settle guard (R-125)", async () => {
    const sent = core();
    const { rerender } = render(
      <MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold({ look: "viewed" })]} />,
    );
    await confirming();

    rerender(
      <MergeHoldNotice
        plane={PLANE}
        session={SESSION}
        asks={[hold({ look: "not-viewed", since_viewed: 2 }, MOVED)]}
      />,
    );

    expect(notice().textContent).toContain("fedcba987654");
    expect(notice().textContent).toContain("2 commits since you looked.");
    expect(buttons()).toContain("Don't merge");
    // Asked once more at the new head, a Merge pressed at once does nothing: the question
    // drawn anew settles anew.
    await confirming();
    await userEvent.click(within(notice()).getByRole("button", { name: "Merge" }));
    expect(sent("merge_held_request")).toEqual([]);
    await read();
    await userEvent.click(within(notice()).getByRole("button", { name: "Merge" }));
    await vi.waitFor(() =>
      expect(sent("merge_held_request")).toEqual([expect.objectContaining({ head: MOVED })]),
    );
  });

  it("View changes marks the head SHA as viewed: it shows the request's changes at the row's head (R-302)", async () => {
    const sent = core();
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold()]} />);

    await userEvent.click(within(notice()).getByRole("button", { name: "View changes" }));

    await vi.waitFor(() =>
      expect(sent("view_held_merge")).toEqual([
        { plane: PLANE, session: SESSION, repo: "o/r", number: 12, head: HEAD },
      ]),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Changes of request 12 of o/r",
    });
    expect(dialog.textContent).toContain("+fn parse() {}");
    expect(dialog.textContent).toContain("At 0123456, into main");
    expect(sent("merge_held_request")).toEqual([]);
  });

  it("says why when the changes can't be shown, and marks nothing", async () => {
    mockIPC((cmd) => {
      if (cmd === "view_held_merge")
        throw new Error("the request moved to another head since its row was drawn");
      return null;
    });
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold()]} />);
    await userEvent.click(within(notice()).getByRole("button", { name: "View changes" }));
    await waitFor(() =>
      expect(notice().textContent).toContain(
        "the request moved to another head since its row was drawn",
      ),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("hands back the digest of every fact its row showed, which the press is bound to", async () => {
    const sent = core();
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold()]} />);
    await confirming();
    await read();
    await userEvent.click(within(notice()).getByRole("button", { name: "Merge" }));
    await vi.waitFor(() =>
      expect(sent("merge_held_request")).toEqual([
        {
          plane: PLANE,
          session: SESSION,
          repo: "o/r",
          number: 12,
          head: HEAD,
          ruleset: "none",
          digest: "d-passed",
        },
      ]),
    );
  });
});
