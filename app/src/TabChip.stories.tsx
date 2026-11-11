// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's tasks, from its pane head's state word** (#5341): the word "waiting on 2 tasks" as the
 * button of the menu a tab's chip opened, which ⌘⇧J opens too: the chat first, its tasks under
 * who asked for them, End a task, Activity, and the summary of the counts at its foot. Opened
 * here by a press, as the window opens it, and drawn once it has the keyboard: a menu draws
 * outside its frame, so each picture is the page's.
 */
import { PaneNumber } from "./LayoutPanes";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, screen, waitFor, within } from "storybook/test";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { ChatRow } from "./chatsTree";
import { waitingOnTasks } from "./shownState";
import { StateShown } from "./StateShown";
import { stateClock } from "./stateClock";
import { ChatTasks } from "./TabChip";
import type { Offer } from "./actions";
import { untilFocused } from "./stories.testkit";

function row(session: number, level: number, name: string): ChatRow {
  return {
    session,
    name,
    persona: level === 1 ? "steward" : "devops",
    workspace: "alpha",
    shell: false,
    parent: level === 1 ? null : 1,
    mode: level === 1 ? null : "task",
    from: null,
    tab: level === 1,
    branch: null,
    report: level === 1 ? null : "owed",
    outcome: null,
    asking: null,
    harness: "Claude Code",
    level,
    posinset: 1,
    setsize: 1,
    orphaned: false,
  };
}

const ROWS = [row(1, 1, "steward 1"), row(4, 2, "talk"), row(5, 2, "sweep")];

/** The two rows that end a task, as the catalogue words them. */
const endsOf = (session: number): Offer[] =>
  ["Stop and get its report", "Close now"].map((title, at) => ({
    id: `task.end:${session}:${at}`,
    title,
    available: true,
    does: { verb: "noop" },
  })) as unknown as Offer[];

/** The head's state line, its word the button of the chat's tasks. */
function OnAHead({ onShow }: { onShow: (session: number) => void }) {
  return (
    <ChatsHere.Provider
      value={fixedChats({
        ...nothingKnown,
        bySession: { 1: "waiting", 4: "running", 5: "waiting" },
      })}
    >
      <div className="pane-head">
        <div className="pane-chips">
          <span className="pane-name">steward 1</span>
          <PaneNumber session={1} />
          <span className="pane-state-mark">
            <ChatTasks
              id="tasks-of-steward-1"
              name="steward 1"
              rows={ROWS}
              ended={[]}
              current={1}
              asked={0}
              clock={stateClock()}
              dragging={() => false}
              onShow={onShow}
              ends={endsOf}
              onPress={() => undefined}
              onActivity={() => undefined}
            >
              <StateShown shown={waitingOnTasks(2)} />
            </ChatTasks>
          </span>
        </div>
      </div>
    </ChatsHere.Provider>
  );
}

const meta = {
  component: OnAHead,
  args: { onShow: fn() },
  parameters: { screenshot: "page" },
} satisfies Meta<typeof OnAHead>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **At rest**: the head's word, which says the chat waits on its tasks, and opens them. */
export const AtRest: Story = {
  play: async ({ canvas }) => {
    const word = canvas.getByRole("button", { name: /waiting on 2 tasks/ });
    await expect(word).toHaveAttribute("aria-haspopup", "menu");
    await expect(word).toHaveAccessibleDescription("Tasks of steward 1: 1 working, 1 waiting");
  },
};

/**
 * **Opened from the keyboard**: on the chat's first task, each task with its mark, End a task,
 * Activity and the summary at its foot. A pick shows the task, and Escape gives the keyboard back.
 * The baseline shows it open on its first task.
 */
export const Opened: Story = {
  play: async ({ canvas, args, userEvent }) => {
    const word = canvas.getByRole("button", { name: /waiting on 2 tasks/ });
    await userEvent.tab();
    await untilFocused(() => word, "Tab onto the head's word");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Tasks of steward 1" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /talk/ }),
      "It opens on the chat's first task",
    );
    // Once the menu has come in: a row is not visible while its surface fades in.
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: "End a task" })).toBeVisible(),
    );
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: "Activity" })).toBeVisible(),
    );
    await expect(screen.getByTestId("tasks-summary")).toHaveTextContent(/1 working, 1 waiting/);
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    await untilFocused(() => word, "Escape gives the keyboard back to the word");
    await userEvent.keyboard("{Enter}");
    const again = await screen.findByRole("menu", { name: "Tasks of steward 1" });
    await untilFocused(
      () => within(again).getByRole("menuitem", { name: /talk/ }),
      "Open again, on its first task",
    );
    await userEvent.keyboard("{Enter}");
    await expect(args.onShow).toHaveBeenCalledWith(4);
    // And open once more, for the picture.
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    word.focus();
    await userEvent.keyboard("{Enter}");
    const shown = await screen.findByRole("menu", { name: "Tasks of steward 1" });
    await untilFocused(
      () => within(shown).getByRole("menuitem", { name: /talk/ }),
      "Open for the picture, on its first task",
    );
  },
};
