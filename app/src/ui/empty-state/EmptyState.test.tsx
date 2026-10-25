// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Inbox } from "lucide-react";
import { EmptyState } from "./EmptyState";
import { drawnWith } from "../../cascade.testkit";
import { Button } from "../button/Button";

/**
 * **The one empty state** (DS-3 #626): what is missing, what goes there, and the way out where a
 * surface has one. Every view tab draws its empty state through it (`emptyStates.guard.test.ts`).
 */

afterEach(cleanup);

describe("EmptyState", () => {
  it("says what is missing, what goes there, and offers the way out", async () => {
    const add = vi.fn();
    render(
      <EmptyState
        mark={Inbox}
        headline="No saves recorded yet"
        body="Each save is listed here."
        action={
          <button type="button" onClick={add}>
            Save
          </button>
        }
        testid="empty"
      />,
    );

    const empty = screen.getByTestId("empty");
    expect(empty).toHaveAttribute("data-scope", "empty-state");
    expect(empty).toHaveAttribute("data-part", "root");
    expect(empty).toHaveAttribute("data-variant", "page");
    expect(screen.getByText("No saves recorded yet")).toHaveAttribute("data-part", "headline");
    expect(screen.getByText("Each save is listed here.")).toHaveAttribute("data-part", "body");
    expect(empty.querySelector('[data-part="icon"] > svg')).not.toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(add).toHaveBeenCalledTimes(1);
    // Not a live region unless the caller says it is one.
    expect(empty).not.toHaveAttribute("role");
  });

  it("draws only the headline where there is nothing more to say, at a panel's size", () => {
    render(<EmptyState headline="Nothing archived" body="" variant="panel" testid="empty" />);

    const empty = screen.getByTestId("empty");
    expect(empty).toHaveAttribute("data-variant", "panel");
    expect(empty.querySelector('[data-part="body"]')).toBeNull();
    expect(empty.querySelector('[data-part="action"]')).toBeNull();
    expect(empty.querySelector('[data-part="icon"]')).toBeNull();
  });

  it("is said to a screen reader as it changes where the caller asks", () => {
    render(<EmptyState headline="Reading" role="status" />);

    expect(screen.getByRole("status")).toHaveTextContent("Reading");
  });
});

/** **An empty state's way out is the shared primary button** (DS-12), wherever a surface draws
 *  one: the one thing to do here, one control tall. */
describe("an empty state's way out", () => {
  it("is the shared primary button, one control tall", () => {
    render(
      <EmptyState
        headline="No chats yet"
        action={<Button variant="primary">Open the first chat</Button>}
      />,
    );
    const way = screen.getByRole("button", { name: "Open the first chat" });
    expect(drawnWith(way, "block-size")).toBe("var(--size-control)");
    expect(drawnWith(way, "background")).toBe("var(--accent-fill)");
    const box = way.closest('[data-scope="empty-state"][data-part="action"]');
    expect(box).not.toBeNull();
    if (box) expect(drawnWith(box, "gap")).toBe("var(--space-3)");
  });
});
