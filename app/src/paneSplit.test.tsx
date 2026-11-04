// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Group, Panel } from "react-resizable-panels";
import { EvenedSeparator } from "./LayoutPanes";

/**
 * **A split's separator, on the keyboard and the pointer** (#4763): the arrows resize it by 5%,
 * and a double-click or Enter evens it. Against the real `react-resizable-panels`, with each
 * element given a size, since jsdom lays nothing out.
 */

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(1000);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(
    () =>
      ({
        left: 0,
        top: 0,
        right: 1000,
        bottom: 600,
        width: 1000,
        height: 600,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect,
  );
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function Split({ heard, evened }: { heard: (first: number) => void; evened: () => void }) {
  return (
    <Group
      orientation="horizontal"
      onLayoutChanged={(sizes, meta) => {
        if (meta.isUserInteraction) heard(Math.round(sizes["pane-a"]));
      }}
    >
      <Panel id="pane-a" defaultSize="70%">
        a
      </Panel>
      <EvenedSeparator sides={["pane-a", "pane-b"]} onEven={evened} />
      <Panel id="pane-b" defaultSize="30%">
        b
      </Panel>
    </Group>
  );
}

describe("a split's separator (#4763)", () => {
  it("resizes by 5% for each arrow", async () => {
    const heard = vi.fn();
    render(<Split heard={heard} evened={() => undefined} />);
    const handle = screen.getByRole("separator", { name: "Resize the split" });
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    await waitFor(() => expect(heard).toHaveBeenLastCalledWith(65));
    fireEvent.keyDown(handle, { key: "ArrowRight" });
    await waitFor(() => expect(heard).toHaveBeenLastCalledWith(70));
  });

  it("evens on a double-click", async () => {
    const evened = vi.fn();
    render(<Split heard={() => undefined} evened={evened} />);
    fireEvent.doubleClick(screen.getByRole("separator", { name: "Resize the split" }));
    expect(evened).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(screen.getByRole("separator", { name: "Resize the split" })).toHaveAttribute(
        "aria-valuenow",
        "50",
      ),
    );
  });

  it("evens on Enter", async () => {
    const evened = vi.fn();
    render(<Split heard={() => undefined} evened={evened} />);
    fireEvent.keyDown(screen.getByRole("separator", { name: "Resize the split" }), {
      key: "Enter",
    });
    expect(evened).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(screen.getByRole("separator", { name: "Resize the split" })).toHaveAttribute(
        "aria-valuenow",
        "50",
      ),
    );
  });
});
