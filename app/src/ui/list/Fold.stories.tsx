// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What folds in a list** (#3670): the sidebar Skills view's three groups (Needs you, in the
 * waiting colour; In this chat, its built-ins folded to one quiet line at its end; Left out,
 * folded), as `prototype/skills-sidebar` draws them, folded and unfolded by the keyboard, and a
 * filter that finds a folded built-in and draws it outside the fold. Held to axe and compared
 * with its baseline in Chromium and WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { Sparkle } from "lucide-react";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Badge } from "../badge/Badge";
import { Button } from "../button/Button";
import { List } from "./List";
import { ListCardRow } from "./CardRow";
import { ListFoldSection, ListRun } from "./Fold";
import { ListFilter } from "./ListFilter";

interface Skill {
  name: string;
  level: string;
  says: string;
  needs?: boolean;
}

const NEEDS: Skill[] = [
  {
    name: "grilling",
    level: "persona",
    says: "Changed by a chat on 28 Oct. This chat keeps the version you accepted.",
    needs: true,
  },
];

const IN_THIS_CHAT: Skill[] = [
  {
    name: "deploy",
    level: "project",
    says: "Deploys a service with the checks the team agreed on.",
  },
  { name: "review-pr", level: "project", says: "Reviews a pull request against REVIEW.md." },
];

const BUILT_IN: Skill[] = [
  { name: "browser", level: "built-in", says: "Drives a real browser for a task that needs one." },
  { name: "handoff", level: "built-in", says: "Moves work into a new chat you'll read yourself." },
  {
    name: "secrets",
    level: "built-in",
    says: "Uses a vault credential without showing its value.",
  },
  { name: "compact", level: "built-in", says: "Compacts a workspace's or persona's memory." },
];

const LEFT_OUT: Skill[] = [
  { name: "lint-rs", level: "workspace", says: "Off in smart-ide" },
  { name: "gpu-profile", level: "local", says: "Claude Code can't take it" },
];

const SKILL = { one: "skill", many: "skills" };

/** The cell a fold's row holds, which says whether it is open. */
const cellOf = (one: HTMLElement) => within(one).getByRole("gridcell");

const matches = (query: string) => (skill: Skill) =>
  query !== "" && skill.name.toLowerCase().includes(query.toLowerCase());

/** One skill's card. */
const card = (skill: Skill, tone?: "warning" | "quiet") => (
  <ListCardRow
    key={skill.name}
    id={skill.name}
    textValue={skill.name}
    name={skill.name}
    mark={<Badge>{skill.level}</Badge>}
    description={tone === undefined ? skill.says : undefined}
    reason={tone === undefined ? undefined : { says: skill.says, tone }}
  />
);

/** **The sidebar Skills view's groups**, in a 260 px side view. */
function SkillGroups() {
  const [query, setQuery] = useState("");
  const [leftOutFolded, setLeftOutFolded] = useState(true);
  const [said, setSaid] = useState("Nothing run");
  const shown = (skills: Skill[]) =>
    query === "" ? skills : skills.filter((one) => one.name.includes(query.toLowerCase()));
  const needs = shown(NEEDS);
  return (
    <div style={{ inlineSize: "260px", background: "var(--surface-base)" }}>
      <ListFilter noun={SKILL} total={9} query={query} onQuery={setQuery} />
      <List aria-label="Skills" size="sm" onAction={(key) => setSaid(`Opened ${String(key)}`)}>
        {/* Needs you is drawn only while it holds something. */}
        {needs.length > 0 && (
          <ListFoldSection id="needs" title="Needs you" count={needs.length} tone="waiting">
            {needs.map((one) => card(one, "warning"))}
          </ListFoldSection>
        )}
        <ListFoldSection
          id="in"
          title="In this chat"
          count={shown(IN_THIS_CHAT).length + BUILT_IN.length}
        >
          {shown(IN_THIS_CHAT).map((one) => card(one))}
          <ListRun
            id="built-in"
            items={BUILT_IN}
            standsOut={matches(query)}
            says={(count) => `${count} built-in skills`}
            mark={<Sparkle aria-hidden="true" />}
            closing={{
              says: "They ship with purlis: they can't be edited or turned off.",
              children: (
                <Button size="sm" variant="ghost" onPress={() => setSaid("Make my own copy")}>
                  Make my own copy…
                </Button>
              ),
            }}
          >
            {(one) => card(one)}
          </ListRun>
        </ListFoldSection>
        <ListFoldSection
          id="out"
          title="Left out"
          count={shown(LEFT_OUT).length}
          folded={leftOutFolded}
          onFoldedChange={setLeftOutFolded}
        >
          {shown(LEFT_OUT).map((one) => card(one, "quiet"))}
        </ListFoldSection>
      </List>
      <p role="status" style={{ margin: "var(--space-3) 0 0" }}>
        {said}
      </p>
    </div>
  );
}

const meta = {
  component: SkillGroups,
} satisfies Meta<typeof SkillGroups>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **The Skills view's groups, folded**: Needs you in the waiting colour, In this chat with its
 * built-ins folded to one quiet line at its end, and Left out folded. ↓ walks the names and the
 * rows; Enter on Left out unfolds it, ← folds it again and → unfolds it; Enter on the quiet line
 * unfolds the built-ins in place, with its closing line after them, and Space folds them again;
 * a group's name runs nothing of the list's. The picture is the groups folded, the keyboard on
 * the quiet line.
 */
export const SkillsViewGroups: Story = {
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Skills" });
    const row = (name: string | RegExp) => within(list).getByRole("row", { name });
    const needs = row("Needs you, 1");
    await expect(cellOf(needs)).toHaveAttribute("aria-expanded", "true");
    const leftOut = row("Left out, 2");
    await expect(cellOf(leftOut)).toHaveAttribute("aria-expanded", "false");
    await expect(within(list).queryByRole("row", { name: "lint-rs" })).toBeNull();
    // The filter, then the list's one stop: the first group's name.
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => needs, "Groups: Tab lands on the first group's name");
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Nothing run");
    // Enter folded it; → unfolds it again.
    await waitFor(() => expect(cellOf(needs)).toHaveAttribute("aria-expanded", "false"));
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(cellOf(needs)).toHaveAttribute("aria-expanded", "true"));
    await untilFocused(() => needs, "Groups: Right unfolds and keeps the keyboard on the name");
    // Down through the rows to the quiet line.
    for (let at = 0; at < 5; at += 1) await userEvent.keyboard("{ArrowDown}");
    const line = row("4 built-in skills");
    await untilFocused(() => line, "Groups: Down reaches the quiet line");
    await expect(cellOf(line)).toHaveAttribute("aria-expanded", "false");
    // The rows above it keep their place as it unfolds.
    const above = row("review-pr").getBoundingClientRect().top;
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(row("browser")).toBeVisible());
    await expect(row("review-pr").getBoundingClientRect().top).toBe(above);
    await expect(
      within(list).getByRole("row", {
        name: "They ship with purlis: they can't be edited or turned off.",
      }),
    ).toBeVisible();
    await untilFocused(() => line, "Groups: the keyboard stays on the quiet line");
    await userEvent.keyboard(" ");
    await waitFor(() => expect(within(list).queryByRole("row", { name: "browser" })).toBeNull());
    // Left out: → unfolds, ← folds.
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => leftOut, "Groups: Down reaches Left out");
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(row("lint-rs")).toBeVisible());
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(within(list).queryByRole("row", { name: "lint-rs" })).toBeNull());
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => line, "Groups: Up goes back to the quiet line");
    expectFocusInSight();
  },
};

/**
 * **A filter finds a folded built-in** and draws it on its own, outside the fold; the quiet line
 * counts only what it still folds. The picture is the filtered groups.
 */
export const AFilterFindsABuiltIn: Story = {
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Skills" });
    await userEvent.type(canvas.getByRole("searchbox", { name: "Filter skills" }), "secr");
    await waitFor(() => expect(within(list).getByRole("row", { name: "secrets" })).toBeVisible());
    await expect(
      cellOf(within(list).getByRole("row", { name: "3 built-in skills" })),
    ).toHaveAttribute("aria-expanded", "false");
    await expect(within(list).queryByRole("row", { name: "browser" })).toBeNull();
    await expect(within(list).queryByRole("row", { name: /Needs you/ })).toBeNull();
  },
};
