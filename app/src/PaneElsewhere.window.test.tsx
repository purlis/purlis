// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { LaidOut } from "./bindings";

/**
 * **A pane showing another workspace's chat wears that workspace's badge** (#4982; #4583
 * rule 2), against the whole window. A drag may put a chat of another workspace in a pane; the
 * chat stays filed in its own, and its pane head names that workspace, in its colour, in the
 * head's "harness · workspace" part, so a narrow pane drops it with that part. The model's half
 * is `tabs.elsewhere.test.ts`.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" data-session={session}>
      session {session}
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";
const at = (workspace: string) => `${PLANE}/workspaces/${workspace}`;

const chat = (session: number, workspace: string) => ({
  session,
  name: `claude ${session}`,
  cwd: `${at(workspace)}/repo`,
  harness: "claude",
  in_front: session === 4,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
  sandbox: null,
  sandbox_why: null,
  browser_sites: null,
});

/** Chat 4 and chat 5 work in alpha; chat 7 works in beta. */
const FOUR = chat(4, "alpha");
const FIVE = chat(5, "alpha");
const SEVEN = chat(7, "beta");

/** One tab on alpha's strip: chat 4 on the left, and `beside` on the right. */
const besideFour = (beside: number): LaidOut => ({
  strip: "alpha",
  tabs: [
    {
      workspace: "alpha",
      pinned: false,
      window: 0,
      front: true,
      focused: "a",
      tree: {
        kind: "split",
        direction: "row",
        share: 500,
        a: { kind: "chat", chat: 4 },
        b: { kind: "chat", chat: beside },
      },
    },
  ],
});

function core(laid: LaidOut, betaColour: string | null) {
  const workspace = (name: string, chats: unknown[], colour: string | null) => ({
    name,
    path: at(name),
    vision: "",
    todos: [],
    chats,
    colour,
  });
  mockIPC(
    (cmd) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [FOUR, FIVE, SEVEN];
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [
            workspace("alpha", [FOUR, FIVE], null),
            workspace("beta", [SEVEN], betaColour),
          ],
        };
      if (cmd === "reopened_layout") return laid;
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "asks_waiting") return { plane: PLANE, asks: [] };
      if (cmd === "mcp_failures") return [];
      if (cmd === "smart_closing") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
}

/** Draws the window with every pane `width` pixels wide, as the layout engine would. */
function panesOf(width: number) {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private readonly told: (entries: unknown[], seen: unknown) => void) {}
      observe() {
        this.told([], this);
      }
      disconnect() {}
      unobserve() {}
    },
  );
  const real = HTMLElement.prototype.getBoundingClientRect;
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.classList.contains("pane-frame")
      ? ({
          width,
          height: 400,
          top: 0,
          left: 0,
          right: width,
          bottom: 400,
          x: 0,
          y: 0,
        } as DOMRect)
      : real.call(this);
  });
}

async function twoPanes(beside: number, betaColour: string | null = "teal") {
  core(besideFour(beside), betaColour);
  render(<App />);
  await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(2));
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** The head of the pane showing chat `session`. */
const headOf = (session: number) =>
  document
    .querySelector(`[data-testid="pane"][data-session="${session}"]`)
    ?.closest(".pane-frame")
    ?.querySelector(".pane-head") as HTMLElement;

const whereOf = (session: number) => headOf(session).querySelector(".pane-where");
const badgeOf = (session: number) =>
  headOf(session).querySelector('.pane-where [data-scope="badge"]') as HTMLElement | null;

describe("a pane showing another workspace's chat (#4982)", () => {
  it("names that workspace on its head with a badge in its colour", async () => {
    await twoPanes(7);

    await waitFor(() => expect(badgeOf(7)).not.toBeNull());
    expect(whereOf(7)).toHaveTextContent(/^claude · (filed in )?beta$/);
    // On its ground, which the badge's tokens inherit: a badge takes no look of its own.
    expect(badgeOf(7)?.parentElement).toHaveAttribute("data-colour", "teal");
    expect(badgeOf(7)?.parentElement?.getAttribute("style") ?? "").toMatch(/--accent-surface:/);
    expect(badgeOf(7)).toHaveAttribute("data-variant", "accent");
  });

  it("says to a screen reader that the chat is filed in that workspace", async () => {
    await twoPanes(7);

    await waitFor(() => expect(badgeOf(7)).not.toBeNull());
    expect(badgeOf(7)).toHaveTextContent(/^filed in beta$/);
    expect(badgeOf(7)?.querySelector(".sr-only")).toHaveTextContent(/^filed in$/);
  });

  it("draws a plain badge for a workspace with no colour, not the front workspace's", async () => {
    await twoPanes(7, null);

    await waitFor(() => expect(badgeOf(7)).not.toBeNull());
    expect(badgeOf(7)?.parentElement).not.toHaveAttribute("data-colour");
    expect(badgeOf(7)?.parentElement?.getAttribute("style") ?? "").not.toMatch(/--accent/);
    expect(badgeOf(7)).toHaveAttribute("data-variant", "neutral");
  });

  it("puts no badge on the pane of the tab's own workspace beside it", async () => {
    await twoPanes(7);

    await waitFor(() => expect(badgeOf(7)).not.toBeNull());
    expect(whereOf(4)).toHaveTextContent(/^claude · alpha$/);
    expect(badgeOf(4)).toBeNull();
  });
});

describe("a pane showing its tab's own workspace's chat (#4982)", () => {
  it("shows no badge", async () => {
    await twoPanes(5);

    await waitFor(() => expect(whereOf(5)).toHaveTextContent(/^claude · alpha$/));
    expect(badgeOf(5)).toBeNull();
    expect(badgeOf(4)).toBeNull();
  });
});

describe("a narrow pane showing another workspace's chat (#4982)", () => {
  it("keeps the badge where harness · workspace still fits", async () => {
    panesOf(520);
    await twoPanes(7);

    await waitFor(() => expect(badgeOf(7)).not.toBeNull());
  });

  it("drops the badge with harness · workspace, first in the ruled order", async () => {
    panesOf(500);
    await twoPanes(7);

    // The head has drawn its narrow form: harness · workspace, badge and all, is gone, and the
    // parts the ruled order drops after it are still there.
    await waitFor(() => expect(headOf(7).querySelector(".pane-name")).not.toBeNull());
    expect(whereOf(7)).toBeNull();
    expect(headOf(7)).not.toHaveTextContent("beta");
    expect(headOf(7).querySelector(".pane-state-mark")).not.toBeNull();
    expect(within(headOf(7)).getByRole("button", { name: "Split down" })).toBeInTheDocument();
  });
});
