// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState } from "react";
import type { PoolLimitHitRow, PoolLimitOffer } from "./bindings";
import { checkLimit } from "./pools";
import { Button } from "./ui/button/Button";
import { TextField } from "./ui/field/Field";

/** What the core's check said of each box: the value, or why not. */
type Checked = Readonly<Record<string, { value: number } | { why: string }>>;

/** Why a value the limit can be still would not do: what was needed doesn't fit in it. */
const tooSmall = (hit: PoolLimitHitRow, value: number) =>
  value < hit.needed
    ? `It needs ${hit.needed} ${hit.unit}, so this would be refused again.`
    : undefined;

/**
 * **Raising a limit where it was hit** (spec #4275, #4279, #4285, #4313): what it needs, and each
 * limit's suggested value as a number the person can edit, checked by the core as they type.
 * `action` saves the values to the pool and goes again. A limit no value would hold offers
 * nothing, and the refusal's words say so.
 */
export function RaiseLimit({
  offer,
  action,
  applying = false,
  refused,
  onApply,
}: {
  offer: PoolLimitOffer;
  /** The press: `Save to the pool and start`. */
  action: string;
  applying?: boolean;
  /** Why the last save was refused, in the core's words, where the caller says it here. */
  refused?: string;
  /** The values typed, by limit key. */
  onApply: (values: ReadonlyArray<readonly [string, number]>) => void;
}) {
  const raisable = offer.hits.flatMap((hit) =>
    hit.suggested === null ? [] : [{ ...hit, suggested: hit.suggested }],
  );
  const [typed, setTyped] = useState<Readonly<Record<string, string>>>(() =>
    Object.fromEntries(raisable.map((hit) => [hit.key, String(hit.suggested)])),
  );
  // Each suggestion is a value until the person types over it.
  const [checked, setChecked] = useState<Checked>(() =>
    Object.fromEntries(raisable.map((hit) => [hit.key, { value: hit.suggested }])),
  );
  // The newest check of each box: an answer to an older one is dropped.
  const asked = useRef<Record<string, number>>({});
  const type = (key: string, to: string) => {
    setTyped((was) => ({ ...was, [key]: to }));
    // Until the core answers, the words typed are no value, so the press can't send an older one.
    setChecked((was) => Object.fromEntries(Object.entries(was).filter(([one]) => one !== key)));
    const mine = (asked.current[key] ?? 0) + 1;
    asked.current[key] = mine;
    void checkLimit(key, to).then((said) => {
      if (asked.current[key] !== mine) return;
      setChecked((was) => ({
        ...was,
        [key]: said.status === "ok" ? { value: said.data } : { why: said.error },
      }));
    });
  };
  // One limit no value holds means raising the other can't get it through either.
  if (raisable.length === 0 || raisable.length < offer.hits.length) return null;
  const why = (hit: PoolLimitHitRow) => {
    const one = checked[hit.key];
    if (one === undefined) return undefined;
    return "why" in one ? one.why : tooSmall(hit, one.value);
  };
  const values = raisable.flatMap((hit) => {
    const one = checked[hit.key];
    return one !== undefined && "value" in one && why(hit) === undefined
      ? [[hit.key, one.value] as const]
      : [];
  });
  return (
    <div className="raise-limit">
      {raisable.map((hit) => (
        <TextField
          key={hit.key}
          label={`${hit.plain} on pool ${offer.pool}, in ${hit.unit}`}
          help={`It needs ${hit.needed} ${hit.unit}. The limit is ${hit.in_force} ${hit.unit}.`}
          inputMode="numeric"
          columns={8}
          value={typed[hit.key] ?? ""}
          error={why(hit)}
          onChange={(to) => type(hit.key, to)}
        />
      ))}
      {refused !== undefined && (
        <p className="honest mid-turn" role="alert">
          {refused}
        </p>
      )}
      <Button
        variant="primary"
        isDisabled={values.length < raisable.length || applying}
        onPress={() => onApply(values)}
      >
        {applying ? "Saving…" : action}
      </Button>
    </div>
  );
}
