// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { BranchChanged, PlaneId, TurnChanged, WhatChanged } from "../bindings";
import { PieceDiffTab } from "./PieceDiff";
import { settleJump, useJumpAsks, type Pending } from "../fileJump";
import { forgetYourEditor, setYourEditor } from "../yourEditor";

const PLANE = "/plane" as unknown as PlaneId;
const CUT = { workspace: "alpha", repo: "svc", piece: "fix-it" };

/** The core, answering `what_changed` with `answer`, or refusing with it when it is a string.
 *  An `open_in_your_editor` is noted as `editor:<path>:<line>:<editor>`. */
const NO_CHECKS: WhatChanged["checks"] = {
  sets: [],
  onLines: [],
  outside: [],
  elsewhere: [],
  moreOnLines: 0,
  moreOutside: 0,
  moreElsewhere: 0,
  trouble: null,
};

function core(answer: WhatChanged | string) {
  const asked: string[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "open_in_your_editor") {
      const a = args as Record<string, unknown>;
      asked.push(`editor:${String(a.path)}:${String(a.line)}:${String(a.editor)}`);
      return null;
    }
    if (cmd !== "what_changed") throw new Error(`unexpected ${cmd}`);
    const a = args as Record<string, string>;
    asked.push(`${a.workspace}/${a.repo}/${a.piece}:${a.path}`);
    if (typeof answer === "string") throw answer;
    return answer;
  });
  return asked;
}

const changed = (diff: WhatChanged["diff"]): WhatChanged => ({
  mark: "changed",
  from: null,
  uncommitted: true,
  base: "main",
  diff,
  stamp: "drawn",
  checks: NO_CHECKS,
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetYourEditor();
});

/** The core, answering both comparisons: what each was asked is noted by its command, and a
 *  turn asked by its pick (`turn:latest` or `turn:<chat>/<number>`). */
function coreWithTurns(base: WhatChanged, turn: TurnChanged | ((number: number) => TurnChanged)) {
  const asked: string[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "what_changed") {
      asked.push(cmd);
      return base;
    }
    if (cmd === "turn_changed") {
      const pick = (args as { turn: { chat: string; number: number } | null }).turn;
      asked.push(pick === null ? "turn:latest" : `turn:${pick.chat}/${pick.number}`);
      return typeof turn === "function" ? turn(pick?.number ?? 3) : turn;
    }
    throw new Error(`unexpected ${cmd}`);
  });
  return asked;
}

const CHAT = "01J9CHAT0000000000000000TN";

describe("one turn beside against base (#5405, #5408)", () => {
  const whole = changed({
    kind: "text",
    base: "one\n",
    head: "one\ntwo\nthree\n",
    hunks: [{ oldStart: 1, oldLines: 0, newStart: 2, newLines: 2 }],
  });
  const turnOf = (number: number, line: string, at: number): TurnChanged => ({
    kind: "of",
    chat: CHAT,
    number,
    turns: [3, 2, 1],
    file: {
      kind: "changed",
      shown: {
        ...changed({
          kind: "text",
          base: "one\ntwo\n",
          head: `one\ntwo\n${line}\n`,
          hunks: [{ oldStart: 2, oldLines: 0, newStart: at, newLines: 1 }],
        }),
        base: `the start of turn ${number}`,
        stamp: null,
      },
    },
  });
  const ofTheTurn = turnOf(3, "three", 3);
  const turnOption = () => screen.getByRole("button", { name: /Turn$/ });

  it("opens against base, and a press draws the latest turn's own lines", async () => {
    const asked = coreWithTurns(whole, ofTheTurn);
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    expect(await screen.findByText("against main")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Against base" })).toBeChecked();
    expect(asked).toEqual(["what_changed"]);

    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));

    expect(await screen.findByText("against the start of turn 3")).toBeInTheDocument();
    await waitFor(() =>
      expect([...document.querySelectorAll(".cm-changedLine")].map((l) => l.textContent)).toEqual([
        "three",
      ]),
    );
    expect(asked).toEqual(["what_changed", "turn:latest"]);
  });

  it("lists the chat's turns newest first, and opens on the latest", async () => {
    coreWithTurns(whole, ofTheTurn);
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await screen.findByText("against main");
    expect(screen.queryByRole("button", { name: /Turn$/ })).toBeNull();

    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));
    await screen.findByText("against the start of turn 3");
    expect(turnOption()).toHaveTextContent("Turn 3, the latest");

    await userEvent.click(turnOption());
    const listed = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(listed.map((option) => option.textContent)).toEqual([
      "Turn 3, the latest",
      "Turn 2",
      "Turn 1",
    ]);
  });

  it("steps to an earlier turn by the keyboard, and draws that turn's own lines", async () => {
    const asked = coreWithTurns(whole, (number) =>
      number === 2 ? turnOf(2, "two and a half", 3) : ofTheTurn,
    );
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await screen.findByText("against main");
    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));
    await screen.findByText("against the start of turn 3");

    turnOption().focus();
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("listbox");
    await userEvent.keyboard("{ArrowDown}{Enter}");

    expect(await screen.findByText("against the start of turn 2")).toBeInTheDocument();
    await waitFor(() =>
      expect([...document.querySelectorAll(".cm-changedLine")].map((l) => l.textContent)).toEqual([
        "two and a half",
      ]),
    );
    expect(asked).toEqual(["what_changed", "turn:latest", `turn:${CHAT}/2`]);
    expect(turnOption()).toHaveTextContent("Turn 2");
  });

  it("goes back to the latest turn each time one turn is chosen again", async () => {
    const asked = coreWithTurns(whole, (number) =>
      number === 2 ? turnOf(2, "two and a half", 3) : ofTheTurn,
    );
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await screen.findByText("against main");
    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));
    await screen.findByText("against the start of turn 3");
    await userEvent.click(turnOption());
    await userEvent.click(await screen.findByRole("option", { name: "Turn 2" }));
    await screen.findByText("against the start of turn 2");

    await userEvent.click(screen.getByRole("radio", { name: "Against base" }));
    await screen.findByText("against main");
    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));

    expect(await screen.findByText("against the start of turn 3")).toBeInTheDocument();
    expect(asked.at(-1)).toBe("turn:latest");
  });

  it("offers no Discard on a turn's hunks, which a discard would take for the base's", async () => {
    coreWithTurns({ ...whole, stamp: "drawn" }, ofTheTurn);
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    expect(await screen.findByRole("button", { name: /^Discard the change/ })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));

    await screen.findByText("against the start of turn 3");
    expect(screen.queryByRole("button", { name: /^Discard/ })).toBeNull();
  });

  it("goes back to the branch's whole change by the arrow keys", async () => {
    coreWithTurns(whole, ofTheTurn);
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await screen.findByText("against main");
    screen.getByRole("radio", { name: "Against base" }).focus();

    await userEvent.keyboard("{ArrowRight}");
    expect(await screen.findByText("against the start of turn 3")).toBeInTheDocument();
    await userEvent.keyboard("{ArrowLeft}");
    expect(await screen.findByText("against main")).toBeInTheDocument();
  });

  it("says why a branch has no turn to show, never an empty diff", async () => {
    const why = "A shell runs a program that does not report its turns to purlis.";
    coreWithTurns(whole, { kind: "no-turn", why });
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await screen.findByText("against main");

    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(why);
    expect(document.querySelector(".cm-mergeView")).toBeNull();
    expect(screen.queryByRole("button", { name: /Turn$/ })).toBeNull();
  });

  it("says a file the turn left alone by the turn's number", async () => {
    coreWithTurns(whole, {
      kind: "of",
      chat: CHAT,
      number: 3,
      turns: [3, 2, 1],
      file: { kind: "not-this-file" },
    });
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await screen.findByText("against main");

    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "Turn 3 did not change lib.rs",
    );
  });

  it("says a turn that changed nothing, and keeps the picker to step past it", async () => {
    coreWithTurns(whole, {
      kind: "of",
      chat: CHAT,
      number: 2,
      turns: [3, 2, 1],
      file: { kind: "nothing" },
    });
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await screen.findByText("against main");

    await userEvent.click(screen.getByRole("radio", { name: "One turn" }));

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "Turn 2 changed no file",
    );
    expect(turnOption()).toHaveTextContent("Turn 2");
  });
});

describe("what changed in one file (FM-11)", () => {
  it("draws the file against the branch's base, with git's hunks marked", async () => {
    const asked = core(
      changed({
        kind: "text",
        base: "one\ntwo\n",
        head: "one\nTWO\n",
        hunks: [{ oldStart: 2, oldLines: 1, newStart: 2, newLines: 1 }],
      }),
    );
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);

    await waitFor(() => expect(document.querySelector(".cm-mergeView")).not.toBeNull());
    const changedLines = [...document.querySelectorAll(".cm-changedLine")].map(
      (line) => line.textContent,
    );
    expect(changedLines).toEqual(["two", "TWO"]);
    expect(screen.getByText("against main")).toBeInTheDocument();
    expect(asked).toEqual(["alpha/svc/fix-it:src/lib.rs"]);
  });

  it("says the core's refusal as a sentence, never as an empty diff", async () => {
    core("'src/same.rs' is not a file this branch changed against its base");
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/same.rs" />);

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "'src/same.rs' is not a file this branch changed against its base",
    );
    expect(screen.queryByTestId("merge-viewer")).toBeNull();
  });

  it("says a binary file and a large one in a sentence", async () => {
    core(changed({ kind: "binary" }));
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="logo.png" />);

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "logo.png is a binary file, so its lines are not compared",
    );
    cleanup();

    core(changed({ kind: "too-large", bytes: 3 * 1024 * 1024 }));
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="dump.sql" />);

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "dump.sql is 3 MiB, past what the comparison draws (2 MiB)",
    );
    expect(screen.queryByTestId("merge-viewer")).toBeNull();
  });

  it("says a change with no line in it rather than drawing two equal sides", async () => {
    core(changed({ kind: "text", base: "same\n", head: "same\n", hunks: [] }));
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="run.sh" />);

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "No line of run.sh differs from main",
    );
    expect(screen.queryByTestId("merge-viewer")).toBeNull();
  });

  it("says a pure rename by the name it moved from", async () => {
    core({
      ...changed({ kind: "text", base: "same\n", head: "same\n", hunks: [] }),
      mark: "renamed",
      from: "src/old.rs",
    });
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/new.rs" />);

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "Only its name changed: moved from src/old.rs",
    );
    expect(screen.getByTestId("piece-diff-trouble")).not.toHaveTextContent("its mode");
    expect(screen.queryByTestId("merge-viewer")).toBeNull();
  });

  it("says a new empty file is added and empty", async () => {
    core({
      ...changed({ kind: "text", base: "", head: "", hunks: [] }),
      mark: "added",
    });
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/empty.rs" />);

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "empty.rs was added, and it is empty",
    );
    expect(screen.getByTestId("piece-diff-trouble")).not.toHaveTextContent("its mode");
  });

  it("offers your editor beside a file it does not draw, as its sentence says", async () => {
    core(changed({ kind: "binary" }));
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="logo.png" />);

    expect(await screen.findByTestId("piece-diff-trouble")).toHaveTextContent(
      "Open in your editor, at the top of this tab",
    );
    expect(
      screen.getByRole("button", { name: "Open in your editor at line 1" }),
    ).toBeInTheDocument();
  });

  it("names where a renamed file came from, and compares again on request", async () => {
    const asked = core({
      ...changed({
        kind: "text",
        base: "a\n",
        head: "b\n",
        hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 }],
      }),
      mark: "renamed",
      from: "src/old.rs",
    });
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/new.rs" />);

    expect(await screen.findByText("against main, moved from src/old.rs")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Compare again" }));

    await waitFor(() => expect(asked).toHaveLength(2));
  });
});

describe("what changed, to a screen reader (#1189)", () => {
  it("says the comparison is being read as a status", () => {
    mockIPC(() => new Promise(() => {}));
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);

    expect(screen.getByRole("status")).toHaveTextContent("Comparing src/lib.rs…");
  });

  it("says a refusal and a change with no line in it as a status", async () => {
    core("'src/same.rs' is not a file this branch changed against its base");
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/same.rs" />);

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent(
        "'src/same.rs' is not a file this branch changed against its base",
      ),
    );
    cleanup();

    core(changed({ kind: "text", base: "same\n", head: "same\n", hunks: [] }));
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="run.sh" />);

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("No line of run.sh differs from main"),
    );
  });

  it("names the merge view after the file and what it is compared against", async () => {
    core(
      changed({
        kind: "text",
        base: "one\n",
        head: "ONE\n",
        hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 }],
      }),
    );
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);

    expect(
      await screen.findByRole("group", { name: "What changed in src/lib.rs against main" }),
    ).toHaveAttribute("data-testid", "merge-viewer");
    expect(screen.queryByRole("status")).toBeNull();
  });
});

describe("what changed, read again when the branch moves (#1189)", () => {
  const TEXT = changed({
    kind: "text",
    base: "one\n",
    head: "ONE\n",
    hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 }],
  });

  /** The core, its `what_changed` answered by hand: each ask waits for `answer()`. */
  function slowCore() {
    const waiting: (() => void)[] = [];
    let asks = 0;
    mockIPC(
      (cmd) => {
        if (cmd !== "what_changed") throw new Error(`unexpected ${cmd}`);
        asks += 1;
        return new Promise((done) => waiting.push(() => done(TEXT)));
      },
      { shouldMockEvents: true },
    );
    return {
      asks: () => asks,
      answer: () => act(async () => waiting.splice(0).forEach((done) => done())),
    };
  }

  const moved = (piece: string | null, workspace = "alpha") =>
    act(() =>
      emit("branch-changed", {
        branches: [{ plane: PLANE, workspace, repo: "svc", piece }],
      } satisfies BranchChanged),
    );

  it("reads again when its own branch moved, and keeps the comparison drawn meanwhile", async () => {
    const core = slowCore();
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await waitFor(() => expect(core.asks()).toBe(1));
    await core.answer();
    await screen.findByTestId("merge-viewer");

    await moved("fix-it");

    await waitFor(() => expect(core.asks()).toBe(2));
    expect(screen.getByTestId("merge-viewer")).toBeInTheDocument();
  });

  it("marks the comparison drawn as being compared again until the new one comes", async () => {
    const core = slowCore();
    const { container } = render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await waitFor(() => expect(core.asks()).toBe(1));
    await core.answer();
    await screen.findByTestId("merge-viewer");
    expect(screen.queryByRole("status")).toBeNull();
    expect(container.querySelector(".piece-file")).toHaveAttribute("aria-busy", "false");

    await moved("fix-it");

    await waitFor(() => expect(core.asks()).toBe(2));
    expect(screen.getByRole("status")).toHaveTextContent("Comparing src/lib.rs again…");
    expect(container.querySelector(".piece-file")).toHaveAttribute("aria-busy", "true");
    // D-1189-3: the old comparison's line buttons stay usable while it is read again.
    expect(screen.getByRole("button", { name: "Open at line 1" })).toBeEnabled();

    await core.answer();

    await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
    expect(container.querySelector(".piece-file")).toHaveAttribute("aria-busy", "false");
    expect(screen.getByTestId("merge-viewer")).toBeInTheDocument();
  });

  it("is not marked as compared again on Compare again, which reads from Comparing…", async () => {
    const core = slowCore();
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await waitFor(() => expect(core.asks()).toBe(1));
    await core.answer();
    await screen.findByTestId("merge-viewer");

    await userEvent.click(screen.getByRole("button", { name: "Compare again" }));

    expect(screen.getByRole("status")).toHaveTextContent("Comparing src/lib.rs…");
    expect(screen.queryByText(/again…/)).toBeNull();
  });

  it("does not read again for another branch", async () => {
    const core = slowCore();
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await waitFor(() => expect(core.asks()).toBe(1));
    await core.answer();

    await moved("other");
    await moved(null);
    await moved("fix-it", "beta");
    await act(async () => {
      await new Promise((done) => setTimeout(done, 20));
    });

    expect(core.asks()).toBe(1);
  });

  it("reads once more after a read in flight, however often the branch moved meanwhile", async () => {
    const core = slowCore();
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    await waitFor(() => expect(core.asks()).toBe(1));

    await moved("fix-it");
    await moved("fix-it");
    await moved("fix-it");
    expect(core.asks()).toBe(1);
    await core.answer();

    await waitFor(() => expect(core.asks()).toBe(2));
    await core.answer();
    await act(async () => {
      await new Promise((done) => setTimeout(done, 20));
    });
    expect(core.asks()).toBe(2);
  });
});

describe("a changed line opened in the file tab (#984)", () => {
  /** The jumps the window heard, as `App` hears them. */
  function Window({ heard }: { heard: Pending[] }) {
    useJumpAsks((jump) => {
      heard.push(jump);
    });
    return null;
  }
  const heard: Pending[] = [];
  afterEach(() => {
    for (const jump of heard.splice(0)) settleJump(jump.at);
  });

  const twoHunks = changed({
    kind: "text",
    base: "a\nb\nc\nd\ne\nf\ng\nh\n",
    head: "a\nB\nc\nd\ne\nf\nG\nh\nI\n",
    hunks: [
      { oldStart: 2, oldLines: 1, newStart: 2, newLines: 1 },
      { oldStart: 7, oldLines: 1, newStart: 7, newLines: 3 },
    ],
  });

  it("offers each change's first line on the head's side, and jumps there", async () => {
    core(twoHunks);
    render(
      <>
        <Window heard={heard} />
        <PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />
      </>,
    );

    const changes = await screen.findByRole("navigation", { name: "Changes in src/lib.rs" });
    expect([...changes.querySelectorAll("button")].map((button) => button.textContent)).toEqual([
      "Open at line 2",
      "Open in your editor at line 2",
      "Discard",
      "Open at line 7",
      "Open in your editor at line 7",
      "Discard",
    ]);
    await userEvent.click(screen.getByRole("button", { name: "Open at line 7" }));

    expect(heard.map(({ plane, place, path, line }) => ({ plane, place, path, line }))).toEqual([
      { plane: PLANE, place: CUT, path: "src/lib.rs", line: 7 },
    ]);
  });

  it("offers the first twenty changes and counts the rest", async () => {
    const head = Array.from({ length: 100 }, (_, i) => `line ${i + 1}`).join("\n");
    core(
      changed({
        kind: "text",
        base: head,
        head,
        hunks: Array.from({ length: 25 }, (_, i) => ({
          oldStart: i * 4 + 1,
          oldLines: 1,
          newStart: i * 4 + 1,
          newLines: 1,
        })),
      }),
    );
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="big.rs" />);

    const changes = await screen.findByRole("navigation", { name: "Changes in big.rs" });
    expect(changes).toHaveTextContent("25 changes, the first 20 here");
    expect(within(changes).getAllByRole("button", { name: /^Open at line/ })).toHaveLength(20);
    expect(
      within(changes).getAllByRole("button", { name: /^Open in your editor at line/ }),
    ).toHaveLength(20);
  });

  it("opens each change in your editor at its own line (#1043)", async () => {
    setYourEditor("zed");
    const asked = core(twoHunks);
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);

    const changes = await screen.findByRole("navigation", { name: "Changes in src/lib.rs" });
    await userEvent.click(
      within(changes).getByRole("button", { name: "Open in your editor at line 7" }),
    );

    await waitFor(() => expect(asked).toContain("editor:src/lib.rs:7:zed"));
    expect(asked).not.toContain("editor:src/lib.rs:2:zed");
  });

  it("says beside a change that no editor is chosen, and links to where one is", async () => {
    core(twoHunks);
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);

    const changes = await screen.findByRole("navigation", { name: "Changes in src/lib.rs" });
    await userEvent.click(
      within(changes).getByRole("button", { name: "Open in your editor at line 2" }),
    );

    expect(within(changes).getByRole("status")).toHaveTextContent(
      "Choose your editor in Settings first.",
    );
    expect(within(changes).getByRole("button", { name: "Choose your editor" })).toBeInTheDocument();
  });

  it("offers no line of a deleted file, which the branch no longer has, only its Discard", async () => {
    core({
      ...changed({
        kind: "text",
        base: "gone\n",
        head: "",
        hunks: [{ oldStart: 1, oldLines: 1, newStart: 0, newLines: 0 }],
      }),
      mark: "deleted",
    });
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/old.rs" />);

    await screen.findByTestId("merge-viewer");
    const changes = screen.getByRole("navigation", { name: "Changes in src/old.rs" });
    expect([...changes.querySelectorAll("button")].map((button) => button.textContent)).toEqual([
      "Discard",
    ]);
  });
});

describe("discarding a hunk (#5406)", () => {
  const TEXT: WhatChanged["diff"] = {
    kind: "text",
    base: "one\ntwo\nthree\nfour\n",
    head: "ONE\ntwo\nthree\nFOUR\n",
    hunks: [
      { oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 },
      { oldStart: 4, oldLines: 1, newStart: 4, newLines: 1 },
    ],
  };
  /** The core: `what_changed` answers `drawn`, and each `discard_hunk` the next of `answers`. */
  function discarding(drawn: WhatChanged, answers: readonly unknown[]) {
    const asked: Record<string, unknown>[] = [];
    const left = [...answers];
    let reads = 0;
    mockIPC((cmd, args) => {
      if (cmd === "what_changed") {
        reads += 1;
        return drawn;
      }
      if (cmd !== "discard_hunk") throw new Error(`unexpected ${cmd}`);
      asked.push(args as Record<string, unknown>);
      return left.shift();
    });
    return { asked, reads: () => reads };
  }
  const opened = async () => {
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="src/lib.rs" />);
    return screen.findByRole("button", { name: "Discard the change at line 4" });
  };

  it("takes the hunk out of the diff at once, from what the core answers", async () => {
    const left: WhatChanged["diff"] = {
      kind: "text",
      base: "one\ntwo\nthree\nfour\n",
      head: "ONE\ntwo\nthree\nfour\n",
      hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 }],
    };
    const { asked, reads } = discarding(changed(TEXT), [
      { kind: "discarded", now: changed(left), unrecorded: null },
    ]);
    const control = await opened();

    await userEvent.click(control);

    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Discard the change at line 4" })).toBeNull(),
    );
    expect(screen.getByRole("button", { name: "Discard the change at line 1" })).toBeEnabled();
    expect(asked).toEqual([
      expect.objectContaining({
        path: "src/lib.rs",
        target: { kind: "hunk", hunk: { oldStart: 4, oldLines: 1, newStart: 4, newLines: 1 } },
        against: { kind: "opened", stamp: "drawn" },
        busyToo: false,
      }),
    ]);
    expect(reads()).toBe(1);
  });

  it("warns of a chat that is not idle by name and state, and goes on only at the operator's word", async () => {
    const { asked } = discarding(changed(TEXT), [
      { kind: "busy", chats: [{ name: "Fix the parser", state: "is running" }] },
      { kind: "discarded", now: changed(TEXT), unrecorded: null },
    ]);
    await userEvent.click(await opened());

    const warned = await screen.findByRole("alertdialog", {
      name: "Fix the parser is running",
    });
    expect(asked).toHaveLength(1);
    await userEvent.click(within(warned).getByRole("button", { name: "Discard anyway" }));

    await waitFor(() => expect(asked).toHaveLength(2));
    expect(asked[1]).toEqual(expect.objectContaining({ busyToo: true }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("writes nothing when the warning is cancelled", async () => {
    const { asked } = discarding(changed(TEXT), [
      { kind: "busy", chats: [{ name: "Fix the parser", state: "is running" }] },
    ]);
    await userEvent.click(await opened());
    const warned = await screen.findByRole("alertdialog");

    await userEvent.click(within(warned).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(asked).toHaveLength(1);
  });

  it("offers Reload for a file changed on disk, which reads it again and writes nothing", async () => {
    const { asked, reads } = discarding(changed(TEXT), [{ kind: "stale" }]);
    await userEvent.click(await opened());
    const stale = await screen.findByRole("alertdialog", {
      name: "src/lib.rs changed on disk since this diff was drawn",
    });

    await userEvent.click(within(stale).getByRole("button", { name: "Reload" }));

    await waitFor(() => expect(reads()).toBe(2));
    expect(asked).toHaveLength(1);
  });

  it("offers Overwrite for a file changed on disk, sending what was drawn and merging nothing", async () => {
    const { asked } = discarding(changed(TEXT), [
      { kind: "stale" },
      { kind: "discarded", now: changed(TEXT), unrecorded: null },
    ]);
    await userEvent.click(await opened());
    const stale = await screen.findByRole("alertdialog");

    await userEvent.click(within(stale).getByRole("button", { name: "Overwrite" }));

    await waitFor(() => expect(asked).toHaveLength(2));
    expect(asked[1]).toEqual(
      expect.objectContaining({
        against: { kind: "overwrite", head: "ONE\ntwo\nthree\nFOUR\n" },
      }),
    );
  });

  it("offers a binary file's change whole, and a deleted file's its one hunk", async () => {
    discarding(changed({ kind: "binary" }), []);
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="logo.png" />);
    expect(
      await screen.findByRole("button", { name: "Discard the change to logo.png" }),
    ).toBeInTheDocument();
    cleanup();

    discarding(
      {
        ...changed({
          kind: "text",
          base: "gone\n",
          head: "",
          hunks: [{ oldStart: 1, oldLines: 1, newStart: 0, newLines: 0 }],
        }),
        mark: "deleted",
      },
      [],
    );
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="gone.txt" />);
    expect(
      await screen.findByRole("button", { name: "Discard the deletion of gone.txt" }),
    ).toBeInTheDocument();
  });

  it("offers no discard for a file past what the comparison draws", async () => {
    discarding({ ...changed({ kind: "too-large", bytes: 9_000_000 }), stamp: null }, []);
    render(<PieceDiffTab plane={PLANE} cut={CUT} path="huge.txt" />);
    await screen.findByTestId("piece-diff-trouble");
    expect(screen.queryByRole("button", { name: /Discard/ })).toBeNull();
  });

  it("says the core's refusal, and that a discard the log refused is not in the chat's history", async () => {
    mockIPC((cmd) => {
      if (cmd === "what_changed") return changed(TEXT);
      throw "'src/lib.rs' was not discarded: it is a link or a submodule on one side";
    });
    await userEvent.click(await opened());
    expect(await screen.findByText(/was not discarded/)).toBeInTheDocument();
    cleanup();

    discarding(changed(TEXT), [
      { kind: "discarded", now: changed(TEXT), unrecorded: "the event log refused it" },
    ]);
    await userEvent.click(await opened());
    expect(
      await screen.findByText(
        "Discarded, but it is not in the chat's history: the event log refused it",
      ),
    ).toBeInTheDocument();
  });
});

describe("a branch's check results on the diff (ADR 0114)", () => {
  const TEXT: WhatChanged["diff"] = {
    kind: "text",
    base: "one\n",
    head: "one\ntwo\nthree\n",
    hunks: [{ oldStart: 1, oldLines: 0, newStart: 2, newLines: 2 }],
  };
  const result = (over: Partial<WhatChanged["checks"]["onLines"][number]>) => ({
    tool: "eslint",
    outOfDate: false,
    severity: "error" as const,
    path: "src/a.ts",
    line: 2,
    shownAt: 2,
    rule: "no-undef",
    message: "x is not defined",
    ...over,
  });
  const reported = (checks: Partial<WhatChanged["checks"]>): WhatChanged => ({
    ...changed(TEXT),
    checks: {
      ...NO_CHECKS,
      sets: [{ tool: "eslint", head: "32f2ddcb58188242", outOfDate: false, results: 1 }],
      ...checks,
    },
  });
  const tab = () =>
    render(
      <PieceDiffTab
        plane={"p" as PlaneId}
        cut={{ workspace: "w", repo: "r", piece: "b" }}
        path="src/a.ts"
      />,
    );

  it("says nothing of checks where nothing was reported", async () => {
    core(changed(TEXT));
    tab();
    await screen.findByTestId("merge-viewer");
    expect(screen.queryByRole("region", { name: /Check results/ })).toBeNull();
  });

  it("marks a result on a changed line in the gutter with its tool, severity and message", async () => {
    core(reported({ onLines: [result({})] }));
    tab();
    await screen.findByTestId("merge-viewer");
    const mark = await waitFor(() => {
      // The gutter's spacer, which holds its width, is one too, and says nothing.
      const found = [...document.querySelectorAll<HTMLElement>(".cm-check-mark")].find(
        (one) => one.title !== "",
      );
      if (!found) throw new Error("no mark yet");
      return found;
    });
    expect(mark.title).toBe("eslint, error: x is not defined");
    expect(mark.textContent).toBe("E");
  });

  it("lists a result outside every hunk under its file, and one on another file with its name", async () => {
    core(
      reported({
        outside: [result({ line: 90, shownAt: null, message: "far away" })],
        elsewhere: [result({ path: "src/b.ts", line: 4, shownAt: null, message: "in b" })],
      }),
    );
    tab();
    const list = await screen.findByRole("region", { name: "Check results for src/a.ts" });
    const inside = within(list);
    inside.getByRole("heading", { name: "Outside the changed lines" });
    inside.getByText("far away");
    inside.getByRole("heading", { name: "In other files" });
    inside.getByText("eslint, no-undef, src/b.ts, line 4");
  });

  it("counts what a list could not hold", async () => {
    core(reported({ elsewhere: [result({ shownAt: null })], moreElsewhere: 7 }));
    tab();
    await screen.findByText("And 7 results more.");
  });

  it("names the commit a set was reported for, and says when the branch has moved", async () => {
    core(
      reported({
        sets: [{ tool: "eslint", head: "32f2ddcb58188242", outOfDate: true, results: 1 }],
        onLines: [result({ outOfDate: true })],
      }),
    );
    tab();
    await screen.findByText("eslint: 1 result, reported for 32f2ddc");
    screen.getByText("Out of date");
    screen.getByText("The branch has moved since. Run the check and report it again.");
  });

  it("opens your editor at a result's line", async () => {
    setYourEditor("zed");
    const asked = core(reported({ outside: [result({ line: 90, shownAt: null })] }));
    tab();
    const list = await screen.findByRole("region", { name: "Check results for src/a.ts" });
    await userEvent.click(within(list).getByRole("button", { name: /your editor/i }));
    await waitFor(() => expect(asked).toContain("editor:src/a.ts:90:zed"));
  });

  it("offers no opening for a file the tool named outside the branch, and still lists it", async () => {
    core(reported({ elsewhere: [result({ path: "/home/runner/a.ts", line: 1, shownAt: null })] }));
    tab();
    const list = await screen.findByRole("region", { name: "Check results for src/a.ts" });
    within(list).getByText("eslint, no-undef, /home/runner/a.ts, line 1");
    expect(within(list).queryByRole("button")).toBeNull();
  });

  it("draws the diff and says why when the results cannot be read", async () => {
    core(reported({ sets: [], trouble: "purlis could not read the results" }));
    tab();
    await screen.findByText("purlis could not read the results");
    await screen.findByTestId("merge-viewer");
  });
});
