// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { useOnShownAgain, useProjectShown } from "./projectShown";
import { commands, type ChatUsage, type PlaneId } from "./bindings";
import type { FootPart } from "./paneHead";
import { everyWhileShown } from "./whileShown";
import { Button } from "./ui/button/Button";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { PaneFootGauge } from "./ui/pane-foot/PaneFoot";

/**
 * **A chat's `ctx`/`cache` gauge, on its own pane's foot** (#5202).
 *
 * charter ADR 0019 wrote the loss down: *"A framed Claude Code session has no context/cache
 * gauge on any surface."* charter's statusline IS Claude Code's `statusLine`, so running Claude
 * Code inside charter cost the operator the context percentage he would have seen outside it.
 * ADR 0038 named it again for the app and did not rule where it goes; this is that ruling.
 *
 * # Why the pane, and not the tab or a panel
 *
 * - **Not a panel, and not the status line.** Both describe the focused WORKSPACE, and `ctx`
 *   describes one CONVERSATION. A window-wide gauge would be one chat's number under fifty,
 *   drawn as if it were the window's.
 * - **Not the chat's tab.** ADR 0026 measured fifty tabs, and ADR 0039 already fights for
 *   their width with a name, a state mark and a pin. A percentage on every tab is fifty
 *   numbers in a row, which is the "furniture" the footer's own zero rule exists to avoid —
 *   and a tab is read to FIND a chat, not to read one.
 * - **The pane is where the conversation is.** It is the chat the operator is typing into,
 *   and the one whose context is about to run out on them. It is also where these numbers
 *   were drawn before — charter's own footer, zone 3, inside the pane, until the app blanked
 *   that footer (ADR 0029) — so it is where the eye already goes.
 *
 * **What it costs, said:** a chat that is not on screen shows no gauge. The number is kept —
 * it is on disk, per conversation — and is drawn the moment its pane is.
 *
 * # When it reads, since nothing pushes it
 *
 * The record is written by `charter statusline`, a process Claude Code runs; nothing tells the
 * window. So it is read when this pane opens, when the chat's state moves (a turn starting or
 * ending — the moments a number changes), once more a moment after each move (Claude Code
 * renders its status line a little after the `Stop` that ended the turn), and every few seconds
 * while the chat is mid-turn and the window is shown. Each read is one bounded read of a
 * sixteen-line file in the core — nothing spawns.
 *
 * # What it draws
 *
 * The core's gauge, on the pane foot's end (look A, spec #5191): two labelled numbers, `ctx NN%`
 * with its fill as a thin bar under it, and `cache NN%`, each in the tone the core's threshold
 * gives it, so this and the frame's panel can never colour one number two ways. **Nothing at all
 * when nothing is known**, never `ctx 0%`. Its tooltip says the rest in words: how many times the
 * prefix was rebuilt and what that cost, and a streak of cold turns (`_cache_hint`'s rule, which
 * Python only ever said in the live footer).
 *
 * **No trend.** ADR 0038 named "the token trend" as its own gap, and for a while a row of bars
 * stood beside the numbers, one per recorded turn. Spec #5191 took it away (story 15): the bar
 * under `ctx` says what the number and the row said twice. The turns are still in the record.
 */

/** How long after a move the record is read again, for the render that lands after `Stop`. */
export const AFTER_A_MOVE_MS = 2000;
/** How often the record is read while the chat is mid-turn. */
export const WHILE_RUNNING_MS = 3000;

/**
 * What one chat's record says, kept current by the rule in the module doc.
 *
 * `moved` is the board's count for this chat (`movedAt`), which changes exactly when its state
 * does; `running` is whether it is mid-turn now.
 */
export function useChatUsage(
  plane: PlaneId,
  session: number,
  moved: number,
  running: boolean,
): ChatUsage | undefined {
  const [usage, setUsage] = useState<{ session: number; usage: ChatUsage | null }>();
  /** The reading last drawn, read in the answer's callback to leave an unchanged one undrawn. */
  const held = useRef(usage);
  // **Nothing is read for a chat of a project that is not in front** (`projectShown.tsx`): its
  // pane is kept and not drawn again, so `running` here is what it was as the project left, and
  // a beat on it would read every three seconds for a number nobody sees.
  //
  // **As the project comes back it is read after the project is painted, not before.** The
  // command is one the core answers on the window's own thread, and every pane of the tab in
  // front has a gauge: read in the switch, that is one such command per pane ahead of the
  // paint, which is the very thing a switch is meant not to wait on. The gauge shows what it
  // held, and the read behind it corrects it.
  const shown = useProjectShown();

  /** The read in force, for the one made as the project comes back. */
  const reading = useRef<() => void>(undefined);
  /** Whether the project was behind when this last ran: coming back is not a move. */
  const wasBehind = useRef(!shown);
  useOnShownAgain(() => reading.current?.());

  useEffect(() => {
    const back = wasBehind.current;
    wasBehind.current = !shown;
    if (!shown) return;
    let gone = false;
    const read = () => {
      void commands
        .chatUsage(plane, session)
        .then((answer) => {
          if (gone || answer.status !== "ok") return;
          // Anything that is not a gauge — a test's catch-all, an older core — draws nothing.
          const found = answer.data;
          const gauge = found && typeof found === "object" && !Array.isArray(found) ? found : null;
          // **A reading that says what the last one said draws nothing** (#4882): every move,
          // and every beat mid-turn, reads again, and most find the record as it was.
          const was = held.current;
          if (was?.session === session && sameReading(was.usage, gauge)) return;
          held.current = { session, usage: gauge };
          setUsage(held.current);
        })
        .catch(() => {
          // A read that failed changes nothing on screen: the last known number stays until
          // a read succeeds, and an unknown is never drawn as a zero.
        });
    };
    reading.current = read;
    // At a move, and when the pane opens: at once, and again a moment later. Not as the
    // project comes back, which reads once, after the paint (above).
    if (!back) read();
    const again = back ? undefined : setTimeout(read, AFTER_A_MOVE_MS);
    // Mid-turn, on the window's one beat: nothing while the window is hidden, one read the
    // moment it is shown again (`whileShown.ts`).
    const ticking = running ? everyWhileShown(WHILE_RUNNING_MS, read) : undefined;
    return () => {
      gone = true;
      reading.current = undefined;
      clearTimeout(again);
      ticking?.();
    };
  }, [plane, session, moved, running, shown]);

  // Keyed on the session, so a pane handed another chat never shows the last one's numbers.
  return usage?.session === session ? (usage.usage ?? undefined) : undefined;
}

/** Whether two of the core's readings of a chat's record say the same: plain data, at most
 *  sixteen turns, so compared whole. */
function sameReading(a: ChatUsage | null, b: ChatUsage | null): boolean {
  return a === b || JSON.stringify(a) === JSON.stringify(b);
}

/** Both of the gauge's numbers, where no pane's model has said otherwise. */
const WHOLE_GAUGE: readonly FootPart[] = ["ctx", "cache"];

/** **The gauge itself, on the pane foot** (#5202, look A), or nothing: `ctx` with its fill as a
 *  thin bar under its number, then `cache`. The separate row of bars, one per turn, is gone: one figure says what the number
 *  and the row said twice. */
export function ChatGauge({
  usage,
  parts = WHOLE_GAUGE,
  context: contextRow,
}: {
  usage: ChatUsage | undefined;
  /** What the pane's model answers for its foot (`paneHead.paneLinesOf`). */
  parts?: readonly FootPart[];
  /** The row a press on `ctx` runs (#5204, spec #5191 story 36): the chat's Activity. */
  context?: { title: string; press: () => void };
}) {
  if (usage === undefined) return null;
  const { rebuilds, cold } = usage;
  const context = parts.includes("ctx") ? usage.context : null;
  const cache = parts.includes("cache") ? usage.cache : null;
  if (context === null && cache === null) return null;
  // **The whole story in words**, whatever the line has room for: a costly rebuild, a cold streak
  // and a cache that has dropped from a narrow foot have no colour of their own here.
  const said = [
    usage.context && `context window ${usage.context.value}% full`,
    usage.cache &&
      `${usage.cache.value}% of the last turn's input served from cache${
        usage.cache.tone === "ok" ? "" : ", which is low"
      }`,
    rebuilds &&
      `${rebuilds.count} prompt-cache rebuilds${
        rebuilds.tone === "ok" ? "" : ", which is costly"
      }, ${rebuilds.cost} tokens`,
    cold != null &&
      `the cache has been cold ${cold} turns running: something keeps changing the prompt's prefix — a model or effort switch, an MCP server toggling, a /compact. /rewind keeps it; /compact rebuilds it`,
  ]
    .filter(Boolean)
    .join("; ");
  const contextDrawn = context && (
    <PaneFootGauge
      label="ctx"
      value={`${context.value}%`}
      fill={{ percent: context.value, tone: context.tone }}
    />
  );
  return (
    // No live region: a screen reader announcing every turn's percentage in every pane would
    // be noise. The words are in the text and the full sentence is its tooltip.
    <span className="chat-gauge" data-testid="chat-gauge">
      {context &&
        (contextRow === undefined ? (
          <Tooltip tip={said}>
            <span>{contextDrawn}</span>
          </Tooltip>
        ) : (
          // **A shortcut to the chat's Activity** (#5204), which shows what fills the context.
          <Button
            inline
            variant="ghost"
            className="pane-foot-press"
            tooltip={said}
            aria-label={`ctx ${context.value}%. ${contextRow.title}`}
            onPress={contextRow.press}
          >
            {contextDrawn}
          </Button>
        ))}
      {cache && (
        <Tooltip tip={said}>
          <span>
            <PaneFootGauge label="cache" value={`${cache.value}%`} tone={cache.tone} />
          </span>
        </Tooltip>
      )}
    </span>
  );
}
