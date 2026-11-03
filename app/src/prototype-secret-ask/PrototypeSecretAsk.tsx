// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (spec #5054, ADR 0109, #5070). The Secret ask's body, drawn inside the ask
 * surfaces #4561 settled: the pinned strip above the input, the top-tab quiz card and the Inbox
 * card. One body, three hosts. The window around it is a stand-in; the dashed bar at the bottom is
 * the prototype's, not the design.
 *
 * URL: `?s=<state>` the state, `&at=strip|quiz|inbox` the surface, `&closed` the strip folded,
 * `&bare` no bar.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Hand, KeyRound, Link2, List, MessageCircleQuestion, SquareTerminal, X } from "lucide-react";
import { KindMark } from "../InboxRow";
import { Badge, Count } from "../ui/badge/Badge";
import { Button, IconButton } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { Segment, SegmentedControl } from "../ui/choice/Radio";
import { TextField } from "../ui/field/Field";
import { Select } from "../ui/select/Select";
import { StatusItem } from "../ui/status/StatusItem";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { Twist } from "../ui/disclosure/Disclosure";
import { ENTRIES, SCENES, VAULTS, needsCount, type Ask, type World } from "./model";
import "./frame.css";
import "./secret-ask.css";

type Act = (run: (w: World) => void) => void;
type Surface = "strip" | "quiz" | "inbox";
const SURFACES: Surface[] = ["strip", "quiz", "inbox"];

const ageSaid = (m: number) => (m < 1 ? "now" : `${m}m`);
const path = (vault: string, key: string) => `${vault}/${key}`;

/** The one line every surface leads with. */
const headline = (a: Ask) => `${a.chat} needs a secret: ${a.key}`;

/** Why it is asked again, or what is wrong with what is there. */
function Reason({ a }: { a: Ask }) {
  const r = a.reason;
  if (r.kind === "replace")
    return (
      <div className="sa-reason" data-testid="sa-reason">
        <p>
          The chat says the current value doesn't work: <q className="sa-chat-words">{r.said}</q>
        </p>
        {r.linkedTo && (
          <p className="ui-text-secondary">
            <Link2 aria-hidden="true" className="sa-inline-mark" /> {a.key} reads{" "}
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
          <Link2 aria-hidden="true" className="sa-inline-mark" /> {a.key} was linked to{" "}
          <code className="sa-key">{r.was}</code>, which is gone. Pick another entry or store a value.
        </p>
      </div>
    );
  if (r.kind === "runner")
    return (
      <div className="sa-reason" data-testid="sa-reason">
        <p>
          {a.chat} runs on {r.runner}, which can't receive secrets. You can still store it here for
          chats on this machine; {a.chat} hears it was declined because its runner can't receive it.
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

/** Why it is needed: the persona's committed words, then the chat's own, each marked as whose. */
function Purpose({ a }: { a: Ask }) {
  return (
    <div className="sa-purpose">
      {a.declared && (
        <p className="sa-said" data-testid="sa-declared">
          <span className="sa-who ui-text-meta">{a.persona} persona</span>
          <span>{a.declared}</span>
        </p>
      )}
      {a.purpose && (
        <p className="sa-said sa-said-chat" data-testid="sa-chat-purpose">
          <span className="sa-who ui-text-meta">{a.chat} says</span>
          <q className="sa-chat-words">{a.purpose}</q>
        </p>
      )}
      {a.raisedBy === "broker" && a.command && (
        <p className="sa-quiet ui-text-secondary">
          <code className="sa-key">{a.command}</code> is waiting for it.
        </p>
      )}
    </div>
  );
}

/** Where the fill lands: the key and vault, with Change to rename or move it. */
function Target({ w, act }: { w: World; act: Act }) {
  const a = w.ask!;
  if (!w.changing)
    return (
      <p className="sa-target">
        <KeyRound aria-hidden="true" className="sa-inline-mark" />
        <code className="sa-key sa-key-strong">{path(w.vaultAs, w.keyAs)}</code>
        {!a.declared && (
          <Tooltip tip={`${a.persona} doesn't list ${a.key} among the secrets it needs. The chat asked on its own.`}>
            <Badge variant="neutral" data-testid="sa-undeclared">
              not declared by this persona
            </Badge>
          </Tooltip>
        )}
        <Button size="sm" variant="link" onPress={() => act((d) => void (d.changing = true))}>
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
        value={w.keyAs}
        onChange={(to) => act((d) => void (d.keyAs = to))}
        spellCheck={false}
        autoComplete="off"
        help={w.keyAs !== a.key ? `${a.chat} hears the new name, ${w.keyAs}.` : undefined}
      />
      </div>
      <div className="sa-change-vault">
        <span className="sa-label ui-text-meta">Vault</span>
        <Select
          size="sm"
          aria-label="Vault"
          value={w.vaultAs}
          onValueChange={(to) => act((d) => void (d.vaultAs = to))}
          options={[...new Set([a.vault, ...VAULTS])].map((v) => ({
            value: v,
            label: v === a.vault ? `${v} (${a.persona}'s vault)` : v,
          }))}
        />
      </div>
      <Button size="sm" variant="ghost" onPress={() => act((d) => void (d.changing = false))}>
        Done
      </Button>
      {!a.declared && <Badge variant="neutral">not declared by this persona</Badge>}
    </div>
  );
}

/** New value or an entry the person keeps already. */
function Fill({ w, act, surface }: { w: World; act: Act; surface: Surface }) {
  const a = w.ask!;
  const to = path(w.vaultAs, w.keyAs);
  const settle = (how: "value" | "link") =>
    act((d) => {
      d.settled = { kind: "filled", how, vault: d.vaultAs, key: d.keyAs, to: how === "link" ? d.entry : undefined };
      if (!a.declared) d.needOffered = "open";
    });
  const replacing = a.reason.kind === "replace";
  const linkedTo = a.reason.kind === "replace" ? a.reason.linkedTo : undefined;
  return (
    <div className="sa-fill">
      <SegmentedControl
        size="sm"
        aria-label="How to fill it"
        value={w.mode}
        onChange={(m) => act((d) => void (d.mode = m as World["mode"]))}
      >
        <Segment value="value">New value</Segment>
        <Segment value="entry">Use an entry I have</Segment>
      </SegmentedControl>
      {w.mode === "value" ? (
        <div className="sa-row">
          <TextField
            key={`${surface}-value`}
            size="sm"
            type="password"
            className="sa-grow"
            aria-label={`Value for ${to}`}
            placeholder="Paste or type the value"
            autoComplete="off"
            onChange={(filled) => act((d) => void (d.valueFilled = filled))}
            help={
              linkedTo
                ? `Stored in ${linkedTo}, which ${a.key} reads.`
                : `Stored in your ${w.vaultAs} vault, on this machine. Never shown to the chat.`
            }
          />
          <Button size="sm" variant="primary" isDisabled={!w.valueFilled} onPress={() => settle("value")}>
            {replacing ? "Replace" : "Store"}
          </Button>
        </div>
      ) : (
        <div className="sa-row">
          <div className="sa-grow sa-col">
            <Select
              size="sm"
              aria-label="Entry"
              placeholder="Pick an entry from your vaults"
              value={w.entry}
              filtered
              onValueChange={(e) => act((d) => void (d.entry = e))}
              options={ENTRIES.map((e) => ({ value: path(e.vault, e.key), label: path(e.vault, e.key) }))}
            />
            <span className="ui-text-meta sa-help">
              {w.entry
                ? `${to} becomes a link to ${w.entry}: the value isn't copied, and a rotation of ${w.entry} reaches it.`
                : "The value isn't copied: the key reads the entry, so a rotation reaches it."}
            </span>
          </div>
          <Button size="sm" variant="primary" isDisabled={!w.entry} onPress={() => settle("link")}>
            Link
          </Button>
        </div>
      )}
    </div>
  );
}

/** The body: the same in the strip, the quiz and the Inbox. */
function Body({ w, act, surface }: { w: World; act: Act; surface: Surface }) {
  const a = w.ask!;
  return (
    <div className="sa-body" data-testid="sa-body" data-says={headline(a)}>
      <Target w={w} act={act} />
      <Reason a={a} />
      <Purpose a={a} />
      <Fill w={w} act={act} surface={surface} />
      <div className="tq-foot">
        <span className="ui-text-meta">
          {a.waiting.length > 1 ? "The chats hear" : `${a.chat} hears`} filled or declined, and the key's name. Never the value.
        </span>
        <Button size="sm" variant="ghost" onPress={() => act((d) => void (d.settled = { kind: "declined" }))}>
          Not now
        </Button>
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

/** What is left once it is answered: one quiet line, and the Secret need offer after an undeclared fill. */
function Settled({ w, act }: { w: World; act: Act }) {
  const a = w.ask!;
  const s = w.settled!;
  return (
    <div className="sa-settled">
      <div className="tq-line" data-testid="sa-settled">
        <KindMark kind="update" />
        <p>
          {s.kind === "declined"
            ? `Not now. ${a.chat} heard it was declined, and won't ask for ${a.key} again.`
            : s.how === "link"
              ? `${path(s.vault, s.key)} now reads ${s.to}. ${a.chat} heard: filled, as ${s.key}.`
              : `Stored ${path(s.vault, s.key)}. ${a.chat} heard: filled, as ${s.key}.`}
        </p>
      </div>
      {w.needOffered === "open" && (
        <div className="tq-line sa-offer" data-testid="sa-need-offer">
          <p>
            Add {s.kind === "filled" ? s.key : a.key} to the {a.persona} persona's secret needs, so a teammate's chat asks for it
            by name? Only the key and its purpose go in the persona, never your entry.
          </p>
          <Button size="sm" onPress={() => act((d) => void (d.needOffered = "proposed"))}>
            Propose the change
          </Button>
          <Button size="sm" variant="ghost" onPress={() => act((d) => void (d.needOffered = "no"))}>
            No
          </Button>
        </div>
      )}
      {w.needOffered === "proposed" && (
        <div className="tq-line">
          <p>Proposed: {a.persona} needs {a.key}. It waits in Changes for your approval.</p>
        </div>
      )}
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
          {w.open ? "Fold" : "Fill"}
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
    if (params().has("closed")) made.open = false;
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
    <div className="tq-root" data-bare={bare ? "" : undefined} data-needs={count} data-scene={scene.id} data-surface={surface}>
      <header className="tq-title">
        <span className="ui-text-body ui-text-strong">charter · smart-ide</span>
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
                {a && w.settled && <Settled w={w} act={act} />}
              </div>
            )}
            <div className="tq-pane-head">
              <span className="ui-text-meta">claude · smart-ide{a?.reason.kind === "runner" ? " · on pool mac-ci" : ""}</span>
            </div>
            <div className="tq-terminal" data-testid="terminal">
              <div className="tq-terminal-lines">
                {"● Running the checkout suite against Stripe test mode.\n"}
                {"  ⎿  purlis secret exec -- pnpm e2e:checkout\n"}
                {live ? `     waiting on you for ${path(a.vault, a.key)}…` : "     …"}
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
