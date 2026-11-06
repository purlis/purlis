// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Library's Select mode and the Deleted page's** (#5332, `prototype/library-select` variant
 * B, the selection page): Select puts a box on each row, the page beside the list becomes the
 * selection (the count and how many the list does not show, every action with how many it
 * reaches, the names), and an action's review takes its place. Walked by the keyboard (Select,
 * Space and the arrows, ⌘A, an action, Esc), held to axe, and pictured in every pictured theme
 * cell. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Studio } from "./bindings";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { inWorkspace, type StudioPage, type StudioScope } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { expectFocusInSight, tabInto, untilFocusWithin } from "./stories.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { updateOffer, updateReview } from "./skillUpdates.testkit";

const MP = "mattpocock-skills";
const SMART_IDE = { at: "workspace" as const, name: "smart-ide" };

/** The prototype's library, cut short: the project's own, a source's, and built-in ones. */
const SKILLS = [
  skill("deploy", {
    description: "Deploys a service to staging or production with the checks it needs.",
    pinned: true,
    pinned_by: [SMART_IDE],
  }),
  skill("grilling", {
    source_label: MP,
    description: "Grills the person about a plan until the gaps show.",
  }),
  skill("tdd", { source_label: MP, description: "Test-driven development: red, green, refactor." }),
  skill("research", {
    source_label: MP,
    on: false,
    turned_off: true,
    left_out: "turned off in smart-ide",
    description: "Investigates a question against primary sources.",
  }),
  skill("domain-modeling", {
    source_label: MP,
    archived: true,
    description: "Builds and sharpens a project's domain model.",
  }),
  skill("purlis-browser", {
    level: "built-in",
    label: "built-in",
    grant: null,
    granted: null,
    description: "Drives a real browser for a task that needs one.",
  }),
  skill("purlis-secrets", {
    level: "built-in",
    label: "built-in",
    grant: null,
    granted: null,
    description: "Uses a credential from a vault without showing it.",
  }),
];

const SET = { id: "s1", from: MP, at: 1_762_300_800 };
const gone = (name: string, over: Record<string, unknown> = {}) => ({
  id: `${name}-id`,
  name,
  level: { at: "project", local: false },
  label: "project",
  days_left: 27,
  in_history: true,
  reserved: false,
  set: null,
  ...over,
});
const DELETED = [
  gone("grilling", { set: SET, days_left: 30 }),
  gone("tdd", { set: SET, days_left: 30 }),
  gone("browser-old"),
  gone("scratch-notes", { in_history: false, label: "local", days_left: 3 }),
];

/** The studio tab as a view tab draws it, `width` by 520 px. */
function Tab({
  width,
}: {
  width: number;
  /** Where an entry point opened it: the page, and whose chats. */
  page: StudioPage;
  scope: StudioScope;
  answer: Studio;
}) {
  return <StudioInPanel width={width} height={520} />;
}

const meta = {
  component: Tab,
  args: {
    width: 1000,
    page: "library",
    scope: inWorkspace("smart-ide"),
    answer: studio(SKILLS, {
      workspace: "smart-ide",
      workspaces: ["smart-ide", "git-ci-cd"],
      levels: [
        { level: { at: "project", local: false }, label: "project" },
        {
          level: { at: "workspace", name: "smart-ide", local: false },
          label: "workspace smart-ide",
        },
      ],
    }),
  },
  beforeEach: ({ args }) => {
    mockIPC((cmd, given) => {
      const asked = (given ?? {}) as Record<string, unknown>;
      if (cmd === "skill_studio") return args.answer;
      if (cmd === "skills_deleted") return DELETED;
      if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
      // Review updates (#5354): grilling's adds a hook, tdd's edits its text.
      if (cmd === "skill_updates")
        return {
          offers: [updateOffer("grilling"), updateOffer("tdd")],
          unasked: [],
          unconfirmed: [],
        };
      if (cmd === "skill_update_review") {
        const name = asked.name as string;
        return updateReview(name, name === "grilling" ? ["hook"] : []);
      }
      if (cmd === "skill_set_review") {
        const members = asked.members as { name: string }[];
        return {
          skills: members.map((member) => ({ member, changed: false, cannot: null })),
          names: members.some((one) => one.name === "grilling")
            ? [
                {
                  namer: "to-spec",
                  named: "grilling",
                  member: members.find((one) => one.name === "grilling"),
                },
              ]
            : [],
          open_chats: 3,
        };
      }
      return [];
    });
    openViewAt(HISTORY, { page: args.page, scope: args.scope });
    return () => {
      clearMocks();
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Presses Select from the keyboard, and answers the list once its rows hold boxes. */
async function selectOn(canvas: ReturnType<typeof within>, testid = "skill-library-select") {
  await tabInto(() => canvas.getByTestId(testid));
  await userEvent.keyboard("{Enter}");
  await canvas.findByTestId("select-none");
}

/**
 * **Chosen with the keyboard**: Select, then Space on a row and Shift+↓ for the next, which the
 * selection page counts, each action with how many it reaches.
 */
export const Selected: Story = {
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Skills" });
    await selectOn(canvas);
    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    await userEvent.keyboard("{ArrowDown} {Shift>}{ArrowDown}{ArrowDown}{/Shift}");
    expectFocusInSight();
    const page = await canvas.findByTestId("select-page");
    await waitFor(() => expect(page).toHaveTextContent("3 selected"));
    await expect(within(page).getByTestId("select-act-delete")).toHaveTextContent("3 of 3");
    await expect(within(page).getByTestId("select-act-turn-on")).toHaveTextContent("1 of 3");
  },
};

/**
 * **⌘A**: every skill the search and filters keep, drawn or not, the built-in ones folded away
 * among them, which the page counts as not shown and names first.
 */
export const EverythingSelected: Story = {
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Skills" });
    await selectOn(canvas);
    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    await userEvent.keyboard("{Meta>}a{/Meta}");
    const page = await canvas.findByTestId("select-page");
    await waitFor(() => expect(page).toHaveTextContent("7 selected, 2 not shown"));
    await expect(within(page).getByTestId("select-names-not-shown")).toBeInTheDocument();
    await tabInto(() => within(page).getByTestId("select-acts"));
    expectFocusInSight();
  },
};

/**
 * **Delete's review in the selection page's place**: who is left naming one that goes, with Keep,
 * the open chats, and the built-in skills, which stay and say so. Esc on the list leaves Select,
 * as Cancel goes back to the selection first.
 */
export const DeleteReview: Story = {
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Skills" });
    await selectOn(canvas);
    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    await userEvent.keyboard("{Meta>}a{/Meta}");
    const page = await canvas.findByTestId("select-page");
    const del = within(page).getByRole("button", { name: "Delete…" });
    await tabInto(() => del);
    await userEvent.keyboard("{Enter}");
    const review = await canvas.findByTestId("set-review");
    await expect(within(review).getByRole("heading", { name: "Delete 5 skills?" })).toBeVisible();
    await expect(within(review).getByTestId("set-stays-purlis-browser")).toHaveTextContent(
      "Built-in skills can't be changed.",
    );
    await expect(await within(review).findByTestId("set-open-chats")).toBeInTheDocument();
    await expect(within(review).getByRole("button", { name: "Keep grilling" })).toBeVisible();
    await tabInto(() => within(review).getByRole("button", { name: "Keep grilling" }));
    expectFocusInSight();
  },
};

/**
 * **Review updates' review in the selection page's place** (#5354): the update that adds a hook
 * first and unticked, its box out of reach until its changes are opened; the plain edit ticked;
 * and those with no update waiting, or not from a source, staying as they are. Opened and ticked
 * from the keyboard.
 */
export const UpdatesReview: Story = {
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Skills" });
    await selectOn(canvas);
    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    await userEvent.keyboard("{Meta>}a{/Meta}");
    const page = await canvas.findByTestId("select-page");
    await tabInto(() => within(page).getByRole("button", { name: "Review updates…" }));
    await userEvent.keyboard("{Enter}");
    const review = await canvas.findByTestId("update-set-review");
    const hooky = await within(review).findByRole("checkbox", {
      name: "Update grilling (project)",
    });
    await expect(hooky).not.toBeChecked();
    await expect(hooky).toBeDisabled();
    await expect(
      within(review).getByRole("checkbox", { name: "Update tdd (project)" }),
    ).toBeChecked();
    await expect(within(review).getByTestId("set-stays-deploy")).toHaveTextContent(
      "It wasn't installed from a source.",
    );
    await tabInto(() => within(review).getByRole("button", { name: "What changes in grilling" }));
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(hooky).toBeEnabled());
    await tabInto(() => hooky);
    await userEvent.keyboard(" ");
    expectFocusInSight();
    await waitFor(() => expect(hooky).toBeChecked());
    await expect(within(review).getByRole("button", { name: "Accept 2 updates" })).toBeVisible();
  },
};

/** **Move's review**: it asks where they go first, and the main action waits for it. */
export const MoveReview: Story = {
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Skills" });
    await selectOn(canvas);
    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    await userEvent.keyboard("{Meta>}a{/Meta}");
    const page = await canvas.findByTestId("select-page");
    await tabInto(() => within(page).getByRole("button", { name: "Move to another level…" }));
    await userEvent.keyboard("{Enter}");
    const review = await canvas.findByTestId("set-review");
    await expect(within(review).getByRole("button", { name: "Move" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await tabInto(() => within(review).getByRole("radiogroup"));
    await userEvent.keyboard("{ArrowDown}");
    expectFocusInSight();
    await waitFor(() =>
      expect(
        within(canvas.getByTestId("set-review")).getByRole("button", { name: "Move" }),
      ).not.toHaveAttribute("aria-disabled", "true"),
    );
  },
};

/** **Nothing chosen**: how to choose, in the page's place. Esc on the list leaves Select, and
 *  Select again starts it afresh. */
export const NothingSelected: Story = {
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Skills" });
    await selectOn(canvas);
    await tabInto(() => canvas.getByRole("grid", { name: "Skills" }));
    await userEvent.keyboard(" ");
    await canvas.findByTestId("select-page");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(canvas.queryByTestId("select-page")).toBeNull());
    await untilFocusWithin(() => canvas.getByRole("grid", { name: "Skills" }), "the list keeps it");
    await selectOn(canvas);
    expectFocusInSight();
  },
};

/** **The Deleted page's Select**: a group's row ticks every skill of it, and Bring back and Delete
 *  now each say how many they reach. */
export const DeletedSelected: Story = {
  args: { page: "deleted" },
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Deleted skills" });
    await selectOn(canvas, "skill-deleted-select");
    await tabInto(() => canvas.getByRole("grid", { name: "Deleted skills" }));
    await userEvent.keyboard(" ");
    const page = await canvas.findByTestId("select-page");
    await waitFor(() => expect(page).toHaveTextContent("2 selected"));
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown} ");
    await waitFor(() => expect(page).toHaveTextContent("3 selected"));
    expectFocusInSight();
  },
};

/** **Delete now's review**: the one action on the page with no undo, which says so, and which
 *  skills leave only this list and which leave this machine for good. */
export const DeleteNowReview: Story = {
  args: { page: "deleted" },
  play: async ({ canvas }) => {
    await canvas.findByRole("grid", { name: "Deleted skills" });
    await selectOn(canvas, "skill-deleted-select");
    await tabInto(() => canvas.getByRole("grid", { name: "Deleted skills" }));
    await userEvent.keyboard("{Meta>}a{/Meta}");
    const page = await canvas.findByTestId("select-page");
    await tabInto(() => within(page).getByRole("button", { name: "Delete now…" }));
    await userEvent.keyboard("{Enter}");
    const review = await canvas.findByTestId("set-review");
    await expect(review).toHaveTextContent("This can't be undone.");
    await tabInto(() => within(review).getByRole("button", { name: "Delete 4 skills now" }));
    expectFocusInSight();
  },
};
