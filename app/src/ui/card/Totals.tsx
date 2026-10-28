// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A row of totals tiles** (#3673, the card family): the few numbers a page is about, side by
 * side: the grade plan's chats, its most, its estimate and where it runs; a running grade's
 * chats done, what it has spent and its tokens; a scorecard's with it, without it, its previous
 * version and what it spent. A screen draws its totals with these and never its own tile.
 *
 * - **A tile** (the `tile` part) holds its label (the label role, in capitals), its value in
 *   figures of one width at a larger size (`tile-value`), and an optional line under it in the
 *   quiet tier ("of at most $3.00").
 * - **The tiles share the row's width**, each at least about 170px, and wrap to more rows where
 *   they do not fit, never sideways.
 * - **A tile whose value changes keeps its size** (a running grade's counts): its value keeps one
 *   line in figures of one width, so the next count draws where the last one was.
 * - **`live` totals say their changes politely** (`role="status"`), at most once a second: what
 *   each tile now says, in one sentence, so a reader hears the counts without being flooded.
 *
 * Drawn by the card family's parts (`card.css`), which a theme reaches (`card.tile`,
 * `card.tile-value`).
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { VisuallyHidden } from "react-aria";
import { useSaidAtMost } from "../said";

/** What a live row hands its tiles: where each says what it shows now. */
const Live = createContext<((id: string, said: string | null) => void) | null>(null);

/** **A row of tiles**: `Tile`s, as many to a row as fit. `live` says their changes politely. */
export function Totals({
  children,
  live = false,
  className,
  "aria-label": label,
}: {
  children: ReactNode;
  /** The values change while the page is open (a running grade): said politely as they do. */
  live?: boolean;
  /** A place for it in its view: a margin. Never a look. */
  className?: string;
  "aria-label"?: string;
}) {
  const [said, setSaid] = useState<ReadonlyArray<readonly [string, string]>>([]);
  const report = useCallback((id: string, words: string | null) => {
    setSaid((now) => {
      const others = now.filter(([one]) => one !== id);
      const at = now.findIndex(([one]) => one === id);
      if (words === null) return others;
      if (at >= 0 && now[at][1] === words) return now;
      const next = [...now];
      if (at >= 0) next[at] = [id, words];
      else next.push([id, words]);
      return next;
    });
  }, []);
  const sentence = said.map(([, words]) => words).join(", ");
  const spoken = useSaidAtMost(live ? sentence : "");
  return (
    <div data-scope="card" data-part="totals" className={className} role="group" aria-label={label}>
      <Live.Provider value={live ? report : null}>{children}</Live.Provider>
      {live && (
        <VisuallyHidden>
          <span role="status">{spoken}</span>
        </VisuallyHidden>
      )}
    </div>
  );
}

/** Words for a reader from what a tile shows: its text, or what `said` says it is. */
const wordsOf = (node: ReactNode): string | null =>
  typeof node === "string" || typeof node === "number" ? String(node) : null;

/**
 * **One tile**: its label, its value and an optional line under it. `said` is how a live row
 * speaks it where its label and value are not words alone ("Chats 3 of 12").
 */
export function Tile({
  label,
  value,
  line,
  said,
}: {
  label: ReactNode;
  value: ReactNode;
  line?: ReactNode;
  said?: string;
}) {
  const id = useId();
  const report = useContext(Live);
  const words = useMemo(() => {
    if (said !== undefined) return said;
    const named = wordsOf(label);
    const shown = wordsOf(value);
    return named === null || shown === null ? null : `${named} ${shown}`;
  }, [said, label, value]);
  useEffect(() => {
    if (report === null) return;
    report(id, words);
    return () => report(id, null);
  }, [report, id, words]);
  return (
    <div data-scope="card" data-part="tile">
      <span data-scope="card" data-part="tile-label">
        {label}
      </span>
      <span data-scope="card" data-part="tile-value">
        {value}
      </span>
      {line !== undefined && (
        <span data-scope="card" data-part="tile-line">
          {line}
        </span>
      )}
    </div>
  );
}
