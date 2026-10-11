import { describe, expect, it } from "vitest";
import type { Shown } from "./bindings";
import { MOST_ON_A_PANE, offItsPane, onItsPane } from "./inboxRules";

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
    // The one the cap keeps off is counted, so the pane can say it waits in the Inbox.
    expect(offItsPane(asks, 3)).toBe(1);
    expect(offItsPane(asks, 4)).toBe(0);
  });

  it("keeps the registry's order where no time is known, after every ask that has one", () => {
    const asks = [
      ask(3, "permission-a", "permission"),
      ask(3, "terminal:3", "terminal"),
      ask(3, "dispatch:1", "dispatch", 100),
    ];
    expect(onItsPane(asks, 3).map((one) => one.ask)).toEqual(["dispatch:1", "permission-a"]);
  });

  it("counts the hosts one block's Notice lists as that one Notice", () => {
    const host = (target: string, since: number): Shown => ({
      ...ask(3, `block:3:connect:host:${target}`, "sandbox-host", since),
      answer: {
        via: "sandbox-block",
        shown: { operation: "connect", kind: "host", what: "host", target },
      },
    });
    const asks = [
      host("a.example.com", 100),
      host("b.example.com", 110),
      ask(3, "dispatch:1", "dispatch", 120),
      ask(3, "block:3:write:home:/w", "sandbox-write", 130),
    ];
    expect(onItsPane(asks, 3).map((one) => one.ask)).toEqual([
      "block:3:connect:host:a.example.com",
      "block:3:connect:host:b.example.com",
      "dispatch:1",
    ]);
    expect(offItsPane(asks, 3)).toBe(1);
  });

  it("draws no Notice for a reply the chat waits on: the pane is where it is typed", () => {
    expect(onItsPane([ask(3, "question:3", "question")], 3)).toEqual([]);
  });
});
