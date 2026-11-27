// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { IDecorationOptions, Terminal } from "@xterm/xterm";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { SessionPane } from "./SessionPane";

/**
 * **A link wrapped over more than one row** (#7819): a real xterm 80 columns wide is written a
 * link too long for one row, wrapped by the terminal (a soft wrap) or by the program, which
 * breaks its own lines at the pane's width (a hard wrap). The pointer hovers one piece and
 * ⌘-clicks another. What is asserted is what the core was asked and handed, and which cells
 * were drawn in the link colour.
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

/** A cell 8 by 17 CSS pixels, as `SessionPane.links.test.tsx` gives xterm's measuring span. */
const WIDTH = 8;
const CELL = 17;
const measuring = (el: HTMLElement) => el.classList.contains("xterm-char-measure-element");

/** The pane's width, in columns. */
const COLS = 80;

/** The files the core finds on the disk, by the words a pane prints for them. */
let files: string[] = [];

/** Every command the window invoked, with what it was handed, in order. */
let invoked: { cmd: string; args: Record<string, unknown> }[] = [];

beforeEach(() => {
  made.length = 0;
  invoked = [];
  files = [];
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
      return { view: 1, columns: COLS, rows: 24, scrollback: 5000, newline: null };
    if (cmd === "pane_link_resolve") {
      const { candidates } = args as {
        candidates: { kind: string; text: string; target?: string }[];
      };
      return candidates.map((one) =>
        one.kind === "url" || one.kind === "osc8"
          ? {
              kind: "url",
              target: one.target ?? one.text,
              shown: one.target ?? one.text,
              does: { does: "opens", into: "browser" },
            }
          : one.kind === "file" && files.includes(one.text)
            ? {
                kind: "file",
                target: `/planes/one/${one.text}`,
                shown: one.text,
                does: { does: "opens", into: "editor-tab" },
              }
            : null,
      );
    }
    if (cmd === "pane_link_open") return { answer: "opened", kind: "url", target: "x" };
    return null;
  });
});
afterEach(() => {
  cleanup();
  letGoOfKeptTerminals();
  clearMocks();
  vi.restoreAllMocks();
});

const write = (term: Terminal, text: string) => new Promise<void>((done) => term.write(text, done));

/** The cells drawn in the link colour since the pointer last moved, by row. */
let painted: { y: number; x: number; width: number }[] = [];

async function opened() {
  render(<SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />);
  const term = made[0];
  await waitFor(() => expect(term.cols).toBe(COLS));
  const drawn = term.element?.querySelector(".xterm-screen");
  if (!drawn) throw new Error("the pane opened no terminal screen");
  painted = [];
  vi.spyOn(term, "registerDecoration").mockImplementation((options: IDecorationOptions) => {
    painted.push({ y: options.marker.line, x: options.x ?? 0, width: options.width ?? 1 });
    return undefined;
  });
  return { term, drawn };
}

/** The pointer over the cell at (`col`, `row`), zero-based. */
const over = (col: number, row: number, init: MouseEventInit = {}): MouseEventInit => ({
  clientX: col * WIDTH + WIDTH / 2,
  clientY: row * CELL + CELL / 2,
  bubbles: true,
  cancelable: true,
  ...init,
});

function hover(on: Element, col: number, row: number) {
  painted = [];
  on.dispatchEvent(new MouseEvent("mousemove", over(col, row)));
}

/** A click on the cell, as the engine raises it: the press, then its release. */
function click(on: Element, col: number, row: number, init: MouseEventInit = {}) {
  on.dispatchEvent(new MouseEvent("mousedown", over(col, row, { button: 0, detail: 1, ...init })));
  on.dispatchEvent(new MouseEvent("mouseup", over(col, row, { button: 0, ...init })));
  on.dispatchEvent(new MouseEvent("click", over(col, row, { button: 0, ...init })));
}

const asked = (cmd: string) => invoked.filter((one) => one.cmd === cmd);

/** Every candidate the core was asked about, in order. */
const resolved = () =>
  asked("pane_link_resolve").flatMap(
    (one) => one.args.candidates as { kind: string; text: string }[],
  );

/** What the core was handed to open, last. */
const openedLast = () => asked("pane_link_open").at(-1)?.args.candidate;

/** A pause for what must not happen. */
const settle = () => new Promise((done) => setTimeout(done, 50));

/** `prefix` and `fill` repeated up to `cols` characters in all. */
const upTo = (cols: number, prefix: string, fill: string) =>
  `${prefix}${fill.repeat(cols - prefix.length)}`;

const osc8 = (target: string, words: string) =>
  `\u001b]8;;${target}\u001b\\${words}\u001b]8;;\u001b\\`;

describe("a link the terminal wrapped over more than one row", () => {
  it("is one link from either row where it wraps once: hovered on its second, opened from it", async () => {
    const { term, drawn } = await opened();
    const url = `https://purlis.dev/${"a".repeat(100)}`;
    await write(term, `see ${url} now`);

    hover(drawn, 10, 1);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: "see ".length, width: COLS - "see ".length },
        { y: 1, x: 0, width: url.length - (COLS - "see ".length) },
      ]),
    );
    click(drawn, 10, 1, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: url }));
  });

  it("is one link over three rows, drawn on each of them", async () => {
    const { term, drawn } = await opened();
    const url = `https://purlis.dev/${"b".repeat(190)}`;
    await write(term, `${url} now`);

    hover(drawn, 3, 2);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: 0, width: COLS },
        { y: 1, x: 0, width: COLS },
        { y: 2, x: 0, width: url.length - 2 * COLS },
      ]),
    );
    click(drawn, 40, 1, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: url }));
  });

  it("is a path and line whole where it wraps", async () => {
    const path = `src/${"d".repeat(80)}/main.rs:42`;
    files = [path];
    const { term, drawn } = await opened();
    await write(term, `at ${path} now`);

    hover(drawn, 5, 1);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "file", text: path }]));
    click(drawn, 5, 0, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "file", text: path }));
  });

  it("is still one link when the pane is made narrower and the terminal wraps it again", async () => {
    const { term, drawn } = await opened();
    const url = `https://purlis.dev/${"c".repeat(100)}`;
    await write(term, `${url} now\r\n`);

    term.resize(50, 24);
    hover(drawn, 3, 2);

    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: 0, width: 50 },
        { y: 1, x: 0, width: 50 },
        { y: 2, x: 0, width: url.length - 100 },
      ]),
    );
    click(drawn, 3, 2, { ctrlKey: true });
    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: url }));
  });
});

describe("a link the program broke over rows at the pane's width", () => {
  it("is one link where its first row fills the pane and the next goes on with it", async () => {
    const { term, drawn } = await opened();
    // The program writes a full row, then breaks the line itself and goes on.
    const head = upTo(COLS - "see ".length, "https://purlis.dev/", "a");
    const url = `${head}tail`;
    await write(term, `see ${head}\r\ntail now`);

    hover(drawn, 1, 1);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url, joined: true }]));
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: "see ".length, width: COLS - "see ".length },
        { y: 1, x: 0, width: "tail".length },
      ]),
    );
    click(drawn, 40, 0, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: url, joined: true }));
  });

  it("is one link over three rows broken by the program", async () => {
    const { term, drawn } = await opened();
    const one = upTo(COLS, "https://purlis.dev/", "x");
    const two = "y".repeat(COLS);
    await write(term, `${one}\r\n${two}\r\nzz now`);
    const url = `${one}${two}zz`;

    hover(drawn, 30, 1);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url, joined: true }]));
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: 0, width: COLS },
        { y: 1, x: 0, width: COLS },
        { y: 2, x: 0, width: 2 },
      ]),
    );
    click(drawn, 1, 2, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: url, joined: true }));
  });

  it("is one link over three rows from its last row, hovered first", async () => {
    const { term, drawn } = await opened();
    const one = upTo(COLS, "https://purlis.dev/", "x");
    const two = "y".repeat(COLS);
    await write(term, `${one}\r\n${two}\r\nzz now`);

    hover(drawn, 1, 2);

    await waitFor(() =>
      expect(resolved()).toEqual([{ kind: "url", text: `${one}${two}zz`, joined: true }]),
    );
  });

  it.each([
    ["an issue's number", "fixed in #12", "3 tests pass", "#12"],
    ["a chat's number", "see chat 6", "41 more", "chat 6"],
    ["a path's line", "at src/main.rs:4", "2 more", "src/main.rs:4"],
  ])("does not take the next row's digits onto %s at the edge", async (_, end, next, alone) => {
    const { term, drawn } = await opened();
    const row = `${"-".repeat(COLS - end.length)}${end}`;
    await write(term, `${row}\r\n${next}`);

    hover(drawn, COLS - 2, 0);
    await waitFor(() => expect(resolved().length).toBeGreaterThan(0));
    await settle();

    expect(resolved().map((one) => one.text)).toEqual([alone]);
  });

  it("is a path and line whole where the program broke it", async () => {
    const head = `${upTo(COLS - "at ".length - "/ma".length, "src/", "e")}/ma`;
    const path = `${head}in.rs:42`;
    files = [path];
    const { term, drawn } = await opened();
    await write(term, `at ${head}\r\nin.rs:42 next`);

    hover(drawn, 2, 1);
    await waitFor(() =>
      expect(resolved()).toContainEqual({ kind: "file", text: path, joined: true }),
    );
    click(drawn, 2, 1, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "file", text: path, joined: true }));
  });

  it.each([
    ["an issue's number as its fragment", "https://a.example/", "#123 is fixed"],
    ["a second slash after its own", "https://a.example/a/", "/etc/hosts"],
  ])("does not take %s from the next row onto an address at the edge", async (_, prefix, next) => {
    const { term, drawn } = await opened();
    const head = prefix.endsWith("/a/")
      ? `${upTo(COLS - 1, prefix.slice(0, -1), "a")}/`
      : upTo(COLS, prefix, "p");
    await write(term, `${head}\r\n${next}`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toContainEqual({ kind: "url", text: head }));
    hover(drawn, 2, 1);
    await settle();

    expect(resolved().map((one) => one.text)).not.toContain(`${head}${next.split(" ")[0]}`);
  });

  it("is the row's own piece where the joined words name no file", async () => {
    const piece = `${upTo(COLS - "at ".length - ".rs".length, "src/", "f")}.rs`;
    files = [piece];
    const { term, drawn } = await opened();
    await write(term, `at ${piece}\r\nNext, the tests`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toContainEqual({ kind: "file", text: piece }));
    click(drawn, 10, 0, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "file", text: piece }));
  });

  it("is two links where one ends at the row's edge and the next row starts another", async () => {
    const { term, drawn } = await opened();
    const first = upTo(COLS, "https://a.example/", "p");
    const second = "https://b.example/q";
    await write(term, `${first}\r\n${second} now`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toContainEqual({ kind: "url", text: first }));
    hover(drawn, 5, 1);
    await waitFor(() => expect(resolved()).toContainEqual({ kind: "url", text: second }));
    click(drawn, 5, 1, { ctrlKey: true });
    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: second }));

    expect(resolved().every((one) => one.text === first || one.text === second)).toBe(true);
  });

  it("is one link where the program indents the rows it goes on with as it did the first", async () => {
    const { term, drawn } = await opened();
    const indent = "  ";
    const head = `https://purlis.dev/${"a".repeat(COLS - indent.length - "https://purlis.dev/".length)}`;
    const url = `${head}tail`;
    await write(term, `${indent}${head}\r\n${indent}tail now`);

    hover(drawn, 3, 1);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url, joined: true }]));
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: indent.length, width: COLS - indent.length },
        { y: 1, x: indent.length, width: "tail".length },
      ]),
    );
    click(drawn, 40, 0, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: url, joined: true }));
  });

  it("is one link over three indented rows, from its last", async () => {
    const { term, drawn } = await opened();
    const indent = "  ";
    const one = `https://purlis.dev/${"x".repeat(COLS - indent.length - "https://purlis.dev/".length)}`;
    const two = "y".repeat(COLS - indent.length);
    await write(term, `${indent}${one}\r\n${indent}${two}\r\n${indent}zz now`);

    hover(drawn, 3, 2);

    await waitFor(() =>
      expect(resolved()).toEqual([{ kind: "url", text: `${one}${two}zz`, joined: true }]),
    );
  });

  it.each([
    ["more blanks than its first row", "", "  "],
    ["fewer blanks than its first row", "    ", "  "],
    ["blanks where its first row has none", "", " "],
  ])("is not joined where the next row starts with %s", async (_, first, next) => {
    const { term, drawn } = await opened();
    const url = `https://a.example/${"p".repeat(COLS - first.length - "https://a.example/".length)}`;
    await write(term, `${first}${url}\r\n${next}indented words`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    hover(drawn, next.length + 1, 1);
    await settle();

    expect(resolved()).toEqual([{ kind: "url", text: url }]);
  });

  it("is one link where its first row opens with a marker and the rows after it hang under its words", async () => {
    const { term, drawn } = await opened();
    // A Claude Code reply: `⏺ ` and the words, then each row after it indented two blanks.
    const head = upTo(COLS - "⏺ ".length, "https://purlis.dev/", "a");
    const url = `${head}tail`;
    await write(term, `⏺ ${head}\r\n  tail now`);

    hover(drawn, 3, 1);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url, joined: true }]));
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: 2, width: COLS - 2 },
        { y: 1, x: 2, width: "tail".length },
      ]),
    );
    click(drawn, 40, 0, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: url, joined: true }));
  });

  it("is one link over three rows hung under a marker, from its last", async () => {
    const { term, drawn } = await opened();
    const one = upTo(COLS - "⏺ ".length, "https://purlis.dev/", "x");
    const two = "y".repeat(COLS - 2);
    await write(term, `⏺ ${one}\r\n  ${two}\r\n  zz now`);

    hover(drawn, 3, 2);

    await waitFor(() =>
      expect(resolved()).toEqual([{ kind: "url", text: `${one}${two}zz`, joined: true }]),
    );
  });

  it.each([
    ["a list item's dash", "- "],
    ["a bullet", "• "],
    ["a reply's marker under an indent of its own", "  ⏺ "],
  ])("is one link where its first row opens with %s", async (_, opening) => {
    const { term, drawn } = await opened();
    const head = upTo(COLS - opening.length, "https://purlis.dev/", "a");
    const hang = " ".repeat(opening.length);
    await write(term, `${opening}${head}\r\n${hang}tail now`);

    hover(drawn, opening.length + 1, 1);

    await waitFor(() =>
      expect(resolved()).toEqual([{ kind: "url", text: `${head}tail`, joined: true }]),
    );
  });

  it.each([
    ["no blanks", ""],
    ["fewer blanks than its first row's words", " "],
    ["more blanks than its first row's words", "    "],
  ])(
    "is not joined where a marker opens its first row and the next row starts with %s",
    async (_, next) => {
      const { term, drawn } = await opened();
      const url = upTo(COLS - "⏺ ".length, "https://a.example/", "p");
      await write(term, `⏺ ${url}\r\n${next}indented words`);

      hover(drawn, 10, 0);
      await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
      hover(drawn, next.length + 1, 1);
      await settle();

      expect(resolved()).toEqual([{ kind: "url", text: url }]);
    },
  );

  it.each([
    ["a nested list item's dash", "- ", "  - next item"],
    ["a nested bullet", "• ", "  • next item"],
    ["a tool result's corner under a reply's marker", "⏺ ", "  ⎿  done"],
  ])(
    "is not joined where the row it hangs opens with %s and a blank of its own",
    async (_, opening, next) => {
      const { term, drawn } = await opened();
      const url = upTo(COLS - opening.length, "https://a.example/", "p");
      await write(term, `${opening}${url}\r\n${next}`);

      hover(drawn, 10, 0);
      await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
      hover(drawn, 6, 1);
      await settle();

      expect(resolved()).toEqual([{ kind: "url", text: url }]);
    },
  );

  it("is not joined under a first row that opens with a character no list or reply marks with", async () => {
    const { term, drawn } = await opened();
    const url = upTo(COLS - "→ ".length, "https://a.example/", "p");
    await write(term, `→ ${url}\r\n  indented words`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    hover(drawn, 3, 1);
    await settle();

    expect(resolved()).toEqual([{ kind: "url", text: url }]);
  });

  it("is one link where a wide character fills its row's last column", async () => {
    const { term, drawn } = await opened();
    // Two cells each: the row ends on the second half of the last one.
    const wide = "漢".repeat((COLS - "https://purlis.dev/a".length) / 2);
    const head = `https://purlis.dev/a${wide}`;
    const url = `${head}tail`;
    await write(term, `${head}\r\ntail now`);

    hover(drawn, 1, 1);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url, joined: true }]));
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: 0, width: COLS },
        { y: 1, x: 0, width: "tail".length },
      ]),
    );
    click(drawn, 1, 1, { ctrlKey: true });

    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: url, joined: true }));
  });

  it("is hovered from the second cell of the wide character it ends on", async () => {
    const { term, drawn } = await opened();
    const url = "https://purlis.dev/漢漢";
    await write(term, `see ${url} now`);

    // Each wide character takes two cells: the last one's second is the link's last cell.
    hover(drawn, "see https://purlis.dev/".length + 3, 0);

    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    await waitFor(() => expect(painted).toEqual([{ y: 0, x: "see ".length, width: 23 }]));
  });

  it("is not joined where a wide character that did not fit leaves its row's last column blank", async () => {
    const { term, drawn } = await opened();
    // 79 columns written, then a wide character the terminal moves to the next row itself.
    const head = `https://purlis.dev/${"漢".repeat((COLS - 1 - "https://purlis.dev/".length) / 2)}`;
    await write(term, `${head} \r\ntail now`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: head }]));
    hover(drawn, 1, 1);
    await settle();

    expect(resolved()).toEqual([{ kind: "url", text: head }]);
  });

  it("stands its tooltip beside its first row on screen once the rows above have scrolled off", async () => {
    const { term, drawn } = await opened();
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      return this.classList.contains("xterm-screen")
        ? new DOMRect(0, 0, COLS * WIDTH, 24 * CELL)
        : new DOMRect(0, 0, 0, 0);
    });
    const head = `https://purlis.dev/${"a".repeat(COLS - "see ".length - "https://purlis.dev/".length)}`;
    // Its first row, then its second, then enough rows to scroll the first off.
    await write(term, `see ${head}\r\ntail now${"\r\nmore".repeat(23)}`);
    expect(term.buffer.active.viewportY).toBe(1);

    hover(drawn, 1, 0);
    await waitFor(() =>
      expect(resolved()).toEqual([{ kind: "url", text: `${head}tail`, joined: true }]),
    );
    await screen.findByRole("tooltip", {}, { timeout: 2000 });

    const at = document.querySelector<HTMLElement>(".pane-link-anchor");
    expect([at?.style.top, at?.style.left, at?.style.width]).toEqual([
      "0px",
      "0px",
      `${"tail".length * WIDTH}px`,
    ]);
  });

  it("is read as each row's own piece once the pane is made narrower and the terminal wraps its first row again", async () => {
    const { term, drawn } = await opened();
    const head = `https://purlis.dev/${"a".repeat(COLS - "https://purlis.dev/".length)}`;
    await write(term, `${head}\r\ntail now`);
    hover(drawn, 1, 1);
    await waitFor(() =>
      expect(resolved()).toEqual([{ kind: "url", text: `${head}tail`, joined: true }]),
    );

    term.resize(50, 24);
    invoked = [];
    hover(drawn, 3, 1);

    // The first row's part no longer reaches the edge: nothing joins until the program draws
    // the screen again at the new width (ADR 0132).
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: head }]));
    click(drawn, 3, 1, { ctrlKey: true });
    await waitFor(() => expect(openedLast()).toEqual({ kind: "url", text: head }));
  });

  it("is not joined on the alternate screen, where a full-screen program cuts its rows at the edge", async () => {
    const { term, drawn } = await opened();
    const address = "https://bank.example";
    const next = ".evil.example/login";
    expect("go to ".repeat(10).length + address.length).toBe(COLS);
    // An editor or a pager that does not wrap draws the part of a line that fits.
    await write(term, `\u001b[?1049h${"go to ".repeat(10)}${address}\r\n${next}`);

    hover(drawn, COLS - 2, 0);
    await waitFor(() => expect(resolved()).toContainEqual({ kind: "url", text: address }));
    hover(drawn, 2, 1);
    await settle();

    expect(resolved().map((one) => one.text)).not.toContain(`${address}${next}`);
  });

  it.each([
    ["a row of box-drawing characters", "", "─".repeat(COLS)],
    ["a row of block characters", "", "▀▀▀▀ more"],
    ["a row that starts a frame's border", "", "│ more"],
    ["a row after a link that runs into a frame's border", "│", "more words"],
  ])("is not joined onto %s", async (_, border, next) => {
    const { term, drawn } = await opened();
    const head = `https://a.example/${"p".repeat(COLS - border.length - "https://a.example/".length)}`;
    await write(term, `${head}${border}\r\n${next}`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: `${head}${border}` }]));
    hover(drawn, 2, 1);
    await settle();

    expect(resolved()).toEqual([{ kind: "url", text: `${head}${border}` }]);
  });

  it.each([
    ["a reply's marker", "⏺ Next step"],
    ["a filled circle", "● done"],
    ["a tool result's corner", "⎿  Read 3 files"],
    ["a prompt's chevron", "❯ prompt"],
    ["a check mark", "✓ ok"],
    ["an arrow", "→ next"],
    ["an ellipsis", "…and more"],
    ["a marker with no blank after it", "⏺Next"],
  ])("is not joined onto a row that opens with %s", async (_, next) => {
    const { term, drawn } = await opened();
    const url = upTo(COLS, "https://a.example/", "p");
    await write(term, `${url}\r\n${next}`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    hover(drawn, 1, 1);
    await settle();

    expect(resolved()).toEqual([{ kind: "url", text: url }]);
  });

  it("is not joined where the words past the break hold a character no link goes on with", async () => {
    const { term, drawn } = await opened();
    const url = upTo(COLS, "https://a.example/", "p");
    await write(term, `${url}\r\nmore⏺words`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    hover(drawn, 1, 1);
    await settle();

    expect(resolved().map((one) => one.text)).not.toContain(`${url}more⏺words`);
  });

  /** Words read from both rows at once: the first row's address and the next row's words. */
  const readsBoth = (text: string) => text.includes("a.example") && text.includes("b.example");

  it.each([
    ["braille blanks", "\u2800"],
    ["halfwidth Hangul fillers", "\uFFA0"],
  ])("is not joined where its row reaches the edge only with %s", async (_, pad) => {
    const { term, drawn } = await opened();
    const address = "see https://a.example";
    await write(term, `${address}${pad.repeat(COLS - address.length)}\r\n.b.example/login`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved().length).toBeGreaterThan(0));
    hover(drawn, 2, 1);
    await settle();

    expect(resolved().some((one) => readsBoth(one.text))).toBe(false);
  });

  it("is not joined where its row reaches the edge only with concealed cells", async () => {
    const { term, drawn } = await opened();
    const address = "see https://a.example";
    const hidden = `\u001b[8m${"x".repeat(COLS - address.length)}\u001b[28m`;
    await write(term, `${address}${hidden}\r\n.b.example/login`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved().length).toBeGreaterThan(0));
    hover(drawn, 2, 1);
    await settle();

    expect(resolved().some((one) => readsBoth(one.text))).toBe(false);
  });

  it("is not joined where its row stops short of the pane's edge", async () => {
    const { term, drawn } = await opened();
    const url = upTo(COLS - 12, "https://a.example/", "p");
    await write(term, `${url}\r\nmore words`);

    hover(drawn, 10, 0);
    await waitFor(() => expect(resolved()).toEqual([{ kind: "url", text: url }]));
    hover(drawn, 1, 1);
    await settle();

    expect(resolved()).toEqual([{ kind: "url", text: url }]);
  });
});

describe("a link a program marked (OSC 8) that wraps over two rows", () => {
  it("is drawn as one link from either row", async () => {
    const { term, drawn } = await opened();
    const words = `${"w".repeat(COLS - "see ".length)}rapped`;
    await write(term, `see ${osc8("https://purlis.dev/long", words)} after`);

    hover(drawn, 2, 1);
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: "see ".length, width: COLS - "see ".length },
        { y: 1, x: 0, width: "rapped".length },
      ]),
    );
    // Off it, and on again from its first row.
    hover(drawn, 30, 10);
    await settle();
    hover(drawn, 30, 0);
    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: "see ".length, width: COLS - "see ".length },
        { y: 1, x: 0, width: "rapped".length },
      ]),
    );
  });
});

describe("a link a program marked (OSC 8) over more rows, or broken by the program", () => {
  it("is drawn over each of three rows it wraps over, from its last", async () => {
    const { term, drawn } = await opened();
    await write(term, `${osc8("https://purlis.dev/long", "w".repeat(2 * COLS + 40))} after`);

    hover(drawn, 2, 2);

    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: 0, width: COLS },
        { y: 1, x: 0, width: COLS },
        { y: 2, x: 0, width: 40 },
      ]),
    );
  });

  it("is drawn only over its own row where a later link was written over the rows after it", async () => {
    const { term, drawn } = await opened();
    await write(term, osc8("https://a.example/long", "a".repeat(2 * COLS + 40)));
    // The program goes back up and writes another link over the whole of the second row.
    await write(term, `\u001b[2;1H${osc8("https://b.example/other", "b".repeat(COLS))}`);

    hover(drawn, 2, 0);

    await waitFor(() => expect(painted).toEqual([{ y: 0, x: 0, width: COLS }]));
  });

  it("is drawn only over the written cells of a row the program broke it on, not the blanks after them", async () => {
    const { term, drawn } = await opened();
    await write(term, osc8("https://purlis.dev/long", "first words   \r\nsecond"));

    hover(drawn, 2, 1);

    await waitFor(() =>
      expect(painted).toEqual([
        { y: 0, x: 0, width: "first words".length },
        { y: 1, x: 0, width: "second".length },
      ]),
    );
  });

  it("is drawn only over the hovered row's part where the program moved the cursor inside it", async () => {
    const { term, drawn } = await opened();
    const words = `${"w".repeat(40)}\u001b[2C${"w".repeat(40)}`;
    await write(term, `see ${osc8("https://purlis.dev/long", words)} after`);

    hover(drawn, 2, 1);

    // The words and the two columns skipped, past the first row's end.
    const second = "see ".length + 40 + 2 + 40 - COLS;
    await waitFor(() => expect(painted).toEqual([{ y: 1, x: 0, width: second }]));
  });

  it("is drawn over the row hovered where it runs on past the most rows read as one", async () => {
    const { term, drawn } = await opened();
    await write(term, osc8("https://purlis.dev/long", "w".repeat(26 * COLS - 10)));
    const top = term.buffer.active.viewportY;

    hover(drawn, 2, 23);

    await waitFor(() => expect(painted.map((row) => row.y)).toContain(top + 23));
  });
});
