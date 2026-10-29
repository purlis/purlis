// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A card pressed whole** (#3655, the card family): the welcome's doors and a repo to get. It is
 * React Aria's button, named by its title and described by its line, drawn by the family's own
 * parts. The look in each theme is held by its stories (`Card.stories.tsx`).
 */
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "../../cascade.testkit";
import { CardButton } from "./CardButton";

afterEach(cleanup);

describe("CardButton", () => {
  it("is a button named by its title and described by its line, in the tab sequence", () => {
    render(
      <CardButton
        title="Create a project"
        line="A new folder with a workspace in it."
        mark={<svg data-testid="mark" />}
      />,
    );
    const door = screen.getByRole("button", { name: "Create a project" });
    expect(door).toHaveAccessibleDescription("A new folder with a workspace in it.");
    expect(door).toHaveAttribute("tabindex", "0");
    expect(door).toHaveAttribute("type", "button");
    expect(screen.getByTestId("mark").closest("[data-part]")).toHaveAttribute("data-part", "mark");
  });

  it("is drawn by the card family's parts, never by a view's class", () => {
    render(
      <CardButton title="Create a project" line="A new folder." className="welcome-door-place" />,
    );
    const door = screen.getByRole("button");
    expect(door).toHaveAttribute("data-scope", "card");
    expect(door).toHaveAttribute("data-part", "pressable");
    expect(door).toHaveAttribute("data-variant", "door");
    expect(door).toHaveClass("welcome-door-place");
    expect(screen.getByText("Create a project")).toHaveAttribute("data-part", "title");
    expect(screen.getByText("A new folder.")).toHaveAttribute("data-part", "line");
    expect(drawnWith(door, "background-color")).toBe("var(--control-base)");
    expect(drawnWith(door, "background-color", { hover: true })).toBe("var(--control-hover)");
    expect(drawnWith(door, "border-radius")).toBe("var(--radius-xl)");
    // The family's ring, outside the card, from the keyboard only.
    expect(drawnWith(door, "outline", { focusVisible: true })).toBe(
      "var(--focus-ring-width) solid var(--focus-ring)",
    );
    expect(drawnWith(door, "outline-offset", { focusVisible: true })).toBe(
      "var(--focus-ring-offset)",
    );
  });

  it("is pressed by the pointer, Enter and Space", async () => {
    const press = vi.fn();
    render(<CardButton title="Open a project" line="One you already have." onPress={press} />);
    const person = userEvent.setup();
    await person.click(screen.getByRole("button", { name: "Open a project" }));
    await person.tab();
    await person.tab({ shift: true });
    expect(screen.getByRole("button")).toHaveFocus();
    await person.keyboard("{Enter}");
    await person.keyboard(" ");
    expect(press).toHaveBeenCalledTimes(3);
  });

  it("is named by its title and what follows it, the words it shows (WCAG 2.5.3)", () => {
    render(
      <CardButton variant="pick" title="acme/api" line="github.com · octocat">
        <span>purlis project</span>
      </CardButton>,
    );
    const pick = screen.getByRole("button", { name: "acme/api purlis project" });
    expect(pick).toHaveAccessibleDescription("github.com · octocat");
    expect(pick).toHaveAttribute("data-variant", "pick");
    expect(drawnWith(pick, "background-color")).toBe("transparent");
    expect(drawnWith(pick, "background-color", { hover: true })).toBe("var(--surface-hover)");
  });

  it("presses nothing while unavailable, and hands its ref on", async () => {
    const press = vi.fn();
    const ref = createRef<HTMLButtonElement>();
    render(<CardButton ref={ref} title="purlis" isDisabled onPress={press} />);
    const pick = screen.getByRole("button", { name: "purlis" });
    expect(ref.current).toBe(pick);
    expect(pick).toBeDisabled();
    await userEvent.setup().click(pick);
    expect(press).not.toHaveBeenCalled();
  });
});
