// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio's Deleted page and a deleted skill's page** (S9, #3691,
 * `prototype/studio-secondary-pages` variant A): the skills deleted in the last 30 days with the
 * days left, the one chosen beside the list with Bring back and its last text; narrow,
 * page-and-back; and the page with nothing on it. Walked by the keyboard, held to axe, and
 * pictured in every pictured theme cell. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DeletedSkill } from "./bindings";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { AT_THE_ROOT } from "./skillStudio";
import { studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, tabInto, untilFocusWithin, untilFocused } from "./stories.testkit";

/** The prototype's three deleted skills. */
const DELETED: DeletedSkill[] = [
  {
    id: "g1",
    name: "browser-old",
    level: { at: "project", local: false },
    label: "project",
    days_left: 27,
    in_history: true,
    reserved: false,
    clash: null,
    set: null,
  },
  {
    id: "g2",
    name: "pr-template",
    level: { at: "workspace", name: "smart-ide", local: false },
    label: "workspace smart-ide",
    days_left: 12,
    in_history: true,
    reserved: false,
    clash: null,
    set: null,
  },
  {
    id: "g3",
    name: "scratch-notes",
    level: { at: "project", local: true },
    label: "local",
    days_left: 3,
    in_history: false,
    reserved: false,
    clash: null,
    set: null,
  },
];

const TEXT =
  "---\nname: browser-old\ndescription: Drove a browser before the purlis-browser skill.\n---\n\n1. Open the page.\n2. Take a picture of it.\n";

/** The studio tab as a view tab draws it, `width` by 520 px, on Deleted at `item`. */
function Tab({
  width,
  narrow,
}: {
  width: number;
  narrow?: boolean;
  deleted: DeletedSkill[];
  /** The deleted skill chosen, by its id. */
  item?: string;
}) {
  return <StudioInPanel width={width} height={520} narrow={narrow} />;
}

const meta = {
  component: Tab,
  args: { width: 1000, deleted: DELETED, item: "g1" },
  beforeEach: ({ args }) => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio") return studio([]);
      if (cmd === "skills_deleted") return args.deleted;
      if (cmd === "skill_deleted_text") return TEXT;
      if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
      if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
      return [];
    });
    openViewAt(HISTORY, {
      page: "deleted",
      scope: AT_THE_ROOT,
      ...(args.item === undefined ? {} : { item: args.item }),
    });
    return () => {
      clearMocks();
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Wide: the list, and the one chosen beside it**, its last text opened. */
export const Wide: Story = {
  play: async ({ canvas }) => {
    const page = await canvas.findByTestId("skill-studio-deleted");
    const show = within(page).getByRole("button", { name: "Show its last text" });
    await tabInto(() => show);
    await userEvent.keyboard("{Enter}");
    await within(page).findByText(/Take a picture of it\./);
    const back = within(page).getByRole("button", { name: "Bring back browser-old" });
    await tabInto(() => back);
    expectFocusInSight();
  },
};

/** **Narrow: page-and-back.** One opened from the list is drawn alone with a way back. */
export const Narrow: Story = {
  args: { width: 460, narrow: true, item: undefined },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByTestId("deleted-browser-old");
    const root = canvasElement.querySelector<HTMLElement>('[data-part="root"][data-narrow]');
    if (root === null) throw new Error("the frame is not narrow");
    await expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1);
    await tabInto(() => canvas.getByRole("grid", { name: "Deleted skills" }));
    await userEvent.keyboard("{Enter}");
    await waitFor(() => canvas.getByRole("button", { name: "Back to Deleted" }));
    expectFocusInSight();
  },
};

/** The fourteen a bulk delete took, as one group (#5331). */
const SET = { id: "1762300800000", from: "mattpocock-skills", at: 1762300800 };
const GROUPED: DeletedSkill[] = [
  ...["grilling", "tdd", "to-spec"].map((name) => ({
    id: name,
    name,
    level: { at: "project", local: false } as const,
    label: "project",
    days_left: 30,
    in_history: true,
    reserved: false,
    clash: null,
    set: SET,
  })),
  ...DELETED,
];

/** **A set deleted together** (#5331): one group with Bring all back, reached by the keyboard, and
 *  each skill of it still chosen on its own. */
export const Group: Story = {
  args: { deleted: GROUPED, item: "grilling" },
  play: async ({ canvas }) => {
    const group = await canvas.findByTestId(`deleted-group-${SET.id}`);
    await expect(group).toHaveTextContent("from mattpocock-skills, 5 Nov");
    const all = within(group).getByRole("button", { name: /Bring all back/ });
    await canvas.findByRole("button", { name: "Bring back grilling" });
    // Into the list, up to the group's own row, and across to what it holds.
    await tabInto(() => canvas.getByRole("grid", { name: "Deleted skills" }));
    await userEvent.keyboard("{ArrowUp}");
    await untilFocusWithin(() => group, "the group's own row");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => all, "Bring all back");
    expectFocusInSight();
  },
};

/** **Nothing deleted**: what appears here, and the way on. */
export const Empty: Story = {
  args: { deleted: [], item: undefined },
  play: async ({ canvas }) => {
    const none = await canvas.findByTestId("skill-deleted-none");
    await expect(none).toHaveTextContent("Nothing deleted in the last 30 days.");
    const go = within(none).getByRole("button", { name: "Go to Library" });
    await tabInto(() => go);
    expectFocusInSight();
  },
};

/** A skill of the group whose name belongs to purlis now (#5457), and the name the core offers. */
const HELD: DeletedSkill = {
  id: "purlis-review",
  name: "purlis-review",
  level: { at: "project", local: false },
  label: "project",
  days_left: 30,
  in_history: true,
  reserved: true,
  clash: {
    held_by: "the name purlis-review belongs to purlis",
    offered: "mattpocock-skills-purlis-review",
    taken: ["grilling", "tdd", "to-spec"],
  },
  set: SET,
};

/** **One whose name belongs to purlis now** (#3729, #5457): its page asks for the new name it
 *  comes back under, filled in as an install fills a taken one, and refuses one it cannot take
 *  as it is typed. */
export const NameTaken: Story = {
  args: { deleted: [HELD, ...DELETED], item: "purlis-review" },
  play: async ({ canvas }) => {
    const page = await canvas.findByTestId("skill-studio-deleted");
    const box = within(page).getByRole("textbox", { name: "New name for purlis-review" });
    await expect(box).toHaveValue("mattpocock-skills-purlis-review");
    await tabInto(() => box);
    await userEvent.clear(box);
    await userEvent.type(box, "tdd");
    await within(page).findByText("There is already a skill named tdd here.");
    expectFocusInSight();
    await userEvent.clear(box);
    await userEvent.type(box, "review-mine");
    const back = within(page).getByRole("button", {
      name: "Bring back purlis-review as review-mine",
    });
    await tabInto(() => back);
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    expectFocusInSight();
  },
};

/** **Bring all back with a name purlis holds now** (#5457): the group's one review, where that
 *  skill gets its own new name and the rest come back as named. */
export const GroupNameTaken: Story = {
  args: { deleted: [...GROUPED.slice(0, 3), HELD, ...DELETED], item: undefined },
  play: async ({ canvas }) => {
    const group = await canvas.findByTestId(`deleted-group-${SET.id}`);
    const all = within(group).getByRole("button", { name: /Bring all back/ });
    await tabInto(() => canvas.getByRole("grid", { name: "Deleted skills" }));
    await untilFocusWithin(() => group, "the group's own row");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => all, "Bring all back");
    await userEvent.keyboard("{Enter}");
    const review = await canvas.findByTestId("set-review");
    const rename = within(review).getByRole("radio", { name: "Bring it back under a new name" });
    // A radio group is one stop for Tab; the arrows move within it, and choose as they move.
    await tabInto(() => rename.closest('[role="radiogroup"]'));
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(rename).toBeChecked());
    const box = await within(review).findByRole("textbox", { name: "New name for purlis-review" });
    await expect(box).toHaveValue("mattpocock-skills-purlis-review");
    await tabInto(() => box);
    expectFocusInSight();
    await tabInto(() => within(review).getByRole("button", { name: "Bring back 4 skills" }));
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    expectFocusInSight();
  },
};
