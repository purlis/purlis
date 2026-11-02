// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { commands, type PoolLimitHitRow, type PoolLimitOffer } from "./bindings";
import { checkLimit, saveLimits } from "./pools";
import { Button } from "./ui/button/Button";
import { TextField } from "./ui/field/Field";

/** What the core's check said of each box: the value, or why not. */
type Checked = Readonly<Record<string, { value: number } | { why: string }>>;

/** Whether a limit is a wait: what ran out is all that is known, so nothing more is "needed". */
const isATime = (unit: string): boolean => unit === "minutes" || unit === "seconds";

/** Why a value the limit can be still would not do: what was needed doesn't fit in it. */
const tooSmall = (hit: PoolLimitHitRow, value: number) =>
  !isATime(hit.unit) && value < hit.needed
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
  working,
  refused,
  onApply,
}: {
  offer: PoolLimitOffer;
  /** The press: `Save to the pool and start`. */
  action: string;
  /** What it says while its press works: `Saving…`. The press keeps the keyboard, doing nothing. */
  working?: string;
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
    void checkLimit(offer.pool, key, to).then((said) => {
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
          help={`${
            isATime(hit.unit)
              ? `It took longer than ${hit.in_force} ${hit.unit}.`
              : `It needs ${hit.needed} ${hit.unit}. The limit is ${hit.in_force} ${hit.unit}.`
          }${hit.team_suggests ? ` The team suggests ${hit.suggested} ${hit.unit}.` : ""}`}
          inputMode="numeric"
          columns={8}
          value={typed[hit.key] ?? ""}
          error={why(hit)}
          isReadOnly={working !== undefined}
          onChange={(to) => type(hit.key, to)}
        />
      ))}
      {refused !== undefined && (
        <p className="raise-limit-refused" role="alert">
          {refused}
        </p>
      )}
      <Button
        variant="primary"
        isDisabled={values.length < raisable.length && working === undefined}
        aria-disabled={working !== undefined}
        onPress={working === undefined ? () => onApply(values) : undefined}
      >
        {working ?? action}
      </Button>
    </div>
  );
}

/**
 * **The form where a refusal on pool `pool` said a limit was hit** (#4397): asked of the core,
 * never read from the words, so a refusal that hit no limit draws nothing. A value saved goes to the
 * pool, then `onRaised` goes again. Used where a person meets a limit with no flow of its own, as
 * Start a chat and a move have. Given a `key` of the refusal, so another refusal starts over.
 */
export function OfferToRaise({
  pool,
  refused,
  action,
  onRaised,
}: {
  pool: string;
  /** The refusal, in the words it was said in. */
  refused: string;
  /** The press: `Save to the pool and ask again`. */
  action: string;
  onRaised: () => void;
}) {
  const [offer, setOffer] = useState<PoolLimitOffer | null>(null);
  const [saving, setSaving] = useState(false);
  const [why, setWhy] = useState<string>();
  useEffect(() => {
    let gone = false;
    void commands
      .poolLimitOfferForMove(pool, refused)
      .catch(() => null)
      .then((said) => {
        if (!gone) setOffer(said);
      });
    return () => {
      gone = true;
    };
  }, [pool, refused]);
  if (offer === null) return null;
  const apply = async (values: ReadonlyArray<readonly [string, number]>) => {
    setSaving(true);
    setWhy(undefined);
    const saved = await saveLimits(offer.pool, values);
    setSaving(false);
    if (saved === undefined) onRaised();
    else setWhy(saved);
  };
  return (
    <RaiseLimit
      key={offer.hits.map((hit) => `${hit.key}:${hit.needed}`).join()}
      offer={offer}
      action={action}
      working={saving ? "Saving…" : undefined}
      refused={why}
      onApply={(values) => void apply(values)}
    />
  );
}
