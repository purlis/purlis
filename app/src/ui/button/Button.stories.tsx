// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The button family** (#2904): every variant, size and state that draws differently, each
 * drawn, held to axe, walked by the keyboard and compared with its baseline in Chromium and
 * WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { Plus, RefreshCw, X } from "lucide-react";
import { expectFocusInSight } from "../../stories.testkit";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "../menu/Menu";
import {
  Button,
  ButtonRow,
  IconButton,
  ToggleButton,
  type ButtonSize,
  type ButtonVariant,
} from "./Button";

/** A button that says how many times it was pressed, under it, as a person would read it. */
function Counted({
  variant,
  size,
  label = "Save",
  disabled = false,
}: {
  variant?: ButtonVariant;
  size?: ButtonSize;
  label?: string;
  disabled?: boolean;
}) {
  const [presses, setPresses] = useState(0);
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <Button
        variant={variant}
        size={size}
        isDisabled={disabled}
        onPress={() => setPresses((was) => was + 1)}
      >
        {label}
      </Button>
      <p role="status" style={{ margin: 0 }}>
        {`Pressed ${presses} ${presses === 1 ? "time" : "times"}`}
      </p>
    </div>
  );
}

const meta = {
  component: Counted,
} satisfies Meta<typeof Counted>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab lands on the button with its ring in sight; Enter and Space press it; Tab leaves it. */
const pressedByTheKeyboard: Story["play"] = async ({ canvas, userEvent, args }) => {
  const button = canvas.getByRole("button", { name: args.label ?? "Save" });
  await userEvent.tab();
  await expect(button).toHaveFocus();
  expectFocusInSight();
  await userEvent.keyboard("{Enter}");
  await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Pressed 1 time"));
  await userEvent.keyboard(" ");
  await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Pressed 2 times"));
  expectFocusInSight();
  await userEvent.tab();
  await expect(button).not.toHaveFocus();
  // Back on it, which is what the baseline shows: the focus ring.
  await userEvent.tab({ shift: true });
  await expect(button).toHaveFocus();
};

export const Secondary: Story = { args: { variant: "secondary" }, play: pressedByTheKeyboard };
export const Primary: Story = {
  args: { variant: "primary", label: "Start" },
  play: pressedByTheKeyboard,
};
export const Ghost: Story = {
  args: { variant: "ghost", label: "Dismiss" },
  play: pressedByTheKeyboard,
};
export const Danger: Story = {
  args: { variant: "danger", label: "Delete vault" },
  play: pressedByTheKeyboard,
};
export const SoftAccent: Story = {
  args: { variant: "soft-accent", label: "Try it" },
  play: pressedByTheKeyboard,
};
export const WarningSoft: Story = {
  args: { variant: "warning-soft", label: "Restart later" },
  play: pressedByTheKeyboard,
};
/** The quiet ghost (#3214): a strip's end mark, a pane's corner, its words in the quiet tier. */
export const Quiet: Story = {
  args: { variant: "quiet", label: "3 more" },
  play: pressedByTheKeyboard,
};
/**
 * **The quiet ghost inline** (#3264): a breadcrumb's crumbs, words in a line of words. In a line
 * too narrow for them they cut to an ellipsis and keep the line's height; Tab reaches each, with
 * its ring in sight, and Enter presses it.
 */
function InlineCrumbs() {
  const [shown, show] = useState("deep");
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <nav
        aria-label="Chat path"
        style={{
          display: "flex",
          alignItems: "center",
          inlineSize: "9rem",
          overflow: "hidden",
          fontSize: "var(--font-size-xs)",
          whiteSpace: "nowrap",
        }}
      >
        {["steward 4 of the long name", "talk"].map((name) => (
          <span key={name} style={{ display: "contents" }}>
            <Button
              variant="quiet"
              size="sm"
              inline
              style={{ flex: "0 1 auto", minInlineSize: "1.5em" }}
              onPress={() => show(name)}
            >
              {name}
            </Button>
            <span aria-hidden="true" style={{ whiteSpace: "pre" }}>
              {" › "}
            </span>
          </span>
        ))}
        <span aria-current="page">deep</span>
      </nav>
      <p role="status" style={{ margin: 0 }}>
        {`Showing ${shown}`}
      </p>
    </div>
  );
}

export const QuietInline: Story = {
  render: () => <InlineCrumbs />,
  play: async ({ canvas, userEvent }) => {
    const own = canvas.getByRole("button", { name: "steward 4 of the long name" });
    const talk = canvas.getByRole("button", { name: "talk" });
    // Cut, as a native button's words are: the box is narrower than what it says.
    await expect(own.scrollWidth).toBeGreaterThan(own.clientWidth);
    await expect(getComputedStyle(own).textOverflow).toBe("ellipsis");
    // As tall as the line's words.
    const word = canvas.getByText("deep").getBoundingClientRect().height;
    await expect(Math.abs(own.getBoundingClientRect().height - word)).toBeLessThanOrEqual(1);
    await userEvent.tab();
    await expect(own).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(talk).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Showing talk"));
  },
};
/**
 * **The quiet ghost as the window draws it** (#3214): small icon marks in a pane's corner, split
 * and close, where the close that ends a chat takes the danger colour under the pointer
 * (`paneFrame.css`), and the split keeps the quiet hover.
 */
export const QuietPaneMarks: Story = {
  render: () => (
    // The pane's group shows itself on the pane's hover or focus; here it is always shown.
    <div className="pane-doing" style={{ visibility: "visible" }}>
      <IconButton icon={<Plus />} aria-label="Split right" variant="quiet" size="sm" />
      <IconButton
        icon={<X />}
        aria-label="End this pane's chat"
        variant="quiet"
        size="sm"
        className="ends-a-chat"
      />
      <span data-testid="danger" style={{ background: "var(--danger-surface)" }} />
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    const split = canvas.getByRole("button", { name: "Split right" });
    const close = canvas.getByRole("button", { name: "End this pane's chat" });
    for (const one of [split, close]) {
      const box = one.getBoundingClientRect();
      await expect(box.height).toBeGreaterThanOrEqual(24);
      await expect(Math.round(box.width)).toBe(Math.round(box.height));
    }
    // What each draws under the pointer, in this cell's theme: the state attribute React Aria
    // writes for a hover, put on each for the reading and taken off after, so the colour is
    // read as drawn and not raced against the pointer.
    const danger = getComputedStyle(canvas.getByTestId("danger")).backgroundColor;
    const hovered = (one: HTMLElement) => {
      one.setAttribute("data-hovered", "true");
      const ground = getComputedStyle(one).backgroundColor;
      one.removeAttribute("data-hovered");
      return ground;
    };
    await expect(hovered(close)).toBe(danger);
    await expect(hovered(split)).not.toBe(danger);
    await expect(getComputedStyle(close).backgroundColor).not.toBe(danger);
    await userEvent.tab();
    await expect(split).toHaveFocus();
    expectFocusInSight();
  },
};
export const Link: Story = {
  args: { variant: "link", label: "Open its settings" },
  play: pressedByTheKeyboard,
};
export const Small: Story = { args: { size: "sm", label: "Copy" }, play: pressedByTheKeyboard };
export const Large: Story = { args: { size: "lg", label: "Filter" }, play: pressedByTheKeyboard };

/** Disabled: never reached by Tab, so never pressed from the keyboard. */
export const Disabled: Story = {
  args: { disabled: true },
  play: async ({ canvas, userEvent }) => {
    const button = canvas.getByRole("button", { name: "Save" });
    await expect(button).toBeDisabled();
    await userEvent.tab();
    await expect(button).not.toHaveFocus();
    await expect(canvas.getByRole("status")).toHaveTextContent("Pressed 0 times");
  },
};

/**
 * **An unavailable menu button with its reason** (#3116): it keeps the focus and says why, and
 * starts nothing: no press, Enter, Space or arrow opens the menu it is the trigger of.
 */
export const UnavailableMenuTrigger: Story = {
  render: () => (
    <MenuTrigger>
      <Button isDisabled tooltip="Only the owner shares this project.">
        Share
      </Button>
      <Menu aria-label="Share">
        <MenuItem id="link" textValue="Copy link">
          <MenuWords said="Copy link" />
        </MenuItem>
      </Menu>
    </MenuTrigger>
  ),
  play: async ({ canvas, userEvent }) => {
    const share = canvas.getByRole("button", { name: "Share" });
    await expect(share).toHaveAttribute("aria-disabled", "true");
    await expect(share).toHaveAccessibleDescription("Only the owner shares this project.");
    await userEvent.tab();
    await expect(share).toHaveFocus();
    expectFocusInSight();
    // After each way of pressing it, no menu: one press that opened it and the next that closed
    // it again would pass a single look at the end.
    const noMenu = () => expect(document.querySelector('[role="menu"]')).toBeNull();
    for (const key of ["{Enter}", " ", "{ArrowDown}", "{ArrowUp}"]) {
      await userEvent.keyboard(key);
      await noMenu();
    }
    await userEvent.click(share);
    await noMenu();
    await expect(share).toHaveAttribute("aria-expanded", "false");
  },
};

/** The variants with a fill under the pointer, each drawn as a trigger whose menu is open. */
const OPENED: readonly ButtonVariant[] = [
  "secondary",
  "primary",
  "ghost",
  "quiet",
  "danger",
  "grants",
  "soft-accent",
  "warning-soft",
];

/**
 * **A trigger whose menu is open** (#3524): drawn as it is under the pointer, in every variant
 * with a hover fill, so the press that opened it still shows while the pointer is in the menu.
 * The row is each variant held open; the `3 more` beside it opens a real menu, and is drawn at
 * rest again once it closes.
 */
export const OpenTrigger: Story = {
  render: () => (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <ButtonRow aria-label="Open triggers" role="group">
        {OPENED.map((variant) => (
          <Button key={variant} variant={variant} aria-haspopup="menu" aria-expanded>
            {variant}
          </Button>
        ))}
      </ButtonRow>
      <MenuTrigger>
        <Button variant="quiet" size="sm">
          3 more
        </Button>
        <Menu aria-label="More">
          <MenuItem id="one" textValue="One">
            <MenuWords said="One" />
          </MenuItem>
        </Menu>
      </MenuTrigger>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    for (const variant of OPENED)
      await expect(canvas.getByRole("button", { name: variant })).toHaveAttribute("data-open");
    const more = canvas.getByRole("button", { name: "3 more" });
    await expect(more).not.toHaveAttribute("data-open");
    await userEvent.click(more);
    await waitFor(() => expect(document.querySelector('[role="menu"]')).not.toBeNull());
    await expect(more).toHaveAttribute("data-open");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(document.querySelector('[role="menu"]')).toBeNull());
    await expect(more).not.toHaveAttribute("data-open");
  },
};

/**
 * **An answer that grants beside Keep blocked** (ADR 0097 §6): Allow is as plain as Keep
 * blocked, told apart by its accent edge and the lock it leads with, never by the primary look.
 * Both are reached in order and answer to Enter.
 */
function GrantsPair() {
  const [said, setSaid] = useState("Not answered");
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <ButtonRow aria-label="Answers to the network ask" role="group">
        <Button onPress={() => setSaid("Kept blocked")}>Keep blocked</Button>
        <Button variant="grants" onPress={() => setSaid("Allowed")}>
          Allow
        </Button>
      </ButtonRow>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

export const GrantsBesideKeepBlocked: Story = {
  render: () => <GrantsPair />,
  play: async ({ canvas, userEvent }) => {
    const keep = canvas.getByRole("button", { name: "Keep blocked" });
    const allow = canvas.getByRole("button", { name: "Allow" });
    await expect(allow).toHaveAttribute("data-variant", "grants");
    await userEvent.tab();
    await expect(keep).toHaveFocus();
    await userEvent.tab();
    await expect(allow).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Allowed"));
    await userEvent.tab({ shift: true });
    await expect(keep).toHaveFocus();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Kept blocked"));
  },
};

/** A row takes one size: every button in a small row is small, whatever it asked for. */
export const RowOfOneSize: Story = {
  render: () => (
    <ButtonRow size="sm" aria-label="Ways out" role="group">
      <Button icon={<Plus />}>New chat</Button>
      <Button size="lg">Open</Button>
      <Button variant="ghost">Dismiss</Button>
    </ButtonRow>
  ),
  play: async ({ canvas, userEvent }) => {
    const buttons = canvas.getAllByRole("button");
    const heights = buttons.map((one) => one.getBoundingClientRect().height);
    await expect(new Set(heights).size).toBe(1);
    for (const one of buttons) {
      await userEvent.tab();
      await expect(one).toHaveFocus();
      expectFocusInSight();
    }
  },
};

/**
 * **The key that presses it** (R33, #3377): the `shortcut` part, small and mono after the words,
 * on the primary and the secondary, and off (`shortcutOff`): its room kept and nothing drawn, so
 * a button whose key does not press it now is as wide as one whose key does (R163). The key is
 * never part of the button's name or words.
 */
export const WithItsKey: Story = {
  render: () => (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <ButtonRow aria-label="Keys on" role="group">
        <Button variant="primary" shortcut="⌘↩" aria-keyshortcuts="Meta+Enter">
          Start chat
        </Button>
        <Button shortcut="⌘S" aria-keyshortcuts="Meta+S">
          Save
        </Button>
      </ButtonRow>
      <ButtonRow aria-label="Key off" role="group">
        <Button variant="primary" shortcut="⌘↩" shortcutOff>
          Start chat
        </Button>
      </ButtonRow>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    const [on, off] = canvas.getAllByRole("button", { name: "Start chat" });
    for (const button of [on, off]) await expect(button).toHaveTextContent(/^Start chat$/);
    const hint = (button: HTMLElement) =>
      button.querySelector('[data-scope="button"][data-part="shortcut"]') as HTMLElement;
    await expect(hint(on)).toBeVisible();
    await expect(hint(off)).not.toBeVisible();
    // Off keeps its room: the two are one width.
    await expect(off.getBoundingClientRect().width).toBe(on.getBoundingClientRect().width);
    await userEvent.tab();
    await expect(on).toHaveFocus();
    expectFocusInSight();
  },
};

/** Icon buttons: square at each size, named by their label alone. */
function IconButtonsDrawn() {
  const [said, setSaid] = useState("Nothing pressed");
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <ButtonRow aria-label="Icon buttons" role="toolbar" wraps={false}>
        <IconButton
          icon={<RefreshCw />}
          aria-label="Refresh"
          size="sm"
          onPress={() => setSaid("Refreshed")}
        />
        <IconButton
          icon={<Plus />}
          aria-label="New chat"
          size="sm"
          variant="ghost"
          onPress={() => setSaid("New chat")}
        />
      </ButtonRow>
      <IconButton icon={<X />} aria-label="Close" onPress={() => setSaid("Closed")} />
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

export const IconButtons: Story = {
  render: () => <IconButtonsDrawn />,
  play: async ({ canvas, userEvent }) => {
    for (const name of ["Refresh", "New chat", "Close"]) {
      const button = canvas.getByRole("button", { name });
      const box = button.getBoundingClientRect();
      await expect(Math.round(box.width)).toBe(Math.round(box.height));
      await expect(box.height).toBeGreaterThanOrEqual(24);
    }
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Close" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Closed"));
  },
};

/**
 * **Two small glyph buttons at the family's tightest gap** (#3210, WCAG 2.5.8): a row's own gap,
 * a hairline in the compact density. Each is a 24px target by itself in every cell, so axe's
 * `target-size` passes without any spacing to lean on.
 */
export const SmallGlyphsAtTheTightestGap: Story = {
  render: () => (
    <ButtonRow size="sm" aria-label="A view's first row" role="toolbar" wraps={false}>
      <IconButton icon={<Plus />} aria-label="Add a memory" variant="ghost" />
      <IconButton icon={<X />} aria-label="Archive the view" variant="ghost" />
    </ButtonRow>
  ),
  play: async ({ canvas, userEvent }) => {
    const [add, archive] = [
      canvas.getByRole("button", { name: "Add a memory" }),
      canvas.getByRole("button", { name: "Archive the view" }),
    ];
    for (const one of [add, archive]) {
      const box = one.getBoundingClientRect();
      await expect(box.height).toBeGreaterThanOrEqual(24);
      await expect(box.width).toBeGreaterThanOrEqual(24);
    }
    await userEvent.tab();
    await expect(add).toHaveFocus();
    expectFocusInSight();
  },
};

/** A toggle: says it is pressed, by its edge and ground as well as its state, and turns. */
function Toggled() {
  const [on, setOn] = useState(false);
  return (
    <ToggleButton isSelected={on} onChange={setOn} size="sm">
      Wrap lines
    </ToggleButton>
  );
}

export const Toggle: Story = {
  render: () => <Toggled />,
  play: async ({ canvas, userEvent }) => {
    const toggle = canvas.getByRole("button", { name: "Wrap lines" });
    await expect(toggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.tab();
    await expect(toggle).toHaveFocus();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(toggle).toHaveAttribute("aria-pressed", "true"));
    expectFocusInSight();
  },
};
