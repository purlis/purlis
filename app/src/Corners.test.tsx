// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The four corners** (#2917, spec #2897 R116–R128; #3144): the left bar a column beside the
 * project rail, the right bar's column, the right panel and the bottom halves, each opened by its
 * corner's icons, and only the part that just opened moving.
 */
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { RegionFrame } from "./RegionFrame";
import { DEFAULT_ARRANGEMENT, picked, type Arrangement, type ViewId } from "./regions";

afterEach(cleanup);

const VIEWS: Partial<Record<ViewId, React.ReactNode>> = {
  chats: <div data-testid="v-chats">chats</div>,
  explorer: <div data-testid="v-explorer">explorer</div>,
  search: <div data-testid="v-search">search</div>,
  changes: <div data-testid="v-changes">changes</div>,
  repos: <div data-testid="v-repos">repos</div>,
  todos: <div data-testid="v-todos">todos</div>,
  memory: <div data-testid="v-memory">memory</div>,
  inbox: <div data-testid="v-inbox">inbox</div>,
  personas: <div data-testid="v-personas">personas</div>,
  sessions: <div data-testid="v-sessions">sessions</div>,
  vaults: <div data-testid="v-vaults">vaults</div>,
};

/** The frame answering its own presses as the window does (`regions.picked`). */
function Window({ from }: { from: Arrangement }) {
  const [arrangement, setArrangement] = useState(from);
  return (
    <RegionFrame
      arrangement={arrangement}
      content={{}}
      views={VIEWS}
      heads={{ chats: null, explorer: "smart-ide" }}
      onPick={(view) => setArrangement((was) => picked(was, view))}
      centre={<div data-testid="centre">centre</div>}
      onResized={vi.fn()}
    />
  );
}

const closed = (...ids: string[]): Arrangement =>
  DEFAULT_ARRANGEMENT.map((one) => (ids.includes(one.id) ? { ...one, collapsed: true } : one));

const tabIn = (list: string, name: string) =>
  within(screen.getByRole("tablist", { name: list })).getByRole("tab", { name });
const slot = (side: string) => document.querySelector(`.slot-${side}`) as HTMLElement;
const half = (side: string) =>
  document.querySelector(`.panel-half[data-side="${side}"]`) as HTMLElement;

const leftColumn = () =>
  document.querySelector(".activity-column[data-side='left']") as HTMLElement | null;

describe("the left corner (#3144, R121–R123)", () => {
  it("is a column at the frame's start, beside the rail: Chats, Explorer, Search, then Repos and Changes at its foot", () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);
    const column = leftColumn();
    expect(column).not.toBeNull();
    // The frame's first column (the rail is the window's, before the frame), before the regions.
    expect(document.querySelector(".region-frame")?.firstElementChild).toBe(column);
    expect(column).toHaveAttribute("data-variant", "bar");
    const order = [...(column?.querySelector('[data-part="items"]')?.children ?? [])].map((one) =>
      one.getAttribute("data-part") === "gap" ? "gap" : one.getAttribute("data-region"),
    );
    expect(order).toEqual(["navigation", "gap", "panel-left"]);
    const names = [...(column as HTMLElement).querySelectorAll('[role="tab"]')].map((one) =>
      one.getAttribute("aria-label"),
    );
    expect(names).toEqual(["Chats", "Explorer", "Search", "Repos", "Changes"]);
    for (const item of (column as HTMLElement).querySelectorAll('[data-part="item"]'))
      expect(item).toHaveAttribute("data-variant", "bar");
    // A column, walked by the up and down arrows.
    expect(screen.getByRole("tablist", { name: "Navigation" })).toHaveAttribute(
      "aria-orientation",
      "vertical",
    );
  });

  it("draws no tabs inside the side panel", () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);
    expect(slot("left").querySelector('[data-scope="activity-bar"]')).toBeNull();
    expect(slot("left").querySelector('[data-variant="tab"]')).toBeNull();
  });

  it("marks the open view's icon, and the side panel shows that view", async () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);
    expect(tabIn("Navigation", "Chats")).toHaveAttribute("data-selected", "true");
    await userEvent.click(tabIn("Navigation", "Search"));
    expect(tabIn("Navigation", "Search")).toHaveAttribute("data-selected", "true");
    expect(tabIn("Navigation", "Chats")).not.toHaveAttribute("data-selected");
    expect(within(slot("left")).getByRole("tabpanel", { name: "Search" })).toBeInTheDocument();
  });

  it("switches the view, and a press of the open one does nothing", async () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);

    await userEvent.click(tabIn("Navigation", "Explorer"));
    expect(screen.getByRole("tabpanel", { name: "Explorer" })).toBeInTheDocument();
    expect(tabIn("Navigation", "Explorer")).toHaveAttribute("data-selected", "true");

    await userEvent.click(tabIn("Navigation", "Explorer"));
    expect(screen.getByRole("tabpanel", { name: "Explorer" })).toBeInTheDocument();
  });

  it("heads the side panel with the open view's words, and lets Chats draw its own", async () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);
    expect(slot("left").querySelector(".side-head")).toBeNull();

    await userEvent.click(tabIn("Navigation", "Explorer"));
    expect(within(slot("left")).getByRole("heading", { name: "smart-ide" })).toBeInTheDocument();

    await userEvent.click(tabIn("Navigation", "Search"));
    expect(within(slot("left")).getByRole("heading", { name: "Search" })).toBeInTheDocument();
  });
});

describe("the right corner (R117, R124, R125)", () => {
  it("is a column: Inbox, Personas, Vaults, then Memory, Todos, Sessions at its foot", () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);
    const column = document.querySelector(".activity-column[data-side='right']") as HTMLElement;
    expect(column).toHaveAttribute("data-variant", "bar");
    const names = [...column.querySelectorAll('[role="tab"]')].map((one) =>
      one.getAttribute("aria-label"),
    );
    expect(names).toEqual(["Inbox", "Personas", "Vaults", "Memory", "Todos", "Sessions"]);
  });

  it("toggles the right panel from its open icon, and its × closes it", async () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);
    expect(within(slot("right")).getByRole("heading", { name: "Inbox" })).toBeInTheDocument();

    await userEvent.click(within(slot("right")).getByRole("button", { name: "Close Inbox" }));
    expect(screen.queryByRole("tabpanel", { name: "Inbox" })).toBeNull();

    await userEvent.click(tabIn("Attention", "Personas"));
    expect(screen.getByRole("tabpanel", { name: "Personas" })).toBeInTheDocument();
    await userEvent.click(tabIn("Attention", "Personas"));
    expect(screen.queryByRole("tabpanel", { name: "Personas" })).toBeNull();
  });
});

describe("the bottom halves (R126, R127)", () => {
  it("opens each half from its side's bottom icons, and its × closes it", async () => {
    render(<Window from={closed("panel-left", "panel-right")} />);
    expect(half("left")).toHaveAttribute("hidden");

    await userEvent.click(tabIn("Panel, left half", "Changes"));
    expect(half("left")).not.toHaveAttribute("hidden");
    expect(within(half("left")).getByRole("heading", { name: "Changes" })).toBeInTheDocument();

    await userEvent.click(
      within(half("left")).getByRole("button", { name: "Put the panel's left half away" }),
    );
    expect(half("left")).toHaveAttribute("hidden");
  });
});

describe("motion: only the part that opened (R122, R124, R127, R128, R151)", () => {
  it("draws nothing arriving at launch", () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);
    expect(slot("right")).not.toHaveClass("arrived");
    expect(half("left")).not.toHaveClass("arrived");
    expect(half("right")).not.toHaveClass("arrived");
    expect(document.querySelector("[data-swapped]")).toBeNull();
  });

  it("brings the right panel in only when it was closed, and never on a switch of its view", async () => {
    render(<Window from={closed("aside")} />);

    await userEvent.click(tabIn("Attention", "Inbox"));
    expect(slot("right")).toHaveClass("arrived");
    fireEvent.animationEnd(slot("right"));
    expect(slot("right")).not.toHaveClass("arrived");

    // Another of its views, while it is open: it does not arrive again.
    await userEvent.click(tabIn("Attention", "Personas"));
    expect(screen.getByRole("tabpanel", { name: "Personas" })).toBeInTheDocument();
    expect(slot("right")).not.toHaveClass("arrived");

    // Closing has no motion: it is gone.
    await userEvent.click(tabIn("Attention", "Personas"));
    expect(slot("right")).not.toHaveClass("arrived");
  });

  it("brings in the half that opened and never replays the other", async () => {
    render(<Window from={closed("panel-right")} />);
    expect(half("left")).not.toHaveClass("arrived");

    await userEvent.click(tabIn("Panel, right half", "Todos"));

    expect(half("right")).toHaveClass("arrived");
    expect(half("left")).not.toHaveClass("arrived");
  });

  it("fades the side view's new list in once, and a list shown again does not replay", async () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);

    await userEvent.click(tabIn("Navigation", "Explorer"));
    const explorer = screen.getByRole("tabpanel", { name: "Explorer" });
    expect(explorer).toHaveAttribute("data-swapped");
    expect(document.querySelectorAll("[data-swapped]")).toHaveLength(1);

    // Its animation ends: the mark goes, and a re-render does not bring it back.
    fireEvent.animationEnd(explorer);
    expect(explorer).not.toHaveAttribute("data-swapped");

    await userEvent.click(tabIn("Navigation", "Chats"));
    expect(explorer).not.toHaveAttribute("data-swapped");
    expect(screen.getByRole("tabpanel", { name: "Chats" })).toHaveAttribute("data-swapped");
  });
});

describe("the keyboard and the side put away (S4, S7, D-2917-18)", () => {
  it("gives the keyboard back to the icon that opened the right panel when its × closes it", async () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);

    await userEvent.click(within(slot("right")).getByRole("button", { name: "Close Inbox" }));

    expect(tabIn("Attention", "Inbox")).toHaveFocus();
  });

  it("gives the keyboard back to the icon that opened a half when its × closes it", async () => {
    render(<Window from={DEFAULT_ARRANGEMENT} />);

    await userEvent.click(
      within(half("right")).getByRole("button", { name: "Put the panel's right half away" }),
    );

    expect(half("right")).toHaveAttribute("hidden");
    expect(tabIn("Panel, right half", "Memory")).toHaveFocus();
  });

  it("lets a half's arrival end, so it is never drawn arriving again", async () => {
    render(<Window from={closed("panel-right")} />);
    await userEvent.click(tabIn("Panel, right half", "Todos"));
    expect(half("right")).toHaveClass("arrived");

    fireEvent.animationEnd(half("right"));
    expect(half("right")).not.toHaveClass("arrived");

    // Taken out of the document and put back, as a project brought in front again is, it
    // stays put: nothing marks it arriving, so its animation has nothing to replay.
    const box = half("right");
    const parent = box.parentElement as HTMLElement;
    parent.removeChild(box);
    parent.appendChild(box);
    expect(half("right")).not.toHaveClass("arrived");
  });

  it("lets the right panel's arrival end, heard on the box that animates", async () => {
    render(<Window from={closed("aside")} />);
    await userEvent.click(tabIn("Attention", "Inbox"));
    expect(slot("right")).toHaveClass("arrived");

    // An animation inside the panel that ends is not the panel's.
    const inner = slot("right").querySelector('[role="tabpanel"]') as HTMLElement;
    fireEvent.animationEnd(inner);
    expect(slot("right")).toHaveClass("arrived");

    fireEvent.animationEnd(slot("right"));
    expect(slot("right")).not.toHaveClass("arrived");
  });

  it("keeps the same column while the side panel is away and when it comes back (D-3144-2)", async () => {
    render(<Window from={closed("navigation")} />);
    const column = leftColumn() as HTMLElement;
    expect(column).not.toBeNull();
    expect(
      [...column.querySelectorAll('[role="tab"]')].map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Chats", "Explorer", "Search", "Repos", "Changes"]);
    // Away, no view of the side is open, so none of its icons is marked.
    expect(
      within(screen.getByRole("tablist", { name: "Navigation" })).queryAllByRole("tab", {
        selected: true,
      }),
    ).toEqual([]);

    await userEvent.click(within(column).getByRole("tab", { name: "Explorer" }));

    expect(screen.getByRole("tabpanel", { name: "Explorer" })).toBeInTheDocument();
    // The very same column, still first: nothing beside it was drawn again or moved.
    expect(leftColumn()).toBe(column);
    expect(document.querySelector(".region-frame")?.firstElementChild).toBe(column);
    expect(slot("left").querySelector('[data-scope="activity-bar"]')).toBeNull();
  });
});
