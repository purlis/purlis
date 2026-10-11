import { describe, expect, it } from "vitest";
import type { Shown } from "./bindings";
import { MOST_ON_A_PANE, onItsPane } from "./inboxRules";

/**
 * **A chat's pane draws at most two of its asks** (spec #1688, #1695): its in-context copy of
 * what the Inbox lists, the longest waiting first; a reply is answered in the pane itself.
 */

const ask = (session: number, key: string, source: Shown["source"], since?: number): Shown => ({
  session,
  ask: key,
  says: key,
  options: [],
  source,
  chain: [`chat ${session}`],
  answer: { via: "in-its-pane" },
  since,
});

describe("the asks a pane draws", () => {
  it("draws the chat's two longest waiting, and leaves the rest to the Inbox", () => {
    const asks = [
      ask(3, "block:3:a", "sandbox-host", 300),
      ask(3, "dispatch:1", "dispatch", 100),
      ask(4, "dispatch:2", "dispatch", 50),
      ask(3, "block:3:b", "sandbox-write", 200),
    ];
    expect(MOST_ON_A_PANE).toBe(2);
    expect(onItsPane(asks, 3).map((one) => one.ask)).toEqual(["dispatch:1", "block:3:b"]);
  });

  it("keeps the registry's order where no time is known, after every ask that has one", () => {
    const asks = [
      ask(3, "permission-a", "permission"),
      ask(3, "terminal:3", "terminal"),
      ask(3, "dispatch:1", "dispatch", 100),
    ];
    expect(onItsPane(asks, 3).map((one) => one.ask)).toEqual(["dispatch:1", "permission-a"]);
  });

  it("draws no Notice for a reply the chat waits on: the pane is where it is typed", () => {
    expect(onItsPane([ask(3, "question:3", "question")], 3)).toEqual([]);
  });
});
