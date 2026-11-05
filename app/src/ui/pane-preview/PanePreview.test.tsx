// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import {
  PanePreviewArriving,
  PanePreviewHeld,
  PanePreviewRefused,
  PanePreviewSize,
  PanePreviewWords,
} from "./PanePreview";

afterEach(cleanup);

const BOX = { left: 10, top: 20, width: 300, height: 200 };
const partOf = (container: HTMLElement, part: string) =>
  container.querySelector(`[data-scope="pane-preview"][data-part="${part}"]`) as HTMLElement;

/** **What a drag draws over the panes** (#5291): drawn by its parts, and read by nobody. */
describe("the pane preview", () => {
  it("draws a pane about to be at its box, with its name", () => {
    const { container } = render(<PanePreviewArriving box={BOX} name="codeql-issues" />);
    const arriving = partOf(container, "arriving");
    expect(arriving.style.left).toBe("10px");
    expect(arriving.style.height).toBe("200px");
    expect(partOf(container, "name")).toHaveTextContent("codeql-issues");
    expect(arriving).toHaveAttribute("aria-hidden", "true");
  });

  it("says a terminal's size in columns and rows, from its pane's top corner", () => {
    const { container } = render(<PanePreviewSize box={BOX} columns={80} rows={24} />);
    const size = partOf(container, "size");
    expect(size).toHaveTextContent("80 × 24");
    expect(size.style.left).toBe("310px");
    expect(size.style.top).toBe("20px");
  });

  it("marks a refused place's pane at its box", () => {
    const { container } = render(<PanePreviewRefused box={BOX} />);
    expect(partOf(container, "refused").style.width).toBe("300px");
  });

  it("says what is held and what letting go would do at the pointer, a refusal marked", () => {
    const { container } = render(
      <>
        <PanePreviewHeld at={{ left: 1, top: 2 }}>codeql-issues</PanePreviewHeld>
        <PanePreviewWords at={{ left: 3, top: 4 }} refused>
          Too narrow.
        </PanePreviewWords>
      </>,
    );
    expect(partOf(container, "held").style.left).toBe("1px");
    expect(partOf(container, "words")).toHaveAttribute("data-refused");
  });
});
