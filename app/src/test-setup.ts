// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// First, before anything that imports `theme.ts`: the built-in themes the app's core hands the
// real window as it is created.
import "./theme/handed.testkit";
import "@testing-library/jest-dom/vitest";
import { toHaveValue } from "@testing-library/jest-dom/matchers";
import { configure } from "@testing-library/react";
import { afterEach, expect } from "vitest";
import { forgetKeptScopes } from "./chatsScope";
import { forgetExtensionThemes } from "./Extensions";
import { forgetEntering } from "./settings/entering";
import { forgetExtensionsOn } from "./extensionsOn";
import { forgetProjectThemes } from "./projectTheme";
import { forgetSwitches } from "./switchClock";
import { forgetUnappliedRepos } from "./unappliedRepos";
import { forgetPaneBands } from "./noticeSettle";

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
});

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
configure({
  defaultIgnore: 'script, style, [data-testid="hidden-select-container"] *',
  asyncUtilTimeout: 5_000,
});

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
