// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useState } from "react";
import type { InboxUpdate, PlaneId, QuietEnded, Shown } from "./bindings";
import { Notice } from "./Notice";
import {
  QUIET_NOW,
  QUIET_NOW_TIP,
  quietPartsSaid,
  quietSaid,
  quietSummaryOf,
  setQuietNow,
  useQuiet,
  type QuietSummaryOf,
} from "./quiet";
import { Button, ToggleButton } from "./ui/button/Button";

/**
 * **Quiet at the top of Asks** (#6017): the "Quiet now" toggle, pressed while the person turned it on,
 * and the summary of the latest Quiet that ended ({@link QuietSummary}).
 */
export function AsksQuiet(
  props: Omit<Parameters<typeof QuietSummary>[0], "ended"> & {
    /** Turns "Quiet now" on or off; the window's own, unless a test says otherwise. */
    onQuietNow?: (on: boolean) => void;
  },
) {
  const { onQuietNow = setQuietNow, ...summary } = props;
  const quiet = useQuiet();
  return (
    <div className="inbox-quiet">
      {/* A toggle, pressed while on: a button of its own, which the keyboard lands on and rings
          where it is drawn. */}
      <ToggleButton
        size="sm"
        variant="ghost"
        isSelected={quiet.by_hand}
        onChange={onQuietNow}
        tooltip={QUIET_NOW_TIP}
      >
        {QUIET_NOW}
      </ToggleButton>
      <QuietSummary ended={quiet.ended} {...summary} />
    </div>
  );
}

/**
 * **While you were quiet: one summary row at the top of Asks** (#6017, ADR 0121 decision 13).
 *
 * Drawn once a Quiet has ended ({@link QuietEnded}), from what it held that is still New in
 * this project: "While you were quiet: 3 asks, 2 finished, 1 failed". **Each part is a link**:
 * the asks to the first chat they came from, its row in Asks beside it; what finished or failed
 * to Updates, where it is listed. **It goes once all of it is seen** (#6002): a part seen drops
 * out, and with none left the row is not drawn. Dismiss puts it away sooner, and nothing else:
 * an ask still waits, an update still stands.
 *
 * It answers nothing. A Quiet that ends with nothing New in this project draws nothing here.
 */
export function QuietSummary({
  ended,
  plane,
  asks,
  updates,
  isNew,
  onGoToAsk,
  onShowUpdates,
}: {
  /** The latest Quiet that ended, as the core said (`QuietNow.ended`). */
  ended: QuietEnded | null | undefined;
  plane: PlaneId;
  /** The project's asks as they stand. */
  asks: readonly Shown[] | undefined;
  /** The project's updates as they stand. */
  updates: readonly InboxUpdate[] | undefined;
  /** Whether a row's seen key is New (`SeenRows.isNew`). */
  isNew: (key: string) => boolean;
  /** Brings forward the chat that asked, its row in Asks beside it. */
  onGoToAsk: (session: number) => void;
  /** Opens Updates. */
  onShowUpdates: () => void;
}) {
  /** The end of the Quiet whose summary was dismissed: a later one is drawn again. */
  const [dismissed, setDismissed] = useState<number>();
  const summary = quietSummaryOf(ended, plane, asks, updates, isNew);
  const parts = quietPartsSaid(summary);
  if (ended === null || ended === undefined || dismissed === ended.to || parts.length === 0)
    return null;
  const go = (part: keyof QuietSummaryOf) => {
    const first = summary.asks[0];
    if (part === "asks" && first !== undefined) onGoToAsk(first.session);
    else if (part !== "asks") onShowUpdates();
  };
  return (
    <Notice
      cause={`quiet-summary:${ended.to}`}
      label={quietSaid(summary)}
      onDismiss={() => setDismissed(ended.to)}
    >
      While you were quiet:{" "}
      {parts.map(({ part, says }, at) => (
        <Fragment key={part}>
          {at > 0 && ", "}
          <Button
            variant="link"
            aria-label={part === "asks" ? `${says}: go to them` : `${says}: open Updates`}
            onPress={() => go(part)}
            className="away-part"
          >
            {says}
          </Button>
        </Fragment>
      ))}
    </Notice>
  );
}
