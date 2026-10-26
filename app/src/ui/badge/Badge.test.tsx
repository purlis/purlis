// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "../../cascade.testkit";
import { Tooltip } from "../tooltip/Tooltip";
import { Badge, Count, CountGroup, HostStatus } from "./Badge";

/**
 * **The badge and count family** (#2912): a word on a pill and a number on a pill, drawn by their
 * parts, a count heard as what it counts, and a badge with a hint reached by the keyboard.
 */

afterEach(cleanup);

describe("Badge", () => {
  it("is the family's root in its variant, its look the family's", () => {
    render(<Badge variant="waiting">deprecated</Badge>);
    const badge = screen.getByText("deprecated");
    expect(badge).toHaveAttribute("data-scope", "badge");
    expect(badge).toHaveAttribute("data-part", "root");
    expect(badge).toHaveAttribute("data-variant", "waiting");
    expect(drawnWith(badge, "background-color")).toBe("var(--wash-warning)");
    expect(drawnWith(badge, "color")).toBe("var(--state-waiting)");
    expect(drawnWith(badge, "border-radius")).toBe("var(--radius-full)");
    expect(drawnWith(badge, "font-size")).toBe("var(--font-size-xs)");
  });

  it("says Your hosts' two statuses in their own words (R66)", () => {
    render(
      <>
        <HostStatus waiting />
        <HostStatus waiting={false} />
      </>,
    );
    expect(screen.getByText("Waiting for you")).toHaveAttribute("data-variant", "waiting");
    expect(drawnWith(screen.getByText("Confirmed"), "color")).toBe("var(--state-success)");
  });

  it("is a Tab stop when it has a hint, and only then", async () => {
    const user = userEvent.setup();
    render(
      <>
        <Tooltip tip="Set on this machine only.">
          <Badge variant="accent">This machine</Badge>
        </Tooltip>
        <Badge>purlis project</Badge>
      </>,
    );
    await user.tab();
    expect(screen.getByText("This machine")).toHaveFocus();
    expect(screen.getByText("purlis project")).not.toHaveAttribute("tabindex");
  });
});

describe("Count", () => {
  it("draws its number and is heard as what it counts", () => {
    render(
      <Count variant="hot" said="2 chats need you" data-testid="count">
        2
      </Count>,
    );
    const count = screen.getByTestId("count");
    expect(count).toHaveAttribute("data-scope", "count");
    expect(count).toHaveAttribute("data-variant", "hot");
    expect(screen.getByText("2")).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByText("2 chats need you")).toHaveClass("sr-only");
    expect(drawnWith(count, "background-color")).toBe("var(--needs-you-base)");
    expect(drawnWith(count, "color")).toBe("var(--needs-you-text)");
  });

  it("draws counts as one pill in a group, square between them", () => {
    render(
      <CountGroup role="img" aria-label="3 chats, 1 needs you">
        <Count aria-hidden="true">3</Count>
        <Count variant="hot" aria-hidden="true">
          1
        </Count>
      </CountGroup>,
    );
    const group = screen.getByRole("img", { name: "3 chats, 1 needs you" });
    expect(group).toHaveAttribute("data-part", "group");
    expect(drawnWith(group, "border-radius")).toBe("var(--radius-full)");
    expect(drawnWith(group.firstElementChild as Element, "border-radius")).toBe("0");
  });
});
