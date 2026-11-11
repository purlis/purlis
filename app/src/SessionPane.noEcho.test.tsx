// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import { SessionPane } from "./SessionPane";
import { chatShown, keptKey, letGoOfKeptTerminals } from "./keptTerminals";
import { forgetStartsAgain, NO_ECHO_MS } from "./noEcho";

/**
 * **A pane whose view stopped sending is started again** (#5478): typed into, a chat's program
 * always writes something back, so a pane that types and is sent nothing for {@link NO_ECHO_MS}
 * has a view that is stuck. The pane lets go of its terminal and view and starts both again,
 * as falling out of the kept chats does (`keptTerminals.ts`), which is what cured the freeze
 * every time it was seen.
 *
 * The real SessionPane and the real xterm, as in `SessionPane.stuckWrites.test.tsx`; the
 * clock is the test's (`setInterval` and `Date` faked, so xterm's own timers still run).
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

let outputs: Channel<ArrayBuffer>[] = [];
let calls: { cmd: string; args: Record<string, unknown> }[] = [];
let views = 0;
let index = 0;

const send = (chunk: string) =>
  act(() => {
    const output = outputs[outputs.length - 1];
    if (!output) throw new Error("the pane has opened no view");
    const bytes = new TextEncoder().encode(chunk);
    window.__TAURI_INTERNALS__.runCallback(output.id, { index: index++, message: bytes.buffer });
  });
const later = (ms: number) =>
  act(async () => {
    vi.advanceTimersByTime(ms);
    await Promise.resolve();
  });
/** Typed into the terminal, and the core's answer to it taken. */
const typeInto = (pane: Terminal, text: string) =>
  act(async () => {
    pane.input(text);
  });
const named = (cmd: string) => calls.filter((call) => call.cmd === cmd);

beforeEach(() => {
  made.length = 0;
  outputs = [];
  calls = [];
  views = 0;
  index = 0;
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"] });
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "watch_session") {
      outputs.push((args as { output: Channel<ArrayBuffer> }).output);
      views += 1;
      return { view: views, columns: 80, rows: 24, scrollback: 5000, newline: null };
    }
    return null;
  });
});
afterEach(() => {
  cleanup();
  letGoOfKeptTerminals();
  forgetStartsAgain();
  clearMocks();
  vi.useRealTimers();
});

async function openPane() {
  // One of the last chats shown, so its terminal would be kept: the case the freeze was seen in.
  chatShown(keptKey("/planes/one", 7));
  render(<SessionPane plane="/planes/one" session={7} focused={false} onFocus={() => {}} />);
  await waitFor(() => expect(made[0]?.options.scrollback).toBe(5000));
  return made[0];
}

describe("a pane typed into and sent nothing back", () => {
  it("starts its terminal and view again, as falling out of the kept chats does", async () => {
    const pane = await openPane();
    act(() => pane.focus());

    await typeInto(pane, "/diag");
    await later(NO_ECHO_MS / 2);
    await typeInto(pane, "nose");
    await later(NO_ECHO_MS / 2 + 1_000);

    await waitFor(() => expect(named("watch_session")).toHaveLength(2));
    expect(named("unwatch_session")).toEqual([
      { cmd: "unwatch_session", args: expect.objectContaining({ session: 7, view: 1 }) },
    ]);
    await waitFor(() => expect(made).toHaveLength(2));
    expect(made[1]).not.toBe(pane);
    // Started while its person types, the new terminal takes their next key.
    expect(made[1].element?.contains(document.activeElement)).toBe(true);
  });

  it("starts an ended chat's terminal again as taking no input", async () => {
    const view = render(
      <SessionPane plane="/planes/one" session={7} focused={false} onFocus={() => {}} />,
    );
    await waitFor(() => expect(made[0]?.options.scrollback).toBe(5000));
    await typeInto(made[0], "ab");
    await typeInto(made[0], "cd");
    view.rerender(
      <SessionPane plane="/planes/one" session={7} focused={false} onFocus={() => {}} readOnly />,
    );
    await later(NO_ECHO_MS + 1_000);

    await waitFor(() => expect(made).toHaveLength(2));
    expect(made[1].options.disableStdin).toBe(true);
  });

  it("keeps its terminal while what is typed is written back", async () => {
    const pane = await openPane();

    for (let key = 0; key < 10; key++) {
      await typeInto(pane, "a");
      await send("a");
      await later(3_000);
    }
    await later(NO_ECHO_MS * 2);

    expect(named("watch_session")).toHaveLength(1);
    expect(named("unwatch_session")).toEqual([]);
  });

  it("keeps its terminal for what a program may answer with nothing: the mouse, a lone key", async () => {
    const pane = await openPane();

    // A mouse report, and a focus report, in the encodings a program asks for.
    await typeInto(pane, "\x1b[<0;10;5M");
    await typeInto(pane, "\x1b[I");
    await later(NO_ECHO_MS / 2);
    await typeInto(pane, "\x1b");
    await later(NO_ECHO_MS * 2);
    // One key alone is not enough to say the view is stuck.
    await typeInto(pane, "x");
    await later(NO_ECHO_MS * 2);

    expect(named("watch_session")).toHaveLength(1);
  });

  it("starts again at most once in a while, should the program truly say nothing", async () => {
    await openPane();

    for (let round = 0; round < 3; round++) {
      const pane = made[made.length - 1];
      await typeInto(pane, "ab");
      await typeInto(pane, "cd");
      await later(NO_ECHO_MS + 1_000);
    }

    await waitFor(() => expect(named("watch_session")).toHaveLength(2));
    expect(made).toHaveLength(2);
  });
});
