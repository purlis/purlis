// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Asks' scope chips** (ADR 0121, amended 2026-11-12, decisions 2 to 5): Chat, the focused chat
 * and every chat it dispatched; Workspace, the workspace in front, not drawn where the project has
 * one; Project, all of this project's. Each says its count above 0, and Asks opens on Chat. Chat
 * is greyed, with why, while no chat has focus, and an empty scope offers the nearest wider one.
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import { forgetAsksScope } from "./asksScope";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/stories/asks-scopes";

const permission = (session: number, says: string, since: number): Shown => ({
  session,
  ask: `permission-${session}`,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: session === 8 ? ["steward 8"] : ["steward 8", `dev ${session}`],
  answer: { via: "hook" },
  since,
});

const reviewer = (session: number, says: string, since: number): Shown => ({
  ...permission(session, says, since),
  chain: [`reviewer ${session}`],
});

/** Steward 8 dispatched dev 12 and dev 13; reviewer 30 is its sibling in the workspace; ops 60
 *  works in another workspace of the project. */
const PARENT: Record<number, number> = { 12: 8, 13: 8 };
const FILED: Record<number, string> = { 8: "app", 12: "app", 13: "app", 30: "app", 60: "infra" };

function Scoped({
  focused = true,
  oneWorkspace = false,
  only,
}: {
  /** Whether steward 8 has focus. */
  focused?: boolean;
  /** Whether the project has one workspace. */
  oneWorkspace?: boolean;
  /** The sessions whose asks wait, where not all of them. */
  only?: readonly number[];
}) {
  const [asks] = useState(() => {
    forgetAsksScope(PLANE);
    const now = Math.floor(Date.now() / 1000);
    const all = [
      permission(8, "Run cargo test -p purlis-core", now - 600),
      permission(12, "Run npm ci", now - 300),
      reviewer(30, "Run gh pr view 6310", now - 240),
      { ...reviewer(60, "Run terraform plan", now - 120), chain: ["ops 60"] },
    ];
    return only === undefined ? all : all.filter((ask) => only.includes(ask.session));
  });
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        part="asks"
        plane={PLANE}
        asks={asks}
        here="app"
        workspaceOf={(session) => FILED[session]}
        workspaceNamed={(key) => key}
        chat={focused ? { session: 8, name: "steward 8", tasks: 2 } : undefined}
        askedBy={(session) => PARENT[session]}
        oneWorkspace={oneWorkspace}
        onGo={() => {}}
        onLeave={() => {}}
      />
    </div>
  );
}

const meta = {
  component: Scoped,
} satisfies Meta<typeof Scoped>;

export default meta;
type Story = StoryObj<typeof meta>;

const chips = (canvasElement: HTMLElement) =>
  within(within(canvasElement).getByRole("radiogroup", { name: "Show what needs you in" }));
const rows = (canvasElement: HTMLElement) => [
  ...canvasElement.querySelectorAll<HTMLElement>('[role="row"].inbox-ask'),
];

/** **Opens on Chat**: Tab lands on Chat, which lists steward 8 and its task's asks; → picks
 *  Workspace, which adds its sibling's, and again Project, which adds the other workspace's. */
export const ChatWorkspaceProject: Story = {
  play: async ({ canvasElement }) => {
    const scopes = chips(canvasElement);
    await expect(scopes.getAllByRole("radio").map((one) => one.getAttribute("aria-label"))).toEqual(
      ["Chat, 2 waiting on you", "Workspace, 3 waiting on you", "Project, 4 waiting on you"],
    );
    // Each count the badge family's round count, in the ✋'s colour (#6342).
    await expect(
      [
        ...canvasElement.querySelectorAll<HTMLElement>('[data-scope="count"][data-part="round"]'),
      ].map((one) => one.dataset.drawn),
    ).toEqual(["2", "3", "4"]);
    await expect(rows(canvasElement)).toHaveLength(2);
    await userEvent.tab();
    await untilFocused(
      () => scopes.getByRole("radio", { name: /^Chat/ }),
      "Tab lands on Chat, picked",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => scopes.getByRole("radio", { name: /^Workspace/ }),
      "→ picks Workspace",
    );
    await expect(rows(canvasElement)).toHaveLength(3);
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => scopes.getByRole("radio", { name: /^Project/ }), "→ picks Project");
    await expect(rows(canvasElement)).toHaveLength(4);
    await userEvent.tab();
    await untilFocused(() => rows(canvasElement)[0], "Tab goes on into the list");
    expectFocusInSight();
  },
};

/** **One workspace**: no Workspace chip, which would list what Project lists. */
export const OneWorkspace: Story = {
  args: { oneWorkspace: true },
  play: async ({ canvasElement }) => {
    const scopes = chips(canvasElement);
    await expect(scopes.getAllByRole("radio").map((one) => one.getAttribute("aria-label"))).toEqual(
      ["Chat, 2 waiting on you", "Project, 4 waiting on you"],
    );
    await userEvent.tab();
    await untilFocused(() => scopes.getByRole("radio", { name: /^Chat/ }), "Tab lands on Chat");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => scopes.getByRole("radio", { name: /^Project/ }), "→ picks Project");
    await expect(rows(canvasElement)).toHaveLength(4);
  },
};

/** **No chat focused**: Chat is greyed and out of the keyboard's way; Asks lists Workspace. */
export const NoChatFocused: Story = {
  args: { focused: false },
  play: async ({ canvasElement }) => {
    const scopes = chips(canvasElement);
    await expect(scopes.getByRole("radio", { name: "Chat" })).toBeDisabled();
    await userEvent.tab();
    await untilFocused(
      () => scopes.getByRole("radio", { name: /^Workspace/ }),
      "Tab lands on Workspace, picked",
    );
    expectFocusInSight();
    await expect(rows(canvasElement)).toHaveLength(3);
  },
};

/** **Chat empty**: it says so and offers the nearest wider scope with something waiting, which
 *  one press widens to. */
export const EmptyChat: Story = {
  args: { only: [30, 60] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Nothing is waiting on you in this chat.")).toBeInTheDocument();
    await userEvent.tab();
    await untilFocused(() => chips(canvasElement).getByRole("radio", { name: "Chat" }), "Chat");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "1 waiting in this workspace" }),
      "Tab reaches the way out",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(rows(canvasElement)).toHaveLength(1);
  },
};
