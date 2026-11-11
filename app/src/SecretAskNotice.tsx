// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { KeyRound, Link2, TriangleAlert } from "lucide-react";
import type { Offered, RunShown, SecretShown, Shown } from "./bindings";
import type { RunGiven } from "./asks";
import { Badge } from "./ui/badge/Badge";
import { Button } from "./ui/button/Button";
import { Checkbox } from "./ui/choice/Checkbox";
import { Segment, SegmentedControl } from "./ui/choice/Radio";
import { TextField } from "./ui/field/Field";
import { submits as chordSends } from "./ui/field/submitKey";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { Select } from "./ui/select/Select";
import {
  SecretAskAllow,
  SecretAskBody,
  SecretAskChat,
  SecretAskCommand,
  SecretAskFacts,
  SecretAskField,
  SecretAskFoot,
  SecretAskLine,
  SecretAskName,
  SecretAskNeedRow,
  SecretAskQuote,
  SecretAskScopeSaid,
  SecretAskTarget,
  SecretAskToolLine,
  SecretAskValue,
} from "./ui/secret-ask/SecretAsk";
import { VaultEntryPicker, type VaultEntries, type VaultEntry } from "./VaultEntryPicker";

/** The most a Secret need's purpose keeps, as the core cuts it (`MOST_PURPOSE_CHARS`). */
const MOST_PURPOSE_CHARS = 200;

/** How a field is filled: a new value, or an entry the person keeps already (#5073). */
export type FillWith = "value" | "entry";

/**
 * **What the person gave the notice besides the press**, read once as it is pressed: a Secret
 * ask's value, entry, name and Remember; a run's values, entries and Remember lines. A value is
 * read from its box and the box emptied, so it is never in React's state or the page's markup.
 */
export type SecretAskGiven = {
  /** A Secret ask's value, typed. */
  typed?: string;
  /** A Secret ask's entry, picked. */
  picked?: VaultEntry;
  /** A Secret ask filled under another name or vault (#5072, Change). */
  named?: { vault?: string; key?: string };
  /** The purpose of the line a Secret ask's Fill writes, where Remember is ticked (#5240). */
  remember?: string;
  /** A run's notice's fields and Remember lines (#5242, #5080). */
  run?: RunGiven;
};

/** The ids a run's options go by (`asking::RUN_ALLOW_*`). */
const SCOPES = ["allow-once", "allow-chat", "allow-always"] as const;
type Scope = (typeof SCOPES)[number] | "keep";
const SCOPE_SAID: Record<Scope, string> = {
  "allow-once": "Allow once",
  "allow-chat": "Allow for this chat",
  "allow-always": "Always allow",
  keep: "Don't allow",
};

const keysSaid = (keys: readonly string[]) =>
  keys.length < 2
    ? (keys[0] ?? "")
    : `${keys.slice(0, -1).join(", ")} and ${keys[keys.length - 1]}`;

/** One field's state: how it is filled, what is picked, and its Remember purpose. */
type Field = { with: FillWith; picked?: VaultEntry; filled: boolean; purpose: string };

/**
 * **The one notice for a secret** (#5080, spec #5235, ADR 0112; the reviewed prototype of #5227):
 * a Secret ask (#5068) or a run's one notice (#5242), drawn from the `secret-ask` family's parts.
 *
 * - **The app's facts in their panel, the chat's words apart**: the program's path, what it is
 *   handed, the hosts, the vault's line, why there is no Allow and the caution are purlis's; the
 *   command and the purpose are the chat's, quoted, and inert.
 * - **A field per key**: a masked box, or "Use an entry I have" with the picker grouped by vault,
 *   with Change in place on a Secret ask's field (#5070), never on a run's, whose key is the one
 *   its command reads (the person's ruling of 2026-11-07).
 * - **One Allow**: the scopes the notice offers, narrowest first and chosen; what the choice does
 *   said under it; the line Always allow writes shown first, and whether anything else of the
 *   persona's waits. Where the notice has no program Allow, the vault's line has its own.
 * - **Remember**, unticked, with the exact line and its purpose to edit (#5240).
 * - **The action bar**: what the chat hears, then Not now and the one action, which says what the
 *   press does. An answer that grants is the `grants` variant, never the primary look, and nothing
 *   here takes the keyboard as it comes (ADR 0097 §6).
 */
export function SecretAskNotice({
  ask,
  chat,
  entries,
  busy,
  onAnswer,
  onSaid,
  start,
}: {
  ask: Shown;
  /** The chat that asks, by the name the person sees. */
  chat: string;
  /** The entries the person keeps, vault by vault: read once they choose to use one. */
  entries: () => Promise<readonly VaultEntries[]>;
  busy: boolean;
  /** A press of `option`, with what to send read once by `given`, and the words the pressed
   *  button showed (an ask asked once more repeats them, #6178). */
  onAnswer: (option: Offered, given: () => SecretAskGiven, said: string) => void;
  /** Why a press sends nothing yet, said where the row says it. */
  onSaid: (said: string) => void;
  /** **A start's ask, in the Start dialog** (#5137): the chat starts with the value in
   *  `variable`. Its key is the one its profile names, so its field offers no Change, as a
   *  run's doesn't (D-5080-1). */
  start?: { variable: string };
}) {
  const secret = ask.secret ?? undefined;
  const run = ask.run ?? undefined;
  const keys = secret ? [secret.key] : (run?.fills.map((fill) => fill.key) ?? []);
  const boxes = useRef(new Map<string, HTMLInputElement>());
  const [fields, setFields] = useState<Record<string, Field>>(() =>
    Object.fromEntries(
      keys.map((key) => [
        key,
        {
          with: "value",
          filled: false,
          purpose: (secret?.remember?.purpose ?? "").slice(0, MOST_PURPOSE_CHARS),
        },
      ]),
    ),
  );
  const set = (key: string, change: Partial<Field>) =>
    setFields((was) => ({ ...was, [key]: { ...was[key], ...change } }));
  const [remember, setRemember] = useState(false);
  const [named, setNamed] = useState<{ vault: string; key: string } | undefined>();
  const offered = (id: string) => ask.options.find((one) => one.id === id);
  const scopes: Scope[] = [
    ...SCOPES.filter((id) => offered(id)),
    ...(run?.offer === "program" && offered("keep") ? ["keep" as const] : []),
  ];
  const [scope, setScope] = useState<Scope>(scopes[0] ?? "keep");
  const listed = useEntries(
    Object.values(fields).some((one) => one.with === "entry") ? entries : undefined,
  );

  if (!secret && !run) return null;
  const persona = secret?.persona ?? run?.persona ?? "";
  const vault = secret?.vault ?? run?.vault ?? "";

  /** What the person gave, read once: the boxes emptied as they are read. */
  const read = (): SecretAskGiven => {
    const typed = (key: string) => {
      const box = boxes.current.get(key);
      const value = box?.value ?? "";
      if (box) box.value = "";
      return value;
    };
    if (secret) {
      const field = fields[secret.key];
      const given: SecretAskGiven = {
        remember: remember ? field.purpose : undefined,
        named:
          named && (named.vault !== secret.vault || named.key !== secret.key) ? named : undefined,
      };
      if (field.with === "entry") given.picked = field.picked;
      else given.typed = typed(secret.key);
      return given;
    }
    const fills = run?.fills ?? [];
    return {
      run: {
        values: fills
          .filter((fill) => fields[fill.key]?.with !== "entry")
          .map((fill) => ({ key: fill.key, value: typed(fill.key) }))
          .filter((one) => one.value !== ""),
        links: fills.flatMap((fill) => {
          const picked = fields[fill.key]?.with === "entry" ? fields[fill.key].picked : undefined;
          return picked ? [{ field: fill.key, vault: picked.vault, key: picked.key }] : [];
        }),
        remember: remember
          ? fills
              .filter((fill) => fill.need)
              .map((fill) => ({ key: fill.key, purpose: fields[fill.key].purpose }))
          : [],
      },
    };
  };

  /** Not now: nothing typed or picked is sent, and nothing is stored or allowed. */
  const declined = (): SecretAskGiven => {
    for (const box of boxes.current.values()) box.value = "";
    return secret ? {} : { run: { values: [], links: [], remember: [] } };
  };

  /** Why a press that fills sends nothing yet, or none. */
  const notYet = (): string | undefined => {
    for (const key of keys) {
      if (fields[key].with === "entry" && !fields[key].picked)
        return `Pick an entry for ${key}, or choose New value.`;
    }
    if (remember) {
      const needs = secret
        ? secret.remember?.need
          ? [secret.key]
          : []
        : (run?.fills.filter((fill) => fill.need).map((fill) => fill.key) ?? []);
      const blank = needs.find((key) => fields[key].purpose.trim() === "");
      if (blank !== undefined) return `Say what ${blank} is for, or untick Remember.`;
    }
    return undefined;
  };

  const press = (option: Offered | undefined, fills: boolean, said: string) => {
    if (option === undefined || busy) return;
    if (fills) {
      const why = notYet();
      if (why !== undefined) {
        onSaid(why);
        return;
      }
    }
    onAnswer(option, fills ? read : declined, said);
  };

  const program = run?.program ?? "";
  const name = program.split("/").pop() ?? program;
  const grantsProgram = run?.offer === "program";
  const keep = offered("keep");

  // The one action, and what it says.
  let action: Offered | undefined;
  let actionSaid = "";
  let actionGrants = false;
  if (secret) {
    action = offered("fill");
    actionSaid = action?.label ?? "Fill";
  } else if (run?.offer === "program") {
    if (scope === "keep") {
      action = keep;
      actionSaid = keys.length > 0 ? "Fill, don't allow" : "Don't allow";
    } else {
      action = offered(scope);
      actionGrants = true;
      actionSaid =
        keys.length > 0
          ? `Fill and ${SCOPE_SAID[scope].replace(/^Allow/, "allow").replace(/^Always/, "always")}`
          : SCOPE_SAID[scope];
    }
  } else if (run?.offer === "fill") {
    action = offered("fill");
    actionSaid = action?.label ?? "Fill";
  }
  const vaultAllow = run?.offer === "vault" ? offered("allow-vault") : undefined;
  const keepSaid = run?.offer === "vault" ? (keep?.label ?? "") : "Not now";

  const hears = (() => {
    const whom = `${chat} hears`;
    // A start's chat is handed the value; any running chat that joined the ask hears as ever.
    if (secret && start)
      return secret.waiting > 1
        ? `${chat} starts with the value in ${start.variable}. Each other chat waiting on it hears filled or declined and the key's name, never the value.`
        : `${chat} starts with the value in ${start.variable}.`;
    if (secret) {
      return `${whom} filled or declined${secret.no_allow != null ? ", why no Allow is offered," : ""} and the key's name. Never the value.`;
    }
    if (run?.offer === "program") {
      return keys.length === 0
        ? `${whom} whether ${name} was allowed. Never a value.`
        : `${whom} filled or declined, whether ${name} was allowed, and the key's name. Never the value.`;
    }
    return `${whom} filled or declined, and the key's name. Never the value.`;
  })();

  // **A start's ask claims the dialog's chord** (#5670): ⌘⏎ anywhere in it fills, as Fill does
  // (and says what is missing first), and never presses the dialog's Start, which the same ask
  // would refuse again. Outside it the chord is the dialog's.
  const claimChord = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!start || !chordSends(event.nativeEvent)) return;
    event.preventDefault();
    event.stopPropagation();
    if (!event.repeat) press(action, true, actionSaid);
  };

  return (
    <SecretAskBody
      onKeyDown={start ? claimChord : undefined}
      label={
        // The app's own words only: never `says`, which quotes the chat's purpose.
        secret
          ? `${persona} asks for ${secret.key} from vault ${vault}`
          : `${persona} runs ${program} with vault ${vault}`
      }
      data-testid="secret-ask"
    >
      {secret && <SecretReason secret={secret} />}
      <Facts
        secret={secret}
        run={run}
        name={name}
        persona={persona}
        vault={vault}
        vaultAllow={vaultAllow}
        busy={busy}
        onVaultAllow={() => press(vaultAllow, true, vaultAllow?.label ?? "")}
      />
      <ChatWords chat={chat} secret={secret} run={run} />
      {keys.map((key) => (
        <OneField
          key={key}
          fieldKey={key}
          persona={persona}
          vault={vault}
          secret={secret}
          declared={
            secret
              ? secret.declared
                ? (secret.declared_purpose ?? "")
                : undefined
              : (run?.fills.find((fill) => fill.key === key)?.declared ??
                (run?.fills.find((fill) => fill.key === key)?.need === false ? "" : undefined))
          }
          gone={secret?.gone ?? run?.fills.find((fill) => fill.key === key)?.gone ?? undefined}
          field={fields[key]}
          set={(change) => set(key, change)}
          box={(el) => {
            if (el === null) boxes.current.delete(key);
            else boxes.current.set(key, el);
          }}
          listed={listed}
          named={named}
          onNamed={secret && !start ? setNamed : undefined}
          handedIn={start?.variable}
          chat={chat}
        />
      ))}
      <Remember
        secret={secret}
        run={run}
        remember={remember}
        onRemember={setRemember}
        fields={fields}
        set={set}
        persona={persona}
      />
      {grantsProgram && run && scopes.length > 0 && (
        <TheAllow
          run={run}
          chat={chat}
          name={name}
          scope={scope}
          scopes={scopes}
          onScope={setScope}
          keys={keys}
        />
      )}
      <SecretAskFoot hears={hears}>
        {keep !== undefined && (secret || run?.offer !== "program" || keys.length > 0) && (
          <Button
            size="sm"
            variant="ghost"
            aria-disabled={busy || undefined}
            aria-busy={busy || undefined}
            onPress={() => press(keep, false, keepSaid)}
            data-testid="sa-not-now"
          >
            {keepSaid}
          </Button>
        )}
        {action !== undefined && (
          <Button
            size="sm"
            variant={actionGrants ? "grants" : "secondary"}
            aria-disabled={busy || undefined}
            aria-busy={busy || undefined}
            onPress={() => press(action, true, actionSaid)}
            data-testid="sa-primary"
          >
            {actionSaid}
          </Button>
        )}
      </SecretAskFoot>
    </SecretAskBody>
  );
}

/** Why a Secret ask is asked again, or what is wrong with what is there (#5075). */
function SecretReason({ secret }: { secret: SecretShown }) {
  if (secret.replace == null && secret.reads == null) return null;
  return (
    <SecretAskFacts>
      {secret.reads != null && (
        <SecretAskLine who="Linked" data-testid="sa-reads">
          <SecretAskName>{secret.key}</SecretAskName> reads{" "}
          <SecretAskName>{`${secret.reads.vault}/${secret.reads.key}`}</SecretAskName>. A new value
          goes there and reaches every key linked to it; an entry you pick re-points the link.
        </SecretAskLine>
      )}
      {secret.replace != null && (
        <SecretAskLine who="Replace">
          The chat says the value doesn't work. Its words are below.
        </SecretAskLine>
      )}
    </SecretAskFacts>
  );
}

/** **The app's facts**, in their panel. */
function Facts({
  secret,
  run,
  name,
  persona,
  vault,
  vaultAllow,
  busy,
  onVaultAllow,
}: {
  secret: SecretShown | undefined;
  run: RunShown | undefined;
  name: string;
  persona: string;
  vault: string;
  vaultAllow: Offered | undefined;
  busy: boolean;
  onVaultAllow: () => void;
}) {
  if (secret) {
    if (secret.no_allow == null && secret.tool == null) return null;
    return (
      <SecretAskFacts>
        {secret.tool != null && (
          <SecretAskLine who="Tool">
            {persona} declares <SecretAskName>{secret.tool}</SecretAskName>, which needs this key.
          </SecretAskLine>
        )}
        {secret.no_allow != null && (
          <SecretAskLine who="No Allow" data-testid="sa-locked">
            {sentence(secret.no_allow)} You can still store the value.
          </SecretAskLine>
        )}
      </SecretAskFacts>
    );
  }
  if (!run) return null;
  return (
    <SecretAskFacts>
      <SecretAskLine who="Program">
        <SecretAskName data-testid="sa-program">{run.program}</SecretAskName>{" "}
        {run.offer === "program" && (
          <Tooltip
            tip={`${persona} doesn't list ${name} among its tools. Its chats can't hand it a value unless you allow it.`}
          >
            <Badge variant="neutral" data-testid="sa-undeclared-program">
              not declared by this persona
            </Badge>
          </Tooltip>
        )}
      </SecretAskLine>
      {run.values.length > 0 && (
        <SecretAskLine who="It is handed">
          {run.values.map((one, at) => (
            <span key={one.var}>
              {at > 0 && "; "}
              <SecretAskName>{one.key}</SecretAskName>{" "}
              {one.in_a_file ? "in a private file, its path in" : "in"}{" "}
              <SecretAskName>{one.var}</SecretAskName>
            </span>
          ))}
          , from vault <SecretAskName>{run.vault}</SecretAskName>
        </SecretAskLine>
      )}
      <SecretAskLine who="This chat can reach" data-testid="sa-hosts">
        {run.every_host
          ? "Every host: this chat's sandbox doesn't limit them."
          : run.hosts.length === 0
            ? "No host."
            : run.hosts.map((host, at) => (
                <span key={host}>
                  {at > 0 && ", "}
                  <SecretAskName>{host}</SecretAskName>
                </span>
              ))}
      </SecretAskLine>
      {!run.ahead && run.cwd !== "" && (
        <SecretAskLine who="Starts in">
          <SecretAskName>{run.cwd}</SecretAskName>, with no input from the chat
        </SecretAskLine>
      )}
      {run.not_its_vault && (
        <SecretAskLine who="Vault" data-testid="sa-vault-line">
          {run.offer === "vault" ? (
            <>
              {persona} can't use vault <SecretAskName>{vault}</SecretAskName>. Allow lets every
              chat opened as {persona} use it, in this project on this machine, and stores what you
              type. Keep blocked stores nothing.{" "}
              {vaultAllow !== undefined && (
                <Button
                  size="sm"
                  variant="grants"
                  aria-disabled={busy || undefined}
                  aria-busy={busy || undefined}
                  onPress={onVaultAllow}
                  data-testid="sa-vault-allow"
                >
                  {vaultAllow.label}
                </Button>
              )}
            </>
          ) : (
            <>
              {persona} can't use vault <SecretAskName>{vault}</SecretAskName>. Allow once and Allow
              for this chat let this program use it for that alone, and write nothing lasting. Only
              Always allow also lets every chat opened as {persona} use it, in this project on this
              machine. What you type is stored there only if you allow the program.
            </>
          )}
        </SecretAskLine>
      )}
      {run.no_allow != null && (
        <SecretAskLine who="No Allow" data-testid="sa-locked">
          {sentence(run.no_allow)} You can still store the value.
        </SecretAskLine>
      )}
      {run.offer === "program" && (
        <SecretAskLine
          who={
            <>
              <TriangleAlert aria-hidden="true" /> Caution
            </>
          }
          data-testid="sa-caution"
        >
          {run.caution != null && <>{sentence(run.caution)} </>}A program can run others and read
          files this chat wrote, and can leave what it's handed where this chat reads it. Allow it
          only if you trust this chat with these values.
        </SecretAskLine>
      )}
      {run.ahead && (
        <SecretAskLine who="Command" data-testid="sa-ahead">
          None yet: the chat asks ahead of its run. Allow for this chat lets it run{" "}
          <SecretAskName>{run.program}</SecretAskName> with these keys and any arguments; its first
          run fixes how they're handed.
        </SecretAskLine>
      )}
    </SecretAskFacts>
  );
}

/** A sentence of the app's, with its full stop. Its first word is left as the core wrote it: it
 *  is often a program's name (`curl`, `python3`), which keeps its own spelling. */
const sentence = (said: string) => {
  const trimmed = said.trim();
  return /[.!?]$/.test(trimmed) ? trimmed : `${trimmed}.`;
};

/** **What the chat wrote**: its command and its purpose, behind the rule, inert. */
function ChatWords({
  chat,
  secret,
  run,
}: {
  chat: string;
  secret: SecretShown | undefined;
  run: RunShown | undefined;
}) {
  const command = run && !run.ahead ? run.command : undefined;
  const purpose = secret?.purpose ?? undefined;
  const replace = secret?.replace ?? undefined;
  if (command === undefined && purpose === undefined && replace === undefined) return null;
  return (
    <SecretAskChat>
      {command !== undefined && <SecretAskCommand who={`${chat} wants to run`} text={command} />}
      {replace !== undefined && (
        <SecretAskQuote who={`${chat} says it doesn't work`} text={replace} />
      )}
      {purpose !== undefined && <SecretAskQuote who={`${chat} says`} text={purpose} />}
    </SecretAskChat>
  );
}

/** **One key's field**: where it lands, how it is filled, and the box or the picker. */
function OneField({
  fieldKey,
  persona,
  vault,
  secret,
  declared,
  gone,
  field,
  set,
  box,
  listed,
  named,
  onNamed,
  chat,
  handedIn,
}: {
  fieldKey: string;
  persona: string;
  vault: string;
  secret: SecretShown | undefined;
  /** What the persona's definition says it is for; `""` declared with no words; undefined not declared. */
  declared: string | undefined;
  gone: { vault: string; key: string } | undefined;
  field: Field;
  set: (change: Partial<Field>) => void;
  box: (el: HTMLInputElement | null) => void;
  listed: Listed;
  named: { vault: string; key: string } | undefined;
  onNamed: ((to: { vault: string; key: string } | undefined) => void) | undefined;
  chat: string;
  /** A start's variable (#5137): the chat is handed the value in it, so it is never said to be
   *  kept from the chat. */
  handedIn?: string;
}) {
  const [changing, setChanging] = useState(false);
  const at = named ?? { vault, key: fieldKey };
  const path = `${at.vault}/${at.key}`;
  // Change is for a value that has a name of its own to choose: never a replace or a mend, which
  // is filled where it is, and never an entry, which is linked under the asked name.
  const changeable =
    onNamed !== undefined &&
    secret?.replace == null &&
    secret?.reads == null &&
    gone === undefined &&
    field.with === "value";
  const vaults = [...new Set([vault, ...listed.entries.map((one) => one.vault)])];
  const badge = declared === undefined && (
    <Tooltip
      tip={`${persona} doesn't list ${fieldKey} among the secrets it needs. The chat asked on its own.`}
    >
      <Badge variant="neutral" data-testid="sa-undeclared">
        not declared by this persona
      </Badge>
    </Tooltip>
  );
  return (
    <SecretAskField>
      <SecretAskTarget mark={<KeyRound />} path={path}>
        {badge}
        {changeable && !changing && (
          <Button
            size="sm"
            variant="link"
            aria-label={`Change where ${fieldKey} is stored`}
            onPress={() => setChanging(true)}
          >
            Change
          </Button>
        )}
      </SecretAskTarget>
      {changeable && changing && (
        <SecretAskValue>
          <TextField
            size="sm"
            aria-label={`Name ${fieldKey} is stored under`}
            value={at.key}
            spellCheck={false}
            autoComplete="off"
            onChange={(to) => onNamed?.({ ...at, key: to })}
            help={at.key !== fieldKey ? `${chat} hears the new name, ${at.key}.` : undefined}
          />
          <Select
            size="sm"
            aria-label={`Vault ${fieldKey} is stored in`}
            value={at.vault}
            onValueChange={(to) => onNamed?.({ ...at, vault: to })}
            options={vaults.map((one) => ({
              value: one,
              label: one === vault ? `${one} (${persona}'s vault)` : one,
            }))}
          />
          <Button size="sm" variant="ghost" onPress={() => setChanging(false)}>
            Done
          </Button>
        </SecretAskValue>
      )}
      {declared !== undefined && declared !== "" && (
        <SecretAskLine who={`${persona} declares it for`} data-testid="sa-declared">
          {declared}
        </SecretAskLine>
      )}
      {gone !== undefined && (
        <SecretAskLine
          who={
            <>
              <Link2 aria-hidden="true" /> Broken link
            </>
          }
          data-testid="sa-gone"
        >
          <SecretAskName>{fieldKey}</SecretAskName> was linked to{" "}
          <SecretAskName>{`${gone.vault}/${gone.key}`}</SecretAskName>, which is gone. Give it a
          value, or pick another entry.
        </SecretAskLine>
      )}
      <SegmentedControl
        size="sm"
        aria-label={`How to fill ${fieldKey}`}
        value={field.with}
        onChange={(to) => {
          set({ with: to as FillWith, picked: to === "value" ? undefined : field.picked });
          if (to === "entry") setChanging(false);
        }}
      >
        <Segment value="value">New value</Segment>
        <Segment value="entry">Use an entry I have</Segment>
      </SegmentedControl>
      <SecretAskValue>
        {field.with === "value" ? (
          <TextField
            size="sm"
            type="password"
            inputRef={box}
            aria-label={`Value of ${fieldKey}`}
            placeholder="Type or paste the value"
            onChange={(filled: boolean) => set({ filled })}
            help={
              secret?.reads != null
                ? `Stored in ${secret.reads.vault}/${secret.reads.key}, which ${fieldKey} reads.`
                : handedIn !== undefined
                  ? `Stored in vault ${at.vault} on this machine, and handed to the chat in ${handedIn}.`
                  : `Stored in vault ${at.vault} on this machine. Never shown to the chat.`
            }
          />
        ) : (
          <>
            <VaultEntryPicker
              size="sm"
              aria-label={`Entry ${fieldKey} reads`}
              entries={listed.entries}
              picked={field.picked}
              onPick={(picked) => set({ picked })}
            />
            <p className="ui-text-meta" role="status">
              {listed.refused ??
                (listed.entries.length === 0 && listed.read
                  ? "No vault holds an entry to link to yet. Fill it with a new value instead."
                  : field.picked !== undefined
                    ? `${fieldKey} becomes a link to ${field.picked.vault}/${field.picked.key}: the value isn't copied, and a rotation of ${field.picked.key} reaches it.${
                        secret?.replace != null && secret.reads == null
                          ? ` The value ${fieldKey} holds now is deleted from ${vault}.`
                          : ""
                      }`
                    : "The value isn't copied: the key reads the entry, so a rotation reaches it.")}
            </p>
          </>
        )}
      </SecretAskValue>
    </SecretAskField>
  );
}

/** **Remember this need for the persona** (#5240), unticked until asked: the exact lines, each
 *  purpose to edit, and a `vault:` line where a Secret ask's persona names none. */
function Remember({
  secret,
  run,
  remember,
  onRemember,
  fields,
  set,
  persona,
}: {
  secret: SecretShown | undefined;
  run: RunShown | undefined;
  remember: boolean;
  onRemember: (to: boolean) => void;
  fields: Record<string, Field>;
  set: (key: string, change: Partial<Field>) => void;
  persona: string;
}) {
  const shown = secret?.remember ?? run?.remember ?? undefined;
  if (shown == null) return null;
  const needs = secret
    ? shown.need
      ? [secret.key]
      : []
    : (run?.fills.filter((fill) => fill.need).map((fill) => fill.key) ?? []);
  return (
    <SecretAskValue>
      <Checkbox isSelected={remember} onChange={onRemember}>
        Remember this need for the persona
      </Checkbox>
      {remember && (
        <div data-testid="secret-need-line">
          <p className="ui-text-meta">
            Adds to <code>{shown.file}</code>. It rides your next save, and teammates read every
            word.
          </p>
          {needs.map((key) => (
            <SecretAskNeedRow key={key}>
              <SecretAskName>{`secret-need.${key}:`}</SecretAskName>
              <TextField
                size="sm"
                aria-label={`What ${key} is for`}
                value={fields[key].purpose}
                maxLength={MOST_PURPOSE_CHARS}
                placeholder="Say what it is for"
                onChange={(to) => set(key, { purpose: to })}
              />
            </SecretAskNeedRow>
          ))}
          {shown.vault != null && (
            <>
              <SecretAskName data-testid="sa-vault-need">{`vault: ${shown.vault}`}</SecretAskName>
              <p className="ui-text-meta">
                This sets {persona}'s vault. A <code>vault: none</code> in that file is replaced.
              </p>
            </>
          )}
        </div>
      )}
    </SecretAskValue>
  );
}

/** **The one Allow**: the scopes offered, narrowest first and chosen, and what the choice does. */
function TheAllow({
  run,
  chat,
  name,
  scope,
  scopes,
  onScope,
  keys,
}: {
  run: RunShown;
  chat: string;
  name: string;
  scope: Scope;
  scopes: readonly Scope[];
  onScope: (to: Scope) => void;
  keys: readonly string[];
}) {
  const handed = keysSaid([...new Set(run.values.map((one) => one.key).concat(keys))]);
  let said: ReactNode;
  if (scope === "allow-once") {
    said = (
      <p>
        Runs this exact command once. Any change to it asks again. Good until it is used or the chat
        ends.
      </p>
    );
  } else if (scope === "allow-chat") {
    said = (
      <p>
        {name} gets {handed} with any arguments {chat} gives it, until the chat ends.
        {run.ahead
          ? " Its first run fixes how they're handed, and later runs must hand them the same way."
          : ""}{" "}
        Nothing is written, and you can remove it from the Allowed list in Settings › Sandbox.
      </p>
    );
  } else if (scope === "allow-always" && run.always != null) {
    said = (
      <>
        <p>
          Adds this line to {run.persona}'s tools in the project's committed file. It rides your
          next save, and teammates read every word. Every chat as {run.persona} on this machine can
          then run it with any arguments.
        </p>
        <SecretAskToolLine text={run.always.line} />
        <p data-testid="sa-others-wait">
          {run.always.others_wait
            ? `Other changes to ${run.persona}'s list wait on your Allow. This approves only this line: their notice stays up, and ${chat} runs on a grant for this chat until you allow them.`
            : `Nothing else of ${run.persona}'s waits on you, so your Allow moves with this line. ${chat} runs it now.`}{" "}
          A teammate's chat waits for their own Allow.
        </p>
        {run.always.by_env && (
          <p data-testid="sa-env-advice">
            This hands the value itself in a variable, as the command asked. A file is safer:
            another program of yours can read a variable while this one runs.
          </p>
        )}
      </>
    );
  } else {
    said = (
      <p>
        {keys.length > 0 &&
          (run.not_its_vault
            ? "Nothing you type is stored: the persona can't use this vault. "
            : `${keysSaid(keys)} is still stored. `)}
        {chat} hears {name} was not allowed, and isn't asked about {name} and {run.vault} again.
      </p>
    );
  }
  return (
    <SecretAskAllow>
      <p className="ui-text-body">
        Let <SecretAskName>{run.program}</SecretAskName> receive {handed} from {run.vault}?
      </p>
      <SegmentedControl
        size="sm"
        aria-label={`Allow ${name}`}
        value={scope}
        onChange={(to) => onScope(to as Scope)}
      >
        {scopes.map((one) => (
          <Segment key={one} value={one}>
            {SCOPE_SAID[one]}
          </Segment>
        ))}
      </SegmentedControl>
      <SecretAskScopeSaid>{said}</SecretAskScopeSaid>
    </SecretAskAllow>
  );
}

type Listed = { entries: readonly VaultEntries[]; read: boolean; refused?: string };

/** The entries `read` lists, read once it is given; why not, where it was refused. */
function useEntries(read: (() => Promise<readonly VaultEntries[]>) | undefined): Listed {
  const [listed, setListed] = useState<Listed>({ entries: [], read: false });
  const wanted = read !== undefined;
  useEffect(() => {
    if (!wanted || read === undefined) return;
    let live = true;
    read().then(
      (entries) => {
        if (live) setListed({ entries, read: true });
      },
      (why: unknown) => {
        if (live)
          setListed({
            entries: [],
            read: true,
            refused: `purlis could not list your vaults' entries: ${why instanceof Error ? why.message : String(why)}. Fill it with a new value instead.`,
          });
      },
    );
    return () => {
      live = false;
    };
    // Read once each time the person chooses to use an entry.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wanted]);
  return listed;
}

/**
 * **A run's facts, alone** (#5998): what a run grant answered on its row in Asks lets run, its
 * program, what it is handed, where it reaches and its caution, read before the answers beside
 * them. Its answers are the row's, so the vault's own Allow is drawn there, not here.
 */
export function RunFacts({ run }: { run: RunShown }) {
  return (
    <Facts
      secret={undefined}
      run={run}
      name={run.program.split("/").pop() ?? run.program}
      persona={run.persona}
      vault={run.vault}
      vaultAllow={undefined}
      busy={false}
      onVaultAllow={() => undefined}
    />
  );
}
