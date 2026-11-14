// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { ChatLogRow } from "./bindings";
import { ChatLogRows } from "./ChatLogTab";
import { blockSaid } from "./chatLog";

/**
 * **A block row in the Chat log** (R-170, R-183, R-185 to R-188; T1-29): an action sentence the
 * window builds, never a guard's id; a tag where the source is the sandbox or the harness's own
 * check, or the report is unverified or can't be told, and none on purlis's own consents; and a
 * detail that always names the source.
 */

afterEach(cleanup);

type Blocked = Extract<ChatLogRow, { kind: "blocked" }>;

const blocked = (over: Partial<Blocked> = {}): Blocked => ({
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
  ...over,
});

/** Every arm of purlis's gate, by the id a block's target carries (`toolgate::Arm`). */
const ARMS = [
  "caps",
  "A",
  "A2",
  "A3",
  "A3b",
  "A3c",
  "A3d",
  "A4",
  "A5",
  "A6",
  "A7",
  "A7b",
  "A7c",
  "A7d",
  "A7e",
  "A8",
  "A9",
  "A9b",
  "A10",
];

/** A guard's id, a group's id or a source's key, as a sentence must never say them. */
const AN_ID = /\b(A\d*[a-e]?|caps)\b|guard_arm|harness_check|project_setting|cant_tell|\bstop\./;

/** The rows drawn, each `<li>`. */
function drawn(rows: ChatLogRow[]): HTMLElement[] {
  render(<ChatLogRows name="steward 3" rows={rows} />);
  return within(screen.getByRole("list", { name: "Chat log of steward 3" })).getAllByRole(
    "listitem",
  );
}

/** The tags a row carries, by their words. */
const tagsOf = (row: HTMLElement) =>
  Array.from(row.querySelectorAll<HTMLElement>('[data-scope="badge"]')).map((tag) =>
    tag.textContent?.trim(),
  );

/** Opens a row's detail and answers it: its disclosure, the first of its closed buttons, before
 *  its ways out (T1-33). */
async function detailOf(row: HTMLElement): Promise<HTMLElement> {
  await userEvent.click(within(row).getAllByRole("button", { expanded: false })[0]);
  return within(row).getAllByRole("group")[0];
}

describe("a block row", () => {
  it("a block row is an action sentence not a guard id", () => {
    for (const arm of ARMS) {
      const said = blockSaid(blocked({ target: arm }));
      expect(said, arm).toMatch(/^purlis blocked an? \S/);
      expect(said, arm).not.toMatch(AN_ID);
    }
    expect(blockSaid(blocked({ target_kind: null, target: null, tool: "Write" }))).toBe(
      "purlis blocked a Write call",
    );
    expect(blockSaid(blocked({ source: "harness_check", target_kind: null, target: null }))).toBe(
      "Claude Code's own check blocked a Bash call",
    );
    expect(
      blockSaid(blocked({ source: "sandbox", target_kind: "host", target: "example.com:443" })),
    ).toBe("The sandbox blocked a connection to example.com:443");
    expect(blockSaid(blocked({ source: "sandbox", target_kind: "folder", target: "." }))).toBe(
      "The sandbox blocked a change in the project's folder",
    );

    // Drawn as the row's line, with Blocked as its word and no Refused anywhere.
    const [row] = drawn([blocked()]);
    const line = row.querySelector(".chat-log-said")?.textContent ?? "";
    expect(line).toBe(blockSaid(blocked()));
    expect(line).not.toMatch(AN_ID);
    expect(row.textContent).not.toMatch(/refused/i);
  });

  it("a block row shows a tag for the Sandbox, the harness check, unverified and Can't tell", () => {
    const rows = drawn([
      blocked({ source: "sandbox", target_kind: "host", target: "example.com" }),
      blocked({ source: "harness_check", target_kind: null, target: null }),
      blocked({ qualifier: "unverified" }),
      blocked({ source: "sandbox", qualifier: "cant_tell", target_kind: null, target: null }),
      blocked({ source: "harness_check", harness: "codex", qualifier: "unverified" }),
    ]);
    // Unverified and can't-tell reports collapse into a line each (R-105, T1-38), tagged with
    // their qualifier alone: who blocked each is in the line's detail, in fixed words.
    expect(rows.map(tagsOf)).toEqual([
      ["Sandbox"],
      ["Claude Code's own check"],
      ["Unverified"],
      ["Can't tell"],
    ]);
  });

  it("a purlis consent row carries no tag", () => {
    const [row] = drawn([blocked()]);
    // Drawn as the block it is, by purlis's own guard, verified...
    expect(row.querySelector(".chat-log-said")?.textContent).toBe(
      "purlis blocked a reset that would destroy commits",
    );
    // ...and with no tag: no badge, and no word of one anywhere in the row.
    expect(tagsOf(row)).toEqual([]);
    expect(row.textContent).not.toMatch(/Sandbox|own check|Unverified|Can't tell/);
  });

  it("every block row detail names its source", async () => {
    const cases: [Blocked, string][] = [
      [blocked(), "Blocked by purlis's guards"],
      // An unverified report's detail names who blocked a call, in fixed words (R-104).
      [blocked({ qualifier: "unverified" }), "purlis blocked a call"],
      [blocked({ source: "harness_check", target: null }), "Blocked by Claude Code's own check"],
      [
        blocked({ source: "sandbox", target_kind: "host", target: "a.dev" }),
        "Blocked by the sandbox",
      ],
      [blocked({ source: "policy" }), "Blocked by your organisation's policy"],
      [blocked({ source: "project_setting" }), "Blocked by the project's settings"],
    ];
    for (const [row, source] of cases) {
      const [drawnRow] = drawn([row]);
      const detail = await detailOf(drawnRow);
      expect(detail.textContent).toContain(source);
      cleanup();
    }
  });

  it("source tags are words with neutral icons and never emoji", () => {
    const rows = drawn([
      blocked({ source: "sandbox", qualifier: "cant_tell", target: null }),
      blocked({ source: "harness_check", qualifier: "unverified", target: null }),
    ]);
    const tags = rows.flatMap((row) =>
      Array.from(row.querySelectorAll<HTMLElement>('[data-scope="badge"]')),
    );
    expect(tags).toHaveLength(2);
    for (const tag of tags) {
      expect(tag.dataset.variant).toBe("neutral");
      const icon = tag.querySelector("svg");
      expect(icon).not.toBeNull();
      expect(icon?.getAttribute("aria-hidden")).toBe("true");
      expect(tag.textContent).toMatch(/^[A-Za-z' ]+$/);
    }
    // The lint over the tag component: no emoji, written or escaped, anywhere in its source.
    const source = readFileSync(join(process.cwd(), "src/BlockTag.tsx"), "utf8");
    expect(source).not.toMatch(/\p{Extended_Pictographic}/u);
    expect(source).not.toMatch(/\\u\{?(1F|26|27)[0-9A-F]{2,3}/i);
  });
});
