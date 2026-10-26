// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skills view** (SK-55, #3266), drawn as the left sidebar draws it: its header, the switch
 * to the chat in front, and this workspace's skills by level with Needs you on top, one mark at
 * most per row, each with the view's own menu; then a project with no skill of its own, whose
 * starts are New skill…'s. Each is walked by the keyboard from the header in, held to axe and
 * pictured in every pictured cell of the theme matrix, light and dark among them
 * (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { expectFocusInSight, pictureNow, untilFocused, untilSettled } from "./stories.testkit";
import { Button } from "./ui/button/Button";
import { useListFilter } from "./ui/list/ListFilter";
import { SkillMenu, SkillsList } from "./SkillsList";
import { groupsOf, type ListedSkill } from "./skillsListing";
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
  skill("ops-runbook", { level: "workspace", folder: "workspaces/ops/skills/ops-runbook" }),
  skill("pdf", { level: "installed", folder: "", lint: ["It asks to pre-approve Bash."] }),
].map((one) => ({
  key: one.name,
  skill: one,
  program: one.level === "installed" ? "Claude Code" : undefined,
}));

/** The view with what was last done on it said under it, and a way out after it. */
function Skills({ starts = false, rows = OPS }: { starts?: boolean; rows?: ListedSkill[] }) {
  const [said, setSaid] = useState("Nothing done");
  const [only, setOnly] = useState(false);
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
            at={{ workspace: "ops", chat: true }}
            busy={false}
            onAct={(what) => setSaid(`Chose ${what}`)}
          />
        )}
        chat={{ only, onOnly: setOnly }}
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
 * list's first row, the arrows walk past the groups' names, Enter opens a skill's tab, the
 * keyboard on a row shows what it does and why it has its mark, Shift+F10 opens its menu, and
 * Tab leaves the list. The picture is a marked row under the keyboard.
 */
export const ByLevel: Story = {
  render: () => <Skills />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Skills" });
    for (const name of ["Needs you", "Built-in", "Project", "Workspace", "Claude Code"])
      expect(within(list).getByText(name)).toBeInTheDocument();
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
    const first = within(list).getByRole("row", { name: /drafted/ });
    await untilFocused(() => first, "Skills: Tab lands on the list's first row");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    const deploy = within(list).getByRole("row", { name: /^deploy/ });
    await untilFocused(() => deploy, "Skills: Down passes the groups' names");
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
    await untilFocused(() => off, "Skills: Escape gives the keyboard back to the row");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "After the view" }),
      "Skills: Tab leaves the list",
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
 * its fade and its placement done, of the page, as the menu is drawn past the view's edge.
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
