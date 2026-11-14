// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { MOST_TARGET } from "./paneLinks";
import { SessionPane } from "./SessionPane";

/**
 * **A link a program marked itself** (OSC 8, #6218, spec #6208's seam 2): a real xterm is
 * written a real OSC 8 sequence, and the pointer hovers it and ⌘-clicks (Ctrl+clicks off a Mac)
 * it. What is asserted is what the core was handed (the words and the target the program named,
 * never a target the window resolved), and what the window showed for its answer.
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
let opens: unknown = null;

/** The words a program printed, and the different address it marked them with. */
const WORDS = "github.com/login";
const TARGET = "https://evil.example/login";
const SAYS = "its text says github.com";
const JS = "purlis opens only http, https and file links, not javascript: links";

/** `words` marked as a link to `target`, as a program writes it (OSC 8, ST-terminated). */
const osc8 = (target: string, words: string) =>
  `\u001b]8;;${target}\u001b\\${words}\u001b]8;;\u001b\\`;

beforeEach(() => {
  made.length = 0;
  invoked = [];
  opens = null;
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
      const { candidates } = args as {
        candidates: { kind: string; text: string; target?: string }[];
      };
      return candidates.map((one) => {
        if (one.kind !== "osc8" || one.target === undefined) return null;
        if (one.target.startsWith("javascript:"))
          return {
            kind: "osc8",
            target: one.target,
            shown: one.target,
            does: { does: "refused", reason: JS },
          };
        // The stand-in core asks where the words name github.com, as the real one does.
        const asks = one.text.includes("github.com");
        return {
          kind: "url",
          target: one.target,
          shown: one.target,
          does: asks ? { does: "asks", reason: SAYS } : { does: "opens", into: "browser" },
          ...(asks ? { its_text: one.text } : {}),
        };
      });
    }
    if (cmd === "pane_link_open") return opens;
    return null;
  });
});
afterEach(() => {
  cleanup();
  letGoOfKeptTerminals();
  clearMocks();
  vi.restoreAllMocks();
  Object.defineProperty(navigator, "platform", { configurable: true, get: () => "" });
});

const write = (term: Terminal, text: string) => new Promise<void>((done) => term.write(text, done));

async function opened() {
  render(<SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />);
  const term = made[0];
  await waitFor(() => expect(term.cols).toBe(80));
  const drawn = term.element?.querySelector(".xterm-screen");
  if (!drawn) throw new Error("the pane opened no terminal screen");
  return { term, drawn };
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

/** The pane written "see <marked words> now", the pointer on the words. */
async function hovered(target = TARGET, words = WORDS) {
  const pane = await opened();
  await write(pane.term, `see ${osc8(target, words)} now`);
  hover(pane.drawn, 8, 0);
  await waitFor(() => expect(asked("pane_link_resolve").length).toBeGreaterThan(0));
  return pane;
}

describe("a link a program marked (OSC 8)", () => {
  it("is asked of the core with its words and the target the program named", async () => {
    await hovered();

    await waitFor(() =>
      expect(asked("pane_link_resolve").map((one) => one.args.candidates)).toContainEqual([
        { kind: "osc8", text: WORDS, target: TARGET },
      ]),
    );
    // The words under it are the program's link, never a path or address of the pane's own.
    for (const one of asked("pane_link_resolve"))
      expect(one.args.candidates).not.toContainEqual(expect.objectContaining({ kind: "file" }));
  });

  it("says where it goes and that it asks first on hover", async () => {
    await hovered();

    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent(`Goes to ${TARGET}`);
    expect(tip).toHaveTextContent("Its text says github.com · Ctrl-click asks first");
  });

  it("is never opened by a plain click", async () => {
    const { drawn } = await hovered();

    click(drawn, 8, 0);
    await new Promise((settle) => setTimeout(settle, 30));

    expect(asked("pane_link_open")).toHaveLength(0);
  });

  it("on Ctrl+click is handed to the core as it was handed, and asks showing both", async () => {
    opens = {
      answer: "ask",
      reason: SAYS,
      shown: WORDS,
      target: TARGET,
      token: "a-one-use-token",
      about: { about: "misleading", kind: "url", host: "evil.example" },
    };
    const { drawn } = await hovered();

    click(drawn, 8, 0, { ctrlKey: true });
    const ask = await screen.findByRole("dialog", {
      name: "This link goes somewhere its text doesn't say",
    });
    expect(asked("pane_link_open")[0].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidate: { kind: "osc8", text: WORDS, target: TARGET },
      token: null,
    });
    expect(ask).toHaveTextContent(WORDS);
    expect(ask).toHaveTextContent(TARGET);
    expect(within(ask).getByText("evil.example").tagName).toBe("STRONG");

    // It starts on Don't open, and Return never opens it.
    const dont = within(ask).getByRole("button", { name: "Don't open" });
    await waitFor(() => expect(dont).toHaveFocus());
    fireEvent.keyDown(dont, { key: "Enter", code: "Enter" });
    await new Promise((settle) => setTimeout(settle, 30));
    expect(asked("pane_link_open")).toHaveLength(1);
  });

  it("opens on the ask's answer with its one-use token", async () => {
    opens = {
      answer: "ask",
      reason: SAYS,
      shown: WORDS,
      target: TARGET,
      token: "a-one-use-token",
      about: { about: "misleading", kind: "url", host: "evil.example" },
    };
    const { drawn } = await hovered();
    click(drawn, 8, 0, { ctrlKey: true });
    const ask = await screen.findByRole("dialog", {
      name: "This link goes somewhere its text doesn't say",
    });
    opens = { answer: "opened", kind: "url", target: TARGET };

    within(ask).getByRole("button", { name: "Open in browser" }).click();

    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(2));
    expect(asked("pane_link_open")[1].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidate: { kind: "osc8", text: WORDS, target: TARGET },
      token: "a-one-use-token",
    });
  });

  it("with another scheme says why on hover, and Ctrl+click shows the refusal", async () => {
    opens = { answer: "refused", reason: JS };
    const { drawn } = await hovered("javascript:alert(1)", "Click here");

    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent("Goes to javascript:alert(1)");
    expect(tip).toHaveTextContent(`${JS}.`);
    click(drawn, 8, 0, { ctrlKey: true });

    const said = await screen.findByRole("dialog", { name: "Link not opened" });
    expect(said).toHaveTextContent("not javascript: links.");
  });

  it("names the refused target in the refusal, beside its words", async () => {
    opens = { answer: "refused", reason: JS };
    const { drawn } = await hovered("javascript:alert(1)", "Click here");
    await screen.findByRole("tooltip", {}, { timeout: 2000 });

    click(drawn, 8, 0, { ctrlKey: true });

    const said = await screen.findByRole("dialog", { name: "Link not opened" });
    expect(within(said).getByText("javascript:alert(1)")).toBeInTheDocument();
    expect(within(said).getByText("Click here")).toBeInTheDocument();
  });

  it("is read whole where its words wrap over two rows", async () => {
    const { term, drawn } = await opened();
    const long = `${"w".repeat(76)}rapped`;
    await write(term, `see ${osc8("https://purlis.dev/long", long)}`);

    hover(drawn, 2, 1);

    await waitFor(() =>
      expect(asked("pane_link_resolve").map((one) => one.args.candidates)).toContainEqual([
        { kind: "osc8", text: long, target: "https://purlis.dev/long" },
      ]),
    );
  });

  it("is read as one word where a line break inside it goes on to the next row", async () => {
    const { term, drawn } = await opened();
    await write(term, `see ${osc8(TARGET, "evil.\r\ncom")} y`);

    hover(drawn, 5, 0);

    await waitFor(() =>
      expect(asked("pane_link_resolve").map((one) => one.args.candidates)).toContainEqual([
        { kind: "osc8", text: "evil.com", target: TARGET },
      ]),
    );
  });

  it("is read with the text the program printed against it, as one word", async () => {
    const { term, drawn } = await opened();
    // Only "hub.com/login" is marked; "git" before it and "!x" after it touch its cells.
    await write(term, `see git${osc8(TARGET, "hub.com/login")}!x and ${osc8(TARGET, "here")} y`);

    hover(drawn, 9, 0);
    await waitFor(() =>
      expect(asked("pane_link_resolve").map((one) => one.args.candidates)).toContainEqual([
        { kind: "osc8", text: "github.com/login!x", target: TARGET },
      ]),
    );

    // Across a blank, the text beside a link is no part of it.
    hover(drawn, 30, 0);
    await waitFor(() =>
      expect(asked("pane_link_resolve").map((one) => one.args.candidates)).toContainEqual([
        { kind: "osc8", text: "here", target: TARGET },
      ]),
    );
  });

  it("is listed in Links in this pane, with its text under its target", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    const { term } = await opened();
    await write(
      term,
      `see ${osc8(TARGET, WORDS)} and ${osc8("https://purlis.dev/", "Click here")}`,
    );
    const textarea = document.querySelector<HTMLElement>(".xterm-helper-textarea");
    if (!textarea) throw new Error("the pane has no terminal textarea");
    textarea.focus();
    textarea.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "L",
        code: "KeyL",
        metaKey: true,
        shiftKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );

    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    await waitFor(() => expect(within(dialog).getAllByRole("option")).toHaveLength(2));
    const [newest, first] = within(dialog).getAllByRole("option");
    expect(newest).toHaveAccessibleName("https://purlis.dev/");
    // Words that name no other target are not said again.
    expect(newest).not.toHaveTextContent("Its text");
    expect(first).toHaveAccessibleName(TARGET);
    expect(first).toHaveTextContent(`Its text: ${WORDS}`);
    expect(first).toHaveTextContent("asks first");
  });

  it("is asked about with what its cells show, where the program printed outside its span", async () => {
    const { term, drawn } = await opened();
    // Opened at column 10, the cursor taken back to the start to print an address there, then
    // on past the span's middle to print plain words: xterm links both, the span holds the
    // plain words only.
    await write(
      term,
      `          \u001b]8;;${TARGET}\u001b\\\u001b[10D${WORDS}\u001b[10CClick here\u001b]8;;\u001b\\`,
    );

    hover(drawn, 3, 0);

    await waitFor(() =>
      expect(
        asked("pane_link_resolve").some((one) =>
          (one.args.candidates as { kind: string; text: string }[]).some(
            (candidate) => candidate.kind === "osc8" && candidate.text.includes(WORDS),
          ),
        ),
      ).toBe(true),
    );
    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent("Its text says github.com");
  });

  it("is listed with its whole rows where the program moved the cursor inside it", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    const { term } = await opened();
    await write(
      term,
      `          \u001b]8;;${TARGET}\u001b\\\u001b[10D${WORDS}\u001b[10CClick here\u001b]8;;\u001b\\`,
    );
    const textarea = document.querySelector<HTMLElement>(".xterm-helper-textarea");
    if (!textarea) throw new Error("the pane has no terminal textarea");
    textarea.focus();
    textarea.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "L",
        code: "KeyL",
        metaKey: true,
        shiftKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );

    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    const [row] = await within(dialog).findAllByRole("option");
    expect(row).toHaveTextContent("asks first");
    expect(row).toHaveTextContent(`Its text: ${WORDS}`);
  });

  it("is handed on with a target cut one past the most, which the core refuses unread", async () => {
    const long = `https://purlis.dev/${"a".repeat(MOST_TARGET * 2)}`;
    await hovered(long, "Click here");

    await waitFor(() => {
      const sent = asked("pane_link_resolve")
        .flatMap((one) => one.args.candidates as { kind: string; target?: string }[])
        .find((one) => one.kind === "osc8");
      expect(sent?.target).toHaveLength(MOST_TARGET + 1);
    });
  });
});
