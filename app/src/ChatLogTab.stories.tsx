// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's Chat log** (R-224, R-225, R-227, R-233, R-310; T1-27): its own simple timeline,
 * apart from the transcript, with a one-line start row built from the start event and the end
 * and its result. Pull-only: no count, no New marker. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import type { ChatLogRow } from "./bindings";
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
    "Started · Basic sandbox, set by the project · charter.toml at 4f2a9c1 · no answers in force",
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
    rows: [START, { kind: "held", at: at(20), tool: "Bash", arm: "merge-hold" }],
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
