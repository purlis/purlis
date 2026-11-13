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
import type { ChatBlocks, ChatLog, OpenChat } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { chatList, pressMenuPath } from "./menus.testkit";
import { findTree } from "./ui/tree/tree.testkit";
import { stripNamed } from "./test-strips";

/**
 * **A block shows as a marker on its chat: "2 new blocked"** (R-172, R-174, R-175, R-176,
 * R-301; T1-30) against the whole window, with a stand-in core: its count is the core's count of
 * verified blocks after the chat's last look (`chat_blocks`), and the last look is kept in the
 * seen record the shipped New markers keep theirs in (`inbox_seen`, `see_inbox`).
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" tabIndex={-1}>
      session {session}
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

beforeEach(() => globalThis.localStorage.clear());
afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

function chat(over: Partial<OpenChat> & Pick<OpenChat, "session" | "name">): OpenChat {
  return {
    cwd: ALPHA,
    harness: "claude",
    in_front: false,
    resumed: null,
    fresh: null,
    profile: "claude",
    persona: "steward",
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
    ...over,
  };
}

/** Chat 3 is in front, on screen; chat 7 is in a tab behind it. */
const CHATS: OpenChat[] = [
  chat({ session: 3, name: "3", in_front: true }),
  chat({ session: 7, name: "talk", label: "talk", persona: "devops" }),
];

/** One block the stand-in core holds: its id, and whether its report is unverified. */
type Held = { id: string; unverified?: boolean };

/**
 * The stand-in core. `blocks` are each chat's blocks, oldest first, and may be added to; the
 * count it answers is the verified ones after the look the window asks from, as the core's
 * `chatlog::Blocks::since` is.
 */
function core(blocks: Record<number, Held[]>) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const seen = new Set<string>();
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: given });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          workspaces: [{ name: "alpha", path: ALPHA, vision: "Ship it", todos: [], chats: CHATS }],
          personas: ["steward", "devops"],
          persona: "steward",
          unfiled: [],
        };
      if (cmd === "opened_chats") return CHATS;
      if (cmd === "reopened_layout") return aTabForEachChat(CHATS, () => "alpha");
      if (
        [
          "reopened_views",
          "chat_states",
          "chats_that_would_not_start",
          "running_sessions",
          "extension_views",
          "extension_commands",
          "extension_panels",
          "extensions_on",
        ].includes(cmd)
      )
        return [];
      if (cmd === "inbox_seen") return [...seen];
      if (cmd === "see_inbox") {
        for (const key of given.keys as string[]) seen.add(key);
        return [...seen];
      }
      if (cmd === "chat_log") {
        const read: ChatLog = { known: true, rows: [] };
        return read;
      }
      if (cmd === "chat_blocks")
        return (given.asked as { session: number; after: string | null }[]).map(
          ({ session, after }): ChatBlocks => {
            const all = blocks[session] ?? [];
            const later = all.filter((one) => after === null || one.id > after);
            return {
              session,
              blocks: later.filter((one) => one.unverified !== true).length,
              unverified: later.filter((one) => one.unverified === true).length,
              latest: all.at(-1)?.id ?? null,
            };
          },
        );
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
    every: () => asked.map(({ cmd }) => cmd),
    seen,
  };
}

/** Chat `name`'s row in the sidebar. */
async function rowOf(name: string): Promise<HTMLElement> {
  const tree = await findTree("Chats of this project");
  return waitFor(() => {
    const found = Array.from(tree.querySelectorAll<HTMLElement>('[role="row"]')).find(
      (one) => one.querySelector(".session")?.textContent === name,
    );
    if (found === undefined) throw new Error(`no row is named ${name}`);
    return found;
  });
}

/** What the window does as it comes back into focus: it reads the counts again. */
async function refocused(): Promise<void> {
  await act(async () => {
    window.dispatchEvent(new Event("focus"));
    await Promise.resolve();
  });
}

describe("the blocked marker", () => {
  it("the marker reads 2 new blocked on the sidebar row and the pane band", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    core({ 7: [{ id: "01A" }, { id: "01B" }] });
    render(<App />);
    await screen.findAllByTestId("pane");

    const row = await rowOf("talk");
    expect(
      await within(row).findByRole("button", { name: "2 new blocked" }, { timeout: 3000 }),
    ).toBeInTheDocument();
    // Chat 3 has no block, and no marker.
    expect(within(await rowOf("steward 3")).queryByRole("button", { name: /blocked/ })).toBeNull();

    // On its pane's band too, while the window does not have the keyboard: it is not looked at.
    await userEvent.click(within(stripNamed("Tabs")).getByRole("tab", { name: /talk/ }));
    const band = document.querySelector<HTMLElement>(".pane-chips");
    expect(band).not.toBeNull();
    expect(
      await within(band as HTMLElement).findByRole("button", { name: "2 new blocked" }),
    ).toBeInTheDocument();
  });

  it("the marker counts blocks since the person last looked", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const held: Record<number, Held[]> = { 7: [{ id: "01A" }, { id: "01B" }] };
    const { asked, seen } = core(held);
    render(<App />);
    await screen.findAllByTestId("pane");
    const row = await rowOf("talk");
    const marker = await within(row).findByRole(
      "button",
      { name: "2 new blocked" },
      { timeout: 3000 },
    );

    // A look: its Chat log opened from the marker. The look is the latest block it counted.
    await userEvent.click(marker);
    await waitFor(() => expect(seen.has("blocks:7:01B")).toBe(true));
    await waitFor(() => expect(within(row).queryByRole("button", { name: /blocked/ })).toBeNull());

    // Only a later block counts, asked from the last look.
    held[7] = [...held[7], { id: "01C" }];
    await refocused();
    expect(
      await within(row).findByRole("button", { name: "1 new blocked" }, { timeout: 3000 }),
    ).toBeInTheDocument();
    expect(asked("chat_blocks").at(-1)?.asked).toContainEqual({ session: 7, after: "01B" });
  });

  it("the marker clears when the chat is on screen or its Chat log is opened", async () => {
    const focused = vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const held: Record<number, Held[]> = { 7: [{ id: "01A" }], 3: [{ id: "01P" }] };
    const { seen } = core(held);
    render(<App />);
    await screen.findAllByTestId("pane");
    const talk = await rowOf("talk");
    const three = await rowOf("steward 3");
    await within(talk).findByRole("button", { name: "1 new blocked" }, { timeout: 3000 });
    await within(three).findByRole("button", { name: "1 new blocked" });

    // On screen in the focused window: chat 3, in front, is looked at as the window has focus.
    focused.mockReturnValue(true);
    await refocused();
    await waitFor(() => expect(seen.has("blocks:3:01P")).toBe(true));
    await waitFor(() =>
      expect(within(three).queryByRole("button", { name: /blocked/ })).toBeNull(),
    );
    // Chat 7, behind it, is not.
    expect(seen.has("blocks:7:01A")).toBe(false);
    expect(within(talk).getByRole("button", { name: "1 new blocked" })).toBeInTheDocument();

    // Its Chat log opened, from its list, is a look too.
    await chatList("talk");
    await pressMenuPath("Open", "Chat log");
    await waitFor(() => expect(seen.has("blocks:7:01A")).toBe(true));
  });

  it("a block sends no notification and no announcement", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const held: Record<number, Held[]> = {};
    const { every } = core(held);
    render(<App />);
    await screen.findAllByTestId("pane");
    const row = await rowOf("talk");

    held[7] = [{ id: "01A" }, { id: "01B" }];
    await refocused();
    await within(row).findByRole("button", { name: "2 new blocked" }, { timeout: 3000 });

    // No system notification of any kind was asked for.
    expect(every().filter((cmd) => cmd.startsWith("plugin:notification|"))).toEqual([]);
    // And no live region says it: the marker is seen, never announced.
    const live = Array.from(
      document.querySelectorAll(
        '[aria-live]:not([aria-live="off"]), [role="status"], [role="alert"]',
      ),
    );
    expect(live.map((one) => one.textContent ?? "").join(" ")).not.toMatch(/blocked/i);
  });

  it("the marker opens the Chat log filtered to blocks", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    core({ 7: [{ id: "01A" }, { id: "01B" }, { id: "01C", unverified: true }] });
    render(<App />);
    await screen.findAllByTestId("pane");
    const row = await rowOf("talk");

    await userEvent.click(
      await within(row).findByRole("button", { name: "2 new blocked" }, { timeout: 3000 }),
    );

    await waitFor(() =>
      expect(within(stripNamed("Tabs")).getByRole("tab", { selected: true }).textContent).toContain(
        "Chat log · talk",
      ),
    );
    const filter = await screen.findByRole("button", { name: "Blocks only" });
    expect(filter).toHaveAttribute("aria-pressed", "true");
    // The count of unverified reports is in the Chat log, never on the sidebar.
    expect(await screen.findByText("2 blocked, 1 unverified")).toBeInTheDocument();
  });
});
