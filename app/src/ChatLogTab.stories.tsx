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
