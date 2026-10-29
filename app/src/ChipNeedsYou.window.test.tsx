// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { asksMoved } from "./asks";
import type { Shown } from "./bindings";
import { forgetRailMode } from "./railMode";

/**
 * **A chat asking to remove a workspace puts a ✋ on its chip** (#3652, spec #3647), on the real
 * window with the core mocked: the question stays on the asking chat's band, where the consent is
 * answered, and the workspace's chip shows a needs-you dot. Pressing it brings the asking chat
 * forward; the dot stays until the question is answered, whatever else is pressed.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

afterEach(() => {
  cleanup();
  clearMocks();
  forgetRailMode();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/one";

/** A chat in `alpha`; `in_front` for the one the window shows first. */
const chat = (session: number, in_front: boolean) => ({
  session,
  name: `alpha.${session}`,
  cwd: `${PLANE}/workspaces/alpha`,
  harness: null,
  in_front,
  resumed: null,
  fresh: null,
  profile: null,
  persona: null,
  unreported: null,
  card: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
});
const CHATS = [chat(1, true), chat(2, false)];

/** Chat 2's question, as the registry lists it: remove `beta` (qw356). */
const REMOVE_BETA: Shown = {
  session: 2,
  ask: "removal:2:beta",
  says: "Removing workspace beta deletes its memory, its todos and its clones",
  options: [
    { id: "remove", label: "Remove…", allows: true },
    { id: "keep", label: "Keep", allows: false },
  ],
  source: "workspace-removal",
  chain: ["Safe remove · beta"],
  answer: { via: "workspace-removal", workspace: "beta", memories: 0, todos: 0 },
};

/** One project with `alpha` and `beta` pinned, chat 1 in front, and chat 2 asking to remove
 *  `beta` while `waiting.asks` holds its question. */
function core() {
  const waiting: { asks: Shown[] } = { asks: [REMOVE_BETA] };
  mockIPC((cmd) => {
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: ["alpha", "beta"].map((name) => ({
          name,
          path: `${PLANE}/workspaces/${name}`,
          vision: "",
          todos: [],
          chats: name === "alpha" ? CHATS : [],
        })),
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "opened_chats") return CHATS;
    if (cmd === "plane_pins")
      return {
        project: false,
        workspaces: ["alpha", "beta"],
        missing: [],
        order: ["alpha", "beta"],
        certain: true,
      };
    if (cmd === "asks_waiting") return { asks: waiting.asks };
    if (cmd === "reachable_repos") return { repos: [], trouble: [] };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  return waiting;
}

const bar = () => screen.getByTestId("title-bar");
/** The ✋ on `workspace`'s chip. */
const queryHand = (workspace: string) =>
  within(bar()).queryByRole("button", {
    name: `Show the chat asking to remove ${workspace}`,
  });
/** `workspace`'s chip in the strip. */
const chipOf = (workspace: string) =>
  within(bar()).getByRole("button", { name: workspace, current: false }) as HTMLElement;

const opened = () =>
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );

describe("a chat asking to remove a workspace (#3652)", () => {
  it("puts a ✋ dot on that workspace's chip, the most urgent kind, and says so", async () => {
    core();
    opened();
    const hand = await waitFor(() => {
      const found = queryHand("beta");
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(hand).toHaveAttribute("data-kind", "needs");
    expect(queryHand("alpha")).toBeNull();
    // The chip's description says it, beside its words (WCAG 2.5.3).
    const described = chipOf("beta").getAttribute("aria-describedby");
    expect(described).not.toBeNull();
    expect(document.getElementById(described ?? "")).toHaveTextContent(
      "a chat asks you to remove it",
    );
  });

  it("brings the asking chat forward when its dot is pressed", async () => {
    core();
    opened();
    expect(await screen.findByTestId("pane")).toHaveTextContent("session 1");
    const hand = await waitFor(() => {
      const found = queryHand("beta");
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });

    await userEvent.click(hand);

    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
    // The consent is answered on its band, not on a card: none comes up on beta's chip.
    expect(screen.queryByRole("region", { name: "Notices for beta" })).toBeNull();
    // And the question is still waiting, so its dot stays.
    expect(queryHand("beta")).not.toBeNull();
  });

  it("stays until the chat is answered, and only then goes", async () => {
    const waiting = core();
    opened();
    await waitFor(() => expect(queryHand("beta")).not.toBeNull());

    // A press elsewhere is no answer.
    await userEvent.click(document.body);
    expect(queryHand("beta")).not.toBeNull();

    // Answered (on its band, or in the Inbox): the registry no longer lists it.
    waiting.asks = [];
    act(() => asksMoved(PLANE));
    await waitFor(() => expect(queryHand("beta")).toBeNull());
  });
});
