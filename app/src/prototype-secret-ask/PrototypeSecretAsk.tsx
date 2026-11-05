// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#5227, spec #5235, ADR 0111; first drawn for #5070). The one Secret ask notice, drawn inside the ask
 * surfaces #4561 settled: the pinned strip above the input, the top-tab quiz card and the Inbox
 * card. One body, three hosts. The window around it is a stand-in; the dashed bar at the bottom is
 * the prototype's, not the design.
 *
 * URL: `?s=<state>` the state, `&at=strip|quiz|inbox` the surface, `&open` the strip opened (it
 * stays folded until Fill: #5070's ruling), `&bare` no bar.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Hand, KeyRound, Link2, List, MessageCircleQuestion, SquareTerminal, TriangleAlert, X } from "lucide-react";
import { KindMark } from "../InboxRow";
import { Badge, Count } from "../ui/badge/Badge";
import { Button, IconButton } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { Checkbox } from "../ui/choice/Checkbox";
import { Segment, SegmentedControl } from "../ui/choice/Radio";
import { TextField } from "../ui/field/Field";
import { Select } from "../ui/select/Select";
import { StatusItem } from "../ui/status/StatusItem";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { Twist } from "../ui/disclosure/Disclosure";
import { VaultEntryPicker } from "../VaultEntryPicker";
import { ENTRIES, PERSONA_FILE, SCENES, TOOL_FILE, VAULTS, needsCount, toolLine, type Ask, type Fill, type Scope, type World } from "./model";
import "./frame.css";
import "./secret-ask.css";

type Act = (run: (w: World) => void) => void;
type Surface = "strip" | "quiz" | "inbox";
const SURFACES: Surface[] = ["strip", "quiz", "inbox"];

const ageSaid = (m: number) => (m < 1 ? "now" : `${m}m`);
const path = (vault: string, key: string) => `${vault}/${key}`;
const keysSaid = (keys: string[]) =>
  keys.length < 2 ? (keys[0] ?? "") : `${keys.slice(0, -1).join(", ")} and ${keys[keys.length - 1]}`;

/** The one line every surface leads with: the key, per #5070's ruling, and the program if any. */
function headline(a: Ask) {
  const keys = a.needs.map((n) => n.key);
  const program = a.allow.kind === "offered" ? a.run?.name : undefined;
  if (keys.length === 0) return `${a.chat} wants to hand ${keysSaid(a.held)} to ${program}`;
  const wanted = keys.length === 1 ? keys[0] : `${keys.length} secrets`;
  return program ? `${a.chat} needs ${wanted} for ${program}` : `${a.chat} needs a secret: ${keysSaid(keys)}`;
}

/** Controls, and what draws nothing or turns text round: built from code points, so no invisible
 *  character sits in this file. */
const CONTROL = new RegExp(
  `([${[[0x00, 0x1f], [0x7f, 0x9f], [0x200b, 0x200f], [0x2028, 0x202e], [0x2060, 0x2069], [0xfeff, 0xfeff]]
    .map(([from, to]) => `\\u{${from.toString(16)}}-\\u{${to.toString(16)}}`)
    .join("")}])`,
  "u",
);

const CONTROL_NAMES: Record<number, string> = { 0: "NUL", 7: "BEL", 8: "BS", 9: "TAB", 10: "LF", 13: "CR", 27: "ESC", 127: "DEL" };

/** A chat's text with every control and invisible character drawn as itself, on one flow. */
function Inert({ text }: { text: string }) {
  const parts = text.split(CONTROL);
  return (
    <>
      {parts.map((part, n) => {
        if (n % 2 === 0) return part;
        const code = part.codePointAt(0)!;
        const shown = CONTROL_NAMES[code] ?? `U+${code.toString(16).toUpperCase().padStart(4, "0")}`;
        return (
          <mark key={n} className="sa-control" data-testid="sa-control">
            {shown}
          </mark>
        );
      })}
    </>
  );
}

/** Why it is asked again, or what is wrong with what is there. */
function Reason({ a }: { a: Ask }) {
  const r = a.reason;
  const key = a.needs[0]?.key;
  if (r.kind === "replace")
    return (
      <div className="sa-reason" data-testid="sa-reason">
        <p>
          The chat says the current value doesn't work: <q className="sa-chat-words"><Inert text={r.said} /></q>
        </p>
        {r.linkedTo && (
          <p className="ui-text-secondary">
            <Link2 aria-hidden="true" className="sa-inline-mark" /> {key} reads{" "}
            <code className="sa-key">{r.linkedTo}</code>. A new value goes there, and reaches the{" "}
            {r.linkedFrom} keys linked to it.
          </p>
        )}
      </div>
    );
  if (r.kind === "broken")
    return (
      <div className="sa-reason" data-testid="sa-reason">
        <p>
          <Link2 aria-hidden="true" className="sa-inline-mark" /> {key} was linked to{" "}
          <code className="sa-key">{r.was}</code>, which is gone. Pick another entry or store a value.
        </p>
      </div>
    );
  if (r.kind === "runner")
    return (
      <div className="sa-reason" data-testid="sa-reason">
        <p>
          {a.chat} runs on {r.runner}. You can't allow a program for a chat there yet, because this
          window can't show what limits that machine holds it to. You can still store the value here,
          for chats on this machine. {a.chat} hears it was declined, and to declare {a.run?.name ?? "the program"} as
          a tool.
        </p>
      </div>
    );
  if (r.kind === "no-vault")
    return (
      <p className="sa-quiet ui-text-secondary" data-testid="sa-reason">
        You have no {a.vault} vault yet. Your first fill makes it, on your default provider (system
        keyring).
      </p>
    );
  return null;
}

/** What the app knows, in its own panel: no word in here is the chat's. */
function Facts({ a }: { a: Ask }) {
  const r = a.run;
  if (!r) return null;
  return (
    <section className="sa-facts" data-testid="sa-facts" aria-label="From purlis">
      <span className="sa-facts-head ui-text-meta">From purlis</span>
      <p className="sa-said">
        <span className="sa-who ui-text-meta">Program</span>
        <span className="sa-fact">
          <code className="sa-key sa-key-strong" data-testid="sa-program">{r.path}</code>{" "}
          <Tooltip tip={`${a.persona} doesn't list ${r.name} among its tools. Its chats can't give it a value unless you allow it.`}>
            <Badge variant="neutral" data-testid="sa-undeclared-program">not declared by this persona</Badge>
          </Tooltip>
        </span>
      </p>
      <p className="sa-said">
        <span className="sa-who ui-text-meta">This chat can reach</span>
        <span className="sa-fact" data-testid="sa-hosts">
          {r.hosts.map((h, n) => (
            <span key={h}>
              {n > 0 && ", "}
              <code className="sa-key">{h}</code>
            </span>
          ))}
        </span>
      </p>
      {a.vaultAccess && (
        <p className="sa-said" data-testid="sa-vault-line">
          <span className="sa-who ui-text-meta">Vault</span>
          <span className="sa-fact">
            {a.persona} can't use vault <code className="sa-key">{a.vault}</code> yet. An Allow here
            also lets every chat opened as {a.persona} use it, in this project on this machine.
          </span>
        </p>
      )}
      {a.allow.kind === "locked" ? (
        <p className="sa-said" data-testid="sa-locked">
          <span className="sa-who ui-text-meta">No Allow</span>
          <span className="sa-fact">{a.allow.why} You can still store the value.</span>
        </p>
      ) : (
        r.caution &&
        a.allow.kind === "offered" && (
          <p className="sa-said" data-testid="sa-caution">
            <span className="sa-who ui-text-meta">
              <TriangleAlert aria-hidden="true" className="sa-inline-mark sa-caution-mark" /> Caution
            </span>
            <span className="sa-fact">{r.caution}</span>
          </p>
        )
      )}
    </section>
  );
}

/** What the chat wrote: the command plain and wrapped, the purpose quoted, each marked as the chat's. */
function ChatWords({ a }: { a: Ask }) {
  const command = a.run?.command ?? a.waitingCommand;
  if (!command && !a.purpose) return null;
  return (
    <div className="sa-purpose" data-testid="sa-chat">
      {command && (
        <div className="sa-said sa-said-chat">
          <span className="sa-who ui-text-meta">{a.chat} wants to run</span>
          <code className="sa-command" data-testid="sa-command"><Inert text={command} /></code>
        </div>
      )}
      {a.purpose && (
        <p className="sa-said sa-said-chat" data-testid="sa-chat-purpose">
          <span className="sa-who ui-text-meta">{a.chat} says</span>
          <q className="sa-chat-words"><Inert text={a.purpose} /></q>
        </p>
      )}
    </div>
  );
}

/** Where a fill lands: the key and vault, with Change to rename or move it, in place. */
function Target({ a, f, at, act }: { a: Ask; f: Fill; at: number; act: Act }) {
  const set = (run: (one: Fill) => void) => act((d) => run(d.fills[at]));
  const badge = !f.declared && (
    <Tooltip tip={`${a.persona} doesn't list ${f.key} among the secrets it needs. The chat asked on its own.`}>
      <Badge variant="neutral" data-testid="sa-undeclared">not declared by this persona</Badge>
    </Tooltip>
  );
  if (!f.changing)
    return (
      <p className="sa-target">
        <KeyRound aria-hidden="true" className="sa-inline-mark" />
        <code className="sa-key sa-key-strong">{path(f.vaultAs, f.keyAs)}</code>
        {badge}
        <Button size="sm" variant="link" aria-label={`Change ${f.key}`} onPress={() => set((o) => void (o.changing = true))}>
          Change
        </Button>
      </p>
    );
  return (
    <div className="sa-change" data-testid="sa-change">
      <div className="sa-change-vault">
        <span className="sa-label ui-text-meta">Key</span>
        <TextField
          size="sm"
          aria-label="Key"
          value={f.keyAs}
          onChange={(to) => set((o) => void (o.keyAs = to))}
          spellCheck={false}
          autoComplete="off"
          help={f.keyAs !== f.key ? `${a.chat} hears the new name, ${f.keyAs}.` : undefined}
        />
      </div>
      <div className="sa-change-vault">
        <span className="sa-label ui-text-meta">Vault</span>
        <Select
          size="sm"
          aria-label="Vault"
          value={f.vaultAs}
          onValueChange={(to) => set((o) => void (o.vaultAs = to))}
          options={[...new Set([a.vault, ...VAULTS])].map((v) => ({
            value: v,
            label: v === a.vault ? `${v} (${a.persona}'s vault)` : v,
          }))}
        />
      </div>
      <Button size="sm" variant="ghost" onPress={() => set((o) => void (o.changing = false))}>
        Done
      </Button>
      {badge}
    </div>
  );
}

/** One key's field: a new value, or an entry the person keeps already. */
function KeyFill({ a, f, at, act, surface }: { a: Ask; f: Fill; at: number; act: Act; surface: Surface }) {
  const set = (run: (one: Fill) => void) => act((d) => run(d.fills[at]));
  const to = path(f.vaultAs, f.keyAs);
  const linkedTo = a.reason.kind === "replace" ? a.reason.linkedTo : undefined;
  const picked = f.entry ? path(f.entry.vault, f.entry.key) : "";
  return (
    <div className="sa-fill" data-testid="sa-key-fill">
      <Target a={a} f={f} at={at} act={act} />
      {f.declared && (
        <p className="sa-said" data-testid="sa-declared">
          <span className="sa-who ui-text-meta">{a.persona} persona</span>
          <span>{f.declared}</span>
        </p>
      )}
      <SegmentedControl
        size="sm"
        aria-label={`How to fill ${f.key}`}
        value={f.mode}
        onChange={(m) => set((o) => void (o.mode = m as Fill["mode"]))}
      >
        <Segment value="value">New value</Segment>
        <Segment value="entry">Use an entry I have</Segment>
      </SegmentedControl>
      {f.mode === "value" ? (
        <TextField
          key={`${surface}-value`}
          size="sm"
          type="password"
          className="sa-row"
          aria-label={`Value for ${to}`}
          placeholder="Paste or type the value"
          autoComplete="off"
          inputRef={(el) => {
            /* A scene drawn already filled: the box holds its own words, as the real one does. */
            if (el && f.valueFilled && !el.value) el.value = "stand-in";
          }}
          onChange={(filled) => set((o) => void (o.valueFilled = filled))}
          help={
            linkedTo
              ? `Stored in ${linkedTo}, which ${f.key} reads.`
              : `Stored in your ${f.vaultAs} vault, on this machine. Never shown to the chat.`
          }
        />
      ) : (
        <div className="sa-col sa-row">
          <VaultEntryPicker
            size="sm"
            aria-label={`Entry ${f.key} reads`}
            entries={ENTRIES}
            picked={f.entry}
            onPick={(entry) => set((o) => void (o.entry = entry))}
          />
          <span className="ui-text-meta sa-help">
            {picked
              ? `${to} becomes a link to ${picked}: the value isn't copied, and a rotation of ${picked} reaches it.`
              : "The value isn't copied: the key reads the entry, so a rotation reaches it."}
          </span>
        </div>
      )}
    </div>
  );
}

/** #5240's Remember, unticked until asked. Copy as `SecretNeedLine` has it, a row per key. */
function Remember({ w, act }: { w: World; act: Act }) {
  const a = w.ask!;
  const needs = w.fills.map((f, at) => ({ f, at })).filter(({ f }) => !f.declared);
  const vaultLine = a.personaNamesVault ? null : a.vault;
  if (needs.length === 0 && vaultLine == null) return null;
  return (
    <div className="sa-remember" data-testid="sa-remember">
      <Checkbox isSelected={w.remember} onChange={(ticked) => act((d) => void (d.remember = ticked))}>
        Remember this need for the persona
      </Checkbox>
      {w.remember && (
        <div className="secret-need-line" data-testid="secret-need-line">
          <p className="ui-text-meta">
            Adds to <code>{PERSONA_FILE.replace("devops", a.persona)}</code>. It rides your next save, and teammates read every word.
          </p>
          {needs.map(({ f, at }) => (
            <div key={f.key} className="secret-need-line-row">
              <code className="sa-key">secret-need.{f.keyAs}:</code>
              <TextField
                size="sm"
                aria-label={`What ${f.keyAs} is for`}
                value={f.purpose}
                maxLength={200}
                placeholder="Say what it is for"
                onChange={(to) => act((d) => void (d.fills[at].purpose = to))}
              />
            </div>
          ))}
          {vaultLine != null && (
            <>
              <code className="sa-key" data-testid="sa-vault-need">vault: {vaultLine}</code>
              <p className="ui-text-meta">
                This sets the persona's vault. A <code>vault: none</code> in that file is replaced.
              </p>
            </>
          )}
        </div>
      )}
    </div>
  );
}

/** A name that isn't the one the command names means the Allow has nothing to bind. */
const moved = (a: Ask, w: World) =>
  w.fills.some((f) => f.keyAs !== f.key || f.vaultAs !== a.vault);

const SCOPE_NAMES: Record<Scope, string> = {
  once: "Allow once",
  chat: "Allow for this chat",
  always: "Always allow",
  dont: "Don't allow",
};

/** The one Allow: four choices, narrowest first and chosen, and what the choice does. */
function Allow({ w, act }: { w: World; act: Act }) {
  const a = w.ask!;
  const r = a.run;
  if (!r || a.allow.kind !== "offered") return null;
  const keys = [...w.fills.map((f) => f.key), ...a.held];
  const said = keysSaid(keys);
  const vault = w.fills[0]?.vaultAs ?? a.vault;
  if (moved(a, w))
    return (
      <p className="sa-quiet ui-text-secondary" data-testid="sa-allow-moved">
        The command names {path(a.vault, a.needs[0].key)}, so with a new name there is no command to allow. {a.chat} hears the
        new name, and its next run asks again.
      </p>
    );
  return (
    <div className="sa-allow" data-testid="sa-allow">
      <p className="sa-allow-ask">
        Let <code className="sa-key sa-key-strong">{r.path}</code> receive {said} from {vault}?
      </p>
      <SegmentedControl
        size="sm"
        aria-label={`Allow ${r.name}`}
        value={w.scope}
        onChange={(to) => act((d) => void (d.scope = to as Scope))}
      >
        {(Object.keys(SCOPE_NAMES) as Scope[]).map((one) => (
          <Segment key={one} value={one}>
            {SCOPE_NAMES[one]}
          </Segment>
        ))}
      </SegmentedControl>
      {w.scope === "once" && (
        <p className="sa-quiet ui-text-secondary" data-testid="sa-scope-said">
          Runs this exact command once. Any change to it asks again. Good until it is used or the chat ends.
        </p>
      )}
      {w.scope === "chat" && (
        <p className="sa-quiet ui-text-secondary" data-testid="sa-scope-said">
          {r.name} gets {said} with any arguments {a.chat} gives it, until the chat ends. Nothing is
          written, and you can revoke it in Settings › Sandbox.
        </p>
      )}
      {w.scope === "always" && (
        <div className="sa-col" data-testid="sa-scope-said">
          <p className="sa-quiet ui-text-secondary">
            Adds to <code>{TOOL_FILE}</code>. It rides your next save, and teammates read every word.
          </p>
          <pre className="sa-tool-line" data-testid="sa-tool-line">{toolLine(a, w.fills).join("\n")}</pre>
          <p className="sa-quiet ui-text-secondary">
            {a.chat} runs it now. A teammate's chat waits for their own Allow, and anything else pending
            keeps its own notice.
            {r.handover === "env" &&
              " This hands the value over in a variable, as the command asked. A file is safer where the program can read one."}
          </p>
        </div>
      )}
      {w.scope === "dont" && (
        <p className="sa-quiet ui-text-secondary" data-testid="sa-scope-said">
          {w.fills.length > 0 ? `${keysSaid(w.fills.map((f) => f.keyAs))} is still stored. ` : ""}
          {a.chat} hears {r.name} was not allowed, and isn't asked about {r.name} and {vault} again.
          {a.vaultAccess ? ` ${a.persona} still can't use vault ${vault}.` : ""}
        </p>
      )}
    </div>
  );
}

/** The one action: what it says follows what the press will do. */
function primaryLabel(w: World): string {
  const a = w.ask!;
  const allow = a.allow.kind === "offered" && !moved(a, w);
  const fills = w.fills;
  if (allow && fills.length === 0) return SCOPE_NAMES[w.scope];
  if (allow)
    return w.scope === "dont"
      ? "Fill, don't allow"
      : `Fill and ${SCOPE_NAMES[w.scope].replace("Allow", "allow").replace("Always", "always")}`;
  if (a.reason.kind === "runner") return "Store here";
  if (a.reason.kind === "replace") return "Replace";
  return fills.length === 1 && fills[0].mode === "entry" ? "Link" : "Store";
}

/** The body: the same in the strip, the quiz and the Inbox. */
function Body({ w, act, surface }: { w: World; act: Act; surface: Surface }) {
  const a = w.ask!;
  const ready = w.fills.every((f) => (f.mode === "value" ? f.valueFilled : f.entry !== undefined));
  const left = w.fills.filter((f) => !(f.mode === "value" ? f.valueFilled : f.entry !== undefined)).length;
  const allow = a.allow.kind === "offered" && !moved(a, w);
  const settle = () =>
    act((d) => {
      d.settled = {
        kind: "answered",
        filled: d.fills.map((f) => ({
          how: f.mode,
          path: path(f.vaultAs, f.keyAs),
          to: f.entry ? path(f.entry.vault, f.entry.key) : undefined,
        })),
        scope: allow ? d.scope : undefined,
        remembered: d.remember ? d.fills.filter((f) => !f.declared).map((f) => `secret-need.${f.keyAs}`) : [],
      };
    });
  const hears =
    a.reason.kind === "runner"
      ? `it was declined, to declare ${a.run?.name ?? "the program"} as a tool, and the key's name`
      : a.allow.kind === "locked"
      ? `filled or declined, ${a.allow.hears}, and the key's name`
      : allow
        ? a.needs.length === 0
          ? `whether ${a.run!.name} was allowed`
          : `filled or declined, whether ${a.run!.name} was allowed, and the key's name`
        : "filled or declined, and the key's name";
  return (
    <div className="sa-body" data-testid="sa-body" data-says={headline(a)}>
      <Reason a={a} />
      <Facts a={a} />
      <ChatWords a={a} />
      {w.fills.map((f, at) => (
        <KeyFill key={f.key} a={a} f={f} at={at} act={act} surface={surface} />
      ))}
      {w.fills.length > 0 && <Remember w={w} act={act} />}
      <Allow w={w} act={act} />
      <div className="tq-foot">
        <span className="ui-text-meta">
          {a.waiting.length > 1 ? "The chats hear" : `${a.chat} hears`} {hears}. Never the value.
        </span>
        <span className="sa-actions">
          {w.fills.length > 1 && left > 0 && <span className="ui-text-meta" data-testid="sa-left">{left} of {w.fills.length} still to fill</span>}
          {w.fills.length > 0 && (
            <Button size="sm" variant="ghost" onPress={() => act((d) => void (d.settled = { kind: "declined", filled: [], remembered: [] }))}>
              Not now
            </Button>
          )}
          <Button size="sm" variant="primary" className="sa-primary" isDisabled={!ready} onPress={settle} data-testid="sa-primary">
            {primaryLabel(w)}
          </Button>
        </span>
      </div>
    </div>
  );
}

function Waiting({ a }: { a: Ask }) {
  if (a.waiting.length < 2) return null;
  return (
    <Tooltip tip={`Waiting: ${a.waiting.join(", ")}. One answer settles all of them.`}>
      <Count variant="waiting" data-testid="sa-waiting" said={`${a.waiting.length} chats waiting`}>
        {a.waiting.length} waiting
      </Count>
    </Tooltip>
  );
}

/** What is left once it is answered: one quiet line. Nothing is offered later (ADR 0111). */
function Settled({ w }: { w: World }) {
  const a = w.ask!;
  const s = w.settled!;
  const name = a.run?.name;
  const stored = s.filled
    .map((f) => (f.how === "entry" ? `${f.path} now reads ${f.to}.` : `Stored ${f.path}.`))
    .join(" ");
  const keys = s.filled.map((f) => f.path.split("/")[1]);
  const said =
    s.kind === "declined"
      ? `Not now. ${a.chat} heard it was declined, and won't ask for ${keysSaid(a.needs.map((n) => n.key))} again.`
      : s.scope === "dont"
        ? `${stored} ${name} was not allowed. ${a.chat} heard: filled, but the program was not allowed, and it won't be asked about ${name} and ${a.vault} again.`
        : s.scope
          ? `${stored} Allowed ${a.run!.path} ${s.scope === "once" ? "once" : s.scope === "chat" ? "for this chat" : "always"}. ${a.chat} heard: filled and allowed, and its command is running.`
          : `${stored} ${a.chat} heard: filled, as ${keysSaid(keys)}.`;
  return (
    <div className="sa-settled">
      <div className="tq-line" data-testid="sa-settled">
        <KindMark kind="update" />
        <p>
          {said}
          {s.remembered.length > 0 && ` Added ${keysSaid(s.remembered)} to ${PERSONA_FILE.replace("devops", a.persona)}. It rides your next save.`}
        </p>
      </div>
    </div>
  );
}

function Strip({ w, act, surface }: { w: World; act: Act; surface: Surface }) {
  const a = w.ask!;
  return (
    <div className="tq-strip sa-strip" data-testid="strip" data-open={w.open ? "" : undefined}>
      <div className="sa-strip-line">
        <span className="tq-strip-mark" aria-hidden="true">
          <Hand />
        </span>
        <div className="tq-strip-words">
          <span className="tq-strip-label ui-text-meta">Waiting on you · secret</span>
          <p className="tq-strip-says">{headline(a)}</p>
        </div>
        <Waiting a={a} />
        <Button size="sm" variant="ghost" aria-expanded={w.open} onPress={() => act((d) => void (d.open = !d.open))}>
          <Twist open={w.open} />
          {w.open ? "Fold" : a.needs.length === 0 ? "Review" : "Fill"}
        </Button>
      </div>
      {w.open && <Body w={w} act={act} surface={surface} />}
    </div>
  );
}

function Quiz({ w, act }: { w: World; act: Act }) {
  const a = w.ask!;
  return (
    <Card variant="ask" className="inbox-ask tq-quiz" data-testid="quiz" data-quiz="secret" role="group" aria-label={headline(a)}>
      <p className="inbox-ask-top">
        <span className="inbox-who ui-text-body ui-text-strong">{headline(a)}</span>
        <span className="tq-top-end">
          <Waiting a={a} />
          <span className="inbox-where inbox-secondary ui-text-meta">{ageSaid(a.age)}</span>
        </span>
      </p>
      <Body w={w} act={act} surface="quiz" />
    </Card>
  );
}

function InboxPanel({ w, act }: { w: World; act: Act }) {
  const a = w.ask;
  return (
    <section className="inbox tq-inbox" data-view="inbox" aria-label="Inbox">
      <div className="inbox-list">
        {(!a || w.settled) && <p className="tq-empty ui-text-secondary">Nothing waits on you.</p>}
        {a && !w.settled && (
          <section className="inbox-chat">
            <h3 className="inbox-chain">smart-ide › {a.persona} › {a.chat}</h3>
            <Card as="li" variant="ask" className="inbox-ask" data-testid="inbox-card">
              <p className="inbox-says">
                <KindMark kind="decision" />
                <span className="ask-says inbox-primary ui-text-body ui-text-strong">{headline(a)}</span>
                <span className="inbox-meta ui-text-meta">
                  <span className="inbox-source">Secret</span>
                  <span className="inbox-meta-dot"> · </span>
                  <time className="inbox-age">{ageSaid(a.age)}</time>
                </span>
              </p>
              <p className="sa-inbox-waiting">
                <Waiting a={a} />
              </p>
              <Body w={w} act={act} surface="inbox" />
            </Card>
          </section>
        )}
      </div>
    </section>
  );
}

const CHATS = [
  { id: "steward", name: "steward", persona: "steward" },
  { id: "developer 7", name: "developer 7", persona: "devops" },
  { id: "reviewer 2", name: "reviewer 2", persona: "devops" },
  { id: "qa 1", name: "qa 1", persona: "devops" },
  { id: "developer 9", name: "developer 9 · runner", persona: "devops" },
];

function ChatList({ w }: { w: World }) {
  const waiting = new Set(w.ask && !w.settled ? w.ask.waiting : []);
  return (
    <aside className="tq-side" aria-label="Navigation">
      <h2 className="tq-side-head ui-text-secondary">Chats</h2>
      <ul className="sa-chats">
        {CHATS.map((c) => (
          <li key={c.id} className="sa-chat" data-front={w.ask?.chat === c.id || (!w.ask && c.id === "developer 7") ? "" : undefined}>
            <SquareTerminal aria-hidden="true" className="sa-inline-mark" />
            <span className="sa-grow">{c.name}</span>
            {waiting.has(c.id) && <span className="tq-row-word ui-text-meta">needs you</span>}
          </li>
        ))}
      </ul>
    </aside>
  );
}

function params() {
  return new URLSearchParams(window.location.search);
}

export type Look = { appearance: "light" | "dark"; theme: string };

export function PrototypeSecretAsk({ look, onLook }: { look: Look; onLook: (l: Look) => void }) {
  const [at, setAt] = useState(() => Math.max(0, SCENES.findIndex((s) => s.id === params().get("s"))));
  const [surface, setSurface] = useState<Surface>(() => {
    const s = params().get("at") as Surface;
    return SURFACES.includes(s) ? s : "quiz";
  });
  const bare = params().has("bare");
  const scene = SCENES[at];
  const [w, setW] = useState<World>(() => {
    const made = scene.make();
    if (params().has("open")) made.open = true;
    return made;
  });
  const [listing, setListing] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const setUrl = (key: string, value: string) => {
    const url = new URL(window.location.href);
    url.searchParams.set(key, value);
    window.history.replaceState(null, "", url);
  };
  const go = useCallback((to: number) => {
    const n = (to + SCENES.length) % SCENES.length;
    setAt(n);
    setW(SCENES[n].make());
    setUrl("s", SCENES[n].id);
  }, []);
  const act = useCallback<Act>((run) => {
    setW((was) => {
      const next = structuredClone(was);
      run(next);
      return next;
    });
  }, []);
  useEffect(() => {
    (window as unknown as { sa: unknown }).sa = { pretend: () => scene.pretend && act(scene.pretend.run) };
  }, [act, scene]);
  useEffect(() => {
    const keys = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest("input, textarea, [role=combobox]")) return;
      if (e.key === "]") go(at + 1);
      if (e.key === "[") go(at - 1);
      if (e.key === "\\") setListing((was) => !was);
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  }, [at, go]);

  const count = needsCount(w);
  const a = w.ask;
  const live = a && !w.settled;
  const inPane = live && surface !== "inbox";
  const front = a?.chat ?? "developer 7";

  return (
    <div className="tq-root" data-bare={bare ? "" : undefined} data-needs={count} data-scene={scene.id} data-surface={surface} data-testid="sa-root">
      <header className="tq-title">
        <span className="ui-text-body ui-text-strong">smart-ide</span>
        <span className="tq-title-end">
          {count > 0 && (
            <StatusItem
              variant="waiting"
              data-testid="needs-you-button"
              aria-label={`${count} needs you`}
              tooltip={`${count} needs you`}
              onPress={() => {
                setSurface("inbox");
                setUrl("at", "inbox");
              }}
              icon={<Hand aria-hidden="true" />}
              count={count}
            />
          )}
        </span>
      </header>
      <div className="tq-body">
        <ChatList w={w} />
        <main className="tq-main">
          <div className="tq-tabs" role="tablist" aria-label="Tabs">
            <span role="tab" aria-selected="true" className="tq-tab">
              <SquareTerminal aria-hidden="true" /> {front}
              {live && <Count variant="waiting">1</Count>}
            </span>
            <span role="tab" aria-selected="false" className="tq-tab">
              <SquareTerminal aria-hidden="true" /> steward
            </span>
          </div>
          <div className="tq-pane" data-testid="pane">
            {(inPane || (a && w.settled)) && (
              <div className="tq-asks" data-testid="asks" data-place="band">
                {inPane && surface === "strip" && <Strip w={w} act={act} surface={surface} />}
                {inPane && surface === "quiz" && <Quiz w={w} act={act} />}
                {a && w.settled && <Settled w={w} />}
              </div>
            )}
            <div className="tq-pane-head">
              <span className="ui-text-meta">claude · smart-ide{a?.reason.kind === "runner" ? " · on pool mac-ci" : ""}</span>
            </div>
            <div className="tq-terminal" data-testid="terminal">
              <div className="tq-terminal-lines">
                {"● Checking the last charge in Stripe's test mode.\n"}
                {`  ⎿  ${a?.run?.name ?? "purlis secret exec"} …\n`}
                {live ? "     waiting on you…" : "     …"}
              </div>
              <label className="tq-prompt">
                <span aria-hidden="true">›</span>
                <textarea
                  ref={inputRef}
                  data-testid="terminal-input"
                  aria-label="Terminal input"
                  rows={1}
                  value={w.typed}
                  onChange={(e) => {
                    const v = e.target.value;
                    act((d) => void (d.typed = v));
                  }}
                />
              </label>
            </div>
          </div>
        </main>
        {surface === "inbox" && (
          <aside className="tq-panel" aria-label="Inbox">
            <div className="tq-panel-head">
              <span className="ui-text-body ui-text-strong">Inbox</span>
              <IconButton
                size="sm"
                variant="ghost"
                aria-label="Close"
                tooltip="Close"
                onPress={() => {
                  setSurface("quiz");
                  setUrl("at", "quiz");
                }}
                icon={<X aria-hidden="true" />}
              />
            </div>
            <InboxPanel w={w} act={act} />
          </aside>
        )}
      </div>

      {!bare && (
        <div className="tq-bar" data-theme-keep="">
          <IconButton size="sm" variant="ghost" aria-label="Previous state" onPress={() => go(at - 1)} icon={<ChevronLeft aria-hidden="true" />} />
          <button type="button" className="tq-bar-name" onClick={() => setListing((x) => !x)}>
            <List aria-hidden="true" />
            <span>
              {at + 1}/{SCENES.length} · {scene.name}
              <small> · stories {scene.stories}</small>
            </span>
          </button>
          <IconButton size="sm" variant="ghost" aria-label="Next state" onPress={() => go(at + 1)} icon={<ChevronRight aria-hidden="true" />} />
          {scene.pretend && (
            <Button size="sm" onPress={() => act(scene.pretend!.run)}>
              <MessageCircleQuestion aria-hidden="true" /> {scene.pretend.label}
            </Button>
          )}
          <Button size="sm" variant="ghost" onPress={() => setW(scene.make())}>
            Reset
          </Button>
          <span className="tq-bar-end">
            <SegmentedControl
              size="sm"
              aria-label="Surface"
              value={surface}
              onChange={(s) => {
                setSurface(s as Surface);
                setUrl("at", s);
              }}
            >
              <Segment value="strip">Strip</Segment>
              <Segment value="quiz">Quiz</Segment>
              <Segment value="inbox">Inbox</Segment>
            </SegmentedControl>
            <SegmentedControl
              size="sm"
              aria-label="Appearance"
              value={look.appearance}
              onChange={(to) => onLook({ ...look, appearance: to as Look["appearance"] })}
            >
              <Segment value="light">Light</Segment>
              <Segment value="dark">Dark</Segment>
            </SegmentedControl>
          </span>
        </div>
      )}
      {listing && !bare && (
        <div className="tq-list" role="dialog" aria-label="Every state">
          <div className="tq-list-head">
            <strong>Every state</strong>
            <span>[ and ] move, \ closes this. Strip, Quiz or Inbox on the bar picks the surface.</span>
            <IconButton size="sm" variant="ghost" aria-label="Close" onPress={() => setListing(false)} icon={<X aria-hidden="true" />} />
          </div>
          <div className="tq-list-groups">
            <section>
              {SCENES.map((one, n) => (
                <button
                  key={one.id}
                  type="button"
                  data-here={n === at ? "" : undefined}
                  onClick={() => {
                    go(n);
                    setListing(false);
                  }}
                >
                  {one.name}
                </button>
              ))}
            </section>
          </div>
        </div>
      )}
    </div>
  );
}
