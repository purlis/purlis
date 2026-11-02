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
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";

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
  },
  {
    id: "g2",
    name: "pr-template",
    level: { at: "workspace", name: "smart-ide", local: false },
    label: "workspace smart-ide",
    days_left: 12,
    in_history: true,
    reserved: false,
  },
  {
    id: "g3",
    name: "scratch-notes",
    level: { at: "project", local: true },
    label: "local",
    days_left: 3,
    in_history: false,
    reserved: false,
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

/** Tab, until the keyboard is inside `target`, at most `steps` times. */
async function tabInto(target: () => Element | null, steps = 30): Promise<void> {
  for (let step = 0; step < steps; step++) {
    const into = target();
    if (into !== null && into.contains(document.activeElement)) return;
    await userEvent.tab();
  }
  await untilFocusWithin(target, "the keyboard reaches it by Tab");
}

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
