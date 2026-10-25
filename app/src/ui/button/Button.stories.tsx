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
