// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A decision, answered on its row in Asks** (#5998, ADR 0121 amended 2026-11-09): its question
 * as its title, its chat leading the line under it, and its options on the row as a chat pane's
 * band draws them: the narrowest grant, Other scopes… and the refusal, then Go to chat. Other
 * scopes… opens the wider grants under the line and keeps the keyboard; Keep blocked opens "Why
 * keep it blocked?" without taking it. An ask that needs typing keeps its form behind a visible
 * Answer button, and its row says what it needs.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { runAsk } from "./secretAsk.testkit";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** A run of `curl` whose vault holds every key it is handed: nothing to type, only to decide. */
const RUN_GRANT: Shown = { ...runAsk({ fills: [] }), session: 2, chain: ["devops 2"] };

/** The same run, its vault lacking the key: a value to type first, behind Answer. */
const RUN_NEEDS_A_VALUE: Shown = { ...runAsk(), session: 3, chain: ["devops 3"] };

const HOST: Shown = {
  session: 4,
  ask: "block:4:connect:host:api.example.com",
  says: "Reach api.example.com",
  options: [
    { id: "you", label: "Allow for me on this machine", allows: true },
    { id: "chat", label: "Allow only for this chat", allows: true },
    { id: "project", label: "Allow for everyone in this project", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: ["steward 4"],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
  },
} as Shown;

const ASKS = { RUN_GRANT, RUN_NEEDS_A_VALUE, HOST } as const;

/** The Inbox as wide as the window's side of it, holding one ask. */
function OneAsk({ ask }: { ask: keyof typeof ASKS }) {
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox plane="/plane" asks={[ASKS[ask]]} onGo={() => {}} onLeave={() => {}} />
    </div>
  );
}

const meta = {
  component: OneAsk,
} satisfies Meta<typeof OneAsk>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The row itself, which Tab lands on. */
const rowIn = (canvasElement: HTMLElement) => () =>
  canvasElement.querySelector<HTMLElement>('[role="row"].inbox-ask');

/** **A run grant's question and its answers, on its row**: → walks Allow once, Other scopes…,
 *  Don't allow and Go to chat; Enter on Other scopes… opens the wider grants under the line and
 *  the keyboard stays on it; Esc gives the keyboard back to the row. */
export const ARunGrantOnItsRow: Story = {
  args: { ask: "RUN_GRANT" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const row = rowIn(canvasElement);
    await expect(canvasElement.querySelector(".inbox-question")).toHaveTextContent(
      "Allow curl to use vault team?",
    );
    await expect(canvas.queryByRole("button", { name: /^Answer/ })).toBeNull();
    await userEvent.tab();
    await untilFocused(row, "Tab lands on the row");
    expectFocusInSight();
    for (const name of ["Allow once", "Other scopes…", "Don't allow", "Go to chat devops 2"]) {
      await userEvent.keyboard("{ArrowRight}");
      await untilFocused(() => canvas.getByRole("button", { name }), `→ reaches ${name}`);
      expectFocusInSight();
    }
    await userEvent.keyboard("{ArrowLeft}{ArrowLeft}");
    const scopes = canvas.getByRole("button", { name: "Other scopes…" });
    await untilFocused(() => scopes, "← goes back to Other scopes…");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(scopes).toHaveAttribute("aria-expanded", "true"));
    await expect(canvas.getByRole("button", { name: "Allow for this chat" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Always allow" })).toBeVisible();
    await untilFocused(() => scopes, "the keyboard stays on Other scopes…");
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await untilFocused(row, "Esc gives the keyboard back to the row");
  },
};

/** **A sandbox host's row**: Allow only for this chat first, the wider grants under Other
 *  scopes…, and Keep blocked, whose press opens "Why keep it blocked?" and keeps the keyboard. */
export const ASandboxHostOnItsRow: Story = {
  args: { ask: "HOST" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvasElement.querySelector(".inbox-question")).toHaveTextContent(
      "Allow reaching this host?",
    );
    await expect(
      within(canvas.getByRole("group", { name: "Answers to steward 4" }))
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["Allow only for this chat", "Other scopes…", "Keep blocked"]);
    await expect(canvas.queryByRole("textbox")).toBeNull();
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
    const keep = canvas.getByRole("button", { name: "Keep blocked" });
    await untilFocused(() => keep, "→ reaches Keep blocked");
    await userEvent.keyboard("{Enter}");
    await expect(
      await canvas.findByRole("textbox", { name: /^Why you keep it blocked/ }),
    ).toBeVisible();
    await untilFocused(() => keep, "the box never takes the keyboard");
    expectFocusInSight();
  },
};

/** **A run whose vault lacks its value**: its row says what it needs, and its form stands behind
 *  a visible Answer button, which opens it with the caret in the value's box. */
export const ARunThatNeedsAValue: Story = {
  args: { ask: "RUN_NEEDS_A_VALUE" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Needs a value for STRIPE_API_KEY")).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "Don't allow" })).toBeNull();
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => canvas.getByRole("button", { name: "Answer devops 3" }),
      "→ reaches Answer",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("button", { name: "Cancel answering devops 3" })).toHaveAttribute(
        "aria-expanded",
        "true",
      ),
    );
  },
};
