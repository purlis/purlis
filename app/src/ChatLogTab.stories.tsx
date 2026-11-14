// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's Chat log** (R-224, R-225, R-227, R-233, R-310; T1-27): its own simple timeline,
 * apart from the transcript, with a one-line start row built from the start event and the end
 * and its result. Pull-only: no count, no New marker. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent } from "storybook/test";
import { expectFocusInSight } from "./stories.testkit";
import type { ChatLogRow } from "./bindings";
import { TOLD } from "./blockWays";
import { ChatLogRows } from "./ChatLogTab";

const START: ChatLogRow = {
  kind: "started",
  at: "2026-11-13T09:00:00Z",
  cause: "start",
  protection: {
    layers: [{ layer: "sandbox", level: "floor", tier: "project" }],
    charter_commit: "4f2a9c1e0b",
    charter_dirty: false,
    answers_in_force: 0,
  },
};

const DONE: ChatLogRow = {
  kind: "ended",
  at: "2026-11-13T10:30:00Z",
  state: "completed",
  cause: "exited",
  code: 0,
};

const FAILED: ChatLogRow = {
  kind: "ended",
  at: "2026-11-13T09:40:00Z",
  state: "failed",
  cause: "exited",
  code: 3,
};

const meta = {
  component: ChatLogRows,
  args: { name: "steward 3", rows: [START, DONE] },
} satisfies Meta<typeof ChatLogRows>;

export default meta;
type Story = StoryObj<typeof meta>;

type Canvas = Parameters<NonNullable<Story["play"]>>[0]["canvas"];

/** The timeline's rows, top to bottom, and each row's line. */
function rowsOf(canvas: Canvas): HTMLElement[] {
  const list = canvas.getByRole("list", { name: "Chat log of steward 3" });
  return Array.from(list.querySelectorAll<HTMLElement>(":scope > li"));
}

const said = (row: HTMLElement) => row.querySelector<HTMLElement>(".chat-log-said");

/**
 * **What every Chat log shows**: its own timeline, which is no transcript (no terminal, no
 * chat message in it); the start first, as one line of the start event's four fields; and no
 * count, New marker or live region that would announce a row (R-225, R-310, R-227).
 */
async function itsOwnTimeline(canvas: Canvas, end: RegExp) {
  const rows = rowsOf(canvas);
  await expect(rows).toHaveLength(2);
  const start = said(rows[0]);
  await expect(start?.textContent).toBe(
    "Started · Basic sandbox, set by the project · purlis.toml at 4f2a9c1 · no answers in force",
  );
  // One line: its words are drawn on one line box, never wrapped.
  await expect(start?.getClientRects()).toHaveLength(1);
  await expect(said(rows[rows.length - 1])?.textContent).toMatch(end);
  await expect(canvas.queryByRole("log")).toBeNull();
  await expect(canvas.queryByRole("status")).toBeNull();
  await expect(canvas.queryByRole("textbox")).toBeNull();
  await expect(canvas.queryByText(/\bnew\b/i)).toBeNull();
}

/** **A chat that is done** (R-233): its last row is its end, done. */
export const Done: Story = {
  play: async ({ canvas }) => itsOwnTimeline(canvas, /^Ended · done$/),
};

/** **A chat that failed** (R-233): its last row is its end, failed, with its exit code. */
export const Failed: Story = {
  args: { rows: [START, FAILED] },
  play: async ({ canvas }) => itsOwnTimeline(canvas, /^Ended · failed, exit code 3$/),
};

/** **A start that recorded no protection** (T1-06's lenient shapes): one line that says so. */
export const StartNotRecorded: Story = {
  args: { rows: [{ ...START, protection: null }] },
  play: async ({ canvas }) => {
    const [start] = rowsOf(canvas);
    await expect(said(start)?.textContent).toBe("Started · what protected it was not recorded");
  },
};

/**
 * **A run the project's settings replaced** (ADR 0076 §2, Q247): it ended superseded, and its
 * end row says it was started again, never that it was closed. The start after it is the run
 * that took its place.
 */
export const RestartedForProjectSettings: Story = {
  args: {
    rows: [
      START,
      {
        kind: "ended",
        at: "2026-11-13T09:20:00Z",
        state: "completed",
        cause: "superseded",
        code: null,
      },
      { ...START, at: "2026-11-13T09:20:01Z", cause: "reopen" },
    ],
  },
  play: async ({ canvas }) => {
    const [, end, again] = rowsOf(canvas);
    await expect(said(end)?.textContent).toBe(
      "Ended · started again because the project's settings changed",
    );
    await expect(said(again)?.textContent).toMatch(/^Started again in its conversation · /);
  },
};

const at = (minute: number) => `2026-11-13T09:${String(minute).padStart(2, "0")}:00Z`;

/** The line of each row of kind `kind`, top to bottom. */
function linesOf(canvas: Canvas, kind: string): (string | null | undefined)[] {
  return rowsOf(canvas)
    .filter((row) => row.dataset.kind === kind)
    .map((row) => said(row)?.textContent);
}

/**
 * **A protection change and who made it** (R-228, T1-28): in phase 1, a change through a
 * shipped path, the sandbox lifted for a run by the person and put back on by purlis. The
 * change sheet's changes are phase 2's.
 */
export const ProtectionChanged: Story = {
  args: {
    rows: [
      START,
      {
        kind: "protection_changed",
        at: at(5),
        change: "off",
        actor_kind: "human",
        actor: "operator",
      },
      { kind: "protection_changed", at: at(9), change: "on", actor_kind: "host", actor: "charter" },
      {
        kind: "protection_changed",
        at: at(12),
        change: "off",
        actor_kind: "agent",
        actor: "a chat",
      },
      DONE,
    ],
  },
  play: async ({ canvas }) => {
    await expect(linesOf(canvas, "protection_changed")).toEqual([
      "No sandbox for this run · set by you",
      "The sandbox is back on · set by purlis",
      "No sandbox for this run · asked for by a chat",
    ]);
  },
};

/**
 * **Asks, each with its answer and who answered** (R-229, T1-28): one row per ask, at the time
 * it was raised. An ask with no answer yet says so.
 */
export const Asks: Story = {
  args: {
    rows: [
      START,
      { kind: "ask_raised", at: at(1), ask: "harness_prompt" },
      { kind: "ask_answered", at: at(2), ask: "harness_prompt", answer: "allowed", by: "window" },
      { kind: "ask_raised", at: at(3), ask: "budget" },
      { kind: "ask_answered", at: at(4), ask: "budget", answer: "refused", by: "inbox" },
      { kind: "ask_raised", at: at(5), ask: "hold" },
    ],
  },
  play: async ({ canvas }) => {
    await expect(linesOf(canvas, "ask")).toEqual([
      "Asked you about its harness's permission prompt · Allowed, by you in the window",
      "Asked you whether to go past a budget · Stopped, by you from purlis inbox",
      "Asked you about an action purlis held · Not answered",
    ]);
  },
};

/**
 * **A hold** (R-231, T1-28): a merge purlis held, which only the person's Merge merges. Grants
 * are listed beside holds in phase 3 (R-133).
 */
export const Held: Story = {
  args: {
    rows: [START, { kind: "held", at: at(20), tool: "Bash", arm: "merge-hold", sub_agent: false }],
  },
  play: async ({ canvas }) => {
    await expect(linesOf(canvas, "held")).toEqual([
      "Held a merge · purlis merges it only when you press Merge",
    ]);
  },
};

/** **A loop pause** (R-232, T1-28): the rule that paused the chat, and at how many times. */
export const LoopPaused: Story = {
  args: {
    rows: [
      START,
      { kind: "loop_paused", at: at(30), rule: "blocked", times: 3 },
      { kind: "loop_paused", at: at(40), rule: "failing", times: 6 },
    ],
  },
  play: async ({ canvas }) => {
    await expect(linesOf(canvas, "loop_paused")).toEqual([
      "Paused · it was blocked 3 times on the same thing",
      "Paused · the same command failed 6 times in a row",
    ]);
  },
};

/**
 * **What heads the log** (R-234, R-235, T1-28): the honesty lines first, for a chat that is
 * unverified and ran with No sandbox, then its allowed calls in one line, then its rows.
 */
export const Heading: Story = {
  args: {
    summary: { allowed: 212, by_your_rules: 9, unverified: true, ran_as_you: true },
  },
  play: async ({ canvas }) => {
    const list = canvas.getByRole("list", { name: "Chat log of steward 3" });
    const lines = [
      canvas.getByText(/^Unverified: /),
      canvas.getByText(/^Ran as you: /),
      canvas.getByText("212 allowed · 9 by your rules"),
    ];
    for (const [at, line] of lines.entries()) {
      const next = lines[at + 1] ?? list;
      await expect(line.compareDocumentPosition(next) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
        Node.DOCUMENT_POSITION_FOLLOWING,
      );
    }
  },
};

/** A block, as the core reads it: purlis's own guard refused a reset that would destroy commits. */
const BLOCK: Extract<ChatLogRow, { kind: "blocked" }> = {
  kind: "blocked",
  at: "2026-11-13T09:10:00Z",
  tool: "Bash",
  source: "guard",
  qualifier: null,
  target_kind: "guard_arm",
  target: "A3b",
  harness: "claude",
  reason: null,
  resolved: false,
};

/** The tags a row carries, by their words. */
const tagsOf = (row: HTMLElement) =>
  Array.from(row.querySelectorAll<HTMLElement>('[data-scope="badge"]')).map((tag) =>
    tag.textContent?.trim(),
  );

/**
 * **What every block row holds** (R-183, R-187, R-188): its line is an action sentence, never a
 * guard's id; its tags are `tags`, each a word beside a neutral icon; and its detail, opened from
 * the keyboard, names the source.
 */
async function aBlockRow(canvas: Canvas, line: string, tags: string[], source: string) {
  const [, row] = rowsOf(canvas);
  await expect(said(row)?.textContent).toBe(line);
  await expect(said(row)?.textContent).not.toMatch(/\bA\d|guard_arm|Refused/);
  await expect(tagsOf(row)).toEqual(tags);
  for (const tag of row.querySelectorAll('[data-scope="badge"]'))
    await expect(tag.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
  const open = canvas.getByRole("button", { expanded: false });
  open.focus();
  await userEvent.keyboard("{Enter}");
  await expect(open).toHaveAttribute("aria-expanded", "true");
  await expectFocusInSight();
  await expect(canvas.getByRole("group").textContent).toContain(source);
}

/** **purlis's own consent** (R-186): its guard blocked the call, and the row carries no tag. */
export const BlockedByPurlis: Story = {
  args: { rows: [START, BLOCK] },
  play: async ({ canvas }) =>
    aBlockRow(
      canvas,
      "purlis blocked a reset that would destroy commits",
      [],
      "Blocked by purlis's guards",
    ),
};

/** **A block by the sandbox** (R-185): the Sandbox tag. */
export const BlockedBySandbox: Story = {
  args: {
    rows: [
      START,
      { ...BLOCK, source: "sandbox", target_kind: "host", target: "registry.example.com" },
    ],
  },
  play: async ({ canvas }) =>
    aBlockRow(
      canvas,
      "The sandbox blocked a connection to registry.example.com",
      ["Sandbox"],
      "Blocked by the sandbox",
    ),
};

/** **A block by the harness's own check** (R-185, R-340): its tag, and its reason as written. */
export const BlockedByTheHarnessCheck: Story = {
  args: {
    rows: [
      START,
      {
        ...BLOCK,
        source: "harness_check",
        target_kind: null,
        target: null,
        reason: "Downloads and runs a remote script",
      },
    ],
  },
  play: async ({ canvas }) => {
    await aBlockRow(
      canvas,
      "Claude Code's own check blocked a Bash call",
      ["Claude Code's own check"],
      "Blocked by Claude Code's own check",
    );
    await expect(canvas.getByText("Downloads and runs a remote script")).toBeVisible();
  },
};

/** **An unverified report** (R-185): a block a chat reported from inside its own reach. */
export const BlockedUnverified: Story = {
  args: { rows: [START, { ...BLOCK, qualifier: "unverified" }] },
  play: async ({ canvas }) =>
    aBlockRow(
      canvas,
      "purlis blocked a reset that would destroy commits",
      ["Unverified"],
      "Blocked by purlis's guards",
    ),
};

/** **A block purlis can't tell more of** (R-185): the Can't tell tag. */
export const BlockedCantTell: Story = {
  args: {
    rows: [
      START,
      { ...BLOCK, source: "sandbox", qualifier: "cant_tell", target_kind: null, target: null },
    ],
  },
  play: async ({ canvas }) =>
    aBlockRow(
      canvas,
      "The sandbox blocked a Bash call",
      ["Sandbox", "Can't tell"],
      "Blocked by the sandbox",
    ),
};

/** **A block row's ways out** (T1-33, R-300): what a story's presses reach, none of them a core's. */
const WAYS = { tell: () => Promise.resolve({ typed: TOLD }), go: () => undefined };

/** The ways out a row offers, by their words. */
const waysOf = (canvas: Canvas) =>
  Array.from(
    canvas.getByRole("group", { name: "Ways out for steward 3" }).querySelectorAll("button"),
  ).map((one) => one.textContent?.trim());

/**
 * **A guard block's ways out** (R-184, R-189, R-191): Tell the chat… and Show this guard, and no
 * Allow. Show this guard, from the keyboard, opens a read-only detail with the guard's group.
 */
export const GuardBlockWaysOut: Story = {
  args: { rows: [START, BLOCK], ways: WAYS },
  play: async ({ canvas }) => {
    await expect(waysOf(canvas)).toEqual(["Tell the chat…", "Show this guard"]);
    await expect(canvas.queryByRole("button", { name: /allow/i })).toBeNull();
    await expect(canvas.queryByText(/Project folder/)).toBeNull();
    const show = canvas.getByRole("button", { name: "Show this guard" });
    show.focus();
    await userEvent.keyboard("{Enter}");
    await expect(show).toHaveAttribute("aria-expanded", "true");
    await expectFocusInSight();
    await expect(canvas.getByRole("region", { name: "This guard" }).textContent).toContain(
      "Project folder",
    );
  },
};

/**
 * **A harness-check block's ways out** (R-192 to R-194): Tell the chat… and Show this check, the
 * sentence that only that check decides, and no Allow and no command to copy.
 */
export const HarnessCheckBlockWaysOut: Story = {
  args: {
    rows: [START, { ...BLOCK, source: "harness_check", target_kind: null, target: null }],
    ways: WAYS,
  },
  play: async ({ canvas }) => {
    await expect(waysOf(canvas)).toEqual(["Tell the chat…", "Show this check"]);
    await expect(
      canvas.getByText("purlis can't allow this; only Claude Code's own check decides"),
    ).toBeVisible();
    await expect(canvas.queryByRole("button", { name: /allow|copy/i })).toBeNull();
    const tell = canvas.getByRole("button", { name: "Tell the chat…" });
    tell.focus();
    await userEvent.keyboard("{Enter}");
    await expect(tell).toHaveAttribute("aria-expanded", "true");
    await expectFocusInSight();
    await expect(canvas.getByRole("button", { name: "Type it in the chat" })).toBeVisible();
  },
};

/** **A Can't-tell block's way out** (R-199): Go to chat, and nothing else. */
export const CantTellBlockWayOut: Story = {
  args: {
    rows: [
      START,
      { ...BLOCK, source: "sandbox", qualifier: "cant_tell", target_kind: null, target: null },
    ],
    ways: WAYS,
  },
  play: async ({ canvas }) => {
    await expect(waysOf(canvas)).toEqual(["Go to chat"]);
    const go = canvas.getByRole("button", { name: "Go to chat" });
    go.focus();
    await expectFocusInSight();
  },
};
