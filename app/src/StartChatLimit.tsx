// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PoolLimitOffer } from "./bindings";
import { Button } from "./ui/button/Button";
import { TextField } from "./ui/field/Field";

/** What a typed number is, or why it can't be one. */
const wholeNumber = (typed: string): string | undefined =>
  /^[1-9][0-9]*$/.test(typed.trim()) ? undefined : "Type a whole number above zero.";

/**
 * **A start refused by a pool's limit, offering to raise it** (spec #4275): what it needs, and
 * each limit's suggested value as a number the person can edit. Applying saves the values to the
 * pool and starts again. A limit no value would hold offers nothing, and the refusal's words say
 * so. A plain form: the final look comes with the limits page (#4285).
 */
export function StartChatLimit({
  offer,
  applying = false,
  onApply,
}: {
  offer: PoolLimitOffer;
  applying?: boolean;
  /** The values typed, by limit key. */
  onApply: (values: ReadonlyArray<readonly [string, number]>) => void;
}) {
  const raisable = offer.hits.flatMap((hit) =>
    hit.suggested === null ? [] : [{ ...hit, suggested: hit.suggested }],
  );
  const [typed, setTyped] = useState<Readonly<Record<string, string>>>(() =>
    Object.fromEntries(raisable.map((hit) => [hit.key, String(hit.suggested)])),
  );
  // One limit no value holds means raising the other can't start the chat either.
  if (raisable.length === 0 || raisable.length < offer.hits.length) return null;
  const ready = raisable.every((hit) => wholeNumber(typed[hit.key] ?? "") === undefined);
  return (
    <div className="start-chat-limit">
      {raisable.map((hit) => (
        <TextField
          key={hit.key}
          label={`${hit.plain} on pool ${offer.pool}, in ${hit.unit}`}
          help={`It needs ${hit.needed}. The limit is ${hit.in_force}.`}
          value={typed[hit.key] ?? ""}
          validate={wholeNumber}
          onChange={(value) => setTyped((was) => ({ ...was, [hit.key]: value }))}
        />
      ))}
      <Button
        isDisabled={!ready || applying}
        onPress={() =>
          onApply(raisable.map((hit) => [hit.key, Number(typed[hit.key]?.trim())] as const))
        }
      >
        {applying ? "Starting…" : "Save to the pool and start"}
      </Button>
    </div>
  );
}
