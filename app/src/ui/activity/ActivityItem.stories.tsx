// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The activity family** (#2917, R116–R120; #3144): the left bar's column beside the project rail,
 * the right bar's column, and the `tab` variant's row across a side panel's top, each with its top
 * group, its gap and its bottom group, a plain count and the hot one. Walked by the keyboard, held to axe and compared with its baseline
 * (`docs/component-stories.md`); a chosen item is the same box as the others, so nothing moves.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect } from "storybook/test";
import {
  Brain,
  FolderGit2,
  FolderTree,
  GitBranch,
  Hand,
  History,
  KeyRound,
  ListTodo,
  MessagesSquare,
  Search,
  UsersRound,
  type LucideIcon,
} from "lucide-react";
import { expectFocusInSight } from "../../stories.testkit";
import { Panel } from "../panel/Panel";
import {
  ActivityBarFrame,
  ActivityCount,
  ActivityGroup,
  ActivityItem,
  type ActivityVariant,
} from "./ActivityItem";

type One = { name: string; mark: LucideIcon; count?: number; hot?: boolean };

const LEFT_TOP: One[] = [
  { name: "Chats", mark: MessagesSquare, count: 1 },
  { name: "Explorer", mark: FolderTree },
  { name: "Search", mark: Search },
];
const LEFT_BOTTOM: One[] = [
  { name: "Branch folders", mark: FolderGit2 },
  { name: "Changes", mark: GitBranch },
];
const RIGHT_TOP: One[] = [
  { name: "Needs you", mark: Hand, count: 2, hot: true },
  { name: "Personas", mark: UsersRound },
  { name: "Vaults", mark: KeyRound },
];
const RIGHT_BOTTOM: One[] = [
  { name: "Memory", mark: Brain },
  { name: "Todos", mark: ListTodo, count: 7 },
  { name: "History", mark: History },
];

function Group({
  name,
  items,
  variant,
  open,
  onPick,
}: {
  name: string;
  items: One[];
  variant: ActivityVariant;
  open: string | undefined;
  onPick: (name: string) => void;
}) {
  return (
    <ActivityGroup
      role="toolbar"
      aria-label={name}
      aria-orientation={variant === "tab" ? "horizontal" : "vertical"}
    >
      {items.map(({ name: called, mark: Mark, count, hot }) => (
        <ActivityItem
          key={called}
          variant={variant}
          selected={open === called}
          aria-label={called}
          aria-pressed={open === called}
          aria-describedby={count === undefined ? undefined : `count-${called}`}
          onClick={() => onPick(called)}
          icon={<Mark />}
          count={
            count === undefined ? undefined : (
              // Hidden from the name, read as the description (axe's name and content match).
              <span id={`count-${called}`} aria-hidden="true">
                <ActivityCount
                  count={count}
                  said={`${count} in ${called}`}
                  tone={hot ? "needs-you" : undefined}
                />
              </span>
            )
          }
        />
      ))}
    </ActivityGroup>
  );
}

/** A corner's bar, its top item open, pressed to switch. */
function Corner({ variant, side }: { variant: ActivityVariant; side: "left" | "right" }) {
  const [top, bottom] = side === "left" ? [LEFT_TOP, LEFT_BOTTOM] : [RIGHT_TOP, RIGHT_BOTTOM];
  const [open, setOpen] = useState<string | undefined>(top[0].name);
  const [half, setHalf] = useState<string | undefined>(undefined);
  const bar = (
    <ActivityBarFrame
      variant={variant}
      data-side={side}
      top={<Group name="Views" items={top} variant={variant} open={open} onPick={setOpen} />}
      bottom={
        <Group
          name="Bottom panel"
          items={bottom}
          variant={variant}
          open={half}
          onPick={(name) => setHalf((was) => (was === name ? undefined : name))}
        />
      }
    />
  );
  return variant === "tab" ? (
    <div style={{ inlineSize: "240px", blockSize: "120px" }}>
      <Panel variant="side">{bar}</Panel>
    </div>
  ) : side === "left" ? (
    // The left column beside the side panel, as the switcher prototype's variant D draws it.
    <div style={{ display: "flex", gap: "var(--space-4)", blockSize: "360px" }}>
      {bar}
      <div style={{ display: "grid", inlineSize: "160px" }}>
        <Panel variant="side" />
      </div>
    </div>
  ) : (
    <div style={{ display: "flex", blockSize: "360px" }}>{bar}</div>
  );
}

const meta = { component: Corner } satisfies Meta<typeof Corner>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab reaches an item with its ring in sight; Enter opens it, and the chosen one does not grow. */
const pressedByTheKeyboard: Story["play"] = async ({ canvas, userEvent }) => {
  const items = canvas.getAllByRole("button");
  const first = items[0];
  const second = items[1];
  const size = (one: HTMLElement) => one.getBoundingClientRect();
  const before = size(second);
  await userEvent.tab();
  await expect(first).toHaveFocus();
  await expect(first).toHaveAttribute("data-focus-visible");
  expectFocusInSight();
  await userEvent.tab();
  await userEvent.keyboard("{Enter}");
  await expect(second).toHaveAttribute("data-selected");
  await expect(first).not.toHaveAttribute("data-selected");
  const after = size(second);
  await expect([after.width, after.height, after.left]).toEqual([
    before.width,
    before.height,
    before.left,
  ]);
};

/** The left bar (#3144): a 44px column beside the side panel, Branch folders and Changes at its
 *  foot; Classic's accent edge along its left. */
export const TheLeftColumn: Story = {
  args: { variant: "bar", side: "left" },
  play: async (context) => {
    await pressedByTheKeyboard(context);
    // The keyboard let go, so the picture shows the chosen item at rest: Classic's accent edge
    // along its left, not under the focus ring.
    const chosen = context.canvas.getAllByRole("button")[1];
    chosen.blur();
    await expect(chosen).not.toHaveAttribute("data-focus-visible");
    await expect(chosen).toHaveAttribute("data-selected");
  },
};

/** The `tab` variant, for a theme that names it: tabs across a side panel's top, the bottom group
 *  at the row's end. No bar of the window draws it since #3144. */
export const TabsAcrossTheSidePanel: Story = {
  args: { variant: "tab", side: "left" },
  play: pressedByTheKeyboard,
};

/** The right bar: a 44px column, Memory, Todos and History at its foot, Needs you's count hot. */
export const TheRightColumn: Story = {
  args: { variant: "bar", side: "right" },
  play: async (context) => {
    await pressedByTheKeyboard(context);
    const hot = context.canvasElement.querySelector('[data-part="count"][data-tone="needs-you"]');
    await expect(hot).toHaveAttribute("data-count", "2");
  },
};
