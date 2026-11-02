// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **New skill…** (#3684, `prototype/skill-changes` A): the dialog that asks how first, then the
 * short form of Write it yourself, its name checked as it is typed, and Ask a chat to draft it.
 * Drawn with the core faked at its command boundary, held to axe, walked by the keyboard, and
 * pictured in every pictured theme cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillLevel, StudioLevel } from "./bindings";
import { NewSkillDialog } from "./NewSkillDialog";
import { NAME_RESERVED } from "./SkillCopy";
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";

const PLANE = "/home/dev/plane";
const SMART_IDE: SkillLevel = { at: "workspace", name: "smart-ide", local: false };

/** The prototype's levels, seen from the workspace smart-ide. */
const LEVELS: StudioLevel[] = [
  { level: { at: "project", local: false }, label: "project" },
  { level: SMART_IDE, label: "workspace smart-ide" },
  { level: { at: "workspace", name: "git-ci-cd", local: false }, label: "workspace git-ci-cd" },
  { level: { at: "project", local: true }, label: "local project" },
];

function Shown() {
  const nothing = () => undefined;
  return (
    <NewSkillDialog
      plane={PLANE}
      levels={LEVELS}
      start={SMART_IDE}
      takenAt={() => ["release-notes"]}
      busy={false}
      onEdit={nothing}
      onMake={nothing}
      onDraft={nothing}
      onSources={nothing}
      onClose={nothing}
    />
  );
}

const meta = {
  component: Shown,
  // A dialog is drawn over the page, outside the story's frame: the page is pictured.
  parameters: { screenshot: "page" },
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "skill_templates")
        return [
          { id: { kind: "blank" }, label: "Blank", about: "A name, a description, a heading." },
          { id: { kind: "steps" }, label: "Steps to follow", about: "A numbered checklist." },
        ];
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

/** **How**: the three ways, the keyboard on the first. */
export const How: Story = {
  play: async () => {
    const dialog = await dialogNamed("New skill");
    const ways = within(dialog).getByRole("list", { name: "Ways to make a skill" });
    await expect(within(ways).getAllByRole("listitem")).toHaveLength(3);
    await untilFocusWithin(
      () => within(ways).getByRole("button", { name: "Write it yourself" }),
      "the dialog starts on its first way",
    );
    expectFocusInSight();
  },
};

/** **Write it yourself**: a built-in's name refused as it is typed, where it lands, a template. */
export const WriteItYourself: Story = {
  play: async () => {
    const asking = await dialogNamed("New skill");
    await userEvent.click(within(asking).getByRole("button", { name: "Write it yourself" }));
    const dialog = await dialogNamed("Write a new skill");
    const box = within(dialog).getByLabelText("Name");
    await untilFocusWithin(() => box, "the form starts on its name");
    await userEvent.type(box, "purlis-secrets");
    await expect(within(dialog).getByTestId("skill-new-problem")).toHaveTextContent(NAME_RESERVED);
    await expect(
      within(dialog).getByRole("button", { name: "Make it and open its page" }),
    ).toBeDisabled();
    await expect(within(dialog).getByRole("radio", { name: /^workspace smart-ide/ })).toBeChecked();
    expectFocusInSight();
  },
};

/** **Ask a chat to draft it**: what it should do, and where it lands. */
export const AskAChat: Story = {
  play: async () => {
    const asking = await dialogNamed("New skill");
    await userEvent.click(within(asking).getByRole("button", { name: "Ask a chat to draft it" }));
    const dialog = await dialogNamed("Ask a chat to draft a skill");
    const box = within(dialog).getByLabelText("What should it do?");
    await untilFocusWithin(() => box, "the form starts on what it should do");
    await userEvent.type(box, "Reviews a pull request against REVIEW.md");
    await expect(within(dialog).getByRole("button", { name: "Open the chat" })).toBeEnabled();
    expectFocusInSight();
  },
};
