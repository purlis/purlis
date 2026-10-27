// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { TimelineMoment } from "./bindings";
import type { ViewRef } from "./tabs";

/**
 * **A chat's browser timeline, in a view tab** (#2017, spec #2008).
 *
 * What the chat's agent did in its browsers, oldest first, with the screenshots its tools
 * returned, and where the person or purlis drove instead. The app keeps it on this device,
 * bounded, and removes it when the chat closes (`purlis_core::browsertimeline`); the view
 * `browser-timeline` is keyed by the chat's id, which it keeps across a launch.
 */

/** The view a chat's browser timeline is drawn in. */
export const BROWSER_TIMELINE_VIEW = "browser-timeline";

/** The event a chat's browser timeline is said to have moved on. */
export const TIMELINE_MOVED = "browser-timeline-moved";

/** The view tab of the browser timeline of the chat whose id is `chat`. */
export function browserTimelineView(chat: string): ViewRef {
  return { from: null, view: BROWSER_TIMELINE_VIEW, key: chat };
}

/** Whether `view` is a chat's browser timeline. */
export function isBrowserTimeline(view: ViewRef): boolean {
  return view.from === null && view.view === BROWSER_TIMELINE_VIEW && view.key !== "";
}

/** What the tab of chat `chat`'s browser timeline is called. */
export function browserTimelineTitle(chat: string): string {
  return `Browser timeline of ${chat}`;
}

/** Why a moment shows no screenshot it had, or nothing for one that had none or keeps it. */
export function unkeptSaid(moment: TimelineMoment): string | null {
  switch (moment.unkept) {
    case "bound":
      return "Its screenshot was let go to keep the timeline small.";
    case "too_large":
      return "Its screenshot was too large to keep.";
    case "driven":
      return "No screenshot kept: you or purlis were driving the browser then.";
    case null:
      return null;
  }
}

/** When a moment was, as its row says it: the time of day, to the second. `at` is in seconds
 *  since 1970. */
export function atSaid(at: number): string {
  return new Date(at * 1000).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
