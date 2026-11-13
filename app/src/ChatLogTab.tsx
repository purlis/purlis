// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type ChatLog, type ChatLogRow, type PlaneId } from "./bindings";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { atSaid, rowSaid } from "./chatLog";

/**
 * **The Chat log as it is drawn** (R-310, R-227, R-233): the chat's own simple timeline, apart
 * from its transcript, one line per row, oldest first. Its first row is the start, its last the
 * end and its result. It only reads, and nothing in it is pressed in phase 1.
 */
export function ChatLogRows({ name, rows }: { name: string; rows: readonly ChatLogRow[] }) {
  return (
    <ol className="chat-log" aria-label={`Chat log of ${name}`}>
      {rows.map((row, at) => (
        <li key={`${row.kind}:${row.at ?? ""}:${at}`} className="chat-log-row" data-kind={row.kind}>
          <span className="chat-log-at ui-text-secondary">{atSaid(row.at)}</span>
          <span className="chat-log-said ui-text-body">{rowSaid(row)}</span>
        </li>
      ))}
    </ol>
  );
}

/**
 * **A chat's Chat log tab** (R-224, R-225): chat `session`'s timeline, read once from the core
 * as the tab opens. Pull-only: nothing heard later reads it again, so it never counts, marks or
 * announces a row.
 */
export function ChatLogTab({
  plane,
  session,
  name,
}: {
  plane: PlaneId;
  session?: number;
  name: string;
}) {
  const [log, setLog] = useState<ChatLog>();
  const [said, setSaid] = useState<string>();
  // The runner the chat runs on, as the open chats say it (`OpenChat.pool`): its log is kept
  // there, not here (Q247). `null` for a chat on this machine, or one not open.
  const [runner, setRunner] = useState<string | null>(null);
  useEffect(() => {
    if (session === undefined) return;
    let gone = false;
    void commands
      .openedChats(plane)
      .then((open) => {
        if (gone || open.status === "error") return;
        setRunner(open.data.find((one) => one.session === session)?.pool ?? null);
      })
      .catch(() => {});
    void commands
      .chatLog(plane, session)
      .then((done) => {
        if (gone) return;
        if (done.status === "error") setSaid(done.error);
        else setLog(done.data);
      })
      .catch((err: unknown) => !gone && setSaid(String(err)));
    return () => {
      gone = true;
    };
  }, [plane, session]);

  if (said !== undefined)
    return <EmptyState headline="purlis could not read this chat's log" body={said} />;
  if (session === undefined || (log !== undefined && !log.known))
    return (
      <EmptyState headline="This chat is not open" body="Open it again to see its chat log." />
    );
  if (log === undefined) return <p className="chat-log-reading">Reading this chat&apos;s log…</p>;
  if (log.rows.length === 0)
    return (
      <EmptyState
        headline="Nothing is logged for this chat on this machine"
        body={
          runner === null
            ? "This chat started before purlis kept this log, so it has none."
            : `This chat runs on ${runner}; its log is kept there.`
        }
      />
    );
  return <ChatLogRows name={name} rows={log.rows} />;
}
