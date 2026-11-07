// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { stripNamed } from "./test-strips";

/**
 * A chat's name, against the whole window (charter-app#254): the default `<persona> <N>`, the
 * picker's Name field, a tab's Rename and Rename chat (#5343), and the name coming back at a
 * relaunch.
 *
 * **What a rename changes is charter's label and nothing else.** The core is asked to hold the
 * name (`rename_chat`), and its answer is what the tab draws; the chat's own name — what its
 * harness was started with — is never sent again, so a running harness is not disturbed.
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

const SIDEBAR = {
  root: PLANE,
  workspaces: [
    { name: "alpha", path: `${PLANE}/workspaces/alpha`, vision: "", todos: [], chats: [] },
  ],
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

/** One chat the core put back at a launch. */
function putBack(session: number, name: string, label: string | null): OpenChat {
  return {
    session,
    name,
    cwd: `${PLANE}/workspaces/alpha`,
    harness: "claude",
    in_front: true,
    resumed: null,
    fresh: null,
    profile: "claude",
    persona: "steward",
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label,
    from: null,
  };
}

type Asked = { cmd: string; args: Record<string, unknown> };

/**
 * The core. `rename_chat` answers the name as the core's rule holds it — trimmed, blank as
 * none — or throws `refuse`, the way a refusal arrives from the real one.
 */
function core({
  open = [],
  refuse,
}: { open?: ReturnType<typeof putBack>[]; refuse?: string } = {}): { asked: Asked[] } {
  const asked: Asked[] = [];
  let started = open.length;
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar") return SIDEBAR;
    if (cmd === "opened_chats") return open;
    // Each chat put back has its tab, as the stored layout keeps it (#4758).
    if (cmd === "reopened_layout") return aTabForEachChat(open, () => "alpha");
    if (cmd === "running_sessions") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "start_details") return [];
    if (cmd === "start_options" || cmd === "start_options_first") return START_OPTIONS;
    if (cmd === "start_chat") {
      if (refuse !== undefined) throw { words: refuse, limit: null };
      const typed = (args as { label: string | null }).label?.trim() ?? "";
      return { session: ++started, label: typed === "" ? null : typed };
    }
    if (cmd === "rename_chat") {
      if (refuse !== undefined) throw refuse;
      const label = String((args as { label: string }).label).trim();
      return label === "" ? null : label;
    }
    return null;
  });
  return { asked };
}

const strip = () => stripNamed("Tabs");
const tabNames = () =>
  within(strip())
    .getAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);
const theTab = () => within(strip()).getAllByRole("tab")[0];
const asked = (asks: Asked[], cmd: string) => asks.filter((one) => one.cmd === cmd);

/**
 * New chat in the empty pane in front (#4758): a chat fills an empty pane (#5289), so where a chat
 * shows already the strip's `+` opens a new tab first, and the chat gets a tab of its own as it
 * did when `+` started one. The sidebar's `+` is called New chat too, so the empty pane's is
 * asked for by where it is.
 */
async function pressNewChat() {
  const empty = await screen.findByTestId("empty-pane").catch(() => undefined);
  if (empty === undefined)
    await userEvent.click(await screen.findByRole("button", { name: "New tab" }));
  await userEvent.click(
    within(await screen.findByTestId("empty-pane")).getByRole("button", { name: "New chat" }),
  );
}

/** New chat, then Start — with `name` typed into the picker's Name field first, if given. */
async function openAChat(name?: string) {
  await pressNewChat();
  const picker = await screen.findByRole("dialog", { name: "Start a chat" });
  if (name !== undefined) await userEvent.type(within(picker).getByLabelText(/^Name/), name);
  await userEvent.click(within(picker).getByRole("button", { name: "Start chat" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
}

describe("a new chat's name", () => {
  it("is the persona and then the chat's number: `steward 1`", async () => {
    core();
    render(<App />);

    await openAChat();

    await waitFor(() => expect(tabNames()).toEqual(["steward 1"]));
  });

  it("is what was typed in the picker's Name field, and the core is told it", async () => {
    const { asked: asks } = core();
    render(<App />);

    await openAChat("billing bug");

    await waitFor(() => expect(tabNames()).toEqual(["billing bug"]));
    expect(asked(asks, "start_chat")[0].args).toMatchObject({ name: "1", label: "billing bug" });
  });

  it("is the default when the Name field is left empty, and no name is sent", async () => {
    const { asked: asks } = core();
    render(<App />);

    await openAChat();

    expect(asked(asks, "start_chat")[0].args).toMatchObject({ label: null });
  });

  it("is the harness and then the number for a chat that adopts no persona", async () => {
    core();
    render(<App />);

    await pressNewChat();
    const picker = await screen.findByRole("dialog", { name: "Start a chat" });
    await userEvent.click(within(picker).getByRole("option", { name: "No persona" }));
    await userEvent.click(within(picker).getByRole("button", { name: "Start chat" }));

    await waitFor(() => expect(tabNames()).toEqual(["claude 1"]));
  });

  it("is refused in the picker, before anything starts, when the core refuses it", async () => {
    core({ refuse: "That name holds a control character or an invisible formatting one." });
    render(<App />);

    await pressNewChat();
    const picker = await screen.findByRole("dialog", { name: "Start a chat" });
    await userEvent.type(within(picker).getByLabelText(/^Name/), "x");
    await userEvent.click(within(picker).getByRole("button", { name: "Start chat" }));

    expect(await within(picker).findByRole("alert")).toHaveTextContent(/invisible formatting/);
  });
});

describe("renaming a tab with one chat (#5343)", () => {
  async function oneChat() {
    const { asked: asks } = core();
    render(<App />);
    await openAChat();
    await waitFor(() => expect(tabNames()).toEqual(["steward 1"]));
    return asks;
  }
  const dialog = () => screen.getByRole("dialog", { name: /^Rename/ });
  const box = () => within(dialog()).getByLabelText("New name");

  it("opens on a double-click", async () => {
    await oneChat();

    await userEvent.dblClick(theTab());

    await screen.findByRole("dialog", { name: "Rename steward 1" });
  });

  it("says it renames the chat, Enter renames it, and the keyboard goes back to the tab", async () => {
    const asks = await oneChat();
    theTab().focus();

    await userEvent.keyboard("{F2}");
    expect(await screen.findByRole("dialog", { name: "Rename steward 1" })).toHaveTextContent(
      "This tab shows one chat, so this renames the chat: on its tab, in the list and on its head.",
    );
    await waitFor(() => expect(box()).toHaveFocus());
    expect(box()).toHaveValue("steward 1");
    await userEvent.keyboard("{Control>}a{/Control}billing bug{Enter}");

    await waitFor(() => expect(tabNames()).toEqual(["billing bug"]));
    // The session and the name, and nothing about the harness's own name.
    expect(asked(asks, "rename_chat").map((one) => one.args)).toEqual([
      { plane: PLANE, session: 1, label: "billing bug" },
    ]);
    // The keyboard is back on the tab it renamed.
    await waitFor(() => expect(theTab()).toHaveFocus());
  });

  it("changes nothing on Escape, asks the core nothing, and hands the keyboard back to the tab", async () => {
    const asks = await oneChat();
    theTab().focus();

    await userEvent.keyboard("{F2}");
    await waitFor(() => expect(box()).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}never mind{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(tabNames()).toEqual(["steward 1"]);
    expect(asked(asks, "rename_chat")).toEqual([]);
    await waitFor(() => expect(theTab()).toHaveFocus());
  });

  it("opens with F2 on a focused tab, the platform's rename key, and not the palette", async () => {
    await oneChat();
    theTab().focus();

    await userEvent.keyboard("{F2}");

    await screen.findByRole("dialog", { name: "Rename steward 1" });
    expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
  });

  it("opens from the tab's own menu", async () => {
    await oneChat();

    fireEvent.contextMenu(theTab());
    await userEvent.click(await screen.findByRole("menuitem", { name: "Rename…" }));

    await screen.findByRole("dialog", { name: "Rename steward 1" });
  });

  it("takes the default back when the name is cleared", async () => {
    await oneChat();
    await userEvent.dblClick(theTab());
    await waitFor(() => expect(box()).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}billing bug{Enter}");
    await waitFor(() => expect(tabNames()).toEqual(["billing bug"]));

    await userEvent.dblClick(theTab());
    await waitFor(() => expect(box()).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}{Backspace}{Enter}");

    await waitFor(() => expect(tabNames()).toEqual(["steward 1"]));
  });

  it("keeps Delete and Backspace in the box: typing never ends the chat", async () => {
    const asks = await oneChat();

    await userEvent.dblClick(theTab());
    await waitFor(() => expect(box()).toHaveFocus());
    await userEvent.keyboard("{Backspace}{Backspace}{Delete}");

    expect(box()).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(asked(asks, "close_session")).toEqual([]);
  });

  it("keeps F2 typed into the box from opening the palette", async () => {
    const asks = await oneChat();

    await userEvent.dblClick(theTab());
    await waitFor(() => expect(box()).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}half{F2}");

    expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
    expect(box()).toHaveValue("half");
    expect(asked(asks, "rename_chat")).toEqual([]);
  });

  it("says why when the core refuses the name, and keeps the one it had", async () => {
    await oneChat();
    clearMocks();
    core({ refuse: "That name is 80 characters long, and a chat's name is at most 64." });

    await userEvent.dblClick(theTab());
    await waitFor(() => expect(box()).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}too long{Enter}");

    expect(await within(dialog()).findByRole("alert")).toHaveTextContent(/at most 64/);
    await userEvent.keyboard("{Escape}");
    expect(tabNames()).toEqual(["steward 1"]);
  });

  it("names only the tab with Only this tab, leaves the chat's name, and can use the chat name again", async () => {
    const asks = await oneChat();

    await userEvent.dblClick(theTab());
    await waitFor(() => expect(box()).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}the build");
    await userEvent.click(within(dialog()).getByRole("checkbox", { name: "Only this tab" }));
    await userEvent.click(within(dialog()).getByRole("button", { name: "Rename tab" }));

    await waitFor(() => expect(tabNames()).toEqual(["the build"]));
    // The chat keeps its name: the core is told nothing (`Rename.window.test.tsx` reads the
    // list and the head).
    expect(asked(asks, "rename_chat")).toEqual([]);

    await userEvent.dblClick(theTab());
    const again = await screen.findByRole("dialog", { name: "Rename the build" });
    expect(within(again).getByRole("checkbox", { name: "Only this tab" })).toBeChecked();
    await userEvent.click(within(again).getByRole("button", { name: "Use the chat name again" }));

    await waitFor(() => expect(tabNames()).toEqual(["steward 1"]));
  });

  it("opens Rename chat from the pane's ⋯, on the chat's own name", async () => {
    await oneChat();

    await userEvent.click(await screen.findByRole("button", { name: "More for steward 1" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Rename chat…" }));

    const asking = await screen.findByRole("dialog", { name: "Rename chat steward 1" });
    expect(asking).toHaveTextContent(
      "This renames the chat everywhere: in the list, on its head and on a tab named for it.",
    );
    await waitFor(() => expect(within(asking).getByLabelText("New name")).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}billing bug{Enter}");

    // The tab it leads follows it.
    await waitFor(() => expect(tabNames()).toEqual(["billing bug"]));
  });

  it("opens Rename chat from the palette's row", async () => {
    await oneChat();

    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.keyboard("rename chat steward 1{Enter}");

    await screen.findByRole("dialog", { name: "Rename chat steward 1" });
  });

  it("names the chat by it wherever the chat is named: the palette's rows", async () => {
    await oneChat();
    await userEvent.dblClick(theTab());
    await waitFor(() => expect(box()).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}billing bug{Enter}");
    await waitFor(() => expect(tabNames()).toEqual(["billing bug"]));

    await userEvent.keyboard("{Meta>}k{/Meta}");
    const palette = await screen.findByRole("dialog", { name: "Command palette" });

    // Its End…, the one row that ends it (#4756), and its tab's close, which ends nothing.
    expect(
      within(palette)
        .getAllByRole("option", { name: /^End chat billing bug/ })
        .map((row) => row.textContent),
    ).toEqual([expect.stringContaining("End chat billing bug…")]);
    expect(
      within(palette).getAllByRole("option", { name: /^Close billing bug/ }).length,
    ).toBeGreaterThan(0);
  });
});

describe("a chat's name at a relaunch", () => {
  it("comes back as the name it was given", async () => {
    core({ open: [putBack(3, "3", "billing bug")] });
    render(<App />);

    await waitFor(() => expect(tabNames()).toEqual(["billing bug"]));
  });

  it("comes back as the harness and then the number where it adopted no persona", async () => {
    core({ open: [{ ...putBack(3, "3", null), persona: null, profile: "work" }] });
    render(<App />);

    // The harness, not the profile: `work` is the operator's word for an account.
    await waitFor(() => expect(tabNames()).toEqual(["claude 3"]));
  });

  it("comes back as its default where none was given", async () => {
    core({ open: [putBack(3, "3", null)] });
    render(<App />);

    await waitFor(() => expect(tabNames()).toEqual(["steward 3"]));
  });
});
