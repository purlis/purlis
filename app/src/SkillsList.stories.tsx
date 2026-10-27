// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skills view** (SK-55, #3266), drawn as the left sidebar draws it: its header, the switch
 * to the chat in front, and this workspace's skills as a tree of levels that fold (#3281), Needs
 * you on top, one mark at most per row, each with the view's own menu; then a project with no
 * skill of its own, whose starts are New skill…'s. Each is walked by the keyboard from the header in, held to axe and
 * pictured in every pictured cell of the theme matrix, light and dark among them
 * (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { SkillPinsRefused } from "./bindings";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { expectFocusInSight, pictureNow, untilFocused, untilSettled } from "./stories.testkit";
import { Button } from "./ui/button/Button";
import { useListFilter } from "./ui/list/ListFilter";
import { SkillMenu, SkillsList } from "./SkillsList";
import { groupsOf, type GroupId, type ListedSkill } from "./skillsListing";
import { foldedAfter } from "./skillsViewPrefs";
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
  const [only, setOnly] = useState(false);
  const [folded, setFolded] = useState<readonly GroupId[]>([]);
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
        groups={groupsOf(filter.shown, { root: false, chat: false })}
        total={rows.length}
        query={filter.query}
        onQuery={filter.setQuery}
        filtered={filter.filtered}
        starts={starts}
        busy={false}
        onStart={(start) => setSaid(`Started ${start}`)}
        onOpenStudio={() => setSaid("Opened Skill studio")}
        onOpen={(row) => setSaid(`Opened ${row.skill.name}`)}
        menuOf={(row) => () => (
          <SkillMenu
            row={row}
            at={{ workspace: "ops", chat: true, explorer: "ops", pinsRefused }}
            busy={false}
            onAct={(what) => setSaid(`Chose ${what}`)}
          />
        )}
        chat={{ only, onOnly: setOnly }}
        folded={folded}
        onFold={(shown, open) => setFolded((was) => foldedAfter(was, shown, open))}
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
 * **By level**: Tab walks the header (Open Skill studio, New skill…, Only this chat's) onto the
 * tree's first level; ↓ walks a level's skills and the next level; ← folds Built-in and ↓ passes
 * it; → moves into Project, Enter opens a skill's tab, the keyboard on a row shows what it does
 * and why it has its mark, Shift+F10 opens its menu, and Tab leaves the tree (#3281). The picture
 * is the tree with Built-in folded.
 */
export const ByLevel: Story = {
  render: () => <Skills />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("treegrid", { name: "Skills" });
    const level = (name: string) => within(list).getByRole("row", { name: new RegExp(`^${name}`) });
    for (const name of ["Needs you", "Built-in", "Project", "Workspace", "Claude Code"])
      expect(level(name)).toHaveAttribute("aria-expanded", "true");
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
    await untilFocused(
      () => canvas.getByRole("button", { name: "Only this chat's" }),
      "Skills: Tab reaches Only this chat's",
    );
    await userEvent.tab();
    await untilFocused(() => level("Needs you"), "Skills: Tab lands on the tree's first level");
    await userEvent.keyboard("{ArrowDown}");
    const first = within(list).getByRole("row", { name: /drafted/ });
    await untilFocused(() => first, "Skills: Down moves into the level");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => level("Built-in"), "Skills: Down reaches the next level");
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(level("Built-in")).toHaveAttribute("aria-expanded", "false"));
    expect(within(list).queryByRole("row", { name: /secrets/ })).toBeNull();
    await untilFocused(() => level("Built-in"), "Skills: Left folds the level, and stays on it");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => level("Project"), "Skills: Down passes a folded level's skills");
    await userEvent.keyboard("{ArrowRight}");
    const deploy = within(list).getByRole("row", { name: /^deploy/ });
    await untilFocused(() => deploy, "Skills: Right moves into an open level");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Opened deploy"));
    await userEvent.keyboard("{ArrowDown}");
    const off = within(list).getByRole("row", { name: /old-deploy/ });
    await untilFocused(() => off, "Skills: Down reaches a row turned off");
    await waitFor(() =>
      expect(screen.getByRole("tooltip")).toHaveTextContent("ops turned it off."),
    );
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "old-deploy" });
    // A row turned off: Use in this chat waits, saying why, so the menu opens on its switch.
    expect(within(menu).getByRole("menuitem", { name: /Use in this chat/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /Turn on in ops/ }),
      "Skills: Shift+F10 opens the row's menu on the first row it can run",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    // **The focus is checked once it has settled on the row** (#3449): in WebKit the menu row
    // the keyboard was on goes with the menu, which leaves the focus on the page, and React
    // Aria's focus scope gives it back to the row a frame later. Checked before that frame,
    // nothing has the focus (run 40415737701).
    await untilFocused(() => off, "Skills: Escape gives the keyboard back to the row");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "After the view" }),
      "Skills: Tab leaves the tree",
    );
    // And once the row it left has taken its tooltip down: the page is at rest.
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull(), { timeout: 3_000 });
    await untilFocused(
      () => canvas.getByRole("button", { name: "After the view" }),
      "Skills: the keyboard stays after the view",
    );
    expectFocusInSight();
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
 * The pinned row's menu offers Unpin, and says under it that the pin is not used, and the core's
 * why. The picture is of that menu, open and settled.
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
    const list = canvas.getByRole("treegrid", { name: "Skills" });
    const pinned = within(list).getByRole("row", { name: /release-notes/ });
    pinned.focus();
    await untilFocused(() => pinned, "Skills: the pinned row has the keyboard");
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "release-notes" });
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

// TEMPORARY (#3449): By Level four more times in every cell, to show it steady. Taken out before
// the PR is ready.
export const ByLevelAgain1: Story = { ...ByLevel, parameters: { screenshot: false } };
export const ByLevelAgain2: Story = { ...ByLevel, parameters: { screenshot: false } };
export const ByLevelAgain3: Story = { ...ByLevel, parameters: { screenshot: false } };
export const ByLevelAgain4: Story = { ...ByLevel, parameters: { screenshot: false } };
