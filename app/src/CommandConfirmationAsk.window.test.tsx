// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { Shown } from "./bindings";
import { askRows, forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";

/**
 * **A `purlis` command's confirmation is a Blocking ask** (#6000, ADR 0121 decision 2), on the
 * real window with the core mocked: `purlis profile sign-in --pool` waiting on the person's yes is
 * counted on the ✋ and listed in Asks, where its row answers it as its dialog does.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";

/** The confirmation as the core lists it among the project's asks (`asking::consents`). */
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
  held_until: Math.floor(Date.now() / 1000) + 120,
};

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
    if (cmd === "answer_pool_sign_in") {
      answered = true;
      return null;
    }
    if (cmd === "asks_waiting")
      return { plane: PLANE, asks: answered ? [] : [CONFIRMATION], with_asking_chat: [] };
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return [];
    if (cmd === "opened_chats" || cmd === "chat_states" || cmd === "running_sessions") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return { asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args) };
}

const handCount = () =>
  within(screen.getByRole("tablist", { name: "Attention" }))
    .getByRole("tab", { name: /^Asks/ })
    .querySelector('[data-part="count"]');
const confirmationRow = () =>
  askRows().find((one) => one.textContent?.includes("A1B2C3")) as HTMLElement | undefined;

beforeEach(() => inboxOpenAtLaunch());

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInboxOpen();
});

describe("a command's confirmation in the window (#6000)", () => {
  it("is counted on the ✋, listed in Asks by its command, and answered from its row", async () => {
    const { asked } = core();
    render(<App />);
    await waitFor(() => expect(handCount()).toHaveAttribute("data-count", "1"));
    await screen.findByRole("tabpanel", { name: "Asks" });
    const row = await waitFor(() => {
      const one = confirmationRow();
      expect(one).toBeDefined();
      return one as HTMLElement;
    });
    expect(row).toHaveTextContent("purlis profile sign-in");
    expect(within(row).getByRole("button", { name: "Sign in" })).toBeInTheDocument();

    await userEvent.click(within(row).getByRole("button", { name: "Cancel" }));

    expect(asked("answer_pool_sign_in")).toEqual([{ id: 7, yes: false }]);
    await waitFor(() => expect(handCount()).toBeNull());
  });
});
