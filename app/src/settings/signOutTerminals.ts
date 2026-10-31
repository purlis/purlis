// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef } from "react";
import { askProfiles } from "../pools";
import { panesOf, type Tabs } from "../tabs";

/**
 * **A terminal sign-out's line is asked again when its tab closes** (#4274): opencode asks which
 * sign-in goes in a shell tab, so only the runner can say what is left, and purlis does not read
 * that terminal. Answers what remembers a tab's session and the pool it signs out on.
 */
export function useSignOutTerminals(tabs: Tabs, plane: string) {
  const open = useRef(new Map<number, string>());
  useEffect(() => {
    const shown = new Set(
      tabs.order.flatMap((id) => panesOf(tabs, id).map((pane) => pane.session)),
    );
    for (const [session, pool] of open.current) {
      if (shown.has(session)) continue;
      open.current.delete(session);
      void askProfiles(pool, plane, { quietly: true });
    }
  }, [tabs, plane]);
  return useCallback((session: number, pool: string) => {
    open.current.set(session, pool);
  }, []);
}
