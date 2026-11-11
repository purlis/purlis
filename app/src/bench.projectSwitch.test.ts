// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Terminal } from "@xterm/xterm";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type * as Bench from "./bench";

/** The `project switch` job's ending, wherever the sentinel came from (#6183, `bench.ts`). */

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
  bench.paneOpened(1, kept, "/switch-1");
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
  expect((state?.result as { ms: number }).ms).toBeGreaterThanOrEqual(100);
});

it("ends on the new pane, when the project it left has a pane of the same session on screen", async () => {
  // The project in front: its chat is session 1, and its pane shows the sentinel too.
  const left = aTerminal(["session ready"]);
  document.body.append(left.element);
  bench.paneOpened(1, left, "/switch-1");
  // The project switched to: its chat is session 1 as well, its terminal new, opened before its
  // view is in the window and while the one it replaces still is.
  const lines: string[] = [];
  const built = aTerminal(lines);
  aStripAndARow("/switch-2", () => {
    bench.paneOpened(1, built, "/switch-2");
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
  // Painted by the new pane, sent its screen 100 ms on, and not by the one it replaced.
  expect((state?.result as { ms: number }).ms).toBeGreaterThanOrEqual(100);
});

it("ends on a pane of the project switched to that was in the window before the press, its sentinel already on it", async () => {
  // The project in front, whose pane shows the sentinel too.
  const front = aTerminal(["session ready"]);
  document.body.append(front.element);
  bench.paneOpened(1, front, "/switch-1");
  // The project switched to: its pane is already in the window as the press comes, holding the
  // sentinel, and nothing opens or is put back for it. Only the strip's mark moves.
  const there = aTerminal(["session ready"]);
  document.body.append(there.element);
  bench.paneOpened(1, there, "/switch-2");
  aStripAndARow("/switch-2", () => {
    setTimeout(() => {
      front.element.remove();
      there.draw();
    }, 100);
  });

  const state = await finished({ kind: "project switch", plane: "/switch-2", sentinel: "ready" });

  expect(state?.trouble).toBeUndefined();
  expect(state).toMatchObject({ running: false, result: { session: 1, built: false } });
});

it("ends on the pane of the project switched to, when a pane of another project with its session stays in the window", async () => {
  // The project switched to: its chat is session 1, its pane built before the press and put in
  // the window by the switch.
  const there = aTerminal(["session ready"]);
  bench.paneOpened(1, there, "/switch-2");
  // A pane of another project, session 1 as well, opened after it and in the window throughout.
  const other = aTerminal(["session ready"]);
  document.body.append(other.element);
  bench.paneOpened(1, other, "/elsewhere");
  aStripAndARow("/switch-2", () => {
    setTimeout(() => {
      document.body.append(there.element);
      requestAnimationFrame(() => there.draw());
    }, 100);
  });

  const state = await finished({ kind: "project switch", plane: "/switch-2", sentinel: "ready" });

  expect(state?.trouble).toBeUndefined();
  expect(state).toMatchObject({ running: false, result: { session: 1, built: false } });
  expect((state?.result as { ms: number }).ms).toBeGreaterThanOrEqual(100);
});

it("ends with what the window showed when the project switched to never paints its sentinel", async () => {
  // Its pane is in the window and holds something else; nothing ever brings the sentinel.
  const there = aTerminal(["charter: session 1 is not running"]);
  bench.paneOpened(1, there, "/switch-3");
  aStripAndARow("/switch-3", () => document.body.append(there.element));

  const state = await finished(
    { kind: "project switch", plane: "/switch-3", sentinel: "ready", patienceMs: 300 },
    2_000,
  );

  expect(state?.running).toBe(false);
  expect(state?.trouble).toMatch(/never painted ready/);
  // Enough to tell, from the job's log alone, which pane it was and what it had been sent.
  expect(state?.trouble).toMatch(/1 pane of \/switch-3 in the window/);
});

/** What a project's tab in front draws when it is not a chat (#6200): `view` names a view
 *  panel, and `undefined` the empty pane a project with no chats shows. */
function aFrontThatIsNotAChat(view?: string): HTMLElement {
  const front = document.createElement("div");
  if (view === undefined) {
    front.setAttribute("data-testid", "empty-pane");
  } else {
    front.setAttribute("role", "tabpanel");
    front.setAttribute("data-view", view);
  }
  return front;
}

it("ends at once when the project switched to has a view in front, with no pane to paint", async () => {
  // Its tab in front is a view: nothing in it is a chat, so nothing will ever paint the sentinel.
  aStripAndARow("/switch-4", () => document.body.append(aFrontThatIsNotAChat("notices")));

  // The job's own patience is its default, a minute: it must end on what the window shows.
  const from = performance.now();
  const state = await finished({ kind: "project switch", plane: "/switch-4", sentinel: "ready" });

  expect(state?.running).toBe(false);
  expect(performance.now() - from).toBeLessThan(2_000);
  expect(state?.trouble).toMatch(/project \/switch-4 is in front with no pane to open/);
  expect(state?.trouble).toMatch(/its tab in front is the notices view/);
});

it("ends at once when the project switched to has no chat, its empty pane in front", async () => {
  aStripAndARow("/switch-5", () => document.body.append(aFrontThatIsNotAChat()));

  const from = performance.now();
  const state = await finished({ kind: "project switch", plane: "/switch-5", sentinel: "ready" });

  expect(state?.running).toBe(false);
  expect(performance.now() - from).toBeLessThan(2_000);
  expect(state?.trouble).toMatch(/project \/switch-5 is in front with no pane to open/);
  expect(state?.trouble).toMatch(/its tab in front is an empty pane/);
});

it("waits for a chat's pane that opens a while after the project's view is drawn, not ending on its empty start", async () => {
  // The view is drawn with an empty pane for a moment, then the chat's pane takes its place.
  const lines: string[] = [];
  const built = aTerminal(lines);
  aStripAndARow("/switch-6", () => {
    const empty = aFrontThatIsNotAChat();
    document.body.append(empty);
    setTimeout(() => {
      empty.remove();
      const pane = document.createElement("div");
      pane.setAttribute("data-testid", "pane");
      pane.append(built.element);
      document.body.append(pane);
      bench.paneOpened(1, built, "/switch-6");
      const parsed = bench.paneSent(built, new TextEncoder().encode("\r\nsession ready\r\n"));
      lines.push("session ready");
      parsed?.();
      requestAnimationFrame(() => built.draw());
    }, 150);
  });

  const state = await finished({ kind: "project switch", plane: "/switch-6", sentinel: "ready" });

  expect(state?.trouble).toBeUndefined();
  expect(state).toMatchObject({ running: false, result: { session: 1, built: true } });
});
