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
import { forgetKeyboard } from "./paneKeyboard";
import { forgetRailMode } from "./railMode";

/**
 * **A chat asking to remove a workspace puts a ✋ on its chip** (#3652, spec #3647), on the real
 * window with the core mocked: the question stays on the asking chat's band, where the consent is
 * answered, and the workspace's chip shows a needs-you dot. Pressing it brings the asking chat
 * forward; the dot stays until the question is answered, whatever else is pressed.
 */

/** Each chat handed the keyboard, as `paneKeyboard` is asked to, which still does it. */
const given = vi.hoisted(() => vi.fn());
vi.mock("./paneKeyboard", async (original) => {
  const real = await original<typeof import("./paneKeyboard")>();
  return {
    ...real,
    giveKeyboardTo: (plane: string, session: number) => {
      given(plane, session);
      real.giveKeyboardTo(plane, session);
    },
  };
});

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

afterEach(() => {
  cleanup();
  clearMocks();
  forgetRailMode();
  forgetKeyboard();
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

/** Chat 2's question, as the registry lists it: remove `workspace` (qw356). */
const removal = (workspace: string): Shown => ({
  session: 2,
  ask: `removal:2:${workspace}`,
  says: `Removing workspace ${workspace} deletes its memory, its todos and its clones`,
  options: [
    { id: "remove", label: "Remove…", allows: true },
    { id: "keep", label: "Keep", allows: false },
  ],
  source: "workspace-removal",
  chain: [`Safe remove · ${workspace}`],
  answer: { via: "workspace-removal", workspace, memories: 0, todos: 0 },
});

/** One project with `alpha` and `beta` pinned and chats 1 and 2 in `alpha`, chat `front` in
 *  front, and chat 2 asking to remove `removes` while `waiting.asks` holds its question.
 *  `workspace_create` makes one, as the core does, and the store keeps a pin written. */
function core({ front = 1, removes = "beta" }: { front?: number; removes?: string } = {}) {
  const chats = [chat(1, front === 1), chat(2, front === 2)];
  const waiting: { asks: Shown[] } = { asks: [removal(removes)] };
  const pinned = ["alpha", "beta"];
  const made: string[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: ["alpha", "beta", ...made].map((name) => ({
          name,
          path: `${PLANE}/workspaces/${name}`,
          vision: "",
          todos: [],
          chats: name === "alpha" ? chats : [],
        })),
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "opened_chats") return chats;
    if (cmd === "plane_pins")
      return { project: false, workspaces: pinned, missing: [], order: pinned, certain: true };
    if (cmd === "pin_workspace") {
      const name = String(given.workspace);
      if (given.pinned === true && !pinned.includes(name)) pinned.push(name);
      return null;
    }
    if (cmd === "workspace_create") {
      const name = String(given.name);
      made.push(name);
      return [`✓ Workspace '${name}' ready (LOCAL) → workspaces/${name}/`];
    }
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

/** Makes `name` from the ⌄'s last row, as a person does. */
async function make(name: string) {
  await userEvent.click(within(bar()).getByRole("button", { name: "All workspaces" }));
  const list = await screen.findByRole("listbox", { name: /^Workspaces in / });
  await userEvent.click(within(list).getByRole("option", { name: "New workspace…" }));
  const dialog = await screen.findByRole("dialog", { name: /workspace/i });
  await userEvent.type(within(dialog).getByLabelText("Name"), name);
  await userEvent.click(within(dialog).getByRole("button", { name: "Create workspace" }));
}

/** The ✋ on `workspace`'s chip, once it is drawn. */
const findHand = (workspace: string) =>
  waitFor(() => {
    const found = queryHand(workspace);
    expect(found).not.toBeNull();
    return found as HTMLElement;
  });

describe("a chat asking to remove a workspace (#3652)", () => {
  it("puts a ✋ dot on that workspace's chip, the most urgent kind, and says so", async () => {
    core();
    opened();
    const hand = await findHand("beta");
    expect(hand).toHaveAttribute("data-kind", "needs");
    expect(hand).toHaveAttribute("data-part", "dot-needs");
    expect(queryHand("alpha")).toBeNull();
    // The chip's description says it, beside its words (WCAG 2.5.3), and no "0 notices".
    const described = chipOf("beta").getAttribute("aria-describedby");
    expect(described).not.toBeNull();
    expect(document.getElementById(described ?? "")).toHaveTextContent(
      /^a chat asks you to remove it$/,
    );
  });

  it("brings the asking chat forward when its dot is pressed, and gives it the keyboard", async () => {
    core();
    opened();
    expect(await screen.findByTestId("pane")).toHaveTextContent("session 1");
    const hand = await findHand("beta");
    given.mockClear();

    await userEvent.click(hand);

    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
    expect(given).toHaveBeenCalledWith(PLANE, 2);
    // The question is still waiting, so its dot stays.
    expect(queryHand("beta")).not.toBeNull();
  });

  it("gives the keyboard to the asking chat even where it is in front already", async () => {
    core({ front: 2 });
    opened();
    expect(await screen.findByTestId("pane")).toHaveTextContent("session 2");
    const hand = await findHand("beta");
    given.mockClear();

    await userEvent.click(hand);

    await waitFor(() => expect(given).toHaveBeenCalledWith(PLANE, 2));
  });

  it("brings up the workspace's own card beside the chat, without the keyboard, and stays while it is up", async () => {
    core({ removes: "gamma" });
    opened();
    await waitFor(() => expect(within(bar()).getByRole("button", { name: "alpha" })).toBeTruthy());
    await make("gamma");
    await screen.findByRole("region", { name: "Notices for gamma" });
    // Hidden by a press elsewhere: gamma's ready notice waits behind the ✋, the most urgent.
    await userEvent.click(document.body);
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Notices for gamma" })).toBeNull(),
    );
    const hand = await waitFor(() => {
      const found = within(bar()).queryByRole("button", {
        name: "Show the chat asking to remove gamma, and its notice",
      });
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });

    await userEvent.click(hand);

    const card = await screen.findByRole("region", { name: "Notices for gamma" });
    await waitFor(() => expect(screen.getByTestId("pane")).toHaveTextContent("session 2"));
    expect(card.contains(document.activeElement)).toBe(false);
    // The ✋ stays while its own chip's card is up: the question is not on the card.
    expect(
      within(bar()).queryByRole("button", { name: /^Show the chat asking to remove gamma/ }),
    ).not.toBeNull();
  });

  it("stays until the chat is answered, and only then goes", async () => {
    const waiting = core();
    opened();
    await findHand("beta");

    // Answered (on its band, or in the Inbox): the registry no longer lists it.
    waiting.asks = [];
    act(() => asksMoved(PLANE));
    await waitFor(() => expect(queryHand("beta")).toBeNull());
  });
});
