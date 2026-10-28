// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Page tabs** (#3668): React Aria's tabs, each panel named by its tab, a count that describes a
 * tab and is no part of its name, automatic activation, and the chosen tab held by its owner.
 * That nothing moves is held by the stories in a real browser (`PageTabs.stories.tsx`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { countShown, PageTabs, type PageTab } from "./PageTabs";

afterEach(cleanup);

const TABS: PageTab[] = [
  { id: "overview", label: "Overview" },
  { id: "text", label: "Text" },
  { id: "history", label: "History", count: 4, countSaid: "4 versions" },
  { id: "grades", label: "Grades", count: 2, countSaid: "2 grades" },
  { id: "runs", label: "Runs", count: 41 },
];

function Skill({ start = "overview" }: { start?: string }) {
  const [tab, setTab] = useState(start);
  return (
    <>
      <p data-testid="kept">{tab}</p>
      <PageTabs label="deploy's sections" tabs={TABS} selected={tab} onSelect={setTab}>
        {(id) => <p>{`The ${id} of deploy`}</p>}
      </PageTabs>
    </>
  );
}

describe("page tabs", () => {
  it("are a tablist named for the page, each panel named by its tab", () => {
    render(<Skill />);
    expect(screen.getByRole("tablist", { name: "deploy's sections" })).toBeInTheDocument();
    expect(screen.getAllByRole("tab")).toHaveLength(5);
    const panel = screen.getByRole("tabpanel");
    expect(panel).toHaveAccessibleName("Overview");
    expect(panel).toHaveTextContent("The overview of deploy");
  });

  it("say a count after the name, as its description, never as part of it", () => {
    render(<Skill />);
    const history = screen.getByRole("tab", { name: "History" });
    expect(history).toHaveAccessibleDescription("4 versions");
    expect(screen.getByRole("tab", { name: "Runs" })).toHaveAccessibleDescription("41");
    expect(history.querySelector('[data-part="count"]')).toHaveTextContent("4");
    expect(screen.getByRole("tab", { name: "Text" })).not.toHaveAttribute("aria-describedby");
  });

  it("are chosen by the owner, who keeps the choice", () => {
    render(<Skill start="grades" />);
    expect(screen.getByRole("tab", { name: "Grades" })).toHaveAttribute("aria-selected", "true");
    act(() => fireEvent.click(screen.getByRole("tab", { name: "Runs" })));
    expect(screen.getByTestId("kept")).toHaveTextContent("runs");
    expect(screen.getByRole("tabpanel")).toHaveTextContent("The runs of deploy");
  });

  it("choose the tab the arrows move to (automatic activation)", () => {
    render(<Skill />);
    const overview = screen.getByRole("tab", { name: "Overview" });
    act(() => overview.focus());
    act(() => {
      fireEvent.keyDown(overview, { key: "ArrowRight" });
      fireEvent.keyUp(overview, { key: "ArrowRight" });
    });
    expect(screen.getByTestId("kept")).toHaveTextContent("text");
  });

  it("show a count past 99 as 99+, in the room three figures keep", () => {
    expect(countShown(9)).toBe("9");
    expect(countShown(99)).toBe("99");
    expect(countShown(100)).toBe("99+");
  });
});
