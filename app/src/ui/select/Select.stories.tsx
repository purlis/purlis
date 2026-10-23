// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The select and combobox families** (#2906): a select holding a value, one holding nothing
 * with its placeholder, its list open from the keyboard, a long list filtered, each size, an
 * unavailable select with its reason, a settings row (the prototype's), and a combobox with its
 * suggestions. Each is drawn, held to axe, walked by the keyboard and compared with its baseline
 * in Chromium and WebKit (`docs/component-stories.md`). A list draws outside its frame, so each
 * picture is the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { ComboBox } from "./ComboBox";
import { FILTERED_FROM, SELECT_SIZES, Select, type SelectOption, type SelectProps } from "./Select";

const HARNESSES: SelectOption[] = [
  { value: "claude", label: "Claude Code" },
  { value: "codex", label: "Codex" },
  { value: "opencode", label: "OpenCode" },
  { value: "aider", label: "Aider", isDisabled: true, tooltip: "Aider is not installed" },
];

const WORKSPACES: SelectOption[] = [
  { value: "", label: "Every workspace" },
  ...[
    "smart-ide",
    "devops",
    "docs",
    "runners",
    "billing",
    "design",
    "website",
    "mobile",
    "research",
    "support",
    "infra",
  ].map((name) => ({ value: name, label: name })),
];

/** A select that holds its own value, so the keyboard can change it, and says what it holds. */
function Held({
  start,
  ...props
}: Omit<SelectProps, "value" | "onValueChange"> & { start?: string }) {
  const [value, setValue] = useState(start ?? "");
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <Select {...props} value={value} onValueChange={setValue} />
      <p role="status" style={{ margin: 0 }}>
        Holds {value === "" ? "nothing" : value}
      </p>
    </div>
  );
}

const meta = {
  component: Held,
  args: { "aria-label": "Harness", options: HARNESSES, start: "claude" },
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Held>;

export default meta;
type Story = StoryObj<typeof meta>;

/** How long a list may take to go: WebKit draws the next frame late in a busy run. */
const CLOSES = { timeout: 3_000 };

/** The select named `name`: React Aria names its button by what it holds, then by its own name. */
const theSelect = (canvas: { getByRole: typeof screen.getByRole }, name: string) =>
  canvas.getByRole("button", { name: new RegExp(`${name}$`) });

/** Tab lands on the select with its ring in sight, showing what it holds and its chevron. */
export const Picked: Story = {
  play: async ({ canvas, userEvent }) => {
    const select = theSelect(canvas, "Harness");
    await expect(select).toHaveTextContent("Claude Code");
    await userEvent.tab();
    await untilFocused(() => select, "Picked: Tab onto the select");
    expectFocusInSight();
  },
};

/** Nothing picked: the placeholder, in the quiet tier (R64). */
export const Placeholder: Story = {
  args: { start: "nowhere", placeholder: "Move to…" },
  play: async ({ canvas }) => {
    const select = theSelect(canvas, "Harness");
    await expect(within(select).getByText("Move to…")).toBeVisible();
  },
};

/**
 * **Its list, from the keyboard**: Enter opens it on what it holds, the arrows move and stop at
 * the ends past an option out of reach, Escape closes it and leaves the value, Enter picks; the
 * baseline shows it open on its last option.
 */
export const ItsList: Story = {
  play: async ({ canvas, userEvent }) => {
    const select = theSelect(canvas, "Harness");
    await userEvent.tab();
    await untilFocused(() => select, "Its list: Tab onto the select");
    await userEvent.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    await untilFocused(
      () => within(list).getByRole("option", { name: "Claude Code" }),
      "Its list: Enter opens it on what it holds",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull(), CLOSES);
    await untilFocused(() => select, "Its list: Escape gives the keyboard back");
    await expect(canvas.getByRole("status")).toHaveTextContent("Holds claude");
    await userEvent.keyboard("{Enter}");
    const again = await screen.findByRole("listbox");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    // Aider is out of reach: OpenCode is the last option the arrows stop on.
    await untilFocused(
      () => within(again).getByRole("option", { name: "OpenCode" }),
      "Its list: the arrows stop at the last option in reach",
    );
    expectFocusInSight();
  },
};

/** Enter picks the option the keyboard is on, and the list closes with the keyboard back. */
export const Picks: Story = {
  play: async ({ canvas, userEvent }) => {
    const select = theSelect(canvas, "Harness");
    await userEvent.tab();
    await untilFocused(() => select, "Picks: Tab onto the select");
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("listbox");
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull(), CLOSES);
    await expect(canvas.getByRole("status")).toHaveTextContent("Holds codex");
    await untilFocused(() => select, "Picks: the keyboard is back on the select");
  },
};

/**
 * **A long list is filtered** ({@link FILTERED_FROM} options or more): it opens with its filter
 * holding the keyboard, and what is typed narrows it; the baseline shows it narrowed.
 */
export const Filtered: Story = {
  args: { "aria-label": "Workspace", options: WORKSPACES, start: "" },
  play: async ({ canvas, userEvent }) => {
    await expect(WORKSPACES.length).toBeGreaterThanOrEqual(FILTERED_FROM);
    const select = theSelect(canvas, "Workspace");
    await userEvent.tab();
    await untilFocused(() => select, "Filtered: Tab onto the select");
    await userEvent.keyboard("{Enter}");
    const filter = await screen.findByRole("textbox", { name: "Filter" });
    await untilFocused(() => filter, "Filtered: the filter takes the keyboard");
    await userEvent.keyboard("de");
    const list = screen.getByRole("listbox");
    await waitFor(() =>
      expect(
        within(list)
          .getAllByRole("option")
          .map((one) => one.textContent),
      ).toEqual(["devops", "design"]),
    );
    await userEvent.keyboard("{ArrowDown}");
    expectFocusInSight();
  },
};

/** Each size, one select to a row: sm 24, md 28, lg 32. */
export const Sizes: Story = {
  render: () => (
    <div style={{ display: "grid", gap: "var(--space-4)", justifyItems: "start" }}>
      {SELECT_SIZES.map((size) => (
        <Held
          key={size}
          aria-label={`Harness, ${size}`}
          size={size}
          options={HARNESSES}
          start="claude"
        />
      ))}
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    for (const size of SELECT_SIZES) {
      await userEvent.tab();
      await untilFocused(() => theSelect(canvas, `Harness, ${size}`), `Sizes: Tab onto ${size}`);
      expectFocusInSight();
    }
  },
};

/** Unavailable, with its reason: kept in reach of the keyboard so its reason is heard, drawn in
 *  the faint tier, and it opens nothing. */
export const Unavailable: Story = {
  args: {
    isDisabled: true,
    tooltip: "purlis opens the project read-only and writes nothing to it.",
  },
  play: async ({ canvas, userEvent }) => {
    const select = theSelect(canvas, "Harness");
    await expect(select).toHaveAttribute("aria-disabled", "true");
    await userEvent.tab();
    await untilFocused(() => select, "Unavailable: Tab onto the select, for its reason");
    await userEvent.keyboard("{Enter}");
    await expect(screen.queryByRole("listbox")).toBeNull();
  },
};

/**
 * **A settings row** (`prototype/settings-page`, R64): the label column with its help under it,
 * and the select in the control column, at most 320 px wide.
 */
export const InASettingsRow: Story = {
  render: () => {
    function Row() {
      const [value, setValue] = useState("claude");
      return (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "minmax(180px, 38%) minmax(0, 1fr)",
            gap: "var(--space-2) var(--space-6)",
            inlineSize: "40rem",
          }}
        >
          <div>
            <label
              id="harness-label"
              htmlFor="harness"
              style={{ fontWeight: "var(--font-weight-medium)" }}
            >
              Default harness
            </label>
            <p id="harness-help" style={{ margin: 0, color: "var(--text-quiet)" }}>
              What a new chat runs.
            </p>
          </div>
          <div style={{ maxInlineSize: "20rem" }}>
            <Select
              id="harness"
              aria-labelledby="harness-label"
              aria-describedby="harness-help"
              className="w-full"
              value={value}
              options={HARNESSES}
              onValueChange={setValue}
            />
          </div>
        </div>
      );
    }
    return <Row />;
  },
  play: async ({ canvas, userEvent }) => {
    const select = theSelect(canvas, "Default harness");
    await expect(select).toHaveAccessibleDescription("What a new chat runs.");
    await userEvent.tab();
    await untilFocused(() => select, "In a settings row: Tab onto the select");
    expectFocusInSight();
  },
};

/** **A combobox**: typing opens what it suggests and narrows it; ↓ moves into it, Enter writes the
 *  one the keyboard is on into the box. */
export const Suggestions: Story = {
  render: () => {
    function Box() {
      const [value, setValue] = useState("");
      return (
        <div style={{ inlineSize: "20rem" }}>
          <ComboBox
            label="Add a skill"
            suggestions={["purlis:handoff", "purlis:persona", "deploy", "superpowers:tdd"]}
            value={value}
            onChange={setValue}
            placeholder="Its name, as the Skill studio lists it"
          />
        </div>
      );
    }
    return <Box />;
  },
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("combobox", { name: "Add a skill" });
    await userEvent.tab();
    await untilFocused(() => box, "Suggestions: Tab into the box");
    await userEvent.keyboard("purlis");
    const list = await screen.findByRole("listbox");
    await waitFor(() => expect(within(list).getAllByRole("option")).toHaveLength(2));
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    await waitFor(() => expect(box).toHaveValue("purlis:persona"));
    await userEvent.keyboard("{Backspace>7/}");
    await waitFor(() => expect(screen.getByRole("listbox")).toBeVisible());
    expectFocusInSight();
  },
};
