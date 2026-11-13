// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { AskSource, Shown } from "./bindings";
import { onItsPane } from "./inboxRules";
import { shapeOf } from "./InboxAsks";

/**
 * **Every kind of ask the core can send has a place it is drawn, with a way to answer it.** A
 * chat that waits on the person and is drawn nowhere waits for ever: a chat's browser asked to
 * open a site, its ask listed only in Asks, and a person looking at the chat saw nothing to
 * press; a browser that needed the person lost its Take over in the Inbox, and nothing else drew
 * one.
 *
 * So each kind names its place here, and a kind the core adds fails this file (and `tsc`, through
 * `satisfies`) until someone decides where it is drawn:
 *
 * - `pane`: its chat's pane draws it, with its answers, as well as Asks.
 * - `its-own`: its chat's pane draws it some other way (its terminal, a task's question band).
 * - `no-chat`: it is no chat's (a sign-in, purlis's own), so Asks alone draws it.
 * - `asks-only`: a chat's, drawn in Asks alone. Each is listed as debt, with why.
 */
type Place =
  | { at: "pane" }
  | { at: "its-own"; how: string }
  | { at: "no-chat" }
  | { at: "asks-only"; debt: string };

const PLACES = {
  permission: { at: "pane" },
  dispatch: { at: "pane" },
  "sandbox-host": { at: "pane" },
  "sandbox-write": { at: "pane" },
  "sandbox-local": { at: "pane" },
  "workspace-removal": { at: "pane" },
  "proposed-change": { at: "pane" },
  "runner-write": { at: "pane" },
  "runner-dispatch": { at: "pane" },
  terminal: { at: "pane" },
  question: { at: "its-own", how: "a reply in Asks, or in its own terminal" },
  "task-question": { at: "its-own", how: "the task's question band on its pane" },
  "sign-in": { at: "no-chat" },
  "sandbox-only": { at: "no-chat" },
  "old-harness-plugins": { at: "no-chat" },
  signing: { at: "no-chat" },
  browser: { at: "pane" },
  "browser-needs-you": { at: "pane" },
  "browser-site": { at: "pane" },
  "browser-login": { at: "pane" },
  "secret-fill": { at: "pane" },
  vault: { at: "pane" },
  "pool-login": { at: "pane" },
  "pool-tool": { at: "pane" },
  secret: { at: "pane" },
  "run-grant": { at: "pane" },
  "sandbox-expects": { at: "no-chat" },
  "persona-allow": { at: "no-chat" },
  confirmation: { at: "no-chat" },
  budget: { at: "pane" },
  looping: { at: "pane" },
} as const satisfies Record<AskSource, Place>;

/** Each kind's place, as any place: a kind may move to or from Asks alone. */
const PLACE: Readonly<Record<AskSource, Place>> = PLACES;

const HERE = dirname(fileURLToPath(import.meta.url));

/** Every kind of ask the generated bindings say the core can send. */
function sourcesTheCoreSends(): string[] {
  const bindings = readFileSync(join(HERE, "bindings.ts"), "utf8");
  const declared = /export type AskSource =([\s\S]*?);/.exec(bindings)?.[1] ?? "";
  // Each member is a string literal, its doc comment before it.
  const code = declared.replace(/\/\*[\s\S]*?\*\//g, "");
  return [...code.matchAll(/"([a-z-]+)"/g)].map((match) => match[1]);
}

/** The path an ask of these kinds is answered on, where it changes what Asks offers. */
const PATHS: Partial<Record<AskSource, Shown["answer"]>> = {
  question: { via: "in-its-pane" },
  terminal: { via: "in-its-pane" },
  "browser-needs-you": { via: "take-over", program: "browser-1" },
};

/** One ask of `source`, offering an answer: chat 4's, or no chat's where it is no chat's. */
const askOf = (source: AskSource): Shown => {
  const session = PLACES[source].at === "no-chat" ? 0 : 4;
  return {
    session,
    ask: `${source}:${session}`,
    says: "It asks",
    options: [{ id: "one", label: "One", allows: false }],
    source,
    chain: session === 0 ? [] : ["claude 4"],
    answer: PATHS[source] ?? { via: "hook" },
  };
};

describe("every kind of ask has a drawn place", () => {
  it("names a place for every kind the core can send", () => {
    const sent = sourcesTheCoreSends();
    expect(sent.length).toBeGreaterThan(20);
    expect(Object.keys(PLACES).sort()).toEqual([...sent].sort());
  });

  it.each(Object.entries(PLACES).filter(([, place]) => place.at === "pane"))(
    "draws %s on its chat's pane",
    (source) => {
      const ask = askOf(source as AskSource);
      expect(onItsPane([ask], 4)).toEqual([ask]);
    },
  );

  it.each(Object.keys(PLACES))("lists %s in Asks with its answers, or a way to them", (source) => {
    const shape = shapeOf(askOf(source as AskSource), undefined, undefined, undefined);
    // Its answers on the row (a decision's, #5998), behind its Answer, or Go to chat, whose pane
    // draws them.
    expect(
      shape.decision || shape.form || (shape.go && PLACE[source as AskSource].at !== "asks-only"),
    ).toBe(true);
  });

  it("lists the chats' asks drawn in Asks alone as debt", () => {
    expect(
      Object.entries(PLACE)
        .filter(([, place]) => place.at === "asks-only")
        .map(([source]) => source),
    ).toEqual([]);
  });
});
