// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a file's diff is compared against** (#5405): the branch's base, or one turn of the chat
 * that worked there. Two views of one thing, side by side, so a `SegmentedControl`.
 *
 * **Which turn** (#5408): beside it, while one turn is shown, a select of that chat's turns,
 * newest first, the newest named the latest. It opens on the latest, so the operator reads what
 * just happened first, and steps back to see how the change grew.
 */
import { Segment, SegmentedControl } from "../ui/choice/Radio";
import { Select } from "../ui/select/Select";

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
      <Segment value="turn">One turn</Segment>
    </SegmentedControl>
  );
}

/** A turn's words in the picker: its number, and the newest said to be the latest. */
export function turnLabel(number: number, turns: readonly number[]): string {
  return number === turns[0] ? `Turn ${number}, the latest` : `Turn ${number}`;
}

/** The chat's turns, newest first as the core lists them, holding the one shown. */
export function TurnPicker({
  turns,
  number,
  onPick,
}: {
  turns: readonly number[];
  number: number;
  onPick: (number: number) => void;
}) {
  return (
    <Select
      aria-label="Turn"
      size="sm"
      value={String(number)}
      options={turns.map((one) => ({ value: String(one), label: turnLabel(one, turns) }))}
      onValueChange={(to) => onPick(Number(to))}
    />
  );
}
