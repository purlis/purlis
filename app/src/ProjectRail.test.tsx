// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { PEEK_DWELL, ProjectRail } from "./ProjectRail";
import { afterTheSlide, forgetTheSlide, railSliding } from "./railSlide";
import { forgetRailMode, useRailMode, type RailMode } from "./railMode";
import { inForce } from "./theme/theme";

beforeEach(() => {
  vi.useFakeTimers();
  forgetTheSlide();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  forgetTheSlide();
});

/** The rail with no projects, only its own two actions: what the peek and the slide need. */
function Window({
  mode,
  onMode = () => {},
}: {
  mode: RailMode;
  onMode?: (mode: RailMode) => void;
}) {
  const edge = useRef<HTMLDivElement>(null);
  return (
    <>
      <ProjectRail
        projects={[]}
        inFront={undefined}
        rows={{ open: { title: "Open a project…" }, switchTo: [], close: [] } as never}
        offers={{} as never}
        onPress={() => {}}
        waitingIn={() => 0}
        unsavedIn={() => undefined}
        onDrop={() => {}}
        lettingGo={0}
        mode={mode}
        onMode={onMode}
        edge={edge}
        nameOf={(plane) => plane}
      />
      <div ref={edge} />
    </>
  );
}

const peek = () => document.querySelector('[data-scope="rail"][data-part="column"][data-peek]');

/** The pointer rests at the window's left edge until the peek comes in (R86). */
function peekIn() {
  act(() => void fireEvent.pointerMove(document, { clientX: 0, clientY: 0 }));
  act(() => void vi.advanceTimersByTime(PEEK_DWELL));
  expect(peek()).not.toBeNull();
  expect(peek()).not.toHaveAttribute("data-leaving");
}

describe("the project rail's peek (#2915)", () => {
  it("goes when the window loses focus (S4)", () => {
    render(<Window mode="auto" />);
    peekIn();
    act(() => void fireEvent.blur(window));
    expect(peek()).toHaveAttribute("data-leaving");
  });

  it.each([
    ["pointerout", (to: Document) => fireEvent.pointerOut(to, { relatedTarget: null })],
    ["mouseout", (to: Document) => fireEvent.mouseOut(to, { relatedTarget: null })],
    ["pointerleave", () => fireEvent.pointerLeave(document.documentElement)],
  ])("goes when the pointer leaves the window, told by %s (S4)", async (_, leave) => {
    render(<Window mode="auto" />);
    peekIn();
    await act(async () => void leave(document));
    expect(peek()).toHaveAttribute("data-leaving");
  });

  it("stays when the pointer only moves from one element to another", () => {
    render(<Window mode="auto" />);
    peekIn();
    act(() => void fireEvent.pointerOut(document.body, { relatedTarget: document.body }));
    expect(peek()).not.toHaveAttribute("data-leaving");
  });

  it("is not left behind by a show for the next hide (S3)", () => {
    const { rerender } = render(<Window mode="auto" />);
    peekIn();
    rerender(<Window mode="shown" />);
    expect(peek()).toBeNull();
    rerender(<Window mode="auto" />);
    expect(peek()).toBeNull();
  });
});

describe("the project rail's slide (#2915, B1)", () => {
  const slot = () => {
    const found = document.querySelector<HTMLElement>('[data-scope="rail"][data-part="slot"]');
    if (!found) throw new Error("no rail slot");
    return found;
  };

  it("starts when the mode changes, and ends when its column's width has moved", () => {
    const { rerender } = render(<Window mode="shown" />);
    expect(railSliding()).toBe(false);
    rerender(<Window mode="auto" />);
    expect(railSliding()).toBe(true);
    // A transition of something else inside it is not the column's.
    act(() => void fireEvent.transitionEnd(slot(), { propertyName: "opacity" }));
    expect(railSliding()).toBe(true);
    act(() => void fireEvent.transitionEnd(slot(), { propertyName: "inline-size" }));
    expect(railSliding()).toBe(false);
  });

  it("ends after its length, should its column never say so", () => {
    const { rerender } = render(<Window mode="auto" />);
    rerender(<Window mode="shown" />);
    expect(railSliding()).toBe(true);
    act(() => void vi.advanceTimersByTime(inForce().motion.durations["duration.slide"] + 100));
    expect(railSliding()).toBe(false);
  });

  it("ends when the rail goes mid-slide, and what waited for it runs", () => {
    const { rerender, unmount } = render(<Window mode="shown" />);
    rerender(<Window mode="auto" />);
    const refit = vi.fn();
    afterTheSlide(refit);
    expect(refit).not.toHaveBeenCalled();
    unmount();
    expect(railSliding()).toBe(false);
    expect(refit).toHaveBeenCalledTimes(1);
    // And what asks after it runs at once.
    afterTheSlide(refit);
    expect(refit).toHaveBeenCalledTimes(2);
  });
});

describe("the project rail in a narrow window (#3077)", () => {
  /** The rail drawn as the window's mode says, which follows the window's width. */
  function Drawn() {
    const [mode, onMode] = useRailMode();
    return <Window mode={mode} onMode={onMode} />;
  }
  const slot = () => document.querySelector<HTMLElement>('[data-scope="rail"][data-part="slot"]');
  function windowIs(width: number) {
    act(() => {
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        writable: true,
        value: width,
      });
      window.dispatchEvent(new Event("resize"));
    });
  }
  beforeEach(() => forgetRailMode());
  afterEach(() => {
    windowIs(1440);
    forgetRailMode();
  });

  it("hides and comes back at once as the window narrows and widens, gliding nothing", () => {
    windowIs(1440);
    render(<Drawn />);
    expect(slot()).not.toHaveAttribute("data-hidden");
    windowIs(1024);
    expect(slot()).toHaveAttribute("data-hidden");
    // Drawn with no transition, and nothing waits for a slide that does not run.
    expect(slot()).toHaveAttribute("data-settling");
    expect(railSliding()).toBe(false);
    act(() => void vi.runAllTimers());
    windowIs(1440);
    expect(slot()).not.toHaveAttribute("data-hidden");
    expect(railSliding()).toBe(false);
  });

  it("slides for the person's own show in a narrow window, and stays", () => {
    windowIs(1024);
    render(<Drawn />);
    expect(slot()).toHaveAttribute("data-hidden");
    // The title bar's show is App's; the rail's Keep from its peek is the same press.
    act(() => void fireEvent.pointerMove(document, { clientX: 0, clientY: 0 }));
    act(() => void vi.advanceTimersByTime(PEEK_DWELL));
    act(
      () =>
        void document.querySelector<HTMLElement>('[data-peek] [data-rail-toggle="keep"]')?.click(),
    );
    expect(slot()).not.toHaveAttribute("data-hidden");
    windowIs(1440);
    windowIs(1024);
    expect(slot()).not.toHaveAttribute("data-hidden");
  });

  it("hands the keyboard to the title bar's show when the window hides the rail it was in", () => {
    windowIs(1440);
    render(
      <>
        <button data-rail-show="">Show the projects sidebar</button>
        <Drawn />
      </>,
    );
    const hide = document.querySelector<HTMLElement>('[data-rail-toggle="hide"]');
    act(() => hide?.focus());
    expect(hide).toHaveFocus();
    windowIs(1024);
    expect(document.querySelector("[data-rail-show]")).toHaveFocus();
  });
});
