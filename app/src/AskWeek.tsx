// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type AskMeasure } from "./bindings";

/**
 * **Each project's week of asking** (ADR 0121 decision 18, #6021): a quiet line per project at
 * the head of All chats, saying how many asks the person answered in Asks in the last seven
 * days, the median time from an ask being raised to its answer, and how many asks its rules saved.
 *
 * It only tells: no control, no count on any icon and no notification. A project with nothing to
 * tell has no line, and with none at all there is nothing here.
 */
export function AskWeek({
  measures,
  projectOf,
}: {
  measures: readonly AskMeasure[];
  /** A project's name, by its plane, as the strip names it. */
  projectOf: (plane: string) => string;
}) {
  const told = measures
    .filter((one) => one.asks > 0 || one.saved > 0)
    .map((one) => ({ plane: one.plane, project: projectOf(one.plane), said: weekSaid(one) }))
    .sort((one, two) => one.project.localeCompare(two.project));
  if (told.length === 0) return null;
  return (
    <div className="all-chats-week">
      {told.map((one) => (
        // A project's name is data: drawn as text.
        <p key={one.plane} className="ui-text-meta">
          <span className="all-chats-week-project">{one.project}</span>
          {` · ${one.said}`}
        </p>
      ))}
    </div>
  );
}

/** `12 asks this week · answered in 2 min at the median · 3 saved by rules`. */
export function weekSaid(week: AskMeasure): string {
  const parts = [`${week.asks === 1 ? "1 ask" : `${week.asks} asks`} this week`];
  if (week.median_secs !== null)
    parts.push(`answered in ${waitSaid(week.median_secs)} at the median`);
  if (week.saved > 0) parts.push(`${week.saved} saved by rules`);
  return parts.join(" · ");
}

/** `under a minute`, `4 min`, `2 h 5 min`. */
export function waitSaid(seconds: number): string {
  if (seconds < 60) return "under a minute";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const left = minutes % 60;
  return left === 0 ? `${hours} h` : `${hours} h ${left} min`;
}

/** **The week's measures, read once as All chats opens** (#6021): none until they come, and
 *  none where they cannot be read, which is no reason to say anything. */
export function useAskWeek(): readonly AskMeasure[] {
  const [measures, setMeasures] = useState<readonly AskMeasure[]>([]);
  useEffect(() => {
    let live = true;
    void commands
      .askMeasures()
      .then((read) => {
        // A stand-in core that answers nothing is read as no week to tell.
        if (live && read.status === "ok" && Array.isArray(read.data)) setMeasures(read.data);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);
  return measures;
}
