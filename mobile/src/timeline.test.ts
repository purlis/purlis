import { describe, expect, it } from "vitest";

import type { Item } from "./ipc.ts";
import { MOST_ROWS, closedBy, dismissed, empty, reduce, type Story } from "./timeline.ts";

function after(items: Item[], from: Story = empty()): Story {
  return items.reduce(reduce, from);
}

describe("reduce", () => {
  it("turns turns, tools, messages, plans and usage into rows, in order", () => {
    const story = after([
      { level: 2, item: "turn", turn: "t1", began: true },
      { level: 2, item: "message", turn: "t1", role: "user", text: "hi", done: true },
      {
        level: 2,
        item: "tool",
        tool: "x",
        title: "Read a file",
        kind: "read",
        status: "done",
        duration_ms: 12.4,
      },
      {
        level: 2,
        item: "plan",
        steps: [
          { text: "one", status: "completed" },
          { text: "two", status: "pending" },
        ],
      },
      { level: 2, item: "usage", input_tokens: 10, output_tokens: 3 },
      { level: 2, item: "turn", turn: "t1", began: false, stop_reason: "end_turn" },
    ]);
    expect(story.rows.map((row) => [row.kind, row.text])).toEqual([
      ["turn", "Turn began"],
      ["message", "You: hi"],
      ["tool", "Read a file · done · 12 ms"],
      ["plan", "✓ one\n○ two"],
      ["usage", "10 in · 3 out"],
      ["turn", "Turn ended (end_turn)"],
    ]);
    expect(new Set(story.rows.map((row) => row.key)).size).toBe(6);
  });

  it("opens an ask without the choice that lasts", () => {
    const story = after([
      {
        level: 2,
        item: "ask",
        ask: "ask-1",
        summary: "Run `rm -rf build`?",
        choices: [
          { id: "once", label: "Allow once", lasts: false },
          { id: "always", label: "Allow always", lasts: true },
          { id: "no", label: "Deny", lasts: false },
        ],
        elicits_secret: false,
      },
    ]);
    expect(story.asks).toHaveLength(1);
    expect(story.asks[0].choices.map((choice) => choice.id)).toEqual(["once", "no"]);
    expect(story.asks[0].summary).toBe("Run `rm -rf build`?");
  });

  it("closes an ask that was on the sheet and remembers who did, until dismissed", () => {
    const opened = after([
      { level: 2, item: "caught_up" },
      { level: 2, item: "ask", ask: "ask-1", summary: "?", choices: [], elicits_secret: false },
    ]);
    const closed = reduce(opened, { level: 2, item: "ask_closed", ask: "ask-1", by: "desktop" });
    expect(closed.asks).toEqual([]);
    expect(closed.closed).toEqual({ ask: "ask-1", by: "desktop" });
    expect(dismissed(closed).closed).toBeNull();
  });

  it("closes quietly an ask replayed from before the attach, or one never shown", () => {
    const replayed = after([
      { level: 2, item: "ask", ask: "old", summary: "?", choices: [], elicits_secret: false },
      { level: 2, item: "ask_closed", ask: "old", by: "desktop" },
      { level: 2, item: "caught_up" },
      { level: 2, item: "ask_closed", ask: "never-shown", by: "desktop" },
    ]);
    expect(replayed.asks).toEqual([]);
    expect(replayed.closed).toBeNull();
  });

  it("marks the snapshot over at caught_up", () => {
    expect(empty().caughtUp).toBe(false);
    expect(after([{ level: 2, item: "caught_up" }]).caughtUp).toBe(true);
  });

  it("keeps at most MOST_ROWS rows, the newest", () => {
    const many: Item[] = Array.from({ length: MOST_ROWS + 5 }, (_, i) => ({
      level: 2,
      item: "message",
      turn: "t",
      role: "agent",
      text: `m${i}`,
      done: true,
    }));
    const story = after(many);
    expect(story.rows).toHaveLength(MOST_ROWS);
    expect(story.rows[0].text).toBe("Agent: m5");
  });
});

describe("closedBy", () => {
  it("says who answered", () => {
    expect(closedBy("desktop")).toBe("Answered on the desktop");
    expect(closedBy("device")).toBe("Answered on this phone");
    expect(closedBy("timeout")).toBe("Closed by timeout");
  });
});
