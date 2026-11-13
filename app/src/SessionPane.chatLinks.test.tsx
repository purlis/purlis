// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import type { ChatCard, LinkSeen } from "./bindings";
import type { Offer } from "./actions";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { candidatesIn } from "./paneLinks";
import { SessionPane } from "./SessionPane";

/**
 * **A chat's number in a pane** (#6217): a real xterm is written `chat 641`, and the pointer
 * hovers and ⌘-clicks it. What is asserted is what the core was asked, and what the window did
 * with its answer: an open chat shown through the pane's rows, an ended one's card beside the
 * link, nothing for the pane's own chat. `#641` and a title are never asked about.
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

const CARD: ChatCard = {
  number: 641,
  chat: "01K6W0RKER000000000000000B",
  title: "Signed commits #5607",
  ended: "2026-11-11T11:08:00",
  place: "smart-ide",
  started_by: "steward 4",
  screen_kept: true,
  task: {
    id: "01K9DISPATCH0000000000000A",
    asker: 0,
    name: "Signed commits #5607",
    chat: null,
    persona: "steward",
    how: "done",
    outcome: "done",
    folds: true,
    report: "Signed all 12.",
    changed: null,
    ended: "2026-11-11T11:08:00",
    place: "smart-ide",
    branch: null,
    reopens: true,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
  },
};

/** What the core says each chat number is: 652 this pane's, 660 open in docs, 641 ended. */
const SEEN: Record<string, LinkSeen> = {
  "chat 652": {
    kind: "chat",
    target: "chat 652",
    shown: "#652 · Grill #5820: chat links",
    does: { does: "opens", into: "window" },
    chat: { is: "this_chat", number: 652 },
  },
  "chat 660": {
    kind: "chat",
    target: "chat 660",
    shown: "#660 · Docs site: the setup page",
    does: { does: "opens", into: "window" },
    chat: { is: "open", number: 660, workspace: "docs" },
  },
  "chat 641": {
    kind: "chat",
    target: "chat 641",
    shown: "#641 · Signed commits #5607",
    does: { does: "opens", into: "window" },
    chat: { is: "ended", card: CARD },
  },
};

const OPENS: Record<string, unknown> = {
  "chat 652": { answer: "chat", go: { to: "this_chat" } },
  "chat 660": { answer: "chat", go: { to: "chat", session: 660 } },
  "chat 641": { answer: "chat", go: { to: "card", card: CARD } },
};

let invoked: { cmd: string; args: Record<string, unknown> }[] = [];
const asked = (cmd: string) => invoked.filter((one) => one.cmd === cmd);

beforeEach(() => {
  made.length = 0;
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
  mockIPC((cmd, args) => {
    invoked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "watch_session")
      return { view: 1, columns: 80, rows: 24, scrollback: 5000, newline: null };
    if (cmd === "pane_link_resolve") {
      const { candidates } = args as { candidates: { text: string }[] };
      return candidates.map((one) => SEEN[one.text] ?? null);
    }
    if (cmd === "pane_link_open") {
      const { candidate } = args as { candidate: { text: string } };
      return OPENS[candidate.text];
    }
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

/** Chat 652's pane, whose rows run through `pressed`. */
async function opened(pressed: (offer: Offer) => void = () => {}) {
  render(
    <SessionPane
      plane="/planes/one"
      session={652}
      focused={false}
      onFocus={() => {}}
      menu={{ offers: new Map(), onPress: pressed }}
    />,
  );
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

/** The pane written "  <text> · finished" on its first row, the pointer on `text`. */
async function onAChat(text: string, pressed?: (offer: Offer) => void) {
  const pane = await opened(pressed);
  await write(pane.term, `  ${text} · finished`);
  hover(pane.drawn, 4, 0);
  await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
  return pane;
}

describe("a chat's number in a pane's text", () => {
  it("is the word chat and a number, and never a bare #641 or a title", () => {
    expect(candidatesIn("  chat 641 · Signed commits #5607")).toEqual([
      { kind: "chat", text: "chat 641", start: 2, end: 10 },
    ]);
    expect(candidatesIn("Chat 641 ended")).toEqual([
      { kind: "chat", text: "Chat 641", start: 0, end: 8 },
    ]);
    for (const line of [
      "issue #641",
      "Signed commits #5607",
      "chat #641",
      "chats 641",
      "chat 0641",
      "groupchat 641",
      "chat 641x",
      "#chat 641",
    ])
      expect(candidatesIn(line), line).toEqual([]);
    // Words inside an address are the address's.
    expect(candidatesIn("https://x.example/chat 641").map((one) => one.kind)).toEqual(["url"]);
  });
});

describe("a chat link in a chat's pane", () => {
  it("is asked of the core as a chat, from the pane's project and chat", async () => {
    await onAChat("chat 660");

    expect(asked("pane_link_resolve")[0].args).toEqual({
      from: { plane: "/planes/one", session: 652 },
      candidates: [{ kind: "chat", text: "chat 660" }],
    });
  });

  it("says its number and title, where it is and what ⌘-click does", async () => {
    Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
    await onAChat("chat 660");

    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent("#660 · Docs site: the setup page");
    expect(tip).toHaveTextContent("Open in docs · ⌘-click shows it there");
  });

  it("shows an open chat's tab on Ctrl+click, through the pane's rows", async () => {
    const pressed = vi.fn();
    const { drawn } = await onAChat("chat 660", pressed);

    click(drawn, 4, 0, { ctrlKey: true });

    await waitFor(() => expect(pressed).toHaveBeenCalledTimes(1));
    expect((pressed.mock.calls[0][0] as Offer).does).toEqual({ verb: "focusChat", session: 660 });
    expect(asked("pane_link_open")[0].args.candidate).toEqual({ kind: "chat", text: "chat 660" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens an ended chat's card beside the link, and Read it runs its view", async () => {
    const pressed = vi.fn();
    const { drawn } = await onAChat("chat 641", pressed);
    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent("#641 · Signed commits #5607");
    expect(tip).toHaveTextContent("Done 11:08 · Ctrl-click shows its card");

    click(drawn, 4, 0, { ctrlKey: true });

    const card = await screen.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    expect(within(card).getByTestId("closed-chat-card")).toHaveTextContent("Signed all 12.");
    // The card stands where the hover's tooltip did: the tooltip goes.
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
    await userEvent.click(within(card).getByRole("button", { name: "Read it" }));
    await waitFor(() => expect(pressed).toHaveBeenCalledTimes(1));
    expect((pressed.mock.calls[0][0] as Offer).does).toMatchObject({ verb: "openView" });
    await waitFor(() => expect(screen.queryByTestId("closed-chat-card")).toBeNull());
  });

  it("is this chat in its own pane, and Ctrl+click does nothing more", async () => {
    const pressed = vi.fn();
    const { drawn } = await onAChat("chat 652", pressed);
    const tip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tip).toHaveTextContent("This chat · Ctrl-click does nothing more");

    click(drawn, 4, 0, { ctrlKey: true });

    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(1));
    await new Promise((settle) => setTimeout(settle, 20));
    expect(pressed).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("leaves #641 and a chat's title as plain text: the core is asked nothing", async () => {
    const { term, drawn } = await opened();
    await write(term, "  #641 · Signed commits #5607");

    hover(drawn, 3, 0);
    hover(drawn, 12, 0);
    click(drawn, 3, 0, { ctrlKey: true });
    await new Promise((settle) => setTimeout(settle, 50));

    expect(asked("pane_link_resolve")).toHaveLength(0);
    expect(asked("pane_link_open")).toHaveLength(0);
  });
});

/**
 * **The card from the Links list or the menu's Open link stands at the link** (#6442): both close
 * on the press, so the row it came from is gone, and the card goes where a ⌘-click's does (the
 * prototype's D7). The terminal's screen is given a box here, as jsdom lays nothing out.
 */
describe("an ended chat's card opened from the keyboard's ways", () => {
  const SCREEN = { left: 100, top: 50, width: 80 * WIDTH, height: 24 * CELL };
  // Row 2, column 2: 100 + 2 × 8 across, 50 + 2 × 17 down.
  const AT_THE_LINK = { left: "116px", top: "84px" };

  beforeEach(() => {
    const real = Element.prototype.getBoundingClientRect;
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      if (!this.classList.contains("xterm-screen")) return real.call(this);
      const { left, top, width, height } = SCREEN;
      return {
        left,
        top,
        width,
        height,
        x: left,
        y: top,
        right: left + width,
        bottom: top + height,
        toJSON: () => ({}),
      } as DOMRect;
    });
  });

  /** The pane written chat 641 on its third row. */
  async function writtenLow() {
    const pane = await opened();
    await write(pane.term, "first\r\nsecond\r\n  chat 641 · finished");
    return pane;
  }

  /** The card's anchor, once the card is up. */
  async function cardAnchor() {
    await screen.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    const anchor = document.querySelector<HTMLElement>(".pane-link-anchor");
    if (!anchor) throw new Error("the card has no anchor");
    return anchor.style;
  }

  it("from Links in this pane…", async () => {
    await writtenLow();
    const target = document.querySelector(".xterm-helper-textarea") as HTMLElement;
    target.focus();
    target.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "L",
        code: "KeyL",
        ctrlKey: true,
        shiftKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
    const list = await screen.findByRole("dialog", { name: "Links in this pane" });
    fireEvent.click(await within(list).findByRole("option", { name: "chat 641" }));

    expect(await cardAnchor()).toMatchObject(AT_THE_LINK);
  });

  it("from the terminal menu's Open link", async () => {
    const { drawn } = await writtenLow();
    // The pointer on the screen's box, which starts at its left and top.
    const at = (init: MouseEventInit = {}) => {
      const cell = over(4, 2, init);
      return {
        ...cell,
        clientX: (cell.clientX ?? 0) + SCREEN.left,
        clientY: (cell.clientY ?? 0) + SCREEN.top,
      };
    };
    drawn.dispatchEvent(new MouseEvent("mousemove", at()));
    await waitFor(() => expect(asked("pane_link_resolve")).toHaveLength(1));
    await screen.findByRole("tooltip", {}, { timeout: 2000 });
    for (const type of ["mousedown", "contextmenu", "mouseup"])
      drawn.dispatchEvent(new MouseEvent(type, at({ button: 2 })));
    const menu = await screen.findByRole("menu", { name: "Terminal" });
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Open link" }));

    expect(await cardAnchor()).toMatchObject(AT_THE_LINK);
  });
});
