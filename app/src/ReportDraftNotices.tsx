// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import {
  commands,
  REPORT_DRAFTS_CHANGED,
  type DraftFiled,
  type FileHow,
  type PlaneId,
  type PossibleDuplicate,
  type ReportDraft,
} from "./bindings";
import { listen } from "./here";
import { Notice } from "./Notice";
import { Button, ButtonRow } from "./ui/button/Button";
import { TextArea, TextField } from "./ui/field/Field";
import { List, ListRow } from "./ui/list/List";
import { RowDescription, RowLabel, RowTrail } from "./ui/list/Row";

/** What the Read panel says first: whose words these are, and what was taken out. */
export const WRITTEN_BY_AN_AGENT = "Written by an agent, unreviewed. Paths and names taken out.";

/** What Dismiss does, beyond putting the card away. */
export const DISMISS_SAYS = "Lets the draft go. Nothing is sent.";

/** A draft the person filed in this window: what the Inbox's update says of it. */
export type SentDraft = {
  id: string;
  title: string;
  session: number | null;
  chat: string;
  /** `github` or `intake`. */
  to: string;
  /** When it left, in ms. */
  at: number;
};

/**
 * **Report drafts in the Inbox** (#5694, spec #5683, Q2/Q3): each report a chat drafted with
 * `purlis report`, as a card Notice in the Notices view, held there only while the person allows
 * Agent drafts. Its head names the chat it came from, with Go to chat; its sentence is the call
 * that failed, its exit and the versions. Read opens the whole scrubbed draft, marked as the
 * agent's and unreviewed, in fields the person can change. File files exactly what is shown: on
 * purlis/purlis as the person where they are signed in to GitHub, after the duplicate search, and
 * to purlis's report service where they are not. Dismiss lets it go, unsent.
 *
 * Each draft filed is handed to `onSent`, for the Inbox's update that says it was sent.
 */
export function ReportDraftNotices({
  plane,
  onGo,
  onSent,
}: {
  plane: PlaneId;
  onGo: (session: number) => void;
  onSent: (sent: SentDraft) => void;
}) {
  const [drafts, setDrafts] = useState<ReportDraft[]>([]);

  const read = useCallback(() => {
    void commands
      .reportDrafts(plane)
      .then((answer) => {
        if (answer.status === "ok") setDrafts(answer.data);
      })
      .catch(() => undefined);
  }, [plane]);

  useEffect(() => {
    let gone = false;
    read();
    const listening = listen(REPORT_DRAFTS_CHANGED, () => {
      if (!gone) read();
    }).catch(() => undefined);
    return () => {
      gone = true;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, [read]);

  const gone = (id: string) => setDrafts((now) => now.filter((one) => one.id !== id));

  return (
    <>
      {drafts.map((draft) => (
        <DraftNotice
          key={draft.id}
          draft={draft}
          onGo={onGo}
          onGone={() => gone(draft.id)}
          onEdited={(edited) =>
            setDrafts((now) => now.map((one) => (one.id === edited.id ? edited : one)))
          }
          onSent={onSent}
        />
      ))}
    </>
  );
}

/** What stands under a card: nothing, the draft to read, or the issues it may duplicate. */
type Under =
  { what: "nothing" } | { what: "read" } | { what: "duplicates"; issues: PossibleDuplicate[] };

function DraftNotice({
  draft,
  onGo,
  onGone,
  onEdited,
  onSent,
}: {
  draft: ReportDraft;
  onGo: (session: number) => void;
  onGone: () => void;
  onEdited: (edited: ReportDraft) => void;
  onSent: (sent: SentDraft) => void;
}) {
  const [under, setUnder] = useState<Under>({ what: "nothing" });
  const [title, setTitle] = useState(draft.title);
  const [body, setBody] = useState(draft.body);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | undefined>(undefined);
  const changed = title !== draft.title || body !== draft.body;
  const readId = `report-draft-read-${draft.id}`;

  /** The draft as shown: the person's own words kept first, where they changed any. */
  const shown = async (): Promise<ReportDraft | undefined> => {
    if (!changed) return draft;
    const kept = await commands.editReportDraft(draft.id, title, body);
    if (kept.status === "error") {
      setRefused(kept.error);
      return undefined;
    }
    onEdited(kept.data);
    return kept.data;
  };

  const file = (how: FileHow) => {
    if (busy) return;
    setBusy(true);
    setRefused(undefined);
    void shown()
      .then(async (now) => {
        if (now === undefined) return;
        const filed = await commands.fileReportDraft(now.id, now.digest, how);
        if (filed.status === "error") {
          setRefused(filed.error);
          return;
        }
        settle(now, filed.data);
      })
      .catch((err: unknown) => setRefused(String(err)))
      .finally(() => setBusy(false));
  };

  const settle = (now: ReportDraft, filed: DraftFiled) => {
    if (filed.kind === "duplicates") {
      setUnder({ what: "duplicates", issues: filed.issues });
      return;
    }
    onSent({
      id: now.id,
      title: filed.title || now.title,
      session: now.session,
      chat: now.chat,
      to: filed.to,
      at: Date.now(),
    });
    onGone();
  };

  const dismiss = () => {
    onGone();
    void commands.dismissReportDraft(draft.id).catch(() => undefined);
  };

  const reading = under.what === "read";
  const session = draft.session;
  return (
    <Notice
      cause={`report-draft:${draft.id}`}
      tone={refused === undefined ? "attention" : "trouble"}
      title={`Report draft: ${draft.title}`}
      from={draft.chat}
      fixes={[
        {
          label: "Read",
          disclosure: true,
          opens: { id: readId, open: reading },
          onPress: () => setUnder(reading ? { what: "nothing" } : { what: "read" }),
        },
        { label: "File", busy, onPress: () => file({ how: "check" }) },
        ...(session === null
          ? []
          : [{ label: "Go to chat", go: true, onPress: () => onGo(session) }]),
      ]}
      onDismiss={dismiss}
      dismissSays={DISMISS_SAYS}
      under={
        under.what === "nothing" ? undefined : (
          <div id={readId}>
            {under.what === "read" && (
              <>
                <p>{WRITTEN_BY_AN_AGENT}</p>
                <TextField label="Title" value={title} onChange={setTitle} />
                <TextArea label="Body" value={body} onChange={setBody} />
              </>
            )}
            {under.what === "duplicates" && (
              <>
                <p>These issues may already cover it. Add it to one, or file it as a new issue.</p>
                <List aria-label="Possible duplicates" items={under.issues}>
                  {(issue) => (
                    <ListRow
                      id={String(issue.number)}
                      textValue={`#${issue.number} ${issue.title}`}
                    >
                      <RowLabel>
                        #{issue.number} {issue.title}
                      </RowLabel>
                      <RowDescription>{issue.state === "open" ? "Open" : "Closed"}</RowDescription>
                      <RowTrail>
                        <Button
                          size="sm"
                          isDisabled={busy}
                          onPress={() => file({ how: "add-to", issue: issue.number })}
                        >
                          Add to #{issue.number}
                        </Button>
                      </RowTrail>
                    </ListRow>
                  )}
                </List>
                <ButtonRow size="sm">
                  <Button size="sm" isDisabled={busy} onPress={() => file({ how: "new" })}>
                    File as new
                  </Button>
                </ButtonRow>
              </>
            )}
          </div>
        )
      }
    >
      {/* A refusal is said in the card's own sentence, its live region, until the next press. */}
      {refused ?? draft.line}
    </Notice>
  );
}
