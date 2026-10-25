// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skills view** (SK-55, #3266), drawn as the left sidebar draws it: its header, the switch
 * to the chat in front, and this workspace's skills by level with Needs you on top, one mark at
 * most per row; then a project with no skill of its own, offered the studio's three starts. Each
 * is walked by the keyboard, held to axe and pictured in every pictured cell of the theme
 * matrix, light and dark among them (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { Menu, MenuItem, MenuWords } from "./ui/menu/Menu";
import { useListFilter } from "./ui/list/ListFilter";
import { SkillsList } from "./SkillsList";
import { groupsOf, menuRowsOf, type ListedSkill } from "./skillsListing";
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

/** The view with what was last done on it said under it. */
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
        gridTemplateRows: "minmax(0, 1fr) auto",
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
          <Menu aria-label={row.skill.name} onAction={(key) => setSaid(`Chose ${String(key)}`)}>
            {menuRowsOf(row, { workspace: "ops", chat: true }).map((one) => (
              <MenuItem
                key={one.act}
                id={one.act}
                textValue={one.title}
                aria-disabled={!one.available}
              >
                <MenuWords said={one.title} under={one.available ? undefined : one.reason} />
              </MenuItem>
            ))}
          </Menu>
        )}
        chat={{ only, onOnly: setOnly }}
      />
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

const meta = {
  component: Skills,
} satisfies Meta<typeof Skills>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **By level**: one Tab stop on the first row, the arrows walk past the groups' names, Enter
 * opens a skill's tab, the keyboard on a row shows what it does and why it has its mark, and
 * Shift+F10 opens its menu. The picture is a marked row under the keyboard.
 */
export const ByLevel: Story = {
  render: () => <Skills />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Skills" });
    const first = within(list).getByRole("row", { name: /drafted/ });
    for (const name of ["Needs you", "Built-in", "Project", "Workspace", "Claude Code"])
      expect(within(list).getByText(name)).toBeInTheDocument();
    first.focus();
    await untilFocused(() => first, "Skills: the first row has the keyboard");
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
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /Use in this chat/ }),
      "Skills: Shift+F10 opens the row's menu on its first row",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    await untilFocused(() => off, "Skills: Escape gives the keyboard back to the row");
    expectFocusInSight();
  },
};

/**
 * **A project with no skill of its own**: the studio's claim and its three starts over the
 * skills its chats already get, each start reached by Tab.
 */
export const NoSkillsOfItsOwn: Story = {
  render: () => <Skills starts rows={OPS.filter((row) => row.skill.level === "built-in")} />,
  play: async ({ canvas, userEvent }) => {
    const starts = canvas.getByRole("list", { name: "Ways to start a skill" });
    const draft = within(starts).getByRole("button", { name: "Ask a chat to draft a skill" });
    draft.focus();
    await untilFocused(() => draft, "Skills: the first start has the keyboard");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Started draft"));
    expectFocusInSight();
  },
};
