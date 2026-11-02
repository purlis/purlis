// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import {
  commands,
  type IdentitiesShown,
  type IdentityFinished,
  type IdentityRemoved,
  type IdentityRow,
  type PlaneId,
} from "./bindings";
import { AgentIdentityFlow, Out, WhoUses, answer, openPage, whyOf } from "./AgentIdentityFlow";
import { Notice } from "./Notice";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { Fact, Facts } from "./ui/card/Facts";
import { AlertDialog } from "./ui/dialog/Dialog";

/**
 * **Agent identity** (#4485, Q6 as amended by Q40): the view tab with a row per App, the place an
 * identity is inspected and removed. Setting one up is the flow Pool setup shares
 * ({@link AgentIdentityFlow}). What it shows is read fresh as it opens and after each press: the
 * project file's lines, the vault's key names, and GitHub.
 */

const said = (row: IdentityRow) =>
  row.acts_as === null
    ? `App ${row.app}`
    : `${row.acts_as} on ${row.repos ?? 0} ${row.repos === 1 ? "repo" : "repos"}`;

function Row({
  row,
  personas,
  busy,
  takeFocus,
  onRemove,
  onAdd,
  onOpenFailed,
}: {
  row: IdentityRow;
  personas: readonly string[];
  busy: boolean;
  takeFocus: boolean;
  onRemove: () => void;
  onAdd: (persona: string) => void;
  onOpenFailed: (why: string) => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (takeFocus) heading.current?.focus();
  }, [takeFocus]);
  const proved = row.acts_as !== null && row.trouble === null;
  return (
    <section className="agent-identity-row" aria-label={row.host}>
      <header className="agent-identity-row-head">
        <h2 ref={heading} tabIndex={-1} className="agent-identity-title">
          {row.host}
        </h2>
        {proved && <Badge variant="ok">Set up</Badge>}
      </header>
      <Facts aria-label={`The identity on ${row.host}`}>
        <Fact label="Acts as">{said(row)}</Fact>
        {row.by_hand && <Fact label="Who uses it">{row.personas.join(", ")}</Fact>}
      </Facts>
      {row.by_hand ? (
        <p className="agent-identity-term">
          This line was written by hand, so purlis doesn't check or remove it. To remove it, take it
          out of your project's settings.
        </p>
      ) : (
        <>
          <WhoUses
            personas={personas}
            ticked={row.personas}
            fixed={row.personas}
            off={busy}
            onChange={(to) => {
              const added = to.find((one) => !row.personas.includes(one));
              if (added !== undefined) onAdd(added);
            }}
          />
          <p className="agent-identity-term">Ticking another persona needs no second App.</p>
        </>
      )}
      {row.trouble && (
        <p className="agent-identity-term">GitHub couldn't confirm this. {row.trouble}</p>
      )}
      {(!row.by_hand || row.settings_page) && (
        <ButtonRow size="md">
          {!row.by_hand && (
            <Button
              variant="secondary"
              aria-disabled={busy || undefined}
              onPress={() => {
                if (!busy) onRemove();
              }}
            >
              Remove
            </Button>
          )}
          {row.settings_page && (
            <Button
              variant="ghost"
              icon={<Out />}
              onPress={() => openPage(row.settings_page ?? "", onOpenFailed)}
            >
              Open on GitHub
            </Button>
          )}
        </ButtonRow>
      )}
    </section>
  );
}

/** What removing did, and what remains on GitHub's side (Q13). */
function Removed({
  gone,
  onDismiss,
  onOpenFailed,
}: {
  gone: IdentityRemoved;
  onDismiss: () => void;
  onOpenFailed: (why: string) => void;
}) {
  const sentence =
    "The tool lines and the key are gone, and chats no longer act as this identity. The App itself stays on GitHub, with its installations, until you delete it there.";
  const page = gone.settings_page;
  return page ? (
    <Notice
      cause="agent-identity-removed"
      title="Removed from this project"
      onDismiss={onDismiss}
      fixes={[{ label: "Delete it on GitHub", onPress: () => openPage(page, onOpenFailed) }]}
    >
      {sentence}
    </Notice>
  ) : (
    <Notice cause="agent-identity-removed" title="Removed from this project" onDismiss={onDismiss}>
      {sentence}
    </Notice>
  );
}

/** What Finish proved (Q17). */
function Proved({ done, onDismiss }: { done: IdentityFinished; onDismiss: () => void }) {
  return (
    <Notice cause="agent-identity-proved" title="The agent has its identity" onDismiss={onDismiss}>
      purlis got a real short-lived token{done.read ? `, and read ${done.read} with it` : ""}. Chats
      won't ask about this again.
      {done.to_allow.length > 0 &&
        ` Give your Allow in Settings › Sandbox for ${done.to_allow.join(", ")}: this Allow covers more than these lines.`}
    </Notice>
  );
}

/** Asks before an identity goes (Q13). */
export function RemoveIdentity({
  host,
  onClose,
  onRemove,
}: {
  host: string;
  onClose: () => void;
  onRemove: () => void;
}) {
  return (
    <AlertDialog
      onClose={onClose}
      title={`Remove the identity on ${host}?`}
      description={
        <p className="came-back">
          Its tool lines, its key and its live tokens go at once, and chats stop acting as it. The
          App itself stays on GitHub until you delete it there.
        </p>
      }
      main={{ children: "Remove", variant: "danger", onPress: onRemove }}
    />
  );
}

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; shown: IdentitiesShown };

export function AgentIdentityTab({ plane, asking }: { plane: PlaneId; asking?: string }) {
  const [read, setRead] = useState<Read>({ state: "reading" });
  const [removing, setRemoving] = useState<IdentityRow | null>(null);
  const [busy, setBusy] = useState(false);
  const [gone, setGone] = useState<IdentityRemoved | null>(null);
  const [done, setDone] = useState<IdentityFinished | null>(null);
  const [trouble, setTrouble] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const again = useCallback(() => {
    void answer(commands.agentIdentities(plane)).then(
      (shown) => setRead({ state: "read", shown }),
      (why: unknown) => setRead({ state: "trouble", trouble: whyOf(why) }),
    );
  }, [plane]);
  useEffect(again, [again]);

  const finished = (made: IdentityFinished) => {
    setGone(null);
    setAdding(false);
    setDone(made);
    again();
  };

  const remove = (row: IdentityRow) => {
    setRemoving(null);
    setTrouble(null);
    setBusy(true);
    void answer(commands.agentIdentityRemove(plane, row.app))
      .then(
        (left) => {
          setGone(left);
          setDone(null);
        },
        (why: unknown) => setTrouble(whyOf(why)),
      )
      .finally(() => {
        setBusy(false);
        again();
      });
  };

  const add = (row: IdentityRow, persona: string) => {
    setTrouble(null);
    setBusy(true);
    void answer(commands.agentIdentityLines(row.app, [persona]))
      .then((lines) => answer(commands.agentIdentityFinish(plane, row.app, [persona], lines)))
      .then(finished, (why: unknown) => setTrouble(whyOf(why)))
      .finally(() => setBusy(false));
  };

  const shown = read.state === "read" ? read.shown : null;
  const setUp = shown?.rows.some((row) => !row.by_hand) === true;
  const flow = shown !== null && (!setUp || adding || shown.pending.length > 0);
  return (
    <div className="agent-identity" data-testid="agent-identity">
      {done && <Proved done={done} onDismiss={() => setDone(null)} />}
      {gone && <Removed gone={gone} onDismiss={() => setGone(null)} onOpenFailed={setTrouble} />}
      {trouble && (
        <Notice
          cause="agent-identity-action"
          tone="trouble"
          title="That didn't finish"
          onDismiss={() => setTrouble(null)}
        >
          {trouble}
        </Notice>
      )}
      {read.state === "reading" && <p role="status">Reading what's set up…</p>}
      {read.state === "trouble" && (
        <Notice
          cause="agent-identity-not-read"
          tone="trouble"
          fixes={[{ label: "Read again", onPress: again }]}
        >
          {read.trouble}
        </Notice>
      )}
      {shown?.rows.map((row, at) => (
        <Row
          key={`${row.app}-${at}`}
          row={row}
          personas={shown.personas}
          busy={busy}
          takeFocus={done !== null && done.row.app === row.app}
          onRemove={() => setRemoving(row)}
          onAdd={(persona) => add(row, persona)}
          onOpenFailed={setTrouble}
        />
      ))}
      {flow && shown && (
        <AgentIdentityFlow
          key={shown.pending[0]?.app ?? "new"}
          plane={plane}
          shown={shown}
          asking={asking}
          onDone={finished}
          onAbandoned={(left) => {
            setGone(left);
            setDone(null);
            again();
          }}
        />
      )}
      {shown && !flow && (
        <ButtonRow size="md">
          <Button variant="ghost" onPress={() => setAdding(true)}>
            Set up for another owner
          </Button>
        </ButtonRow>
      )}
      {removing && (
        <RemoveIdentity
          host={removing.host}
          onClose={() => setRemoving(null)}
          onRemove={() => remove(removing)}
        />
      )}
    </div>
  );
}
