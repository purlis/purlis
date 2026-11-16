// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import type { ChatLogRow } from "./bindings";
import { ChatLogRows } from "./ChatLogTab";
import { askSaid, endSaid, startSaid } from "./chatLog";
import { windowCopy } from "./windowCopy";

/**
 * **The Chat log's rows in words** (R-227's row, R-233, R-310; T1-27), as its stories draw
 * them: the start row is one line built from the start event's four fields, never from the
 * chat; the end row says the result.
 */

afterEach(cleanup);

type Started = Extract<ChatLogRow, { kind: "started" }>;
type Ended = Extract<ChatLogRow, { kind: "ended" }>;

const started = (over: Partial<NonNullable<Started["protection"]>> = {}): Started => ({
  kind: "started",
  at: "2026-11-13T09:00:00Z",
  cause: "start",
  protection: {
    layers: [{ layer: "sandbox", level: "off", tier: "chat" }],
    charter_commit: "9b1d2e3f4a",
    charter_file: "purlis.toml",
    charter_dirty: true,
    answers_in_force: 2,
    ...over,
  },
});

const ended = (over: Partial<Ended>): Ended => ({
  kind: "ended",
  at: "2026-11-13T10:00:00Z",
  state: "completed",
  cause: "exited",
  code: 0,
  ...over,
});

describe("the Chat log start row is one line built from the start event", () => {
  it("says each of the four fields as the event recorded them", () => {
    expect(startSaid(started())).toBe(
      "Started · No sandbox, set by you for this chat only · purlis.toml at 9b1d2e3, with changes not committed · 2 answers in force",
    );
    expect(
      startSaid(
        started({
          layers: [{ layer: "sandbox", level: "on", tier: "policy" }],
          charter_commit: "unknown",
          charter_dirty: false,
          answers_in_force: 0,
        }),
      ),
    ).toBe(
      "Started · Full sandbox, set by an administrator's policy · purlis.toml's commit not known · no answers in force",
    );
  });

  // Q280. Mutation caught: the file named as `purlis.toml` whatever the plane has, which says
  // a `charter.toml` plane's commit is of a file it does not have.
  it("names the plane's own file the start recorded the commit of", () => {
    expect(startSaid(started({ charter_file: "charter.toml" }))).toContain(
      "charter.toml at 9b1d2e3, with changes not committed",
    );
    expect(startSaid(started({ charter_file: "charter.toml", charter_commit: "none" }))).toContain(
      "charter.toml in no commit",
    );
    expect(
      startSaid(started({ charter_file: "charter.toml", charter_commit: "unknown" })),
    ).toContain("charter.toml's commit not known");
    expect(startSaid(started())).not.toContain("charter.toml");
  });

  it("says a start that recorded none, or a tier this version does not know, as such", () => {
    expect(startSaid({ ...started(), protection: null })).toBe(
      "Started · what protected it was not recorded",
    );
    expect(
      startSaid(started({ layers: [{ layer: "sandbox", level: "floor", tier: "workspace" }] })),
    ).toContain("Basic sandbox, set by a tier this version does not know");
  });

  it("says a layer whose setter the event could not tell as not known", () => {
    expect(
      startSaid(started({ layers: [{ layer: "sandbox", level: "on", tier: "unknown" }] })),
    ).toContain("Full sandbox, who set it is not known");
  });
});

describe("the Chat log lists the chat end and its result", () => {
  it("says a done chat and a failed one apart", () => {
    expect(endSaid(ended({}))).toBe("Ended · done");
    expect(endSaid(ended({ state: "failed", code: 3 }))).toBe("Ended · failed, exit code 3");
    expect(endSaid(ended({ state: "failed", cause: "host-crash", code: null }))).toBe(
      "Ended · failed, as purlis stopped while it ran",
    );
    expect(endSaid(ended({ state: "stopped", cause: "operator", code: null }))).toBe(
      "Ended · stopped by you",
    );
  });

  it("says a run the project's settings replaced was started again, never closed (Q247)", () => {
    expect(endSaid(ended({ state: "completed", cause: "superseded", code: null }))).toBe(
      "Ended · started again because the project's settings changed",
    );
  });

  it("draws its own timeline, the start first and the end last, with no live region", () => {
    render(
      <ChatLogRows name="steward 3" rows={[started(), ended({ state: "failed", code: 3 })]} />,
    );

    const rows = within(screen.getByRole("list", { name: "Chat log of steward 3" })).getAllByRole(
      "listitem",
    );
    expect(rows.map((row) => row.textContent)).toEqual([
      "2026-11-13 09:00 UTCStarted · No sandbox, set by you for this chat only · purlis.toml at 9b1d2e3, with changes not committed · 2 answers in force",
      "2026-11-13 10:00 UTCEnded · failed, exit code 3",
    ]);
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("log")).toBeNull();
  });
});

describe("every Ask group has a Chat log sentence (R-229, T1a-61)", () => {
  /** Every group the core writes an ask under (`AskGroup`), phase 1a's two last. */
  const GROUPS = [
    "sandbox",
    "harness_prompt",
    "consent",
    "hold",
    "loop_pause",
    "budget",
    "hook_give_up",
    "model_key",
    "pool_allow",
  ];

  it("every_ask_group_has_a_chat_log_sentence", () => {
    for (const ask of GROUPS) {
      expect(askSaid({ kind: "ask", at: null, ask, answered: null })).not.toMatch(
        /Asked you something/,
      );
    }
  });

  it("says phase 1a's groups in W94 and W95, and a model key's Allow in W122", () => {
    expect(askSaid({ kind: "ask", at: null, ask: "model_key", answered: null })).toBe(
      `Asked you ${windowCopy.W94} · Not answered`,
    );
    expect(askSaid({ kind: "ask", at: null, ask: "pool_allow", answered: null })).toBe(
      `Asked you ${windowCopy.W95} · Not answered`,
    );
    expect(
      askSaid({
        kind: "ask",
        at: null,
        ask: "model_key",
        answered: { answer: "allowed", by: "window" },
      } as Parameters<typeof askSaid>[0]),
    ).toBe(`Asked you ${windowCopy.W94} · Allowed ${windowCopy.W122}, by you in the window`);
  });
});
