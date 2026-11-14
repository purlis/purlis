// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { MergeHoldNotice } from "./MergeHoldNotice";
import type { MergeShown, Shown } from "./bindings";
import { SETTLE_MS } from "./noticeSettle";

/** An answer whose sending rejects outright, where a test says so; the real one otherwise. */
const rejects = vi.hoisted(() => ({ with: undefined as Error | undefined }));
vi.mock("./asks", async (importOriginal) => {
  const real = await importOriginal<typeof import("./asks")>();
  return {
    ...real,
    answerTaken: (...args: Parameters<typeof real.answerTaken>) =>
      rejects.with === undefined ? real.answerTaken(...args) : Promise.reject(rejects.with),
  };
});

/**
 * **A held merge on its chat's pane** (T1-39; R-114, Q239): the hold its chat waits on is drawn
 * where the person looks at the chat, as well as in Asks. Merge is the app's own forge call and
 * asks once more before it acts; Don't merge merges nothing. Never a rule, so nothing on it is
 * remembered.
 */

afterEach(() => {
  rejects.with = undefined;
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const SESSION = 3;
const HEAD = "0123456789abcdef0123456789abcdef01234567";
/** When the forge was read for the hold, and when the press that asks once more read it again. */
const READ_AT = 1_763_000_000;
const REREAD_AT = READ_AT + 60;
const clock = (secs: number) =>
  new Date(secs * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

const FACTS: MergeShown = {
  title: "Add the parser",
  author: "octo",
  base: "main",
  checks: "PASSED",
  read_at: READ_AT,
  ruleset: "none",
};

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
  answer: { via: "merge-hold", repo: "o/r", number: 12, head: HEAD, facts: FACTS },
  risk: {
    class: "merge",
    why: "Merges request 12 of o/r at 0123456 into main, with no undo",
    confirm: "Merge request 12 of o/r at 0123456 into main?",
  },
  since: 1_000,
};

/** A core that records what the window sends; its re-read of a merge gives `gives` over the
 *  first read's facts, a minute later. */
function core(gives: Partial<MergeShown> = {}) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "reread_held_merge")
      return {
        head: (args as { head: string }).head,
        facts: { ...FACTS, read_at: REREAD_AT, ...gives },
      };
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

const notice = () => screen.getByRole("status", { name: "Merge held" });
/** The person reads the question asked once more for longer than a press of it waits. */
const read = () => new Promise((done) => setTimeout(done, SETTLE_MS + 50));
/** Presses Merge, and waits for it to ask once more. */
const confirming = async () => {
  await userEvent.click(screen.getByRole("button", { name: "Merge" }));
  await screen.findByRole("button", { name: "Cancel" });
};
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

    await confirming();
    expect(sent("merge_held_request")).toEqual([]);
    expect(notice().textContent).toContain("Merge request 12 of o/r at 0123456 into main?");
    expect(buttons()).toEqual(["Merge", "Cancel"]);

    await read();
    await userEvent.click(screen.getByRole("button", { name: "Merge" }));
    await vi.waitFor(() =>
      expect(sent("merge_held_request")).toEqual([
        { plane: PLANE, session: SESSION, repo: "o/r", number: 12, head: HEAD, ruleset: "none" },
      ]),
    );
  });

  it("takes the question back on Cancel, and sends nothing", async () => {
    const sent = core();
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await confirming();
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
      answer: { via: "merge-hold", repo: "o/r", number: 12, head: "89abcdef", facts: FACTS },
      risk: {
        class: "merge",
        why: "Merges request 12 of o/r at 89abcde into main, with no undo",
        confirm: "Merge request 12 of o/r at 89abcde into main?",
      },
    };
    const { rerender } = render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await confirming();

    rerender(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[moved]} />);

    expect(buttons()).toEqual(["Merge", "Don't merge"]);
    expect(sent("merge_held_request")).toEqual([]);
  });

  it("asks once more in purlis's own sentence when the hold arrives without its risk, and never merges on the first press (Q268)", async () => {
    const sent = core();
    const bare: Shown = { ...hold, risk: null };
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[bare]} />);

    await confirming();
    expect(sent("merge_held_request")).toEqual([]);
    expect(notice().textContent).toContain("Merge request 12 of o/r at 0123456? There is no undo.");
    expect(buttons()).toEqual(["Merge", "Cancel"]);

    await read();
    await userEvent.click(screen.getByRole("button", { name: "Merge" }));
    await vi.waitFor(() =>
      expect(sent("merge_held_request")).toEqual([
        { plane: PLANE, session: SESSION, repo: "o/r", number: 12, head: HEAD, ruleset: "none" },
      ]),
    );
  });

  it("takes an open confirmation back when the ask's facts refresh under it (Q268, Q248)", async () => {
    const sent = core();
    const refreshed: Shown = {
      ...hold,
      answer: {
        via: "merge-hold",
        repo: "o/r",
        number: 12,
        head: HEAD,
        facts: { ...FACTS, checks: "FAILED" },
      },
    };
    const { rerender } = render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await confirming();
    expect(buttons()).toEqual(["Merge", "Cancel"]);

    rerender(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[refreshed]} />);

    expect(buttons()).toEqual(["Merge", "Don't merge"]);
    expect(sent("merge_held_request")).toEqual([]);
  });

  it("says so when sending the answer rejects, and merges nothing (Q268)", async () => {
    const sent = core();
    rejects.with = new Error("the core went away");
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await userEvent.click(screen.getByRole("button", { name: "Don't merge" }));
    await waitFor(() =>
      expect(notice().textContent).toContain("purlis could not send it: Error: the core went away"),
    );
    expect(sent("merge_held_request")).toEqual([]);
  });

  it("shows the checks as of the forge's read, and re-reads it before it asks once more (Q248)", async () => {
    const sent = core();
    const { rerender } = render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    expect(notice().textContent).toContain(`Checks PASSED as of ${clock(READ_AT)}`);

    await confirming();

    expect(sent("reread_held_merge")).toEqual([
      { plane: PLANE, session: SESSION, repo: "o/r", number: 12, head: HEAD },
    ]);
    // The registry's next reading carries the re-read: the question stands, its time moves on.
    rerender(
      <MergeHoldNotice
        plane={PLANE}
        session={SESSION}
        asks={[
          {
            ...hold,
            answer: { ...hold.answer, facts: { ...FACTS, read_at: REREAD_AT } } as Shown["answer"],
          },
        ]}
      />,
    );
    expect(buttons()).toEqual(["Merge", "Cancel"]);
    expect(notice().textContent).toContain(`Checks PASSED as of ${clock(REREAD_AT)}`);
  });

  it("asks once more only of what the re-read found, never of what it replaced (R-150)", async () => {
    const sent = core({ ruleset: "protects" });
    const { rerender } = render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await userEvent.click(screen.getByRole("button", { name: "Merge" }));
    await vi.waitFor(() => expect(sent("reread_held_merge")).toHaveLength(1));
    // Still drawn as it was read before: no question is asked of it.
    expect(buttons()).toEqual(["Merge", "Don't merge"]);

    rerender(
      <MergeHoldNotice
        plane={PLANE}
        session={SESSION}
        asks={[
          {
            ...hold,
            answer: {
              ...hold.answer,
              facts: { ...FACTS, read_at: REREAD_AT, ruleset: "protects" },
            } as Shown["answer"],
          },
        ]}
      />,
    );
    expect(buttons()).toEqual(["Merge", "Cancel"]);
    expect(notice().textContent).toContain("A ruleset protects main.");
    await read();
    await userEvent.click(screen.getByRole("button", { name: "Merge" }));
    await vi.waitFor(() =>
      expect(sent("merge_held_request")).toEqual([
        expect.objectContaining({ ruleset: "protects" }),
      ]),
    );
  });

  it("shows the title as text the chat may have written, never as markup (Q248)", () => {
    core();
    const title = "**Ship it** <img src=x onerror=alert(1)> [docs](https://evil.example)";
    render(
      <MergeHoldNotice
        plane={PLANE}
        session={SESSION}
        asks={[
          { ...hold, answer: { ...hold.answer, facts: { ...FACTS, title } } as Shown["answer"] },
        ]}
      />,
    );
    expect(notice().textContent).toContain(`Title, as the chat may have written it: ${title}`);
    expect(notice().querySelector("img, a[href], strong")).toBeNull();
  });

  it("merges nothing on a double click: the question asked once more waits to settle", async () => {
    const sent = core();
    render(<MergeHoldNotice plane={PLANE} session={SESSION} asks={[hold]} />);
    await confirming();

    await userEvent.click(screen.getByRole("button", { name: "Merge" }));

    expect(sent("merge_held_request")).toEqual([]);
    expect(buttons()).toEqual(["Merge", "Cancel"]);
    await read();
    await userEvent.click(screen.getByRole("button", { name: "Merge" }));
    await vi.waitFor(() => expect(sent("merge_held_request")).toHaveLength(1));
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
