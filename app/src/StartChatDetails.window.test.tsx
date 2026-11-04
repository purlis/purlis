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

/** **Opens the dialog as a person does since #4758**: the plane's one empty pane (a plane with no
 *  chat keeps one), and its New chat. The strip's `+` opens an empty tab, not the dialog. */
async function openTheDialog() {
  const empty = await screen.findByTestId("empty-pane");
  await userEvent.click(within(empty).getByRole("button", { name: "New chat" }));
}

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

    await openTheDialog();

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

    await openTheDialog();
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

    await openTheDialog();
    await userEvent.click(await screen.findByRole("button", { name: /^Start chat/ }));

    answer([{ name: "work", approval: "new", sandbox: null, billed_to: null }]);

    expect(await screen.findByRole("button", { name: "Approve and start" })).toBeTruthy();
    expect(said(asked, "start_chat")).toEqual([]);
    expect(said(asked, "approve_profile")).toEqual([]);
  });

  // ADR 0097 §6: a Start pressed early that turns into a grant is under the person's focus and
  // pointer. A second press right after must not grant what they have not read.
  const grants = [
    {
      what: "an approval",
      row: { name: "work", approval: "new", sandbox: null, billed_to: null },
      button: "Approve and start",
      said: /Not started\. Its command needs your approval first/,
    },
    {
      what: "a start without the sandbox",
      row: {
        name: "work",
        approval: null,
        sandbox: {
          state: "refused",
          said: "The sandbox cannot be applied here.",
          install: null,
          locked: null,
        },
        billed_to: null,
      },
      button: "Start without the sandbox",
      said: /Not started\. The sandbox cannot hold this chat here/,
    },
  ];
  it.each(grants)(
    "never grants $what on a press right after a Start pressed early turned into it",
    async ({ row, button, said: told }) => {
      const { asked, answer } = core();
      render(<App />);

      await openTheDialog();
      const dialog = await screen.findByRole("dialog", { name: "Start a chat" });
      await userEvent.click(within(dialog).getByRole("button", { name: /^Start chat/ }));

      answer([row]);

      const turned = await within(dialog).findByRole("button", { name: button });
      // The keyboard is taken off it, and the turn is said.
      expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Cancel" }));
      expect(within(dialog).getByRole("status").textContent).toMatch(told);

      await userEvent.click(turned);

      expect(said(asked, "approve_profile")).toEqual([]);
      expect(said(asked, "start_chat")).toEqual([]);
    },
  );

  it("approves on a press of its own once the turned Start has held still", async () => {
    const { asked, answer } = core();
    render(<App />);

    await openTheDialog();
    const dialog = await screen.findByRole("dialog", { name: "Start a chat" });
    await userEvent.click(within(dialog).getByRole("button", { name: /^Start chat/ }));

    answer([{ name: "work", approval: "new", sandbox: null, billed_to: null }]);

    const turned = await within(dialog).findByRole("button", { name: "Approve and start" });
    await waitFor(() => expect((turned as HTMLButtonElement).disabled).toBe(false), {
      timeout: 2000,
    });
    await userEvent.click(turned);

    await waitFor(() => expect(said(asked, "approve_profile")).toHaveLength(1));
  });

  it("keeps the room of an approval sentence while the details are read", async () => {
    const { answer } = core();
    render(<App />);

    await openTheDialog();
    const dialog = await screen.findByRole("dialog", { name: "Start a chat" });
    const kept = () =>
      [...dialog.querySelectorAll<HTMLElement>('[data-scope="reserved"][data-part="kept"]')].map(
        (one) => one.dataset.text ?? "",
      );
    expect(kept().some((text) => text.includes("It would run: CLAUDE_CONFIG_DIR"))).toBe(true);

    answer([{ name: "work", approval: "new", sandbox: null, billed_to: null }]);

    // The sentence is drawn in the room it kept: the cards under it do not move.
    const sentence = await within(dialog).findByText(/purlis has not run this profile before/);
    expect(sentence.closest('[data-scope="reserved"][data-part="shown"]')).toBeTruthy();
  });
});
