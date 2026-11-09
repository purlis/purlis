// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The component parts a theme sets, in Settings › Theme** (#2947): the families the chosen
 * theme sets, each named with the parts manifest's description, and under Show each part every
 * family and part of the manifest, folded to its families, with what the theme and its options
 * set. Its keyboard: Tab to the family names, which say what each family is, then to Show each
 * part, its filter and the tree, whose arrows open a family and walk into its parts.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import type { GalleryPart, PartsManifest } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { ThemeParts } from "./themeParts";

/** A manifest of three families, as `theme_parts` hands it. */
const MANIFEST: PartsManifest = {
  version: 13,
  states: [
    { name: "hover", description: "The pointer is over it.", since: 1 },
    { name: "selected", description: "It is chosen.", since: 1 },
  ],
  families: [
    {
      name: "button",
      description: "A button, an icon button or a link button.",
      since: 1,
      states: ["hover", "pressed", "focused", "disabled"],
      parts: [
        {
          name: "root",
          description: "The button itself: its ground, edge, corners and room.",
          since: 1,
          properties: ["fill", "corner-radius"],
        },
        { name: "label", description: "Its words.", since: 2, properties: ["text"] },
        { name: "icon", description: "Its mark.", since: 2, properties: ["text"] },
      ],
      variants: [{ name: "primary", description: "The main answer.", since: 1 }],
    },
    {
      name: "field",
      description: "A box a person types a value into.",
      since: 1,
      states: ["hover", "focused", "invalid"],
      parts: [{ name: "root", description: "The box.", since: 1, properties: ["fill"] }],
      variants: [],
    },
    {
      name: "tabs",
      description: "A session tab, docked on the content panel or on a strip of its own.",
      since: 4,
      states: ["hover", "selected"],
      parts: [
        { name: "tab", description: "One tab.", since: 4, properties: ["corner-radius"] },
        { name: "count", description: "Its count.", since: 14, properties: ["fill"] },
      ],
      variants: [],
    },
  ],
};

const set = (key: string, value: string, from: GalleryPart["from"] = { kind: "theme" }) => {
  const [family, part] = key.split(".");
  return { key, family, part, value, from } satisfies GalleryPart;
};

const ROUNDED: GalleryPart[] = [
  set("button.root.corner-radius", "full", { kind: "option", option: "Pills", value: "On" }),
  set("button.root.primary.hover.fill", "accent.base"),
  set("tabs.tab.corner-radius", "10"),
];

const meta = {
  component: ThemeParts,
  args: {
    manifest: MANIFEST,
    name: "Rounded",
    set: ROUNDED,
    ids: { id: "theme-parts", labelledBy: undefined },
  },
  render: (args) => (
    <div style={{ inlineSize: "34rem" }}>
      <ThemeParts {...args} />
    </div>
  ),
} satisfies Meta<typeof ThemeParts>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **A theme that sets parts**: it names the two families it sets; Show each part opens the
 * manifest's families, folded; → opens button and ↓ walks into its root, whose keys say their
 * value and what sets each.
 */
export const SetsParts: Story = {
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await untilFocused(() => canvas.getByText("button"), "Parts: Tab reaches the first family");
    await userEvent.tab();
    await untilFocused(() => canvas.getByText("tabs"), "Parts: Tab reaches the next family");
    await userEvent.tab();
    const show = canvas.getByRole("button", { name: "Show each part" });
    await untilFocused(() => show, "Parts: Tab reaches Show each part");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(show).toHaveAttribute("aria-expanded", "true"));
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("searchbox", { name: "Filter parts" }),
      "Parts: Tab reaches the filter",
    );
    await userEvent.tab();
    const tree = canvas.getByRole("treegrid", { name: "Every component part" });
    const row = (name: string) => within(tree).getByRole("row", { name });
    await untilFocused(() => row("button"), "Parts: Tab lands on the first family");
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(row("button")).toHaveAttribute("aria-expanded", "true"));
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    await waitFor(() => expect(row("button.root")).toHaveAttribute("aria-expanded", "true"));
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => row("button.root.corner-radius"),
      "Parts: Down walks into the keys the theme sets",
    );
    expectFocusInSight();
  },
};

/** **A filter**: typing a value finds the key that has it, its family and part opened. */
export const Filtered: Story = {
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Show each part" }));
    await userEvent.type(canvas.getByRole("searchbox", { name: "Filter parts" }), "accent");
    const tree = canvas.getByRole("treegrid", { name: "Every component part" });
    await waitFor(() =>
      expect(
        within(tree).getByRole("row", { name: "button.root.primary.hover.fill" }),
      ).toBeVisible(),
    );
    await userEvent.tab();
    await untilFocused(
      () => within(tree).getByRole("row", { name: "button" }),
      "Filtered: Tab lands on the family found",
    );
    expectFocusInSight();
  },
};

/** **A theme that sets none**: it says every control is drawn as its family draws it. */
export const SetsNone: Story = {
  args: { name: "Plain", set: [] },
  play: async ({ canvas }) => {
    await expect(
      canvas.getByText("Plain sets no parts: every control is drawn as its family draws it."),
    ).toBeVisible();
  },
};
