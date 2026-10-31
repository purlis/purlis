// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { useWindowLeft, type WindowLeft } from "./windowLeft";

/**
 * **Whether the pointer left the window, or the window the front** (#3095): one answer, however
 * the engine says it. Chromium says the root's `pointerleave`; WebKit may say only an `out` into
 * nothing, so all are heard, and one trip out is told once.
 */

afterEach(cleanup);

function Watching({ heard, on = true }: { heard: WindowLeft[]; on?: boolean }) {
  useWindowLeft((why) => heard.push(why), on);
  return null;
}

const root = () => document.documentElement;
const back = () => fireEvent.pointerMove(document, { clientX: 10, clientY: 10 });

describe("the pointer leaving the window", () => {
  it.each<[string, () => void]>([
    ["the root's pointerleave", () => void fireEvent.pointerLeave(root())],
    [
      "a pointerout into nothing",
      () => void fireEvent.pointerOut(document, { relatedTarget: null }),
    ],
    ["a mouseout into nothing", () => void fireEvent.mouseOut(document, { relatedTarget: null })],
  ])("is told by %s", (_, leave) => {
    const heard: WindowLeft[] = [];
    render(<Watching heard={heard} />);
    back();
    leave();
    expect(heard).toEqual(["pointer"]);
  });

  it("is told once for one trip out, however many ways the engine says it", () => {
    const heard: WindowLeft[] = [];
    render(<Watching heard={heard} />);
    back();
    fireEvent.pointerOut(document, { relatedTarget: null });
    fireEvent.mouseOut(document, { relatedTarget: null });
    fireEvent.pointerLeave(root());
    expect(heard).toEqual(["pointer"]);
  });

  it("is told again after the first move back in the window", () => {
    const heard: WindowLeft[] = [];
    render(<Watching heard={heard} />);
    back();
    fireEvent.pointerLeave(root());
    back();
    fireEvent.pointerLeave(root());
    expect(heard).toEqual(["pointer", "pointer"]);
  });

  it("is not told for a move from one element to another", () => {
    const heard: WindowLeft[] = [];
    render(<Watching heard={heard} />);
    back();
    fireEvent.pointerOut(document.body, { relatedTarget: document.body });
    fireEvent.mouseOut(document.body, { relatedTarget: document.body });
    expect(heard).toEqual([]);
  });
});

describe("the window losing focus", () => {
  it("is told apart from the pointer, each time", () => {
    const heard: WindowLeft[] = [];
    render(<Watching heard={heard} />);
    fireEvent.blur(window);
    fireEvent.blur(window);
    expect(heard).toEqual(["focus", "focus"]);
  });

  it("does not stand in for the pointer's own trip out", () => {
    const heard: WindowLeft[] = [];
    render(<Watching heard={heard} />);
    back();
    fireEvent.blur(window);
    fireEvent.pointerLeave(root());
    expect(heard).toEqual(["focus", "pointer"]);
  });
});

describe("who hears it", () => {
  it("is everyone watching, until they stop or are off", () => {
    const one: WindowLeft[] = [];
    const two: WindowLeft[] = [];
    const off: WindowLeft[] = [];
    const drawn = render(<Watching heard={one} />);
    const other = render(<Watching heard={two} />);
    render(<Watching heard={off} on={false} />);
    fireEvent.blur(window);
    drawn.unmount();
    fireEvent.blur(window);
    other.unmount();
    expect(one).toEqual(["focus"]);
    expect(two).toEqual(["focus", "focus"]);
    expect(off).toEqual([]);
  });

  it("tells a watcher that threw's neighbours all the same", () => {
    const heard: WindowLeft[] = [];
    function Throws() {
      useWindowLeft(() => {
        throw new Error("a watcher broke");
      });
      return null;
    }
    // Thrown again in a microtask of its own, kept here, so it is still reported.
    const stray: unknown[] = [];
    const later = vi.spyOn(globalThis, "queueMicrotask").mockImplementation((run) => {
      try {
        run();
      } catch (err: unknown) {
        stray.push(err);
      }
    });
    render(<Throws />);
    render(<Watching heard={heard} />);
    fireEvent.blur(window);
    later.mockRestore();
    expect(stray).toHaveLength(1);
    expect(heard).toEqual(["focus"]);
  });
});
