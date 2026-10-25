// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { TimelineMoment } from "./bindings";
import { BrowserTimeline } from "./BrowserTimeline";

/**
 * **A chat's browser timeline, as its tab draws it** (#2017): the newest moment first, each
 * screenshot read once and drawn on a canvas, what was let go said in words, where the person
 * or purlis drove said with no screenshot, and read again when the core says it moved. jsdom
 * decodes no image, so what reaches a canvas is read at the one seam that draws.
 */

const painted = vi.hoisted(() => [] as { png: number[]; type?: string }[]);
vi.mock("./browserFrames", async (actual) => ({
  ...(await actual<typeof import("./browserFrames")>()),
  paint: async (_canvas: HTMLCanvasElement, png: Uint8Array, _across?: number, type?: string) => {
    painted.push({ png: Array.from(png), type });
  },
}));

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const PLANE = "/home/dev/plane";
const CHAT = "01JB0000000000000000000001";

function moment(over: Partial<TimelineMoment>): TimelineMoment {
  return {
    at: Date.UTC(2026, 9, 26, 9, 0, 0) / 1000,
    browser: "b1",
    by: "agent",
    did: "Clicked e12",
    page: null,
    shot: null,
    unkept: null,
    ...over,
  };
}

function core(moments: TimelineMoment[]) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let listener: number | undefined;
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plugin:event|listen") {
      listener = (args as { handler: number }).handler;
      return 1;
    }
    if (cmd === "chat_browser_timeline") return moments;
    // "\x89PNG", in base64.
    if (cmd === "chat_browser_timeline_shot") return "iVBORw==";
    return null;
  });
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    moved: (now: TimelineMoment[], chat = CHAT) => {
      moments.splice(0, moments.length, ...now);
      act(() => {
        window.__TAURI_INTERNALS__.runCallback(listener ?? -1, {
          event: "browser-timeline-moved",
          id: 1,
          payload: { plane: PLANE, chat },
        });
      });
    },
  };
}

afterEach(() => {
  cleanup();
  clearMocks();
  painted.length = 0;
});

describe("a chat's browser timeline", () => {
  it("lists the newest moment first, with the screenshot the agent took drawn from its PNG", async () => {
    const host = core([
      moment({ did: "Clicked e12", page: "https://shop.example.test/cart" }),
      moment({ at: Date.UTC(2026, 9, 26, 9, 0, 5) / 1000, did: "Took a screenshot", shot: 3 }),
    ]);

    render(<BrowserTimeline plane={PLANE} chat={CHAT} />);

    const list = await screen.findByRole("list", { name: "Browser timeline" });
    const rows = within(list).getAllByRole("listitem");
    expect(
      rows.map((row) => row.querySelector(".browser-timeline-said span:nth-child(3)")?.textContent),
    ).toEqual(["Took a screenshot", "Clicked e12"]);
    expect(rows[1]).toHaveTextContent("https://shop.example.test/cart");
    expect(within(rows[0]).getByRole("img", { name: /Took a screenshot/ })).toBeInTheDocument();
    await waitFor(() =>
      expect(painted).toEqual([{ png: [0x89, 0x50, 0x4e, 0x47], type: "image/png" }]),
    );
    expect(host.asked("chat_browser_timeline_shot")).toEqual([
      { plane: PLANE, chat: CHAT, shot: 3 },
    ]);
  });

  it("says where the person or purlis drove, and draws no screenshot there", async () => {
    core([
      moment({ by: "person", did: "You took the browser over" }),
      moment({ did: "Took a screenshot", unkept: "driven" }),
      moment({ by: "purlis", did: "purlis signed in with shop" }),
    ]);

    render(<BrowserTimeline plane={PLANE} chat={CHAT} />);

    const list = await screen.findByRole("list", { name: "Browser timeline" });
    expect(list).toHaveTextContent("purlis signed in with shop");
    expect(list).toHaveTextContent("You took the browser over");
    expect(list).toHaveTextContent(
      "No screenshot kept: you or purlis were driving the browser then.",
    );
    expect(within(list).queryByRole("img")).toBeNull();
  });

  it("says a screenshot was let go to stay within its bound", async () => {
    core([moment({ did: "Took a screenshot", unkept: "bound" })]);

    render(<BrowserTimeline plane={PLANE} chat={CHAT} />);

    expect(
      await screen.findByText("Its screenshot was let go to keep the timeline small."),
    ).toBeInTheDocument();
  });

  it("is read again when the core says this chat's timeline moved, and not another's", async () => {
    const host = core([]);
    render(<BrowserTimeline plane={PLANE} chat={CHAT} />);
    await screen.findByTestId("browser-timeline-empty");
    await waitFor(() => expect(host.asked("plugin:event|listen").length).toBe(1));

    host.moved([moment({ did: "Scrolled" })], "01JB0000000000000000000002");
    expect(host.asked("chat_browser_timeline").length).toBe(1);

    host.moved([moment({ did: "Scrolled" })]);
    expect(await screen.findByText("Scrolled")).toBeInTheDocument();
  });
});
