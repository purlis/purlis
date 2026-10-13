// A chat's story as the strip draws it (iOS design §5.1, §6.1): the timeline's items reduced
// to rows, the asks still open, and who closed the last one. A choice that lasts is left out
// before the sheet ever sees it (D8).

import type { Choice, Item } from "./ipc.ts";

export type Row = {
  key: string;
  kind: "turn" | "tool" | "message" | "plan" | "usage";
  text: string;
};

export type OpenAsk = {
  ask: string;
  summary: string;
  /** The choices a phone may make: none that lasts. */
  choices: Choice[];
  elicitsSecret: boolean;
};

export type Closed = { ask: string; by: string };

export type Story = {
  rows: Row[];
  asks: OpenAsk[];
  /** The last ask that closed under the phone, and by whom, until the sheet is dismissed. */
  closed: Closed | null;
  /** Whether the snapshot is over: everything after happened after the attach. */
  caughtUp: boolean;
};

export function empty(): Story {
  return { rows: [], asks: [], closed: null, caughtUp: false };
}

/** The most rows the strip keeps: the host's buffer is bounded, and so is this. */
export const MOST_ROWS = 500;

let counter = 0;
function key(prefix: string): string {
  counter += 1;
  return `${prefix}-${counter}`;
}

function withRow(story: Story, row: Row): Story {
  const rows = [...story.rows, row];
  return { ...story, rows: rows.length > MOST_ROWS ? rows.slice(rows.length - MOST_ROWS) : rows };
}

/** The story after `item`. */
export function reduce(story: Story, item: Item): Story {
  switch (item.item) {
    case "turn":
      return withRow(story, {
        key: key("turn"),
        kind: "turn",
        text: item.began
          ? "Turn began"
          : `Turn ended${item.stop_reason ? ` (${item.stop_reason})` : ""}`,
      });
    case "tool": {
      const took = item.duration_ms == null ? "" : ` · ${Math.round(item.duration_ms)} ms`;
      return withRow(story, {
        key: key("tool"),
        kind: "tool",
        text: `${item.title} · ${item.status}${took}`,
      });
    }
    case "message":
      return withRow(story, {
        key: key("message"),
        kind: "message",
        text: `${item.role === "user" ? "You" : "Agent"}: ${item.text}`,
      });
    case "plan":
      return withRow(story, {
        key: key("plan"),
        kind: "plan",
        text: item.steps
          .map((step) => `${step.status === "completed" ? "✓" : "○"} ${step.text}`)
          .join("\n"),
      });
    case "ask":
      return {
        ...story,
        asks: [
          ...story.asks.filter((open) => open.ask !== item.ask),
          {
            ask: item.ask,
            summary: item.summary,
            choices: item.choices.filter((choice) => !choice.lasts),
            elicitsSecret: item.elicits_secret,
          },
        ],
      };
    case "ask_closed": {
      // Only an ask that closed under the sheet is announced: one replayed from before the
      // attach, or one the phone never showed, closes quietly.
      const shown = story.caughtUp && story.asks.some((open) => open.ask === item.ask);
      return {
        ...story,
        asks: story.asks.filter((open) => open.ask !== item.ask),
        closed: shown ? { ask: item.ask, by: item.by } : story.closed,
      };
    }
    case "usage":
      return withRow(story, {
        key: key("usage"),
        kind: "usage",
        text: `${item.input_tokens} in · ${item.output_tokens} out`,
      });
    case "caught_up":
      return { ...story, caughtUp: true };
    default:
      return story;
  }
}

/** The sheet's closing line: who answered. */
export function closedBy(by: string): string {
  if (by === "desktop") return "Answered on the desktop";
  if (by === "device") return "Answered on this phone";
  return `Closed by ${by}`;
}

/** The sheet, dismissed. */
export function dismissed(story: Story): Story {
  return { ...story, closed: null };
}
