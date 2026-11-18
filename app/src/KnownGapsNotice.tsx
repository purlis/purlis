// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type KnownGapShown, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { askSettingsLink } from "./settings/links";
import { KNOWN_GAPS } from "./settings/KnownGaps";

/** What the Notice says of `gaps`: how many the chat started with, and that it started anyway. */
export function knownGapsSay(gaps: readonly KnownGapShown[]): string {
  const count = gaps.length === 1 ? "1 known gap" : `${gaps.length} known gaps`;
  return `This chat started with ${count}: ${gaps.map((gap) => gap.id).join(", ")}. Each is a rule in force that its harness can't enforce here.`;
}

/**
 * **The known gaps a chat started with, on its pane's head** (M75-37, #7046; **proposed**, review
 * round 1, S5): the ids its start event recorded (`run.started`'s protection), said as the core
 * says them now. Read by its own command, never the Chat log's, which waits for its tab (R-225). A standing line: the chat started anyway (Q319 C1), so it is `news`, and the way
 * out is Settings' Known gaps, where each is told in full. Dismiss puts it away until the gaps
 * change.
 */
export function KnownGapsNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const [gaps, setGaps] = useState<readonly KnownGapShown[]>([]);
  const [dismissed, setDismissed] = useState<string | null>(null);
  useEffect(() => {
    let gone = false;
    void commands
      .chatKnownGaps(plane, session)
      .then((named) => {
        if (!gone && named.status === "ok") setGaps(named.data ?? []);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane, session]);
  if (gaps.length === 0) return null;
  const cause = `known-gaps:${session}:${gaps.map((gap) => gap.id).join(",")}`;
  if (dismissed === cause) return null;
  return (
    <Notice
      cause={cause}
      at="pane"
      label="Known gaps"
      link={{
        label: "Show known gaps",
        onPress: () => askSettingsLink(plane, { group: "project.sandbox", setting: KNOWN_GAPS }),
      }}
      onDismiss={() => setDismissed(cause)}
    >
      {knownGapsSay(gaps)}
    </Notice>
  );
}
