// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The meter family** (#3674): what a reader hears of a meter and a progress bar (its role, its
 * name, its value), the scale a group's meters share, a progress bar's value said politely at
 * most once a second, and the attributes a theme's parts are keyed on (ADR 0103). The look
 * itself is held by the stories (`Meter.stories.tsx`).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { drawnWith } from "../../cascade.testkit";
import { SAID_AT_MOST_EVERY } from "../said";
import { Meter, MeterGroup, ProgressBar } from "./Meter";

afterEach(cleanup);

/** The bar's fill, which is drawn as long as the value. */
const fillOf = (bar: Element) => bar.querySelector<HTMLElement>('[data-part="fill"]');

describe("Meter", () => {
  it("is a meter named by its label, its value said as its words", () => {
    render(<Meter label="With it" value={86} valueLabel="86%" />);
    const meter = screen.getByRole("meter", { name: "With it" });
    expect(meter).toHaveAttribute("aria-valuenow", "86");
    expect(meter).toHaveAttribute("aria-valuetext", "86%");
    expect(meter).toHaveAttribute("data-scope", "meter");
    expect(meter).toHaveAttribute("data-part", "root");
    expect(meter).toHaveAttribute("data-variant", "subject");
    expect(within(meter).getByText("86%")).toHaveAttribute("data-part", "value");
    expect(fillOf(meter)?.getAttribute("style")).toContain("inline-size: 86%");
  });

  it("draws each variant's fill in its own colour, the baseline hatched too", () => {
    render(
      <>
        <Meter label="With it" value={86} variant="subject" />
        <Meter label="Without it" value={68} variant="baseline" />
        <Meter label="Previous version" value={80} variant="previous" />
      </>,
    );
    const fill = (name: string) => fillOf(screen.getByRole("meter", { name })) as HTMLElement;
    expect(drawnWith(fill("With it"), "background-color")).toBe("var(--accent-base)");
    expect(drawnWith(fill("Without it"), "background-color")).toBe("var(--state-waiting)");
    expect(drawnWith(fill("Without it"), "background-image")).toMatch(/repeating-linear-gradient/);
    expect(drawnWith(fill("Previous version"), "background-color")).toBe("var(--text-muted)");
  });
});

describe("MeterGroup", () => {
  it("is a named group whose meters share one scale, so their lengths compare", () => {
    render(
      <MeterGroup aria-label="Latest grade" maxValue={200}>
        <Meter label="With it" value={100} />
        <Meter label="Without it" value={50} variant="baseline" />
      </MeterGroup>,
    );
    const group = screen.getByRole("group", { name: "Latest grade" });
    const [withIt, without] = within(group).getAllByRole("meter");
    expect(withIt).toHaveAttribute("aria-valuemax", "200");
    expect(fillOf(withIt)?.getAttribute("style")).toContain("inline-size: 50%");
    expect(fillOf(without)?.getAttribute("style")).toContain("inline-size: 25%");
    expect(withIt).toHaveAttribute("data-grouped");
  });
});

describe("ProgressBar", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("is a progress bar of done of N, named by its label", () => {
    render(<ProgressBar label="Chats" value={3} maxValue={12} valueLabel="3 of 12" />);
    const bar = screen.getByRole("progressbar", { name: "Chats" });
    expect(bar).toHaveAttribute("aria-valuenow", "3");
    expect(bar).toHaveAttribute("aria-valuetext", "3 of 12");
    expect(fillOf(bar)?.getAttribute("style")).toContain("inline-size: 25%");
  });

  it("says its value politely as it changes, at most once a second, and only the last", () => {
    const { rerender } = render(
      <ProgressBar label="Chats" value={1} maxValue={12} valueLabel="1 of 12" />,
    );
    const status = screen.getByRole("status");
    // What it shows as it is drawn is no change.
    expect(status).toHaveTextContent("");
    rerender(<ProgressBar label="Chats" value={2} maxValue={12} valueLabel="2 of 12" />);
    act(() => vi.advanceTimersByTime(0));
    expect(status).toHaveTextContent("2 of 12");
    rerender(<ProgressBar label="Chats" value={3} maxValue={12} valueLabel="3 of 12" />);
    rerender(<ProgressBar label="Chats" value={4} maxValue={12} valueLabel="4 of 12" />);
    act(() => vi.advanceTimersByTime(SAID_AT_MOST_EVERY / 2));
    expect(status).toHaveTextContent("2 of 12");
    act(() => vi.advanceTimersByTime(SAID_AT_MOST_EVERY / 2));
    expect(status).toHaveTextContent("4 of 12");
  });

  it("says no value while it is going with no end known, and draws its stripes", () => {
    render(<ProgressBar label="Starting the chats" isIndeterminate />);
    const bar = screen.getByRole("progressbar", { name: "Starting the chats" });
    expect(bar).not.toHaveAttribute("aria-valuenow");
    expect(bar).toHaveAttribute("data-indeterminate");
    expect(drawnWith(fillOf(bar) as HTMLElement, "animation")).toMatch(/meter-going/);
  });
});
