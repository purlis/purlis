// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What happened to a task's question** (#4567): the fold under a question's line in its task's
 * Activity, folded as it is found, and open on each trail the reviewed prototype drew (#4561):
 * an agent answered, no reply and then you answered, the task ended unanswered. Then nothing
 * recorded, and a read that failed. Each is walked by the keyboard, held to axe in Chromium and
 * WebKit, and compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { QuestionStep } from "./bindings";
import { QuestionTrail } from "./QuestionTrail";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** A step as the core hands it, at `hh:mm` on one day. */
function step(at: string, over: Partial<QuestionStep> & Pick<QuestionStep, "step">): QuestionStep {
  return { at: `2026-11-02T${at}:00Z`, noted: false, why: null, by: null, what: null, ...over };
}

const AGENT_ANSWERED = [
  step("14:27", { step: "asked" }),
  step("14:27", { step: "shown" }),
  step("14:28", { step: "passed_on", noted: true }),
  step("14:28", { step: "reached", why: "passed_on" }),
  step("14:30", { step: "answered", by: "asking_chat" }),
  step("14:30", { step: "closed", by: "asking_chat" }),
];

const YOU_ANSWERED = [
  step("14:28", { step: "asked" }),
  step("14:28", { step: "shown" }),
  step("14:29", { step: "reached", why: "no_reply" }),
  step("14:34", { step: "answered", by: "person" }),
  step("14:34", { step: "closed", by: "person" }),
];

const TASK_ENDED = [
  step("14:28", { step: "asked" }),
  step("14:28", { step: "shown" }),
  step("14:41", { step: "closed", by: "nobody" }),
];

/** The fold as it stands in an Activity line's last column, reading `steps` or failing. */
function Fold({
  steps,
  refuses,
  open = true,
}: {
  steps?: QuestionStep[];
  refuses?: string;
  open?: boolean;
}) {
  return (
    <div style={{ inlineSize: 560 }}>
      <QuestionTrail
        task="developer 7"
        asker="steward 4"
        defaultExpanded={open}
        read={() =>
          refuses === undefined ? Promise.resolve(steps ?? []) : Promise.reject(new Error(refuses))
        }
      />
    </div>
  );
}

const meta = {
  component: Fold,
} satisfies Meta<typeof Fold>;

export default meta;
type Story = StoryObj<typeof meta>;

const trigger = (canvasElement: HTMLElement) =>
  within(canvasElement).getByRole("button", { name: "What happened" });

/** Tab reaches What happened, and Return folds and opens it again. */
async function walked(canvasElement: HTMLElement) {
  await userEvent.tab();
  await untilFocused(() => trigger(canvasElement), "Tab reaches What happened");
  expectFocusInSight();
  await userEvent.keyboard("{Enter}");
  await expect(trigger(canvasElement)).toHaveAttribute("aria-expanded", "false");
  await userEvent.keyboard("{Enter}");
  await expect(trigger(canvasElement)).toHaveAttribute("aria-expanded", "true");
  expectFocusInSight();
}

/** As it is found under a question: folded, nothing read. */
export const Folded: Story = {
  args: { open: false, steps: AGENT_ANSWERED },
  play: async ({ canvasElement }) => {
    await userEvent.tab();
    await untilFocused(() => trigger(canvasElement), "Tab reaches What happened");
    expectFocusInSight();
    await expect(trigger(canvasElement)).toHaveAttribute("aria-expanded", "false");
    await userEvent.keyboard("{Enter}");
    await within(canvasElement).findByText("developer 7 asked");
    await userEvent.keyboard("{Enter}");
    await expect(trigger(canvasElement)).toHaveAttribute("aria-expanded", "false");
  },
};

/** The asking chat passed it on with a note, and then answered it. */
export const AnAgentAnswered: Story = {
  args: { steps: AGENT_ANSWERED },
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByText("steward 4 answered");
    await walked(canvasElement);
    await within(canvasElement).findByText("steward 4 passed it on, with a note");
  },
};

/** The asking chat did not reply in time, it reached you, and you answered. */
export const YouAnswered: Story = {
  args: { steps: YOU_ANSWERED },
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByText("steward 4 did not reply in 100 seconds");
    await walked(canvasElement);
    await within(canvasElement).findByText("You answered");
  },
};

/** The task ended with its question unanswered. */
export const TheTaskEnded: Story = {
  args: { steps: TASK_ENDED },
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByText("The task ended, unanswered");
    await walked(canvasElement);
  },
};

/** A question asked before its steps were recorded. */
export const NothingRecorded: Story = {
  args: { steps: [] },
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByText(/purlis has no record of what happened/);
    await walked(canvasElement);
  },
};

/** The log could not be read: why, and Read again. */
export const NotRead: Story = {
  args: { refuses: "the event log is locked" },
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByText(/the event log is locked/);
    await walked(canvasElement);
    await within(canvasElement).findByRole("button", { name: "Read again" });
    await userEvent.tab();
    await untilFocused(
      () => within(canvasElement).getByRole("button", { name: "Read again" }),
      "Tab reaches Read again",
    );
    expectFocusInSight();
  },
};
