// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import { catalogued, type Offer } from "./actions";
import { paneMenuOf } from "./LayoutPanes";
import { SessionPane } from "./SessionPane";
import { Menued } from "./Menus";
import { ProjectShownProvider } from "./projectShown";
import {
  asksByPointer,
  asksForTheMenu,
  NOTHING_SELECTED,
  NOTHING_TO_PASTE,
  SHIFT_F10,
  terminalRows,
} from "./terminalRows";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **A right-click on a terminal offers the terminal's own rows** (the operator, 2026-10-13: "in
 * terminal we don't have right click -> copy/paste").
 *
 * The real xterm and the real Radix menu, with the IPC mocked: what is asserted is the menu a
 * `contextmenu` event opens, what a row puts on a clipboard that records it, and every byte
 * the core is asked to write to the session's pty — which, for every row here, is none.
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

/** Every byte the core was asked to write to the session's pty, in order. */
let written: number[] = [];
/** Everything put on the clipboard, in order. */
let copied: string[] = [];
/** What the core answers a read of the clipboard: its text, none, or a refusal's sentence. */
let held: { text: string | null } | { refused: string } = { text: null };
/** Every command the window invoked, by name, in order. */
let invoked: string[] = [];

/** The page's clipboard, for the test: jsdom has none. */
function clipboardIs(clipboard: { writeText: (text: string) => Promise<void> }) {
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: clipboard });
}

beforeEach(() => {
  made.length = 0;
  written = [];
  copied = [];
  held = { text: null };
  invoked = [];
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
  clipboardIs({
    writeText: (text: string) => {
      copied.push(text);
      return Promise.resolve();
    },
  });
  mockIPC((cmd, args) => {
    invoked.push(cmd);
    if (cmd === "watch_session")
      return { view: 1, columns: 80, rows: 24, scrollback: 5000, newline: null };
    if (cmd === "clipboard_text") {
      // A refusal is the command's error, which the bindings hand on as a sentence.
      if ("refused" in held) throw held.refused;
      return held.text;
    }
    const sent = args as { text?: string; bytes?: number[] };
    if (cmd === "send_input" && sent.text !== undefined)
      written.push(...new TextEncoder().encode(sent.text));
    if (cmd === "send_input_bytes" && sent.bytes !== undefined) written.push(...sent.bytes);
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const write = (term: Terminal, text: string) => new Promise<void>((done) => term.write(text, done));

function offer(id: string, title: string): Offer {
  return { id, title, available: true, reason: "", does: { verb: "nothing" } };
}

/** The pane's own rows, as `LayoutPanes` hands the catalogue's to a pane. */
const paneRows = catalogued([
  offer("chat.new", "New tab"),
  offer("pane.split.right", "Split right"),
  offer("pane.close", "End chat 1"),
]);

async function opened(pressed: (offer: Offer) => void = () => {}, onFocus: () => void = () => {}) {
  render(
    <SessionPane
      plane="/planes/one"
      session={1}
      focused={false}
      onFocus={onFocus}
      menu={{ offers: paneRows, onPress: pressed }}
    />,
  );
  const term = made[0];
  await waitFor(() => expect(term.cols).toBe(80));
  const drawn = term.element?.querySelector(".xterm-screen");
  if (!drawn) throw new Error("the pane opened no terminal screen");
  return { term, drawn };
}

/** The pointer over the cell at (`col`, `row`), zero-based. */
const over = (col: number, row: number) => ({
  clientX: col * WIDTH + WIDTH / 2,
  clientY: row * CELL + CELL / 2,
  bubbles: true,
  cancelable: true,
});

/** A right-click, as the engine raises it: the press, then `contextmenu`. */
function rightClick(on: Element, init: MouseEventInit = {}) {
  on.dispatchEvent(new MouseEvent("mousedown", { ...over(10, 3), button: 2, ...init }));
  const asked = new MouseEvent("contextmenu", { ...over(10, 3), button: 2, ...init });
  on.dispatchEvent(asked);
  on.dispatchEvent(new MouseEvent("mouseup", { ...over(10, 3), button: 2, ...init }));
  return asked;
}

const row = (menu: HTMLElement, name: string) => within(menu).getByRole("menuitem", { name });

describe("the terminal's rows", () => {
  it("are Copy, Paste and Select all, with a Mac's keys on a Mac and none elsewhere", () => {
    expect(terminalRows({ selected: true, mac: true }).map((r) => [r.title, r.keys])).toEqual([
      ["Copy", "⌘C"],
      ["Paste", "⌘V"],
      ["Select all", "⌘A"],
    ]);
    expect(terminalRows({ selected: true, mac: false }).map((r) => [r.title, r.keys])).toEqual([
      ["Copy", undefined],
      ["Paste", undefined],
      ["Select all", undefined],
    ]);
  });

  it("turn Copy off with nothing selected, and say why", () => {
    const [copy] = terminalRows({ selected: false, mac: true });
    expect(copy.available).toBe(false);
    expect(copy.reason).toBe(NOTHING_SELECTED);
    expect(terminalRows({ selected: true, mac: true })[0].available).toBe(true);
  });

  it("keep Paste on, a terminal taking input whenever it is drawn", () => {
    for (const selected of [true, false]) {
      const paste = terminalRows({ selected, mac: true })[1];
      expect(paste.available).toBe(true);
      expect(paste.reason).toBe("");
    }
  });
});

describe("a right-click on a terminal", () => {
  it("opens the terminal's rows first, then the pane's under a line", async () => {
    const { drawn } = await opened();

    const asked = rightClick(drawn);

    const menu = await screen.findByRole("menu");
    expect(asked.defaultPrevented).toBe(true);
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((item) => item.getAttribute("aria-label")),
    ).toEqual(["Copy", "Paste", "Select all", "New tab", "Split right", "End chat 1"]);
    // The terminal's rows are the first group, and the pane's follow in the groups every
    // menu on the panes has (`menuModel.menuOf`), a line before each: what the pane starts,
    // how it is split, and last what ends the chat.
    expect(within(menu).getAllByRole("separator")).toHaveLength(3);
    // What the menu is about is said once, on the menu, and by no row.
    expect(menu.getAttribute("aria-label")).toBe("Terminal");
    expect(written).toEqual([]);
  });

  it("offers Copy off with nothing selected, and Paste and Select all on", async () => {
    const { drawn } = await opened();

    rightClick(drawn);

    const menu = await screen.findByRole("menu");
    expect(row(menu, "Copy").getAttribute("aria-disabled")).toBe("true");
    expect(row(menu, "Copy")).toHaveAccessibleDescription(NOTHING_SELECTED);
    expect(row(menu, "Paste").getAttribute("aria-disabled")).toBeNull();
    expect(tipOf(row(menu, "Paste"))).toBeNull();
    expect(row(menu, "Select all").getAttribute("aria-disabled")).toBeNull();
  });

  it("says a Mac's keys at the rows' ends, outside their names", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    try {
      const { drawn } = await opened();

      rightClick(drawn);

      const menu = await screen.findByRole("menu");
      expect(
        within(menu)
          .getAllByRole("menuitem")
          .slice(0, 3)
          .map((item) => [
            item.getAttribute("aria-label"),
            item.querySelector(".ui-menu-title")?.textContent,
            item.querySelector(".ui-menu-keys")?.textContent,
          ]),
      ).toEqual([
        ["Copy", "Copy", "⌘C"],
        ["Paste", "Paste", "⌘V"],
        ["Select all", "Select all", "⌘A"],
      ]);
      // Why Copy cannot run is the one line under its words, as in every menu.
      expect(row(menu, "Copy").querySelector(".ui-menu-note")?.textContent).toBe(NOTHING_SELECTED);
      expect(row(menu, "Paste").querySelector(".ui-menu-note")).toBeNull();
    } finally {
      // Back to the platform jsdom reports, which is its prototype's.
      Reflect.deleteProperty(navigator, "platform");
    }
  });

  it("copies exactly what is selected, keeps the selection, and sends the program nothing", async () => {
    const { term, drawn } = await opened();
    await write(term, "one two  \r\nthree");
    term.select(4, 0, 3);
    expect(term.getSelection()).toBe("two");

    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Copy"));

    await waitFor(() => expect(copied).toEqual(["two"]));
    expect(term.getSelection()).toBe("two");
    expect(written).toEqual([]);
  });

  it("copies lines as the keyboard's copy does: xterm's own selection text", async () => {
    const { term, drawn } = await opened();
    await write(term, "one two  \r\nthree");
    term.selectLines(0, 1);
    const selected = term.getSelection();
    // Two rows are two lines, and what becomes of a row's trailing blanks is the library's
    // rule: whatever it selected is what is copied, to the character.
    expect(selected.split("\n").map((line) => line.trimEnd())).toEqual(["one two", "three"]);

    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Copy"));

    await waitFor(() => expect(copied).toEqual([selected]));
  });

  it("selects everything on Select all, after which Copy can run", async () => {
    const { term, drawn } = await opened();
    await write(term, "one\r\ntwo");

    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Select all"));

    await waitFor(() => expect(term.hasSelection()).toBe(true));
    expect(term.getSelection()).toContain("one\ntwo");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());

    rightClick(drawn);
    expect(row(await screen.findByRole("menu"), "Copy").getAttribute("aria-disabled")).toBeNull();
    expect(written).toEqual([]);
  });

  it("says so in the pane when the clipboard refuses, and sends the program nothing", async () => {
    clipboardIs({ writeText: () => Promise.reject(new Error("no")) });
    const { term, drawn } = await opened();
    await write(term, "one");
    term.selectAll();

    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Copy"));

    await waitFor(() => {
      term.selectAll();
      expect(term.getSelection()).toContain("the selection could not be put on the clipboard");
    });
    expect(written).toEqual([]);
  });

  /** Every input command the window sent the core. */
  const sends = () => invoked.filter((cmd) => cmd === "send_input" || cmd === "send_input_bytes");
  const bytes = (text: string) => [...new TextEncoder().encode(text)];

  it("pastes the clipboard's text by the terminal's own paste, once, and sends nothing else", async () => {
    held = { text: "echo one\necho two" };
    const { term, drawn } = await opened();
    const pasting = vi.spyOn(term, "paste");

    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Paste"));

    await waitFor(() => expect(pasting).toHaveBeenCalledTimes(1));
    expect(pasting).toHaveBeenCalledWith("echo one\necho two");
    // What reached the program is what xterm's paste makes of it (a line break is a carriage
    // return, as the keyboard's paste sends it), in one send, and nothing before or after.
    await vi.waitFor(() => expect(written).toEqual(bytes("echo one\recho two")));
    expect(sends()).toEqual(["send_input"]);
    // The clipboard was read once, by the core, and the page's own clipboard was not touched.
    expect(invoked.filter((cmd) => cmd === "clipboard_text")).toHaveLength(1);
    expect(copied).toEqual([]);
  });

  it("brackets the paste when the program asked, so several lines cannot run one by one", async () => {
    held = { text: "echo one\necho two\n" };
    const { term, drawn } = await opened();
    await write(term, "\x1b[?2004h");

    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Paste"));

    await vi.waitFor(() =>
      expect(written).toEqual(bytes("\x1b[200~echo one\recho two\r\x1b[201~")),
    );
    expect(sends()).toEqual(["send_input"]);
  });

  it("reads the clipboard on the press and at no other time", async () => {
    held = { text: "never asked for" };
    const { term, drawn } = await opened();
    await write(term, "one");

    // Opening the menu, selecting, copying and closing it read nothing.
    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Select all"));
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Copy"));
    await waitFor(() => expect(copied).toHaveLength(1));

    expect(invoked).not.toContain("clipboard_text");
    expect(written).toEqual([]);
  });

  for (const empty of [null, ""]) {
    it(`pastes nothing from a clipboard holding ${empty === null ? "no text" : "an empty text"}, and says so`, async () => {
      held = { text: empty };
      const { term, drawn } = await opened();
      const pasting = vi.spyOn(term, "paste");

      rightClick(drawn);
      fireEvent.click(row(await screen.findByRole("menu"), "Paste"));

      await waitFor(() => {
        term.selectAll();
        expect(term.getSelection()).toContain(NOTHING_TO_PASTE);
      });
      expect(pasting).not.toHaveBeenCalled();
      expect(written).toEqual([]);
    });
  }

  it("says the core's sentence when the read is refused, and pastes nothing", async () => {
    held = { refused: "The clipboard holds more than 1 MiB of text." };
    const { term, drawn } = await opened();
    const pasting = vi.spyOn(term, "paste");

    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Paste"));

    await waitFor(() => {
      term.selectAll();
      expect(term.getSelection()).toContain("The clipboard holds more than 1 MiB of text.");
    });
    expect(pasting).not.toHaveBeenCalled();
    expect(written).toEqual([]);
  });

  it("copies nothing, and says so, when the selection went while the menu was up", async () => {
    const { term, drawn } = await opened();
    await write(term, "one");
    term.selectAll();

    rightClick(drawn);
    const copy = row(await screen.findByRole("menu"), "Copy");
    // Output that arrives while the menu is up can clear the selection under it.
    term.clearSelection();
    fireEvent.click(copy);

    await waitFor(() => {
      term.selectAll();
      expect(term.getSelection()).toContain("nothing is selected any more");
    });
    // Not an empty string, which would have emptied the clipboard.
    expect(copied).toEqual([]);
    expect(written).toEqual([]);
  });

  it("carries a pane's row out through the pane", async () => {
    const pressed: string[] = [];
    const { drawn } = await opened((offer) => pressed.push(offer.id));

    rightClick(drawn);
    fireEvent.click(row(await screen.findByRole("menu"), "Split right"));

    expect(pressed).toEqual(["pane.split.right"]);
  });

  it("is handed the panes' own rows by id, and no row the panes' menu does not list", () => {
    const all = catalogued([
      offer("chat.new", "New tab"),
      offer("pane.close", "End chat 1"),
      offer("tab.rename:1", "Rename tab"),
    ]);

    expect([...paneMenuOf((id) => all.get(id)).keys()]).toEqual(["chat.new", "pane.close"]);
  });

  it("gives the keyboard back to the terminal when the menu closes", async () => {
    const { term, drawn } = await opened();

    rightClick(drawn);
    const menu = await screen.findByRole("menu");
    fireEvent.keyDown(menu, { key: "Escape" });

    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(document.activeElement).toBe(term.textarea);
  });
});

describe("a terminal's menu inside the panes' menu", () => {
  /** The pane as the window draws it: inside the box the panes' own menu is on. */
  async function nested() {
    render(
      <Menued on={{ on: "pane" }} offers={paneRows} onPress={() => {}}>
        <div className="panes">
          <SessionPane
            plane="/planes/one"
            session={1}
            focused={false}
            onFocus={() => {}}
            menu={{ offers: paneRows, onPress: () => {} }}
          />
        </div>
      </Menued>,
    );
    const term = made[0];
    await waitFor(() => expect(term.cols).toBe(80));
    const drawn = term.element?.querySelector(".xterm-screen");
    if (!drawn) throw new Error("the pane opened no terminal screen");
    return { term, drawn };
  }

  it("opens one menu, the terminal's, and not the panes' as well", async () => {
    const { drawn } = await nested();

    rightClick(drawn);

    await screen.findByRole("menu");
    expect(screen.getAllByRole("menu")).toHaveLength(1);
    expect(row(screen.getByRole("menu"), "Copy")).toBeTruthy();
  });

  it("gives the keyboard back to the terminal after Shift+right-click past a program", async () => {
    // **What a real engine must confirm**: jsdom has no default action for a press, so it
    // cannot show the keyboard leaving the terminal when the press is hidden from xterm (the
    // engine's default for a press on what cannot take focus). What is held here is the two
    // things that keep it: the press's default is prevented, and the terminal is focused
    // outright when the menu closes, whatever the engine thought had the keyboard before.
    const { term, drawn } = await nested();
    await write(term, "\x1b[?1049h\x1b[?1000h");
    const focusing = vi.spyOn(term, "focus");

    const press = new MouseEvent("mousedown", { ...over(10, 3), button: 2, shiftKey: true });
    drawn.dispatchEvent(press);
    expect(press.defaultPrevented).toBe(true);
    drawn.dispatchEvent(
      new MouseEvent("contextmenu", { ...over(10, 3), button: 2, shiftKey: true }),
    );
    const menu = await screen.findByRole("menu");
    expect(screen.getAllByRole("menu")).toHaveLength(1);

    // As the engine would have left it: the keyboard on nothing when the menu closes.
    focusing.mockClear();
    fireEvent.keyDown(menu, { key: "Escape" });

    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await waitFor(() => expect(focusing).toHaveBeenCalled());
    expect(document.activeElement).toBe(term.textarea);
    expect(written).toEqual([]);
  });

  it("is held back while its project is not in front, like every overlay", async () => {
    // A pane of a project behind another stays drawn, and what it had put over the window
    // does not: the menu is Radix's context menu through `overlays/context-menu`.
    const pane = (shown: boolean) => (
      <ProjectShownProvider value={shown}>
        <SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />
      </ProjectShownProvider>
    );
    const { rerender } = render(pane(true));
    const term = made[0];
    await waitFor(() => expect(term.cols).toBe(80));
    const drawn = term.element?.querySelector(".xterm-screen");
    if (!drawn) throw new Error("the pane opened no terminal screen");

    rightClick(drawn);
    expect(row(await screen.findByRole("menu"), "Paste")).toBeTruthy();

    rerender(pane(false));
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(invoked).not.toContain("clipboard_text");
    expect(written).toEqual([]);
  });

  it("leaves a plain right-click to a program that has the mouse: neither menu opens", async () => {
    const { term, drawn } = await nested();
    await write(term, "\x1b[?1049h\x1b[?1000h");

    rightClick(drawn);

    await vi.waitFor(() => expect(written.length).toBeGreaterThan(0));
    expect(screen.queryByRole("menu")).toBeNull();
  });
});

/** A default-encoding report: `CSI M`, then the button, column and row, each plus 32, one-based. */
const report = (button: number, col: number, row: number) => [
  0x1b,
  0x5b,
  0x4d,
  button + 32,
  col + 1 + 32,
  row + 1 + 32,
];

describe("a right-click while the program has the mouse", () => {
  it("goes to the program, and opens no menu", async () => {
    const { term, drawn } = await opened();
    await write(term, "\x1b[?1049h\x1b[?1000h");

    const asked = rightClick(drawn);

    // The press and the release of button 2, as the program asked for them.
    await vi.waitFor(() => expect(written).toEqual([...report(2, 10, 3), ...report(3, 10, 3)]));
    expect(screen.queryByRole("menu")).toBeNull();
    // And not the engine's own menu either.
    expect(asked.defaultPrevented).toBe(true);
  });

  it("opens the menu with Shift held, and the program is sent nothing", async () => {
    const { term, drawn } = await opened();
    await write(term, "\x1b[?1049h\x1b[?1000h");

    rightClick(drawn, { shiftKey: true });

    const menu = await screen.findByRole("menu");
    expect(row(menu, "Select all")).toBeTruthy();
    expect(written).toEqual([]);
  });

  it("keeps the Shift press from xterm, which on a Mac would report it", async () => {
    // xterm takes Shift as its own way past the program everywhere but on a Mac, where it
    // reports the press with Shift among the modifiers. Which it is, xterm decides once from
    // the platform, and under a test that is never a Mac — so what is held here is that the
    // press does not reach the element xterm listens on, and that the pane still takes the
    // keyboard, which React's listener would have done had the press reached it.
    let focused = 0;
    const { term, drawn } = await opened(undefined, () => {
      focused += 1;
    });
    const reached: MouseEvent[] = [];
    term.element?.addEventListener("mousedown", (event) => reached.push(event));

    await write(term, "\x1b[?1049h\x1b[?1000h");
    rightClick(drawn, { shiftKey: true });
    expect(reached).toHaveLength(0);
    expect(focused).toBe(1);

    // A plain press is the program's, and one with no program holding the mouse is xterm's.
    fireEvent.keyDown(await screen.findByRole("menu"), { key: "Escape" });
    rightClick(drawn);
    expect(reached).toHaveLength(1);
    await write(term, "\x1b[?1000l");
    rightClick(drawn, { shiftKey: true });
    expect(reached).toHaveLength(2);
  });

  it("takes a Mac's Ctrl+click as the right button it stands for", async () => {
    expect(asksByPointer({ button: 2, ctrlKey: false }, false)).toBe(true);
    expect(asksByPointer({ button: 0, ctrlKey: true }, true)).toBe(true);
    // Off a Mac, Ctrl with the main button is a click, and the program's.
    expect(asksByPointer({ button: 0, ctrlKey: true }, false)).toBe(false);
    expect(asksByPointer({ button: 0, ctrlKey: false }, true)).toBe(false);

    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    try {
      const { term, drawn } = await opened();
      const reached: MouseEvent[] = [];
      term.element?.addEventListener("mousedown", (event) => reached.push(event));
      await write(term, "\x1b[?1049h\x1b[?1000h");

      // Ctrl+Shift+click: the press is kept from xterm, so the program is not sent it, and
      // the `contextmenu` the engine raises for it opens the menu.
      const held = { ...over(10, 3), button: 0, ctrlKey: true, shiftKey: true };
      const press = new MouseEvent("mousedown", held);
      drawn.dispatchEvent(press);
      drawn.dispatchEvent(new MouseEvent("contextmenu", held));

      expect(reached).toHaveLength(0);
      expect(press.defaultPrevented).toBe(true);
      expect(await screen.findByRole("menu")).toBeTruthy();
      expect(written).toEqual([]);
    } finally {
      Reflect.deleteProperty(navigator, "platform");
    }
  });

  it("is the terminal's again once the program lets the mouse go", async () => {
    const { term, drawn } = await opened();
    await write(term, "\x1b[?1049h\x1b[?1000h");
    await write(term, "\x1b[?1000l\x1b[?1049l");

    rightClick(drawn);

    expect(await screen.findByRole("menu")).toBeTruthy();
    expect(written).toEqual([]);
  });
});

describe("the menu from the keyboard", () => {
  for (const key of [{ key: "F10", shiftKey: true }, { key: "ContextMenu" }]) {
    const named = key.shiftKey ? "Shift+F10" : "the menu key";

    it(`opens on ${named} in the terminal, and the program is sent nothing`, async () => {
      const { term } = await opened();
      if (!term.textarea) throw new Error("the terminal has no textarea");
      term.focus();

      fireEvent.keyDown(term.textarea, key);
      fireEvent.keyUp(term.textarea, key);

      const menu = await screen.findByRole("menu");
      expect(row(menu, "Copy")).toBeTruthy();
      expect(written).toEqual([]);
    });
  }

  it("hands Shift+F10 back on a second press: the menu closes and the program gets the key", async () => {
    const { term } = await opened();
    if (!term.textarea) throw new Error("the terminal has no textarea");
    term.focus();

    fireEvent.keyDown(term.textarea, { key: "F10", shiftKey: true });
    const menu = await screen.findByRole("menu");
    expect(written).toEqual([]);

    fireEvent.keyDown(menu, { key: "F10", shiftKey: true });

    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await vi.waitFor(() => expect(written).toEqual([...new TextEncoder().encode(SHIFT_F10)]));
    expect(SHIFT_F10).toBe("\x1b[21;2~");
    expect(document.activeElement).toBe(term.textarea);
  });

  it("rests on no row when opened from the keyboard, so Enter pastes nothing", async () => {
    // With nothing selected Copy cannot run, and the first row that can is Paste: a menu that
    // put the keyboard there would paste on the menu key and Enter.
    held = { text: "rm -rf one\nrm -rf two\n" };
    const { term } = await opened();
    if (!term.textarea) throw new Error("the terminal has no textarea");
    const pasting = vi.spyOn(term, "paste");
    term.focus();

    fireEvent.keyDown(term.textarea, { key: "ContextMenu" });
    const menu = await screen.findByRole("menu");
    // The keyboard is on the menu itself, and on no row of it.
    await waitFor(() => expect(document.activeElement).toBe(menu));
    expect(within(menu).queryAllByRole("menuitem")).not.toContain(document.activeElement);

    fireEvent.keyDown(document.activeElement ?? menu, { key: "Enter" });
    fireEvent.keyUp(document.activeElement ?? menu, { key: "Enter" });
    fireEvent.keyDown(document.activeElement ?? menu, { key: " " });

    await new Promise((soon) => setTimeout(soon, 50));
    expect(pasting).not.toHaveBeenCalled();
    expect(invoked).not.toContain("clipboard_text");
    expect(written).toEqual([]);
    expect(screen.queryByRole("menu")).toBe(menu);
  });

  it("still moves by the arrows and by a letter, and closes on Escape", async () => {
    const { term } = await opened();
    if (!term.textarea) throw new Error("the terminal has no textarea");
    term.focus();
    fireEvent.keyDown(term.textarea, { key: "ContextMenu" });
    const menu = await screen.findByRole("menu");
    await waitFor(() => expect(document.activeElement).toBe(menu));

    // The first arrow goes to the first row that can run, the last to the last.
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    await waitFor(() => expect(document.activeElement).toBe(row(menu, "Paste")));
    fireEvent.keyDown(document.activeElement ?? menu, { key: "ArrowDown" });
    await waitFor(() => expect(document.activeElement).toBe(row(menu, "Select all")));
    // A letter finds a row by its words.
    fireEvent.keyDown(document.activeElement ?? menu, { key: "p" });
    await waitFor(() => expect(document.activeElement).toBe(row(menu, "Paste")));

    fireEvent.keyDown(document.activeElement ?? menu, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(document.activeElement).toBe(term.textarea);
    expect(written).toEqual([]);
  });

  it("takes Shift+F10 held down as one press: it neither closes the menu nor sends the key", async () => {
    const { term } = await opened();
    if (!term.textarea) throw new Error("the terminal has no textarea");
    term.focus();

    fireEvent.keyDown(term.textarea, { key: "F10", shiftKey: true });
    const menu = await screen.findByRole("menu");
    // The repeats of the held key land on the menu, which now has the keyboard.
    for (let i = 0; i < 5; i += 1)
      fireEvent.keyDown(menu, { key: "F10", shiftKey: true, repeat: true });

    await new Promise((soon) => setTimeout(soon, 50));
    expect(screen.queryByRole("menu")).toBe(menu);
    expect(written).toEqual([]);
  });

  it("opens nothing on a repeat of Shift+F10 or of the menu key in the terminal", async () => {
    const { term } = await opened();
    if (!term.textarea) throw new Error("the terminal has no textarea");
    term.focus();

    fireEvent.keyDown(term.textarea, { key: "F10", shiftKey: true, repeat: true });
    fireEvent.keyDown(term.textarea, { key: "ContextMenu", repeat: true });

    await new Promise((soon) => setTimeout(soon, 50));
    expect(screen.queryByRole("menu")).toBeNull();
    expect(written).toEqual([]);
  });

  it("opens on the menu key while the program has the mouse too", async () => {
    const { term } = await opened();
    if (!term.textarea) throw new Error("the terminal has no textarea");
    await write(term, "\x1b[?1049h\x1b[?1000h");
    term.focus();

    fireEvent.keyDown(term.textarea, { key: "ContextMenu" });

    expect(await screen.findByRole("menu")).toBeTruthy();
    expect(written).toEqual([]);
  });

  it("leaves Shift+F10 to a program that has the mouse, as the bytes it always was", async () => {
    const { term } = await opened();
    if (!term.textarea) throw new Error("the terminal has no textarea");
    await write(term, "\x1b[?1049h\x1b[?1000h");
    term.focus();

    fireEvent.keyDown(term.textarea, { key: "F10", keyCode: 121, shiftKey: true });

    await vi.waitFor(() => expect(written).toEqual([...new TextEncoder().encode("\x1b[21;2~")]));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("takes neither key with another modifier held", () => {
    const pressed = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);
    expect(asksForTheMenu(pressed({ key: "F10", shiftKey: true, ctrlKey: true }), false)).toBe(
      false,
    );
    expect(asksForTheMenu(pressed({ key: "F10" }), false)).toBe(false);
    expect(asksForTheMenu(pressed({ key: "ContextMenu", altKey: true }), false)).toBe(false);
  });
});
