// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A card pressed whole** (#3655, its own family since #3885): the welcome's doors and a repo
 * to get. Its button is React Aria's, named by the words it shows and described by its line, in a
 * box the family's own parts draw. The look in each theme is held by its stories
 * (`Card.stories.tsx`).
 */
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "../../cascade.testkit";
import { Card, CardTitle } from "./Card";
import { CardButton } from "./CardButton";

afterEach(cleanup);

/** The box a pressed card's button stands in. */
const boxOf = (button: HTMLElement) =>
  button.closest('[data-scope="card-button"][data-part="root"]') as HTMLElement;

describe("CardButton", () => {
  it("is a button named by its title alone, described by its line, in the tab sequence", () => {
    render(
      <CardButton
        title="Create a project"
        line="A new folder with a workspace in it."
        mark={<svg data-testid="mark" />}
      />,
    );
    const door = screen.getByRole("button", { name: "Create a project" });
    expect(door).toHaveAccessibleDescription("A new folder with a workspace in it.");
    // Its name is its own words (WCAG 2.5.3): the line and the mark stand beside it, in the card.
    expect(door).toHaveTextContent(/^Create a project$/);
    expect(door).not.toHaveAttribute("aria-label");
    expect(door).not.toHaveAttribute("aria-labelledby");
    expect(door).toHaveAttribute("tabindex", "0");
    expect(door).toHaveAttribute("type", "button");
    expect(screen.getByTestId("mark").parentElement).toHaveAttribute("data-part", "mark");
    expect(screen.getByTestId("mark").parentElement).toHaveAttribute("aria-hidden", "true");
  });

  it("is drawn by its own family's parts, its place given by the view", () => {
    render(<CardButton title="Create a project" line="A new folder." className="a-place" />);
    const door = screen.getByRole("button");
    const box = boxOf(door);
    expect(door).toHaveAttribute("data-scope", "card-button");
    expect(door).toHaveAttribute("data-part", "press");
    expect(box).toHaveAttribute("data-scope", "card-button");
    expect(box).toHaveAttribute("data-variant", "door");
    expect(box).toHaveClass("a-place");
    expect(screen.getByText("Create a project")).toHaveAttribute("data-part", "title");
    expect(screen.getByText("A new folder.")).toHaveAttribute("data-part", "line");
    expect(drawnWith(box, "background-color")).toBe("var(--control-base)");
    expect(drawnWith(box, "border")).toBe("1px solid var(--border-subtle)");
    expect(drawnWith(box, "border-radius")).toBe("var(--radius-xl)");
    expect(drawnWith(box, "background-color", { hover: true })).toBe("var(--control-hover)");
    expect(drawnWith(box, "border-color", { hover: true })).toBe("var(--border-strong)");
    expect(drawnWith(door, "cursor")).toBe("pointer");
    expect(drawnWith(door, "border-radius")).toBe("inherit");
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

  it("is named by its title and what follows it, the words it shows", () => {
    render(
      <CardButton variant="pick" title="acme/api" line="github.com · octocat">
        <span>purlis project</span>
      </CardButton>,
    );
    const pick = screen.getByRole("button", { name: "acme/api purlis project" });
    expect(pick).toHaveAccessibleDescription("github.com · octocat");
    expect(boxOf(pick)).toHaveAttribute("data-variant", "pick");
    expect(drawnWith(boxOf(pick), "background-color")).toBe("transparent");
    expect(drawnWith(boxOf(pick), "border-color")).toBe("transparent");
    expect(drawnWith(boxOf(pick), "background-color", { hover: true })).toBe(
      "var(--surface-hover)",
    );
  });

  it("draws nothing after the title where nothing follows it", () => {
    render(
      <CardButton variant="pick" title="acme/api">
        {false}
      </CardButton>,
    );
    expect(screen.getByRole("button", { name: "acme/api" })).toHaveTextContent(/^acme\/api$/);
  });

  it("says on its box that its button is under the pointer, pressed or unavailable", async () => {
    const { rerender } = render(<CardButton title="Create a project" />);
    const person = userEvent.setup();
    const box = boxOf(screen.getByRole("button"));
    expect(box).not.toHaveAttribute("data-hovered");
    await person.hover(screen.getByRole("button"));
    expect(box).toHaveAttribute("data-hovered");
    await person.unhover(screen.getByRole("button"));
    expect(box).not.toHaveAttribute("data-hovered");
    rerender(<CardButton title="Create a project" isDisabled />);
    expect(box).toHaveAttribute("data-disabled");
    expect(drawnWith(box, "color")).toBe("var(--text-faint)");
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

  it("says on its box that its button has the keyboard's focus", async () => {
    render(<CardButton title="Create a project" />);
    const box = boxOf(screen.getByRole("button"));
    expect(box).not.toHaveAttribute("data-focus-visible");
    await userEvent.setup().tab();
    expect(screen.getByRole("button")).toHaveFocus();
    expect(box).toHaveAttribute("data-focus-visible");
  });
});

describe("a pressed card's theme parts (#3885)", () => {
  // As the engine writes `card-button.*` and `card.title.*` (`parts_card_button.rs`).
  const root = '[data-scope="card-button"][data-part="root"]';
  const reached = (selector: string) => [...document.querySelectorAll(`:where(${selector})`)];

  it("reaches its box, its mark, its title and its line by its own scope", () => {
    render(
      <CardButton
        title="Create a project"
        line="A new folder."
        mark={<svg data-testid="mark" />}
      />,
    );
    const box = boxOf(screen.getByRole("button"));
    expect(reached(`${root}[data-variant="door"]`)).toEqual([box]);
    expect(reached(`${root} [data-scope="card-button"][data-part="mark"]`)).toEqual([
      screen.getByTestId("mark").parentElement,
    ]);
    expect(reached(`${root} [data-scope="card-button"][data-part="title"]`)).toEqual([
      screen.getByText("Create a project"),
    ]);
    expect(reached(`${root} [data-scope="card-button"][data-part="line"]`)).toEqual([
      screen.getByText("A new folder."),
    ]);
  });

  it("is never reached by a card's title, even inside a card", () => {
    // The order is explicit: a card's `card.title` reaches a card's own title, and a pressed
    // card's title is its own family's (`card-button.title`), wherever the card stands.
    render(
      <Card>
        <CardTitle>Doors</CardTitle>
        <CardButton title="Create a project" />
      </Card>,
    );
    const title = screen.getByText("Create a project");
    const card = '[data-scope="card"][data-part="root"] [data-scope="card"][data-part="title"]';
    expect(reached(card)).toEqual([screen.getByText("Doors")]);
    expect(reached(`${root} [data-scope="card-button"][data-part="title"]`)).toEqual([title]);
  });

  it("keys its states on its box, as its button's states are", async () => {
    render(<CardButton variant="pick" title="acme/api" />);
    const person = userEvent.setup();
    const box = boxOf(screen.getByRole("button"));
    await person.hover(screen.getByRole("button"));
    expect(reached(`${root}[data-variant="pick"][data-hovered]:not([data-disabled])`)).toEqual([
      box,
    ]);
  });
});
