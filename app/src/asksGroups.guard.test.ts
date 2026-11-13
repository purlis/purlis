// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { AskSource } from "./bindings";

/**
 * **Asks holds asks only, in seven groups** (R-240, Q220): sandbox, harness prompt, consent,
 * hold, loop pause, budget and hook give-up. The groups are a grouping over every kind of ask
 * the core can send (`AskSource`), not a replacement: no kind is dropped, and each is in exactly
 * one group. A block is never the source of an ask, so no group holds one.
 *
 * Each group is named by the core's word for it (`purlis_core::eventlog::stops::AskGroup`), the
 * one an answered ask is written under. A kind the core adds fails this file (and `tsc`, through
 * `satisfies`) until it is grouped: Trust this clone and Trust these clones join, as consents,
 * when T1-14 (#6405) and T1-15 (#6406) add them.
 */
type Group =
  "sandbox" | "harness_prompt" | "consent" | "hold" | "loop_pause" | "budget" | "hook_give_up";

/** Every group, in the order R-240 names them. */
const GROUPS: readonly Group[] = [
  "sandbox",
  "harness_prompt",
  "consent",
  "hold",
  "loop_pause",
  "budget",
  "hook_give_up",
];

const GROUP_OF = {
  // A refusal of the chat's sandbox while it waits on it (R-195), or a start it refused.
  "sandbox-host": "sandbox",
  "sandbox-write": "sandbox",
  "sandbox-local": "sandbox",
  "sandbox-only": "sandbox",
  // What the chat or its harness asks the person, mid-turn or at its end.
  permission: "harness_prompt",
  question: "harness_prompt",
  "task-question": "harness_prompt",
  "browser-needs-you": "harness_prompt",
  // What only the person's yes lets purlis do: a grant, a write, a sign-in, a confirmation.
  dispatch: "consent",
  "workspace-removal": "consent",
  "proposed-change": "consent",
  "runner-write": "consent",
  "runner-dispatch": "consent",
  "sign-in": "consent",
  "old-harness-plugins": "consent",
  signing: "consent",
  browser: "consent",
  "browser-site": "consent",
  "browser-login": "consent",
  "pool-login": "consent",
  "pool-tool": "consent",
  "secret-fill": "consent",
  secret: "consent",
  "run-grant": "consent",
  vault: "consent",
  // The sandbox consent (R-205): what the project expects and this machine has not allowed.
  "sandbox-expects": "consent",
  confirmation: "consent",
  // What purlis refused the chat and raised as an ask (R-114).
  "merge-hold": "hold",
  looping: "loop_pause",
  budget: "budget",
  // A prompt in the harness's own terminal that no hook holds: "Waiting in its terminal".
  terminal: "hook_give_up",
} as const satisfies Record<AskSource, Group>;

/** Each kind's group, as any group. */
const GROUP: Readonly<Record<AskSource, Group>> = GROUP_OF;

const HERE = dirname(fileURLToPath(import.meta.url));

/** Every kind of ask the generated bindings say the core can send. */
function sourcesTheCoreSends(): string[] {
  const bindings = readFileSync(join(HERE, "bindings.ts"), "utf8");
  const declared = /export type AskSource =([\s\S]*?);/.exec(bindings)?.[1] ?? "";
  // Each member is a string literal, its doc comment before it.
  const code = declared.replace(/\/\*[\s\S]*?\*\//g, "");
  return [...code.matchAll(/"([a-z-]+)"/g)].map((match) => match[1]);
}

describe("Asks holds asks only, in seven groups (R-240)", () => {
  it("every ask kind maps to exactly one of the seven Asks groups", () => {
    const sent = sourcesTheCoreSends();
    // The 30 kinds shipped before phase 1, and the merge hold (T1-40's R-114).
    expect(sent.length).toBeGreaterThanOrEqual(31);
    expect(Object.keys(GROUP).sort()).toEqual([...sent].sort());
    for (const source of sent) expect(GROUPS).toContain(GROUP[source as AskSource]);
  });

  it("holds an ask in every one of the seven groups, and nothing else", () => {
    expect([...new Set(Object.values(GROUP))].sort()).toEqual([...GROUPS].sort());
  });

  it("names no kind of block as the source of an ask", () => {
    expect(sourcesTheCoreSends().filter((source) => /block/.test(source))).toEqual([]);
  });
});
