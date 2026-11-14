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
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { runAsk } from "./secretAsk.testkit";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { SETTLE_MS } from "./noticeSettle";

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

/** What the project expects of this machine, still to be allowed (#6000): a consent of no chat's. */
const CONSENT: Shown = {
  session: 0,
  ask: "sandbox-expects",
  says: "This project expects more than this machine allows: the system service com.example.helper",
  options: [
    { id: "allow", label: "Allow on this machine", allows: true },
    { id: "not-now", label: "Not now", allows: false },
  ],
  source: "sandbox-expects",
  chain: ["purlis"],
  answer: {
    via: "sandbox-expects",
    items: [
      {
        what: "service",
        target: "com.example.helper",
        named: "com.example.helper",
        says: "It is a service on this machine.",
        refused: null,
      },
    ],
  },
};

/** steward's tools, changed since this machine allowed them (ADR 0121, amended 2026-11-12). */
const PERSONA_ALLOW: Shown = {
  session: 0,
  ask: "persona-allow:steward",
  says:
    "You allowed steward's tools on this machine and they have changed since, so no chat gets " +
    "them until you allow them again. Allowed, chats as steward run gh with token from vault " +
    "devops as GH_TOKEN.",
  options: [
    { id: "allow", label: "Allow for steward chats", allows: true },
    { id: "not-now", label: "Not now", allows: false },
  ],
  source: "persona-allow",
  chain: ["steward"],
  answer: { via: "persona-allow", persona: "steward", digest: "steward-digest-2" },
};

/** `purlis profile sign-in --pool` waiting on the person's yes in the window (#6000). */
const CONFIRMATION: Shown = {
  session: 0,
  ask: "confirmation:7",
  says: "Asks to sign work in on pool team. Its terminal shows the code A1B2C3.",
  options: [
    { id: "yes", label: "Sign in", allows: true },
    { id: "no", label: "Cancel", allows: false },
  ],
  source: "confirmation",
  chain: ["purlis profile sign-in"],
  answer: { via: "confirmation", id: 7 },
};

/** A permission that pushes to main, a protected branch (#6011): high risk, asked once more. */
const PUSH: Shown = {
  session: 1,
  ask: "ask-1",
  says: "Run git push origin main",
  options: [
    { id: "allow", label: "Allow once", allows: true },
    { id: "always", label: "Allow always", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: ["steward 1"],
  answer: { via: "hook" },
  risk: {
    class: "protected-push",
    why: "Pushes to main, a protected branch",
    confirm: "Allow a push to main?",
  },
};

/** A task paused at its time limit (#6015): Continue, Raise the budget or Stop. */
const BUDGET: Shown = {
  session: 7,
  ask: "paused:7",
  says:
    "Budget reached: It had worked 30 minutes and a task may work 30 minutes here (minutes " +
    "per task: working time only, not time waiting on the person or on its own tasks). The " +
    "person sets that limit in Settings › Project › Dispatch.",
  options: [
    { id: "continue", label: "Continue", allows: true },
    { id: "raise", label: "Raise the budget", allows: true },
    { id: "stop", label: "Stop", allows: false },
  ],
  source: "budget",
  chain: ["steward 7"],
  answer: { via: "paused" },
  // Stamped when the story draws, `PAUSED_FOR` before then (`OneAsk`): a fixed instant would make
  // the row's age, and so its pictures, change every day.
  since: null,
};

/** How long the paused chats have waited when the story draws: 2½ hours, read as 2h, with half
 *  an hour either way before the picture could change. */
const PAUSED_FOR = 9_000;

/** A chat paused as looping (#6015): It's fine, continue, or Stop. */
const LOOPING: Shown = {
  ...BUDGET,
  session: 8,
  ask: "paused:8",
  says: "It may be looping: the same command failed 6 times in a row.",
  options: [
    { id: "fine", label: "It's fine, continue", allows: true },
    { id: "stop", label: "Stop", allows: false },
  ],
  source: "looping",
  chain: ["steward 8"],
};

/** A chat paused at 3 blocks on the same target (T1-36, R-216): Tell it to stop trying, the lead,
 *  then Let it keep trying and Stop chat. */
const BLOCK_LOOP: Shown = {
  ...LOOPING,
  session: 9,
  ask: "paused:9",
  says: "steward 9 was blocked 3 times on the same thing. Let it keep trying?",
  options: [
    { id: "stop-trying", label: "Tell it to stop trying", allows: false, preselected: true },
    { id: "keep-trying", label: "Let it keep trying", allows: true },
    { id: "stop", label: "Stop chat", allows: false },
  ],
  chain: ["steward 9"],
};

/** A permission that deletes a folder (#6178): high risk, asked once more. */
const DELETE: Shown = {
  ...PUSH,
  ask: "ask-2",
  says: "Run rm -rf build",
  risk: { class: "delete", why: "Deletes build", confirm: "Allow deleting build?" },
};

/** A secret fill (#6178): a vault's key into a page, answered in its form, asked once more. */
const VAULT_FILL: Shown = {
  session: 7,
  ask: "secret-fill:1",
  says: "Wants to put dash-api-key from vault qa into a field on dash.example.test",
  options: [
    { id: "once", label: "Allow once", allows: true, preselected: null },
    { id: "keep", label: "Keep blocked", allows: false, preselected: null },
  ],
  source: "secret-fill",
  chain: ["steward 7"],
  answer: { via: "secret-fill", id: 1, shown: "digest" },
  fill: {
    key: "dash-api-key",
    vault: "qa",
    site: "dash.example.test",
    page: "https://dash.example.test/settings",
    label: "API key",
    browser: "b1",
    warnings: [],
    card: false,
    multi_line: false,
    vault_too: false,
    code: false,
  },
  risk: {
    class: "vault-use",
    why: "Hands dash-api-key from vault qa to a field on dash.example.test",
    confirm: "Allow handing dash-api-key to a field on dash.example.test?",
  },
};

const ASKS = {
  RUN_GRANT,
  RUN_NEEDS_A_VALUE,
  HOST,
  CONSENT,
  PERSONA_ALLOW,
  CONFIRMATION,
  PUSH,
  BUDGET,
  LOOPING,
  BLOCK_LOOP,
  DELETE,
  VAULT_FILL,
} as const;

/** The Inbox as wide as the window's side of it, holding one ask. */
function OneAsk({ ask }: { ask: keyof typeof ASKS }) {
  const [now] = useState(() => Math.floor(Date.now() / 1000));
  const shown = ASKS[ask];
  const stamped = shown.answer?.via === "paused" ? { ...shown, since: now - PAUSED_FOR } : shown;
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox plane="/plane" asks={[stamped]} onGo={() => {}} onLeave={() => {}} />
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
 *  the keyboard stays on it. */
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

/** **What a project expects, on its row** (#6000): a consent of no chat's, asked here alone
 *  (R-205), its two answers on the row, with no Go to chat; → walks them. */
export const AConsentOnItsRow: Story = {
  args: { ask: "CONSENT" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvasElement.querySelector(".inbox-question")).toHaveTextContent(
      "Allow what this project expects?",
    );
    await expect(canvas.queryByRole("button", { name: /^Go to chat/ })).toBeNull();
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    for (const name of ["Allow on this machine", "Not now"]) {
      await userEvent.keyboard("{ArrowRight}");
      await untilFocused(() => canvas.getByRole("button", { name }), `→ reaches ${name}`);
      expectFocusInSight();
    }
  },
};

/** **A persona's Allow, on its row** (ADR 0121, amended 2026-11-12): one ask for the persona, of
 *  no chat's, its Notice's Allow and Not now on the row, with no Go to chat; → walks them. */
export const APersonasAllowOnItsRow: Story = {
  args: { ask: "PERSONA_ALLOW" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvasElement.querySelector(".inbox-question")).toHaveTextContent(
      "Allow this persona's hosts and tools?",
    );
    await expect(canvas.queryByRole("button", { name: /^Go to chat/ })).toBeNull();
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    for (const name of ["Allow for steward chats", "Not now"]) {
      await userEvent.keyboard("{ArrowRight}");
      await untilFocused(() => canvas.getByRole("button", { name }), `→ reaches ${name}`);
      expectFocusInSight();
    }
  },
};

/** **A command's confirmation, on its row** (#6000): named by its command, its code in its line,
 *  its yes and Cancel on the row; → walks them. */
export const ACommandsConfirmationOnItsRow: Story = {
  args: { ask: "CONFIRMATION" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvasElement.querySelector(".inbox-question")).toHaveTextContent(
      "Confirm what this command asks?",
    );
    await expect(canvas.getByText(/A1B2C3/)).toBeVisible();
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    for (const name of ["Sign in", "Cancel"]) {
      await userEvent.keyboard("{ArrowRight}");
      await untilFocused(() => canvas.getByRole("button", { name }), `→ reaches ${name}`);
      expectFocusInSight();
    }
  },
};

/** **A high-risk ask's row** (ADR 0121 decision 8, #6011): its risk said first, and its Allow
 *  asks once more under the line once the row has settled, keeping the keyboard where it was,
 *  and Cancel takes the question back. */
export const AHighRiskAskOnItsRow: Story = {
  args: { ask: "PUSH" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const row = rowIn(canvasElement);
    await expect(canvasElement.querySelector(".inbox-risk")).toHaveTextContent(
      "Pushes to main, a protected branch",
    );
    await userEvent.tab();
    await untilFocused(row, "Tab lands on the row");
    await userEvent.keyboard("{ArrowRight}");
    const allow = canvas.getByRole("button", { name: "Allow once" });
    await untilFocused(() => allow, "→ reaches Allow once");
    expectFocusInSight();
    // The person reads the row first: a grant pressed as it is drawn is held (ADR 0097).
    await new Promise((done) => setTimeout(done, SETTLE_MS + 50));
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(allow).toHaveAttribute("aria-expanded", "true"));
    const again = canvas.getByRole("group", { name: "Allow a push to main?" });
    await expect(again).toBeVisible();
    await untilFocused(() => allow, "the keyboard stays on Allow once");
    expectFocusInSight();
    await userEvent.click(within(again).getByRole("button", { name: "Cancel" }));
    await waitFor(() =>
      expect(canvas.queryByRole("group", { name: "Allow a push to main?" })).toBeNull(),
    );
  },
};

/** **A chat paused at its budget, on its row** (#6015): Continue, Raise the budget and Stop on the
 *  row, then Go to chat; → walks them. */
export const ABudgetReachedOnItsRow: Story = {
  args: { ask: "BUDGET" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvasElement.querySelector(".inbox-question")).toHaveTextContent(
      "Let it go on past its budget?",
    );
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    for (const name of ["Continue", "Raise the budget", "Stop", "Go to chat steward 7"]) {
      await userEvent.keyboard("{ArrowRight}");
      await untilFocused(() => canvas.getByRole("button", { name }), `→ reaches ${name}`);
      expectFocusInSight();
    }
  },
};

/** **A chat paused as looping, on its row** (#6015): It's fine, continue and Stop on the row;
 *  → walks them. */
export const ALoopOnItsRow: Story = {
  args: { ask: "LOOPING" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvasElement.querySelector(".inbox-question")).toHaveTextContent("Let it go on?");
    await expect(canvas.getByText(/failed 6 times in a row/)).toBeVisible();
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    for (const name of ["It's fine, continue", "Stop"]) {
      await userEvent.keyboard("{ArrowRight}");
      await untilFocused(() => canvas.getByRole("button", { name }), `→ reaches ${name}`);
      expectFocusInSight();
    }
  },
};

/** **A block loop's pause, on its row** (T1-36, R-216, R-217): the chat and the count, then Tell it
 *  to stop trying, Let it keep trying and Stop chat. The lead is the primary answer, where ⌘⇧I
 *  lands; → walks the three in that order. */
export const ABlockLoopOnItsRow: Story = {
  args: { ask: "BLOCK_LOOP" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByText("steward 9 was blocked 3 times on the same thing. Let it keep trying?"),
    ).toBeVisible();
    const lead = canvas.getByRole("button", { name: "Tell it to stop trying" });
    await expect(lead).toHaveAttribute("data-variant", "primary");
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    for (const name of ["Tell it to stop trying", "Let it keep trying", "Stop chat"]) {
      await userEvent.keyboard("{ArrowRight}");
      await untilFocused(() => canvas.getByRole("button", { name }), `→ reaches ${name}`);
      expectFocusInSight();
    }
  },
};

/** **A delete's row** (#6178): "High risk" and what it deletes said first, and its Allow asks
 *  once more under the line, as a protected push's does. */
export const ADeleteOnItsRow: Story = {
  args: { ask: "DELETE" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvasElement.querySelector(".inbox-risk")).toHaveTextContent(
      "High risk Deletes build",
    );
    await userEvent.tab();
    await untilFocused(rowIn(canvasElement), "Tab lands on the row");
    await userEvent.keyboard("{ArrowRight}");
    const allow = canvas.getByRole("button", { name: "Allow once" });
    await untilFocused(() => allow, "→ reaches Allow once");
    await new Promise((done) => setTimeout(done, SETTLE_MS + 50));
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(allow).toHaveAttribute("aria-expanded", "true"));
    await expect(canvas.getByRole("group", { name: "Allow deleting build?" })).toBeVisible();
    expectFocusInSight();
  },
};

/** **A vault use's row** (#6178): its risk said first; in the form its Answer opens, Allow once
 *  asks once more ("Allow handing dash-api-key to…?") before it fills anything, and Cancel takes
 *  the question back. */
export const AVaultUseOnItsRow: Story = {
  args: { ask: "VAULT_FILL" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvasElement.querySelector(".inbox-risk")).toHaveTextContent(
      "Hands dash-api-key from vault qa to a field on dash.example.test",
    );
    await userEvent.click(canvas.getByRole("button", { name: /^Answer/ }));
    const allow = await canvas.findByRole("button", { name: "Allow once" });
    await new Promise((done) => setTimeout(done, SETTLE_MS + 50));
    await userEvent.click(allow);
    await waitFor(() => expect(allow).toHaveAttribute("aria-expanded", "true"));
    const again = canvas.getByRole("group", {
      name: "Allow handing dash-api-key to a field on dash.example.test?",
    });
    await expect(again).toBeVisible();
    expectFocusInSight();
    await userEvent.click(within(again).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(allow).toHaveAttribute("aria-expanded", "false"));
  },
};
