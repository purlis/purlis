// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $ } from "@wdio/globals";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";
import { READY } from "../harness.js";
import { pressAndStart } from "../opening.js";
import { roomForTwoPanes } from "../paneRoom.js";
import { openedSince } from "../reach.js";

/**
 * **A pane dragged in the real window** (#5291): the panes make room, a terminal keeps its grid
 * until the drop, changes it once then, and a cancel leaves nothing behind. jsdom lays
 * nothing out, so where a frame lands, and what its terminal takes are
 * asked here, of real panes and a real xterm.
 *
 * **The pointer's events are dispatched in the page.** Everything from there is the window's own
 * (its listeners, its layout, its observers); how a real pointer feels is a check on hardware
 * (`needs-hardware`), which no driver stands in for.
 */

type Box = { left: number; top: number; width: number; height: number };

type Pane = { frame: Box; grid: Grid };

/** A terminal's grid as it draws it. `xterm` tells its program a size as its grid changes, and only
 *  then (`SessionPane`'s `onResize`), and the IPC is not a thing a spec can listen on. */
type Grid = { width: number; columns: number; rows: number };

declare global {
  interface Window {
    /** Each change of a terminal's grid since {@link listenForGrids}, by its chat's number. */
    __gridChanges?: { session: number; width: number; height: number }[];
  }
}

/** Every pane on screen, by its chat's number. */
async function panesNow(): Promise<Record<number, Pane>> {
  return browser.execute(() => {
    const out: Record<number, Pane> = {};
    for (const pane of document.querySelectorAll('[data-testid="pane"]')) {
      const frame = pane.closest(".pane-frame")?.getBoundingClientRect();
      if (frame === undefined) continue;
      const screen = pane.querySelector(".xterm-screen")?.getBoundingClientRect();
      const measure = pane.querySelector(".xterm-char-measure-element");
      const cell =
        (measure?.getBoundingClientRect().width ?? 0) / (measure?.textContent?.length || 1);
      out[Number(pane.getAttribute("data-session"))] = {
        frame: { left: frame.left, top: frame.top, width: frame.width, height: frame.height },
        grid: {
          width: screen?.width ?? 0,
          columns: cell > 0 && screen !== undefined ? Math.round(screen.width / cell) : 0,
          rows: pane.querySelector(".xterm-rows")?.children.length ?? 0,
        },
      };
    }
    return out;
  });
}

/** From here on, every change of a terminal's grid is written down, the frame it is drawn in. */
async function listenForGrids(): Promise<void> {
  await browser.execute(() => {
    window.__gridChanges = [];
    for (const pane of document.querySelectorAll('[data-testid="pane"]')) {
      const screen = pane.querySelector(".xterm-screen");
      if (screen === null) continue;
      const session = Number(pane.getAttribute("data-session"));
      const box = screen.getBoundingClientRect();
      let was = `${box.width}×${box.height}`;
      new ResizeObserver(() => {
        const { width, height } = screen.getBoundingClientRect();
        const now = `${width}×${height}`;
        if (now === was) return;
        was = now;
        window.__gridChanges?.push({ session, width, height });
      }).observe(screen);
    }
  });
}

const gridChanges = () =>
  browser.execute(() => window.__gridChanges ?? []) as Promise<
    { session: number; width: number; height: number }[]
  >;

/** A press on chat `session`'s head, and a move far enough to pick it up. */
async function pickUp(session: number): Promise<void> {
  await browser.execute((session: number) => {
    const frame = document
      .querySelector(`[data-testid="pane"][data-session="${session}"]`)
      ?.closest(".pane-frame");
    const grip = frame?.querySelector("[data-pane-grip]");
    if (!grip) throw new Error(`chat ${session}'s pane has no head to pick up`);
    const box = grip.getBoundingClientRect();
    // The head's blank middle: not its grip's control, nor a button at its end.
    const at = {
      clientX: box.left + box.width / 2,
      clientY: box.top + box.height / 2,
      button: 0,
      bubbles: true,
      isPrimary: true,
    };
    grip.dispatchEvent(new PointerEvent("pointerdown", at));
    window.dispatchEvent(
      new PointerEvent("pointermove", {
        ...at,
        clientX: at.clientX + 10,
        clientY: at.clientY + 10,
      }),
    );
  }, session);
}

/**
 * Waits until the panes hold still: no finite animation runs on a pane or a preview, and two
 * looks a beat apart read the same frames. A fixed pause read them mid-motion on a loaded
 * runner, so a drop seemed to land off its preview (#6229).
 */
async function panesStill(): Promise<void> {
  let last = "";
  await browser.waitUntil(
    async () => {
      const moving = await browser.execute(() =>
        document.getAnimations().some((one) => {
          if (one.playState !== "running") return false;
          const effect = one.effect as KeyframeEffect | null;
          if (effect === null || effect.getTiming().iterations === Infinity) return false;
          const target = effect.target;
          return (
            target instanceof Element &&
            target.closest('.pane-frame, [data-scope="pane-preview"]') !== null
          );
        }),
      );
      const now = JSON.stringify(await panesNow());
      const still = !moving && now === last;
      last = now;
      return still;
    },
    { timeout: 10_000, interval: 100, timeoutMsg: "the panes never held still" },
  );
}

/** Moves the pointer, and waits for the target's wait and the panes' motion to be over. */
async function moveTo(x: number, y: number): Promise<void> {
  await browser.execute(
    (x: number, y: number) => {
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: x, clientY: y }));
    },
    x,
    y,
  );
  await browser.pause(400);
  await panesStill();
}

/** The sizes a preview says it will take, as drawn on the panes. */
const labels = (): Promise<string[]> =>
  browser.execute(() =>
    [...document.querySelectorAll('[data-scope="pane-preview"][data-part="size"]')].map(
      (one) => one.textContent ?? "",
    ),
  );

/** The tabs on the strip, as the other specs read them. */
const tabNames = (): Promise<string[]> =>
  browser.execute(() =>
    [
      ...(document.querySelector('[data-strip="Tabs"]')?.querySelectorAll('[role="tab"]') ?? []),
    ].map((tab) => tab.querySelector(".tab-name")?.textContent ?? ""),
  );

describe("a pane dragged in the window", () => {
  let chatsBefore: Running = {};
  let tabsBefore: string[] = [];
  /** The two chats: the first left, the second right. */
  let left = 0;
  let right = 0;
  let natural: Record<number, Pane> = {};

  before(async () => {
    tabsBefore = await tabNames();
    chatsBefore = await chatsRunning();
    await pressAndStart("New tab");
    await $(`//*[contains(@class,"xterm-rows")][contains(.,"${READY}")]`).waitForExist({
      timeout: 30_000,
    });
    await roomForTwoPanes();
    await pressAndStart("Split right");
    await browser.waitUntil(
      async () => {
        const shown = Object.entries(await panesNow()).filter(([, pane]) => pane.grid.rows > 0);
        return shown.length >= 2;
      },
      { timeout: 30_000, timeoutMsg: "the split never drew two terminals" },
    );
    // The two the tab in front shows, left to right, once their fits have landed.
    await browser.pause(1_500);
    natural = await panesNow();
    const byPlace = Object.entries(natural).sort(([, a], [, b]) => a.frame.left - b.frame.left);
    left = Number(byPlace[0][0]);
    right = Number(byPlace[1][0]);
    await listenForGrids();
  });

  after(async () => {
    await endTheChatsStartedSince(chatsBefore);
    await browser.waitUntil(async () => openedSince(tabsBefore, await tabNames()).length === 0, {
      timeout: 20_000,
      timeoutMsg: "the moving-panes spec left a chat open behind it",
    });
  });

  it("moves the panes to the result before the drop, and changes no terminal's grid", async () => {
    const target = natural[left].frame;
    await pickUp(right);
    // Sampled through a sweep around the pane's edges, to say what the motion costs.
    await browser.execute(() => {
      const frames: number[] = [];
      let last = performance.now();
      const sample = (now: number) => {
        frames.push(now - last);
        last = now;
        if ((window as unknown as { __sampling?: boolean }).__sampling)
          requestAnimationFrame(sample);
      };
      (window as unknown as { __sampling?: boolean; __frames?: number[] }).__sampling = true;
      (window as unknown as { __frames?: number[] }).__frames = frames;
      requestAnimationFrame(sample);
    });
    await moveTo(target.left + target.width / 2, target.top + 4);
    const above = await panesNow();
    expect(
      Math.abs(above[right].frame.width - (target.width + natural[right].frame.width)),
    ).toBeLessThan(40);
    expect(above[right].frame.top).toBeLessThan(above[left].frame.top);
    // The terminal keeps its grid, and each pane says what it will take.
    expect(above[right].grid).toEqual(natural[right].grid);
    expect((await labels()).length).toBe(2);
    await moveTo(target.left + 4, target.top + target.height / 2);
    await moveTo(target.left + target.width / 2, target.top + target.height - 4);
    await moveTo(target.left + target.width / 2, target.top + target.height / 2);
    const swapped = await panesNow();
    expect(Math.round(swapped[right].frame.left)).toBe(Math.round(natural[left].frame.left));
    const long = await browser.execute(() => {
      const held = window as unknown as { __sampling?: boolean; __frames?: number[] };
      held.__sampling = false;
      const frames = (held.__frames ?? []).slice().sort((a, b) => a - b);
      return {
        frames: frames.length,
        p95: frames[Math.floor(frames.length * 0.95)] ?? 0,
        worst: frames[frames.length - 1] ?? 0,
      };
    });
    console.log(`moving-panes: during a sweep ${JSON.stringify(long)}`);
    expect(await gridChanges()).toEqual([]);
  });

  it("puts everything back on Esc, and changes no terminal's grid", async () => {
    await browser.execute(() =>
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })),
    );
    await browser.pause(300);
    const back = await panesNow();
    for (const session of [left, right]) {
      expect(Math.round(back[session].frame.left)).toBe(Math.round(natural[session].frame.left));
      expect(Math.round(back[session].frame.width)).toBe(Math.round(natural[session].frame.width));
      expect(back[session].grid).toEqual(natural[session].grid);
    }
    expect(await gridChanges()).toEqual([]);
    expect(await labels()).toEqual([]);
  });

  it("lands where it said it would, and changes each terminal's grid once on the drop", async () => {
    const target = natural[left].frame;
    await pickUp(right);
    await moveTo(target.left + target.width / 2, target.top + 4);
    const promised = await panesNow();
    const said = await labels();
    expect(await gridChanges()).toEqual([]);
    await browser.execute(
      (x: number, y: number) => {
        window.dispatchEvent(new PointerEvent("pointerup", { clientX: x, clientY: y }));
      },
      target.left + target.width / 2,
      target.top + 4,
    );
    await browser.waitUntil(async () => (await gridChanges()).length >= 2, {
      timeout: 10_000,
      timeoutMsg: "a terminal's grid never changed after the drop",
    });
    await browser.pause(1_000);
    await panesStill();
    const told = await gridChanges();
    console.log(
      `moving-panes: changed ${JSON.stringify(told)}; the labels said ${JSON.stringify(said)}`,
    );
    const dropped = await panesNow();
    // Once each: the layout's one change is the one the observer heard.
    expect(told.filter(({ session }) => session === left)).toHaveLength(1);
    expect(told.filter(({ session }) => session === right)).toHaveLength(1);
    // Where the preview drew it, to the pixel its rounding allows.
    for (const session of [left, right]) {
      expect(Math.abs(dropped[session].frame.left - promised[session].frame.left)).toBeLessThan(2);
      expect(Math.abs(dropped[session].frame.top - promised[session].frame.top)).toBeLessThan(2);
      expect(Math.abs(dropped[session].frame.width - promised[session].frame.width)).toBeLessThan(
        2,
      );
      expect(Math.abs(dropped[session].frame.height - promised[session].frame.height)).toBeLessThan(
        2,
      );
    }
    // What the label said is what the terminal took.
    expect(said.slice().sort()).toEqual(
      [left, right].map((one) => `${dropped[one].grid.columns} × ${dropped[one].grid.rows}`).sort(),
    );
  });
});
