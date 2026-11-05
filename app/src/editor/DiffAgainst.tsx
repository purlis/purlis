// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a file's diff is compared against** (#5405): the branch's base, or the latest turn that
 * ended on it. Two views of one thing, side by side, so a `SegmentedControl`.
 */
import { Segment, SegmentedControl } from "../ui/choice/Radio";

export type Against = "base" | "turn";

export function DiffAgainst({
  value,
  onChange,
}: {
  value: Against;
  onChange: (next: Against) => void;
}) {
  return (
    <SegmentedControl
      aria-label="Compare against"
      size="sm"
      value={value}
      onChange={(next) => onChange(next === "turn" ? "turn" : "base")}
    >
      <Segment value="base">Against base</Segment>
      <Segment value="turn">Latest turn</Segment>
    </SegmentedControl>
  );
}
