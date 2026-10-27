// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { BrowserFrame } from "./bindings";
import { pickIn, selectNamed } from "./ui/select/select.testkit";
import { onAMac } from "./tabKeys";
import { stripNamed } from "./test-strips";

/**
 * **A browser, in a view tab of the window** (#2012): the palette's New browser starts one in
 * the session host and opens its tab, whose view draws each frame the host sends on a canvas;
 * a refusal (Stop all) is said and opens nothing; and when the last tab showing a browser
 * closes, the browser ends. jsdom decodes no image, so what reaches the canvas is read at the
 * one seam that draws (`browserFrames.paint`).
 */

const painted: number[][] = [];
vi.mock("./browserFrames", async (actual) => ({
  ...(await actual<typeof import("./browserFrames")>()),
  paint: async (_canvas: HTMLCanvasElement, jpeg: Uint8Array) => {
    painted.push(Array.from(jpeg));
  },
}));

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

/** What the core answers besides, per test (#2176): a command's name, and its answer. */
let besides: Record<string, (args: Record<string, unknown>) => unknown> = {};

/** The pool whose runner the watched browser runs on, as `watch_browser` answers (#2050). */
let onPool: string | null = null;
/** Whether `watch_browser` answers at all: a view not attached yet says nowhere it runs. */
let watchAnswers = true;

beforeEach(() => {
  painted.length = 0;
  besides = {};
  onPool = null;
  watchAnswers = true;
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

function core(
  open: () => unknown = () => "B1",
  personDrives = false,
  input: (args: Record<string, unknown>) => unknown = () => ({ outcome: "taken" }),
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let frames: Channel<BrowserFrame> | undefined;
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "chat_states") return [];
      if (cmd === "plane_sidebar")
        return { root: PLANE, personas: [], persona: null, unfiled: [], workspaces: [] };
      if (cmd === "open_browser") return open();
      if (cmd === "watch_browser") {
        frames = (args as { frames: Channel<BrowserFrame> }).frames;
        if (!watchAnswers) return new Promise(() => {});
        return { view: 4, width: 1280, height: 800, person_drives: personDrives, pool: onPool };
      }
      if (cmd === "browser_input") return input((args ?? {}) as Record<string, unknown>);
      const answer = besides[cmd];
      if (answer !== undefined) return answer((args ?? {}) as Record<string, unknown>);
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
    send: (jpeg: string, aim = "p1:1") =>
      frames?.onmessage({ aim, jpeg, reconnection: null, page: null }),
    reconnection: (reconnection: "reconnecting" | "reconnected") =>
      frames?.onmessage({ aim: "", jpeg: "", reconnection, page: null }),
    page: (origin: string | null) =>
      frames?.onmessage({ aim: "", jpeg: "", reconnection: null, page: { origin } }),
    order: () => asked.map(({ cmd }) => cmd),
  };
}

async function fromThePalette(row: string) {
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard(row);
  await userEvent.keyboard("{Enter}");
}

/** What the bar above the page says: who drives it. */
const barSaid = () => screen.getByRole("status");

const tabNames = () =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

describe("a browser's view tab", () => {
  it("opens from the palette and draws each frame the host sends, as the focused view", async () => {
    const host = core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");

    await fromThePalette("New browser");

    await waitFor(() => expect(tabNames()).toEqual(["Browser"]));
    expect(host.asked("open_browser")).toEqual([{ plane: PLANE }]);
    await screen.findByLabelText("The browser's page");
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B1", role: "focused" }),
      ),
    );

    host.send("/9j/AA==");
    await waitFor(() => expect(painted).toEqual([[0xff, 0xd8, 0xff, 0x00]]));

    host.send("");
    expect(await screen.findByText("This browser has ended")).toBeInTheDocument();
  });

  it("is refused in words while Stop all holds, and opens nothing", async () => {
    core(() => Promise.reject("Every chat purlis started is stopped."));
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");

    await fromThePalette("New browser");

    expect(await screen.findByText("Every chat purlis started is stopped.")).toBeInTheDocument();
    expect(tabNames()).toEqual([]);
  });

  it("ends its browser when its tab closes, after closing its view", async () => {
    const host = core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");
    await fromThePalette("New browser");
    await waitFor(() => expect(tabNames()).toEqual(["Browser"]));
    await waitFor(() => expect(host.asked("watch_browser").length).toBeGreaterThan(0));

    await fromThePalette("Close this view");

    await waitFor(() => expect(host.asked("close_browser")).toEqual([{ program: "B1" }]));
    expect(host.asked("unwatch_browser")).toContainEqual({ program: "B1", view: 4 });
  });
});

describe("a browser's view on a runner whose link is down (#2542)", () => {
  it("says Reconnecting over its last frame until it is reconnected, and has not ended", async () => {
    const { BrowserView } = await import("./BrowserView");
    const host = core();
    render(<BrowserView plane={PLANE} program="B1" />);
    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(1));
    host.send("/9j/AA==");
    await waitFor(() => expect(painted).toEqual([[0xff, 0xd8, 0xff, 0x00]]));
    const surface = screen.getByTestId("browser-surface");

    host.reconnection("reconnecting");

    await waitFor(() => expect(barSaid()).toHaveTextContent(/^Reconnecting…$/));
    expect(within(surface).getByText("Reconnecting…")).toBeInTheDocument();
    expect(surface).toHaveClass("reconnecting");
    expect(screen.queryByText("This browser has ended")).toBeNull();
    expect(painted).toHaveLength(1);

    host.reconnection("reconnected");
    host.send("/9j/AQ==");

    await waitFor(() =>
      expect(barSaid()).toHaveTextContent("Click or type in the page to take it over."),
    );
    expect(barSaid()).not.toHaveTextContent("Reconnecting…");
    expect(within(surface).queryByText("Reconnecting…")).toBeNull();
    expect(surface).not.toHaveClass("reconnecting");
    await waitFor(() => expect(painted).toHaveLength(2));
  });
});

describe("a browser's view nobody can see", () => {
  it("is closed while its project is not in front or the window is hidden, and opened again when seen", async () => {
    const { BrowserView } = await import("./BrowserView");
    const { ProjectShownProvider } = await import("./projectShown");
    const host = core();
    const drawn = (inFront: boolean) => (
      <ProjectShownProvider value={inFront}>
        <BrowserView plane={PLANE} program="B1" />
      </ProjectShownProvider>
    );
    const { rerender } = render(drawn(false));
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(host.asked("watch_browser")).toEqual([]);

    rerender(drawn(true));
    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(1));

    let hidden = true;
    Object.defineProperty(document, "hidden", { configurable: true, get: () => hidden });
    document.dispatchEvent(new Event("visibilitychange"));
    await waitFor(() =>
      expect(host.asked("unwatch_browser")).toEqual([{ program: "B1", view: 4 }]),
    );

    hidden = false;
    document.dispatchEvent(new Event("visibilitychange"));
    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(2));

    rerender(drawn(false));
    await waitFor(() => expect(host.asked("unwatch_browser")).toHaveLength(2));
    delete (document as unknown as { hidden?: boolean }).hidden;
  });
});

describe("taking a browser over from its view (#2014)", () => {
  // jsdom lays nothing out: the page is drawn over 1280 by 800 pixels of the view, at its size.
  const box = { left: 0, top: 0, width: 1280, height: 800, right: 1280, bottom: 800, x: 0, y: 0 };
  beforeEach(() => {
    vi.spyOn(HTMLCanvasElement.prototype, "getBoundingClientRect").mockReturnValue({
      ...box,
      toJSON: () => box,
    } as DOMRect);
  });
  afterEach(() => vi.restoreAllMocks());

  async function drawn(personDrives = false, input?: (args: Record<string, unknown>) => unknown) {
    const { BrowserView } = await import("./BrowserView");
    const { ProjectShownProvider } = await import("./projectShown");
    const host = core(undefined, personDrives, input);
    const view = (seen: boolean) => (
      <ProjectShownProvider value={seen}>
        <BrowserView plane={PLANE} program="B1" />
      </ProjectShownProvider>
    );
    const { rerender } = render(view(true));
    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(1));
    host.send("/9j/AA==", "p1:7");
    return { ...host, seen: (seen: boolean) => rerender(view(seen)) };
  }

  /** An answer to the person's input that waits until `let` is called. */
  function held() {
    const lets: (() => void)[] = [];
    return {
      input: () => new Promise((answer) => lets.push(() => answer({ outcome: "taken" }))),
      let: () => lets.splice(0).forEach((go) => go()),
    };
  }

  it("takes the browser over with a click, which then reaches the page where it was aimed", async () => {
    const host = await drawn();
    expect(barSaid()).toHaveTextContent("Click or type in the page to take it over.");

    fireEvent.pointerDown(screen.getByTestId("browser-surface"), {
      clientX: 640,
      clientY: 400,
      button: 0,
      detail: 1,
      pointerId: 1,
    });

    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));
    const order = host.order();
    expect(order.indexOf("take_over_browser")).toBeLessThan(order.indexOf("browser_input"));
    expect(host.asked("browser_input")[0]).toEqual({
      program: "B1",
      input: expect.objectContaining({
        kind: "mouse",
        aim: "p1:7",
        action: "pressed",
        x: 640,
        y: 400,
        button: "left",
      }),
    });
    expect(barSaid()).toHaveTextContent(
      "You are driving this browser. The agent is paused until you press Done.",
    );
  });

  it("lets go of a click that took the browser over, even when let go before the take-over answered", async () => {
    const host = await drawn();
    const surface = screen.getByTestId("browser-surface");
    const at = { clientX: 640, clientY: 400, button: 0, detail: 1, pointerId: 1 };

    fireEvent.pointerDown(surface, at);
    fireEvent.pointerUp(surface, at);

    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(2));
    expect(
      host.asked("browser_input").map(({ input }) => (input as { action: string }).action),
    ).toEqual(["pressed", "released"]);
  });

  it("takes the browser over with a key, and Done hands it back", async () => {
    const host = await drawn();

    fireEvent.keyDown(screen.getByTestId("browser-surface"), {
      key: "x",
      code: "KeyX",
      keyCode: 88,
    });
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));
    expect(host.asked("take_over_browser")).toEqual([{ program: "B1" }]);
    expect(host.asked("browser_input")[0].input).toMatchObject({
      kind: "key",
      key: "x",
      action: "down",
    });

    await userEvent.click(screen.getByRole("button", { name: "Done" }));

    await waitFor(() => expect(host.asked("hand_back_browser")).toEqual([{ program: "B1" }]));
    await waitFor(() =>
      expect(barSaid()).toHaveTextContent("Click or type in the page to take it over."),
    );
    expect(screen.queryByRole("button", { name: "Done" })).toBeNull();
  });

  it("hands the browser back on the one chord, which never reaches the page", async () => {
    const host = await drawn(true);
    await screen.findByRole("button", { name: "Done" });
    const mac = onAMac();

    fireEvent.keyDown(screen.getByTestId("browser-surface"), {
      key: "Enter",
      shiftKey: true,
      metaKey: mac,
      ctrlKey: !mac,
    });

    await waitFor(() => expect(host.asked("hand_back_browser")).toEqual([{ program: "B1" }]));
    expect(host.asked("browser_input")).toEqual([]);
  });

  it("says the person still drives a browser whose view opens again after a crash of the window", async () => {
    const host = await drawn(true);

    expect(await screen.findByRole("button", { name: "Done" })).toBeInTheDocument();
    expect(barSaid()).toHaveTextContent("The agent is paused until you press Done.");
    expect(host.asked("take_over_browser")).toEqual([]);
  });

  it("takes the browser over to set a size preset, and sets it", async () => {
    const host = await drawn();

    await pickIn(selectNamed("Page size"), "phone");

    await waitFor(() =>
      expect(host.asked("size_browser")).toEqual([{ program: "B1", width: 390, height: 844 }]),
    );
    const order = host.order();
    expect(order.indexOf("take_over_browser")).toBeLessThan(order.indexOf("size_browser"));
  });

  it("moves of a mouse the person does not drive with reach nothing", async () => {
    const host = await drawn();

    const surface = screen.getByTestId("browser-surface");

    fireEvent.pointerMove(surface, { clientX: 10, clientY: 10 });
    expect(host.asked("take_over_browser")).toEqual([]);
    // A key after it takes the browser over, and is the only input sent: the move went nowhere.
    fireEvent.keyDown(surface, { key: "x", code: "KeyX", keyCode: 88 });
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));

    expect(host.asked("browser_input")[0].input).toMatchObject({ kind: "key", key: "x" });
  });

  it("hands back only after the keys already typed have reached the page", async () => {
    const answers = held();
    const host = await drawn(true, answers.input);
    const surface = screen.getByTestId("browser-surface");
    await screen.findByRole("button", { name: "Done" });

    fireEvent.keyDown(surface, { key: "a", code: "KeyA", keyCode: 65 });
    fireEvent.keyDown(surface, { key: "b", code: "KeyB", keyCode: 66 });
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));
    expect(host.asked("hand_back_browser")).toEqual([]);
    answers.let();
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(2));
    answers.let();

    await waitFor(() => expect(host.asked("hand_back_browser")).toEqual([{ program: "B1" }]));
    const order = host.order();
    expect(order.lastIndexOf("browser_input")).toBeLessThan(order.indexOf("hand_back_browser"));
  });

  it("sends nothing queued before its view was watched again", async () => {
    const answers = held();
    const host = await drawn(true, answers.input);
    const surface = screen.getByTestId("browser-surface");
    await screen.findByRole("button", { name: "Done" });
    fireEvent.keyDown(surface, { key: "a", code: "KeyA", keyCode: 65 });
    fireEvent.keyDown(surface, { key: "b", code: "KeyB", keyCode: 66 });
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));

    host.seen(false);
    host.seen(true);
    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(2));
    answers.let();
    await waitFor(() => expect(host.asked("unwatch_browser").length).toBeGreaterThan(0));

    fireEvent.keyDown(screen.getByTestId("browser-surface"), {
      key: "c",
      code: "KeyC",
      keyCode: 67,
    });
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(2));
    answers.let();
    expect(host.asked("browser_input").map(({ input }) => (input as { key: string }).key)).toEqual([
      "a",
      "c",
    ]);
  });

  /** The bar's words for input the page moved on from before it arrived. */
  const DROPPED = "The page changed before your key or click reached it, so purlis dropped it.";

  /** A host that takes input aimed at what `showing()` names, and drops the rest as missed. */
  const onlyAt =
    (showing: () => string) =>
    ({ input }: Record<string, unknown>) =>
      (input as { aim: string }).aim === showing() ? { outcome: "taken" } : { outcome: "missed" };

  const typed = (surface: HTMLElement, key: string) =>
    fireEvent.keyDown(surface, {
      key,
      code: `Key${key.toUpperCase()}`,
      keyCode: key.toUpperCase().charCodeAt(0),
    });

  it("says once in the bar that keys typed while a frame inside the page reloaded were dropped, and to type them again", async () => {
    let showing = "p1:7";
    const host = await drawn(
      true,
      onlyAt(() => showing),
    );
    const surface = screen.getByTestId("browser-surface");
    await screen.findByRole("button", { name: "Done" });
    typed(surface, "a");
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));
    expect(screen.getByRole("status")).not.toHaveTextContent(DROPPED);

    // A frame inside the page loads another document before the window draws a frame of it:
    // the next keys are aimed at what the page no longer shows.
    showing = "p1:8";
    typed(surface, "b");
    typed(surface, "c");
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(3));

    const status = screen.getByRole("status");
    await waitFor(() => expect(status).toHaveTextContent(`${DROPPED} Type or click again.`));
    expect(status.textContent?.split(DROPPED)).toHaveLength(2);

    // The window draws the new document, and the person types again: it reaches the page, and
    // the bar no longer says anything was dropped.
    host.send("/9j/AA==", "p1:8");
    typed(surface, "b");
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(4));
    expect(host.asked("browser_input")[3].input).toMatchObject({ key: "b", aim: "p1:8" });
    await waitFor(() => expect(screen.getByRole("status")).not.toHaveTextContent(DROPPED));
  });

  it("says nothing of a mouse move the page moved on from", async () => {
    const host = await drawn(true, () => ({ outcome: "missed" }));
    const surface = screen.getByTestId("browser-surface");
    await screen.findByRole("button", { name: "Done" });

    fireEvent.pointerMove(surface, { clientX: 10, clientY: 10 });
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));

    expect(screen.getByRole("status")).not.toHaveTextContent(DROPPED);
  });

  it("stops saying input was dropped once the person hands the browser back", async () => {
    const host = await drawn(true, () => ({ outcome: "missed" }));
    const surface = screen.getByTestId("browser-surface");
    await screen.findByRole("button", { name: "Done" });
    typed(surface, "a");
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(DROPPED));

    await userEvent.click(screen.getByRole("button", { name: "Done" }));

    await waitFor(() => expect(host.asked("hand_back_browser")).toEqual([{ program: "B1" }]));
    await waitFor(() => expect(screen.getByRole("status")).not.toHaveTextContent(DROPPED));
  });

  it("stops saying input was dropped once its view is watched again", async () => {
    const host = await drawn(true, () => ({ outcome: "missed" }));
    const surface = screen.getByTestId("browser-surface");
    await screen.findByRole("button", { name: "Done" });
    typed(surface, "a");
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(DROPPED));

    host.seen(false);
    host.seen(true);

    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(2));
    await waitFor(() => expect(screen.getByRole("status")).not.toHaveTextContent(DROPPED));
  });

  it("says nothing of a key dropped from the watch before, whose answer comes after the view is watched again", async () => {
    const lets: (() => void)[] = [];
    const host = await drawn(
      true,
      () => new Promise((answer) => lets.push(() => answer({ outcome: "missed" }))),
    );
    const surface = screen.getByTestId("browser-surface");
    await screen.findByRole("button", { name: "Done" });
    typed(surface, "a");
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));

    host.seen(false);
    host.seen(true);
    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(2));
    lets.splice(0).forEach((go) => go());
    await waitFor(() => expect(host.asked("unwatch_browser").length).toBeGreaterThan(0));

    // The next key, of the new watch, is sent only once the old answer has been heard, and
    // that answer said nothing.
    typed(screen.getByTestId("browser-surface"), "b");
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(2));
    expect(screen.getByRole("status")).not.toHaveTextContent(DROPPED);
    lets.splice(0).forEach((go) => go());
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(DROPPED));
  });
});

describe("a browser the person opened goes where they send it (#2176)", () => {
  async function opened() {
    const host = core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");
    await fromThePalette("New browser");
    await screen.findByLabelText("The browser's page");
    return host;
  }

  it("goes to the address typed in its view tab, and its chip names where it went", async () => {
    besides.go_browser = () => ({ outcome: "went", address: "https://example.com/docs" });
    const host = await opened();

    fireEvent.change(await screen.findByRole("textbox", { name: "Address" }), {
      target: { value: "example.com/docs" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Go" }));

    await waitFor(() =>
      expect(host.asked("go_browser")).toEqual([
        { plane: PLANE, program: "B1", address: "example.com/docs" },
      ]),
    );
    expect(await screen.findByText(/^example\.com on this/)).toBeInTheDocument();
  });

  it("names a runner's pool as the device its localhost is on (#2050)", async () => {
    onPool = "builds";
    const host = await opened();

    host.page("http://localhost:5173");

    expect(await screen.findByText("localhost:5173 on pool builds")).toBeInTheDocument();
    expect(screen.queryByText(/on this (Mac|computer)/)).toBeNull();
  });

  it("names no device while neither its view nor the chats' browsers say where it runs (#2050)", async () => {
    watchAnswers = false;
    besides.chat_browsers = () => [];
    const host = await opened();
    await waitFor(() => expect(host.asked("chat_browsers").length).toBeGreaterThan(0));

    host.page("http://localhost:5173");

    expect(await screen.findByText("localhost:5173")).toBeInTheDocument();
    expect(screen.queryByText(/on this (Mac|computer)/)).toBeNull();
  });

  it("asks once about a site new to the project, and an Allow on this machine lets it go there", async () => {
    let goes = 0;
    besides.go_browser = () => {
      goes += 1;
      return goes === 1
        ? {
            outcome: "asks",
            sites: ["example.com:443"],
            answers: [
              { id: "you", label: "Always allow in this project, on this machine", allows: true },
              { id: "keep", label: "Keep blocked", allows: false },
            ],
          }
        : { outcome: "went", address: "https://example.com/" };
    };
    besides.allow_person_browser_sites = () => null;
    const host = await opened();

    fireEvent.change(await screen.findByRole("textbox", { name: "Address" }), {
      target: { value: "example.com" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Go" }));

    expect(
      await screen.findByText(/example\.com:443 is a site new to this project/),
    ).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Always allow in this project, on this machine" }),
    );

    await waitFor(() =>
      expect(host.asked("allow_person_browser_sites")).toEqual([
        { plane: PLANE, program: "B1", sites: ["example.com:443"], level: "you" },
      ]),
    );
    await waitFor(() => expect(host.asked("go_browser")).toHaveLength(2));
    await waitFor(() =>
      expect(screen.queryByText(/is a site new to this project/)).not.toBeInTheDocument(),
    );
  });

  it("keeps the site blocked on Keep blocked, and goes nowhere", async () => {
    besides.go_browser = () => ({
      outcome: "asks",
      sites: ["example.com:443"],
      answers: [
        { id: "you", label: "Always allow in this project, on this machine", allows: true },
        { id: "keep", label: "Keep blocked", allows: false },
      ],
    });
    const host = await opened();
    fireEvent.change(await screen.findByRole("textbox", { name: "Address" }), {
      target: { value: "example.com" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Go" }));
    await screen.findByText(/is a site new to this project/);

    await userEvent.click(screen.getByRole("button", { name: "Keep blocked" }));

    await waitFor(() =>
      expect(screen.queryByText(/is a site new to this project/)).not.toBeInTheDocument(),
    );
    expect(host.asked("allow_person_browser_sites")).toEqual([]);
    expect(host.asked("go_browser")).toHaveLength(1);
  });

  it("says in words why an address of another scheme was refused", async () => {
    const refused =
      "A browser goes only to http and https addresses, and this one is javascript: type one " +
      "that starts with https:// or http://.";
    besides.go_browser = () => Promise.reject(refused);
    await opened();

    fireEvent.change(await screen.findByRole("textbox", { name: "Address" }), {
      target: { value: "javascript:alert(1)" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Go" }));

    expect(await screen.findByText(refused)).toBeInTheDocument();
  });

  it("hands a browser the person drives back before it goes, after what they typed", async () => {
    besides.go_browser = () => ({ outcome: "went", address: "https://example.com/" });
    const host = core(undefined, true);
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");
    await fromThePalette("New browser");
    await screen.findByRole("button", { name: "Done" });

    fireEvent.change(await screen.findByRole("textbox", { name: "Address" }), {
      target: { value: "example.com" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Go" }));

    await waitFor(() => expect(host.asked("go_browser")).toHaveLength(1));
    const order = host.order().filter((cmd) => cmd === "hand_back_browser" || cmd === "go_browser");
    expect(order).toEqual(["hand_back_browser", "go_browser"]);
    await waitFor(() => expect(screen.queryByRole("button", { name: "Done" })).toBeNull());
  });

  it("has no address field on a chat's browser, which its chat drives", async () => {
    let listed = false;
    besides.chat_browsers = () => {
      listed = true;
      return [
        {
          session: 3,
          name: "b1",
          program: "B1",
          unended: false,
          pinned: false,
          was: null,
          login: null,
          driver: null,
          pool: null,
        },
      ];
    };
    await opened();
    await waitFor(() => expect(listed).toBe(true));
    // Long enough for the listing to have been drawn, as the other tests' field is.
    await new Promise((settled) => setTimeout(settled, 50));
    expect(screen.queryByRole("textbox", { name: "Address" })).not.toBeInTheDocument();
  });
});
