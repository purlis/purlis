// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { ReadOnlyProject } from "./bindings";
import { forgetThisLaunch } from "./regions";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **A project this purlis opens read-only says why once, on its tab, and its write buttons are
 * off with that sentence** (#826, FR-24). The whole window against a mocked core: the project's
 * line is drawn once, the workspace strip's `+` is off with the sentence as its reason, and the
 * palette lists the row unavailable with the same words, never dropping it.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;
const READ_ONLY: ReadOnlyProject = {
  sentence:
    "this project requires the feature memory-proposals, which purlis 0.8.0 does not have; " +
    "purlis 0.9.0 or later has it. purlis opens the project read-only and writes nothing to it.",
  feature: "memory-proposals",
  since: "0.9.0",
  mendable: false,
};

/** One project with one workspace, read-only when `readOnly` is given. */
function core(readOnly: ReadOnlyProject | null) {
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "project_read_only") return readOnly;
    if (cmd === "opened_chats") return [];
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [] }],
      };
    if (cmd === "workspace_panels")
      return {
        workspace: a.workspace,
        repos: [],
        paths: {},
        absent: [],
        refused: [],
        todos: [],
        todos_refused: null,
        personas: ["steward"],
        persona: "steward",
        sessions: [],
        contributed: [],
      };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    return null;
  });
}

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const newWorkspace = () => screen.findByRole("button", { name: "New workspace…" });

describe("a project this purlis opens read-only (#826)", () => {
  it("says why once, on the project's tab", async () => {
    core(READ_ONLY);
    render(<App />);

    const line = await screen.findByRole("note", { name: "Read-only project" });
    expect(line.textContent).toContain("memory-proposals");
    expect(line.textContent).toContain("purlis 0.9.0 or later has it");
    expect(screen.getAllByRole("note", { name: "Read-only project" })).toHaveLength(1);
  });

  it("turns off the workspace strip's + with that sentence", async () => {
    core(READ_ONLY);
    render(<App />);
    await screen.findByRole("note", { name: "Read-only project" });

    await waitFor(async () =>
      expect(await newWorkspace()).toHaveAttribute("aria-disabled", "true"),
    );
    // Its name, then why, as an icon button's tooltip draws them; why is its description too.
    expect(tipOf(await newWorkspace())).toBe(`New workspace…${READ_ONLY.sentence}`);
    expect(await newWorkspace()).toHaveAccessibleDescription(READ_ONLY.sentence);
  });

  it("lists the palette's row unavailable, with the sentence, rather than dropping it", async () => {
    core(READ_ONLY);
    render(<App />);
    await screen.findByRole("note", { name: "Read-only project" });

    await userEvent.keyboard("{F2}");
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.keyboard("New workspace");
    await waitFor(() => expect(palette.textContent).toContain(READ_ONLY.sentence));
  });

  it("draws no line and keeps the + for a project this purlis may write", async () => {
    core(null);
    render(<App />);

    await waitFor(async () => expect(await newWorkspace()).toBeEnabled());
    expect(screen.queryByRole("note", { name: "Read-only project" })).toBeNull();
  });
});
