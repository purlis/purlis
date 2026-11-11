// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { candidatesIn } from "./paneLinks";
import { LINK_CORE } from "./paneLinkCore";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { SessionPane } from "./SessionPane";

/**
 * **Links in a pane's output** (#6215, spec #6208's seam 2): a real xterm is written an address,
 * and the pointer hovers it, ⌘-clicks (Ctrl+clicks off a Mac) and plain-clicks it. What is
 * asserted is what the core was asked, and what the window showed for its answer.
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

/** A cell 8 by 17 CSS pixels, as `SessionPane.mouse.test.tsx` gives xterm's measuring span. */
const WIDTH = 8;
const CELL = 17;
const measuring = (el: HTMLElement) => el.classList.contains("xterm-char-measure-element");

/** Every command the window invoked, with what it was handed, in order. */
let invoked: { cmd: string; args: Record<string, unknown> }[] = [];
/** What the core answers the open command. */
let opens: unknown = { answer: "opened", kind: "url", target: "https://purlis.dev/setup" };

const ADDRESS = "https://purlis.dev/setup";

beforeEach(() => {
  made.length = 0;
  invoked = [];
  opens = { answer: "opened", kind: "url", target: ADDRESS };
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
        one.text.startsWith("https://")
          ? { kind: "url", target: one.text, does: { does: "opens" } }
          : null,
      );
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

/** The pointer over the cell at (`col`, `row`), zero-based. */
const over = (col: number, row: number, init: MouseEventInit = {}): MouseEventInit => ({
  clientX: col * WIDTH + WIDTH / 2,
  clientY: row * CELL + CELL / 2,
  bubbles: true,
  cancelable: true,
  ...init,
});

const hover = (on: Element, col: number, row: number) =>
  on.dispatchEvent(new MouseEvent("mousemove", over(col, row)));

/** A click on the cell, as the engine raises it: the press, then its release. */
function click(on: Element, col: number, row: number, init: MouseEventInit = {}) {
  on.dispatchEvent(new MouseEvent("mousedown", over(col, row, { button: 0, detail: 1, ...init })));
  on.dispatchEvent(new MouseEvent("mouseup", over(col, row, { button: 0, ...init })));
  on.dispatchEvent(new MouseEvent("click", over(col, row, { button: 0, ...init })));
}

const asked = (cmd: string) => invoked.filter((one) => one.cmd === cmd);

/** The pane, written "see <address> now" on its first row, with the pointer on the address. */
async function hovered() {
  const pane = await opened();
  await write(pane.term, `see ${ADDRESS} now`);
  hover(pane.drawn, 8, 0);
  await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
  return pane;
}

describe("a pane's link candidates", () => {
  it("are its http(s) addresses, without the punctuation a sentence puts after one", () => {
    expect(
      candidatesIn("Read https://a.example/x, then (see http://b.example/y). Not ftp://c.example"),
    ).toEqual([
      { kind: "url", text: "https://a.example/x", start: 5, end: 24 },
      { kind: "url", text: "http://b.example/y", start: 36, end: 54 },
    ]);
    expect(candidatesIn("https://en.wikipedia.org/wiki/Foo_(bar)")[0].text).toBe(
      "https://en.wikipedia.org/wiki/Foo_(bar)",
    );
    expect(candidatesIn("no links here, nor #5820 nor chat 641")).toEqual([]);
  });
});

describe("a link in a chat's pane", () => {
  it("is asked of the core with only its text, its kind and the pane it is in", async () => {
    await hovered();

    expect(asked("pane_link_resolve")[0].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidates: [{ kind: "url", text: ADDRESS }],
    });
  });

  it("shows its target and what Ctrl-click does on hover", async () => {
    await hovered();

    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent(ADDRESS);
    expect(tip).toHaveTextContent("Ctrl-click opens it in your browser");
  });

  it("says ⌘-click on a Mac", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    await hovered();

    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent("⌘-click opens it in your browser");
  });

  it("is asked of the core once, however often it is hovered", async () => {
    const { drawn } = await hovered();

    hover(drawn, 40, 3);
    hover(drawn, 10, 0);
    hover(drawn, 40, 3);
    hover(drawn, 12, 0);
    await new Promise((settle) => setTimeout(settle, 50));

    expect(asked("pane_link_resolve")).toHaveLength(1);
  });

  it("opens on Ctrl+click: the core is handed the candidate's text, never a target", async () => {
    const { drawn } = await hovered();

    click(drawn, 8, 0, { ctrlKey: true });

    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(1));
    expect(asked("pane_link_open")[0].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidate: { kind: "url", text: ADDRESS },
      confirmed: null,
    });
    // Opened, the window says nothing more.
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens on ⌘-click on a Mac, and not on Ctrl+click there", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    const { drawn } = await hovered();

    click(drawn, 8, 0, { ctrlKey: true });
    await new Promise((settle) => setTimeout(settle, 20));
    expect(asked("pane_link_open")).toHaveLength(0);

    click(drawn, 8, 0, { metaKey: true });
    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(1));
  });

  it("is never opened by a plain click, which selects as in any terminal", async () => {
    const { term, drawn } = await hovered();

    click(drawn, 8, 0);
    // A drag over it, with no key held, selects its text.
    drawn.dispatchEvent(new MouseEvent("mousedown", over(4, 0, { button: 0, detail: 1 })));
    document.dispatchEvent(new MouseEvent("mousemove", over(29, 0, { button: 0, buttons: 1 })));
    document.dispatchEvent(new MouseEvent("mouseup", over(29, 0, { button: 0 })));
    await new Promise((settle) => setTimeout(settle, 20));

    expect(asked("pane_link_open")).toHaveLength(0);
    expect(term.getSelection()).toContain("https://purlis.dev/setup");
  });

  it("says why when the core refuses to open it", async () => {
    opens = {
      answer: "refused",
      reason: "purlis will not open an address that carries a name or a password",
    };
    const { drawn } = await hovered();

    click(drawn, 8, 0, { ctrlKey: true });

    const said = await screen.findByRole("dialog", { name: "Link not opened" });
    expect(said).toHaveTextContent(
      "Purlis will not open an address that carries a name or a password.",
    );
  });

  it("asks first when the core says so, and sends back the target it showed", async () => {
    opens = {
      answer: "ask",
      reason: "its text names another address",
      shown: "docs.purlis.dev/setup",
      target: "https://purlis-setup.example.net/s",
    };
    const { drawn } = await hovered();

    click(drawn, 8, 0, { ctrlKey: true });
    const ask = await screen.findByRole("dialog", { name: "Open this link?" });
    expect(ask).toHaveTextContent("https://purlis-setup.example.net/s");
    opens = { answer: "opened", kind: "url", target: "https://purlis-setup.example.net/s" };
    screen.getByRole("button", { name: "Open" }).click();

    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(2));
    expect(asked("pane_link_open")[1].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidate: { kind: "url", text: ADDRESS },
      confirmed: "https://purlis-setup.example.net/s",
    });
  });

  it("is not a link where the core says it is none", async () => {
    const { term, drawn } = await opened();
    await write(term, "see https://purlis.dev/setup now");
    mockIPC((cmd, args) => {
      invoked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "pane_link_resolve") return [null];
      return null;
    });

    hover(drawn, 8, 0);
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
    click(drawn, 8, 0, { ctrlKey: true });
    await new Promise((settle) => setTimeout(settle, 600));

    expect(asked("pane_link_open")).toHaveLength(0);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});

describe("links while output streams", () => {
  /** `lines` lines of output, each naming an address of its own, in `writes` writes. */
  async function stream(term: Terminal, lines: number, writes: number) {
    const per = lines / writes;
    for (let write_ = 0; write_ < writes; write_ += 1) {
      let chunk = "";
      for (let at = 0; at < per; at += 1)
        chunk += `line ${write_ * per + at}: https://purlis.dev/${write_}/${at}\r\n`;
      await write(term, chunk);
    }
  }

  it("asks the core nothing while no pointer is on the pane", async () => {
    const { term } = await opened();

    await stream(term, 2000, 20);

    expect(asked("pane_link_resolve")).toHaveLength(0);
  });

  it("asks at most once per write for the line under a resting pointer", async () => {
    const { term, drawn } = await opened();
    hover(drawn, 20, 10);

    await stream(term, 2000, 20);
    await new Promise((settle) => setTimeout(settle, 50));

    expect(asked("pane_link_resolve").length).toBeLessThanOrEqual(20 + 1);
  });
});

describe("a link in a read-only terminal", () => {
  it("is asked of the core from no project's pane, and opens on Ctrl+click", async () => {
    render(
      <ReadOnlyTerminal
        screen={`report: ${ADDRESS}`}
        label="A saved chat"
        cols={80}
        rows={4}
        fontSize={13}
        links={LINK_CORE}
      />,
    );
    const term = made[0];
    const drawn = term.element?.querySelector(".xterm-screen");
    if (!drawn) throw new Error("no screen");
    await waitFor(() =>
      expect(term.buffer.active.getLine(0)?.translateToString()).toContain(ADDRESS),
    );

    hover(drawn, 12, 0);
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
    expect(asked("pane_link_resolve")[0].args.from).toEqual({ plane: null, session: null });
    click(drawn, 12, 0, { ctrlKey: true });

    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(1));
  });

  it("has none where its owner hands it no core, as the theme specimen's pictures", async () => {
    render(
      <ReadOnlyTerminal
        screen={`report: ${ADDRESS}`}
        label="A picture"
        cols={80}
        rows={4}
        fontSize={13}
      />,
    );
    const term = made[0];
    const drawn = term.element?.querySelector(".xterm-screen");
    if (!drawn) throw new Error("no screen");
    await waitFor(() =>
      expect(term.buffer.active.getLine(0)?.translateToString()).toContain(ADDRESS),
    );

    hover(drawn, 12, 0);
    click(drawn, 12, 0, { ctrlKey: true });
    await new Promise((settle) => setTimeout(settle, 50));

    expect(invoked).toEqual([]);
  });
});
