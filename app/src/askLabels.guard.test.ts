// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * **One place for the Allow labels** (#1700): the words an ask's answers say are written once,
 * in the asks registry (`asking.rs`, `allow_said`), and every Notice that answers an ask, and
 * the Inbox, draws the label its ask's option carries. No string these files write may spell
 * one of them: a second spelling is how the Notice and the Inbox came to say "Allow for this
 * chat" and "Allow only for this chat" for one answer.
 */

const HERE = dirname(fileURLToPath(import.meta.url));

/** The files that draw an ask's answers. */
const DRAWING_ASKS = [
  "SandboxBlockNotice.tsx",
  "DispatchGrantNotice.tsx",
  "TaskBlocksNotice.tsx",
  "TaskPromptNotice.tsx",
  "WorkspaceRemovalNotice.tsx",
  "Inbox.tsx",
  "InboxAsks.tsx",
  // A held merge, on its row and on its chat's pane (T1-39, T1-40).
  "MergeHoldNotice.tsx",
  "MergeFacts.tsx",
  "PausedNotice.tsx",
  // A block's ways out (T1-33): its Chat log row, and what draws them there and on a finish row.
  "ChatLogTab.tsx",
  "BlockWaysOut.tsx",
];

/** The registry's labels, as `asking.rs` writes them. */
const LABELS = [
  "Allow for this chat",
  "Allow only for this chat",
  "Allow for me on this machine",
  "Allow for everyone in this project",
  "Keep blocked",
  "Never for this pair",
  // A held merge's word for no (R-303).
  "Don't merge",
  // A block loop's pause (T1-36, R-217), as `asking/pauses.rs` writes them.
  "Tell it to stop trying",
  "Let it keep trying",
  "Stop chat",
  // A block's ways out (T1-33; R-189, R-192, R-199), as `blockWays.ts` writes them.
  "Tell the chat…",
  "Show this guard",
  "Show this check",
  "Go to chat",
];

/** A block's ways out (T1-33), each by the name `blockWays.ts` writes it under. */
const WAYS_OUT: Record<string, string> = {
  TELL_THE_CHAT: "Tell the chat…",
  SHOW_THIS_GUARD: "Show this guard",
  SHOW_THIS_CHECK: "Show this check",
  GO_TO_CHAT: "Go to chat",
};

/** The loop pause's labels (R-217), in the order its ask offers them. */
const LOOP_PAUSE = ["Tell it to stop trying", "Let it keep trying", "Stop chat"];

const PAUSES = readFileSync(join(HERE, "../src-tauri/src/asking/pauses.rs"), "utf8");

/**
 * **Labels a sentence may name by name**: "Go to chat" is the shipped control every chat row has
 * (`ChatActions.tsx`), and a sentence may tell the person to press it ("No message to show. Go
 * to chat to see what it said."). Only a literal that is the label, or the label and a name
 * after it, spells it.
 */
const NAMED_IN_PROSE = ["Go to chat"];

/** Whether the literal `one` spells `label`. */
const spells = (one: string, label: string) =>
  NAMED_IN_PROSE.includes(label)
    ? one.trim() === label || one.trim().startsWith(label + " ${")
    : one.includes(label);

/** Every string literal of `source`, comments left out. */
function literals(source: string): string[] {
  const code = source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
  return [...code.matchAll(/"((?:[^"\\\n]|\\.)*)"|`((?:[^`\\]|\\.)*)`|>([^<>{}]+)</g)].map(
    (match) => match[1] ?? match[2] ?? match[3] ?? "",
  );
}

describe("an ask's answers are labelled once, in the registry", () => {
  it.each(DRAWING_ASKS)("%s spells no answer of its own", (file) => {
    const said = literals(readFileSync(join(HERE, file), "utf8"));
    const spelled = said.filter((one) => LABELS.some((label) => spells(one, label)));
    expect(spelled).toEqual([]);
  });

  it("knows the loop pause's answers as pauses.rs writes them, the lead first (R-217)", () => {
    const said = ["STOP_TRYING_SAID", "KEEP_TRYING_SAID", "STOP_CHAT_SAID"].map(
      (name) => new RegExp(`const ${name}: &str = "([^"]*)";`).exec(PAUSES)?.[1],
    );
    expect(said).toEqual(LOOP_PAUSE);
    expect(LOOP_PAUSE.every((label) => LABELS.includes(label))).toBe(true);
  });

  it("knows a block's ways out as blockWays.ts writes them, once (T1-33)", () => {
    const ways = readFileSync(join(HERE, "blockWays.ts"), "utf8");
    for (const [name, label] of Object.entries(WAYS_OUT)) {
      expect(new RegExp(`export const ${name} = "([^"]*)";`).exec(ways)?.[1], name).toBe(label);
      expect(LABELS, name).toContain(label);
    }
  });

  it("would see a label spelled in a string or in markup", () => {
    expect(literals('const keep = { label: "Keep blocked" };')).toContain("Keep blocked");
    expect(literals("<button>Allow for this chat</button>")).toContain("Allow for this chat");
    expect(literals("// Keep blocked, said in a comment")).toEqual([]);
    // A label a sentence may name is spelled only as itself, or with a name after it.
    expect(spells("Go to chat", "Go to chat")).toBe(true);
    expect(spells("Go to chat ${who}", "Go to chat")).toBe(true);
    expect(spells("No message to show. Go to chat to see what it said.", "Go to chat")).toBe(false);
    expect(spells("Press Keep blocked.", "Keep blocked")).toBe(true);
  });
});
