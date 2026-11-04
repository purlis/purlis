// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { PaneFoot, PaneFootEnd, PaneFootGauge, PaneFootSeparator, PaneFootStart } from "./PaneFoot";

afterEach(cleanup);

/** **The pane foot family** (#5202): drawn by its parts, a gauge's bar a mark. */
describe("the pane foot", () => {
  it("is drawn by its parts: the root, its start and its end", () => {
    const { container } = render(
      <PaneFoot className="pane-foot">
        <PaneFootStart>Claude Code</PaneFootStart>
        <PaneFootEnd>gauge</PaneFootEnd>
      </PaneFoot>,
    );
    const root = container.querySelector('[data-scope="pane-foot"][data-part="root"]');
    expect(root).toHaveClass("pane-foot");
    expect([...(root?.children ?? [])].map((one) => one.getAttribute("data-part"))).toEqual([
      "start",
      "end",
    ]);
  });

  it("joins two parts with a · a reader is not given", () => {
    const { container } = render(<PaneFootSeparator />);
    const dot = container.querySelector('[data-part="separator"]');
    expect(dot).toHaveTextContent("·");
    expect(dot).toHaveAttribute("aria-hidden", "true");
  });

  it("draws a gauge as its label and value, and its fill as a bar under them in its tone", () => {
    const { container } = render(
      <PaneFootGauge label="ctx" value="62%" fill={{ percent: 62, tone: "warn" }} />,
    );
    const gauge = container.querySelector('[data-part="gauge"]') as HTMLElement;
    expect(gauge.textContent?.replace(/\s+/g, " ")).toBe("ctx 62%");
    expect(gauge).toHaveAttribute("data-barred");
    const fill = gauge.querySelector('[data-part="track"] > [data-part="fill"]') as HTMLElement;
    expect(fill).toHaveAttribute("data-tone", "warn");
    expect(fill.style.inlineSize).toBe("62%");
    expect(gauge.querySelector('[data-part="track"]')).toHaveAttribute("aria-hidden", "true");
  });

  it("keeps the bar on its track, whatever the value says", () => {
    const { container } = render(
      <>
        <PaneFootGauge label="ctx" value="130%" fill={{ percent: 130, tone: "bad" }} />
        <PaneFootGauge label="ctx" value="-4%" fill={{ percent: -4, tone: "ok" }} />
      </>,
    );
    const fills = [...container.querySelectorAll<HTMLElement>('[data-part="fill"]')];
    expect(fills.map((one) => one.style.inlineSize)).toEqual(["100%", "0%"]);
  });

  it("draws no bar for a gauge without a fill", () => {
    const { container } = render(<PaneFootGauge label="cache" value="91%" />);
    expect(container.querySelector('[data-part="track"]')).toBeNull();
    expect(container.querySelector('[data-part="gauge"]')).not.toHaveAttribute("data-barred");
  });
});
