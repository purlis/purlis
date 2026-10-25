// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The button family's wrapper** (#2904): what a person presses and hears, and the attributes a
 * theme's parts are keyed on (ADR 0103). The look itself is held by the stories, in a real
 * browser (`Button.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Plus } from "lucide-react";
import { useState } from "react";
import { drawnWith } from "../../cascade.testkit";
import { tipOf } from "../tooltip/tooltip.testkit";
import { Button, ButtonRow, IconButton, ToggleButton, type ButtonVariant } from "./Button";

afterEach(cleanup);

describe("a button", () => {
  it("is a native button, named by its words, pressed by a click, Enter and Space", async () => {
    const pressed = vi.fn();
    render(<Button onPress={pressed}>Save</Button>);
    const button = screen.getByRole("button", { name: "Save" });
    expect(button.tagName).toBe("BUTTON");
    expect(button).toHaveAttribute("type", "button");

    await userEvent.click(button);
    expect(pressed).toHaveBeenCalledTimes(1);
    button.focus();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard(" ");
    expect(pressed).toHaveBeenCalledTimes(3);
  });

  it("is in WebKit's tab sequence, and out of it when it asks to be (#190)", () => {
    render(
      <>
        <Button>In</Button>
        <Button excludeFromTabOrder>Out</Button>
      </>,
    );
    expect(screen.getByRole("button", { name: "In" })).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("button", { name: "Out" })).toHaveAttribute("tabindex", "-1");
  });

  it("carries the family's parts, its variant and its size", () => {
    render(
      <Button variant="primary" size="sm" icon={<Plus />}>
        New {"chat"}
      </Button>,
    );
    const button = screen.getByRole("button", { name: "New chat" });
    expect(button).toHaveAttribute("data-scope", "button");
    expect(button).toHaveAttribute("data-part", "root");
    expect(button).toHaveAttribute("data-variant", "primary");
    expect(button).toHaveAttribute("data-size", "sm");
    // One run of words is one label, as one anonymous flex item was.
    const labels = button.querySelectorAll('[data-scope="button"][data-part="label"]');
    expect([...labels].map((one) => one.textContent)).toEqual(["New chat"]);
    const icon = button.querySelector('[data-scope="button"][data-part="icon"]');
    expect(icon).toHaveAttribute("aria-hidden", "true");
    expect(icon?.querySelector("svg")).not.toBeNull();
  });

  it("is the secondary button at md when nothing is said", () => {
    render(<Button>Cancel</Button>);
    const button = screen.getByRole("button", { name: "Cancel" });
    expect(button).toHaveAttribute("data-variant", "secondary");
    expect(button).toHaveAttribute("data-size", "md");
  });

  it("keeps the native attributes React Aria would drop", () => {
    render(
      <Button tooltip="Saves every file" aria-busy="true" aria-keyshortcuts="Meta+S">
        Save
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Save" });
    // Its hint is the shared tooltip (#2908), never a native one.
    expect(button).not.toHaveAttribute("title");
    expect(tipOf(button)).toBe("Saves every file");
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(button).toHaveAttribute("aria-keyshortcuts", "Meta+S");
  });

  it("is not pressed while disabled", () => {
    const pressed = vi.fn();
    render(
      <Button isDisabled onPress={pressed}>
        Save
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Save" });
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(pressed).not.toHaveBeenCalled();
  });

  it("hands a click's event to an onClick, as a Radix trigger it is the child of needs", () => {
    const clicked = vi.fn();
    render(<Button onClick={clicked}>Open</Button>);
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(clicked).toHaveBeenCalledTimes(1);
    expect(clicked.mock.calls[0][0]).toHaveProperty("currentTarget");
  });
});

describe("an answer that grants (ADR 0097 §6)", () => {
  it("leads with the open lock, hidden from a screen reader, and is never the primary look", () => {
    render(<Button variant="grants">Allow</Button>);
    const allow = screen.getByRole("button", { name: "Allow" });
    expect(allow).toHaveAttribute("data-variant", "grants");
    const first = allow.firstElementChild;
    expect(first).toHaveAttribute("data-part", "icon");
    expect(first).toHaveAttribute("aria-hidden", "true");
    expect(first?.querySelector("svg.lucide-lock-open")).not.toBeNull();
  });
});

describe("an icon button", () => {
  it("shows only its icon and is named by its label", () => {
    render(<IconButton icon={<Plus />} aria-label="New chat" size="sm" />);
    const button = screen.getByRole("button", { name: "New chat" });
    expect(button).toHaveAttribute("data-icon-only", "");
    expect(button.querySelector('[data-part="label"]')).toBeNull();
  });
});

describe("a toggle button", () => {
  it("says it is pressed, and turns with a press", async () => {
    const changed = vi.fn();
    render(
      <ToggleButton isSelected onChange={changed}>
        Wrap lines
      </ToggleButton>,
    );
    const toggle = screen.getByRole("button", { name: "Wrap lines" });
    expect(toggle).toHaveAttribute("aria-pressed", "true");
    expect(toggle).toHaveAttribute("data-selected", "true");
    await userEvent.click(toggle);
    expect(changed).toHaveBeenCalledWith(false);
  });
});

describe("a row of buttons", () => {
  it("gives every button in it the row's size, so a mixed row cannot be built", () => {
    render(
      <ButtonRow size="sm">
        <Button>Keep</Button>
        <Button size="lg">Discard</Button>
      </ButtonRow>,
    );
    for (const name of ["Keep", "Discard"])
      expect(screen.getByRole("button", { name })).toHaveAttribute("data-size", "sm");
  });
});

/** A button's height and ring, as the window's cascade draws them (`cascade.testkit.ts`). */
function heldToTheTokens(button: Element, height = "var(--size-control)") {
  expect(drawnWith(button, "block-size"), "its height").toBe(height);
  expect(drawnWith(button, "outline", { focusVisible: true }), "its ring").toBe(
    "var(--focus-ring-width) solid var(--focus-ring)",
  );
  expect(drawnWith(button, "outline-offset", { focusVisible: true }), "its ring's place").toBe(
    "var(--focus-ring-offset)",
  );
}

describe("the family's look, from the tokens (button.css)", () => {
  const VARIANTS: ButtonVariant[] = ["secondary", "primary", "ghost", "danger", "grants"];

  it("is one height at each size in every variant, with the outside ring (R162)", () => {
    render(
      <>
        {VARIANTS.map((variant) => (
          <Button key={variant} variant={variant}>{`Save ${variant}`}</Button>
        ))}
        <Button size="sm">Small</Button>
        <Button size="lg">Large</Button>
      </>,
    );
    for (const variant of VARIANTS)
      heldToTheTokens(screen.getByRole("button", { name: `Save ${variant}` }));
    heldToTheTokens(
      screen.getByRole("button", { name: "Small" }),
      "max(var(--size-chrome), var(--space-7))",
    );
    heldToTheTokens(
      screen.getByRole("button", { name: "Large" }),
      "calc(var(--size-control) + var(--space-2))",
    );
  });

  it("draws each variant on its own ground", () => {
    render(
      <>
        {[...VARIANTS, "soft-accent", "warning-soft", "link"].map((variant) => (
          <Button key={variant} variant={variant as ButtonVariant}>{`Save ${variant}`}</Button>
        ))}
      </>,
    );
    const drawn = (variant: string, property: string, hover = false) =>
      drawnWith(screen.getByRole("button", { name: `Save ${variant}` }), property, { hover });
    expect(drawn("secondary", "background")).toBe("var(--control-base)");
    // The primary is filled by its own token (#3177): Purlis's accent fill, Classic's surface.
    expect(drawn("primary", "background")).toBe("var(--accent-fill)");
    expect(drawn("primary", "color")).toBe("var(--accent-fill-text)");
    expect(drawn("primary", "background", true)).toBe("var(--accent-fill-hover)");
    expect(drawn("ghost", "background")).toBe("transparent");
    expect(drawn("danger", "background")).toBe("var(--control-base)");
    expect(drawn("danger", "color")).toBe("var(--danger-base)");
    expect(drawn("danger", "background", true)).toBe("var(--danger-surface)");
    expect(drawn("soft-accent", "background")).toBe("var(--accent-surface)");
    expect(drawn("soft-accent", "border-color")).toBe("transparent");
    expect(drawn("warning-soft", "color")).toBe("var(--state-waiting)");
    expect(drawn("link", "color")).toBe("var(--markdown-link)");
    expect(drawn("link", "background")).toBe("transparent");
    expect(drawn("link", "text-decoration", true)).toBe("underline");
  });

  // #3214: a mark at the end of a strip, a crumb, a pane's split and close.
  it("draws the quiet ghost's words in the quiet tier, primary on the hover fill", () => {
    render(<Button variant="quiet">More</Button>);
    const quiet = screen.getByRole("button", { name: "More" });
    expect(quiet).toHaveAttribute("data-variant", "quiet");
    expect(drawnWith(quiet, "background")).toBe("transparent");
    expect(drawnWith(quiet, "border-color")).toBe("transparent");
    expect(drawnWith(quiet, "color")).toBe("var(--text-quiet)");
    expect(drawnWith(quiet, "background", { hover: true })).toBe("var(--surface-hover)");
    expect(drawnWith(quiet, "color", { hover: true })).toBe("var(--text-primary)");
    heldToTheTokens(quiet);
  });

  it("draws a grant as plain as the secondary, edged in the accent (ADR 0097 §6)", () => {
    render(<Button variant="grants">Allow</Button>);
    const grant = screen.getByRole("button", { name: "Allow" });
    heldToTheTokens(grant);
    expect(drawnWith(grant, "background")).toBe("var(--control-base)");
    expect(drawnWith(grant, "color")).toBe("var(--text-primary)");
    expect(drawnWith(grant, "border-color")).toBe("var(--accent-base)");
    expect(drawnWith(grant, "background", { hover: true })).toBe("var(--control-hover)");
  });

  it("stands a pressed toggle on the accent, under the pointer too", async () => {
    function Toggle() {
      const [on, setOn] = useState(false);
      return (
        <ToggleButton size="sm" isSelected={on} onChange={setOn}>
          Changed only
        </ToggleButton>
      );
    }
    render(<Toggle />);
    const toggle = screen.getByRole("button", { name: "Changed only", pressed: false });
    expect(drawnWith(toggle, "background")).toBe("var(--control-base)");
    await userEvent.tab();
    await userEvent.keyboard(" ");
    expect(screen.getByRole("button", { name: "Changed only", pressed: true })).toBe(toggle);
    expect(drawnWith(toggle, "background")).toBe("var(--accent-surface)");
    expect(drawnWith(toggle, "border-color")).toBe("var(--accent-base)");
    expect(drawnWith(toggle, "background", { hover: true })).toBe("var(--tab-active)");
  });

  it("draws what is unavailable or busy in faint words, without changing its size", () => {
    render(
      <>
        <Button isDisabled>Off</Button>
        <Button variant="danger" aria-disabled="true" aria-busy="true">
          Busy
        </Button>
      </>,
    );
    const off = screen.getByRole("button", { name: "Off" });
    const busy = screen.getByRole("button", { name: "Busy" });
    expect(drawnWith(off, "color")).toBe("var(--text-faint)");
    expect(drawnWith(busy, "color")).toBe("var(--text-faint)");
    expect(drawnWith(off, "opacity")).toBeUndefined();
    expect(drawnWith(busy, "cursor")).toBe("progress");
    heldToTheTokens(off);
    heldToTheTokens(busy);
  });

  it("marks a button that keeps the focus while unavailable as disabled, never hovered or pressed", async () => {
    const pressed = vi.fn();
    render(
      <Button aria-disabled="true" onPress={pressed}>
        Allow
      </Button>,
    );
    const allow = screen.getByRole("button", { name: "Allow" });
    expect(allow).toHaveAttribute("data-disabled", "true");
    // Still a Tab stop (#1700), so the keyboard is not dropped to the page.
    expect(allow).toHaveAttribute("tabindex", "0");
    await userEvent.hover(allow);
    expect(allow).not.toHaveAttribute("data-hovered");
    fireEvent.pointerDown(allow, { pointerId: 1, button: 0 });
    expect(allow).not.toHaveAttribute("data-pressed");
    // Its ground under the pointer is its ground at rest.
    expect(drawnWith(allow, "background", { hover: true })).toBe("var(--control-base)");
  });
});

describe("a key a button's handler leaves alone", () => {
  it("goes on to the row, as a native button's does", async () => {
    const row = vi.fn();
    const own = vi.fn();
    render(
      <div onKeyDown={(event) => row(event.key)}>
        <Button onKeyDown={(event) => own(event.key)}>Reveal</Button>
      </div>,
    );
    screen.getByRole("button", { name: "Reveal" }).focus();
    await userEvent.keyboard("{Escape}");
    expect(own).toHaveBeenCalledWith("Escape");
    expect(row).toHaveBeenCalledWith("Escape");
  });
});
