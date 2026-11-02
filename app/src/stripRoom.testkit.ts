// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { LEAST, leastAt } from "./fits";
import { DEFAULT_TEXT } from "./textSize";
import { stripNamed } from "./test-strips";

/**
 * **The chat strip's room, as a test says it is.** jsdom lays nothing out (#149), so what the
 * strips measure is stubbed rather than pretended at: `clientWidth` answers what a test set, and
 * a `ResizeObserver` of this file's own tells the app when it changed.
 */

/** The wire to what the strips measure. */
export type Room = {
  /** Gives the chat strip room for exactly `tabs` tabs, and tells the app its size changed. */
  roomFor: (tabs: number) => void;
  /** How many live observers are watching the chat strip itself. Scoped to it deliberately:
   *  the resizable panels and Radix's own positioning each put observers up, so a count of
   *  every observer in the window would measure somebody else's library. */
  live: () => number;
  /** Puts `clientWidth` back, so one test's stub is not every later test's. */
  stop: () => void;
};

/**
 * Stubs the one number the strips measure, and the observer that reports changes to it.
 *
 * It answers per element, which is what the real one does — the window has three strips and
 * each measures itself. Only the chat strip is given a width here; the other two measure zero,
 * which is "nobody has said", and draw everything.
 */
export function measuring(): Room {
  const widths = new WeakMap<Element, number>();
  const live: { callback: ResizeObserverCallback; targets: Element[]; gone: boolean }[] = [];
  const clientWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");
  Object.defineProperty(HTMLElement.prototype, "clientWidth", {
    configurable: true,
    get(this: HTMLElement) {
      return widths.get(this) ?? 0;
    },
  });
  const was = globalThis.ResizeObserver;
  globalThis.ResizeObserver = class {
    private readonly mine: (typeof live)[number];
    constructor(callback: ResizeObserverCallback) {
      this.mine = { callback, targets: [], gone: false };
      live.push(this.mine);
    }
    observe(target: Element) {
      this.mine.targets.push(target);
    }
    unobserve() {}
    disconnect() {
      this.mine.gone = true;
    }
  } as unknown as typeof ResizeObserver;
  return {
    roomFor: (tabs) => {
      const strip = stripNamed("Tabs");
      // The floor the strip fits by at the default window text size (charter-app#283).
      widths.set(strip, tabs * leastAt(LEAST.chat, DEFAULT_TEXT.window));
      for (const one of live) {
        if (one.gone || !one.targets.includes(strip)) continue;
        one.callback([], undefined as unknown as ResizeObserver);
      }
    },
    live: () => {
      const strip = stripNamed("Tabs");
      return live.filter((one) => !one.gone && one.targets.includes(strip)).length;
    },
    stop: () => {
      if (clientWidth) Object.defineProperty(HTMLElement.prototype, "clientWidth", clientWidth);
      else Reflect.deleteProperty(HTMLElement.prototype, "clientWidth");
      globalThis.ResizeObserver = was;
    },
  };
}
