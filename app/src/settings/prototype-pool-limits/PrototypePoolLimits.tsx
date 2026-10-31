// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/pool-limits, never merged to main).
 *
 * A pool's limits page, in Settings › You › This machine › Pools, in three variants: A one list
 * plus Advanced; B grouped by what they affect; C beside the runner's live use. The floating bar
 * switches the variant, the pool (Docker or cluster) and the error's suggestion. Fake data in
 * memory only.
 */
import "./prototype.css";
import { useEffect, useRef, useState } from "react";
import { Notice } from "../../Notice";
import { Badge } from "../../ui/badge/Badge";
import { Button } from "../../ui/button/Button";
import { AlertDialog } from "../../ui/dialog/Dialog";
import { Disclosure } from "../../ui/disclosure/Disclosure";
import { TextField } from "../../ui/field/Field";
import { Meter } from "../../ui/meter/Meter";
import { Select } from "../../ui/select/Select";
import { SettingGroup, SettingRow, SettingsLayout } from "../components";
import {
  APPLIES_NEXT,
  APPLIES_REPLACED,
  AREAS,
  boxed,
  defaultOf,
  highestOf,
  judge,
  limit,
  LIMITS,
  POOLS,
  PROFILE_NEEDS,
  PROFILE_SET_LOW,
  PROFILE_SUGGESTS,
  say,
  SUGGESTION_SAID,
  SUGGESTIONS,
  UNITS,
  type LimitDef,
  type PoolId,
  type Suggestion,
} from "./data";

export const PROTOTYPE_VARIANTS = ["A", "B", "C"] as const;
export type PrototypeVariant = (typeof PROTOTYPE_VARIANTS)[number];
const VARIANT_SAID: Record<PrototypeVariant, string> = {
  A: "One list",
  B: "Grouped",
  C: "Beside live use",
};
export const PROTOTYPE_POOLS: readonly PoolId[] = ["docker", "cluster"];
export { SUGGESTIONS };

type Values = Record<PoolId, Record<string, number>>;
type Draft = { text: string; unit: string };

/** Everything a row needs from the page. */
interface Page {
  pool: PoolId;
  aimedAt: string | undefined;
  valueOf: (def: LimitDef) => number;
  setValue: (def: LimitDef) => number | undefined;
  madeOf: (def: LimitDef) => number;
  pending: (def: LimitDef) => boolean;
  draftOf: (def: LimitDef) => Draft;
  type: (def: LimitDef, draft: Draft) => void;
  reset: (def: LimitDef) => void;
  askReplace: () => void;
}

function defaultLine(def: LimitDef, pool: PoolId): string {
  const value = say(def.kind, defaultOf(def, pool));
  return pool === "cluster"
    ? `Your team's default ${value} · highest allowed ${say(def.kind, def.teamHighest)}`
    : `Default ${value}`;
}

function PendingLine({ def, page, button }: { def: LimitDef; page: Page; button: boolean }) {
  return (
    <p className="ui-setting-help proto-pl-pending">
      The runner has {say(def.kind, page.madeOf(def))}; {say(def.kind, page.valueOf(def))} when it
      is replaced.
      {button && (
        <Button size="sm" onPress={page.askReplace}>
          Replace runner now
        </Button>
      )}
    </p>
  );
}

function LiveUse({ def, page }: { def: LimitDef; page: Page }) {
  // What the runner runs under now: a shaping limit is what it was made with.
  const of = def.shaping ? page.madeOf(def) : page.valueOf(def);
  const use = def.live.use;
  const share = use / of;
  return (
    <div className="proto-pl-live">
      <Meter
        label={def.live.said}
        value={Math.min(use, of)}
        minValue={0}
        maxValue={of}
        valueLabel={`${say(def.kind, use)} of ${say(def.kind, of)}`}
      />
      {share > 1 && <Badge variant="waiting">Over this limit</Badge>}
      {share >= 0.9 && share <= 1 && <Badge variant="waiting">Near this limit</Badge>}
    </div>
  );
}

function LimitRow({
  def,
  page,
  says = true,
  live = false,
  replaceHere = true,
}: {
  def: LimitDef;
  page: Page;
  /** Says when it applies on the row. A group may say it once instead. */
  says?: boolean;
  live?: boolean;
  replaceHere?: boolean;
}) {
  const set = page.setValue(def) !== undefined;
  const draft = page.draftOf(def);
  const judged = judge(def, page.pool, draft.text, draft.unit);
  const units = UNITS[def.kind];
  const placeholder = boxed(def.kind, defaultOf(def, page.pool));
  return (
    <div className="proto-pl-row" data-aimed={page.aimedAt === def.id ? "" : undefined}>
      <SettingRow
        label={def.label}
        modified={set}
        reset={{
          label: "Reset",
          // A refused value on a limit never set still has to be clearable.
          disabled: !set && judged.state !== "refused",
          onReset: () => page.reset(def),
        }}
        control={(ids) => (
          <span className="proto-pl-edit">
            <TextField
              id={ids.id}
              aria-labelledby={ids.labelledBy}
              inputMode="text"
              columns={7}
              value={draft.text}
              placeholder={placeholder.unit === draft.unit ? placeholder.text : undefined}
              isInvalid={judged.state === "refused"}
              onChange={(text) => page.type(def, { ...draft, text })}
            />
            {units.length > 1 ? (
              <Select
                aria-label={`Unit of ${def.label}`}
                value={draft.unit}
                options={units.map((one) => ({ value: one.unit, label: one.unit }))}
                onValueChange={(unit) => page.type(def, { ...draft, unit })}
              />
            ) : (
              <span className="ui-setting-help">{units[0].unit}</span>
            )}
          </span>
        )}
      />
      <div className="proto-pl-under">
        {judged.state === "refused" && (
          <p className="proto-pl-refused" role="alert">
            {judged.why}
          </p>
        )}
        <p className="ui-setting-help proto-pl-says">
          {set && <Badge variant="accent">You set this</Badge>}
          <span>{defaultLine(def, page.pool)}</span>
          {def.placeholder && (
            <span className="proto-pl-tag" title="The real default comes from measurement">
              PROTOTYPE default
            </span>
          )}
          {says && <span>· {def.shaping ? APPLIES_REPLACED : APPLIES_NEXT}</span>}
        </p>
        {live && <LiveUse def={def} page={page} />}
        {page.pending(def) && <PendingLine def={def} page={page} button={replaceHere} />}
      </div>
    </div>
  );
}

const inPool = (pool: PoolId) => LIMITS.filter((def) => !def.clusterOnly || pool === "cluster");

function Advanced({ page, live = false, says = true }: { page: Page; live?: boolean; says?: boolean }) {
  return (
    <Disclosure label="Advanced" level={3} className="proto-pl-advanced">
      {inPool(page.pool)
        .filter((def) => def.advanced)
        .map((def) => (
          <LimitRow key={def.id} def={def} page={page} live={live} says={says} />
        ))}
    </Disclosure>
  );
}

function VariantA({ page }: { page: Page }) {
  return (
    <>
      {inPool(page.pool)
        .filter((def) => !def.advanced)
        .map((def) => (
          <LimitRow key={def.id} def={def} page={page} />
        ))}
      <Advanced page={page} />
    </>
  );
}

function VariantB({ page }: { page: Page }) {
  const any = inPool(page.pool).some((def) => page.pending(def));
  return (
    <>
      {AREAS.map((area) => (
        <section className="proto-pl-area" key={area.id}>
          <header className="proto-pl-area-head">
            <h3>{area.label}</h3>
            <p className="ui-setting-help">
              {area.help} {area.id === "runner" ? `${APPLIES_REPLACED}.` : `${APPLIES_NEXT}.`}
            </p>
            {area.id === "runner" && any && (
              <Button size="sm" onPress={page.askReplace}>
                Replace runner now
              </Button>
            )}
          </header>
          {inPool(page.pool)
            .filter((def) => def.area === area.id && !def.advanced)
            .map((def) => (
              <LimitRow key={def.id} def={def} page={page} says={false} replaceHere={false} />
            ))}
        </section>
      ))}
      <Advanced page={page} />
    </>
  );
}

function VariantC({ page }: { page: Page }) {
  return (
    <>
      {inPool(page.pool)
        .filter((def) => !def.advanced)
        .map((def) => (
          <LimitRow key={def.id} def={def} page={page} live />
        ))}
      <Advanced page={page} live />
    </>
  );
}

/** The error's suggestion, as it stands where the attempt failed. */
function SuggestionNotice({
  which,
  page,
  onApply,
  onDismiss,
}: {
  which: Exclude<Suggestion, "none">;
  page: Page;
  onApply: (def: LimitDef, value: number) => void;
  onDismiss: () => void;
}) {
  const def = limit(which === "wait" ? "code-time" : "profile-size");
  const unit = which === "wait" ? "min" : "MiB";
  const of = which === "wait" ? 60 : 1;
  const now = page.valueOf(def);
  const highest = highestOf(def, page.pool);
  const offered =
    which === "wait"
      ? Math.min(now * 2, highest ?? now * 2) / of
      : Math.min(PROFILE_SUGGESTS, highest ?? PROFILE_SUGGESTS);
  const [text, setText] = useState(String(offered));
  const [phase, setPhase] = useState<"asked" | "retrying" | "done">("asked");
  const applied = useRef(0);
  useEffect(() => {
    if (phase !== "retrying") return;
    const timer = setTimeout(() => setPhase("done"), 1200);
    return () => clearTimeout(timer);
  }, [phase]);
  if (which === "fixed") {
    return (
      <Notice
        cause="prototype-fixed"
        tone="trouble"
        title="This output is over what a check reads"
        onDismiss={onDismiss}
      >
        Checks read at most 1 MiB of a command's output. This limit is fixed, so nothing passes a
        check unread. Write the output to a file, and check a part of it.
      </Notice>
    );
  }
  if (phase === "done") {
    return (
      <Notice cause="prototype-done" tone="news" onDismiss={onDismiss}>
        {which === "wait"
          ? `Sent in 14 min. The limit of ${say(def.kind, applied.current)} is saved to this pool.`
          : `Sent. The limit of ${say(def.kind, applied.current)} is saved to this pool.`}
      </Notice>
    );
  }
  if (phase === "retrying") {
    return (
      <Notice cause="prototype-retrying" tone="news" running onDismiss={onDismiss}>
        Saved to this pool. Trying again…
      </Notice>
    );
  }
  const judged = judge(def, page.pool, text, unit);
  const tooSmall =
    which === "profile" && judged.state === "ok" && judged.value <= PROFILE_NEEDS
      ? `This profile is ${PROFILE_NEEDS} MiB, so it needs more than that.`
      : undefined;
  const refused =
    judged.state === "refused" ? judged.why : judged.state === "empty" ? "Write a number." : tooSmall;
  const typedAt = judged.state === "ok" ? say(def.kind, judged.value) : "this";
  return (
    <Notice
      cause={`prototype-${which}`}
      tone="trouble"
      title={
        which === "wait"
          ? `Took longer than ${say(def.kind, now)}. Wait up to ${typedAt} on this pool?`
          : `This profile is ${PROFILE_NEEDS} MiB; raise to ${typedAt}`
      }
      fixes={[
        {
          label: "Apply and retry",
          primary: true,
          off: refused,
          onPress: () => {
            if (judged.state !== "ok") return;
            applied.current = judged.value;
            onApply(def, judged.value);
            setPhase("retrying");
          },
        },
      ]}
      onDismiss={onDismiss}
    >
      <span className="proto-pl-suggest">
        <span>
          {which === "wait"
            ? "Code send to this pool's runner stopped at its limit."
            : `The most this pool lets a profile send is ${say(def.kind, now)}.`}
        </span>
        <span className="proto-pl-edit">
          <TextField
            aria-label={which === "wait" ? "Wait up to" : "Raise to"}
            columns={7}
            value={text}
            isInvalid={refused !== undefined}
            onChange={setText}
          />
          <span>{unit}</span>
        </span>
        {refused !== undefined && <span className="proto-pl-refused">{refused}</span>}
        {highest !== undefined && (
          <span className="ui-setting-help">
            Highest allowed on this pool: {say(def.kind, highest)}
          </span>
        )}
      </span>
    </Notice>
  );
}

function Bar<T extends string>({
  name,
  values,
  current,
  said,
  onChange,
}: {
  name: string;
  values: readonly T[];
  current: T;
  said: (value: T) => string;
  onChange: (to: T) => void;
}) {
  return (
    <div className="proto-pl-bar-axis" role="group" aria-label={name}>
      <span className="proto-pl-bar-name">{name}</span>
      {values.map((value) => (
        <Button
          key={value}
          size="sm"
          variant={value === current ? "primary" : "ghost"}
          aria-pressed={value === current}
          onPress={() => onChange(value)}
        >
          {said(value)}
        </Button>
      ))}
    </div>
  );
}

export function PrototypePoolLimits({
  variant,
  pool,
  suggestion,
  onChange,
}: {
  variant: PrototypeVariant;
  pool: PoolId;
  suggestion: Suggestion;
  onChange: (to: { variant?: PrototypeVariant; pool?: PoolId; suggestion?: Suggestion }) => void;
}) {
  const [set, setSet] = useState<Values>({
    docker: { memory: 8192, "signin-wait": 1800 },
    cluster: { memory: 12288, "signin-wait": 1800 },
  });
  const [made, setMade] = useState<Values>({ docker: {}, cluster: {} });
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [asking, setAsking] = useState(false);
  const [said, setSaid] = useState<string | undefined>();
  const [filter, setFilter] = useState("");

  const key = (def: LimitDef) => `${pool}:${def.id}`;
  const forget = (def: LimitDef) =>
    setDrafts((was) => {
      const { [key(def)]: _gone, ...rest } = was;
      return rest;
    });
  const write = (def: LimitDef, value: number | undefined) =>
    setSet((was) => {
      const { [def.id]: _gone, ...rest } = was[pool];
      return { ...was, [pool]: value === undefined ? rest : { ...rest, [def.id]: value } };
    });

  // The first error needs a profile over its limit, so its state sets that limit low.
  useEffect(() => {
    const def = limit("profile-size");
    if (suggestion === "profile") {
      setSet((was) => ({ ...was, [pool]: { ...was[pool], [def.id]: PROFILE_SET_LOW } }));
      setDrafts((was) => {
        const { [`${pool}:${def.id}`]: _gone, ...rest } = was;
        return rest;
      });
      return () => {
        setSet((was) => {
          if (was[pool][def.id] !== PROFILE_SET_LOW) return was;
          const { [def.id]: _gone, ...rest } = was[pool];
          return { ...was, [pool]: rest };
        });
      };
    }
    return undefined;
  }, [suggestion, pool]);

  const valueOf = (def: LimitDef) => set[pool][def.id] ?? defaultOf(def, pool);
  const madeOf = (def: LimitDef) => made[pool][def.id] ?? defaultOf(def, pool);
  const page: Page = {
    pool,
    aimedAt:
      suggestion === "profile" ? "profile-size" : suggestion === "wait" ? "code-time" : undefined,
    valueOf,
    setValue: (def) => set[pool][def.id],
    madeOf,
    pending: (def) => def.shaping === true && valueOf(def) !== madeOf(def),
    draftOf: (def) => {
      const draft = drafts[key(def)];
      if (draft !== undefined) return draft;
      const value = set[pool][def.id];
      return value === undefined
        ? { text: "", unit: boxed(def.kind, defaultOf(def, pool)).unit }
        : boxed(def.kind, value);
    },
    type: (def, draft) => {
      setDrafts((was) => ({ ...was, [key(def)]: draft }));
      const judged = judge(def, pool, draft.text, draft.unit);
      if (judged.state === "ok") write(def, judged.value);
      if (judged.state === "empty") write(def, undefined);
    },
    reset: (def) => {
      forget(def);
      write(def, undefined);
    },
    askReplace: () => setAsking(true),
  };
  const Variant = variant === "A" ? VariantA : variant === "B" ? VariantB : VariantC;
  const waiting = inPool(pool).filter((def) => page.pending(def));

  return (
    <div className="proto-pl-root">
      <p className="proto-pl-banner" role="note">
        <strong>PROTOTYPE</strong> · Variant {variant}: {VARIANT_SAID[variant]} · Fake data, nothing
        is saved
      </p>
      <SettingsLayout
        levels={[
          { id: "you", label: "You" },
          { id: "project", label: "Project" },
          { id: "workspace", label: "Workspace" },
        ]}
        level="you"
        onLevelChange={() => undefined}
        about="Your own settings, on this machine. Kept in your purlis folder, which only you see."
        groups={[
          { id: "you.look", label: "Look" },
          { id: "you.profiles", label: "Profiles" },
          { id: "you.machine", label: "This machine" },
          { id: "you.machine.pools", label: "Pools", sub: true },
          { id: "you.notifications", label: "Notifications" },
        ]}
        group="you.machine.pools"
        onGroupChange={() => undefined}
        filter={filter}
        onFilterChange={setFilter}
      >
        <SettingGroup
          label={`Limits of pool ${POOLS[pool].name}`}
          help="Every default is already the best value, so most people never change these. A limit you leave empty follows purlis's default as it improves. One you set stays, and is marked."
        >
          <p className="ui-setting-help">
            {POOLS[pool].said} · <Badge variant="accent">linked</Badge>
            {pool === "cluster" && " · Your team sets the defaults and the highest allowed."}
          </p>
          {suggestion !== "none" && (
            <div className="proto-pl-error">
              <p className="proto-pl-tag">PROTOTYPE: stands where the attempt failed</p>
              <SuggestionNotice
                key={`${suggestion}-${pool}`}
                which={suggestion}
                page={page}
                onApply={(def, value) => {
                  forget(def);
                  write(def, value);
                }}
                onDismiss={() => onChange({ suggestion: "none" })}
              />
            </div>
          )}
          {said !== undefined && (
            <p className="ui-setting-help" role="status">
              {said}
            </p>
          )}
          <Variant page={page} />
        </SettingGroup>
      </SettingsLayout>
      {asking && (
        <AlertDialog
          onClose={() => setAsking(false)}
          title={`Replace the runner of pool ${POOLS[pool].name} now?`}
          description={
            <p className="came-back">
              {POOLS[pool].chats === 1 ? "1 chat runs" : `${POOLS[pool].chats} chats run`} on it and
              would stop. Its home is kept. New limits:{" "}
              {waiting.map((def) => `${def.label} ${say(def.kind, valueOf(def))}`).join(", ")}.
            </p>
          }
          main={{
            children: "Replace runner",
            onPress: () => {
              setMade((was) => ({
                ...was,
                [pool]: {
                  ...was[pool],
                  ...Object.fromEntries(waiting.map((def) => [def.id, valueOf(def)])),
                },
              }));
              setAsking(false);
              setSaid("Runner replaced. Its limits now apply.");
            },
          }}
        />
      )}
      {!import.meta.env.PROD && (
        <div className="proto-pl-bar" role="toolbar" aria-label="PROTOTYPE switcher">
          <span className="proto-pl-bar-tag">PROTOTYPE</span>
          <Bar
            name="Variant"
            values={PROTOTYPE_VARIANTS}
            current={variant}
            said={(one) => `${one}: ${VARIANT_SAID[one]}`}
            onChange={(to) => onChange({ variant: to })}
          />
          <Bar
            name="Pool"
            values={PROTOTYPE_POOLS}
            current={pool}
            said={(one) => (one === "docker" ? "Docker" : "Cluster")}
            onChange={(to) => onChange({ pool: to })}
          />
          <Bar
            name="Error"
            values={SUGGESTIONS}
            current={suggestion}
            said={(one) => SUGGESTION_SAID[one]}
            onChange={(to) => onChange({ suggestion: to })}
          />
        </div>
      )}
    </div>
  );
}
