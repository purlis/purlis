// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Rename, Move and Delete a skill** (#3685, `prototype/skill-changes`): each dialog with what
 * changes and who is affected before the person confirms. Drawn with the core faked at its
 * command boundary, held to axe, walked by the keyboard, and pictured in every pictured theme
 * cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillLevel, StudioLevel, StudioSkill } from "./bindings";
import { DeleteDialog, MoveDialog, RenameDialog } from "./SkillChangeDialogs";
import { NAME_RESERVED } from "./SkillCopy";
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";

const PLANE = "/home/dev/plane";
const PROJECT: SkillLevel = { at: "project", local: false };

/** The prototype's `deploy`, a project skill. */
const DEPLOY: StudioSkill = {
  source_label: null,
  name: "deploy",
  level: "project",
  label: "project",
  local: false,
  grant: "all",
  granted: { to: "all" },
  turned_off: false,
  not_granted: null,
  on: true,
  left_out: null,
  accept: null,
  description: "Deploys a service to staging or production with the checks the team agreed on.",
  folder: "skills/deploy",
  written: "deploy",
  named: false,
  unchecked: null,
  owner: null,
  switch: null,
  compat: [],
  lint: [],
  hidden: [],
  pinned: false,
  pinned_by: [],
};

/** The prototype's levels. */
const LEVELS: StudioLevel[] = [
  { level: PROJECT, label: "project" },
  { level: { at: "workspace", name: "smart-ide", local: false }, label: "workspace smart-ide" },
  { level: { at: "workspace", name: "git-ci-cd", local: false }, label: "workspace git-ci-cd" },
  { level: { at: "project", local: true }, label: "local project" },
];

/** Which dialog a story draws. */
type Which = "rename" | "move" | "delete";

function Shown({ which }: { which: Which }) {
  const nothing = () => undefined;
  if (which === "rename")
    return (
      <RenameDialog
        plane={PLANE}
        skill={DEPLOY}
        level={PROJECT}
        taken={["review-pr"]}
        builtIns={["browser", "secrets"]}
        busy={false}
        onEdit={nothing}
        onRename={nothing}
        onClose={nothing}
      />
    );
  if (which === "move")
    return (
      <MoveDialog
        plane={PLANE}
        skill={DEPLOY}
        from={PROJECT}
        levels={LEVELS}
        workspaces={["smart-ide", "git-ci-cd"]}
        busy={false}
        onEdit={nothing}
        onMove={nothing}
        onClose={nothing}
      />
    );
  return (
    <DeleteDialog
      plane={PLANE}
      skill={DEPLOY}
      level={PROJECT}
      busy={false}
      onDelete={nothing}
      onClose={nothing}
    />
  );
}

const meta = {
  component: Shown,
  args: { which: "rename" },
  // A dialog is drawn over the page, outside the story's frame: the page is pictured.
  parameters: { screenshot: "page" },
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "skill_users")
        return {
          root: true,
          workspaces: ["smart-ide", "git-ci-cd"],
          persona: null,
          personas: ["steward", "reviewer"],
          skills: ["release-notes"],
          pinned_by: [{ at: "workspace", name: "smart-ide" }],
        };
      if (cmd === "skill_move_needs_grant") return false;
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Shown>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The dialog open, by its name. */
async function dialogNamed(name: string): Promise<HTMLElement> {
  return waitFor(() => within(document.body).getByRole("dialog", { name }));
}

/** **Rename**: what changes with it, and a built-in's name refused as it is typed. */
export const Rename: Story = {
  play: async () => {
    const dialog = await dialogNamed("Rename deploy");
    const reach = await within(dialog).findByRole("region", { name: "Changed with it" });
    await waitFor(() => expect(reach).toHaveTextContent("2 personas that name it"));
    const box = within(dialog).getByLabelText("New name");
    await untilFocusWithin(() => box, "the dialog starts on its field");
    await userEvent.clear(box);
    await userEvent.type(box, "secrets");
    await expect(within(dialog).getByTestId("skill-rename-problem")).toHaveTextContent(
      NAME_RESERVED,
    );
    await expect(within(dialog).getByRole("button", { name: "Rename" })).toBeDisabled();
    expectFocusInSight();
  },
};

/** **Move**: each level with who gets it there, and what changes for the one picked. */
export const Move: Story = {
  args: { which: "move" },
  play: async () => {
    const dialog = await dialogNamed("Move deploy to another level");
    await userEvent.click(within(dialog).getByRole("radio", { name: /^workspace smart-ide/ }));
    const changes = await within(dialog).findByRole("region", { name: "What changes" });
    await waitFor(() =>
      expect(changes).toHaveTextContent(
        "Chats at the project's root, chats in git-ci-cd stop getting it.",
      ),
    );
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Move" })).toBeEnabled());
    expectFocusInSight();
  },
};

/** **Delete**: who uses it now, and that it waits in Deleted for 30 days. */
export const Delete: Story = {
  args: { which: "delete" },
  play: async () => {
    const dialog = await dialogNamed("Delete deploy?");
    const users = await within(dialog).findByTestId("skill-users");
    await waitFor(() =>
      expect(users).toHaveTextContent("Chats at the project's root, chats in smart-ide"),
    );
    // An alert of what can't be taken back starts on Cancel, never on Delete.
    await expect(within(dialog).getByRole("button", { name: "Delete" })).not.toHaveFocus();
    expectFocusInSight();
  },
};
