// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **⌘⇧J's menu of the focused chat's own tasks** (#5058): its running tasks, which a pick
 * shows, and one that has finished, said by how it ended and passed by a pick. Opened here from
 * a stand-in for the pane's head, as the window opens it from the key. A menu draws outside
 * its frame, so each picture is the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useRef, useState } from "react";
import { expect, fn, screen, waitFor, within } from "storybook/test";
import { ChatTasksMenu, type ChatTask } from "./ChatTasksMenu";
import { Button } from "./ui/button/Button";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const TASKS: readonly ChatTask[] = [
  { kind: "running", session: 4, name: "talk" },
  { kind: "running", session: 5, name: "sweep" },
  { kind: "finished", id: "01K6", name: "probe", ended: "done" },
];

/** The menu under a pane's head, opened by a press of it, as ⌘⇧J opens it. */
function UnderAHead({ onShow }: { onShow: (session: number) => void }) {
  const head = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [from, setFrom] = useState<Element | null>(null);
  return (
    <>
      <Button
        ref={head}
        onPress={() => {
          setFrom(head.current);
          setOpen((was) => !was);
        }}
      >
        steward 1
      </Button>
      <ChatTasksMenu
        open={open}
        name="steward 1"
        tasks={TASKS}
        at={head}
        from={from}
        onShow={onShow}
        onClose={() => setOpen(false)}
      />
    </>
  );
}

const meta = {
  component: UnderAHead,
  args: { onShow: fn() },
  parameters: { screenshot: "page" },
} satisfies Meta<typeof UnderAHead>;

export default meta;
type Story = StoryObj<typeof meta>;

/** How long a menu may take to go: WebKit draws the next frame late in a busy run. */
const CLOSES = { timeout: 3_000 };

/**
 * **Walked by the keyboard**: it opens on its first task, the arrows reach the finished one,
 * whose Enter shows nothing, and Escape gives the keyboard back to where it was. The baseline
 * shows it open on the finished line.
 */
export const Walked: Story = {
  play: async ({ canvas, args, userEvent }) => {
    const head = canvas.getByRole("button", { name: "steward 1" });
    await userEvent.tab();
    await untilFocused(() => head, "Tab onto the head");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Tasks of steward 1" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /talk/ }),
      "It opens on its first task",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /probe/ }),
      "The arrows reach the finished task",
    );
    await userEvent.keyboard("{Enter}");
    await expect(args.onShow).not.toHaveBeenCalled();
    await expect(screen.getByRole("menu", { name: "Tasks of steward 1" })).toBeVisible();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await untilFocused(() => head, "Escape gives the keyboard back to the head");
    // Open again for the picture, on the finished line.
    await userEvent.keyboard("{Enter}");
    const again = await screen.findByRole("menu", { name: "Tasks of steward 1" });
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    await untilFocused(
      () => within(again).getByRole("menuitem", { name: /probe/ }),
      "Open again, on the finished task",
    );
  },
};

/** **A pick shows the task**: Enter on a running task hands its chat on, and the menu goes. */
export const Picked: Story = {
  play: async ({ canvas, args, userEvent }) => {
    const head = canvas.getByRole("button", { name: "steward 1" });
    await userEvent.tab();
    await untilFocused(() => head, "Tab onto the head");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Tasks of steward 1" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /talk/ }),
      "It opens on its first task",
    );
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await expect(args.onShow).toHaveBeenCalledWith(5);
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
  },
};
