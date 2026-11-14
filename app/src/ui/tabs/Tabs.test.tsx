// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The session tabs' parts** (#2914): the attributes the family default and a theme's parts are
 * keyed on (ADR 0103). The look itself, and that no tab moves when the choice changes, are held
 * by the stories in a real browser (`Tabs.stories.tsx`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { pictureOf } from "../../layoutPicture";
import type { Layout } from "../../tabs";
import { editorToolbar, TabCell, TabPicture, tabPart, tabsPart } from "./Tabs";

afterEach(cleanup);

describe("a tab's cell", () => {
  it("is the family's tab, in its variant, saying whether it is chosen and first", () => {
    render(
      <>
        <TabCell variant="chat" selected first data-testid="one">
          <button role="tab" aria-selected>
            steward 1
          </button>
        </TabCell>
        <TabCell variant="file" selected={false} data-testid="two">
          <button role="tab" aria-selected={false}>
            purlis.toml
          </button>
        </TabCell>
      </>,
    );
    const one = screen.getByTestId("one");
    expect(one).toHaveAttribute("data-scope", "tabs");
    expect(one).toHaveAttribute("data-part", "tab");
    expect(one).toHaveAttribute("data-variant", "chat");
    expect(one).toHaveAttribute("data-selected", "true");
    expect(one).toHaveAttribute("data-first", "true");
    const two = screen.getByTestId("two");
    expect(two).toHaveAttribute("data-variant", "file");
    expect(two).not.toHaveAttribute("data-selected");
    expect(two).not.toHaveAttribute("data-first");
  });

  it("holds the theme's indicator, out of what a screen reader reads", () => {
    render(
      <TabCell variant="view" selected data-testid="cell">
        <button role="tab">Memory</button>
      </TabCell>,
    );
    const indicator = screen
      .getByTestId("cell")
      .querySelector('[data-scope="tabs"][data-part="indicator"]');
    expect(indicator).toHaveAttribute("aria-hidden", "true");
    expect(indicator?.textContent).toBe("");
  });

  it("is under the pointer for a mouse, never for a touch, and leaves it with the mouse", () => {
    render(
      <TabCell variant="chat" selected={false} data-testid="cell">
        <button role="tab">talk</button>
      </TabCell>,
    );
    const cell = screen.getByTestId("cell");
    fireEvent.pointerEnter(cell, { pointerType: "touch" });
    expect(cell).not.toHaveAttribute("data-hovered");
    fireEvent.pointerEnter(cell, { pointerType: "mouse" });
    expect(cell).toHaveAttribute("data-hovered", "true");
    fireEvent.pointerLeave(cell, { pointerType: "mouse" });
    expect(cell).not.toHaveAttribute("data-hovered");
  });

  it("hands its own pointer handlers on, and places by its class, never draws by it", () => {
    let entered = 0;
    render(
      <TabCell
        variant="chat"
        selected={false}
        className="tab"
        data-testid="cell"
        onPointerEnter={() => (entered += 1)}
      >
        <button role="tab">talk</button>
      </TabCell>,
    );
    const cell = screen.getByTestId("cell");
    fireEvent.pointerEnter(cell, { pointerType: "mouse" });
    expect(entered).toBe(1);
    expect(cell).toHaveClass("tab");
  });
});

describe("the parts around and inside the tabs", () => {
  it("name the family and the part, and the editor's toolbar names its own", () => {
    expect(tabsPart("layout")).toEqual({ "data-scope": "tabs", "data-part": "layout" });
    expect(tabsPart("strip")).toEqual({ "data-scope": "tabs", "data-part": "strip" });
    expect(tabPart("closer")).toEqual({ "data-scope": "tabs", "data-part": "closer" });
    expect(editorToolbar("piece-files-head")).toEqual({
      className: "ui-text-secondary piece-files-head",
      "data-scope": "editor",
      "data-part": "toolbar",
      "data-variant": "default",
    });
  });
});

describe("a split tab's picture", () => {
  const chat = (pane: number): Layout => ({
    kind: "pane",
    pane,
    content: { kind: "session", session: pane, chat: String(pane) },
  });
  const three: Layout = {
    kind: "split",
    direction: "row",
    children: [chat(1), { kind: "split", direction: "column", children: [chat(2), chat(3)] }],
  };

  it("is the family's picture part: a tile for each pane, parted by a gap (#5577)", () => {
    const picture = pictureOf(three);
    if (picture === undefined) throw new Error("three panes have a picture");
    const { container } = render(<TabPicture picture={picture} />);
    const drawn = container.querySelector('[data-scope="tabs"][data-part="picture"]');
    expect(drawn?.tagName.toLowerCase()).toBe("svg");
    // In the box from 2 to 14 of the 16 the icon is drawn on, 1.5 between tiles: the left half
    // whole, then the right half's two, one over the other.
    expect(
      [...(drawn?.querySelectorAll("rect") ?? [])].map((rect) =>
        ["x", "y", "width", "height"].map((at) => Number(rect.getAttribute(at))),
      ),
    ).toEqual([
      [2, 2, 5.25, 12],
      [8.75, 2, 5.25, 5.25],
      [8.75, 8.75, 5.25, 5.25],
    ]);
  });

  it("is out of what a screen reader reads: the tab's words say how many panes", () => {
    const picture = pictureOf(three);
    if (picture === undefined) throw new Error("three panes have a picture");
    const { container } = render(<TabPicture picture={picture} />);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });
});
