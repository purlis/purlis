// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { drawnWith } from "../cascade.testkit";
import { Field, SettingRow } from "./components";

/**
 * **A list field** (DS-10, #1842; D7 of the spec on #1832): one entry per line — the hosts a
 * persona reaches, the repos never listed — in a box as wide as its row, under its name, that
 * grows to show every line. jsdom lays nothing out, so the width is read through the window's
 * cascade and the growth through the heights a browser would report.
 */

afterEach(cleanup);

function Hosts({ start = "", minRows }: { start?: string; minRows?: number }) {
  const [value, setValue] = useState(start);
  return (
    <SettingRow
      label="Hosts"
      control={(ids) => (
        <Field ids={ids} kind="list" value={value} minRows={minRows} onChange={setValue} />
      )}
    />
  );
}

const box = () => screen.getByLabelText("Hosts") as HTMLTextAreaElement;

/** Lays the box out as a browser would: `needs` pixels of text in a box that shows `shows`,
 *  inside `borders` of border. */
function laidOut(element: HTMLElement, { needs, shows, borders = 2 }: Record<string, number>) {
  Object.defineProperty(element, "scrollHeight", { configurable: true, get: () => needs });
  Object.defineProperty(element, "clientHeight", { configurable: true, get: () => shows });
  Object.defineProperty(element, "offsetHeight", {
    configurable: true,
    get: () => shows + borders,
  });
}

describe("a list field", () => {
  it("is the shared field, drawn under its name at the row's width", () => {
    render(<Hosts />);

    expect(box()).toHaveClass("ui-input");
    expect(drawnWith(box(), "inline-size")).toBe("100%");
    // Its control takes a line of its own, under the label, rather than the space beside it.
    const control = box().closest(".ui-setting-control");
    expect(control).not.toBeNull();
    expect(drawnWith(control as Element, "flex-basis")).toBe("100%");
  });

  it("shows a line for every entry, and at least the rows it asks for", () => {
    render(<Hosts start={"a.example\nb.example\nc.example\nd.example\ne.example"} />);
    expect(box()).toHaveAttribute("rows", "5");
    cleanup();

    render(<Hosts start="a.example" minRows={3} />);
    expect(box()).toHaveAttribute("rows", "3");
  });

  it("grows a line as an entry is typed on a new one", () => {
    render(<Hosts start="a.example" />);

    fireEvent.change(box(), { target: { value: "a.example\nb.example\nc.example" } });

    expect(box()).toHaveAttribute("rows", "3");
  });

  it("grows past its rows to show a line that wraps, and never scrolls inside", () => {
    render(<Hosts start="a.example" />);
    laidOut(box(), { needs: 120, shows: 60 });

    fireEvent.change(box(), { target: { value: `a.example ${"very-long-host ".repeat(20)}` } });

    expect(box().style.blockSize).toBe("122px");
    expect(drawnWith(box(), "overflow-y")).toBe("hidden");
    expect(drawnWith(box(), "resize")).toBe("none");
  });

  it("sets no height of its own while its rows show every line", () => {
    render(<Hosts start="a.example" />);
    laidOut(box(), { needs: 60, shows: 60 });

    fireEvent.change(box(), { target: { value: "a.example\nb.example" } });

    expect(box().style.blockSize).toBe("");
  });
});
