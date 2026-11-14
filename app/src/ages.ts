// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { counted } from "./dispatches";

/**
 * **An age a person reads**, `seconds` before `now`: "just now", then minutes, hours and days
 * ago, and from a month the day itself. A moment after now (a clock that moved) is just now.
 * What a skill source's last check (#3840) and a host's add (#6172) each say their when with.
 */
export function agoSaid(seconds: number, now: Date): string {
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${counted(Math.floor(seconds / 60), "minute", "minutes")} ago`;
  if (seconds < 86_400) return `${counted(Math.floor(seconds / 3600), "hour", "hours")} ago`;
  const days = Math.floor(seconds / 86_400);
  if (days < 30) return `${counted(days, "day", "days")} ago`;
  return daySaid(new Date(now.getTime() - seconds * 1000));
}

/** **A day a person reads**, as every age past a month is said: "28 Oct 2026". */
export const daySaid = (at: Date): string =>
  at.toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });
