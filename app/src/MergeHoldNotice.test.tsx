// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { MergeHoldNotice } from "./MergeHoldNotice";
import type { Shown } from "./bindings";

/**
 * **A held merge on its chat's pane** (T1-39; R-114, Q239): the hold its chat waits on is drawn
 * where the person looks at the chat, as well as in Asks. Merge is the app's own forge call and
 * asks once more before it acts; Don't merge merges nothing. Never a rule, so nothing on it is
 * remembered.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const SESSION = 3;
const HEAD = "0123456789abcdef0123456789abcdef01234567";

const hold: Shown = {
  session: SESSION,
  ask: `merge:${SESSION}:o/r#12@${HEAD}`,
  says: "Merge request 12 of o/r at 0123456789ab? There is no undo.",
  options: [
    { id: "merge", label: "Merge", allows: true },
    { id: "dont-merge", label: "Don't merge", allows: false },
  ],
  source: "merge-hold",
  chain: ["steward 3"],
  answer: { via: "merge-hold", repo: "o/r", number: 12, head: HEAD },
  risk: {
    class: "merge",
    why: "Merges request 12 of o/r at 0123456 into main, with no undo",
    confirm: "Merge request 12 of o/r at 0123456 into main?",
  },
  since: 1_000,
};

/** A core that records what the window sends. */
function core() {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

const notice = () => screen.getByRole("status", { name: "Merge held" });
const buttons = () => screen.getAllByRole("button").map((one) => one.textContent);

describe("a held merge on its chat's pane", () => {
  it("says what it would merge and its risk, and offers Merge and Don't merge", () => {
    const sent = core();
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    expect(notice().textContent).toContain(
      "Merge request 12 of o/r at 0123456789ab? There is no undo.",
    );
    expect(notice().textContent).toContain(
      "Merges request 12 of o/r at 0123456 into main, with no undo",
    );
    expect(buttons()).toEqual(["Merge", "Don't merge"]);
    expect(sent("merge_held_request")).toEqual([]);
  });

  it("draws nothing for another chat's hold", () => {
    core();
    render(<MergeHoldNotice plane={PLANE} session={4} asks={[hold]} />);
    expect(screen.queryByRole("status", { name: "Merge held" })).toBeNull();
  });

  it("asks once more before Merge acts, then merges the request at the head it was raised at", async () => {
    const sent = core();
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);

    await userEvent.click(screen.getByRole("button", { name: "Merge" }));
    expect(sent("merge_held_request")).toEqual([]);
    expect(notice().textContent).toContain("Merge request 12 of o/r at 0123456 into main?");
    expect(buttons()).toEqual(["Merge", "Cancel"]);

    await userEvent.click(screen.getByRole("button", { name: "Merge" }));
    await vi.waitFor(() =>
      expect(sent("merge_held_request")).toEqual([
        { plane: PLANE, session: SESSION, repo: "o/r", number: 12, head: HEAD },
      ]),
    );
  });

  it("takes the question back on Cancel, and sends nothing", async () => {
    const sent = core();
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await userEvent.click(screen.getByRole("button", { name: "Merge" }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(buttons()).toEqual(["Merge", "Don't merge"]);
    expect(sent("merge_held_request")).toEqual([]);
  });

  it("sends Don't merge, which merges nothing", async () => {
    const sent = core();
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await userEvent.click(screen.getByRole("button", { name: "Don't merge" }));
    await vi.waitFor(() =>
      expect(sent("dont_merge_held_request")).toEqual([
        { plane: PLANE, session: SESSION, repo: "o/r", number: 12, head: HEAD },
      ]),
    );
    expect(sent("merge_held_request")).toEqual([]);
  });

  it("carries no confirmation over to the hold at a moved head (Q242)", async () => {
    const sent = core();
    const moved: Shown = {
      ...hold,
      ask: `merge:${SESSION}:o/r#12@89abcdef`,
      answer: { via: "merge-hold", repo: "o/r", number: 12, head: "89abcdef" },
      risk: {
        class: "merge",
        why: "Merges request 12 of o/r at 89abcde into main, with no undo",
        confirm: "Merge request 12 of o/r at 89abcde into main?",
      },
    };
    const { rerender } = render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await userEvent.click(screen.getByRole("button", { name: "Merge" }));

    rerender(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[moved]} />);

    expect(buttons()).toEqual(["Merge", "Don't merge"]);
    expect(sent("merge_held_request")).toEqual([]);
  });

  it("offers nothing to remember (R-143)", () => {
    core();
    render(
      <MergeHoldNotice
        plane={PLANE}
        session={SESSION}
        asks={[
          {
            ...hold,
            rule: { request: "r", offer: { answered: "merge", times: 2, options: ["merge"] } },
          },
        ]}
      />,
    );
    expect(buttons()).toEqual(["Merge", "Don't merge"]);
    expect(screen.queryByRole("group", { name: /^Make a rule/ })).toBeNull();
  });
});
