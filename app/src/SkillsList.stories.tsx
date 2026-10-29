// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skills view** (SK-55, #3266; spec #3679 S7, #3693), drawn as the left sidebar draws it:
 * whose skills it shows, its header, and the chat's skills as small cards in three groups that
 * fold (Needs you, In this chat with its built-in skills folded to one quiet line, Left out
 * folded), each card with its level, Use, its ⋯ menu, two lines or its reason, and its one fix;
 * then a project with no skill of its own, whose starts are New skill…'s. Each is walked by the
 * keyboard from the header in, held to axe and pictured in every pictured cell of the theme
 * matrix, light and dark among them (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { SkillPinsRefused } from "./bindings";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { expectFocusInSight, pictureNow, untilFocused, untilSettled } from "./stories.testkit";
import { Button } from "./ui/button/Button";
import { useListFilter } from "./ui/list/ListFilter";
import { SkillMenu, SkillsList } from "./SkillsList";
import { fixOf, groupsOf, markOf, type GroupId, type ListedSkill } from "./skillsListing";
import { DEFAULT_SKILLS_VIEW } from "./skillsViewPrefs";
import { skill } from "./skillsListing.testkit";

/** A workspace's skills, as the studio answers them for `ops`. */
const OPS: ListedSkill[] = [
  skill("drafted", {
    on: false,
    accept: "workspaces/ops/skills",
    left_out: "A chat made its folder: accept it to use it.",
  }),
  skill("secrets", {
    level: "built-in",
    folder: "/app/skills/secrets",
    description: "Uses a vault's secret without its value entering the chat.",
  }),
  skill("deploy", { description: "Deploys the service to staging, then production." }),
  skill("old-deploy", { on: false, turned_off: true, left_out: "ops turned it off." }),
  skill("release-notes", {
    pinned_by: [{ at: "workspace", name: "ops" }],
    hidden: [
      {
        harness: "claude",
        title: "Claude Code",
        hidden: {
          router: "built-in",
          why: "over-budget",
          said: "hidden by the built-in router: over this chat's skill budget, so it is in the index skill",
        },
      },
    ],
  }),
  skill("ops-runbook", { level: "workspace", folder: "workspaces/ops/skills/ops-runbook" }),
  skill("pdf", { level: "installed", folder: "", lint: ["It asks to pre-approve Bash."] }),
].map((one) => ({
  key: one.name,
  skill: one,
  program: one.level === "installed" ? "Claude Code" : undefined,
}));

/** The view with what was last done on it said under it, and a way out after it. */
function Skills({
  starts = false,
  rows = OPS,
  pinsRefused = null,
}: {
  starts?: boolean;
  rows?: ListedSkill[];
  /** Why ops's chats read none of its pins, where they read none (#3432). */
  pinsRefused?: SkillPinsRefused | null;
}) {
  const [said, setSaid] = useState("Nothing done");
  const [folded, setFolded] = useState<readonly GroupId[]>(DEFAULT_SKILLS_VIEW.folded);
  const filter = useListFilter(rows, (row) => row.skill.name);
  return (
    <div
      style={{
        inlineSize: "18rem",
        blockSize: "34rem",
        display: "grid",
        gridTemplateRows: "minmax(0, 1fr) auto auto",
      }}
    >
      <SkillsList
        whose="steward 5"
        groups={groupsOf(filter.shown)}
        count={rows.length}
        clashes={() => false}
        total={rows.length}
        query={filter.query}
        onQuery={filter.setQuery}
        filtered={filter.filtered}
        starts={starts}
        busy={false}
        onStart={(start) => setSaid(`Started ${start}`)}
        onOpenStudio={() => setSaid("Opened Skill studio")}
        onOpen={(row) => setSaid(`Opened ${row.skill.name}`)}
        usable={(row) => {
          const mark = markOf(row);
          return mark !== "left-out" && mark !== "off" && mark !== "needs-review";
        }}
        onUse={(row) => setSaid(`Used ${row.skill.name}`)}
        fixOf={(row) => fixOf(row, "ops")}
        onFix={(_, fix) => setSaid(fix.label)}
        onCopy={() => setSaid("Make my own copy")}
        menuOf={(row) => () => (
          <SkillMenu
            row={row}
            at={{ workspace: "ops", chat: true, explorer: "ops", pinsRefused }}
            busy={false}
            onAct={(what) => setSaid(`Chose ${what}`)}
          />
        )}
        folded={folded}
        onFold={(id, to) =>
          setFolded((was) =>
            to ? [...was.filter((one) => one !== id), id] : was.filter((one) => one !== id),
          )
        }
      />
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
      {/* What the keyboard reaches after the view, as the editor follows the sidebar. */}
      <Button>After the view</Button>
    </div>
  );
}

const meta = {
  component: Skills,
} satisfies Meta<typeof Skills>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **In this chat**: Tab walks the header (Open Skill studio, New skill…) onto the list's first
 * group, Needs you; ↓ walks its card and the next group's; Enter opens a card in the studio; ↓
 * reaches the built-in skills' quiet line, which Enter unfolds in place, with its closing line
 * and Make my own copy, and Space folds again; ↓ reaches Left out, folded, which → unfolds; a
 * card turned off says why and offers Turn on in ops; Shift+F10 opens a card's menu, whose Use in
 * this chat waits, saying why. The picture is the groups as they start.
 */
export const InThisChat: Story = {
  render: () => <Skills />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Skills" });
    const row = (name: string | RegExp) => within(list).getByRole("row", { name });
    const cell = (one: HTMLElement) => within(one).getByRole("gridcell");
    await expect(canvas.getByText("steward 5")).toBeVisible();
    await expect(cell(row("Left out, 2"))).toHaveAttribute("aria-expanded", "false");
    await expect(cell(row("1 built-in skill"))).toHaveAttribute("aria-expanded", "false");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Open Skill studio" }),
      "Skills: Tab lands on Open Skill studio",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "New skill…" }),
      "Skills: Tab reaches New skill…",
    );
    await userEvent.tab();
    const needs = row(/^Needs you/);
    await untilFocused(() => needs, "Skills: Tab lands on the first group");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row(/^drafted/), "Skills: Down reaches the group's card");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row(/^pdf/), "Skills: Down reaches the group's next card");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row(/^In this chat/), "Skills: Down reaches the next group");
    await userEvent.keyboard("{ArrowDown}");
    const deploy = row(/^deploy/);
    await untilFocused(() => deploy, "Skills: Down reaches a card");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Opened deploy"));
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row(/^ops-runbook/), "Skills: Down reaches the next card");
    await userEvent.keyboard("{ArrowDown}");
    const line = row("1 built-in skill");
    await untilFocused(() => line, "Skills: Down reaches the built-in skills' quiet line");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(row(/^secrets/)).toBeVisible());
    await expect(
      row("They ship with purlis: they can't be edited, turned off or replaced."),
    ).toBeVisible();
    await untilFocused(() => line, "Skills: the keyboard stays on the quiet line");
    await userEvent.keyboard(" ");
    await waitFor(() => expect(within(list).queryByRole("row", { name: /^secrets/ })).toBeNull());
    await userEvent.keyboard("{ArrowDown}");
    const leftOut = row(/^Left out/);
    await untilFocused(() => leftOut, "Skills: Down reaches Left out");
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(cell(leftOut)).toHaveAttribute("aria-expanded", "true"));
    await userEvent.keyboard("{ArrowDown}");
    const off = row(/^old-deploy/);
    await untilFocused(() => off, "Skills: Down reaches a card turned off");
    await expect(off).toHaveTextContent("ops turned it off.");
    await expect(within(off).getByRole("button", { name: "Turn on in ops" })).toBeVisible();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "old-deploy" });
    expect(within(menu).getByRole("menuitem", { name: /Use in this chat/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    await untilFocused(() => off, "Skills: Escape gives the keyboard back to the card");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowLeft}");
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => leftOut, "Skills: Up goes back to Left out");
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(cell(leftOut)).toHaveAttribute("aria-expanded", "false"));
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull(), { timeout: 3_000 });
  },
};

/**
 * **A project with no skill of its own**: the studio's claim over the skills its chats already
 * get, naming New skill…, whose menu holds the three starts. Tab reaches it past Open Skill
 * studio, Enter opens it on its first start, Enter starts one and closes it.
 *
 * The picture is of the menu open on its first start (#3414): taken once the menu has arrived,
 * its fade and its placement done, of the page, as a menu is drawn over the view. It opens from
 * the button's end, so it stays inside the view's width (#3308).
 */
export const NoSkillsOfItsOwn: Story = {
  parameters: { screenshot: "page" },
  render: () => <Skills starts rows={OPS.filter((row) => row.skill.level === "built-in")} />,
  play: async ({ canvas, userEvent }) => {
    expect(canvas.getByTestId("skills-view-empty")).toHaveTextContent("from New skill…");
    await userEvent.tab();
    await userEvent.tab();
    const button = canvas.getByRole("button", { name: "New skill…" });
    await untilFocused(() => button, "Skills: Tab reaches New skill…");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Ways to start a skill" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /Ask a chat/ }),
      "Skills: the menu opens on its first start",
    );
    expectFocusInSight();
    await untilSettled(menu);
    await pictureNow();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Started draft"));
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
  },
};

/**
 * **A pin no chat reads** (#3432): ops's pins are more than 64, so its chats read none of them.
 * Left out unfolds (→) and ↓ reaches the pinned card, hidden by the router; its menu offers
 * Unpin, and says under it that the pin is not used, and the core's why. The picture is of that menu, open and settled.
 */
export const APinNoChatReads: Story = {
  parameters: { screenshot: "page" },
  render: () => (
    <Skills
      pinsRefused={{
        by: { at: "workspace", name: "ops" },
        why: "the list holds more than 64 pins",
        bad: null,
        file: "workspaces/ops/skills.pins",
      }}
    />
  ),
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Skills" });
    // The pinned card is hidden by the router: Left out is unfolded (→) to reach it.
    const leftOut = within(list).getByRole("row", { name: /^Left out/ });
    leftOut.focus();
    await untilFocused(() => leftOut, "Skills: Left out has the keyboard");
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() =>
      expect(within(leftOut).getByRole("gridcell")).toHaveAttribute("aria-expanded", "true"),
    );
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(list).getByRole("row", { name: /^old-deploy/ }),
      "Skills: Down reaches Left out's first card",
    );
    await userEvent.keyboard("{ArrowDown}");
    const pinned = within(list).getByRole("row", { name: /^release-notes/ });
    await untilFocused(() => pinned, "Skills: the pinned row has the keyboard");
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "release-notes" }, { timeout: 3_000 });
    const unpin = within(menu).getByRole("menuitem", { name: "Unpin from this workspace" });
    expect(unpin).toHaveAccessibleDescription(
      "Pinned, not used: the list holds more than 64 pins.",
    );
    await untilSettled(menu);
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
  },
};
