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
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { Phase, SavedRecord, SmartCloseOffer } from "./bindings";
import { forgetThisLaunch } from "./regions";
import { showNotices, stripNamed } from "./test-strips";
import { tipOf } from "./ui/tooltip/tooltip.testkit";
import { findResult } from "./actionResult.testkit";
import { askToEnd } from "./menus.testkit";
import { aTabForEachChat } from "./laidOut.testkit";

/**
 * **Smart close, as the window draws it** (ADR 0064, SI-8c): the close dialog's three answers,
 * the tab wrapping up while the chat writes its record, the menu's Cancel, and the tab going
 * when — and only when — the core says the record landed.
 *
 * The core's side (the prompt, the queue, the record, the timeout, typing cancelling it) is
 * `planes.rs`'s smart-close tests, on a real terminal and a real socket. This is the window
 * reading what the core tells it.
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
  vi.restoreAllMocks();
  // A test that opened Notices leaves the next one on Memory, where a launch starts (P-14).
  forgetThisLaunch();
});

const PLANE = "/home/dev/plane";

const SIDEBAR = {
  root: PLANE,
  workspaces: [
    {
      name: "alpha",
      path: `${PLANE}/workspaces/alpha`,
      vision: "Ship it",
      todos: [],
      chats: [],
    },
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

/** A chat with turns behind it, waiting: Smart close is offered and is the default. */
const OFFERED: SmartCloseOffer = { available: true, why: null, close_first: false };

/** Answers every command, with `offer` as the core's answer about Smart close, and `next` as
 *  the number the core gives the next chat it starts. */
function core(
  offer: SmartCloseOffer = OFFERED,
  next?: () => number,
): { asked: { cmd: string; args: unknown }[] } {
  const asked: { cmd: string; args: unknown }[] = [];
  let opened = 0;
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "plane_sidebar") return SIDEBAR;
      if (cmd === "running_sessions") return [];
      if (cmd === "opened_chats") return [];
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "start_details") return [];
      if (cmd === "start_options" || cmd === "start_options_first") return START_OPTIONS;
      if (cmd === "start_chat") return { session: next?.() ?? ++opened };
      if (cmd === "smart_close_offer") return offer;
      if (cmd === "smart_close") return "sent";
      if (cmd === "smart_closing") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked };
}

/**
 * New chat in the empty pane in front (#4758): a chat takes the focused pane, so where a chat
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

async function openAChat() {
  await pressNewChat();
  await userEvent.click(await screen.findByRole("button", { name: "Start chat" }));
}

/** The core telling the window a step of chat `session`'s smart close. */
async function step(
  session: number,
  phase: Phase,
  plane = PLANE,
  record: SavedRecord | null = null,
) {
  await act(() => emit("smart-close", { plane, session, phase, record }));
}

const tabOf = (name: string) =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .find((tab) => tab.querySelector(".tab-name")?.textContent === name);

/** Whether the tab called `name` wears the wrapping-up look: a tab still, never a chip (#4760). */
const wrapsUp = (name: string) =>
  tabOf(name)?.closest(".tab")?.hasAttribute("data-wrapping-up") === true;

/** Whether any tab of the strip is drawn as the Smart close chip it used to shrink to. */
const anyChip = () => stripNamed("Tabs").querySelector(".tab[data-chip]") !== null;

/**
 * **The Smart close that stopped, as the Inbox lists it** (#1693): an update about its chat,
 * saying why. The hand's list has no row for it.
 */
async function stoppedUpdate(name: RegExp) {
  // Pressed only where it is not shown already: a press on the view in front puts the side away.
  if (screen.queryByRole("tabpanel", { name: "Updates" }) === null)
    await userEvent.click(
      within(screen.getByRole("tablist", { name: "Attention" })).getByRole("tab", {
        name: /^Updates\b/,
      }),
    );
  const inbox = await screen.findByRole("tabpanel", { name: "Updates" });
  const updates = await within(inbox).findByRole("region", { name: "Updates" });
  // A line of Mine is named by whose it is and what happened (#5649), as its words read.
  return within(updates).findByRole("row", { name });
}

const strip = () =>
  within(stripNamed("Tabs"))
    .getAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent ?? tab.getAttribute("aria-label"));

/** The tab called `name`, which the test needs to be there. */
function theTab(name: string): HTMLElement {
  const tab = tabOf(name);
  if (tab === undefined) throw new Error("no such tab");
  return tab;
}

const asked = (log: { cmd: string; args: unknown }[], cmd: string) =>
  log.filter((one) => one.cmd === cmd).map((one) => one.args);

describe("the close dialog's three answers", () => {
  it("asks Cancel, Close and Smart close, with Smart close first for a chat with turns behind it", async () => {
    const { asked: log } = core();
    render(<App />);
    await openAChat();

    const asking = await askToEnd("steward 1");

    const answers = within(asking).getAllByRole("button");
    expect(answers.map((answer) => answer.textContent)).toEqual(["Cancel", "Close", "Smart close"]);
    await waitFor(() => expect(answers[2]).toHaveFocus());
    expect(asked(log, "smart_close_offer")).toEqual([{ plane: PLANE, session: 1 }]);
  });

  it("starts on Close for a chat that has had at most one turn", async () => {
    core({ available: true, why: null, close_first: true });
    render(<App />);
    await openAChat();

    const asking = await askToEnd("steward 1");

    await waitFor(() =>
      expect(within(asking).getByRole("button", { name: "Close" })).toHaveFocus(),
    );
    expect(within(asking).getByRole("button", { name: "Smart close" })).toBeEnabled();
  });

  it("offers Close only, with the core's reason, where the chat cannot write a record", async () => {
    const why = "This chat is asking you something. Answer it first, then smart close it.";
    core({ available: false, why, close_first: false });
    render(<App />);
    await openAChat();

    const asking = await askToEnd("steward 1");

    const smart = within(asking).getByRole("button", { name: "Smart close" });
    expect(smart).toBeDisabled();
    expect(within(asking).getByText(why)).toBeInTheDocument();
    // Nothing charter knows says which answer is right, so a reflex Return cancels.
    await waitFor(() =>
      expect(within(asking).getByRole("button", { name: "Cancel" })).toHaveFocus(),
    );
  });

  it("starts a smart close on the press and ends nothing", async () => {
    const { asked: log } = core();
    render(<App />);
    await openAChat();

    const asking = await askToEnd("steward 1");
    await userEvent.click(within(asking).getByRole("button", { name: "Smart close" }));

    expect(screen.queryByRole("alertdialog")).toBeNull();
    await vi.waitFor(() =>
      expect(asked(log, "smart_close")).toEqual([{ plane: PLANE, session: 1 }]),
    );
    expect(asked(log, "close_session")).toEqual([]);
    // The chat stays a tab, in front, its pane on screen (#4760).
    expect(tabOf("steward 1")).toBeDefined();
    expect(anyChip()).toBe(false);
  });

  it("closes as it always did when Close is pressed", async () => {
    const { asked: log } = core();
    render(<App />);
    await openAChat();

    const asking = await askToEnd("steward 1");
    await userEvent.click(within(asking).getByRole("button", { name: "Close" }));

    expect(asked(log, "close_session")).toEqual([{ plane: PLANE, session: 1 }]);
    expect(asked(log, "smart_close")).toEqual([]);
  });
});

describe("a chat wrapping up", () => {
  it("stays a tab with its name, wearing the wrap-up look, and keeps its pane (#4760)", async () => {
    core();
    render(<App />);
    await openAChat();

    await step(1, "sent");

    await waitFor(() => expect(wrapsUp("steward 1")).toBe(true));
    expect(anyChip()).toBe(false);
    // Its pane stays on screen; its head breathes where the plane lists the chat
    // (`PaneHead.window.test.tsx`).
    expect(screen.getByTestId("pane")).toHaveTextContent("session 1");
  });

  it("offers Cancel smart close first on its list, and cancelling asks the core (#4753)", async () => {
    const { asked: log } = core();
    render(<App />);
    await openAChat();
    await step(1, "queued");
    await waitFor(() => expect(wrapsUp("steward 1")).toBe(true));

    // On the chat's own list, not its tab's (#4753): first, as the row for its state now.
    await userEvent.click(await screen.findByRole("button", { name: "More for steward 1" }));
    const menu = await screen.findByRole("menu");
    const first = within(menu).getAllByRole("menuitem")[0];
    expect(first).toHaveAccessibleName("Cancel smart close");
    await userEvent.click(first);

    await vi.waitFor(() =>
      expect(asked(log, "cancel_smart_close")).toEqual([{ plane: PLANE, session: 1 }]),
    );
    expect(asked(log, "close_session")).toEqual([]);
  });

  it("goes back to normal when the core says it was cancelled — the operator typed into it", async () => {
    core();
    render(<App />);
    await openAChat();
    await step(1, "sent");
    await waitFor(() => expect(wrapsUp("steward 1")).toBe(true));

    await step(1, "cancelled");

    await waitFor(() =>
      expect(theTab("steward 1").closest(".tab")).not.toHaveAttribute("data-wrapping-up"),
    );
    expect(tipOf(theTab("steward 1")) ?? "").not.toContain("Wrapping up");
    expect(screen.queryByRole("menuitem", { name: /Cancel smart close/ })).toBeNull();
  });

  it("leaves its pane on the ended band when the core says the record landed, and never ends the chat a second time (#5019)", async () => {
    const { asked: log } = core();
    render(<App />);
    await openAChat();
    await step(1, "sent");

    await step(1, "closed");

    // It ends like End… (spec #4750 story 62): its tab is back from the background, in front,
    // its pane on the ended band.
    expect(await screen.findByRole("status", { name: /ended/i })).toBeInTheDocument();
    expect(tabOf("steward 1")?.getAttribute("aria-selected")).toBe("true");
    expect(asked(log, "close_session")).toEqual([]);
  });

  it("stays open, and says so, when no record arrived in time", async () => {
    core();
    render(<App />);
    await openAChat();
    await step(1, "sent");

    await step(1, "no_record");

    expect(
      await findResult(
        "No session record arrived from steward 1 within five minutes, so it was left open.",
      ),
    ).toBeInTheDocument();
    expect(theTab("steward 1").closest(".tab")).not.toHaveAttribute("data-wrapping-up");
  });

  it("says the record was not written when the chat ended on its own", async () => {
    core();
    render(<App />);
    await openAChat();
    await step(1, "sent");

    await step(1, "ended");

    expect(
      await findResult("steward 1 ended before it wrote its session record."),
    ).toBeInTheDocument();
  });

  it("says so when a Smart close it held no pass for writes no record in time", async () => {
    // #1361: the core watches a `/smart-close` with no pass behind it, which was never
    // wrapping up, and tells the window when no record came. Never a silent miss.
    const { asked: log } = core();
    render(<App />);
    await openAChat();

    await step(1, "no_record");

    expect(
      await findResult(
        "No session record arrived from steward 1 within five minutes, so it was left open.",
      ),
    ).toBeInTheDocument();
    expect(tabOf("steward 1")).toBeDefined();
    expect(asked(log, "close_session")).toEqual([]);
  });

  /** The Notice a Smart close with no pass leaves on chat `session`'s tab (#1361). */
  const keptOpenNotice = (session: number) =>
    document.querySelector<HTMLElement>(`[data-cause="smart-close-kept-open:${session}"]`);

  it("stays open after a Smart close it held no pass for, says so, and End chat ends it plainly", async () => {
    const { asked: log } = core();
    render(<App />);
    await openAChat();

    await step(1, "kept_open", PLANE, { path: "sessions/a.md", title: "A" });

    await waitFor(() => expect(keptOpenNotice(1)).not.toBeNull());
    // Listed in Notices, which the person opens: its arrival opens nothing (ADR 0121).
    await showNotices();
    const notice = keptOpenNotice(1) as HTMLElement;
    expect(notice).toHaveTextContent(
      "steward 1 saved its session record. purlis did not see " +
        "/purlis-smart-close typed in it, so it was not ended.",
    );
    expect(tabOf("steward 1")).toBeDefined();
    expect(asked(log, "close_session")).toEqual([]);

    await userEvent.click(within(notice).getByRole("button", { name: "End chat" }));

    // Ended as End… ends it: its pane stays, on the ended band (#5019).
    expect(await screen.findByRole("status", { name: /ended/i })).toBeInTheDocument();
    expect(asked(log, "close_session")).toEqual([{ plane: PLANE, session: 1 }]);
    expect(keptOpenNotice(1)).toBeNull();
  });

  it("puts that Notice away on Dismiss and closes nothing", async () => {
    const { asked: log } = core();
    render(<App />);
    await openAChat();
    await step(1, "kept_open", PLANE, { path: "sessions/a.md", title: "A" });
    await waitFor(() => expect(keptOpenNotice(1)).not.toBeNull());
    await showNotices();

    await userEvent.click(
      within(keptOpenNotice(1) as HTMLElement).getByRole("button", { name: /Dismiss/ }),
    );

    await waitFor(() => expect(keptOpenNotice(1)).toBeNull());
    expect(tabOf("steward 1")).toBeDefined();
    expect(asked(log, "close_session")).toEqual([]);
  });

  it("forgets that Notice when its tab closes another way, so a chat of the same number has none", async () => {
    // #1361: the Notice went with its tab, but the chat stayed listed until the window
    // reloaded, and the next chat the core numbered the same drew it again.
    core(OFFERED, () => 1);
    render(<App />);
    await openAChat();
    await step(1, "kept_open", PLANE, { path: "sessions/a.md", title: "A" });
    await waitFor(() => expect(keptOpenNotice(1)).not.toBeNull());

    // Closed by a Smart close that went through after all: not the Notice's own buttons. Its
    // pane is left on the ended band (#5019), whose Clear pane takes the tab's chat away.
    await step(1, "closed");
    const band = await screen.findByRole("status", { name: /ended/i });
    await userEvent.click(within(band).getByRole("button", { name: "Clear pane" }));
    await screen.findByText("Nothing in this pane yet");
    await openAChat();
    // The core numbers it 1 again; the window names it afresh.
    await waitFor(() => expect(tabOf("steward 2")).toBeDefined());
    await act(() => new Promise((done) => setTimeout(done, 50)));

    expect(keptOpenNotice(1)).toBeNull();
  });

  it("hears nothing about another project's chat of the same number", async () => {
    core();
    render(<App />);
    await openAChat();

    await step(1, "closed", "/somewhere/else");

    expect(tabOf("steward 1")).toBeDefined();
  });
});

/** One chat as the core reports it at a launch, named exactly `name` on its tab. */
function restored(session: number, name: string, { inFront = false, pinned = false } = {}) {
  return {
    session,
    name,
    cwd: `${PLANE}/workspaces/alpha`,
    harness: null,
    in_front: inFront,
    resumed: null,
    fresh: null,
    // On a profile, so a chat and not a shell: End… on a shell has no Smart close (#4761).
    profile: "claude",
    persona: null,
    unreported: null,
    guessed: null,
    pinned,
  };
}

/** The core with `open` put back at the launch, `one` pinned; Smart close answers `began`. */
function coreWith(
  open = [
    restored(1, "one", { pinned: true }),
    restored(2, "two", { inFront: true }),
    restored(3, "three"),
  ],
  began: () => unknown = () => "sent",
): { asked: { cmd: string; args: unknown }[] } {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "plane_sidebar")
        return { ...SIDEBAR, workspaces: [{ ...SIDEBAR.workspaces[0], chats: open }] };
      if (cmd === "opened_chats") return open;
      // A chat gets a tab only from the stored layout (#4758): each has one, on alpha's strip,
      // pinned where the chat is.
      if (cmd === "reopened_layout") {
        const laid = aTabForEachChat(open, () => "alpha");
        return {
          ...laid,
          tabs: laid.tabs.map((tab, at) => ({ ...tab, pinned: open[at].pinned })),
        };
      }
      if (cmd === "running_sessions") return [];
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "smart_close_offer") return OFFERED;
      if (cmd === "smart_close") return began();
      if (cmd === "smart_closing") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked };
}

/** Smart close on the tab called `name`, answered through the close dialog. */
async function smartClose(name: string) {
  const asking = await askToEnd(name);
  await userEvent.click(within(asking).getByRole("button", { name: "Smart close" }));
}

const selected = () =>
  within(stripNamed("Tabs"))
    .getAllByRole("tab")
    .find((tab) => tab.getAttribute("aria-selected") === "true");

describe("a chat wrapping up among several (#4760: no tab goes into the background)", () => {
  it("keeps its tab in its place on the strip, and turns none into a chip", async () => {
    coreWith();
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));

    await step(3, "sent");

    await waitFor(() => expect(wrapsUp("three")).toBe(true));
    expect(strip()).toEqual(["one", "two", "three"]);
    expect(anyChip()).toBe(false);
  });

  it("keeps the front where it was, so the chat's pane stays on screen", async () => {
    coreWith();
    render(<App />);
    await waitFor(() => expect(selected()?.textContent).toContain("two"));

    await smartClose("two");

    await waitFor(() => expect(wrapsUp("two")).toBe(true));
    expect(selected()?.querySelector(".tab-name")?.textContent).toBe("two");
    expect(screen.getByTestId("pane")).toHaveTextContent("session 2");
  });

  it("keeps the last chat's pane in front, with no empty state", async () => {
    coreWith([restored(1, "only", { inFront: true })]);
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["only"]));

    await smartClose("only");

    await waitFor(() => expect(wrapsUp("only")).toBe(true));
    expect(screen.queryByTestId("empty-window")).toBeNull();
    expect(strip()).toEqual(["only"]);
  });

  it("ends like End… when its record lands: its tab stays on the ended band, and a quiet notice offers to open the record (#5019)", async () => {
    coreWith();
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    await step(2, "sent");
    const record = {
      path: "workspaces/alpha/sessions/20260928-160000-ship-it.md",
      title: "Ship it",
    };

    await step(2, "closed", PLANE, record);

    // Its pane is left on the ended band (spec #4750 story 62), its tab where it was.
    expect(strip()).toEqual(["one", "two", "three"]);
    // Listed in Notices, which the person opens: a record landing opens nothing (ADR 0121).
    await showNotices();
    const notice = (await screen.findByText(/^Session saved — Ship it/)).closest<HTMLElement>(
      '[role="status"]',
    );
    if (notice === null) throw new Error("the notice is not a status");
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await userEvent.click(within(notice).getByRole("button", { name: "Open record" }));
    await waitFor(() => expect(tabOf("Session · Ship it")).toBeDefined());
    expect(screen.queryByText(/^Session saved — Ship it/)).toBeNull();
  });

  it("brings Notices on screen for no record that lands, a task's closing with its asker included (ADR 0121)", async () => {
    // A task closed with the chat that asked for it is smart-closed by the core, with no press
    // of the person's (`handoff::close_reported`), and its record lands minutes later. Its
    // "Session saved" opened Notices over Memory each time, which the person read as Notices
    // opening on its own for every new one.
    coreWith();
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    // The Panel's right half opens on Memory (P-14): Notices is behind it (#5645).
    await screen.findByRole("tabpanel", { name: "Memory" });

    await step(3, "closed", PLANE, {
      path: "workspaces/alpha/sessions/20261112-100000-reported.md",
      title: "Reported",
    });

    await waitFor(() =>
      expect(
        document.querySelector('[data-scope=notice-list] [data-cause="session-saved"]'),
      ).not.toBeNull(),
    );
    expect(screen.getByRole("tabpanel", { name: "Memory" })).toBeInTheDocument();
    expect(screen.queryByRole("tabpanel", { name: "Notices" })).toBeNull();
  });

  it("goes back to normal in its place when no record arrives, and the Inbox says why", async () => {
    coreWith();
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    await step(2, "sent");
    await waitFor(() => expect(wrapsUp("two")).toBe(true));

    await step(2, "no_record");

    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    expect(
      await findResult(
        "No session record arrived from two within five minutes, so it was left open.",
      ),
    ).toBeInTheDocument();
    expect(
      await stoppedUpdate(/^two: Smart close stopped — no session record arrived/),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /needs you/ })).toBeNull();
  });

  it("goes back to normal in its place when the chat ends before its record, listed too", async () => {
    coreWith();
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    await step(1, "sent");
    await waitFor(() => expect(wrapsUp("one")).toBe(true));

    await step(1, "ended");

    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    expect(await stoppedUpdate(/^one: Smart close stopped — it ended/)).toBeInTheDocument();
  });

  it("goes back to normal in its place when its queued prompt could not be sent, and says so", async () => {
    coreWith();
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    await step(2, "queued");
    await waitFor(() => expect(wrapsUp("two")).toBe(true));

    await step(2, "not_sent");

    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    expect(
      await findResult("Smart close could not send two its prompt, so it was left open."),
    ).toBeInTheDocument();
    expect(
      await stoppedUpdate(/^two: Smart close stopped — its prompt could not be sent/),
    ).toBeInTheDocument();
  });

  it("stays in its place when Smart close could not start, listed too", async () => {
    coreWith(undefined, () => {
      throw "That chat is not open any more.";
    });
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));

    await smartClose("two");

    expect(await findResult("That chat is not open any more.")).toBeInTheDocument();
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    expect(await stoppedUpdate(/^two: Smart close did not start/)).toBeInTheDocument();
  });

  it("goes back to normal in its place when cancelled, and nothing is listed", async () => {
    coreWith();
    render(<App />);
    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    await step(3, "sent");
    await waitFor(() => expect(wrapsUp("three")).toBe(true));

    await step(3, "cancelled");

    await waitFor(() => expect(strip()).toEqual(["one", "two", "three"]));
    expect(screen.queryByRole("button", { name: /needs you/ })).toBeNull();
  });
});
