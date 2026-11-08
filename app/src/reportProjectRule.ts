// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands } from "./bindings";

/**
 * **Whether the project `plane` lets its members send report drafts without asking** (#5696,
 * spec #5683, Q12): the core reads `[reports].always_send` in the project's shared settings
 * file, which the team sees, on unless that file turns it off. With no project, or until the
 * core has said, nothing holds it: the core still refuses an unread send the rule forbids, so
 * this only says why the row is held.
 */
export function useMembersMayAlwaysSend(plane: string | undefined): boolean {
  const [said, setSaid] = useState<{ plane: string; may: boolean }>();
  useEffect(() => {
    if (plane === undefined) return;
    let live = true;
    void commands.membersMayAlwaysSend(plane).then((answer) => {
      if (live && answer.status === "ok") setSaid({ plane, may: answer.data });
    });
    return () => {
      live = false;
    };
  }, [plane]);
  return plane === undefined || said?.plane !== plane || said.may;
}
