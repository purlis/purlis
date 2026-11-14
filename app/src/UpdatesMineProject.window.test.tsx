// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox, type UpdateRow, type Updates } from "./Inbox";
import { Notice } from "./Notice";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import type { InboxUpdate } from "./bindings";

/**
 * **Updates: Mine and Project** (#5646, ADR 0119): a switch of two lists. Mine (From your chats)
 * holds what the person's own chats and tasks did; Project (From the project) holds what is the
 * project's, the teammate Notices among it. It opens on Mine, and keeps the scope switch.
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

const update = (
  key: string,
  kind: InboxUpdate["kind"],
  says: string,
  session: number | null,
  read = false,
): InboxUpdate => ({
  key,
  kind,
  at: AT,
  session,
  chain: session === null ? [] : [`steward ${session}`],
  says,
  read,
});

/** Chat 3 works in alpha, chat 5 in beta. */
const FILED: Record<number, string> = { 3: "alpha", 5: "beta" };
const strip = {
  here: "alpha",
  workspaceOf: (session: number) => FILED[session],
  workspaceNamed: (key: string) => key,
};

const ROWS: UpdateRow[] = [
  update("task:a", "task-done", "Fix the build: done", 3),
  update("task:b", "task-failed", "Ship the docs: failed", 5),
  update("doctor:x:warn", "doctor", "personas: reviewer has no charter", null),
].map((one) => ({ update: one, dismiss: vi.fn() }));

const updates = (over: Partial<Updates> = {}): Updates => ({
  rows: ROWS,
  onMarkAllRead: vi.fn(),
  onDismissAll: vi.fn(),
  ...over,
});

/** A teammate's change to the project, as the hosts' Notice says it. */
const HOSTS = "The hosts chats in this project may reach have changed in the project's settings.";
const teammates = (
  <Notice
    cause="sandbox-hosts"
    label="The project's hosts changed"
    fixes={[{ label: "Got it", onPress: () => {} }]}
  >
    <p>{HOSTS}</p>
  </Notice>
);

function draw(more: Partial<Parameters<typeof Inbox>[0]> = {}) {
  return render(
    <Inbox
      part="updates"
      plane={PLANE}
      asks={[]}
      onGo={vi.fn()}
      onLeave={vi.fn()}
      updates={updates()}
      project={teammates}
      {...more}
    />,
  );
}

const lists = () => within(screen.getByRole("radiogroup", { name: "Show updates from" }));
const scopes = () => within(screen.getByRole("radiogroup", { name: "Show updates in" }));

describe("Updates' Mine and Project (#5646)", () => {
  it("opens on Mine, listing what the person's own chats did and nothing of the project's", () => {
    draw();
    expect(lists().getByRole("radio", { name: "Mine, from your chats" })).toBeChecked();
    expect(lists().getByRole("radio", { name: "Project, from the project" })).not.toBeChecked();
    expect(screen.getByText(/Fix the build: done/)).toBeInTheDocument();
    expect(screen.getByText(/Ship the docs: failed/)).toBeInTheDocument();
    expect(screen.queryByText(/reviewer has no charter/)).toBeNull();
    expect(screen.queryByText(HOSTS)).toBeNull();
  });

  it("lists the project's lines and the teammate Notices under Project, and none of Mine", async () => {
    draw();
    await userEvent.click(lists().getByRole("radio", { name: "Project, from the project" }));
    expect(screen.getByText(/reviewer has no charter/)).toBeInTheDocument();
    expect(screen.getByText(HOSTS)).toBeInTheDocument();
    expect(screen.queryByText(/Fix the build: done/)).toBeNull();
  });

  it("says Mine is empty in its own words, while Project still has lines", () => {
    draw({ updates: updates({ rows: ROWS.filter((one) => one.update.kind === "doctor") }) });
    expect(screen.getByText("Nothing new from your chats")).toBeInTheDocument();
  });

  it("marks read and dismisses only the lines of the list shown", async () => {
    const shown = updates();
    draw({ updates: shown });
    await userEvent.click(screen.getByRole("button", { name: "Mark all read" }));
    await userEvent.click(screen.getByRole("button", { name: "Dismiss all" }));
    expect(shown.onMarkAllRead).toHaveBeenCalledWith(["task:a", "task:b"]);
    expect(shown.onDismissAll).toHaveBeenCalledWith(["task:a", "task:b"]);
  });

  it("keeps the scope switch, said Workspace and Project: this project's updates, or this workspace's alone", async () => {
    draw(strip);
    expect(
      scopes()
        .getAllByRole("radio")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Workspace, 1 update", "Project, 2 updates"]);
    await userEvent.click(scopes().getByRole("radio", { name: "Workspace, 1 update" }));
    expect(screen.getByText(/Fix the build: done/)).toBeInTheDocument();
    expect(screen.queryByText(/Ship the docs: failed/)).toBeNull();
  });
});
