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
import { Menu, MenuItem, MenuTrigger, MenuWords } from "../menu/Menu";
import { PopoverDialog, PopoverTrigger } from "../popover/Popover";
import { StatusItem } from "../status/StatusItem";
import { tipOf } from "../tooltip/tooltip.testkit";
import {
  BUTTON_VARIANTS,
  Button,
  ButtonRow,
  IconButton,
  ToggleButton,
  type ButtonVariant,
} from "./Button";

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

describe("a button's key hint (R33)", () => {
  it("draws the key in its shortcut part, after its words, and keeps it out of its name", () => {
    render(<Button shortcut="⌘↩">Start chat</Button>);
    const button = screen.getByRole("button", { name: "Start chat" });
    const hint = button.querySelector('[data-scope="button"][data-part="shortcut"]');
    if (hint === null) throw new Error("no shortcut part");
    expect(hint.tagName).toBe("KBD");
    expect(hint).toHaveAttribute("aria-hidden", "true");
    expect(hint).toHaveAttribute("data-key", "⌘↩");
    // Drawn by the stylesheet from its attribute, so the button's words are its words alone.
    expect(button.textContent).toBe("Start chat");
    expect(button.lastElementChild).toBe(hint);
  });

  it("keeps the hint's room, undrawn, while the key does not press it", () => {
    render(
      <Button shortcut="⌘↩" shortcutOff>
        Approve and start
      </Button>,
    );
    const hint = screen.getByRole("button").querySelector('[data-part="shortcut"]');
    if (hint === null) throw new Error("no shortcut part");
    expect(hint).toHaveAttribute("data-off", "");
    expect(drawnWith(hint, "visibility")).toBe("hidden");
  });

  it("draws the key small and mono, in the button's own words' colour", () => {
    render(<Button shortcut="⌘↩">Start chat</Button>);
    const hint = screen.getByRole("button").querySelector('[data-part="shortcut"]');
    if (hint === null) throw new Error("no shortcut part");
    expect(drawnWith(hint, "font-family")).toBe("var(--font-mono)");
    expect(drawnWith(hint, "font-size")).toBe("var(--font-size-xs)");
    // No colour of its own: the words' (the quiet tier would not hold on the primary fill).
    expect(drawnWith(hint, "color")).toBeUndefined();
  });

  it("is drawn by no button that names no key", () => {
    render(<Button>Save</Button>);
    expect(screen.getByRole("button").querySelector('[data-part="shortcut"]')).toBeNull();
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
      "max(24px, var(--size-chrome), var(--space-7))",
    );
    heldToTheTokens(
      screen.getByRole("button", { name: "Large" }),
      "calc(var(--size-control) + var(--space-2))",
    );
  });

  // WCAG 2.5.8 (#3210): the compact density draws the chrome at 18px, so the floor is 24 CSS px,
  // as a target is measured, under the chrome's height and the scale's. A row that wraps keeps
  // the same floor.
  it("never draws an sm button under the 24px target, in any density or text size", () => {
    render(
      <>
        <Button size="sm">A</Button>
        <ToggleButton size="sm">B</ToggleButton>
        <IconButton size="sm" icon={<Plus />} aria-label="Add" />
        <ButtonRow size="sm" aria-label="Wrapping">
          <Button>Wrapped</Button>
        </ButtonRow>
      </>,
    );
    for (const name of ["A", "B", "Add"]) {
      const button = screen.getByRole("button", { name });
      expect(drawnWith(button, "block-size"), name).toBe(
        "max(24px, var(--size-chrome), var(--space-7))",
      );
    }
    expect(drawnWith(screen.getByRole("button", { name: "Wrapped" }), "min-block-size")).toBe(
      "max(24px, var(--size-chrome), var(--space-7))",
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

  // #3264: a breadcrumb's crumb, words in a line of words.
  it("draws an inline quiet button as the line's words, cut to an ellipsis, underlined under the pointer", () => {
    render(
      <Button variant="quiet" size="sm" inline>
        steward 4
      </Button>,
    );
    const crumb = screen.getByRole("button", { name: "steward 4" });
    expect(crumb).toHaveAttribute("data-inline", "");
    expect(drawnWith(crumb, "block-size")).toBe("auto");
    expect(drawnWith(crumb, "padding")).toBe("0");
    expect(drawnWith(crumb, "border-width")).toBe("0");
    expect(drawnWith(crumb, "font-size")).toBe("inherit");
    expect(drawnWith(crumb, "text-overflow")).toBe("ellipsis");
    expect(drawnWith(crumb, "color")).toBe("var(--text-quiet)");
    expect(drawnWith(crumb, "background", { hover: true })).toBe("transparent");
    expect(drawnWith(crumb, "color", { hover: true })).toBe("var(--text-primary)");
    expect(drawnWith(crumb, "text-decoration", { hover: true })).toBe("underline");
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

/** Each way a person presses a button: the pointer, Enter, Space, a screen reader's press (a
 *  click with no pointer before it), and the arrows a menu's trigger opens on. */
async function pressedEveryWay(button: HTMLElement) {
  await userEvent.click(button);
  button.focus();
  await userEvent.keyboard("{Enter}");
  await userEvent.keyboard(" ");
  await userEvent.keyboard("{ArrowDown}");
  await userEvent.keyboard("{ArrowUp}");
  fireEvent.click(button, { detail: 0 });
}

describe("a button made unavailable by hand, with aria-disabled (#3522)", () => {
  it("runs no press, from the pointer, the keyboard or a screen reader, and keeps the focus", async () => {
    const pressed = vi.fn();
    render(
      <Button aria-disabled onPress={pressed}>
        Allow
      </Button>,
    );
    const allow = screen.getByRole("button", { name: "Allow" });
    await pressedEveryWay(allow);
    expect(pressed).not.toHaveBeenCalled();
    expect(allow).toHaveFocus();
    expect(allow).toHaveAttribute("aria-disabled", "true");
  });

  it("submits no form by a click, and stays its submit button", async () => {
    const submitted = vi.fn((event: Event) => event.preventDefault());
    const form = (off: boolean) => (
      <form onSubmit={(event) => submitted(event.nativeEvent)}>
        <Button type="submit" aria-disabled={off || undefined}>
          Go
        </Button>
      </form>
    );
    const { rerender } = render(form(true));
    // Still the form's submit button (#3636): its click, not its type, is refused.
    expect(screen.getByRole("button", { name: "Go" })).toHaveAttribute("type", "submit");
    await userEvent.click(screen.getByRole("button", { name: "Go" }));
    fireEvent.click(screen.getByRole("button", { name: "Go" }), { detail: 0 });
    expect(submitted).not.toHaveBeenCalled();
    rerender(form(false));
    expect(screen.getByRole("button", { name: "Go" })).toHaveAttribute("type", "submit");
    await userEvent.click(screen.getByRole("button", { name: "Go" }));
    expect(submitted).toHaveBeenCalledTimes(1);
  });

  // #3636: an Inbox answer while busy, in its roving group, still moves on the arrows.
  for (const [way, off] of [
    ["by hand", { "aria-disabled": true }],
    ["with a reason", { isDisabled: true, tooltip: "Answering…" }],
  ] as const)
    it(`hands every key but Enter and Space to its own handler, unavailable ${way}`, async () => {
      const keys = vi.fn();
      const pressed = vi.fn();
      render(
        <Button {...off} onKeyDown={(event) => keys(event.key)} onPress={pressed}>
          Allow
        </Button>,
      );
      screen.getByRole("button", { name: "Allow" }).focus();
      await userEvent.keyboard("{ArrowRight}{ArrowLeft}{Enter} ");
      expect(keys.mock.calls.map(([key]) => key)).toEqual(["ArrowRight", "ArrowLeft"]);
      expect(pressed).not.toHaveBeenCalled();
    });

  it("hands a trigger's keys to nothing while it is unavailable: its arrows would open it", async () => {
    const keys = vi.fn();
    render(
      <Button aria-disabled aria-haspopup="menu" aria-expanded={false} onKeyDown={keys}>
        More
      </Button>,
    );
    screen.getByRole("button", { name: "More" }).focus();
    await userEvent.keyboard("{ArrowDown}{Enter}");
    expect(keys).not.toHaveBeenCalled();
  });

  it("flips no toggle", async () => {
    const changed = vi.fn();
    render(
      <ToggleButton aria-disabled onChange={changed}>
        Wrap
      </ToggleButton>,
    );
    const wrap = screen.getByRole("button", { name: "Wrap" });
    await pressedEveryWay(wrap);
    expect(changed).not.toHaveBeenCalled();
    expect(wrap).toHaveAttribute("aria-pressed", "false");
  });
});

describe("an unavailable button that opens a popover or a menu (#3116, #3522)", () => {
  const ways = [
    [
      "with a tooltip saying why",
      { isDisabled: true, tooltip: "Only the owner shares this project." },
    ],
    ["with no tooltip", { isDisabled: true }],
    ["made unavailable by hand (#3522)", { "aria-disabled": true }],
  ] as const;

  const opener = (tooltip?: string) => (
    <Button isDisabled tooltip={tooltip}>
      Share
    </Button>
  );

  for (const [way, props] of ways)
    describe(way, () => {
      it("opens no popover", async () => {
        const opened = vi.fn();
        render(
          <PopoverTrigger onOpenChange={opened}>
            <Button {...props}>Share</Button>
            <PopoverDialog aria-label="Share">shared</PopoverDialog>
          </PopoverTrigger>,
        );
        await pressedEveryWay(screen.getByRole("button", { name: "Share" }));
        expect(opened).not.toHaveBeenCalled();
        expect(screen.queryByRole("dialog")).toBeNull();
      });

      it("opens no menu", async () => {
        const opened = vi.fn();
        render(
          <MenuTrigger onOpenChange={opened}>
            <Button {...props}>Share</Button>
            <Menu aria-label="Share">
              <MenuItem id="link" textValue="Copy link">
                <MenuWords said="Copy link" />
              </MenuItem>
            </Menu>
          </MenuTrigger>,
        );
        await pressedEveryWay(screen.getByRole("button", { name: "Share" }));
        expect(opened).not.toHaveBeenCalled();
        expect(screen.queryByRole("menu")).toBeNull();
      });
    });

  it("keeps its tooltip and its reason as a trigger", () => {
    render(
      <MenuTrigger>
        {opener("Only the owner shares this project.")}
        <Menu aria-label="Share">
          <MenuItem id="link" textValue="Copy link">
            <MenuWords said="Copy link" />
          </MenuItem>
        </Menu>
      </MenuTrigger>,
    );
    const share = screen.getByRole("button", { name: "Share" });
    expect(share).toHaveAttribute("aria-disabled", "true");
    expect(share).not.toBeDisabled();
    expect(share).toHaveAttribute("tabindex", "0");
    expect(tipOf(share)).toBe("Only the owner shares this project.");
    expect(share).toHaveAccessibleDescription("Only the owner shares this project.");
    // Still said to be a menu's trigger, and a closed one (D-3116-1): only the press is dropped.
    expect(share).toHaveAttribute("aria-haspopup", "true");
    expect(share).toHaveAttribute("aria-expanded", "false");
  });

  it("opens nothing as a status item either, the other pressable on the shared element", async () => {
    const branches = (isDisabled: boolean, opened: () => void) => (
      <MenuTrigger onOpenChange={opened}>
        <StatusItem isDisabled={isDisabled} tooltip="No branch to switch to here.">
          main
        </StatusItem>
        <Menu aria-label="Branches">
          <MenuItem id="main" textValue="main">
            <MenuWords said="main" />
          </MenuItem>
        </Menu>
      </MenuTrigger>
    );
    // Available, a press opens the menu: the trigger reaches it.
    const reached = vi.fn();
    const { unmount } = render(branches(false, reached));
    await userEvent.click(screen.getByRole("button", { name: "main" }));
    expect(reached).toHaveBeenCalledWith(true);
    unmount();

    const opened = vi.fn();
    render(branches(true, opened));
    const item = screen.getByRole("button", { name: "main" });
    await pressedEveryWay(item);
    expect(opened).not.toHaveBeenCalled();
    expect(screen.queryByRole("menu")).toBeNull();
    expect(item).toHaveAttribute("aria-haspopup", "true");
    expect(item).toHaveAccessibleDescription("No branch to switch to here.");
  });
});

describe("a trigger whose menu or popover is open (#3524)", () => {
  const ALL = [...BUTTON_VARIANTS];

  it("is marked open while its menu is, and not after it closes", async () => {
    render(
      <MenuTrigger>
        <Button variant="quiet">3 more</Button>
        <Menu aria-label="More">
          <MenuItem id="one" textValue="One">
            <MenuWords said="One" />
          </MenuItem>
        </Menu>
      </MenuTrigger>,
    );
    const more = screen.getByRole("button", { name: "3 more" });
    expect(more).not.toHaveAttribute("data-open");
    await userEvent.click(more);
    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(more).toHaveAttribute("data-open", "");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(more).not.toHaveAttribute("data-open");
  });

  it("is marked open while its popover is", async () => {
    render(
      <PopoverTrigger>
        <Button>Share</Button>
        <PopoverDialog aria-label="Share">shared</PopoverDialog>
      </PopoverTrigger>,
    );
    const share = screen.getByRole("button", { name: "Share" });
    await userEvent.click(share);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(share).toHaveAttribute("data-open", "");
  });

  it("is marked open when it says itself that it opens a menu that is open", () => {
    render(
      <>
        <Button aria-haspopup="menu" aria-expanded>
          Counts
        </Button>
        <Button aria-haspopup="menu" aria-expanded={false}>
          Closed
        </Button>
      </>,
    );
    expect(screen.getByRole("button", { name: "Counts" })).toHaveAttribute("data-open", "");
    expect(screen.getByRole("button", { name: "Closed" })).not.toHaveAttribute("data-open");
  });

  it("is not marked open as a disclosure whose region is out: it opens nothing over the window", () => {
    render(<Button aria-expanded>Details</Button>);
    const details = screen.getByRole("button", { name: "Details" });
    expect(details).toHaveAttribute("aria-expanded", "true");
    expect(details).not.toHaveAttribute("data-open");
  });

  it("is drawn open as it is under the pointer, in every variant that has a hover fill", () => {
    render(
      <>
        {ALL.map((variant) => (
          <Button key={variant} variant={variant} aria-haspopup="menu" aria-expanded>
            {`Open ${variant}`}
          </Button>
        ))}
        {ALL.map((variant) => (
          <Button key={variant} variant={variant}>{`Rest ${variant}`}</Button>
        ))}
      </>,
    );
    for (const variant of ALL) {
      const open = screen.getByRole("button", { name: `Open ${variant}` });
      const rest = screen.getByRole("button", { name: `Rest ${variant}` });
      for (const property of ["background", "color", "border-color"])
        expect(drawnWith(open, property), `${variant}'s ${property}`).toBe(
          drawnWith(rest, property, { hover: true }),
        );
    }
  });

  it("is drawn open inline as it is under the pointer: underlined, with no fill", () => {
    render(
      <Button inline variant="quiet" aria-haspopup="menu" aria-expanded>
        crumb
      </Button>,
    );
    const crumb = screen.getByRole("button", { name: "crumb" });
    expect(drawnWith(crumb, "background")).toBe("transparent");
    expect(drawnWith(crumb, "text-decoration")).toBe("underline");
    expect(drawnWith(crumb, "color")).toBe("var(--text-primary)");
  });

  it("is a disclosure when it says it opens no popup", () => {
    render(
      <Button aria-haspopup="false" aria-expanded>
        Details
      </Button>,
    );
    expect(screen.getByRole("button", { name: "Details" })).not.toHaveAttribute("data-open");
  });

  it("is never marked open as a status item, which has no open state", async () => {
    render(
      <MenuTrigger>
        <StatusItem>main</StatusItem>
        <Menu aria-label="Branches">
          <MenuItem id="main" textValue="main">
            <MenuWords said="main" />
          </MenuItem>
        </Menu>
      </MenuTrigger>,
    );
    const item = screen.getByRole("button", { name: "main" });
    await userEvent.click(item);
    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(item).toHaveAttribute("aria-expanded", "true");
    expect(item).not.toHaveAttribute("data-open");
  });

  it("keeps a data-open a wrapper hands it", () => {
    render(<Button data-open="">Held</Button>);
    expect(screen.getByRole("button", { name: "Held" })).toHaveAttribute("data-open", "");
  });

  it("is drawn as a toggle that is on under the pointer when it is on and open", () => {
    render(
      <>
        <ToggleButton isSelected aria-haspopup="menu" aria-expanded>
          Open on
        </ToggleButton>
        <ToggleButton isSelected>Rest on</ToggleButton>
      </>,
    );
    const open = screen.getByRole("button", { name: "Open on" });
    expect(open).toHaveAttribute("data-open", "");
    expect(drawnWith(open, "background")).toBe("var(--tab-active)");
    expect(drawnWith(open, "background")).toBe(
      drawnWith(screen.getByRole("button", { name: "Rest on" }), "background", { hover: true }),
    );
    expect(drawnWith(open, "border-color")).toBe("var(--accent-base)");
  });

  it("is never drawn open while unavailable", () => {
    render(
      <Button aria-disabled aria-haspopup="menu" aria-expanded>
        Off
      </Button>,
    );
    expect(screen.getByRole("button", { name: "Off" })).not.toHaveAttribute("data-open");
  });
});

describe("a button drawn inline", () => {
  it("is only a ghost: a variant with a ground of its own is a type error (#3264)", () => {
    // @ts-expect-error -- the plain button has a ground, tight against its letters inline.
    const plain = <Button inline aria-label="Save" />;
    // @ts-expect-error -- so has the suggested answer.
    const primary = <Button inline variant="primary" aria-label="Save" />;
    const ghost = (
      <Button inline variant="ghost">
        Save
      </Button>
    );
    expect([plain, primary, ghost]).toHaveLength(3);
  });
});
