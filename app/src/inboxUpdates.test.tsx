// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type {
  AskEnded,
  AwayRefusal,
  ChatBlocked,
  FinishedTask,
  InboxUpdate,
  OpenChat,
  UpdateNoted,
} from "./bindings";
import { needSays } from "./chatState";
import { refusedWord } from "./copy";
import {
  KEPT_FOR_MS,
  KIND_SAID,
  LIVE,
  arrivedUpdates,
  awayUpdates,
  browserAskUpdates,
  fromOf,
  listedFrom,
  listed,
  reasonUpdates,
  reportSentUpdates,
  resumeUpdates,
  sandboxUpdates,
  smartCloseUpdates,
  stuckUpdates,
  taskUpdates,
  unreadMine,
  useInboxUpdates,
} from "./inboxUpdates";

/**
 * **The Inbox's updates** (#1693): what each source says becomes one update, kept a day by the
 * core, and what Mark all read and Dismiss all do. The sources are plain inputs; the core is a
 * mock that keeps what it is told, as the real store does.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const NOW = Date.parse("2026-10-11T12:00:00Z");
const PLANE = "/home/dev/plane";
const nameOf = (session: number) => `steward ${session}`;

const finished = (id: string, how: string, ended: string | null): FinishedTask =>
  ({
    id,
    asker: 3,
    name: `task ${id}`,
    chat: 9,
    persona: null,
    how,
    outcome: how,
    folds: false,
    report: "",
    changed: null,
    ended,
    place: "",
    branch: null,
    reopens: false,
    not_reopened: null,
    did_not_start: false,
    attempts: 1,
    waits: null,
  }) as unknown as FinishedTask;

describe("each source, said as updates", () => {
  it("makes a finished task one update, in the count its row is in, at the time it ended", () => {
    const ended = new Date(NOW - 60_000).toISOString();
    const said = taskUpdates(
      [finished("a", "done", ended), finished("b", "failed", ended), finished("c", "done", null)],
      nameOf,
      NOW,
    );
    expect(said.map((one) => [one.key, one.kind, one.session, one.chain])).toEqual([
      ["task:a", "task-done", 3, ["steward 3"]],
      ["task:b", "task-failed", 3, ["steward 3"]],
    ]);
    expect(said[0].at).toBe(Math.floor((NOW - 60_000) / 1000));
    expect(said[0].says).toMatch(/^task a: /);
  });

  it("leaves out a task that ended a day ago or more", () => {
    const old = new Date(NOW - KEPT_FOR_MS).toISOString();
    expect(taskUpdates([finished("a", "done", old)], nameOf, NOW)).toEqual([]);
  });

  it("makes a chat that came back an update, resumed or fresh, and a chat that simply started none", () => {
    const chat = (session: number, resumed: string | null, fresh: string | null) =>
      ({ session, name: `chat ${session}`, resumed, fresh }) as unknown as OpenChat;
    const said = resumeUpdates(
      [chat(1, "conv-1", null), chat(2, null, "its conversation was gone"), chat(3, null, null)],
      NOW,
    );
    expect(said.map((one) => [one.session, one.says])).toEqual([
      [1, "Resumed its conversation"],
      [2, "Came back as a new chat: its conversation was gone"],
    ]);
  });

  it("makes an announced chat the person has not looked at an update, keyed by its id (#4759)", () => {
    const chat = (
      session: number,
      unseen: boolean | null,
      from: { name: string; task: boolean } | null,
      cant_report_asks: boolean | null = null,
    ) => ({ session, id: `id-${session}`, unseen, from, cant_report_asks }) as unknown as OpenChat;
    const said = arrivedUpdates(
      [
        chat(1, true, { name: "steward 3", task: false }),
        chat(2, true, { name: "steward 3", task: true }, true),
        chat(3, null, { name: "steward 3", task: false }),
        chat(4, true, null),
      ],
      (session) => `chat ${session}`,
      NOW,
    );
    expect(said.map((one) => [one.key, one.kind, one.session, one.says])).toEqual([
      ["arrived:id-1", "arrived", 1, "chat 1 arrived from steward 3"],
      [
        "arrived:id-2",
        "arrived",
        2,
        "chat 2 arrived from steward 3. It can't report asks, so look at it from time to time.",
      ],
    ]);
    expect(KIND_SAID.arrived).toBe("Arrived");
  });

  it("makes a block that is no host ask a sandbox update, and leaves a host ask to the asks", () => {
    const block = (offer: string, target: string | null): ChatBlocked =>
      ({
        plane: PLANE,
        session: 4,
        operation: offer === "host" ? "connect" : "write",
        kind: offer === "host" ? "host" : "folder",
        ours: false,
        harness: null,
        said: `blocked ${offer}`,
        offer,
        target,
        route: null,
        levels: [],
      }) as unknown as ChatBlocked;
    const said = sandboxUpdates(
      { 4: [block("host", "api.example.com"), block("write", "/srv")] },
      nameOf,
      NOW,
    );
    expect(said.map((one) => [one.kind, one.says, one.chain])).toEqual([
      ["sandbox", "blocked write", ["steward 4"]],
    ]);
  });

  it("makes a dispatch refused while nobody was there an update at its last refusal", () => {
    const refused = {
      asking: "steward",
      target: "devops",
      workspace: null,
      latest: 1_800_000_000,
      times: 2,
      allows: "",
      nowhere: null,
      shown: "d",
    } satisfies AwayRefusal;
    expect(awayUpdates([refused])).toEqual([
      {
        key: "away:steward#devops#",
        kind: "refused-away",
        at: 1_800_000_000,
        session: null,
        chain: [],
        says: "steward wanted devops while you were away",
      },
    ]);
  });

  it("makes a report with nowhere to go and a refused commit updates about their chat (#1694)", () => {
    // What the app found a chat needs the person for is information, not a decision the chat
    // waits on (I-1): each is an update, drawn while its chat still has it.
    const said = reasonUpdates(
      { 4: ["its report has nowhere to go because steward 2 has closed"] },
      { 6: ["git commit -m wip: a secret-shaped line"] },
      nameOf,
      NOW,
    );
    expect(said.map((one) => [one.kind, one.session, one.chain, one.says])).toEqual([
      [
        "report-undelivered",
        4,
        ["steward 4"],
        "its report has nowhere to go because steward 2 has closed",
      ],
      ["commit-refused", 6, ["steward 6"], "git commit -m wip: a secret-shaped line"],
    ]);
    // Said again, the same updates: a key of their own each.
    expect(
      reasonUpdates(
        { 4: ["its report has nowhere to go because steward 2 has closed"] },
        { 6: ["git commit -m wip: a secret-shaped line"] },
        nameOf,
        NOW + 60_000,
      ).map((one) => one.key),
    ).toEqual(said.map((one) => one.key));
  });

  it("makes a chat refused an update of the project an update of its own kind (qw348)", () => {
    const said = reasonUpdates({ 4: [needSays({ kind: "root_update_refused" })] }, {}, nameOf, NOW);
    expect(said.map((one) => [one.kind, one.session])).toEqual([["root-update-refused", 4]]);
    expect(KIND_SAID[said[0].kind]).toBe("Asks you to update the project");
    expect(said[0].says).toContain("Update, in Saving");
    expect(LIVE.has(said[0].kind)).toBe(true);
  });

  it("makes an Ask that ended with its browser an update about its chat, standing its day (#2015)", () => {
    const ended: AskEnded = {
      plane: "/home/dev/plane",
      session: 4,
      browser: "b1",
      at: 1_800_000_100,
      says: "Its browser b1 ended, so what it asked you to do there ended too",
    };
    const said = browserAskUpdates([ended], nameOf);
    expect(said).toEqual([
      {
        key: "browser-ask:4:b1:1800000100",
        kind: "browser-ask-ended",
        at: 1_800_000_100,
        session: 4,
        chain: [nameOf(4)],
        says: "Its browser b1 ended, so what it asked you to do there ended too",
      },
    ]);
    expect(KIND_SAID["browser-ask-ended"]).toBe("Browser ended");
    expect(LIVE.has("browser-ask-ended")).toBe(false);
  });

  it("makes a Smart close that stopped an update about its chat", () => {
    const said = smartCloseUpdates({ 6: "Smart close stopped — it ended" }, nameOf, NOW);
    expect(said.map((one) => [one.kind, one.session, one.says])).toEqual([
      ["smart-close", 6, "Smart close stopped — it ended"],
    ]);
  });

  it("makes each report draft the person filed an update on the chat it came from (#5694)", () => {
    const said = reportSentUpdates([
      { id: "d1", title: "Update refuses", session: 4, chat: "steward 4", to: "github", at: NOW },
      { id: "d2", title: "Picker is slow", session: null, chat: "deep", to: "intake", at: NOW },
    ]);
    expect(said.map((one) => [one.key, one.kind, one.session, one.chain, one.says])).toEqual([
      [
        "report-sent:d1",
        "report-sent",
        4,
        ["steward 4"],
        "Sent a report on GitHub: Update refuses",
      ],
      ["report-sent:d2", "report-sent", null, ["deep"], "Sent a report: Picker is slow"],
    ]);
    expect(KIND_SAID["report-sent"]).toBe("Report sent");
    expect(LIVE.has("report-sent")).toBe(false);
  });

  it("says a filed report the intake did not answer for waits to send (#5694, #5693)", () => {
    const said = reportSentUpdates([
      { id: "d3", title: "Update refuses", session: 4, chat: "steward 4", to: "waiting", at: NOW },
    ]);
    expect(said.map((one) => one.says)).toEqual(["Waiting to send a report: Update refuses"]);
    expect(fromOf("report-sent")).toBe("mine");
  });
});

describe("the list", () => {
  const kept = (key: string, at: number, kind: InboxUpdate["kind"] = "task-done"): InboxUpdate => ({
    key,
    kind,
    at,
    session: null,
    chain: [],
    says: key,
    read: false,
  });

  it("is newest first, and drops an answered-at-its-source update its source no longer lists", () => {
    const list = listed(
      [
        kept("old", 1),
        kept("away:gone", 5, "refused-away"),
        kept("away:here", 3, "refused-away"),
        kept("new", 4),
      ],
      [{ ...kept("away:here", 3, "refused-away") }],
    );
    expect(list.map((one) => one.key)).toEqual(["new", "away:here", "old"]);
  });

  it("lists no doctor finding the core kept from before, as the Notices view says them (#5645)", () => {
    const list = listed([kept("doctor:git:fail", 5, "doctor"), kept("new", 4)], []);
    expect(list.map((one) => one.key)).toEqual(["new"]);
  });

  it("lists an update its source still lists though the core keeps it no longer", () => {
    // A dispatch refused two days ago is past the day the core keeps, and still answered here.
    const list = listed([kept("new", 4)], [kept("away:old", 1, "refused-away")]);
    expect(list.map((one) => [one.key, one.read])).toEqual([
      ["new", false],
      ["away:old", false],
    ]);
  });
});

/**
 * **Mine or Project** (#5646, ADR 0119): each kind of update is the person's own chat's or
 * task's, under Mine (From your chats), or the project's, under Project (From the project). The
 * bell counts the unread Mine lines alone.
 */
describe("Mine or Project (#5646)", () => {
  it("puts what the person's own chats and tasks did under Mine", () => {
    const mine = [
      "task-done",
      "task-failed",
      "resumed",
      "arrived",
      "report-undelivered",
      "commit-refused",
      "root-update-refused",
      "smart-close",
      "refused-away",
      "browser-ask-ended",
      "sandbox",
      "question-closed",
      "question-failed",
    ] as const;
    expect(mine.map(fromOf)).toEqual(mine.map(() => "mine"));
  });

  it("puts the doctor's findings, which are about the project and not a chat's, under Project", () => {
    expect(fromOf("doctor")).toBe("project");
  });

  it("sorts every kind there is, and each under one list", () => {
    for (const kind of Object.keys(KIND_SAID) as (keyof typeof KIND_SAID)[])
      expect(["mine", "project"]).toContain(fromOf(kind));
  });

  it("counts the unread Mine lines, and never a read one or a Project one", () => {
    const one = (key: string, kind: InboxUpdate["kind"], read: boolean): InboxUpdate => ({
      key,
      kind,
      at: 1,
      session: null,
      chain: [],
      says: key,
      read,
    });
    expect(
      unreadMine([
        one("a", "task-done", false),
        one("b", "task-failed", false),
        one("c", "task-done", true),
        one("d", "doctor", false),
      ]),
    ).toBe(2);
    expect(unreadMine(undefined)).toBe(0);
  });

  it("lists a line the first-open baseline set aside under Project, whatever its kind (#5648)", () => {
    const done = { kind: "task-done" as const };
    expect(listedFrom(done)).toBe("mine");
    expect(listedFrom({ ...done, baselined: true })).toBe("project");
    expect(
      unreadMine([
        {
          key: "a",
          ...done,
          at: 1,
          session: 3,
          chain: [],
          says: "a",
          read: false,
          baselined: true,
        },
      ]),
    ).toBe(0);
  });
});

/**
 * A core that keeps updates as `note_inbox_updates` and `settle_inbox_updates` do, and counts
 * what it is asked.
 */
function store(kept: InboxUpdate[] = []) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const shown = () =>
    kept.filter((one) => !dismissed.has(one.key)).sort((one, other) => other.at - one.at);
  const dismissed = new Set<string>();
  mockIPC((cmd, args) => {
    const given = args as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "inbox_updates") return shown();
    if (cmd === "note_inbox_updates") {
      for (const one of given.noted as UpdateNoted[])
        if (!kept.some((was) => was.key === one.key)) kept.push({ ...one, read: false });
      return shown();
    }
    if (cmd === "settle_inbox_updates") {
      const keys = given.keys as string[] | null;
      for (const one of kept) {
        if (keys !== null && !keys.includes(one.key)) continue;
        one.read = true;
        if (given.how === "dismissed") dismissed.add(one.key);
      }
      return shown();
    }
    return null;
  });
  return asked;
}

const noted = (key: string, at: number): UpdateNoted => ({
  key,
  kind: "task-done",
  at,
  session: 3,
  chain: ["steward 3"],
  says: key,
});

describe("noting, keeping and settling (#1693)", () => {
  it("notes what its sources say once, and lists what the core keeps, newest first", async () => {
    const asked = store([{ ...noted("kept:before", 1), read: true }]);
    const derived = [noted("task:a", 10), noted("task:b", 20)];
    const { result, rerender } = renderHook(({ now }) => useInboxUpdates(PLANE, now), {
      initialProps: { now: derived },
    });
    await waitFor(() =>
      expect(result.current.updates?.map((one) => one.key)).toEqual([
        "task:b",
        "task:a",
        "kept:before",
      ]),
    );
    // The same sources said again write nothing more.
    rerender({ now: [...derived] });
    expect(asked.filter((one) => one.cmd === "note_inbox_updates")).toHaveLength(1);
  });

  it("marks every update read, and dismisses every one, on the core", async () => {
    const asked = store();
    const { result } = renderHook(() => useInboxUpdates(PLANE, DERIVED));
    await waitFor(() => expect(result.current.updates).toHaveLength(1));
    act(() => result.current.settle(null, "read"));
    await waitFor(() => expect(result.current.updates?.[0].read).toBe(true));
    act(() => result.current.settle(null, "dismissed"));
    await waitFor(() => expect(result.current.updates).toEqual([]));
    expect(
      asked
        .filter((one) => one.cmd === "settle_inbox_updates")
        .map((one) => [one.args.keys, one.args.how]),
    ).toEqual([
      [null, "read"],
      [null, "dismissed"],
    ]);
  });

  it("keeps the list in this window, and says why, where the machine keeps none", async () => {
    mockIPC((cmd) => {
      if (cmd.endsWith("inbox_updates")) throw "purlis has nowhere to keep the Inbox's updates";
      return null;
    });
    const { result } = renderHook(() => useInboxUpdates(PLANE, DERIVED));
    await waitFor(() => expect(result.current.trouble).toMatch(/nowhere to keep/));
    expect(result.current.updates?.map((one) => one.key)).toEqual(["task:a"]);
    act(() => result.current.settle(["task:a"], "dismissed"));
    await waitFor(() => expect(result.current.updates).toEqual([]));
  });
});

const DERIVED = [noted("task:a", 10)];

describe("a chat that reads Stuck? (#6014, ADR 0121 decision 19)", () => {
  it("is a Should-know update From your chats, drawn while it reads so", () => {
    const said = stuckUpdates(
      {
        3: { stuck: "no new command or output for 15 minutes", leftWork: null, mode: null },
        4: { stuck: null, leftWork: true, mode: null },
      },
      nameOf,
      NOW,
    );
    expect(said).toEqual([
      {
        key: "stuck:3:no new command or output for 15 minutes",
        kind: "stuck",
        at: Math.floor(NOW / 1000),
        session: 3,
        chain: ["steward 3"],
        says: "Stuck? no new command or output for 15 minutes",
      },
    ]);
    expect(fromOf("stuck")).toBe("mine");
    expect(LIVE.has("stuck")).toBe(true);
    expect(KIND_SAID.stuck).toBe("Stuck?");
  });

  it("says nothing of a chat that is not stuck, or of none noticed", () => {
    expect(stuckUpdates({}, nameOf, NOW)).toEqual([]);
    expect(stuckUpdates(undefined, nameOf, NOW)).toEqual([]);
  });
});

describe("what a row says of its kind (R-170, Q259)", () => {
  it("says Blocked for a stop, never Refused, so a commit a guard stopped reads Commit blocked", () => {
    expect(KIND_SAID["commit-refused"]).toBe("Commit blocked");
    for (const said of Object.values(KIND_SAID)) expect(refusedWord(said, "shown")).toEqual([]);
  });
});
