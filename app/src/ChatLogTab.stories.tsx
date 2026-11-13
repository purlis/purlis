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
