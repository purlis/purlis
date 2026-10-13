// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";

/** Panes started and stopped, counted by the stand-in below. */
export const panes = { started: 0, stopped: 0 };

/**
 * A pane that only says it was started and stopped. The real one starts an xterm.js terminal,
 * which jsdom cannot draw; what a switch costs there is one terminal built and one
 * `watch_session` per start, and one terminal disposed and one `unwatch_session` per stop.
 */
export function CountedPane({ session }: { session: number }) {
  useEffect(() => {
    panes.started += 1;
    return () => {
      panes.stopped += 1;
    };
  }, []);
  return <div data-testid="pane">session {session}</div>;
}
