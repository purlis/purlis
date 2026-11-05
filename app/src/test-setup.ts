// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Before anything loads `react-dom`: the hook React's commits are counted through, which React
// looks for only as it is first loaded (`reactCommits.testkit.ts`).
import "./reactCommits.testkit";
// Then, before anything that imports `theme.ts`: the built-in themes the app's core hands the
// real window as it is created.
import "./theme/handed.testkit";
import "@testing-library/jest-dom/vitest";
import { toHaveValue } from "@testing-library/jest-dom/matchers";
import { configure } from "@testing-library/react";
import { afterEach, beforeEach, expect, vi } from "vitest";
import { forgetKeptScopes } from "./chatsScope";
import { forgetExtensionThemes } from "./Extensions";
import { forgetEntering } from "./settings/entering";
import { forgetExtensionsOn } from "./extensionsOn";
import { forgetProjectThemes } from "./projectTheme";
import { forgetSwitches } from "./switchClock";
import { forgetUnappliedRepos } from "./unappliedRepos";
import { forgetPaneBands } from "./noticeSettle";
import { letGoOfKeptTerminals } from "./keptTerminals";
import { PANE_NOTICES } from "./paneNoticesPref";
import { forgetStandIn, whyNoPane } from "./storedLayoutGuard.testkit";

// **Every stand-in core is watched** (#4758, `storedLayoutGuard.testkit.ts`): a test that fails
// after its stand-in reopened chats and laid none of them out is told why no pane came, and the
// fix. What a stand-in answers is unchanged.
vi.mock("@tauri-apps/api/mocks", async (original) => {
  const mocks = await original<typeof import("@tauri-apps/api/mocks")>();
  const { watched } = await import("./storedLayoutGuard.testkit");
  return {
    ...mocks,
    mockIPC: ((handler, options) =>
      mocks.mockIPC(watched(handler), options)) as typeof mocks.mockIPC,
  };
});
beforeEach(() => forgetStandIn());
afterEach((context) => {
  if (context.task.result?.state !== "fail") return;
  const why = whyNoPane();
  if (why !== undefined) throw new Error(`Why no pane may have come: ${why}`);
});

// What each project has on is kept per plane for the window's life (`extensionsOn.ts`); every
// test's window is a new one, and most of them share a plane path.
afterEach(() => {
  // The switches a window timed are its own (`switchClock.ts`); every test's window is a new one.
  forgetSwitches();
  forgetExtensionsOn();
  forgetExtensionThemes();
  forgetProjectThemes();
  // Settings › Repos' unapplied ticks are held per workspace for the window's life (#1192).
  forgetUnappliedRepos();
  // A reload keeps the window's session storage (the Chats list's folds, `chatFolds.ts`); a
  // new window, as every test's is, starts without it.
  globalThis.sessionStorage?.clear();
  // The Chats view's scope is kept per project on this machine (#1679); every test's machine is
  // a new one.
  forgetKeptScopes();
  // A way into Settings stands, and retries on a timer, for up to two seconds (#1206): one a
  // test left standing never fires into the next test, or after its window is gone.
  forgetEntering();
  // What each pane's band held is kept by the pane for the window's life (#3167).
  forgetPaneBands();
  // Minimised or expanded is the machine's choice (`layout.json`, #4754); every test's machine
  // is a new one (below).
  PANE_NOTICES.forget();
  // The last chats shown keep their terminals for the window's life (#4735); every test's window
  // is a new one, and most of them show chat 1 of one plane.
  letGoOfKeptTerminals();
});

// **A test's machine has its panes' notices expanded** (#4754): a test of a note on a pane reads
// the note, as it did before the band could fold. A test whose window is handed a layout file
// has it there too (`test-inbox.ts`). The window's default, minimised, is pinned by
// `paneNotices.test.ts`, and the tests of the fold ask for it themselves
// (`PaneNotices.window.test.tsx`, the bell's in `Notice.pane.test.tsx`).
beforeEach(() => PANE_NOTICES.keep("expanded"));

// jsdom's window is 1024px wide, the narrowest purlis supports, where the project rail hides
// itself to leave the work its room (`railRoom.ts`, #3077). A test's window holds the rail beside
// both sidebars, as the default window (1440 px) does; a test of a narrow window sets its own.
Object.defineProperty(window, "innerWidth", { configurable: true, writable: true, value: 1440 });

// jsdom has no ResizeObserver, and the panes and their splits watch their own size with one.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

// jsdom has no IntersectionObserver either, and the tree family's loading row (React Aria's
// `TreeLoadMoreItem`, "Reading…") watches its place with one. Everything it is given is told at
// once to be in sight, which is what the window takes a webview without one to mean.
globalThis.IntersectionObserver ??= class {
  readonly root = null;
  readonly rootMargin = "";
  readonly thresholds: readonly number[] = [];
  constructor(private readonly told: IntersectionObserverCallback) {}
  observe(target: Element) {
    const entry = {
      target,
      isIntersecting: true,
      intersectionRatio: 1,
    } as IntersectionObserverEntry;
    this.told([entry], this as unknown as IntersectionObserver);
  }
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
} as unknown as typeof IntersectionObserver;

// jsdom has no layout, so a Range has no client rects. CodeMirror measures one when it scrolls
// to a line (RC-20 opens a file at a line), and the measure runs after the test that caused it.
// An empty rect list is what an unlaid-out document would report.
if (typeof Range !== "undefined") {
  Range.prototype.getClientRects ??= () => [] as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect ??= () => new DOMRect(0, 0, 0, 0);
}

// jsdom lays nothing out and has no pointer capture: a list scrolls its chosen row into view and
// asks whether the pointer is held. A real WebView has both.
if (typeof Element !== "undefined") {
  Element.prototype.scrollIntoView ??= () => {};
  Element.prototype.hasPointerCapture ??= () => false;
  Element.prototype.releasePointerCapture ??= () => {};
}

// A select (`ui/select/Select.tsx`) keeps a native `<select>` beside its button, hidden from the
// page and from a screen reader, for a form's autofill: its options are no words a person reads,
// so no query finds them.
//
// **How long a query waits for what it finds** (`findBy…`, `waitFor`): 5 s, not Testing Library's
// one. A whole window comes up through several answers under StrictMode, and a tab of the light
// editor first imports its chunk (`Views.tsx`'s `lazy`); a loaded runner takes past a second for
// either. What a test asserts is WHAT the window becomes, never how fast, so it waits as long as
// a slow machine needs; a wait that must be short says its own `timeout`. Vitest's `testTimeout`
// (`vite.config.ts`) is raised to match.
//
// **Vitest's own waits get the same budget** (#5249). Each stops at a second unless told otherwise,
// and that second took merge groups down. `expect.poll` has a setting for it. `vi.waitFor` and
// `vi.waitUntil` have none (the second is a default in their signature), so they are wrapped here.
// Moving their callers to Testing Library's `waitFor` wouldn't do: that one polls inside `act`,
// which is a different wait (AGENTS.md), and the next `vi.waitFor` written would have the second
// again. A wait only lasts longer when it would have failed.
const WAIT_BUDGET_MS = 5_000;

configure({
  defaultIgnore: 'script, style, [data-testid="hidden-select-container"] *',
  asyncUtilTimeout: WAIT_BUDGET_MS,
});

vi.setConfig({ expect: { poll: { timeout: WAIT_BUDGET_MS } } });

/** `options` as a wait took them, with the budget where they name no timeout of their own. */
const withBudget = <Options extends { timeout?: number }>(options?: number | Options) =>
  typeof options === "number"
    ? options
    : { ...options, timeout: options?.timeout ?? WAIT_BUDGET_MS };

const { waitFor: vitestWaitFor, waitUntil: vitestWaitUntil } = vi;
vi.waitFor = (callback, options) => vitestWaitFor(callback, withBudget(options));
vi.waitUntil = (callback, options) => vitestWaitUntil(callback, withBudget(options));

// A select's value is the option it holds, as `toHaveValue` reads a native one's. Every select in
// the window is the select family's (`ui/select/Select.tsx`, #2906), a `<button>` that carries
// what it holds as `data-value`, so the matcher reads that there and stays jest-dom's everywhere
// else.
expect.extend({
  toHaveValue(this: unknown, element: HTMLElement, expected?: unknown) {
    if (
      !(element instanceof HTMLElement) ||
      !element.matches('[data-scope="select"][data-part="root"]')
    )
      return toHaveValue.call(this as never, element, expected as never);
    const held = element.getAttribute("data-value") ?? "";
    const pass = expected === undefined ? held !== "" : held === expected;
    return {
      pass,
      message: () =>
        `expected the select ${element.getAttribute("aria-label") ?? element.id} ` +
        `${pass ? "not " : ""}to hold ${JSON.stringify(expected)}, and it holds ${JSON.stringify(held)}`,
    };
  },
});
