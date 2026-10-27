// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { RepoInstructionsTab } from "./RepoInstructionsTab";
import type { InstructionFile } from "./bindings";

/**
 * **A workspace whose repos hold no agent instructions says so** (DS-3 #626, FR-19 #614): the
 * tab's empty state names the files it looks for and reads again on a press, instead of an
 * empty list under a sentence that says the repo has some.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const AGENTS: InstructionFile = {
  repo: "svc",
  file: "AGENTS.md",
  text: "Run make check.\n",
  standing: { kind: "offered", caution: null },
};

/** The core: `repo_instructions` answers each of `reads` in turn, the last one again after. */
function core(reads: InstructionFile[][]) {
  let read = 0;
  const asked = vi.fn();
  mockIPC((cmd) => {
    if (cmd !== "repo_instructions") return null;
    asked();
    return reads[Math.min(read++, reads.length - 1)];
  });
  return asked;
}

describe("a workspace whose repos hold no agent instructions", () => {
  it("says what the tab looks for, and offers nothing to add", async () => {
    core([[]]);
    render(<RepoInstructionsTab plane={PLANE} workspace="alpha" onClose={vi.fn()} />);

    const empty = await screen.findByTestId("repo-instructions-empty");
    expect(
      within(empty).getByText("No instructions for AI agents in this workspace's repos"),
    ).toBeInTheDocument();
    expect(empty).toHaveTextContent(/CLAUDE\.md/);
    expect(empty).toHaveTextContent(/AGENTS\.md/);
    expect(screen.queryByRole("button", { name: "Add to memory" })).toBeNull();
    expect(screen.queryByText(/This repo has instructions for AI agents/)).toBeNull();
  });

  it("reads the repos again on Read again, and offers a file added since", async () => {
    const asked = core([[], [AGENTS]]);
    render(<RepoInstructionsTab plane={PLANE} workspace="alpha" onClose={vi.fn()} />);
    const empty = await screen.findByTestId("repo-instructions-empty");
    const before = asked.mock.calls.length;

    await userEvent.click(within(empty).getByRole("button", { name: "Read again" }));

    expect(await screen.findByText("svc/AGENTS.md")).toBeInTheDocument();
    expect(asked.mock.calls.length).toBeGreaterThan(before);
    expect(screen.queryByTestId("repo-instructions-empty")).toBeNull();
  });
});

/**
 * **Add to memory carries no emphasis** (#2591, D-2591-1): it saves a repository's own text into
 * memory, so the press is a trust decision. It is drawn as plainly as Not now beside it, and,
 * being no grant, it is not the `grants` variant either.
 */
describe("the answers under a repository's instruction files", () => {
  it("draws Add to memory and Not now with equal weight, in their order", async () => {
    core([[AGENTS]]);
    render(<RepoInstructionsTab plane={PLANE} workspace="alpha" onClose={vi.fn()} />);

    const add = await screen.findByRole("button", { name: "Add to memory" });
    const notNow = screen.getByRole("button", { name: "Not now" });

    for (const answer of [add, notNow]) {
      expect(answer).toHaveAttribute("data-scope", "button");
      expect(answer).not.toHaveAttribute("data-variant", "primary");
      expect(answer).not.toHaveAttribute("data-variant", "grants");
    }
    expect(add.className).toBe(notNow.className);
    expect(add.compareDocumentPosition(notNow) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});

/**
 * **The preview marks what the core's one rule hides** (SK-51, #3202): the very characters that
 * untick a file, each as `⟦U+XXXX⟧`, while a joiner inside an emoji and the file's own line ends
 * read as text.
 */
describe("a repository's instruction file holding a character that draws as nothing", () => {
  it("draws each as the core's mark, and an emoji's joiner as text", async () => {
    core([
      [
        {
          ...AGENTS,
          text: "Runㅤ make️ check \u{1F468}‍\u{1F469}.\nThen‮ ship.\n",
          standing: { kind: "offered", caution: "It holds characters that draw as nothing" },
        },
      ],
    ]);
    render(<RepoInstructionsTab plane={PLANE} workspace="alpha" onClose={vi.fn()} />);

    await screen.findByText("svc/AGENTS.md");
    const marks = Array.from(document.querySelectorAll("mark.invisible-char")).map(
      (one) => one.textContent,
    );
    expect(marks).toEqual(["⟦U+3164⟧", "⟦U+FE0F⟧", "⟦U+202E⟧"]);
    expect(document.querySelector("pre.repo-instruction-text")?.textContent).toBe(
      "Run⟦U+3164⟧ make⟦U+FE0F⟧ check \u{1F468}‍\u{1F469}.\nThen⟦U+202E⟧ ship.\n",
    );
  });
});
