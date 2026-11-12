// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { menuRowKey } from "./keys/registry";
import { rowName } from "./menus.testkit";
import { SessionPane } from "./SessionPane";
import { TerminalMenu } from "./TerminalMenu";
import { linkRows, terminalRows } from "./terminalRows";
import { MenuShortcuts } from "./ui/menu/Menu";

/**
 * **Open link and Copy link on the terminal's menu** (#6219, spec #6208; the prototype's S3 and
 * S4): a real xterm is written an address, the pointer rests on it and right-clicks, and the
 * menu's rows are pressed. What is asserted is the menu drawn, what the core was asked and what
 * reached the clipboard.
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

/** What the pane prints: the core resolves it to {@link TARGET}, which is what is copied. */
const TEXT = "HTTPS://Purlis.dev/setup";
const TARGET = "https://purlis.dev/setup";
const FTP_TEXT = "ftp://files.example/a";
const FTP = "purlis opens only http and https addresses, so it will not open this ftp: one";

let invoked: { cmd: string; args: Record<string, unknown> }[] = [];
let opens: unknown = { answer: "opened", kind: "url", target: TARGET };
let copied: string[] = [];

beforeEach(() => {
  made.length = 0;
  invoked = [];
  copied = [];
  opens = { answer: "opened", kind: "url", target: TARGET };
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
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: {
      writeText: (text: string) => {
        copied.push(text);
        return Promise.resolve();
      },
    },
  });
  mockIPC((cmd, args) => {
    invoked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "watch_session")
      return { view: 1, columns: 80, rows: 24, scrollback: 5000, newline: null };
    if (cmd === "pane_link_resolve") {
      const { candidates } = args as { candidates: { kind: string; text: string }[] };
      return candidates.map((one) =>
        one.text === TEXT
          ? { kind: "url", target: TARGET, does: { does: "opens" } }
          : one.text === FTP_TEXT
            ? { kind: "url", target: one.text, does: { does: "refused", reason: FTP } }
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
  Reflect.deleteProperty(navigator, "platform");
});

const write = (term: Terminal, text: string) => new Promise<void>((done) => term.write(text, done));
const asked = (cmd: string) => invoked.filter((one) => one.cmd === cmd);

/** The pointer over the cell at (`col`, `row`), zero-based. */
const over = (col: number, row: number, init: MouseEventInit = {}): MouseEventInit => ({
  clientX: col * WIDTH + WIDTH / 2,
  clientY: row * CELL + CELL / 2,
  bubbles: true,
  cancelable: true,
  ...init,
});

/** The pane, written "see <text> now" on its first row, the pointer resting on the text. */
async function restingOn(text: string) {
  render(<SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />);
  const term = made[0];
  await waitFor(() => expect(term.cols).toBe(80));
  const drawn = term.element?.querySelector(".xterm-screen");
  if (!drawn) throw new Error("the pane opened no terminal screen");
  await write(term, `see ${text} now`);
  drawn.dispatchEvent(new MouseEvent("mousemove", over(8, 0)));
  await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
  // The core's answer reaches xterm, which then calls the link's hover.
  await new Promise((settle) => setTimeout(settle, 20));
  return { term, drawn };
}

/** A right-click on the cell, as the engine raises it: the press, `contextmenu`, the release. */
function rightClick(on: Element, col: number, row: number) {
  on.dispatchEvent(new MouseEvent("mousedown", over(col, row, { button: 2 })));
  on.dispatchEvent(new MouseEvent("contextmenu", over(col, row, { button: 2 })));
  on.dispatchEvent(new MouseEvent("mouseup", over(col, row, { button: 2 })));
}

const names = (menu: HTMLElement) => within(menu).getAllByRole("menuitem").map(rowName);
const under = (item: HTMLElement) =>
  item.querySelector('[data-part="description"]')?.textContent ?? null;

describe("a right-click over a link", () => {
  it("puts Open link and Copy link first, each with its target under it", async () => {
    const { drawn } = await restingOn(TEXT);

    rightClick(drawn, 8, 0);

    const menu = await screen.findByRole("menu", { name: "Terminal" });
    expect(names(menu).slice(0, 5)).toEqual([
      "Open link",
      "Copy link",
      "Copy",
      "Paste",
      "Select all",
    ]);
    const [open, copy] = within(menu).getAllByRole("menuitem");
    expect(under(open)).toBe(TARGET);
    expect(under(copy)).toBe(TARGET);
  });

  it("opens through the core on Open link, handed the text and never a target", async () => {
    const { drawn } = await restingOn(TEXT);
    rightClick(drawn, 8, 0);
    const menu = await screen.findByRole("menu", { name: "Terminal" });

    fireEvent.click(within(menu).getByRole("menuitem", { name: "Open link" }));

    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(1));
    expect(asked("pane_link_open")[0].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidate: { kind: "url", text: TEXT },
      confirmed: null,
    });
  });

  it("asks first on Open link when the core says so, as ⌘-click does", async () => {
    opens = {
      answer: "ask",
      reason: "its text names another address",
      shown: TEXT,
      target: TARGET,
    };
    const { drawn } = await restingOn(TEXT);
    rightClick(drawn, 8, 0);
    const menu = await screen.findByRole("menu", { name: "Terminal" });

    fireEvent.click(within(menu).getByRole("menuitem", { name: "Open link" }));

    const ask = await screen.findByRole("dialog", { name: "Open this link?" });
    fireEvent.click(within(ask).getByRole("button", { name: "Open link" }));
    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(2));
    expect(asked("pane_link_open")[1].args.confirmed).toBe(TARGET);
  });

  it("copies the resolved target on Copy link, and opens nothing", async () => {
    const { drawn } = await restingOn(TEXT);
    rightClick(drawn, 8, 0);
    const menu = await screen.findByRole("menu", { name: "Terminal" });

    fireEvent.click(within(menu).getByRole("menuitem", { name: "Copy link" }));

    await waitFor(() => expect(copied).toEqual([TARGET]));
    expect(asked("pane_link_open")).toHaveLength(0);
  });

  it("offers only Copy link over an address the core refuses", async () => {
    const { drawn } = await restingOn(FTP_TEXT);

    rightClick(drawn, 8, 0);

    const menu = await screen.findByRole("menu", { name: "Terminal" });
    expect(names(menu).slice(0, 2)).toEqual(["Copy link", "Copy"]);
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Copy link" }));
    await waitFor(() => expect(copied).toEqual([FTP_TEXT]));
  });

  it("has no link rows away from a link: Links in this pane… stands in their place", async () => {
    const { drawn } = await restingOn(TEXT);
    drawn.dispatchEvent(new MouseEvent("mousemove", over(60, 5)));
    await new Promise((settle) => setTimeout(settle, 20));

    rightClick(drawn, 60, 5);

    const menu = await screen.findByRole("menu", { name: "Terminal" });
    expect(names(menu).slice(0, 2)).toEqual(["Links in this pane…", "Copy"]);
    expect(names(menu)).not.toContain("Open link");
    expect(names(menu)).not.toContain("Copy link");
  });

  it("says Open link's ⌘-click at its end on a Mac, and no key to a reader: a click is no key", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    const { drawn } = await restingOn(TEXT);
    rightClick(drawn, 8, 0);
    const menu = await screen.findByRole("menu", { name: "Terminal" });

    const open = within(menu).getByRole("menuitem", { name: "Open link" });
    expect(open.querySelector('[data-part="shortcut"]')?.getAttribute("data-key")).toBe("⌘-click");
    expect(open.hasAttribute("aria-keyshortcuts")).toBe(false);
  });
});

describe("the menu from the keyboard", () => {
  it("has no link rows, though the pointer rests on one: the keyboard has no pointer", async () => {
    const { term } = await restingOn(TEXT);
    if (!term.textarea) throw new Error("the terminal has no textarea");
    term.focus();

    fireEvent.keyDown(term.textarea, { key: "ContextMenu" });
    fireEvent.keyUp(term.textarea, { key: "ContextMenu" });

    const menu = await screen.findByRole("menu", { name: "Terminal" });
    expect(names(menu)).not.toContain("Open link");
    expect(names(menu)).not.toContain("Copy link");
  });
});

describe("Links in this pane… on the terminal's menu", () => {
  /** The terminal's menu as a pane draws it, opened by a `contextmenu` on its terminal. */
  function drawn(link: Parameters<typeof linkRows>[0]["link"], mac = false) {
    const pressed: string[] = [];
    render(
      // The window's key registry, as `main.tsx` hands every menu its rows' keys.
      <MenuShortcuts.Provider value={(act) => menuRowKey(act, mac)}>
        <TerminalMenu
          rows={terminalRows({ selected: false, mac })}
          links={linkRows({ link, mac, lists: true })}
          onRun={(id) => pressed.push(id)}
          onAsked={() => {}}
          onOpenChange={() => {}}
          onGiveBack={() => {}}
          onPassKey={() => {}}
        >
          <div data-testid="terminal" />
        </TerminalMenu>
      </MenuShortcuts.Provider>,
    );
    fireEvent.contextMenu(screen.getByTestId("terminal"));
    return pressed;
  }

  it("takes the link rows' place with no link under the pointer, and is said with its key", async () => {
    const pressed = drawn(undefined, true);

    const menu = await screen.findByRole("menu", { name: "Terminal" });
    expect(names(menu)[0]).toBe("Links in this pane…");
    const row = within(menu).getByRole("menuitem", { name: "Links in this pane…" });
    expect(row.querySelector('[data-part="shortcut"]')?.getAttribute("data-key")).toBe("⇧⌘L");
    expect(row.getAttribute("aria-keyshortcuts")).toBe("Shift+Meta+L");
    fireEvent.click(row);
    expect(pressed).toEqual(["terminal.links"]);
  });

  it("says Ctrl+Shift+L off a Mac", async () => {
    drawn(undefined, false);

    const menu = await screen.findByRole("menu", { name: "Terminal" });
    const row = within(menu).getByRole("menuitem", { name: "Links in this pane…" });
    expect(row.getAttribute("aria-keyshortcuts")).toBe("Control+Shift+L");
  });

  it("follows the terminal's rows over a link, the link rows first", async () => {
    drawn({
      candidate: { kind: "url", text: TEXT },
      seen: { kind: "url", target: TARGET, does: { does: "opens" } },
    });

    const menu = await screen.findByRole("menu", { name: "Terminal" });
    expect(names(menu)).toEqual([
      "Open link",
      "Copy link",
      "Copy",
      "Paste",
      "Select all",
      "Links in this pane…",
    ]);
  });

  it("is not drawn while the pane has no list to open", async () => {
    render(
      <TerminalMenu
        rows={terminalRows({ selected: false, mac: false })}
        links={linkRows({ link: undefined, mac: false, lists: false })}
        onRun={() => {}}
        onAsked={() => {}}
        onOpenChange={() => {}}
        onGiveBack={() => {}}
        onPassKey={() => {}}
      >
        <div data-testid="terminal" />
      </TerminalMenu>,
    );
    fireEvent.contextMenu(screen.getByTestId("terminal"));

    const menu = await screen.findByRole("menu", { name: "Terminal" });
    expect(names(menu)[0]).toBe("Copy");
  });
});
