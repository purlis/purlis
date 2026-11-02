// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";

/**
 * **The new-chat dialog opens on a cheap first answer** (#4736, spec #4724): the personas, the
 * profiles and the default persona, from `start_options_first`. Each row's approval, sandbox and
 * billing come in `start_details`, which takes a moment more (a sandbox compile per harness and
 * a check of each approved profile's program), and fill the rows in as they arrive.
 */

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

const PLANE = "/home/dev/plane";

const FIRST = {
  profiles: [
    {
      name: "work",
      kind: "claude",
      shown: "CLAUDE_CONFIG_DIR=~/.claude-work claude",
      source: "charter.local.toml",
      is_default: true,
      approval: null,
      ready_to_type: true,
      harness: null,
      sandbox: null,
      model: "the harness's default",
      billed_to: null,
    },
  ],
  refused: [],
  personas: ["steward", "release"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  ignore_fix_id: null,
  declares_none: false,
};

/** The core, with the details held until `answer` is called with them. */
function core() {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let answer: (rows: unknown) => void = () => {};
  const details = new Promise((resolve) => {
    answer = resolve;
  });
  let opened = 0;
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [],
        personas: FIRST.personas,
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "running_sessions") return [];
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "start_options_first") return FIRST;
    if (cmd === "start_details") return details;
    if (cmd === "start_options") throw new Error("the picker asked for the whole answer");
    if (cmd === "approve_profile") return null;
    if (cmd === "start_chat") return { session: ++opened };
    return null;
  });
  return { asked, answer };
}

const said = <T extends { cmd: string }>(asked: T[], cmd: string) =>
  asked.filter((one) => one.cmd === cmd);

describe("the new-chat dialog's two answers (#4736)", () => {
  it("draws the personas and profiles before the details arrive, then fills the rows in", async () => {
    const { answer } = core();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "New tab" }));

    const dialog = await screen.findByRole("dialog", { name: "Start a chat" });
    expect(within(dialog).getByRole("option", { name: "release" })).toBeTruthy();
    const card = within(dialog).getByRole("radio", { name: /work/ });
    expect(card.closest("label")?.textContent).toContain("Checking billing and approval…");

    answer([
      { name: "work", approval: null, sandbox: null, billed_to: "Claude subscription · work" },
    ]);

    await waitFor(() =>
      expect(within(dialog).getByText(/Billed to Claude subscription · work\./)).toBeTruthy(),
    );
    expect(dialog.textContent).not.toContain("Checking billing and approval…");
  });

  it("starts a chat pressed for early once the details say it is a plain start", async () => {
    const { asked, answer } = core();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "New tab" }));
    await userEvent.click(await screen.findByRole("button", { name: /^Start chat/ }));
    expect(said(asked, "start_chat")).toEqual([]);

    answer([{ name: "work", approval: null, sandbox: null, billed_to: null }]);

    await waitFor(() => expect(said(asked, "start_chat")).toHaveLength(1));
    expect(said(asked, "start_chat")[0].args).toMatchObject({
      profile: "work",
      persona: "steward",
    });
  });

  it("asks for the approval a profile turns out to need, rather than starting it", async () => {
    const { asked, answer } = core();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "New tab" }));
    await userEvent.click(await screen.findByRole("button", { name: /^Start chat/ }));

    answer([{ name: "work", approval: "new", sandbox: null, billed_to: null }]);

    expect(await screen.findByRole("button", { name: "Approve and start" })).toBeTruthy();
    expect(said(asked, "start_chat")).toEqual([]);
    expect(said(asked, "approve_profile")).toEqual([]);
  });
});
