// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { LaidNode, LaidOut, OpenChat } from "./bindings";
import { stripNamed } from "./test-strips";
import { hoverTipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **A chat with no sidebar row yet, against the whole window** (#5308): a chat the core runs
 * but the plane does not list yet, as a chat just started, or put back at launch, is until its
 * row arrives. It is never called what the tab it is in says, which is its first pane's name:
 * beside the tab's own chat it is called what its row will say, and a tab it is left first in
 * takes that name, never its session number.
 */

vi.mock("./SessionPane", async (original) => {
  const real = await original<typeof import("./SessionPane")>();
  return {
    ...real,
    SessionPane: ({ session }: { session: number }) => (
      <div data-testid="pane" data-session={session}>
        session {session}
      </div>
    ),
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);
const PLANE = "/home/dev/plane";

function chat(session: number, name: string, more: Partial<OpenChat> = {}): OpenChat {
  return {
    session,
    name,
    cwd: `${PLANE}/workspaces/ide`,
    harness: null,
    in_front: false,
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
    ...more,
  };
}

/** The picker's answers: one profile, and the persona `steward` picked for a new chat. */
const START_OPTIONS = {
  profiles: [
    {
      name: "claude",
      kind: "claude",
      shown: "claude",
      source: "built-in",
      is_default: true,
      approval: null,
    },
  ],
  refused: [],
  personas: ["steward"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  declares_none: true,
};

/** One tab, as the stored layout keeps it, holding `tree`. */
const oneTab = (tree: LaidNode): LaidOut => ({
  strip: null,
  tabs: [{ workspace: null, pinned: false, window: 0, front: true, focused: "a", tree }],
});

/**
 * The core: `open` run and come back at launch, laid out as `laid`; the plane lists `listed`
 * alone, and never the chat a start begins, which is given the next session number.
 */
function core(open: OpenChat[], listed: OpenChat[], laid: LaidOut) {
  let next = Math.max(...open.map((one) => one.session));
  mockIPC((cmd) => {
    if (cmd === "plugin:event|listen") return 1;
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return open;
    if (cmd === "plane_sidebar")
      return { root: PLANE, personas: [], persona: null, unfiled: listed, workspaces: [] };
    if (cmd === "reopened_layout") return laid;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "start_details") return [];
    if (cmd === "start_options" || cmd === "start_options_first") return START_OPTIONS;
    if (cmd === "start_chat") return { session: ++next };
    return null;
  });
}

const theTab = () => within(stripNamed("Tabs")).getAllByRole("tab")[0];
const tabName = () => theTab().querySelector(".tab-name")?.textContent;

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a chat with no row yet (#5308)", () => {
  it("started beside a tab's own chat, is called what its row will say, never what the tab says", async () => {
    // The tab's own chat has a label, so the tab says it; a chat split beside it is started
    // under the tab chat's own name, `build`, as steward, and has no label.
    const labelled = chat(1, "build", { label: "the build" });
    core([labelled], [labelled], oneTab({ kind: "chat", chat: 1 }));
    render(<App />);
    await waitFor(() => expect(tabName()).toBe("the build"));

    await userEvent.click(screen.getByRole("button", { name: "Split right" }));
    await userEvent.click(await screen.findByRole("button", { name: "Start chat" }));
    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(2));

    expect(tabName()).toBe("the build");
    expect(await hoverTipOf(theTab())).toContain("the build · steward build");
  });

  it("left first by its tab's own chat's close, names the tab as its row will, never its number", async () => {
    const listed = chat(1, "build");
    // Run as a persona, so what its row will say is more than its pane's own name (#5308).
    const unlisted = chat(2, "review", { persona: "devops" });
    core(
      [listed, unlisted],
      [listed],
      oneTab({
        kind: "split",
        direction: "row",
        share: 500,
        a: { kind: "chat", chat: 1 },
        b: { kind: "chat", chat: 2 },
      }),
    );
    render(<App />);
    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(2));
    await waitFor(() => expect(tabName()).toBe("build"));

    const first = screen.getByRole("region", { name: "Left pane" });
    await userEvent.click(within(first).getByRole("button", { name: "Close this pane" }));

    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(1));
    expect(tabName()).toBe("devops review");
  });
});
