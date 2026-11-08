// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands } from "./bindings";

/**
 * What the project's rule says of sending report drafts without asking: it `allows` it, it
 * `forbids` it, or it is `unread`, the core having refused or failed to read it.
 */
export type ProjectRule = "allows" | "forbids" | "unread";

/**
 * **Whether the project `plane` lets its members send report drafts without asking** (#5696,
 * spec #5683, Q12): the core reads `[reports].always_send` in the project's shared settings
 * file, which the team sees, on unless that file turns it off. With no project, or until the
 * core has said, nothing holds it: the core still refuses an unread send the rule forbids, so
 * this only says why the row is held. A rule the core could not read is `unread`, never a yes:
 * the window offers no send it cannot say the project allows.
 */
export function useMembersMayAlwaysSend(plane: string | undefined): ProjectRule {
  const [said, setSaid] = useState<{ plane: string; rule: ProjectRule }>();
  useEffect(() => {
    if (plane === undefined) return;
    let live = true;
    const say = (rule: ProjectRule) => {
      if (live) setSaid({ plane, rule });
    };
    commands.membersMayAlwaysSend(plane).then(
      (answer) => say(answer.status === "ok" ? (answer.data ? "allows" : "forbids") : "unread"),
      () => say("unread"),
    );
    return () => {
      live = false;
    };
  }, [plane]);
  return plane === undefined || said?.plane !== plane ? "allows" : said.rule;
}
