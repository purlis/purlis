// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { LINK_CORE } from "./paneLinkCore";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { listLinksOf } from "./paneLinksOpen";
import { SessionPane } from "./SessionPane";

/**
 * **Links in this pane…** (#6220, spec #6208 D5): a real xterm is written addresses, and the
 * keyboard alone opens the list (⇧⌘L, Ctrl+Shift+L off a Mac), walks it and picks one. What is
 * asserted is what the list says, what a screen reader is handed for each entry, and what the
 * core was asked.
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
let opens: unknown;

const FTP = "purlis opens only http and https addresses, so it will not open this ftp: one";

beforeEach(() => {
  made.length = 0;
  invoked = [];
  opens = { answer: "opened", kind: "url", target: "https://a.example/one" };
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
          ? {
              kind: "url",
              target: one.text,
              shown: one.text,
              does: { does: "opens", into: "browser" },
            }
          : one.text.startsWith("ftp://")
            ? {
                kind: "url",
                target: one.text,
                shown: one.text,
                does: { does: "refused", reason: FTP },
              }
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

const onAMac = () =>
  Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });

const write = (term: Terminal, text: string) => new Promise<void>((done) => term.write(text, done));

async function opened() {
  render(<SessionPane plane="/planes/one" session={1} focused={false} onFocus={() => {}} />);
  const term = made[0];
  await waitFor(() => expect(term.cols).toBe(80));
  return term;
}

/** A key pressed in the terminal, at its textarea, as the engine delivers one. */
function press(init: KeyboardEventInit) {
  const target = document.querySelector(".xterm-helper-textarea");
  if (!target) throw new Error("the pane has no terminal textarea");
  (target as HTMLElement).focus();
  target.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init }));
}

const LIST_KEY_MAC = { key: "L", code: "KeyL", metaKey: true, shiftKey: true };
const LIST_KEY_OTHER = { key: "L", code: "KeyL", ctrlKey: true, shiftKey: true };

const asked = (cmd: string) => invoked.filter((one) => one.cmd === cmd);

/** The pane written three addresses, one refused, and the list opened from the keyboard. */
async function listed() {
  onAMac();
  const term = await opened();
  await write(
    term,
    "see https://a.example/one\r\nthen ftp://b.example/two\r\nand https://c.example/three now",
  );
  press(LIST_KEY_MAC);
  const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
  await waitFor(() => expect(within(dialog).getAllByRole("option")).toHaveLength(3));
  return { term, dialog };
}

describe("Links in this pane…", () => {
  it("opens when the window asks for this pane's, as its palette, ⋯ and head rows do (#6332)", async () => {
    const term = await opened();
    await write(term, "see https://a.example/one");
    expect(listLinksOf("/planes/one", 2)).toBe(false);
    act(() => {
      expect(listLinksOf("/planes/one", 1)).toBe(true);
    });
    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    expect(await within(dialog).findByRole("option")).toHaveAccessibleName("https://a.example/one");
    cleanup();
    expect(listLinksOf("/planes/one", 1)).toBe(false);
  });

  it("opens on ⇧⌘L in the pane and lists the links on screen, newest first", async () => {
    const { dialog } = await listed();
    const rows = within(dialog).getAllByRole("option");
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining("https://c.example/three"),
      expect.stringContaining("ftp://b.example/two"),
      expect.stringContaining("https://a.example/one"),
    ]);
    // One batched question for the screen, not one per link.
    expect(asked("pane_link_resolve")).toHaveLength(1);
  });

  it("opens on Ctrl+Shift+L off a Mac", async () => {
    const term = await opened();
    await write(term, "see https://a.example/one");
    press(LIST_KEY_OTHER);
    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    expect(await within(dialog).findByRole("option")).toHaveAccessibleName("https://a.example/one");
  });

  it("lets a screen reader hear each entry's target and kind", async () => {
    const { dialog } = await listed();
    const [newest, refused] = within(dialog).getAllByRole("option");
    expect(newest).toHaveAccessibleName("https://c.example/three");
    expect(newest).toHaveAccessibleDescription(expect.stringContaining("Web address"));
    expect(refused).toHaveAccessibleName("ftp://b.example/two");
    expect(refused).toHaveAccessibleDescription(expect.stringContaining("Web address"));
  });

  it("lists refused text with its reason, and it cannot be picked", async () => {
    const { dialog } = await listed();
    const refused = within(dialog).getAllByRole("option")[1];
    expect(refused).toHaveAttribute("aria-disabled", "true");
    expect(refused).toHaveAccessibleDescription(expect.stringContaining("only http and https"));
    fireEvent.click(refused);
    fireEvent.keyDown(refused, { key: "Enter" });
    await new Promise((done) => setTimeout(done, 20));
    expect(asked("pane_link_open")).toHaveLength(0);
    expect(screen.getByRole("dialog", { name: "Links in this pane" })).toBeInTheDocument();
  });

  it("opens the entry picked through the open command, and closes", async () => {
    const { dialog } = await listed();
    const newest = within(dialog).getAllByRole("option")[0];
    fireEvent.click(newest);
    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(1));
    expect(asked("pane_link_open")[0].args).toEqual({
      from: { plane: "/planes/one", session: 1 },
      candidate: { kind: "url", text: "https://c.example/three" },
      token: null,
    });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Links in this pane" })).toBeNull(),
    );
  });

  it("asks first where the core asks, and opens on the answer with the ask's token", async () => {
    opens = {
      answer: "ask",
      reason: "it is outside the workspace",
      shown: "x",
      target: "/x",
      token: "a-one-use-token",
      about: { about: "link" },
    };
    const { dialog } = await listed();
    fireEvent.click(within(dialog).getAllByRole("option")[0]);
    const ask = await screen.findByRole("dialog", { name: "Open this link?" });
    opens = { answer: "opened", kind: "url", target: "/x" };
    fireEvent.click(within(ask).getByRole("button", { name: "Open link" }));
    await waitFor(() => expect(asked("pane_link_open")).toHaveLength(2));
    expect(asked("pane_link_open")[1].args.token).toBe("a-one-use-token");
  });

  it("lists a link printed twice once, where it was printed last", async () => {
    onAMac();
    const term = await opened();
    await write(term, "https://a.example/one\r\nhttps://c.example/three\r\nhttps://a.example/one");
    press(LIST_KEY_MAC);
    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    await waitFor(() => expect(within(dialog).getAllByRole("option")).toHaveLength(2));
    expect(within(dialog).getAllByRole("option")[0]).toHaveAccessibleName("https://a.example/one");
  });

  it("lists only what is on screen, not the history scrolled past", async () => {
    onAMac();
    const term = await opened();
    await write(term, "https://gone.example/old\r\n" + "\r\n".repeat(40) + "https://a.example/one");
    press(LIST_KEY_MAC);
    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    await waitFor(() => expect(within(dialog).getAllByRole("option")).toHaveLength(1));
    expect(within(dialog).queryByText(/gone\.example/)).toBeNull();
  });

  it("says so when there are no links on screen", async () => {
    onAMac();
    const term = await opened();
    await write(term, "nothing to see here");
    press(LIST_KEY_MAC);
    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    expect(await within(dialog).findByText("No links on screen.")).toBeVisible();
  });

  it("narrows to what is typed in its filter", async () => {
    const { dialog } = await listed();
    fireEvent.change(within(dialog).getByRole("searchbox", { name: "Filter links" }), {
      target: { value: "c.example" },
    });
    await waitFor(() => expect(within(dialog).getAllByRole("option")).toHaveLength(1));
  });

  it("opens from a saved chat's read-only terminal too, and from none that links nothing", async () => {
    onAMac();
    const { unmount } = render(
      <ReadOnlyTerminal
        screen="see https://a.example/one"
        label="Saved"
        cols={80}
        rows={4}
        fontSize={13}
        links={LINK_CORE}
      />,
    );
    const region = await screen.findByRole("region", { name: "Saved" });
    fireEvent.keyDown(region, LIST_KEY_MAC);
    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    expect(await within(dialog).findByRole("option")).toHaveAccessibleName("https://a.example/one");
    unmount();
    render(
      <ReadOnlyTerminal
        screen="see https://a.example/one"
        label="Plain"
        cols={80}
        rows={4}
        fontSize={13}
      />,
    );
    fireEvent.keyDown(await screen.findByRole("region", { name: "Plain" }), LIST_KEY_MAC);
    await new Promise((done) => setTimeout(done, 20));
    expect(screen.queryByRole("dialog", { name: "Links in this pane" })).toBeNull();
  });

  it("copies the target of the row it is on with ⌘C, and not with ⌘⇧C or ⌥⌘C", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    const { dialog } = await listed();
    const newest = within(dialog).getAllByRole("option")[0];
    fireEvent.keyDown(newest, { key: "c", metaKey: true, shiftKey: true });
    fireEvent.keyDown(newest, { key: "c", metaKey: true, altKey: true });
    expect(writeText).not.toHaveBeenCalled();
    fireEvent.keyDown(newest, { key: "c", metaKey: true });
    expect(writeText).toHaveBeenCalledWith("https://c.example/three");
  });
});
