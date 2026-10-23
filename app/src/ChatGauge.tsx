// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { useOnShownAgain, useProjectShown } from "./projectShown";
import { commands, type ChatUsage, type GaugeTone, type PlaneId, type UsageTurn } from "./bindings";
import { everyWhileShown } from "./whileShown";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **A chat's `ctx`/`cache` gauge, in its own pane's corner.**
 *
 * charter ADR 0019 wrote the loss down: *"A framed Claude Code session has no context/cache
 * gauge on any surface."* charter's statusline IS Claude Code's `statusLine`, so running Claude
 * Code inside charter cost the operator the context percentage he would have seen outside it.
 * ADR 0038 named it again for the app and did not rule where it goes; this is that ruling.
 *
 * # Why the pane's corner, and not the tab or a panel
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
 * The core's gauge, whole: `ctx NN%`, `cache NN%`, and `↻N cost` when the prefix has been
 * rebuilt — each in the tone the core's threshold gives it, so this and the frame's panel can
 * never colour one number two ways. **Nothing at all when nothing is known**, never `ctx 0%`.
 *
 * # And the trend beside it
 *
 * ADR 0038 names "usage and the token trend" as its own gap: *"the trend over a session's
 * turns rather than this turn's percentage"*. It lives here and not in a panel of its own,
 * because it is the same conversation's history — a trend window would be a second place to
 * look for one chat's numbers. It is drawn as one bar per recorded turn (at most sixteen, the
 * record's ring), each as tall as that turn's cache share and in its tone, so a rebuild reads
 * as the dip it is. The numbers behind every bar are on the bar's title. And once the cache has
 * been cold three turns running — `_cache_hint`'s rule, which Python only ever said in the
 * live footer — it says so in words: `cold 3`.
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
          setUsage({ session, usage: gauge });
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

const TONE: Record<GaugeTone, string> = { ok: "gauge-ok", warn: "gauge-warn", bad: "gauge-bad" };

/** The gauge itself, or nothing. */
export function ChatGauge({ usage }: { usage: ChatUsage | undefined }) {
  if (usage === undefined) return null;
  const { context, cache, rebuilds, turns, cold } = usage;
  const said = [
    context && `context window ${context.value}% full`,
    cache && `${cache.value}% of the last turn's input served from cache`,
    rebuilds && `${rebuilds.count} prompt-cache rebuilds, ${rebuilds.cost} tokens`,
  ]
    .filter(Boolean)
    .join("; ");
  return (
    // No live region: a screen reader announcing every turn's percentage in every pane would
    // be noise. The words are in the text and the full sentence is its tooltip.
    <Tooltip tip={said}>
      <div className="chat-gauge" data-testid="chat-gauge">
        {context && (
          <span className="gauge-part">
            <span className="gauge-label">ctx</span>{" "}
            <span className={TONE[context.tone]}>{context.value}%</span>
          </span>
        )}
        {cache && (
          <span className="gauge-part">
            <span className="gauge-label">cache</span>{" "}
            <span className={TONE[cache.tone]}>{cache.value}%</span>
          </span>
        )}
        {rebuilds && (
          <span className={`gauge-part ${TONE[rebuilds.tone]}`}>
            ↻{rebuilds.count} {rebuilds.cost}
          </span>
        )}
        {cold != null && (
          <Tooltip
            tip={`The cache has been cold ${cold} turns running: something keeps changing the prompt's prefix — a model or effort switch, an MCP server toggling, a /compact. /rewind keeps it; /compact rebuilds it.`}
          >
            <span className="gauge-part gauge-warn">cold {cold}</span>
          </Tooltip>
        )}
        {turns.length > 1 && <Trend turns={turns} />}
      </div>
    </Tooltip>
  );
}

/** What one turn's bar says, a line of the trend's tooltip. */
function turnSaid(turn: UsageTurn, index: number): string {
  return [
    `turn ${index + 1}`,
    turn.cache ? `cache ${turn.cache.value}%` : "cache unknown",
    turn.context ? `ctx ${turn.context.value}%` : undefined,
    turn.written ? `wrote ${turn.written}` : undefined,
  ]
    .filter(Boolean)
    .join(", ");
}

/**
 * The trend: one bar per recorded turn, oldest on the left, as tall as its cache share.
 *
 * An SVG with a bar per turn rather than a charting library: sixteen rectangles is not a chart
 * a library earns its weight on, and the colour comes from the theme through each bar's class,
 * never from a value written here. A turn whose share could not be read draws a stub in the
 * muted colour — a gap in the record, not a zero.
 */
export function Trend({ turns }: { turns: readonly UsageTurn[] }) {
  const width = turns.length * 3;
  return (
    // Every turn's line in the trend's one tooltip, oldest first: a mark the gauge's words
    // already say, so the pointer's.
    <Tooltip tip={turns.map(turnSaid).join("\n")}>
      <svg
        className="gauge-trend"
        data-testid="gauge-trend"
        viewBox={`0 0 ${width} 10`}
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        {turns.map((turn, i) => {
          const height = turn.cache ? Math.max(1, Math.round(turn.cache.value / 10)) : 1;
          return (
            <rect
              key={i}
              x={i * 3}
              y={10 - height}
              width={2}
              height={height}
              className={turn.cache ? TONE[turn.cache.tone] : "gauge-unknown"}
            />
          );
        })}
      </svg>
    </Tooltip>
  );
}
