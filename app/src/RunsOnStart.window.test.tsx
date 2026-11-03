// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { selectNamed } from "./ui/select/select.testkit";

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

const SIDEBAR = {
  root: "/home/dev/plane",
  workspaces: [],
  personas: ["steward"],
  persona: "steward",
  unfiled: [],
};

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

/** The window's core, where a new chat runs on the project's pool `builds`, and `refusing` is
 *  how many starts are refused first because that pool is stopped. */
function core(refusing = 0) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let opened = 0;
  let refused = 0;
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "plane_at_launch")
      return { plane: "/home/dev/plane", from: "/home/dev/plane", why: null };
    if (cmd === "plane_sidebar") return SIDEBAR;
    if (cmd === "running_sessions") return [];
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "start_options") return START_OPTIONS;
    if (cmd === "start_runs_on")
      return { pool: "builds", level: "project", at_root: false, pools: ["builds", "gpu"] };
    if (cmd === "start_chat") {
      if (refused < refusing) {
        refused += 1;
        throw {
          words: "Pool builds is stopped, so the chat did not start.",
          limit: null,
          pool_unavailable: "builds",
        };
      }
      return { session: ++opened };
    }
    return null;
  });
  const boxes = () => asked.filter(({ cmd }) => cmd === "start_chat").map(({ args }) => args.boxes);
  return { asked, boxes };
}

describe("Runs on, from the picker to the start (#4536)", () => {
  it("asks where a new chat would run as the picker opens, and draws it", async () => {
    const { asked } = core();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "New tab" }));

    await waitFor(() => expect(selectNamed("Runs on")).toHaveTextContent("As set (pool builds)"));
    expect(asked.filter(({ cmd }) => cmd === "start_runs_on")).toHaveLength(1);
  });

  it("sends no choice while the row follows the levels", async () => {
    const { boxes } = core();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "New tab" }));
    await waitFor(() => expect(selectNamed("Runs on")).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "Start chat" }));

    await vi.waitFor(() => expect(boxes()).toHaveLength(1));
    expect(boxes()[0]).not.toHaveProperty("runs_on");
  });

  it("sends the pool picked for this start", async () => {
    const { boxes } = core();
    render(<App />);
    const user = userEvent.setup();

    await user.click(await screen.findByRole("button", { name: "New tab" }));
    await waitFor(() => expect(selectNamed("Runs on")).toBeInTheDocument());
    act(() => selectNamed("Runs on").focus());
    await user.keyboard("{Enter}");
    await user.click(await screen.findByRole("option", { name: "Pool gpu" }));
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    await vi.waitFor(() => expect(boxes()).toHaveLength(1));
    expect(boxes()[0]).toMatchObject({ runs_on: { pool: "gpu" } });
  });

  it("refused on a stopped pool, starts on this machine only on Run on this machine this once", async () => {
    const { boxes } = core(1);
    render(<App />);
    const user = userEvent.setup();

    await user.click(await screen.findByRole("button", { name: "New tab" }));
    await waitFor(() => expect(selectNamed("Runs on")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Run on this machine this once" })).toBeVisible(),
    );
    expect(screen.getByRole("button", { name: "Try again" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Pool setup…" })).toBeVisible();
    // Nothing moved to this machine by itself.
    expect(boxes()).toHaveLength(1);
    expect(boxes()[0]).not.toHaveProperty("runs_on");

    await user.click(screen.getByRole("button", { name: "Run on this machine this once" }));

    await vi.waitFor(() => expect(boxes()).toHaveLength(2));
    expect(boxes()[1]).toMatchObject({ runs_on: { pool: null } });
  });
});
