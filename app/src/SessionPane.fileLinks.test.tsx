// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import { useJumpAsks, type Pending } from "./fileJump";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { useOutsideFileAsks, type OutsideAsk } from "./paneLinkFiles";
import { FILE_ANSWER_LIFE, candidatesIn } from "./paneLinks";
import { SessionPane } from "./SessionPane";
import { PieceFileTab } from "./editor/PieceFiles";
import { EditorView } from "@codemirror/view";
import { isPlaneRoot, type Place } from "./pieceViews";
import { useState } from "react";

/**
 * **File paths in a pane's output** (#6216, spec #6208's seam 2): a real xterm is written a path,
 * and the pointer hovers it and ⌘-clicks it. What is asserted is what the core was asked, and
 * where the window opened what it answered: the editor tab at the file's line, by the jump every
 * file-at-a-line surface uses, or a file outside the project's own tab. Nothing is ever handed
 * to the system.
 */
const made: Terminal[] = [];
vi.mock("@xterm/xterm", async (real) => {
  const xterm = await real<typeof import("@xterm/xterm")>();
  class Recorded extends xterm.Terminal {
    constructor(...args: ConstructorParameters<typeof xterm.Terminal>) {
      super(...args);
      made.push(this);
    }
  }
  return { ...xterm, Terminal: Recorded };
});
vi.mock("./renderer", async (real) => ({
  ...(await real<typeof import("./renderer")>()),
  draw: () => new Promise(() => {}),
}));
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

const WIDTH = 8;
const CELL = 17;
const measuring = (el: HTMLElement) => el.classList.contains("xterm-char-measure-element");

let invoked: { cmd: string; args: Record<string, unknown> }[] = [];
/** What the core answers the open command, one answer per call. */
let opens: unknown[] = [];
/** Whether `src/main.rs` is there, as the core finds it. */
let there = true;

const PATH = "src/main.rs:42:7";
const FULL = "/planes/one/workspaces/app/repos/app/src/main.rs:42:7";
const HOSTS = "/etc/hosts:3";
/** The file in the workspace as its hover says it: from the workspace (D2). */
const SHORT = "repos/app/src/main.rs:42:7";

/** The core's answer for the file in the workspace. */
const IN_THE_WORKSPACE = {
  answer: "file",
  target: FULL,
  at: { in: "branch", workspace: "app", repo: "app", piece: null, path: "src/main.rs" },
  line: 42,
  column: 7,
};

/** A file at the project's root, outside any workspace (#6356), and what the core reads of it. */
const AT_THE_ROOT = "README.md:3:5";
const README = "# project\n\nthe third line\n";

/** The core's ask for a file outside the workspace. */
const ASKS = {
  answer: "ask",
  reason: "outside app",
  shown: HOSTS,
  target: HOSTS,
  token: "one-use-token",
  about: {
    about: "file-outside",
    home: { is: "workspace", name: "app", folder: "/planes/one/workspaces/app" },
  },
};

beforeEach(() => {
  made.length = 0;
  invoked = [];
  opens = [];
  there = true;
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return measuring(this) ? WIDTH * 32 : 0;
  });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return measuring(this) ? CELL : 0;
  });
  mockIPC((cmd, args) => {
    invoked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "watch_session")
      return { view: 1, columns: 80, rows: 24, scrollback: 5000, newline: null };
    if (cmd === "pane_link_resolve") {
      const { candidates } = args as { candidates: { kind: string; text: string }[] };
      return candidates.map((one) =>
        one.text === PATH && there
          ? {
              kind: "file",
              target: FULL,
              shown: SHORT,
              does: { does: "opens", into: "editor-tab" },
            }
          : one.text === AT_THE_ROOT
            ? {
                kind: "file",
                target: `/planes/one/${AT_THE_ROOT}`,
                shown: AT_THE_ROOT,
                does: { does: "opens", into: "editor-tab" },
              }
            : one.text === HOSTS
              ? {
                  kind: "file",
                  target: HOSTS,
                  shown: HOSTS,
                  does: { does: "asks", reason: "outside app" },
                }
              : null,
      );
    }
    if (cmd === "pane_link_open") return opens.shift() ?? { answer: "refused", reason: "no" };
    if (cmd === "plane_file") return { kind: "text", text: README };
    return null;
  });
});
afterEach(() => {
  cleanup();
  letGoOfKeptTerminals();
  clearMocks();
  vi.useRealTimers();
  vi.restoreAllMocks();
  Object.defineProperty(navigator, "platform", { configurable: true, get: () => "" });
});

const write = (term: Terminal, text: string) => new Promise<void>((done) => term.write(text, done));

/** Hears the jumps and the outside files the window was asked to open. */
function Opened({ jumps, outside }: { jumps: Pending[]; outside: OutsideAsk[] }) {
  useJumpAsks((jump) => jumps.push(jump));
  useOutsideFileAsks("/planes/one", (ask) => outside.push(ask));
  return null;
}

async function pane(text: string, title?: string) {
  const jumps: Pending[] = [];
  const outside: OutsideAsk[] = [];
  render(
    <>
      <Opened jumps={jumps} outside={outside} />
      <SessionPane
        plane="/planes/one"
        session={1}
        title={title}
        focused={false}
        onFocus={() => {}}
      />
    </>,
  );
  const term = made[0];
  await waitFor(() => expect(term.cols).toBe(80));
  const drawn = term.element?.querySelector(".xterm-screen");
  if (!drawn) throw new Error("the pane opened no terminal screen");
  await write(term, text);
  return { term, drawn, jumps, outside };
}

const over = (col: number, row: number, init: MouseEventInit = {}): MouseEventInit => ({
  clientX: col * WIDTH + WIDTH / 2,
  clientY: row * CELL + CELL / 2,
  bubbles: true,
  cancelable: true,
  ...init,
});

const hover = (on: Element, col: number, row: number) =>
  on.dispatchEvent(new MouseEvent("mousemove", over(col, row)));

function click(on: Element, col: number, row: number, init: MouseEventInit = {}) {
  on.dispatchEvent(new MouseEvent("mousedown", over(col, row, { button: 0, detail: 1, ...init })));
  on.dispatchEvent(new MouseEvent("mouseup", over(col, row, { button: 0, ...init })));
  on.dispatchEvent(new MouseEvent("click", over(col, row, { button: 0, ...init })));
}

const asked = (cmd: string) => invoked.filter((one) => one.cmd === cmd);

/**
 * A pane, and the tab a jump into the project's root opens, as the project's view opens it: a
 * file there has no branch's file tab, so it opens in a tab of its own (`PlaneView.tsx`).
 */
function PaneAndRootTab() {
  const [tab, setTab] = useState<{ place: Place; path: string }>();
  useJumpAsks((jump) => {
    if (isPlaneRoot(jump.place)) setTab({ place: jump.place, path: jump.path });
  });
  return (
    <>
      <SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />
      {tab !== undefined && <PieceFileTab plane="/planes/one" cut={tab.place} path={tab.path} />}
    </>
  );
}

/** Where the cursor of the light editor in the page is: its line and column, from 1. */
function cursorOf(): { line: number; column: number } | undefined {
  const drawn = document.querySelector<HTMLElement>(".cm-editor");
  const view = drawn === null ? null : EditorView.findFromDOM(drawn);
  if (view === null) return undefined;
  const head = view.state.selection.main.head;
  const line = view.state.doc.lineAt(head);
  return { line: line.number, column: head - line.from + 1 };
}

describe("a pane's file candidates", () => {
  it("are absolute, home and relative paths, with their line and column", () => {
    expect(
      candidatesIn("at src/main.rs:42:7, see ~/notes.md and /etc/hosts:3. Also ./a/b.ts"),
    ).toEqual([
      { kind: "file", text: "src/main.rs:42:7", start: 3, end: 19 },
      { kind: "file", text: "~/notes.md", start: 25, end: 35 },
      { kind: "file", text: "/etc/hosts:3", start: 40, end: 52 },
      { kind: "file", text: "./a/b.ts", start: 59, end: 67 },
    ]);
    // A bare file name with an extension is one; a word, a number or a version is not.
    expect(
      candidatesIn("error in main.rs:3 for v1.2 at 12:30, e.g. done").map((f) => f.text),
    ).toEqual(["main.rs:3"]);
  });

  it("take a file: address as a file, and leave an address's own path alone", () => {
    expect(candidatesIn("open file:///etc/hosts or https://a.example/src/x.rs")).toEqual([
      { kind: "file", text: "file:///etc/hosts", start: 5, end: 22 },
      { kind: "url", text: "https://a.example/src/x.rs", start: 26, end: 52 },
    ]);
  });
});

describe("a file path in a chat's pane", () => {
  it("is asked of the core with only its text, its kind and the pane it is in", async () => {
    const { drawn } = await pane(`at ${PATH} now`);
    hover(drawn, 6, 0);

    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
    expect(asked("pane_link_resolve")[0].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidates: [{ kind: "file", text: PATH }],
    });
  });

  it("shows the file from the workspace and that ⌘-click opens it in an editor tab", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    const { drawn } = await pane(`at ${PATH} now`);
    hover(drawn, 6, 0);

    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent(SHORT);
    expect(tip).not.toHaveTextContent("/planes/one");
    expect(tip).toHaveTextContent("⌘-click opens it in an editor tab");
  });

  it("opens in the editor tab at its line and column on Ctrl+click", async () => {
    opens = [IN_THE_WORKSPACE];
    const { drawn, jumps, outside } = await pane(`at ${PATH} now`);
    hover(drawn, 6, 0);
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));

    click(drawn, 6, 0, { ctrlKey: true });

    await waitFor(() => expect(jumps).toHaveLength(1));
    expect(jumps[0]).toMatchObject({
      plane: "/planes/one",
      place: { workspace: "app", repo: "app", piece: null },
      path: "src/main.rs",
      line: 42,
      column: 7,
    });
    expect(asked("pane_link_open")[0].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidate: { kind: "file", text: PATH },
      token: null,
    });
    expect(outside).toEqual([]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens a file at the project's root at its line and column (#6356)", async () => {
    opens = [
      {
        answer: "file",
        target: `/planes/one/${AT_THE_ROOT}`,
        at: { in: "project", path: "README.md" },
        line: 3,
        column: 5,
      },
    ];
    render(<PaneAndRootTab />);
    const term = made[0];
    await waitFor(() => expect(term.cols).toBe(80));
    const drawn = term.element?.querySelector(".xterm-screen");
    if (!drawn) throw new Error("the pane opened no terminal screen");
    await write(term, `see ${AT_THE_ROOT} now`);
    hover(drawn, 6, 0);
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));

    click(drawn, 6, 0, { ctrlKey: true });

    await waitFor(() => expect(cursorOf()).toEqual({ line: 3, column: 5 }), { timeout: 3000 });
    expect(screen.getByTestId("light-editor")).toHaveTextContent(/the third line/);
  });

  it("is no link when the core finds no file, and says nothing on its hover", async () => {
    there = false;
    const { drawn } = await pane(`at ${PATH} now`);
    hover(drawn, 6, 0);
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
    await new Promise((settle) => setTimeout(settle, 700));

    expect(screen.queryByRole("tooltip")).toBeNull();
    click(drawn, 6, 0, { ctrlKey: true });
    await new Promise((settle) => setTimeout(settle, 20));
    expect(asked("pane_link_open")).toHaveLength(0);
  });

  it("is asked again once its answer is past its life, so a file made since links", async () => {
    there = false;
    const now = vi.spyOn(Date, "now");
    now.mockReturnValue(1_000);
    const { drawn } = await pane(`at ${PATH} now`);
    hover(drawn, 6, 0);
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));

    // Hovered again within its life: kept.
    hover(drawn, 40, 3);
    hover(drawn, 7, 0);
    await new Promise((settle) => setTimeout(settle, 30));
    expect(asked("pane_link_resolve")).toHaveLength(1);

    there = true;
    now.mockReturnValue(1_000 + FILE_ANSWER_LIFE + 1);
    hover(drawn, 40, 3);
    hover(drawn, 6, 0);
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(2));
  });
});

describe("a file outside the workspace", () => {
  async function asking(title?: string) {
    opens = [ASKS];
    const opened = await pane(`wrote ${HOSTS} now`, title);
    hover(opened.drawn, 8, 0);
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
    click(opened.drawn, 8, 0, { ctrlKey: true });
    const ask = await screen.findByRole("dialog", {
      name: "Open a file outside this workspace?",
    });
    return { ...opened, ask };
  }

  it("says where it goes on hover, and that ⌘-click asks first", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    const { drawn } = await pane(`wrote ${HOSTS} now`);
    hover(drawn, 8, 0);

    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent("Outside app · ⌘-click asks first");
  });

  it("asks first, showing the file and the workspace, starting on Open file", async () => {
    const { ask } = await asking();

    expect(ask).toHaveTextContent("This chat's output names a file that is not in app.");
    expect(ask).toHaveTextContent(HOSTS);
    expect(ask).toHaveTextContent("/planes/one/workspaces/app");
    expect(ask).toHaveTextContent("It opens in an editor tab. Nothing is run.");
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Open file" })),
    );
  });

  it("names the chat it came from under its title (#6355)", async () => {
    const { ask } = await asking("Fix the login form");

    expect(ask).toHaveTextContent("From chat #1 · Fix the login form");
    // The line under the title describes the dialog, so a screen reader says it too.
    const subtitle = screen.getByText("From chat #1 · Fix the login form");
    expect(ask.getAttribute("aria-describedby") ?? "").toContain(subtitle.id);
  });

  it("names no chat for a shell's pane, which has no title (#6355)", async () => {
    const { ask } = await asking();

    expect(ask).not.toHaveTextContent("From chat");
  });

  it("opens on Open file, answered with the ask's token, in a tab of its own", async () => {
    const { outside, jumps } = await asking();
    opens = [
      {
        answer: "file",
        target: HOSTS,
        at: { in: "outside", path: "/etc/hosts" },
        line: 3,
        column: null,
      },
    ];

    await userEvent.click(screen.getByRole("button", { name: "Open file" }));

    await waitFor(() => expect(outside).toHaveLength(1));
    expect(outside[0]).toEqual({ plane: "/planes/one", session: 1, path: "/etc/hosts", line: 3 });
    expect(asked("pane_link_open")[1].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidate: { kind: "file", text: HOSTS },
      token: "one-use-token",
    });
    expect(jumps).toEqual([]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("does not open on Don't open, and asks the core nothing more", async () => {
    const { outside, jumps } = await asking();

    await userEvent.click(screen.getByRole("button", { name: "Don't open" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(asked("pane_link_open")).toHaveLength(1);
    expect(outside).toEqual([]);
    expect(jumps).toEqual([]);
  });

  it("does not open on Escape either", async () => {
    const { outside } = await asking();

    await userEvent.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(asked("pane_link_open")).toHaveLength(1);
    expect(outside).toEqual([]);
  });

  it("copies the path on Copy path, and opens nothing", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      get: () => ({ writeText }),
    });
    const { outside } = await asking();

    await userEvent.click(screen.getByRole("button", { name: "Copy path" }));

    expect(writeText).toHaveBeenCalledWith(HOSTS);
    expect(asked("pane_link_open")).toHaveLength(1);
    expect(outside).toEqual([]);
  });
});
