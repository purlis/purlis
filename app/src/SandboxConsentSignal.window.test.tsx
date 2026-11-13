// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { SandboxState, Shown } from "./bindings";
import { askRows, forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";

/**
 * **A sandbox consent that waits is a Blocking ask** (#6000, ADR 0121 decision 2), on the real
 * window with the core mocked. Before, a consent ("Allow on this machine") was a Notice alone:
 * no ✋ count, no dot, no notification, and the status line's Notices count its only signal (the
 * operator's report of 2026-11-09). Now the core lists it among the project's asks, so the ✋
 * counts it and its row in Asks answers it. While it can be answered there, it is in Asks alone
 * (R-205): no Notice carries it beside its row.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";

const ITEM = {
  what: "service",
  target: "com.example.helper",
  named: "com.example.helper",
  says: "It is a service on this machine; purlis can't tell what it does.",
  refused: null,
} as const;

const WAITING: SandboxState = {
  on: true,
  offer: false,
  said: null,
  never: [],
  hosts_changed: null,
  presets_changed: null,
  presets: [],
  persona_hosts: [],
  besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
  policy: null,
  expects: { asked: true, locked: null, items: [ITEM] },
};

/** The consent as the core lists it among the project's asks (`asking::consents`). */
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
  answer: { via: "sandbox-expects", items: [ITEM] },
};

/** A core whose consent waits until the window answers it. */
function core() {
  const asked: { cmd: string; args: unknown }[] = [];
  let answered = false;
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [
          { name: "alpha", path: `${PLANE}/workspaces/alpha`, vision: "", todos: [], chats: [] },
        ],
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "plane_doctor") return { rows: [], app_rows: [], full: false, path: "" };
    if (cmd === "sandbox_state") return answered ? { ...WAITING, expects: null } : WAITING;
    if (cmd === "decline_project_expects" || cmd === "allow_project_expects") {
      answered = true;
      return { ...WAITING, expects: null };
    }
    if (cmd === "asks_waiting")
      return { plane: PLANE, asks: answered ? [] : [CONSENT], with_asking_chat: [] };
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return [];
    if (cmd === "opened_chats" || cmd === "chat_states" || cmd === "running_sessions") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return { asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args) };
}

const hand = () =>
  within(screen.getByRole("tablist", { name: "Attention" })).getByRole("tab", { name: /^Asks/ });
const handCount = () => hand().querySelector('[data-part="count"]');
const consentNotice = () =>
  screen.queryByRole("status", { name: "This project expects more than this machine allows" });
/** The consent's row in Asks. */
const consentRow = () =>
  askRows().find((one) => one.textContent?.includes("com.example.helper")) as
    HTMLElement | undefined;

beforeEach(() => inboxOpenAtLaunch());

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInboxOpen();
});

describe("a sandbox consent that waits (#6000)", () => {
  it("is counted on the ✋ and listed in Asks, its options on its row, and in no Notice", async () => {
    const { asked } = core();
    render(<App />);
    await waitFor(() => expect(handCount()).toHaveAttribute("data-count", "1"));
    await screen.findByRole("tabpanel", { name: "Asks" });
    const row = await waitFor(() => {
      const one = consentRow();
      expect(one).toBeDefined();
      return one as HTMLElement;
    });
    expect(within(row).getByRole("button", { name: "Allow on this machine" })).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Not now" })).toBeInTheDocument();
    // In Asks alone (R-205), once the Notices have read what the project expects.
    await screen.findByRole("tabpanel", { name: "Notices" });
    await waitFor(() => expect(asked("sandbox_state").length).toBeGreaterThan(0));
    expect(consentNotice()).toBeNull();
  });

  it("is settled by Not now on its row in Asks", async () => {
    const { asked } = core();
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Asks" });
    const row = await waitFor(() => {
      const one = consentRow();
      expect(one).toBeDefined();
      return one as HTMLElement;
    });

    await userEvent.click(within(row).getByRole("button", { name: "Not now" }));

    expect(asked("decline_project_expects")).toEqual([
      { plane: PLANE, shown: [{ target: ITEM.target, named: ITEM.named }] },
    ]);
    await waitFor(() => expect(consentRow()).toBeUndefined());
    await waitFor(() => expect(handCount()).toBeNull());
    await waitFor(() => expect(consentNotice()).toBeNull());
  });
});
