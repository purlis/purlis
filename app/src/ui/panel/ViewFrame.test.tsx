// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The view frame** (#3666): the page navigation's one Tab stop and its keys, the current page
 * said by more than its colour, a page's count, the item beside the list, and page-and-back when
 * narrow. How it is drawn, and that nothing moves, are held by its stories in a real browser
 * (`ViewFrame.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { HEADER_SIZE, ViewFrame, ViewListHead, ViewPageHeader, type ViewPage } from "./ViewFrame";

afterEach(cleanup);

const PAGES: ViewPage[] = [
  { id: "library", label: "Library" },
  { id: "needs", label: "Needs you", count: 2, waiting: true, countSaid: "2 need you" },
  { id: "drafts", label: "Drafts", count: 1 },
  { id: "sources", label: "Sources" },
  { id: "harness", label: "Harness skills", startsGroup: true },
  { id: "routing", label: "Routing" },
];

/** A frame whose owner holds the page and the item, as a view would. */
function Studio({
  narrow,
  onPage = () => undefined,
  withItem = true,
}: {
  narrow?: boolean;
  onPage?: (id: string) => void;
  withItem?: boolean;
}) {
  const [page, setPage] = useState("library");
  const [opened, setOpened] = useState(false);
  return (
    <ViewFrame
      label="Skill studio pages"
      pages={PAGES}
      page={page}
      onPage={(id) => {
        onPage(id);
        setPage(id);
      }}
      narrow={narrow}
      opened={opened}
      onBack={() => setOpened(false)}
      list={
        <>
          <ViewListHead title="Library" lead="Every skill the project's chats can use." />
          <button type="button" onClick={() => setOpened(true)}>
            deploy
          </button>
        </>
      }
      item={
        withItem ? (
          <ViewPageHeader
            name="deploy"
            description="Deploys a service to staging or production."
            actions={<button type="button">Edit</button>}
          />
        ) : undefined
      }
    />
  );
}

const nav = () => screen.getByRole("navigation", { name: "Skill studio pages" });
const page = (name: RegExp | string) => screen.getByRole("button", { name });

describe("the page navigation", () => {
  it("names every page, the current one aria-current, with its count after its name", () => {
    render(<Studio />);
    expect(nav()).toBeInTheDocument();
    expect(page("Library")).toHaveAttribute("aria-current", "page");
    expect(page(/^Needs you/)).not.toHaveAttribute("aria-current");
    // The current page is marked by more than a colour: its state and its mark.
    expect(page("Library")).toHaveAttribute("data-selected", "true");
    expect(
      page("Library").querySelector('[data-scope="view-nav"][data-part="indicator"]'),
    ).toHaveAttribute("aria-hidden", "true");
    // A waiting page's count is in the waiting tone, and heard as its sentence.
    const count = page(/^Needs you/).querySelector('[data-scope="count"]');
    expect(count).toHaveAttribute("data-variant", "waiting");
    expect(page(/^Needs you/)).toHaveAccessibleName("Needs you 2 need you");
    expect(page(/^Drafts/).querySelector('[data-scope="count"]')).toHaveAttribute(
      "data-variant",
      "neutral",
    );
    expect(page("Harness skills")).toHaveAttribute("data-starts-group", "true");
  });

  it("is one Tab stop, the current page, walked by the arrows, Home and End", () => {
    render(<Studio />);
    const stops = [...nav().querySelectorAll("button")].filter((one) => one.tabIndex === 0);
    expect(stops).toEqual([page("Library")]);
    page("Library").focus();
    fireEvent.keyDown(page("Library"), { key: "ArrowDown" });
    expect(page(/^Needs you/)).toHaveFocus();
    // The keyboard's place is the stop while it is in the navigation; nothing is chosen yet.
    expect(page(/^Needs you/).tabIndex).toBe(0);
    expect(page("Library")).toHaveAttribute("aria-current", "page");
    fireEvent.keyDown(page(/^Needs you/), { key: "End" });
    expect(page("Routing")).toHaveFocus();
    fireEvent.keyDown(page("Routing"), { key: "ArrowDown" });
    expect(page("Routing")).toHaveFocus();
    fireEvent.keyDown(page("Routing"), { key: "Home" });
    expect(page("Library")).toHaveFocus();
    fireEvent.keyDown(page("Library"), { key: "ArrowUp" });
    expect(page("Library")).toHaveFocus();
  });

  it("chooses a page on a press, which takes the stop and the mark", () => {
    const chosen = vi.fn();
    render(<Studio onPage={chosen} />);
    fireEvent.click(page("Sources"));
    expect(chosen).toHaveBeenCalledWith("sources");
    expect(page("Sources")).toHaveAttribute("aria-current", "page");
    expect(page("Library")).not.toHaveAttribute("aria-current");
  });

  it("gives the stop back to the current page as the keyboard leaves it", () => {
    render(<Studio />);
    page("Library").focus();
    fireEvent.keyDown(page("Library"), { key: "ArrowDown" });
    fireEvent.blur(page(/^Needs you/), { relatedTarget: document.body });
    expect(page("Library").tabIndex).toBe(0);
    expect(page(/^Needs you/).tabIndex).toBe(-1);
  });
});

describe("the panes", () => {
  it("draw the list beside the item, a separator between them that resizes them", () => {
    render(<Studio />);
    expect(screen.getByRole("region", { name: "Library" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Library" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "deploy" })).toBeInTheDocument();
    expect(
      screen.getByRole("separator", { name: "Resize the list and the item beside it" }),
    ).toBeInTheDocument();
  });

  it("give the list the room on a page with no item beside it", () => {
    render(<Studio withItem={false} />);
    expect(screen.queryByRole("separator")).toBeNull();
    expect(screen.queryByRole("heading", { name: "deploy" })).toBeNull();
  });
});

describe("narrow: page-and-back", () => {
  it("draws the navigation as a picker and the list first", () => {
    render(<Studio narrow />);
    expect(screen.queryByRole("navigation")).toBeNull();
    expect(screen.getByRole("button", { name: /Skill studio pages/ })).toHaveTextContent("Library");
    expect(screen.getByRole("region", { name: "Library" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "deploy" })).toBeNull();
  });

  it("draws an opened item alone with a way back, and comes back to the list", () => {
    render(<Studio narrow />);
    act(() => fireEvent.click(screen.getByRole("button", { name: "deploy" })));
    expect(screen.queryByRole("region", { name: "Library" })).toBeNull();
    expect(screen.getByRole("heading", { name: "deploy" })).toBeInTheDocument();
    const back = screen.getByRole("button", { name: "Back to Library" });
    expect(back).toHaveFocus();
    act(() => fireEvent.click(back));
    expect(screen.queryByRole("heading", { name: "deploy" })).toBeNull();
    expect(screen.getByRole("region", { name: "Library" })).toHaveFocus();
  });
});

describe("the page header over the item's body (#3944)", () => {
  it("tells the item's pane how tall it is, so the keyboard scrolls clear of it", () => {
    // jsdom lays nothing out: the header is 96.4px tall here, as a browser would measure it.
    const tall = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockImplementation(function (this: HTMLElement) {
        const height = this.getAttribute("data-part") === "header" ? 96.4 : 0;
        return new DOMRect(0, 0, 0, height);
      });
    try {
      const { unmount } = render(<Studio />);
      const header = screen.getByRole("heading", { name: "deploy" });
      const pane = header.closest('[data-scope="view-frame"][data-part="item"]') as HTMLElement;
      // Its exact height, never rounded down under it.
      expect(pane.style.getPropertyValue(HEADER_SIZE)).toBe("96.4px");
      // Gone with the header: a pane without one keeps no room for it.
      unmount();
      expect(pane.style.getPropertyValue(HEADER_SIZE)).toBe("");
    } finally {
      tall.mockRestore();
    }
  });
});
