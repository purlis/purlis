// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, useState } from "react";
import type { PoolLimitGroup, PoolLimitRow } from "../bindings";
import { askLimits, checkLimit, saveLimits, usePools } from "../pools";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Disclosure } from "../ui/disclosure/Disclosure";
import { TextField } from "../ui/field/Field";
import { SettingRow } from "./components";

/** What the page says of limits, over its groups (A1, Q11). */
const ABOUT =
  "Every default is already the best value, so most people never change these. A limit left empty follows purlis's default as it improves. One you set stays, and is marked.";

/**
 * **A pool's limits** (#4285, spec #4275, the person's choice of variant B): folded under the
 * pool's row, grouped by what they affect, with the uncommon ones under Advanced.
 *
 * **It draws whatever the core's list holds** (`purlis_core::poollimits`), and names no limit
 * here: a limit added there reaches this page as it is, and a group with none isn't drawn.
 *
 * **Each limit** ({@link LimitRow}) shows its plain name, its unit, its default, a mark on a value
 * the person set, and Reset. Its box is empty while it follows purlis's default, which it shows
 * greyed. What is typed is checked by the core as it is typed, and saved on leaving the box or on
 * Enter, never on the window losing the focus. A refused value stays in the box, unsaved, with why
 * under it, and Reset clears it. An emptied box is a reset, and Escape puts back what is held.
 *
 * **One row's saves run in order** ({@link LimitRow}), so a Reset pressed just after typing, whose
 * press leaves the box and so saves what was typed, is what holds at the end.
 */
export function PoolLimits({ pool }: { pool: string }) {
  const read = usePools().limits[pool];
  useEffect(() => {
    void askLimits(pool);
  }, [pool]);
  const shown = read !== undefined && "groups" in read ? read : undefined;
  const set = shown
    ? [...shown.groups.flatMap((group) => group.limits), ...shown.advanced].filter(
        (row) => row.set !== null,
      ).length
    : 0;
  return (
    <Disclosure
      className="pool-limits"
      label={`Limits of pool ${pool}`}
      level={3}
      trail={set > 0 ? <Badge variant="accent">{`${set} set by you`}</Badge> : undefined}
      // Saved elsewhere meanwhile, say from a refused start in another window.
      onExpandedChange={(open) => {
        if (open) void askLimits(pool);
      }}
    >
      {read === undefined && <p className="ui-setting-help">Reading…</p>}
      {read !== undefined && "trouble" in read && (
        <>
          <p className="ui-setting-help" role="alert">
            {read.trouble}
          </p>
          <Button size="sm" onPress={() => void askLimits(pool)}>
            Read again
          </Button>
        </>
      )}
      {shown && (
        <div className="pool-limits-body">
          <p className="ui-setting-help">{ABOUT}</p>
          {shown.groups.map((group) => (
            <Group key={group.label} pool={pool} group={group} />
          ))}
          {shown.advanced.length > 0 && (
            <Disclosure className="pool-limits-advanced" label="Advanced" level={4}>
              {shown.advanced.map((row) => (
                <LimitRow key={row.key} pool={pool} row={row} />
              ))}
            </Disclosure>
          )}
        </div>
      )}
    </Disclosure>
  );
}

/** One group: what its limits affect, when a change applies if that is the same for them all. */
function Group({ pool, group }: { pool: string; group: PoolLimitGroup }) {
  const heading = useId();
  return (
    <section className="pool-limits-group" aria-labelledby={heading}>
      <header className="pool-limits-head">
        <h4 id={heading}>{group.label}</h4>
        <p className="ui-setting-help">
          {group.help}
          {group.applies !== null && ` ${group.applies}`}
        </p>
      </header>
      {group.limits.map((row) => (
        <LimitRow key={row.key} pool={pool} row={row} />
      ))}
    </section>
  );
}

/** One limit: its box and unit, its default, the mark of a value set, and Reset. */
function LimitRow({ pool, row }: { pool: string; row: PoolLimitRow }) {
  const unit = useId();
  const says = useId();
  const refusal = useId();
  // What is typed, until what the core holds replaces it.
  const [typed, setTyped] = useState<string>();
  const [why, setWhy] = useState<string>();
  // The newest typing: an answer about older words is dropped.
  const typing = useRef(0);
  const isSet = row.set !== null;
  const held = isSet ? String(row.set) : "";
  const type = (to: string) => {
    setTyped(to);
    const mine = ++typing.current;
    if (to.trim() === "") {
      setWhy(undefined);
      return;
    }
    void checkLimit(row.key, to).then((said) => {
      if (mine === typing.current) setWhy(said.status === "error" ? said.error : undefined);
    });
  };
  // One save after another; how many are still to answer, which the last draw can't know of.
  const saves = useRef<Promise<unknown>>(Promise.resolve());
  const saving = useRef(0);
  const save = (value: number | null, mine: number) => {
    saving.current += 1;
    saves.current = saves.current.then(async () => {
      const refused = await saveLimits(pool, [[row.key, value]]);
      saving.current -= 1;
      if (mine !== typing.current) return;
      setWhy(refused);
      if (refused === undefined) setTyped(undefined);
    });
  };
  const commit = async () => {
    if (typed === undefined) return;
    const mine = typing.current;
    if (typed.trim() === "") {
      if (isSet || row.refused !== null || saving.current > 0) save(null, mine);
      else setTyped(undefined);
      return;
    }
    const checked = await checkLimit(row.key, typed);
    if (mine !== typing.current) return;
    if (checked.status === "error") setWhy(checked.error);
    else if (checked.data === row.set && saving.current === 0) setTyped(undefined);
    else save(checked.data, mine);
  };
  const refused = why !== undefined;
  return (
    <div className="pool-limit">
      <SettingRow
        label={row.plain}
        reset={{
          label: `Reset to ${row.default_said}`,
          name: `Reset to ${row.default_said}: ${row.plain} on pool ${pool}`,
          // A refused value on a limit never set still has to be clearable.
          disabled: !isSet && !refused && row.refused === null && (typed ?? "") === "",
          onReset: () => {
            const mine = ++typing.current;
            setWhy(undefined);
            setTyped(undefined);
            if (isSet || row.refused !== null || saving.current > 0) save(null, mine);
          },
        }}
        control={(ids) => (
          <span className="pool-limit-edit">
            <TextField
              id={ids.id}
              aria-labelledby={`${ids.labelledBy} ${unit}`}
              aria-describedby={[ids.describedBy, refusal, says].filter(Boolean).join(" ")}
              inputMode="numeric"
              columns={8}
              placeholder={String(row.default)}
              value={typed ?? held}
              isInvalid={why !== undefined}
              onChange={type}
              onBlur={(event) => {
                // Still the focused box: it is the window that lost the focus, mid-typing.
                if (document.activeElement !== event.target) void commit();
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void commit();
                } else if (event.key === "Escape" && typed !== undefined) {
                  event.preventDefault();
                  typing.current += 1;
                  setWhy(undefined);
                  setTyped(undefined);
                }
              }}
            />
            <span className="ui-setting-help" id={unit}>
              {row.unit}
            </span>
          </span>
        )}
      />
      {/* There from the start, so its words are heard as they land, politely, under the typing:
        empty, it takes no room, so the box stays where it is. */}
      <div className="ui-setting-error" id={refusal} aria-live="polite" aria-atomic="true">
        {why !== undefined ? <p>{why}</p> : row.refused !== null && <p>{row.refused}</p>}
      </div>
      <p className="ui-setting-help pool-limit-says" id={says}>
        {isSet && <Badge variant="accent">You set this</Badge>}
        <span>Default {row.default_said}.</span>
        {row.applies !== null && <span>{row.applies}</span>}
      </p>
    </div>
  );
}
