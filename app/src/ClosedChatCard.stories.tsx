// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A closed or finished chat's card** (#6214, parent #5820), built from the accepted prototype's
 * `ClosedChatCardFromALink` and `ClosedChatCardOutcomes` (`prototype/pane-links`, S6, D7, D8):
 * the popover beside the row it came from, with *Read it* first and *Reopen* plain, and the card
 * in each outcome, including a task whose screen was not kept. In Chromium and WebKit, in every
 * cell of the theme matrix, with axe.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useRef, useState } from "react";
import { expect, screen, userEvent, waitFor, within } from "storybook/test";
import type { ChatCard } from "./bindings";
import { ClosedChatCard, ClosedChatCardPopover } from "./ClosedChatCard";
import { Button } from "./ui/button/Button";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** "#641 · Signed commits #5607", finished 11:08 (the prototype's `CLOSED_CHAT`). */
const FINISHED: ChatCard = {
  number: 641,
  chat: "01K6W0RKER000000000000000B",
  started_by: "steward 4",
  screen_kept: true,
  task: {
    id: "01K9DISPATCH0000000000000A",
    asker: 0,
    name: "Signed commits #5607",
    chat: null,
    persona: "steward",
    how: "done",
    outcome: "done",
    folds: true,
    report:
      "Done. Commits are signed on every lane: the app signs with the machine's SSH key, and CI checks each signature before the merge queue takes a PR. Two lanes had unsigned history, rewritten and pushed. #5607 is ready to close.",
    changed: null,
    // Local time, so every cell says 11:08 whatever its clock's zone.
    ended: "2026-11-11T11:08:00",
    place: "smart-ide",
    branch: null,
    reopens: true,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
  },
};

const BLOCKED: ChatCard = {
  ...FINISHED,
  number: 633,
  task: {
    ...FINISHED.task,
    name: "Docker pool on a full disk",
    how: "blocked",
    outcome: "blocked",
    folds: false,
    ended: "2026-11-11T09:41:00",
    report:
      "Blocked: the Docker VM's disk is full (ENOSPC). It needs the person to prune images; nothing else was changed.",
  },
};

const CLOSED: ChatCard = {
  ...FINISHED,
  number: 629,
  started_by: "you",
  screen_kept: false,
  task: {
    ...FINISHED.task,
    name: "talk",
    how: "closed_by_person",
    outcome: "closed by you",
    folds: false,
    ended: "2026-11-11T08:15:00",
    report: "",
    reopens: false,
  },
};

/** The palette's finished row, standing in: a press opens the card beside it. */
function FromTheRow({ card }: { card: ChatCard }) {
  const row = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [said, setSaid] = useState("");
  return (
    <div style={{ display: "grid", gap: "var(--space-4)", justifyItems: "start" }}>
      <Button ref={row} onPress={() => setOpen(true)}>
        Finished chat #641 · Signed commits #5607
      </Button>
      <p data-testid="said" style={{ margin: 0 }}>
        {said}
      </p>
      <ClosedChatCardPopover
        card={card}
        triggerRef={row}
        isOpen={open}
        onOpenChange={setOpen}
        onRead={() => {
          setOpen(false);
          setSaid("Read it: its kept screen, in a view tab.");
        }}
        onReopen={() => {
          setOpen(false);
          setSaid("Reopen: its conversation, as a new chat.");
        }}
      />
    </div>
  );
}

const meta = {
  title: "Chats/Closed chat card",
  component: FromTheRow,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof FromTheRow>;
export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **The card from the switcher's finished row** (D7): pressed, it opens beside the row and
 * takes the keyboard. Tab goes to *Read it*, then *Reopen*; Escape takes it down and hands the
 * keyboard back to the row. *Read it* is pressed from the keyboard.
 */
export const FromTheSwitchersRow: Story = {
  args: { card: FINISHED },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const row = canvas.getByRole("button", { name: /Finished chat #641/ });
    await userEvent.click(row);
    const card = await screen.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    // Drawn once the popover has come in.
    await waitFor(() => expect(within(card).getByText("Finished")).toBeVisible());
    await expect(within(card).getByText("steward 4")).toBeVisible();
    await userEvent.tab();
    const read = within(card).getByRole("button", { name: "Read it" });
    await untilFocused(() => read, "Tab to Read it");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => within(card).getByRole("button", { name: "Reopen" }), "Tab to Reopen");
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await untilFocused(() => row, "Escape hands the keyboard back to the row");
    await userEvent.keyboard("{Enter}");
    const again = await screen.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    await userEvent.tab();
    await untilFocused(() => within(again).getByRole("button", { name: "Read it" }), "Read it");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByTestId("said")).toHaveTextContent(/Read it/));
  },
};

/** **A card whose task's screen was not kept**: *Read it* is off and says why. */
export const ScreenNotKept: Story = {
  args: { card: CLOSED },
  play: async ({ canvasElement }) => {
    await userEvent.click(within(canvasElement).getByRole("button", { name: /Finished chat/ }));
    const card = await screen.findByRole("dialog", { name: "#629 · talk" });
    // The family keeps a disabled button focusable, so its reason is heard: aria-disabled.
    await expect(within(card).getByRole("button", { name: "Read it" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    // Said in words under the answers, besides the button's own description.
    await waitFor(() =>
      expect(within(card).getByText("Its screen was not kept.", { selector: "p" })).toBeVisible(),
    );
    await expect(within(card).getByText("It sent no report.")).toBeVisible();
  },
};

/** **The card in each outcome** (D8): Finished, Blocked, and Closed with no report and no kept
 *  screen, as the switcher's finished row opens them. */
export const Outcomes: Story = {
  args: { card: FINISHED },
  render: () => (
    <div style={{ display: "grid", gap: "var(--space-4)", inlineSize: "26rem" }}>
      {[FINISHED, BLOCKED, CLOSED].map((card) => (
        <section key={card.number} aria-labelledby={`card-${card.number}`}>
          <ClosedChatCard
            card={card}
            titleId={`card-${card.number}`}
            onRead={() => {}}
            onReopen={() => {}}
          />
        </section>
      ))}
    </div>
  ),
};
