// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Another open project's asks, answered in place** (#1921, ADR 0121 decisions 4 and 6): Asks
 * lists every open project's asks under All projects as rows of its own, in one order by facts.
 * A decision of project web shows its options on its row, a typed ask of project ops keeps a
 * visible Answer, and the same host asked by a chat here and a chat in web is one row.
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import type { Shown } from "./bindings";
import { Inbox } from "./Inbox";
import type { Elsewhere } from "./InboxElsewhere";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const host = (session: number, target: string, since: number): Shown => ({
  session,
  ask: `block:${session}:connect:host:${target}`,
  says: `The sandbox refused ${target}`,
  options: [
    { id: "chat", label: "Allow only for this chat", allows: true },
    { id: "you", label: "Allow for me on this machine", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: [`steward ${session}`],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target },
  },
  since,
});

const permission = (session: number, says: string, since: number): Shown => ({
  session,
  ask: `permission-${session}`,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`dev ${session}`],
  answer: { via: "hook" },
  since,
});

const question = (session: number, since: number): Shown => ({
  session,
  ask: `question:${session}`,
  says: "Which port?",
  options: [],
  source: "question",
  chain: [`ops ${session}`],
  answer: { via: "in-its-pane" },
  last_message: "Which port should the dev server use?",
  since,
});

/** This project with one host ask; web, in this window, with the same host and a command; ops,
 *  in another window, with a question. */
function EveryProject() {
  const [now] = useState(() => Math.floor(Date.now() / 1000));
  const elsewhere: Elsewhere = {
    asking: [],
    quiet: [],
    onPress: () => {},
    waitingHere: [
      { plane: "/plane", project: "plane", count: 1 },
      {
        plane: "/web",
        project: "web",
        count: 2,
        asks: [permission(5, "Run npm test", now - 720), host(3, "api.example.com", now - 60)],
        where: { "5": "site" },
      },
    ],
    there: [{ plane: "/ops", project: "ops", count: 1, asks: [question(8, now - 300)] }],
  };
  return (
    <div style={{ inlineSize: 420 }}>
      <Inbox
        part="asks"
        plane="/plane"
        asks={[host(3, "api.example.com", now - 120)]}
        elsewhere={elsewhere}
        onGo={() => {}}
        onLeave={() => {}}
      />
    </div>
  );
}

const meta = {
  component: EveryProject,
} satisfies Meta<typeof EveryProject>;

export default meta;
type Story = StoryObj<typeof meta>;

const rows = (canvasElement: HTMLElement) => [
  ...canvasElement.querySelectorAll<HTMLElement>('[role="row"].inbox-ask'),
];

/** **One list, every project's**: Tab passes the scope switch, on All projects, into the list;
 *  the longest wait first, whichever project; ↓ walks them and → reaches another project's Allow
 *  on its own row. */
export const EveryProjectInOneList: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("2 chats want to reach api.example.com")).toBeInTheDocument();
    await expect(rows(canvasElement)).toHaveLength(3);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("radio", { name: /^All projects/ }),
      "Tab lands on the scope switch, on All projects",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => rows(canvasElement)[0], "Tab lands on web's command, waiting longest");
    expectFocusInSight();
    await expect(within(rows(canvasElement)[0]).getByText("Run npm test")).toBeInTheDocument();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(rows(canvasElement)[0]).getByRole("button", { name: "Allow" }),
      "→ reaches its Allow, on its row",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows(canvasElement)[1], "↓ reaches ops's question");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(rows(canvasElement)[1]).getByRole("button", { name: /^Answer/ }),
      "→ reaches its visible Answer",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows(canvasElement)[2], "↓ reaches the host both projects ask for");
    expectFocusInSight();
  },
};
