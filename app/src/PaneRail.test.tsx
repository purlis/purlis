// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PaneLines } from "./paneHead";
import { PaneRail } from "./PaneRail";

/** What the pane's model answers for a folded pane, while a test says otherwise. */
const answered: { lines?: PaneLines } = {};

vi.mock("./paneHead", async (original) => {
  const model = await original<typeof import("./paneHead")>();
  return {
    ...model,
    paneLinesOf: (...args: Parameters<typeof model.paneLinesOf>) =>
      answered.lines ?? model.paneLinesOf(...args),
  };
});

afterEach(() => {
  answered.lines = undefined;
  cleanup();
});

const ONE = {
  pane: 2,
  name: "devops 2",
  mark: <svg data-testid="mark" />,
  holds: { notes: 3, trouble: 0, asks: 0 },
  asks: true,
};

/**
 * **A folded pane is drawn from the pane's model** (#5366, `paneHead.paneLinesOf`): its rail's
 * button wears the persona's mark and says the name where the model's head has them, and its
 * notes count beside it where the model keeps the bell. Its ✋ is no part of either line: an ask
 * never hides (spec #4750 stories 81 to 83).
 */
describe("a folded pane's rail", () => {
  it("draws what the model answers for a folded pane: its mark, its name and its bell's count", () => {
    render(<PaneRail as="bar" onOpen={vi.fn()} panes={[ONE]} />);

    expect(screen.getByTestId("mark")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "devops 2, 3 notices, folded, asks you" }),
    ).toBeInTheDocument();
    expect(document.querySelector(".pane-rail-count")).toHaveTextContent("3");
  });

  it("leaves out what the model leaves out, and keeps the ask", () => {
    answered.lines = { head: ["name"], actions: [], foot: null };
    render(<PaneRail as="bar" onOpen={vi.fn()} panes={[ONE]} />);

    expect(screen.queryByTestId("mark")).toBeNull();
    expect(document.querySelector(".pane-rail-count")).toBeNull();
    expect(screen.getByRole("button", { name: "devops 2, folded, asks you" })).toBeInTheDocument();
    expect(document.querySelector(".pane-rail-asks")).not.toBeNull();
  });
});
