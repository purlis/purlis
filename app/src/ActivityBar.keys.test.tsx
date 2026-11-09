// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { ActivityBar, ActivityCount, VIEW_MARKS, type Tabbed } from "./ActivityBar";
import type { ViewId } from "./regions";

/**
 * **An activity bar's keyboard, focus and names** (#3134): what a person on the keyboard or a
 * screen reader gets from a side's bar, held across its move onto the tabs family. A column
 * (`bar`) is walked by Up and Down, a row of tabs (`tab`) by Left and Right; Home and End go to
 * either end and the walk wraps there, as the WAI-ARIA tabs pattern does; moving opens nothing
 * and Enter or Space presses; the bar is one Tab stop, the open view's tab, and coming back in
 * lands on it again.
 */

const VIEWS: Tabbed[] = [
  { view: "chats", name: "Chats", mark: VIEW_MARKS.chats },
  { view: "explorer", name: "Explorer", mark: VIEW_MARKS.explorer },
  { view: "search", name: "Search", mark: VIEW_MARKS.search },
];

afterEach(cleanup);

function Bar({
  variant = "bar",
  open = "explorer",
  onPick = () => {},
}: {
  variant?: "bar" | "tab";
  /** The open view, or `null` while the side is put away. */
  open?: ViewId | null;
  onPick?: (view: ViewId) => void;
}) {
  return (
    <>
      <button type="button">before</button>
      <ActivityBar
        side="left"
        variant={variant}
        name="Navigation"
        views={VIEWS}
        open={open ?? undefined}
        current={open ?? "explorer"}
        badges={{ search: <ActivityCount count={3} said="3 results" /> }}
        panelOf={(view) => `panel-${view}`}
        tabOf={(view) => `tab-${view}`}
        onPick={onPick}
      />
      <button type="button">after</button>
    </>
  );
}

const bar = () => screen.getByRole("tablist", { name: "Navigation" });
const tab = (name: string) => screen.getByRole("tab", { name });

describe("an activity bar's keyboard (#3134)", () => {
  it("is one tablist named for its region, its tabs the views in order", () => {
    render(<Bar />);
    expect(bar()).toHaveAttribute("aria-orientation", "vertical");
    expect(
      within(document.body)
        .getAllByRole("tab")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Chats", "Explorer", "Search"]);
  });

  it("says which tab is open, what each controls, whether its region is out, and its badge", () => {
    render(<Bar />);
    expect(tab("Explorer")).toHaveAttribute("aria-selected", "true");
    expect(tab("Chats")).toHaveAttribute("aria-selected", "false");
    expect(tab("Explorer")).toHaveAttribute("aria-controls", "panel-explorer");
    expect(tab("Explorer")).toHaveAttribute("id", "tab-explorer");
    expect(tab("Explorer")).toHaveAttribute("aria-expanded", "true");
    expect(tab("Chats")).not.toHaveAttribute("aria-expanded");
    expect(tab("Search")).toHaveAccessibleDescription("3 results");
  });

  it("is one Tab stop, on the open view's tab", async () => {
    render(<Bar />);
    await userEvent.click(screen.getByRole("button", { name: "before" }));
    await userEvent.tab();
    expect(tab("Explorer")).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "after" })).toHaveFocus();
  });

  it("is a stop on the first tab while its side is put away", async () => {
    render(<Bar open={null} />);
    await userEvent.click(screen.getByRole("button", { name: "before" }));
    await userEvent.tab();
    expect(tab("Chats")).toHaveFocus();
  });

  it("walks a column with Up and Down, Home and End, and opens nothing", async () => {
    const onPick = vi.fn();
    render(<Bar onPick={onPick} />);
    await userEvent.click(screen.getByRole("button", { name: "before" }));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    expect(tab("Search")).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}{ArrowUp}");
    expect(tab("Chats")).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(tab("Search")).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(tab("Chats")).toHaveFocus();
    expect(onPick).not.toHaveBeenCalled();
  });

  // Radix's roving focus stopped at either end; the tabs family's (React Aria's) wraps, as the
  // WAI-ARIA tabs pattern does (#3134).
  it("wraps at its ends, as the tabs pattern does", async () => {
    render(<Bar />);
    await userEvent.click(screen.getByRole("button", { name: "before" }));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    expect(tab("Chats")).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}");
    expect(tab("Search")).toHaveFocus();
  });

  it("walks a row of tabs with Left and Right", async () => {
    render(<Bar variant="tab" />);
    expect(bar()).toHaveAttribute("aria-orientation", "horizontal");
    await userEvent.click(screen.getByRole("button", { name: "before" }));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowRight}");
    expect(tab("Search")).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}{ArrowLeft}");
    expect(tab("Chats")).toHaveFocus();
  });

  it("presses the tab the keyboard is on with Enter or Space", async () => {
    const onPick = vi.fn();
    render(<Bar onPick={onPick} />);
    await userEvent.click(screen.getByRole("button", { name: "before" }));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await userEvent.keyboard("{Home} ");
    expect(onPick.mock.calls).toEqual([["search"], ["chats"]]);
  });

  it("comes back in on the open view's tab after the keyboard walked and left", async () => {
    render(<Bar />);
    await userEvent.click(screen.getByRole("button", { name: "before" }));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "after" })).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(tab("Explorer")).toHaveFocus();
  });
});
