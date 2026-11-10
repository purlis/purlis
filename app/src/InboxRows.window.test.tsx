// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox, type UpdateRow, type Updates } from "./Inbox";
import { Notice } from "./Notice";
import { drawnWith } from "./cascade.testkit";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import type { InboxUpdate, Shown } from "./bindings";

/**
 * **Messages are rows of their view, without borders** (#5649, spec #5638, ADR 0119 decision 6):
 * Asks, Mine, Project and Notices draw every line with the one row of the message family, never
 * a boxed card. Each row says its state by a tinted glyph and its words, has its title and its
 * time at the right, and, where it can be put away, a × that takes the time's place under the
 * pointer or the keyboard. Driven as the person reads it: the views, their rows, their words.
 */

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2026-11-07T12:00:00Z"));
  mockIPC(() => null);
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

const PLANE = "/home/dev/plane";
const AT = Math.floor(Date.parse("2026-11-07T11:00:00Z") / 1000);

const ASKS: Shown[] = [
  {
    session: 3,
    ask: "01J0A",
    says: "Run cargo test",
    options: [
      { id: "allow", label: "Allow", allows: true },
      { id: "deny", label: "Deny", allows: false },
    ],
    source: "permission",
    chain: ["steward 3"],
    answer: { via: "hook" },
  },
  {
    session: 6,
    ask: "question:6",
    says: "Waiting on your reply",
    options: [],
    source: "question",
    chain: ["steward 6"],
    answer: { via: "in-its-pane" },
    last_message: "Which branch?",
  },
];

const update = (key: string, kind: InboxUpdate["kind"], says: string, session: number | null) => ({
  key,
  kind,
  at: AT,
  session,
  chain: session === null ? [] : [`steward ${session}`],
  says,
  read: false,
});

const ROWS: UpdateRow[] = [
  update("task:a", "task-done", "Fix the build: done", 3),
  update("task:b", "task-failed", "Ship the docs: failed", 5),
  update("resumed:c", "resumed", "came back", 7),
  update("doctor:x:warn", "doctor", "personas: reviewer has no charter", null),
].map((one) => ({ update: one, dismiss: vi.fn() }));

const UPDATES: Updates = { rows: ROWS, onMarkAllRead: vi.fn(), onDismissAll: vi.fn() };

const teammates = (
  <Notice
    cause="sandbox-hosts"
    label="The project's hosts changed"
    fixes={[{ label: "Got it", onPress: () => {} }]}
  >
    <p>The hosts chats in this project may reach have changed in the project's settings.</p>
  </Notice>
);

const notices = (
  <>
    <Notice
      cause="runner-key"
      tone="trouble"
      label="Runner key changed"
      fixes={[{ label: "Pair again", onPress: () => {} }]}
      onDismiss={() => {}}
    >
      The runner no longer matches the key purlis paired with.
    </Notice>
    <Notice cause="browsers-at-quit" onDismiss={() => {}}>
      2 browsers were open at quit.
    </Notice>
  </>
);

function draw(part: "asks" | "updates" | "notices") {
  return render(
    <Inbox
      part={part}
      plane={PLANE}
      asks={ASKS}
      onGo={vi.fn()}
      onLeave={vi.fn()}
      onIgnore={vi.fn()}
      updates={UPDATES}
      project={teammates}
      notices={notices}
    />,
  );
}

/** Every line the view lists, by its row of the message family. */
const rows = () =>
  [...document.querySelectorAll<HTMLElement>('[data-scope="message"][data-part="root"]')].filter(
    (one) => one.closest("section.inbox") !== null,
  );

/** What the view lists that is drawn as a card, or as a Notice's own box. */
const boxes = () =>
  document.querySelectorAll('[data-scope="card"], [data-scope="notice"][data-part="root"]');

const part = (row: HTMLElement, name: string) =>
  row.querySelector<HTMLElement>(`[data-scope="message"][data-part="${name}"]`);

describe("every line of the Inbox's views is a row, without a border (#5649)", () => {
  it("draws Asks as rows, each in the waiting state, its question or its chat its title and its age at the right", () => {
    draw("asks");
    const listed = rows();
    expect(listed).toHaveLength(2);
    expect(boxes()).toHaveLength(0);
    for (const row of listed) {
      expect(row).toHaveAttribute("data-variant", "waiting");
      expect(drawnWith(row, "border")).toBe("0");
      expect(part(row, "mark")?.querySelector("svg")).not.toBeNull();
      expect(part(row, "time")).not.toBeNull();
    }
    // A decision's title is its question (#5998); a chat waiting on a reply's is the chat.
    expect(listed.map((row) => part(row, "title")?.textContent)).toEqual([
      "Allow this action?",
      "steward 6",
    ]);
  });

  it("marks the chat each ask names Waiting on you, as its row and its tab do (#6012)", () => {
    draw("asks");
    for (const row of rows()) {
      const mark = part(row, "title")?.querySelector('[data-scope="chat-mark"][data-part="root"]');
      expect(mark).toHaveAttribute("data-variant", "needs");
      expect(mark?.querySelector('[data-part="badge"] svg')).toHaveClass("lucide-hand");
    }
  });

  it("draws Mine as rows: done, failed or a report by its mark, its time at the right, a × to dismiss", () => {
    draw("updates");
    const listed = rows();
    expect(boxes()).toHaveLength(0);
    expect(listed.map((row) => row.getAttribute("data-variant"))).toEqual([
      "done",
      "failed",
      "report",
    ]);
    for (const row of listed) {
      expect(part(row, "time")?.querySelector("time")).toHaveAttribute("datetime");
      expect(row).toHaveAttribute("data-dismiss");
      expect(
        within(part(row, "dismiss") as HTMLElement).getByRole("button", { name: /^Dismiss: / }),
      ).toBeTruthy();
    }
  });

  it("draws Project as rows, the teammate Notices among them", async () => {
    draw("updates");
    await userEvent.click(screen.getByRole("radio", { name: "Project, from the project" }));
    await waitFor(() => expect(rows()).toHaveLength(2));
    expect(boxes()).toHaveLength(0);
    expect(rows().map((row) => part(row, "title")?.textContent)).toEqual(
      expect.arrayContaining(["personas: reviewer has no charter", "The project's hosts changed"]),
    );
  });

  it("draws Notices as rows: trouble failed by its mark, its fix a link, its × over its time", async () => {
    draw("notices");
    await waitFor(() => expect(rows()).toHaveLength(2));
    expect(boxes()).toHaveLength(0);
    const [trouble, note] = rows();
    expect(trouble).toHaveAttribute("data-variant", "failed");
    expect(part(trouble, "title")).toHaveTextContent("Runner key changed");
    expect(
      within(part(trouble, "links") as HTMLElement).getByRole("button", { name: "Pair again" }),
    ).toHaveAttribute("data-variant", "quiet-link");
    expect(note).toHaveAttribute("data-variant", "plain");
    for (const row of rows()) {
      expect(row).toHaveAttribute("data-dismiss");
      expect(within(part(row, "dismiss") as HTMLElement).getByRole("button", { name: "Dismiss" }));
    }
  });

  it("gives a row's time to its × under the pointer or the keyboard, in every view", () => {
    draw("updates");
    const row = rows()[0];
    const time = part(row, "time") as HTMLElement;
    const dismiss = part(row, "dismiss") as HTMLElement;
    expect(drawnWith(dismiss, "opacity")).toBe("0");
    // The pointer on the row, as the cascade marks it.
    row.setAttribute("data-cascade-hover", "");
    expect(drawnWith(time, "visibility", { hover: true })).toBe("hidden");
    expect(drawnWith(dismiss, "opacity", { hover: true })).toBe("1");
    row.removeAttribute("data-cascade-hover");
  });
});
