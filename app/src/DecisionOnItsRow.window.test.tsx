// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { askRows } from "./test-inbox";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { MergeShown, Shown } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { forgetThisLaunch } from "./regions";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { runAsk } from "./secretAsk.testkit";
import { MOVED_JUST_NOW } from "./InboxAsks";
import { dismissNotice } from "./dismissals";
import { cloneVouchCause } from "./CloneVouchNotice";

/**
 * **A decision is answered on its row** (#5998, ADR 0121 amended 2026-11-09), on the real window
 * with the core mocked. A Blocking ask that is a decision (a permission, a run grant, a sandbox
 * host) shows its question as its title and its options on its row, laid out as a chat pane's
 * band lays them out: the narrowest grant, Other scopes…, and the refusal. Nothing is behind an
 * Answer link. An ask that needs typing (a run's value, a reply) keeps its form behind a visible
 * Answer button, and its row says what is needed. The settle guard holds every button.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";

const chat = (session: number) => ({
  session,
  name: `steward ${session}`,
  cwd: `${PLANE}/workspaces/alpha`,
  harness: null,
  in_front: session === 1,
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
});

const PERMISSION: Shown = {
  session: 1,
  ask: "ask-1",
  says: "Run cargo test",
  options: [
    { id: "allow", label: "Allow once", allows: true },
    { id: "always", label: "Allow always", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: ["steward 1"],
  answer: { via: "hook" },
};

/** A run of `curl` whose vault holds every key it is handed: nothing to type, only to decide. */
const RUN_GRANT: Shown = { ...runAsk({ fills: [] }), session: 2, chain: ["steward 2"] };

/** A run of `curl` whose vault lacks the key it is handed: a value to type first. */
const RUN_NEEDS_A_VALUE: Shown = {
  ...runAsk(),
  ask: "run-grant:10",
  answer: { via: "run-grant", id: 10 },
  session: 3,
  chain: ["steward 3"],
};

const HOST: Shown = {
  session: 4,
  ask: "block:4:connect:host:api.example.com",
  says: "Reach api.example.com",
  options: [
    { id: "you", label: "Allow for me on this machine", allows: true },
    { id: "chat", label: "Allow only for this chat", allows: true },
    { id: "project", label: "Allow for everyone in this project", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: ["steward 4"],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
  },
} as Shown;

type Asked = { cmd: string; args: Record<string, unknown> };

/** Chats 1 to 4 in alpha, holding `asks`. */
function core(asks: readonly Shown[]) {
  const chats = [1, 2, 3, 4].map(chat);
  const asked: Asked[] = [];
  let held = [...asks];
  /** What a merge's re-read finds (T1-40, Q248): the facts it gave, a minute later. */
  let rereadGives: Partial<MergeShown> = { read_at: REREAD_AT };
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: a });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "asks_waiting") return { plane: PLANE, asks: held, with_asking_chat: [] };
      if (cmd === "answer_ask") {
        held = held.filter((one) => one.ask !== a.ask);
        return null;
      }
      if (cmd === "answer_run_grant") {
        held = held.filter((one) => one.answer.via !== "run-grant" || one.answer.id !== a.id);
        return null;
      }
      if (cmd === "keep_sandbox_block") {
        held = held.filter((one) => one.source !== "sandbox-host");
        return null;
      }
      if (cmd === "reread_held_merge") {
        const one = held.find(
          (ask) => ask.answer.via === "merge-hold" && ask.answer.head === a.head,
        );
        if (one === undefined || one.answer.via !== "merge-hold")
          throw new Error("this merge is not waiting any more, so nothing was merged");
        const facts = { ...one.answer.facts, ...rereadGives };
        held = held.map((ask) =>
          ask === one && one.answer.via === "merge-hold"
            ? { ...ask, answer: { ...one.answer, facts } }
            : ask,
        );
        return { head: one.answer.head, facts };
      }
      if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return [];
      if (cmd === "finished_tasks") return [];
      if (cmd === "opened_chats") return chats;
      if (cmd === "reopened_layout") return aTabForEachChat(chats, () => "alpha");
      if (cmd === "plane_pins")
        return { project: false, workspaces: ["alpha"], missing: [], order: ["alpha"] };
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: ["steward"],
          persona: "steward",
          unfiled: [],
          workspaces: [
            { name: "alpha", path: `${PLANE}/workspaces/alpha`, vision: "", todos: [], chats },
          ],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
      if (cmd === "extensions_on") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    /** What a merge's re-read finds from now on. */
    rereadFinds: (gives: Partial<MergeShown>) => {
      rereadGives = { read_at: REREAD_AT, ...gives };
    },
    /** The asks the core holds are now `now`, and it says so. */
    moves: async (now: readonly Shown[]) => {
      held = [...now];
      await act(async () => {
        await emit("asks-changed", { plane: PLANE });
        await new Promise((done) => setTimeout(done, 80));
      });
    },
  };
}

beforeEach(() => {
  forgetThisLaunch();
  vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ["setTimeout", "clearTimeout"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

const press = userEvent.setup({ advanceTimers: (ms) => vi.advanceTimersByTime(ms) });

/** The person reads what was drawn for longer than an answer waits for its row to settle. */
const read = () => new Promise((done) => setTimeout(done, SETTLE_MS + 50));

/** The row of chat `who`'s ask in Asks. */
const rowOf = (view: HTMLElement, who: string) =>
  askRows()
    .filter((one) => view.contains(one))
    .find(
      (one) =>
        one.classList.contains("inbox-ask") && one.querySelector(".inbox-who")?.textContent === who,
    ) as HTMLElement;

/** The words on a row's buttons, in order. */
const buttonsOf = (row: HTMLElement) =>
  within(row)
    .getAllByRole("button")
    .map((one) => one.textContent)
    .filter((said) => said !== "");

async function opened(asks: readonly Shown[]) {
  const one = core(asks);
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  const view = await screen.findByRole("tabpanel", { name: "Asks" });
  await waitFor(() =>
    expect(view.querySelectorAll('[role="row"].inbox-ask')).toHaveLength(asks.length),
  );
  return { ...one, view };
}

describe("a decision on its row in Asks (#5998)", () => {
  it("shows a run grant's question as its title and its options on the row, with no Answer", async () => {
    const { view } = await opened([RUN_GRANT]);
    const row = rowOf(view, "steward 2");

    expect(row.querySelector(".inbox-question")).toHaveTextContent("Allow curl to use vault team?");
    expect(buttonsOf(row)).toEqual([
      "Allow once",
      "Other scopes…",
      "Don't allow",
      "Go to chat",
      "Snooze",
      "Stop",
    ]);
    expect(within(row).queryByRole("button", { name: /^Answer/ })).toBeNull();
  });

  it("tells the command Don't allow from the row, without opening anything", async () => {
    const { view, asked } = await opened([RUN_GRANT]);
    const row = rowOf(view, "steward 2");

    await read();
    await press.click(within(row).getByRole("button", { name: "Don't allow" }));

    await waitFor(() =>
      expect(asked("answer_run_grant")).toEqual([
        expect.objectContaining({ id: 9, press: "refuse" }),
      ]),
    );
  });

  it("refuses each grant pressed just after the row moved, and says why, but never holds Don't allow", async () => {
    const { view, asked } = await opened([RUN_GRANT]);
    const row = rowOf(view, "steward 2");
    await read();
    // Other scopes… opens the wider grants under the line: the row moved, and is read again.
    await press.click(within(row).getByRole("button", { name: "Other scopes…" }));

    await press.click(within(row).getByRole("button", { name: "Allow once" }));
    await press.click(within(row).getByRole("button", { name: "Allow for this chat" }));

    expect(within(row).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(asked("answer_run_grant")).toEqual([]);

    await press.click(within(row).getByRole("button", { name: "Don't allow" }));
    await waitFor(() =>
      expect(asked("answer_run_grant")).toEqual([expect.objectContaining({ press: "refuse" })]),
    );
  });

  it("puts the wider scopes under Other scopes…, which keeps the keyboard where it was", async () => {
    const { view, asked } = await opened([RUN_GRANT]);
    const row = rowOf(view, "steward 2");
    await read();

    const scopes = within(row).getByRole("button", { name: "Other scopes…" });
    expect(scopes).toHaveAttribute("aria-expanded", "false");
    act(() => scopes.focus());
    await press.keyboard("{Enter}");

    expect(scopes).toHaveAttribute("aria-expanded", "true");
    expect(scopes).toHaveFocus();
    await read();
    await press.click(within(row).getByRole("button", { name: "Allow for this chat" }));
    await waitFor(() =>
      expect(asked("answer_run_grant")).toEqual([expect.objectContaining({ press: "chat" })]),
    );
  });

  it("makes the narrowest grant a sandbox host's first way, as a pane's band does", async () => {
    const { view } = await opened([HOST]);
    const row = rowOf(view, "steward 4");

    expect(row.querySelector(".inbox-question")).toHaveTextContent("Allow reaching this host?");
    expect(buttonsOf(row)).toEqual([
      "Allow only for this chat",
      "Other scopes…",
      "Keep blocked",
      "Go to chat",
      "Snooze",
      "Stop",
    ]);
  });

  it("opens why keep it blocked only after Keep blocked, never taking the keyboard, and sends it with the second", async () => {
    const { view, asked } = await opened([HOST]);
    const row = rowOf(view, "steward 4");
    expect(within(row).queryByRole("textbox")).toBeNull();
    await read();

    const keep = within(row).getByRole("button", { name: "Keep blocked" });
    act(() => keep.focus());
    await press.keyboard("{Enter}");

    const why = within(row).getByRole("textbox", { name: /^Why you keep it blocked/ });
    expect(keep).toHaveFocus();
    expect(asked("keep_sandbox_block")).toEqual([]);
    act(() => why.focus());
    await press.keyboard("not this host");
    await read();
    await press.click(keep);

    await waitFor(() =>
      expect(asked("keep_sandbox_block")).toEqual([
        expect.objectContaining({ reason: "not this host" }),
      ]),
    );
  });

  it("answers a permission on its row: Allow once first, Allow always under Other scopes…", async () => {
    const { view, asked } = await opened([PERMISSION]);
    const row = rowOf(view, "steward 1");

    expect(row.querySelector(".inbox-question")).toHaveTextContent("Allow this action?");
    expect(buttonsOf(row)).toEqual([
      "Allow once",
      "Other scopes…",
      "Deny",
      "Go to chat",
      "Snooze",
      "Stop",
    ]);
    await read();
    await press.click(within(row).getByRole("button", { name: "Deny" }));
    await waitFor(() =>
      expect(asked("answer_ask")).toEqual([expect.objectContaining({ option: "deny" })]),
    );
  });

  it("keeps a run that needs a value behind a visible Answer button, and says what it needs", async () => {
    const { view } = await opened([RUN_NEEDS_A_VALUE]);
    const row = rowOf(view, "steward 3");

    expect(row).toHaveTextContent("Needs a value for STRIPE_API_KEY");
    const answer = within(row).getByRole("button", { name: "Answer steward 3" });
    expect(answer).not.toHaveAttribute("data-variant", "quiet-link");
    expect(within(row).queryByRole("button", { name: "Don't allow" })).toBeNull();
  });
});

/** A permission that pushes to main, which the core reads as a push to a protected branch. */
const PUSH: Shown = {
  ...PERMISSION,
  says: "Run git push origin main",
  risk: {
    class: "protected-push",
    why: "Pushes to main, a protected branch",
    confirm: "Allow a push to main?",
  },
};

describe("a high-risk ask on its row (ADR 0121 decision 8, #6011)", () => {
  it("says its risk, which is spoken with the row, and is marked for keyboard triage to pass by", async () => {
    const { view } = await opened([PUSH, { ...HOST }]);
    const row = rowOf(view, "steward 1");

    expect(row.querySelector(".inbox-risk")).toHaveTextContent(
      "High risk Pushes to main, a protected branch",
    );
    expect(row).toHaveAccessibleDescription(/Pushes to main, a protected branch/);
    expect(row).toHaveAttribute("data-risk");
    expect(rowOf(view, "steward 4")).not.toHaveAttribute("data-risk");
  });

  it("asks once more before its Allow acts, and Cancel takes the question back", async () => {
    const { view, asked } = await opened([PUSH]);
    const row = rowOf(view, "steward 1");
    await read();

    const allow = within(row).getByRole("button", { name: "Allow once" });
    await press.click(allow);

    expect(asked("answer_ask")).toEqual([]);
    expect(allow).toHaveAttribute("aria-expanded", "true");
    const again = within(row).getByRole("group", { name: "Allow a push to main?" });
    await press.click(within(again).getByRole("button", { name: "Cancel" }));
    expect(within(row).queryByRole("group", { name: "Allow a push to main?" })).toBeNull();
    expect(asked("answer_ask")).toEqual([]);

    await read();
    await press.click(allow);
    await read();
    await press.click(
      within(within(row).getByRole("group", { name: "Allow a push to main?" })).getByRole(
        "button",
        { name: "Allow once" },
      ),
    );
    await waitFor(() =>
      expect(asked("answer_ask")).toEqual([expect.objectContaining({ option: "allow" })]),
    );
  });

  it("holds the Allow it asks again with to the settle guard", async () => {
    const { view, asked } = await opened([PUSH]);
    const row = rowOf(view, "steward 1");
    await read();

    await press.click(within(row).getByRole("button", { name: "Allow once" }));
    const again = within(row).getByRole("group", { name: "Allow a push to main?" });
    await press.click(within(again).getByRole("button", { name: "Allow once" }));

    expect(within(row).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(asked("answer_ask")).toEqual([]);
  });

  it("refuses with no question asked: Deny acts on its press", async () => {
    const { view, asked } = await opened([PUSH]);
    const row = rowOf(view, "steward 1");
    await read();

    await press.click(within(row).getByRole("button", { name: "Deny" }));
    await waitFor(() =>
      expect(asked("answer_ask")).toEqual([expect.objectContaining({ option: "deny" })]),
    );
  });
});

/** When the forge was read for a merge ask, and when its re-read was (T1-40, Q248). */
const READ_AT = 1_763_000_000;
const REREAD_AT = READ_AT + 60;
/** A time as the merge row says it: "Checks PASSED as of 14:02". */
const clock = (secs: number) =>
  new Date(secs * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

/** What the forge said of request 12 when its ask was raised: no ruleset on main. */
const FACTS: MergeShown = {
  title: "Add the parser",
  author: "octo",
  base: "main",
  checks: "PASSED",
  read_at: READ_AT,
  ruleset: "none",
  look: "not-viewed",
  since_viewed: null,
  pushed_here: true,
  digest: "d-passed",
};

/** A merge chat 3's hook held (R-114), on its row (T1-40). It is high-risk (Q239): the core
 *  always sends its risk. */
const MERGE: Shown = {
  session: 3,
  ask: "merge:3:o/r#12@0123456789abcdef0123456789abcdef01234567",
  says: "Merge request 12 of o/r at 0123456789ab? There is no undo.",
  options: [
    { id: "merge", label: "Merge", allows: true },
    { id: "dont-merge", label: "Don't merge", allows: false },
  ],
  source: "merge-hold",
  chain: ["steward 3"],
  answer: {
    via: "merge-hold",
    repo: "o/r",
    number: 12,
    head: "0123456789abcdef0123456789abcdef01234567",
    facts: FACTS,
  },
  risk: {
    class: "merge",
    why: "Merges request 12 of o/r at 0123456 into main, with no undo",
    confirm: "Merge request 12 of o/r at 0123456 into main?",
  },
};

describe("a held merge in Asks (R-115)", () => {
  /** Presses Merge, then the Merge it asks once more with (Q239), each once read. */
  const mergeTwice = async () => {
    await press.click(await screen.findByRole("button", { name: "Merge" }));
    const again = await screen.findByRole("group", {
      name: "Merge request 12 of o/r at 0123456 into main?",
    });
    await read();
    await press.click(within(again).getByRole("button", { name: "Merge" }));
  };

  it("is high-risk: it says so, and keyboard triage passes it by", async () => {
    const { view } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");

    expect(row.querySelector(".inbox-risk")).toHaveTextContent(
      "High risk Merges request 12 of o/r at 0123456 into main, with no undo",
    );
    expect(row).toHaveAttribute("data-risk");
  });

  it("the merge ask carries the Merge risk class", async () => {
    // R-139 (T1-39): Merge is a risk class of its own, and the hold's row is drawn as it, not as
    // a high-risk ask of no class.
    const { view } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");

    expect(row).toHaveAttribute("data-risk", "merge");
    expect(row.querySelector(".inbox-risk")).toHaveTextContent(
      "Merges request 12 of o/r at 0123456 into main, with no undo",
    );
  });

  it("asks once more before Merge acts, and Cancel takes the question back", async () => {
    const { view, asked } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");
    await answersOf(row);
    const merge = await screen.findByRole("button", { name: "Merge" });
    await read();

    await press.click(merge);

    expect(asked("merge_held_request")).toEqual([]);
    const again = await screen.findByRole("group", {
      name: "Merge request 12 of o/r at 0123456 into main?",
    });
    await press.click(within(again).getByRole("button", { name: "Cancel" }));
    expect(
      screen.queryByRole("group", { name: "Merge request 12 of o/r at 0123456 into main?" }),
    ).toBeNull();
    expect(asked("merge_held_request")).toEqual([]);
  });

  /** Opens the merge ask's answers, behind its Answer, once its row has been read. */
  const answersOf = async (row: HTMLElement) => {
    await read();
    await press.click(within(row).getByRole("button", { name: /^Answer/ }));
  };

  it("holds a Merge pressed just after its answers were drawn to the settle guard", async () => {
    const { view, asked } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");
    await answersOf(row);

    await press.click(await screen.findByRole("button", { name: "Merge" }));

    expect(asked("merge_held_request")).toEqual([]);
  });

  it("asks the app to merge exactly the request and head the ask was raised at", async () => {
    const { view, asked } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");
    await answersOf(row);
    await screen.findByRole("button", { name: "Merge" });
    await read();

    await mergeTwice();

    await waitFor(() =>
      expect(asked("merge_held_request")).toEqual([
        {
          plane: PLANE,
          session: 3,
          repo: "o/r",
          number: 12,
          head: "0123456789abcdef0123456789abcdef01234567",
          ruleset: "none",
          digest: "d-passed",
        },
      ]),
    );
  });

  /** The chat tried again after a push: the forge says the request is at another head. */
  const MOVED_HEAD = "89abcdef0123456789abcdef0123456789abcdef";
  const movedTo = (ask: string): Shown => ({
    ...MERGE,
    ask,
    answer: { via: "merge-hold", repo: "o/r", number: 12, head: MOVED_HEAD, facts: FACTS },
    risk: {
      class: "merge",
      why: "Merges request 12 of o/r at 89abcde into main, with no undo",
      confirm: "Merge request 12 of o/r at 89abcde into main?",
    },
  });

  /** Opens the merge ask's answers and presses Merge, so it asks once more. */
  const confirming = async (view: HTMLElement) => {
    await answersOf(rowOf(view, "steward 3"));
    const merge = await screen.findByRole("button", { name: "Merge" });
    await read();
    await press.click(merge);
    await screen.findByRole("group", { name: "Merge request 12 of o/r at 0123456 into main?" });
  };

  it("carries no confirmation over to the ask at a moved head, which names its own head (Q242)", async () => {
    const { view, asked, moves } = await opened([MERGE]);
    await confirming(view);

    await moves([movedTo(`merge:3:o/r#12@${MOVED_HEAD}`)]);

    await waitFor(() =>
      expect(
        screen.queryByRole("group", { name: "Merge request 12 of o/r at 0123456 into main?" }),
      ).toBeNull(),
    );
    expect(
      screen.queryByRole("group", { name: "Merge request 12 of o/r at 89abcde into main?" }),
    ).toBeNull();
    expect(rowOf(view, "steward 3").querySelector(".inbox-risk")).toHaveTextContent(
      "Merges request 12 of o/r at 89abcde into main, with no undo",
    );
    expect(asked("merge_held_request")).toEqual([]);
  });

  it("drops a confirmation whose question moved under it, even where its id did not", async () => {
    // The window's own guard: a confirmation holds only for the ask and the question it was
    // asked of, whatever the core keys the row on.
    const { view, asked, moves } = await opened([MERGE]);
    await confirming(view);

    await moves([movedTo(MERGE.ask)]);

    await waitFor(() =>
      expect(
        screen.queryByRole("group", { name: "Merge request 12 of o/r at 0123456 into main?" }),
      ).toBeNull(),
    );
    expect(
      screen.queryByRole("group", { name: "Merge request 12 of o/r at 89abcde into main?" }),
    ).toBeNull();
    expect(asked("merge_held_request")).toEqual([]);
  });

  /** The merge ask at the head it was raised at, showing `facts` over the first read's. */
  const showing = (facts: Partial<MergeShown>): Shown => ({
    ...MERGE,
    answer: {
      via: "merge-hold",
      repo: "o/r",
      number: 12,
      head: "0123456789abcdef0123456789abcdef01234567",
      facts: { ...FACTS, ...facts },
    },
  });

  it("No ruleset here is void once the observed state changes", async () => {
    // R-150: the person was shown no ruleset on main and pressed Merge; then purlis read one.
    // The confirmation is taken back, the row says what it read now, and nothing is merged.
    const { view, asked, moves } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");
    expect(row).toHaveTextContent(
      "No ruleset on main. That's fine: the merge still waits for you.",
    );
    await confirming(view);

    await moves([showing({ ruleset: "protects", read_at: REREAD_AT + 60 })]);

    await waitFor(() =>
      expect(
        screen.queryByRole("group", { name: "Merge request 12 of o/r at 0123456 into main?" }),
      ).toBeNull(),
    );
    expect(rowOf(view, "steward 3")).toHaveTextContent("A ruleset protects main.");
    expect(rowOf(view, "steward 3")).not.toHaveTextContent("No ruleset on main");
    expect(asked("merge_held_request")).toEqual([]);
  });

  it("re-reads the request before it asks once more, and shows the checks as of that read", async () => {
    // Q248: the row says how old its checks are, and the press that asks once more reads the
    // forge again first, so the question is asked of what the forge says now.
    const { view, asked } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");
    expect(row).toHaveTextContent(`Checks PASSED as of ${clock(READ_AT)}`);

    await confirming(view);

    expect(asked("reread_held_merge")).toEqual([
      {
        plane: PLANE,
        session: 3,
        repo: "o/r",
        number: 12,
        head: "0123456789abcdef0123456789abcdef01234567",
      },
    ]);
    await waitFor(() =>
      expect(rowOf(view, "steward 3")).toHaveTextContent(`Checks PASSED as of ${clock(REREAD_AT)}`),
    );
    expect(asked("merge_held_request")).toEqual([]);
  });

  it("asks once more with purlis's own sentence when the hold arrived without its risk (Q268, Q269)", async () => {
    // Q269: a merge hold with no risk class still takes two presses on its row: the first
    // reads the request again and asks once more, and never merges.
    const { view, asked } = await opened([{ ...MERGE, risk: null }]);
    await answersOf(rowOf(view, "steward 3"));
    const merge = await screen.findByRole("button", { name: "Merge" });
    await read();

    await press.click(merge);

    await waitFor(() => expect(asked("reread_held_merge")).toHaveLength(1));
    await screen.findByRole("group", {
      name: "Merge request 12 of o/r at 0123456? There is no undo.",
    });
    expect(asked("merge_held_request")).toEqual([]);
  });

  it("asks once more of a ruleset the re-read found, and sends the state it showed", async () => {
    // R-150: the re-read found a ruleset since the row was drawn: the question is asked of it,
    // and the press says it was shown one.
    const { view, asked, rereadFinds } = await opened([MERGE]);
    rereadFinds({ ruleset: "protects" });
    await answersOf(rowOf(view, "steward 3"));
    await screen.findByRole("button", { name: "Merge" });
    await read();

    await mergeTwice();

    expect(rowOf(view, "steward 3")).toHaveTextContent("A ruleset protects main.");
    await waitFor(() =>
      expect(asked("merge_held_request")).toEqual([
        expect.objectContaining({ number: 12, ruleset: "protects" }),
      ]),
    );
  });

  it("shows the request's title as text the chat may have written, never as markup", async () => {
    // Q248: the title is the request's author's, which may be the chat: drawn inert and said so.
    const { view } = await opened([
      showing({ title: "**Ship it** <img src=x onerror=alert(1)> [docs](https://evil.example)" }),
    ]);
    const row = rowOf(view, "steward 3");

    const title = row.querySelector(".merge-title");
    expect(title).toHaveTextContent(
      "**Ship it** <img src=x onerror=alert(1)> [docs](https://evil.example)",
    );
    expect(row).toHaveTextContent("Title, as the chat may have written it:");
    expect(row.querySelector("img, a[href], strong")).toBeNull();
    expect(row).toHaveTextContent("Into main, opened by octo");
  });

  it("states beside the ruleset that a script merging with the chat's token gets past purlis", async () => {
    // R-151 (phase 1 wording), with the ruleset state it stands beside (R-147).
    const { view } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");
    expect(row.querySelector(".merge-protection")).toHaveTextContent(
      "No ruleset on main. That's fine: the merge still waits for you. A script that merges with the chat's token gets past purlis.",
    );
  });

  it("holds a Don't merge pressed just after its answers were drawn to the settle guard", async () => {
    // ADR 0097: on a merge row, Don't merge waits for the row to settle as Merge does.
    const { view, asked } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");
    await answersOf(row);

    await press.click(await screen.findByRole("button", { name: "Don't merge" }));

    expect(within(row).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(asked("dont_merge_held_request")).toEqual([]);

    await read();
    await press.click(screen.getByRole("button", { name: "Don't merge" }));
    await waitFor(() => expect(asked("dont_merge_held_request")).toHaveLength(1));
  });

  it("sends Don't merge, which merges nothing", async () => {
    const { view, asked } = await opened([MERGE]);
    const row = rowOf(view, "steward 3");
    await answersOf(row);
    const dont = await screen.findByRole("button", { name: "Don't merge" });
    await read();

    await press.click(dont);

    await waitFor(() =>
      expect(asked("dont_merge_held_request")).toEqual([
        expect.objectContaining({ session: 3, repo: "o/r", number: 12 }),
      ]),
    );
    expect(asked("merge_held_request")).toEqual([]);
  });
});

/** A GitHub App token chat 2 couldn't be minted for an unrecorded clone (R-330): Asks alone
 *  draws it until the finish row and Blocking (T1-42). */
const TRUST_CLONE: Shown = {
  session: 2,
  ask: "trust-clone:2:workspaces/alpha/api|github.com/acme/api",
  says: "Trust this clone so chats can use GitHub: workspaces/alpha/api was not cloned by purlis, and its remote is github.com/acme/api. Trust it only if you made this clone yourself.",
  options: [{ id: "press", label: "Trust this clone", allows: true }],
  source: "trust-clone",
  chain: ["steward 2"],
  answer: { via: "trust-clone", folder: "workspaces/alpha/api", place: "github.com/acme/api" },
};

describe("a refusal whose only way out is the window's press (R-328, R-330, R-335)", () => {
  /** Opens the ask's answers, behind its Answer where it has one, once its row has been read. */
  const controlOf = async (row: HTMLElement) => {
    await read();
    const answer = within(row).queryByRole("button", { name: /^Answer/ });
    if (answer !== null) await press.click(answer);
    return screen.findByRole("button", { name: "Trust this clone" });
  };

  it("a_window_only_refusal_raises_one_ask_carrying_its_control: pressing it trusts the clone at the address it was raised at", async () => {
    const { view, asked } = await opened([TRUST_CLONE]);
    const rows = askRows().filter(
      (one) => view.contains(one) && one.classList.contains("inbox-ask"),
    );
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveTextContent("Trust this clone so chats can use GitHub");
    const trust = await controlOf(rowOf(view, "steward 2"));
    await read();

    await press.click(trust);

    await waitFor(() =>
      expect(asked("vouch_clone")).toEqual([
        { plane: PLANE, folder: "workspaces/alpha/api", place: "github.com/acme/api" },
      ]),
    );
  });

  it("a_dismissed_notice_that_later_stops_a_chat_is_raised_again_as_an_ask", async () => {
    // The person dismissed the clone's Notice; then the clone stopped a chat's mint.
    dismissNotice(
      PLANE,
      cloneVouchCause({
        workspace: "alpha",
        folder: "workspaces/alpha/api",
        place: "github.com/acme/api",
        places: ["github.com/acme/api"],
      }),
    );
    const { view } = await opened([TRUST_CLONE]);

    const rows = askRows().filter(
      (one) => view.contains(one) && one.classList.contains("inbox-ask"),
    );
    expect(rows).toHaveLength(1);
    expect(await controlOf(rowOf(view, "steward 2"))).toBeInTheDocument();
  });
});
