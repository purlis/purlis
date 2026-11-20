// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useId, useRef, useState } from "react";
import {
  commands,
  type GitlabFinished,
  type GitlabRow,
  type GitlabShown,
  type PlaneId,
} from "./bindings";
import { WhoUses, answer, whyOf } from "./AgentIdentityFlow";
import { Notice } from "./Notice";
import { askSettingsLink, type SettingsLink } from "./settings/links";
import { KeyRound } from "lucide-react";
import { Badge } from "./ui/badge/Badge";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Button, ButtonRow } from "./ui/button/Button";
import { AlertDialog } from "./ui/dialog/Dialog";
import { TextField } from "./ui/field/Field";

/** Settings › You › Accounts, where one sign-in per forge host is made (ADR 0128). */
const ACCOUNTS: SettingsLink = { group: "you.accounts" };

/**
 * **GitLab access** (#1349): what lets a chat use `glab` where its sandbox holds your own sign-in
 * back. A GitLab token you make is kept in a persona's vault, and the persona's `glab` tool hands
 * it to the program: the chat never sees it. A row for each GitLab host the project's clones are
 * on, read fresh as the tab opens and after each press.
 *
 * **The token is a secret's box**: it is read from the field as Set up is pressed, wiped there,
 * and never in React's state.
 */

function Setup({
  plane,
  row,
  personas,
  onDone,
  onTrouble,
}: {
  plane: PlaneId;
  row: GitlabRow;
  personas: readonly string[];
  onDone: (done: GitlabFinished) => void;
  onTrouble: (why: string) => void;
}) {
  const box = useRef<HTMLInputElement>(null);
  const [ticked, setTicked] = useState<string[]>([]);
  const [filled, setFilled] = useState(false);
  const [busy, setBusy] = useState(false);
  const [lines, setLines] = useState<{ for: string[]; text: string } | null>(null);
  const linesId = useId();

  useEffect(() => {
    if (ticked.length === 0) return;
    let current = true;
    void answer(commands.gitlabAccessLines(plane, row.host, ticked)).then(
      (text) => {
        if (current) setLines({ for: ticked, text });
      },
      (why: unknown) => {
        if (current) onTrouble(whyOf(why));
      },
    );
    return () => {
      current = false;
    };
  }, [plane, row.host, ticked, onTrouble]);

  const shown = lines !== null && lines.for === ticked ? lines.text : null;
  const set = () => {
    if (shown === null) return;
    const token = box.current?.value ?? "";
    if (box.current) box.current.value = "";
    setFilled(false);
    setBusy(true);
    void answer(commands.gitlabAccessSet(plane, row.host, ticked, token, shown))
      .then(onDone, (why: unknown) => onTrouble(whyOf(why)))
      .finally(() => setBusy(false));
  };

  return (
    <div className="agent-identity-flow">
      <p className="agent-identity-lead">
        Make a project or group access token on {row.host} in the Developer role with the api scope,
        and paste it here. It acts as a bot of its own and can do no more than a Developer. The chat
        never sees the token: purlis keeps it in the persona's vault and hands it to glab.
      </p>
      {personas.length === 0 ? (
        <p className="agent-identity-term">
          This project has no personas yet. Add one from the palette, then come back here.
        </p>
      ) : (
        <WhoUses personas={personas} ticked={ticked} onChange={setTicked} off={busy} />
      )}
      <TextField
        type="password"
        label={`GitLab token for ${row.host}`}
        inputRef={box}
        onChange={setFilled}
      />
      {shown !== null && (
        <figure className="agent-identity-lines">
          <figcaption id={linesId}>The lines this adds to your project</figcaption>
          <pre tabIndex={0} role="group" aria-labelledby={linesId}>
            <code>{shown}</code>
          </pre>
        </figure>
      )}
      <ButtonRow>
        <Button onPress={set} isDisabled={busy || !filled || shown === null}>
          Set up GitLab access
        </Button>
      </ButtonRow>
    </div>
  );
}

function Row({
  plane,
  row,
  personas,
  busy,
  onRemove,
  onDone,
  onTrouble,
}: {
  plane: PlaneId;
  row: GitlabRow;
  personas: readonly string[];
  busy: boolean;
  onRemove: () => void;
  onDone: (done: GitlabFinished) => void;
  onTrouble: (why: string) => void;
}) {
  const setUp = row.personas.length > 0;
  return (
    <section className="agent-identity-row" aria-label={row.host}>
      <header className="agent-identity-row-head">
        <h2 className="agent-identity-title ui-text-title">{row.host}</h2>
        {setUp && <Badge variant="ok">Set up</Badge>}
      </header>
      {setUp && (
        <p className="agent-identity-say">
          Chats of {row.personas.join(", ")} use glab on this host.
        </p>
      )}
      {row.by_hand.length > 0 && (
        <p className="agent-identity-term">
          {row.by_hand.join(", ")} declare their own glab tool here, which purlis leaves alone.
        </p>
      )}
      <Setup plane={plane} row={row} personas={personas} onDone={onDone} onTrouble={onTrouble} />
      {setUp && (
        <ButtonRow>
          <Button variant="danger" isDisabled={busy} onPress={onRemove}>
            Remove
          </Button>
        </ButtonRow>
      )}
    </section>
  );
}

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; shown: GitlabShown };

export function RemoveGitlabAccess({
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
      title={`Remove GitLab access for ${host}?`}
      description={
        <p className="came-back ui-text-secondary">
          The token and the glab lines purlis wrote go at once, and chats stop using glab here.
          Revoke the token on GitLab too.
        </p>
      }
      main={{ children: "Remove", variant: "danger", onPress: onRemove }}
    />
  );
}

export function GitLabAccessTab({ plane }: { plane: PlaneId }) {
  const [read, setRead] = useState<Read>({ state: "reading" });
  const [removing, setRemoving] = useState<GitlabRow | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<GitlabFinished | null>(null);
  const [gone, setGone] = useState<string | null>(null);
  const [trouble, setTrouble] = useState<string | null>(null);
  const again = useCallback(() => {
    void answer(commands.gitlabAccessList(plane)).then(
      (shown) => setRead({ state: "read", shown }),
      (why: unknown) => setRead({ state: "trouble", trouble: whyOf(why) }),
    );
  }, [plane]);
  useEffect(again, [again]);

  const remove = (row: GitlabRow) => {
    setRemoving(null);
    setTrouble(null);
    setBusy(true);
    void answer(commands.gitlabAccessRemove(plane, row.host))
      .then(
        () => {
          setGone(row.host);
          setDone(null);
        },
        (why: unknown) => setTrouble(whyOf(why)),
      )
      .finally(() => {
        setBusy(false);
        again();
      });
  };

  const shown = read.state === "read" ? read.shown : null;
  return (
    <div className="agent-identity" data-testid="gitlab-access">
      <Notice
        cause="gitlab-access-one-login"
        title="One sign-in per forge host is coming"
        fixes={[{ label: "Open accounts", onPress: () => askSettingsLink(plane, ACCOUNTS) }]}
      >
        purlis is moving to your own GitLab sign-in, made once for each host, so a chat works as you
        and needs no token of its own. Make it in Settings › You › Accounts. A token set up here
        keeps working until then.
      </Notice>
      {done && (
        <Notice
          cause="gitlab-access-set-up"
          title="Chats can use glab here"
          onDismiss={() => setDone(null)}
        >
          The token is in the persona's vault, and its glab line is in your project.
          {done.to_allow.length > 0
            ? ` Give your Allow for ${done.to_allow.join(", ")} in Asks, under the ✋, or in Settings › Sandbox: this Allow covers more than these lines.`
            : ""}{" "}
          It takes effect when the chat restarts.
        </Notice>
      )}
      {gone && (
        <Notice cause="gitlab-access-removed" title="Removed" onDismiss={() => setGone(null)}>
          The token and the glab lines for {gone} are gone. The token itself stays valid on GitLab
          until you revoke it there.
        </Notice>
      )}
      {trouble && (
        <Notice
          cause="gitlab-access-action"
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
          cause="gitlab-access-not-read"
          tone="trouble"
          fixes={[{ label: "Read again", onPress: again }]}
        >
          {read.trouble}
        </Notice>
      )}
      {shown && shown.rows.length === 0 && (
        <EmptyState
          mark={KeyRound}
          headline="No GitLab remote in this project"
          body="Clone a project from GitLab, or declare a self-hosted one with a forge block in the project's settings, then come back."
          testid="gitlab-access-no-remote"
        />
      )}
      {shown?.rows.map((row) => (
        <Row
          key={row.host}
          plane={plane}
          row={row}
          personas={shown.personas}
          busy={busy}
          onRemove={() => setRemoving(row)}
          onDone={(made) => {
            setGone(null);
            setDone(made);
            again();
          }}
          onTrouble={setTrouble}
        />
      ))}
      {removing && (
        <RemoveGitlabAccess
          host={removing.host}
          onClose={() => setRemoving(null)}
          onRemove={() => remove(removing)}
        />
      )}
    </div>
  );
}
