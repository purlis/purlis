// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Dot } from "lucide-react";
import { Button } from "./ui/button/Button";
import { UNVERIFIED_SAID, useBlockedMarker } from "./blockedMarkers";

/** The dot's size: the line's, as the row's other quiet marks are. */
const DOT = { width: "1em", height: "1em" };

/**
 * **Chat `session`'s blocked marker** (R-172, R-299, R-301; T1-30), on its sidebar row's end and
 * its pane's band: "2 new blocked", a quiet button that opens its Chat log filtered to blocks;
 * a muted dot where only unverified reports are new, which never count here. Nothing where
 * nothing is new. Drawn and never announced (R-176): no live region holds it.
 *
 * A control of the row's, as the row's other buttons are (#3198): out of the Tab order, and
 * reached by → from the row.
 */
export function BlockedMarker({ session }: { session: number }) {
  const { marker, open } = useBlockedMarker(session);
  if (marker === undefined) return null;
  if (marker.kind === "dot")
    return (
      <span
        className="ui-text-secondary"
        data-mark="blocked-unverified"
        role="img"
        aria-label={UNVERIFIED_SAID}
      >
        <Dot aria-hidden="true" style={DOT} />
      </span>
    );
  return (
    <Button variant="quiet" size="sm" data-mark="blocked" excludeFromTabOrder onPress={open}>
      {marker.said}
    </Button>
  );
}
