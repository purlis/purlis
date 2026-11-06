// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { vi } from "vitest";
import { act } from "@testing-library/react";
import { FitAddon } from "@xterm/addon-fit";
import type { Terminal } from "@xterm/xterm";

/**
 * **A browser's layout, for a drag over real panes and a real xterm** (#5291, #5523). jsdom lays
 * nothing out, so this does what a browser's layout does for a window test: each box is what the
 * test's window gives it (`data-box`), or the pixels a drag writes on it, and a resize observer
 * hears only an element whose box changed ({@link layOut}). A fit reads the holder as
 * `@xterm/addon-fit` reads its parent, in whole cells of {@link CELL} pixels.
 */

/** How many pixels a terminal's cell is, across and down. */
export const CELL = 10;

export type Box = { left: number; top: number; width: number; height: number };

/** The box an element is drawn at: the pixels written on it, else its frame's, else its own. */
export function boxOf(el: Element): Box {
  const own = el.getAttribute("data-box");
  const written = el as HTMLElement;
  const pixels = (value: string) => (value === "" ? undefined : parseFloat(value));
  const from =
    own !== null
      ? (() => {
          const [left, top, width, height] = own.split(",").map(Number);
          return { left, top, width, height };
        })()
      : el.parentElement !== null
        ? boxOf(el.parentElement)
        : { left: 0, top: 0, width: 0, height: 0 };
  return {
    ...from,
    width: pixels(written.style?.width ?? "") ?? from.width,
    height: pixels(written.style?.height ?? "") ?? from.height,
  };
}

/** Every observer the panes made, and what each heard last. */
const watched: { owner: object; heard: () => void; on: Element; size: string }[] = [];
const sizeOf = (el: Element) => {
  const { width, height } = boxOf(el);
  return `${width}×${height}`;
};

/** What a browser does after a frame: an observer hears an element whose box changed. */
export function layOut() {
  act(() => {
    for (const one of watched) {
      if (!one.on.isConnected || sizeOf(one.on) === one.size) continue;
      one.size = sizeOf(one.on);
      one.heard();
    }
  });
}

/** The layout stood in for, from a test's `beforeEach`: undone by `vi.restoreAllMocks`. */
export function layOutAsABrowserDoes() {
  watched.length = 0;
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private readonly heard: () => void) {}
      observe(on: Element) {
        // As a browser does, it is heard once when it starts being watched.
        watched.push({ owner: this, heard: () => this.heard(), on, size: "" });
      }
      unobserve() {}
      disconnect() {
        for (let at = watched.length - 1; at >= 0; at--)
          if (watched[at].owner === this) watched.splice(at, 1);
      }
    },
  );
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const { left: x, top: y, width, height } = boxOf(this);
    return DOMRect.fromRect({ x, y, width, height });
  });
  vi.spyOn(FitAddon.prototype, "fit").mockImplementation(function (this: FitAddon) {
    const terminal = (this as unknown as { _terminal: Terminal })._terminal;
    const holder = terminal.element?.parentElement;
    if (!holder) return;
    const { width, height } = boxOf(holder);
    const columns = Math.floor(width / CELL);
    const rows = Math.floor(height / CELL);
    if (columns > 0 && rows > 0 && (terminal.cols !== columns || terminal.rows !== rows))
      terminal.resize(columns, rows);
  });
}

/** A primary pointer's event at a point. */
export const at = (x: number, y: number) => ({
  clientX: x,
  clientY: y,
  button: 0,
  isPrimary: true,
});
