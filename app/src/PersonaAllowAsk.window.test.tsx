// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { PersonaHosts, SandboxState, Shown } from "./bindings";
import { askRows, forgetInboxOpen } from "./test-inbox";
import { forgetThisLaunch } from "./regions";
import { showNotices } from "./test-strips";
import { SETTLE_MS } from "./TaskBlocksNotice";

/**
 * **A persona whose hosts or tools wait for an Allow is a Blocking ask** (ADR 0121, amended
 * 2026-11-12), on the real window with the core mocked. The person's report of 2026-11-12:
 * steward's tools had changed since their Allow (`changed: true` in this machine's grants), so
 * steward's chats were refused their tool, and only the chat's terminal said so. The one thing
 * that asked was a Notice among many, which they never found; they allowed it in Settings ›
 * Sandbox. Now the ✋ counts it, its row in Asks answers it with the Notice's own Allow, sent the
 * digest the row showed, and its Notice is drawn from the same ask, so one answer settles both.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";

/** steward's tools, changed since this machine allowed them. */
const STEWARD: PersonaHosts = {
  persona: "steward",
  hosts: [],
  reached: [],
  tools: ["gh with token from vault devops as GH_TOKEN"],
  digest: "steward-digest-2",
  allowed: false,
  default: false,
  waiting: true,
};

const WAITING: SandboxState = {
  on: true,
  offer: false,
  said: null,
  never: [],
  hosts_changed: null,
  presets_changed: null,
  presets: [],
  persona_hosts: [STEWARD],
  besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
  policy: null,
  expects: null,
};

/** steward's Allow as the core lists it among the project's asks (`asking::consents`). */
const ASK: Shown = {
  session: 0,
  ask: "persona-allow:steward",
  says:
    "You allowed steward's tools on this machine and they have changed since, so no chat gets " +
    "them until you allow them again. Allowed, chats as steward run gh with token from vault " +
    "devops as GH_TOKEN.",
  options: [
    { id: "allow", label: "Allow for steward chats", allows: true },
    { id: "not-now", label: "Not now", allows: false },
  ],
  source: "persona-allow",
  chain: ["steward"],
  answer: { via: "persona-allow", persona: "steward", digest: "steward-digest-2" },
};

/** A core whose persona waits until the window answers it, from either place. */
function core(ask: Shown = ASK) {
  const asked: { cmd: string; args: unknown }[] = [];
  let answered = false;
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [
          { name: "alpha", path: `${PLANE}/workspaces/alpha`, vision: "", todos: [], chats: [] },
        ],
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "plane_doctor") return { rows: [], app_rows: [], full: false, path: "" };
    if (cmd === "sandbox_state")
      return answered ? { ...WAITING, persona_hosts: [{ ...STEWARD, allowed: true }] } : WAITING;
    if (cmd === "allow_persona_hosts") {
      answered = true;
      return { ...WAITING, persona_hosts: [{ ...STEWARD, allowed: true }] };
    }
    if (cmd === "persona_allow_not_now") {
      answered = true;
      return null;
    }
    if (cmd === "asks_waiting")
      return { plane: PLANE, asks: answered ? [] : [ask], with_asking_chat: [] };
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return [];
    if (cmd === "opened_chats" || cmd === "chat_states" || cmd === "running_sessions") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return { asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args) };
}

const hand = () =>
  within(screen.getByRole("tablist", { name: "Attention" })).getByRole("tab", { name: /^Asks/ });
const handCount = () => hand().querySelector('[data-part="count"]');
const notice = () => screen.queryByRole("status", { name: "steward's tools wait for you" });
/** steward's row in Asks. */
const row = () =>
  askRows().find((one) => one.getAttribute("data-source") === "persona-allow") as
    HTMLElement | undefined;
/** The person reads what was drawn for longer than an answer waits for its row to settle. */
const read = () => new Promise((done) => setTimeout(done, SETTLE_MS + 50));

/** From the ✋ alone: the hand is the one thing pressed, and its row is found under it. */
async function fromTheHand(): Promise<HTMLElement> {
  await waitFor(() => expect(handCount()).toHaveAttribute("data-count", "1"));
  // As a person does: a press of the open view would put it away (#6001 presses only then).
  if (hand().getAttribute("aria-expanded") !== "true") await userEvent.click(hand());
  await screen.findByRole("tabpanel", { name: "Asks" });
  return waitFor(() => {
    const one = row();
    expect(one).toBeDefined();
    return one as HTMLElement;
  });
}

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInboxOpen();
  forgetThisLaunch();
});

describe("a persona whose tools wait for an Allow (ADR 0121, amended 2026-11-12)", () => {
  it("is counted on the ✋ and found from it alone, asking with its Notice's own Allow", async () => {
    core();
    render(<App />);

    const found = await fromTheHand();

    expect(within(found).getByText("Allow this persona's hosts and tools?")).toBeInTheDocument();
    expect(found).toHaveTextContent("changed since");
    expect(within(found).getByRole("button", { name: "Allow for steward chats" })).toBeVisible();
    expect(within(found).getByRole("button", { name: "Not now" })).toBeVisible();
    // Nothing behind Answer: its options stand on its row.
    expect(within(found).queryByRole("button", { name: /^Answer/ })).toBeNull();
  });

  it("is allowed on its row as it was shown, which settles its Notice and the ✋ too", async () => {
    const { asked } = core();
    render(<App />);
    const found = await fromTheHand();
    await read();

    await userEvent.click(within(found).getByRole("button", { name: "Allow for steward chats" }));

    await waitFor(() =>
      expect(asked("allow_persona_hosts")).toEqual([
        { plane: PLANE, persona: "steward", digest: "steward-digest-2" },
      ]),
    );
    await waitFor(() => expect(row()).toBeUndefined());
    await waitFor(() => expect(handCount()).toBeNull());
    await showNotices();
    expect(notice()).toBeNull();
  });

  it("is put away from Asks by Not now on its Notice", async () => {
    const { asked } = core();
    render(<App />);
    await waitFor(() => expect(handCount()).toHaveAttribute("data-count", "1"));
    const notices = await showNotices();
    const standing = await within(notices).findByRole("status", {
      name: "steward's tools wait for you",
    });

    await read();
    await userEvent.click(within(standing).getByRole("button", { name: "Not now" }));

    await vi.waitFor(() =>
      expect(asked("persona_allow_not_now")).toEqual([
        { plane: PLANE, persona: "steward", digest: "steward-digest-2" },
      ]),
    );
    await waitFor(() => expect(handCount()).toBeNull());
    expect(notice()).toBeNull();
  });

  it("asks once more before allowing a host that looks like production (decision 8)", async () => {
    const { asked } = core({
      ...ASK,
      risk: {
        class: "production-host",
        why: "Reaches db.prod.example.com, which looks like production",
        confirm: "Allow reaching db.prod.example.com?",
      },
    });
    render(<App />);
    const found = await fromTheHand();
    await read();

    await userEvent.click(within(found).getByRole("button", { name: "Allow for steward chats" }));

    expect(asked("allow_persona_hosts")).toEqual([]);
    const again = within(found).getByRole("group", { name: "Allow reaching db.prod.example.com?" });
    await read();
    await userEvent.click(within(again).getByRole("button", { name: "Allow for steward chats" }));
    await waitFor(() => expect(asked("allow_persona_hosts")).toHaveLength(1));
  });
});
