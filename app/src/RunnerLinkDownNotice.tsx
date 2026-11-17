// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { commands, type ChatsReconnecting, type PlaneId, type RunnerLinkDown } from "./bindings";
import { listen } from "./here";
import { Notice } from "./Notice";
import { RECONNECTING } from "./reconnecting";
import { counted } from "./shownState";

/**
 * How long a runner's link may be down before the window says why (#7134, ADR 0078 as amended
 * 2026-11-18). Until then its chats' panes say Reconnecting… and nothing more, since most drops
 * come back within a few tries.
 */
export const QUIET_FOR_MS = 30_000;

/** How often a link that is down is read again, and its time said again: the core says a link
 *  is back on `chats-reconnecting` only where it has chats to say it of. */
const SAID_AGAIN_MS = 15_000;

/**
 * **A pool's runner whose link stays down** (#7134, ADR 0078 as amended 2026-11-18): the Notice
 * the window shows, at its own level and not on each chat's pane, once the link has been down
 * for longer than {@link QUIET_FOR_MS}.
 *
 * This machine's session host keeps the link to a pool's runner and links again on its own,
 * trying again at most 30 seconds apart. It tells the core why the link is down, again each time
 * a try fails for another reason, and the core keeps the last reason for each pool
 * (`runner_links_down`). Its chats' panes say Reconnecting… quietly; past the quiet while this
 * says, once for each pool, for how long and why, in the host's own words, that purlis keeps
 * linking to it again, and offers **Try now**, which tries at once rather than after the pause,
 * **Check pool**, which opens the pool's setup, and **Dismiss**, which puts it away until the
 * reason changes. What a try finds is said as ever: the Notice says the new reason where one
 * fails for another, and goes once the link is back.
 */
export function RunnerLinkDownNotice({
  plane,
  onCheckPool,
}: {
  plane: PlaneId;
  /** Opens pool `pool`'s setup in Settings, where its runner is checked. */
  onCheckPool: (pool: string) => void;
}) {
  const { shown, now, dismiss } = useLinksDown(plane);
  return shown.map((down) => (
    <Notice
      key={down.pool}
      cause={`runner-link-down:${plane}:${down.pool}`}
      tone="trouble"
      label={`Pool ${down.pool}'s runner is not linked`}
      fixes={[{ label: "Try now", onPress: () => void commands.runnerLinkNow(plane, down.pool) }]}
      link={{ label: `Check pool ${down.pool}`, onPress: () => onCheckPool(down.pool) }}
      onDismiss={() => dismiss(down)}
    >
      {downSaid(down, now)}
    </Notice>
  ));
}

/** What the Notice says of `down`, at `now`. */
export function downSaid(down: RunnerLinkDown, now: number): string {
  const why = down.why.endsWith(".") ? down.why : `${down.why}.`;
  return (
    `Its chats have waited ${forHowLong(now - down.since * 1000)}: ${why} ` +
    "purlis keeps linking to it again, trying at most 30 seconds apart."
  );
}

/** A while, in the largest whole unit: 45 seconds, 1 minute, 6 minutes, 2 hours. */
function forHowLong(ms: number): string {
  const secs = Math.max(0, Math.floor(ms / 1000));
  const [count, unit] =
    secs < 60
      ? [secs, "second"]
      : secs < 3600
        ? [Math.floor(secs / 60), "minute"]
        : [Math.floor(secs / 3600), "hour"];
  return counted(count, unit) ?? `0 ${unit}s`;
}

/** A Notice's key for Dismiss: put away until the reason changes. */
const keyOf = (down: RunnerLinkDown) => `${down.pool}\n${down.why}`;

/** What the core last said of `plane`'s runners' links, kept with the plane it was said of. */
type Said = { plane: PlaneId; down: readonly RunnerLinkDown[] };

/**
 * Which of `plane`'s runners' links are shown as down: down for longer than the quiet while, and
 * not put away. Read as it mounts and again each time the core says `plane`'s chats moved.
 */
function useLinksDown(plane: PlaneId): {
  shown: readonly RunnerLinkDown[];
  /** When it was last looked at: what the time each link has been down is said from. */
  now: number;
  dismiss: (down: RunnerLinkDown) => void;
} {
  const [said, setSaid] = useState<Said | null>(null);
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  const [now, setNow] = useState(() => Date.now());
  // The read of the plane now mounted, for the tick to call.
  const readAgain = useRef<() => void>(() => {});
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    const read = async () => {
      try {
        const answer = await commands.runnerLinksDown(plane);
        // Only a list is taken: a core that answers nothing says no link is down.
        if (!gone && answer.status === "ok" && Array.isArray(answer.data)) {
          const down = answer.data;
          setSaid({ plane, down });
          setNow(Date.now());
          // A Notice put away stays away only while its outage lasts: the next one is said.
          const still = new Set(down.map(keyOf));
          setDismissed((was) =>
            [...was].every((key) => still.has(key))
              ? was
              : new Set([...was].filter((key) => still.has(key))),
          );
        }
      } catch {
        // What cannot be read shows nothing.
      }
    };
    void (async () => {
      try {
        const unlisten = await listen<ChatsReconnecting>(RECONNECTING, (event) => {
          if (event.payload.plane === plane) void read();
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
      await read();
    })();
    readAgain.current = () => void read();
    return () => {
      gone = true;
      stop?.();
      readAgain.current = () => {};
    };
  }, [plane]);
  const down = said?.plane === plane ? said.down : [];
  // While a link is down, the time is said again now and then, and a link's quiet while ends.
  const waiting = down.length > 0;
  useEffect(() => {
    if (!waiting) return;
    const tick = setInterval(() => readAgain.current(), SAID_AGAIN_MS);
    return () => clearInterval(tick);
  }, [waiting]);
  const quietUntil = down
    .map((one) => one.since * 1000 + QUIET_FOR_MS)
    .filter((until) => until > now)
    .reduce((first, until) => Math.min(first, until), Infinity);
  useEffect(() => {
    if (quietUntil === Infinity) return;
    const ends = setTimeout(() => setNow(Date.now()), quietUntil - Date.now() + 1);
    return () => clearTimeout(ends);
  }, [quietUntil]);
  const shown = down.filter(
    (one) => now - one.since * 1000 >= QUIET_FOR_MS && !dismissed.has(keyOf(one)),
  );
  const dismiss = (one: RunnerLinkDown) => setDismissed((was) => new Set(was).add(keyOf(one)));
  return { shown, now, dismiss };
}
