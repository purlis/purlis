// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { Shown } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetInboxOpen, inboxAwayAtLaunch } from "./test-inbox";
import { forgetInbox } from "./inboxRules";
import { FILL_ONLY, runAsk, secretAsk } from "./secretAsk.testkit";

/**
 * **A chat's pool and secret asks on its own pane** (plane #2697), against the whole window. A
 * runner chat's browser login on its pool, one run of its tool on the pool, a key its brokered
 * run missed, and a run of a program its persona does not declare each wait on the person, and
 * the chat waits on them. Before, only the Inbox's Asks view listed them, and a person looking
 * at the chat, with the right side on another view, saw nothing to press. Each is drawn on the
 * chat's pane now, and its answer sends what the Inbox's would.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">
      session {session}
      <textarea className="xterm-helper-textarea" aria-label="Terminal input" />
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

beforeEach(() => inboxAwayAtLaunch());
afterEach(forgetInboxOpen);
afterEach(forgetInbox);
afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 4,
  name: "claude 4",
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

/** Chat 4, on a pool's runner, asks to log its browser in there (#2050). As the core writes it. */
const POOL_LOGIN: Shown = {
  session: 4,
  ask: "pool-login:4",
  says: "Its browser on the pool's runner asks to be logged in with your persona's logins.",
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "keep", label: "Keep off", allows: false },
  ],
  source: "pool-login",
  chain: ["claude 4"],
  answer: { via: "pool-login" },
  since: 10,
};

/** One run of chat 4's declared tool on the pool's runner wants its vault values (#2047). */
const POOL_TOOL: Shown = {
  session: 4,
  ask: "pool-tool:4:12",
  says: "Its tool gh asks for its vault values for one run on the pool's runner.",
  options: [
    { id: "allow", label: "Allow this run", allows: true },
    { id: "keep", label: "Keep off", allows: false },
  ],
  source: "pool-tool",
  chain: ["claude 4"],
  answer: { via: "pool-tool", run: 12 },
  since: 10,
};

/** A key chat 4's brokered run missed (#5068): Fill and Not now. */
const SECRET: Shown = { ...secretAsk(), session: 4, chain: ["claude 4"], since: 10 };

/** A run of chat 4's that asks for a program its persona does not declare (#5242). */
const RUN: Shown = { ...runAsk({}, FILL_ONLY), session: 4, chain: ["claude 4"], since: 10 };

const SENT = new Set([
  "answer_pool_login",
  "answer_pool_tool",
  "answer_secret_ask",
  "answer_run_grant",
]);

function core(asks: Shown[]) {
  const sent: { cmd: string; args: unknown }[] = [];
  mockIPC(
    (cmd, args) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "reopened_layout") return aTabForEachChat([CHAT], () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT] }],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "asks_waiting") return { plane: PLANE, asks };
      if (cmd === "mcp_failures") return [];
      if (SENT.has(cmd)) sent.push({ cmd, args });
      return null;
    },
    { shouldMockEvents: true },
  );
  return { sent };
}

async function aPane(asks: Shown[]) {
  const said = core(asks);
  render(<App />);
  await screen.findByTestId("pane");
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return said;
}

const frame = () => screen.getByTestId("pane").closest(".pane-frame") as HTMLElement;

describe("a chat's pool and secret asks on its own pane", () => {
  it("draws its browser login on a pool, and Allow lets its logins be filled there", async () => {
    const { sent } = await aPane([POOL_LOGIN]);

    const row = await within(frame()).findByRole("status", { name: "Browser login on a pool" });
    expect(within(row).getByText(/logged in with your persona's logins/)).toBeTruthy();

    await userEvent.click(within(row).getByRole("button", { name: "Allow" }));

    await waitFor(() =>
      expect(sent).toEqual([
        { cmd: "answer_pool_login", args: { plane: PLANE, session: 4, allow: true, reason: null } },
      ]),
    );
  });

  it("draws one run of its tool on a pool, and Keep off refuses that run", async () => {
    const { sent } = await aPane([POOL_TOOL]);

    const row = await within(frame()).findByRole("status", { name: "Tool run on a pool" });
    expect(within(row).getByText(/Its tool gh asks/)).toBeTruthy();

    await userEvent.click(within(row).getByRole("button", { name: "Keep off" }));

    await waitFor(() =>
      expect(sent).toEqual([
        { cmd: "answer_pool_tool", args: { plane: PLANE, session: 4, run: 12, allow: false } },
      ]),
    );
  });

  it("draws a key its run missed, and Fill stores the value typed on the pane", async () => {
    const { sent } = await aPane([SECRET]);

    const row = await within(frame()).findByRole("status", { name: "Secret" });
    expect(within(row).getByText(/STRIPE_API_KEY/)).toBeTruthy();

    await userEvent.click(within(row).getByRole("button", { name: "Answer" }));
    const box = await within(frame()).findByLabelText("Value of STRIPE_API_KEY");
    await userEvent.type(box, "sk_test_123");
    await userEvent.click(within(frame()).getByTestId("sa-primary"));

    await waitFor(() =>
      expect(sent).toEqual([
        {
          cmd: "answer_secret_ask",
          args: {
            plane: PLANE,
            id: 1,
            value: "sk_test_123",
            vault: null,
            key: null,
            remember: null,
          },
        },
      ]),
    );
  });

  it("draws a run that asks for a secret, and Not now refuses it", async () => {
    const { sent } = await aPane([RUN]);

    const row = await within(frame()).findByRole("status", { name: "Run with a secret" });
    expect(within(row).getByText(/\/usr\/bin\/curl/)).toBeTruthy();

    await userEvent.click(within(row).getByRole("button", { name: "Answer" }));
    await userEvent.click(await within(frame()).findByTestId("sa-not-now"));

    await waitFor(() =>
      expect(sent).toEqual([
        {
          cmd: "answer_run_grant",
          args: expect.objectContaining({ plane: PLANE, id: 9, press: "refuse" }),
        },
      ]),
    );
  });
});
