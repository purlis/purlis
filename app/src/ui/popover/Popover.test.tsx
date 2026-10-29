// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The popover family's wrapper** (#2909): a popover held up by its owner comes and goes as its
 * owner says, every time, and R165's anatomy is one list.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { useRef, useState } from "react";
import { InlineMenu, MenuItem } from "../menu/Menu";
import { AnchoredPopover, HungPopover, PopoverFilter, PopoverFooter, PopoverList } from "./Popover";

/** jsdom's own `scrollIntoView`, if it has one, put back after a test that stood one in. */
const SCROLL_INTO_VIEW = Element.prototype.scrollIntoView;

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  Element.prototype.scrollIntoView = SCROLL_INTO_VIEW;
  document.documentElement.style.removeProperty("--motion-duration-slide");
});

/** A box per element: the strip 0–300 across, the chip wherever `chipAt` says now. */
function laidOut(chipAt: () => number) {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const strip = this.getAttribute("data-testid") === "clip";
    const left = strip ? 0 : this.textContent === "gamma" ? chipAt() : 0;
    const width = strip ? 300 : 60;
    return {
      x: left,
      y: 0,
      left,
      top: 0,
      right: left + width,
      bottom: 28,
      width,
      height: 28,
      toJSON: () => ({}),
    } as DOMRect;
  });
}

/** One frame, and what React draws in it. */
const aFrame = () => act(() => new Promise<void>((done) => requestAnimationFrame(() => done())));

function Refusal({ up }: { up: boolean }) {
  const box = useRef<HTMLSpanElement>(null);
  return (
    <>
      <span ref={box}>web</span>
      <AnchoredPopover triggerRef={box} isOpen={up}>
        <div role="alert">That name is taken.</div>
      </AnchoredPopover>
    </>
  );
}

describe("a popover held up by its owner", () => {
  it("comes back each time its owner holds it up again", () => {
    const { rerender } = render(<Refusal up />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    for (let again = 0; again < 3; again += 1) {
      rerender(<Refusal up={false} />);
      expect(screen.queryByRole("alert")).toBeNull();
      rerender(<Refusal up />);
      expect(screen.getByRole("alert")).toBeInTheDocument();
    }
  });
});

describe("a popover laid out as a list (R165)", () => {
  it("has one list part, which the rows of a menu in it are laid out in", () => {
    render(
      <div data-scope="popover" data-part="root" data-layout="list">
        <PopoverFilter>
          <span>filter</span>
        </PopoverFilter>
        <PopoverList>
          <InlineMenu aria-label="Workspaces">
            <MenuItem textValue="alpha">alpha</MenuItem>
          </InlineMenu>
        </PopoverList>
        <PopoverFooter />
      </div>,
    );
    const parts = [...document.querySelectorAll("[data-part]")].map((one) =>
      one.getAttribute("data-part"),
    );
    expect(parts.filter((one) => one === "list")).toHaveLength(1);
    expect(parts.filter((one) => ["filter", "rule", "list", "footer"].includes(one ?? ""))).toEqual(
      ["filter", "rule", "list", "footer"],
    );
    expect(screen.getByRole("menu", { name: "Workspaces" })).toHaveAttribute("data-in-list");
  });
});

/** A card hung from a chip (#3648): up until a press elsewhere hides it. */
function Hung() {
  const chip = useRef<HTMLButtonElement>(null);
  const strip = useRef<HTMLDivElement>(null);
  const [up, setUp] = useState(true);
  return (
    <>
      <div ref={strip} data-testid="clip">
        <button ref={chip} type="button">
          gamma
        </button>
      </div>
      <button type="button">elsewhere</button>
      <HungPopover
        triggerRef={chip}
        clipRef={strip}
        isOpen={up}
        onHide={() => setUp(false)}
        aria-label="gamma"
      >
        <div role="region" aria-label="Notices for gamma">
          <button type="button">Add a vision</button>
        </div>
      </HungPopover>
    </>
  );
}

describe("a popover hung from what it is about (#3648)", () => {
  it("is the family's notice surface, centred under it, with its point", () => {
    render(<Hung />);
    const root = screen
      .getByRole("region", { name: "Notices for gamma" })
      .closest('[data-scope="popover"][data-part="root"]');
    expect(root).toHaveAttribute("data-variant", "notice");
    expect(root).toHaveAttribute("data-placement", "bottom");
    // Its point is a part of its own: the SVG arrow a held popover draws is not it.
    expect(root?.querySelector('[data-scope="popover"][data-part="point"]')).not.toBeNull();
    expect(root?.querySelector('[data-part="arrow"]')).toBeNull();
  });

  it("takes no keyboard as it comes", () => {
    const elsewhere = document.createElement("input");
    document.body.append(elsewhere);
    elsewhere.focus();
    render(<Hung />);
    expect(document.activeElement).toBe(elsewhere);
    elsewhere.remove();
  });

  it("stays up for a press inside it, and is hidden by a press anywhere else", () => {
    render(<Hung />);
    fireEvent.pointerDown(screen.getByRole("button", { name: "Add a vision" }));
    expect(screen.getByRole("region", { name: "Notices for gamma" })).toBeInTheDocument();
    fireEvent.pointerDown(screen.getByRole("button", { name: "elsewhere" }));
    expect(screen.queryByRole("region", { name: "Notices for gamma" })).toBeNull();
  });

  it("stays up while what it hangs from scrolls, as a strip brings a new chip into view", () => {
    // React Aria closes a popover that is not modal when an ancestor of its trigger scrolls; a
    // chip's card is not taken down by its strip scrolling the chip into sight (R104).
    render(
      <div data-testid="strip" style={{ overflowX: "auto" }}>
        <Hung />
      </div>,
    );
    fireEvent.scroll(screen.getByTestId("strip"));
    expect(screen.getByRole("region", { name: "Notices for gamma" })).toBeInTheDocument();
  });

  it("is hidden once what it hangs from has scrolled out of its strip's sight (#3648's review)", async () => {
    let chipAt = 100;
    laidOut(() => chipAt);
    render(<Hung />);
    // Still in sight: it follows, and stays up.
    chipAt = 200;
    fireEvent.scroll(screen.getByTestId("clip"));
    await aFrame();
    expect(screen.getByRole("region", { name: "Notices for gamma" })).toBeInTheDocument();
    // Out of sight past the strip's start: down, and nothing is dismissed (its owner hides it).
    chipAt = -80;
    fireEvent.scroll(screen.getByTestId("clip"));
    await aFrame();
    expect(screen.queryByRole("region", { name: "Notices for gamma" })).toBeNull();
  });

  it("brings what it hangs from into sight as it comes, and stays up while that scrolls in", async () => {
    // The window's own motion, not reduced (`workspaceSlide.moves`): a smooth scroll, frame by
    // frame, with what it hangs from out of sight on the first of them (#3648's round 3).
    // A slide's duration the motion layer would write (`theme/motion.ts`), so the window moves.
    document.documentElement.style.setProperty("--motion-duration-slide", `${200}ms`);
    let chipAt = -80;
    laidOut(() => chipAt);
    const into = vi.fn();
    Element.prototype.scrollIntoView = into;
    render(<Hung />);
    await aFrame();
    expect(into).toHaveBeenCalledWith({ inline: "nearest", block: "nearest", behavior: "smooth" });
    for (const at of [-80, -40, 20, 100]) {
      chipAt = at;
      fireEvent.scroll(screen.getByTestId("clip"));
      await aFrame();
      expect(
        screen.getByRole("region", { name: "Notices for gamma" }),
        String(at),
      ).toBeInTheDocument();
    }
    // Arrived, then scrolled away: down.
    chipAt = -80;
    fireEvent.scroll(screen.getByTestId("clip"));
    await aFrame();
    expect(screen.queryByRole("region", { name: "Notices for gamma" })).toBeNull();
  });

  it("is hidden once a scroll that should have brought it in ends with it still out of sight", async () => {
    // The scroll it asked for was cut short (a wheel back, a step to another chip): what it hangs
    // from never came into sight, so the card is not left over nothing (#3648's round 4).
    let chipAt = -80;
    laidOut(() => chipAt);
    Element.prototype.scrollIntoView = vi.fn();
    render(<Hung />);
    await aFrame();
    chipAt = -60;
    fireEvent.scroll(screen.getByTestId("clip"));
    await aFrame();
    expect(screen.getByRole("region", { name: "Notices for gamma" })).toBeInTheDocument();
    fireEvent(screen.getByTestId("clip"), new Event("scrollend"));
    await aFrame();
    expect(screen.queryByRole("region", { name: "Notices for gamma" })).toBeNull();
  });

  it("is placed again on a scroll's last frame, though the scroll ends in the same frame", () => {
    // A scroll and its end in one frame (a scroll set at once): the end never cancels the frame
    // that places it again where what it hangs from now is (#3648's round 4).
    laidOut(() => 100);
    let next = 0;
    vi.spyOn(window, "requestAnimationFrame").mockImplementation(() => ++next);
    const cancelled = vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
    render(<Hung />);
    const before = next;
    fireEvent.scroll(screen.getByTestId("clip"));
    const following = Array.from({ length: next - before }, (_, at) => before + 1 + at);
    expect(following.length).toBeGreaterThan(0);
    cancelled.mockClear();
    fireEvent(screen.getByTestId("clip"), new Event("scrollend"));
    const gone = cancelled.mock.calls.map(([id]) => id);
    expect(gone.filter((id) => following.includes(id))).toEqual([]);
  });

  it("stays up when a scroll ends with what it hangs from in sight", async () => {
    let chipAt = -80;
    laidOut(() => chipAt);
    Element.prototype.scrollIntoView = vi.fn();
    render(<Hung />);
    chipAt = 100;
    fireEvent(screen.getByTestId("clip"), new Event("scrollend"));
    await aFrame();
    expect(screen.getByRole("region", { name: "Notices for gamma" })).toBeInTheDocument();
  });

  it("stays up when the keyboard leaves it, by Tab or F6: only a press elsewhere hides it", () => {
    render(<Hung />);
    screen.getByRole("button", { name: "Add a vision" }).focus();
    screen.getByRole("button", { name: "elsewhere" }).focus();
    expect(screen.getByRole("region", { name: "Notices for gamma" })).toBeInTheDocument();
  });

  it("is hidden by a press on what it hangs from, too", () => {
    render(<Hung />);
    fireEvent.pointerDown(screen.getByRole("button", { name: "gamma" }));
    expect(screen.queryByRole("region", { name: "Notices for gamma" })).toBeNull();
  });
});

describe("the family's stylesheet (#3648's review)", () => {
  const css = readFileSync(join(process.cwd(), "src/ui/popover/popover.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );
  it("draws a held popover's SVG arrow by its path alone, and the notice's point apart", () => {
    // A box, an edge or a turn on the arrow would draw a diamond behind a held popover's triangle.
    const arrow = [...css.matchAll(/([^{}]*\[data-part="arrow"\][^{}]*)\{([^}]*)\}/g)];
    expect(arrow.map(([, selector]) => selector.trim())).toEqual([
      '[data-scope="popover"][data-part="arrow"] svg',
    ]);
    expect(css).toContain('[data-scope="popover"][data-part="point"] {');
  });
});
