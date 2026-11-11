// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Terminal } from "@xterm/xterm";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type * as Bench from "./bench";

/**
 * **The `project switch` job ends when the pane the switch put on screen shows its sentinel**
 * (#6183): whatever brought the text there. `project-switch.e2e.ts` failed in the merge queue
 * with "the pane never painted ready" while the pane on screen showed it: the job only took the
 * sentinel arriving as new output, after it had armed a pane found by its session number, and
 * every `switch-<n>` project's chat is session 1.
 */

let bench: typeof Bench;

beforeEach(async () => {
  vi.stubEnv("VITE_E2E", "1");
  vi.resetModules();
  bench = await import("./bench");
  bench.attach();
});

afterEach(() => {
  vi.unstubAllEnvs();
  document.body.replaceChildren();
});

/** A terminal as the job reads one: its element, its lines, and its draws. */
function aTerminal(lines: string[]): Terminal & { draw(): void; element: HTMLElement } {
  const element = document.createElement("div");
  const drawing = new Set<() => void>();
  return {
    element,
    buffer: {
      active: {
        get length() {
          return lines.length;
        },
        getLine: (y: number) =>
          y < lines.length ? { translateToString: () => lines[y] } : undefined,
      },
    },
    onRender(run: () => void) {
      drawing.add(run);
      return { dispose: () => drawing.delete(run) };
    },
    draw() {
      for (const run of [...drawing]) run();
    },
  } as unknown as Terminal & { draw(): void; element: HTMLElement };
}

/** The project strip with a tab for `plane`, and the switcher's row for it, which runs `switched`. */
function aStripAndARow(plane: string, switched: () => void): void {
  const strip = document.createElement("div");
  strip.setAttribute("data-strip", "Projects");
  const tab = document.createElement("div");
  tab.setAttribute("role", "tab");
  tab.setAttribute("data-directory", plane);
  tab.setAttribute("aria-selected", "false");
  strip.append(tab);
  const row = document.createElement("div");
  row.id = `palette-row-project.select:${plane}`;
  row.addEventListener("click", () => {
    tab.setAttribute("aria-selected", "true");
    switched();
  });
  document.body.append(strip, row);
}

/** Runs a job and answers with what it ended with, or `undefined` if it is still running. */
async function finished(
  plan: Bench.Plan,
  withinMs = 3_000,
): Promise<ReturnType<Bench.Bench["poll"]> | undefined> {
  const seam = window.charterBench as Bench.Bench;
  seam.begin(plan);
  const until = performance.now() + withinMs;
  while (performance.now() < until) {
    const state = seam.poll();
    if (!state.running) return state;
    await new Promise((next) => setTimeout(next, 20));
  }
  return undefined;
}

it("ends on a kept terminal put back after the strip marked the switch, its sentinel already on it", async () => {
  // Built while its project was in front, then kept off the window: it holds what it was sent.
  const kept = aTerminal(["…", "session ready"]);
  bench.paneOpened(1, kept);
  aStripAndARow("/switch-1", () => {
    // The view comes back a moment after the strip says the project is in front, and puts the
    // kept terminal back: it is sent nothing, since it already has it all.
    setTimeout(() => {
      document.body.append(kept.element);
      bench.paneShownAgain(1, kept);
      requestAnimationFrame(() => kept.draw());
    }, 100);
  });

  const state = await finished({ kind: "project switch", plane: "/switch-1", sentinel: "ready" });

  expect(state?.trouble).toBeUndefined();
  expect(state).toMatchObject({ running: false, result: { session: 1, built: false } });
});

it("ends on the new pane, when the project it left has a pane of the same session on screen", async () => {
  // The project in front: its chat is session 1, and its pane shows the sentinel too.
  const left = aTerminal(["session ready"]);
  document.body.append(left.element);
  bench.paneOpened(1, left);
  // The project switched to: its chat is session 1 as well, its terminal new, opened before its
  // view is in the window and while the one it replaces still is.
  const lines: string[] = [];
  const built = aTerminal(lines);
  aStripAndARow("/switch-2", () => {
    bench.paneOpened(1, built);
    setTimeout(() => {
      left.element.remove();
      document.body.append(built.element);
      // The screen it is sent, parsed into its grid, then drawn.
      const parsed = bench.paneSent(built, new TextEncoder().encode("\r\nsession ready\r\n"));
      lines.push("session ready");
      parsed?.();
      requestAnimationFrame(() => built.draw());
    }, 100);
  });

  const state = await finished({ kind: "project switch", plane: "/switch-2", sentinel: "ready" });

  expect(state?.trouble).toBeUndefined();
  expect(state).toMatchObject({ running: false, result: { session: 1, built: true } });
});
