// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { aTabForEachChat } from "./laidOut.testkit";

/**
 * **A pane moved names what it shows** (#5283), against the whole window: each pane head and each
 * sidebar row says its own chat, whatever a drag did to the tab holding it. jsdom lays nothing
 * out, so every pane and row is given a box here, and the point under the pointer is found from
 * those.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const chat = (session: number, name: string, inFront = false) => ({
  session,
  name,
  cwd: ALPHA,
  harness: "claude",
  in_front: inFront,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: name,
  from: null,
});

const CODEQL = chat(1, "codeql-issues");
const PERFORMANCE = chat(2, "performance");
const COPY = chat(3, "copy-past-crash", true);

/** The core: three chats, of which only the last has a tab. */
function core(stored?: unknown): { asked: string[]; laid: () => unknown } {
  const asked: string[] = [];
  let laid: unknown = stored;
  mockIPC(
    (cmd, args) => {
      asked.push(cmd);
      if (cmd === "window_layout") laid = (args as { layout: unknown }).layout;
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CODEQL, PERFORMANCE, COPY];
      if (cmd === "reopened_layout") return stored ?? aTabForEachChat([COPY], () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: ["steward"],
          persona: "steward",
          unfiled: [],
          workspaces: [
            {
              name: "alpha",
              path: ALPHA,
              vision: "",
              todos: [],
              chats: [CODEQL, PERFORMANCE, COPY],
            },
          ],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked, laid: () => laid };
}

/** Each pane side by side, 800 wide, at the top; each sidebar row its own line far below them. */
function boxOf(el: Element) {
  const pane = el.closest("[data-drop-pane]");
  const row = el.closest("[data-drag-chat]");
  const box =
    pane !== null
      ? {
          left: [...document.querySelectorAll("[data-drop-pane]")].indexOf(pane) * 800,
          top: 40,
          width: 800,
          height: 600,
        }
      : row !== null
        ? {
            left: 0,
            top: 1000 + Number(row.getAttribute("data-drag-chat")) * 30,
            width: 200,
            height: 20,
          }
        : { left: 0, top: 0, width: 0, height: 0 };
  return {
    ...box,
    right: box.left + box.width,
    bottom: box.top + box.height,
    x: box.left,
    y: box.top,
  };
}

beforeEach(() => {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const { left: x, top: y, width, height } = boxOf(this);
    return DOMRect.fromRect({ x, y, width, height });
  });
  document.elementFromPoint = (x: number, y: number) => {
    for (const el of document.querySelectorAll("[data-drop-pane], [data-drag-chat]")) {
      const b = boxOf(el);
      if (x >= b.left && x < b.right && y >= b.top && y < b.bottom) return el;
    }
    return null;
  };
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const at = (x: number, y: number) => ({ clientX: x, clientY: y, button: 0, isPrimary: true });

/** The chat each pane head names, left to right. */
const heads = () =>
  [...document.querySelectorAll("[data-drop-pane] .pane-name")].map((one) => one.textContent);
/** What the sidebar's row of chat `session` says. */
const row = (session: number) =>
  document.querySelector(`[data-drag-chat="${session}"]`)?.textContent ?? "";

/** Drags chat `session`'s sidebar row to the point in the panes. */
function dragRowTo(session: number, x: number, y: number) {
  const from = document.querySelector(`[data-drag-chat="${session}"]`);
  if (from === null) throw new Error(`no row for chat ${session}`);
  fireEvent.pointerDown(from, at(10, 1000 + session * 30 + 5));
  fireEvent.pointerMove(document, at(x, y));
  act(() => {
    fireEvent.pointerUp(document, at(x, y));
  });
}

async function theWindow() {
  const told = core();
  render(<App />);
  await waitFor(() => expect(heads()).toEqual(["copy-past-crash"]));
  await waitFor(() => expect(row(1)).toContain("codeql-issues"));
  return told;
}

/** Waits for the split the drop makes to be drawn, right chats in the right panes. */
async function splitDrawn(sessions: number[]) {
  await waitFor(() =>
    expect(screen.getAllByTestId("pane").map((pane) => pane.textContent)).toEqual(
      sessions.map((session) => `session ${session}`),
    ),
  );
}

describe("a pane moved names what it shows (#5283)", () => {
  it("names each pane and each row its own chat after a sidebar chat splits a pane on its right", async () => {
    await theWindow();
    dragRowTo(1, 790, 340);
    await splitDrawn([3, 1]);
    await waitFor(() => expect(heads()).toEqual(["copy-past-crash", "codeql-issues"]));
    expect(row(1)).toContain("codeql-issues");
    expect(row(1)).not.toContain("copy-past-crash");
    expect(row(3)).toContain("copy-past-crash");
  });

  it("names the tab's new first chat when a sidebar chat splits a pane on its left", async () => {
    await theWindow();
    dragRowTo(1, 10, 340);
    await splitDrawn([1, 3]);
    await waitFor(() => expect(heads()).toEqual(["codeql-issues", "copy-past-crash"]));
    expect(row(1)).toContain("codeql-issues");
    expect(row(3)).toContain("copy-past-crash");
  });

  it("names what a pane shows when a sidebar chat takes its place", async () => {
    await theWindow();
    dragRowTo(1, 400, 340);
    await splitDrawn([1]);
    await waitFor(() => expect(heads()).toEqual(["codeql-issues"]));
    expect(row(3)).toContain("copy-past-crash");
  });

  it("asks the core to hold no name for a drag, and tells it the layout by chat number alone", async () => {
    const told = await theWindow();
    dragRowTo(1, 790, 340);
    await splitDrawn([3, 1]);
    await waitFor(() => expect(JSON.stringify(told.laid())).toContain('"chat":1'));
    expect(told.asked).not.toContain("rename_chat");
    expect(JSON.stringify(told.laid())).not.toMatch(/codeql-issues|copy-past-crash/);
  });

  it("names each pane its own chat when the window comes back on the split", async () => {
    const told = await theWindow();
    dragRowTo(1, 790, 340);
    await splitDrawn([3, 1]);
    await waitFor(() => expect(JSON.stringify(told.laid())).toContain('"chat":1'));
    const stored = told.laid();
    cleanup();
    clearMocks();
    core(stored);
    render(<App />);
    await splitDrawn([3, 1]);
    await waitFor(() => expect(heads()).toEqual(["copy-past-crash", "codeql-issues"]));
  });
});
