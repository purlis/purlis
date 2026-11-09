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

/**
 * **A chat's browser asks the person on the chat's own pane** (#5985), against the
 * whole window. A chat's first browser call asks what its browser may reach, and a site new to
 * the project its browser was sent to asks to be let on. Both wait on the person, and the chat
 * waits on them: so the chat's pane draws each, with the core's own answers, as it draws every
 * other ask of its chat's that has answers. Before, only the Inbox's Asks view listed them, and a
 * person looking at the chat, with the right side on another view, saw nothing to press.
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

/** Chat 4's first browser call: what may its browser reach? As the core writes it. */
const REACH: Shown = {
  session: 4,
  ask: "browser:4",
  says: "Its browser is asked to open https://example.com/. What may it reach?",
  options: [
    { id: "project", label: "This project's sites", allows: true },
    { id: "open-web", label: "Also the open web", allows: true },
    { id: "keep", label: "Keep off", allows: false },
  ],
  source: "browser",
  chain: ["claude 4"],
  answer: { via: "browser" },
  since: 10,
};

/** Chat 4's browser was sent to a site new to the project, and held it back. */
const SITE: Shown = {
  session: 4,
  ask: "browser-sites:4",
  says: "Its browser wants to reach news.example.org:443. An Allow is its browser's alone, from its next act; nothing restarts.",
  options: [
    { id: "chat", label: "Allow for this chat", allows: true },
    { id: "you", label: "Always allow in this project, on this machine", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "browser-site",
  chain: ["claude 4"],
  answer: { via: "browser-sites", sites: ["news.example.org:443"] },
  since: 10,
};

/** Chat 4's browser stopped on a page that needs the person: a consent page, a sign-in. */
const NEEDS_YOU: Shown = {
  session: 4,
  ask: "browser-needs-you:browser-1",
  says: "Its browser waits for you on accounts.example.com",
  options: [{ id: "take-over", label: "Take over", allows: false }],
  source: "browser-needs-you",
  chain: ["claude 4"],
  answer: { via: "take-over", program: "browser-1" },
  since: 10,
};

function core(asks: Shown[]) {
  const layouts: string[] = [];
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
      if (cmd === "write_layout") layouts.push((args as { text: string }).text);
      if (
        cmd === "allow_chat_browser" ||
        cmd === "allow_browser_sites" ||
        cmd === "take_over_browser"
      )
        sent.push({ cmd, args });
      // Take over opens the browser's view, which watches it: one that never answers.
      if (cmd === "watch_browser") return new Promise(() => {});
      return null;
    },
    { shouldMockEvents: true },
  );
  return { layouts, sent };
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

describe("a chat's browser asks on its own pane", () => {
  it("draws what its browser may reach with the core's answers, and an answer allows it", async () => {
    const { sent } = await aPane([REACH]);

    const row = await within(frame()).findByRole("status", { name: "Browser" });
    expect(within(row).getByText(/What may it reach\?/)).toBeTruthy();
    expect(
      within(row)
        .getAllByRole("button")
        .map((one) => one.getAttribute("aria-label") ?? one.textContent),
    ).toEqual(expect.arrayContaining(["This project's sites", "Also the open web", "Keep off"]));

    await userEvent.click(within(row).getByRole("button", { name: "This project's sites" }));

    await waitFor(() =>
      expect(sent).toEqual([
        {
          cmd: "allow_chat_browser",
          args: { plane: PLANE, session: 4, openWeb: false, always: false },
        },
      ]),
    );
  });

  it("draws a site its browser held back with the core's answers, and Allow lets it on", async () => {
    const { sent } = await aPane([SITE]);

    const row = await within(frame()).findByRole("status", { name: "Browser site" });
    expect(within(row).getByText(/news\.example\.org:443/)).toBeTruthy();

    await userEvent.click(within(row).getByRole("button", { name: "Allow for this chat" }));

    await waitFor(() =>
      expect(sent).toEqual([
        {
          cmd: "allow_browser_sites",
          args: { plane: PLANE, session: 4, sites: ["news.example.org:443"], level: "chat" },
        },
      ]),
    );
  });

  it("draws a browser that waits for the person, and Take over takes it over", async () => {
    const { sent } = await aPane([NEEDS_YOU]);

    const row = await within(frame()).findByRole("status", { name: "Browser needs you" });
    expect(within(row).getByText(/accounts\.example\.com/)).toBeTruthy();

    await userEvent.click(within(row).getByRole("button", { name: "Take over" }));

    await waitFor(() =>
      expect(sent).toEqual([{ cmd: "take_over_browser", args: { program: "browser-1" } }]),
    );
  });
});
