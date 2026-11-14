// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  commands,
  type ChatBlocks,
  type ChatLog,
  type ChatLogRow,
  type ChatLogSummary,
  type PlaneId,
} from "./bindings";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Disclosure } from "./ui/disclosure/Disclosure";
import { BlockTag } from "./BlockTag";
import {
  allowedSaid,
  atSaid,
  blockSaid,
  blockSourceSaid,
  blockTags,
  drawnRows,
  honestyLines,
  qualifiedDetail,
  rowSaid,
  type BlockRow,
  type QualifiedRow,
} from "./chatLog";
import { ToggleButton } from "./ui/button/Button";
import { useBlocksLookedAt, useChatLogFilter } from "./blockedMarkers";
import { BlockWaysOut } from "./BlockWaysOut";
import { TOLD, type Told } from "./blockWays";

/** **A block row's ways out** (T1-33, R-300): Tell the chat…'s typing and Go to chat. */
export type ChatLogWays = { tell: () => Promise<Told>; go: () => void };

/** What heads a log with nothing to say above its rows. */
const QUIET: ChatLogSummary = {
  allowed: 0,
  by_your_rules: 0,
  unverified: false,
  ran_as_you: false,
  harness: null,
  cant_tell_its_checks_blocks: false,
  cant_tell_its_mode: false,
};

/** What the reason is, above it: the words of the harness's own check, never purlis's. */
const REASON_OF = "The check's reason, as written";

/**
 * **A block row** (R-183, R-185 to R-188, R-340; T1-29): its action sentence, the tags of its
 * source beside it, and its detail, which always names the source and, while the app runs and
 * holds it, shows the harness's reason as written. The reason is the chat's side's text: drawn
 * as one text node, never parsed or marked up, and nowhere but the detail.
 */
function BlockedRow({ row, name, ways }: { row: BlockRow; name: string; ways?: ChatLogWays }) {
  const tags = blockTags(row);
  // The reason is in the page only while its detail is open: a closed panel is hidden, and still
  // a copy a find or a screen reader's browse could reach.
  const [open, setOpen] = useState(false);
  return (
    <>
      <Disclosure
        size="sm"
        isExpanded={open}
        onExpandedChange={setOpen}
        label={<span className="chat-log-said ui-text-body">{rowSaid(row)}</span>}
        trail={
          tags.length === 0 ? undefined : (
            <span className="chat-log-tags">
              {tags.map((tag) => (
                <BlockTag key={tag.kind} kind={tag.kind} word={tag.word} />
              ))}
            </span>
          )
        }
      >
        <p className="chat-log-source ui-text-secondary">{blockSourceSaid(row)}</p>
        {open && row.reason !== null && (
          <figure className="chat-log-reason-of">
            <figcaption className="ui-text-secondary">{REASON_OF}</figcaption>
            <bdi className="chat-log-reason ui-text-body">{row.reason}</bdi>
          </figure>
        )}
      </Disclosure>
      {/* Its ways out (T1-33, R-300): beside the row, never in its closed detail. */}
      {ways !== undefined && (
        <BlockWaysOut blocks={[row]} who={name} tell={ways.tell} go={ways.go} />
      )}
    </>
  );
}

/** The tag of a collapsed line, by its qualifier. */
const QUALIFIED_TAG = { unverified: "Unverified", cant_tell: "Can't tell" } as const;

/**
 * **A chat's unverified blocks, or those purlis can't tell more of, as one line** (R-104,
 * R-105; T1-38): their count, "12 unverified", and its tag; its detail lists the latest of them
 * in fixed words, who blocked a call and when, and counts the rest. Nothing the chat's side
 * wrote, its tool, its target or a harness's reason, is drawn. Its blocks' ways out (T1-33,
 * R-300) stand beside it, as a block row's do.
 */
function QualifiedLine({
  row,
  name,
  ways,
}: {
  row: QualifiedRow;
  name: string;
  ways?: ChatLogWays;
}) {
  const detail = qualifiedDetail(row);
  return (
    <>
      <Disclosure
        size="sm"
        label={<span className="chat-log-said ui-text-body">{rowSaid(row)}</span>}
        trail={
          <span className="chat-log-tags">
            <BlockTag kind={row.qualifier} word={QUALIFIED_TAG[row.qualifier]} />
          </span>
        }
      >
        <p className="chat-log-source ui-text-secondary">{detail.said}</p>
        <ol className="chat-log-qualified">
          {detail.listed.map((one, at) => (
            <li key={`${one.at ?? ""}:${at}`} className="ui-text-body">
              {[atSaid(one.at), blockSaid(one)].filter((part) => part !== "").join(" · ")}
            </li>
          ))}
        </ol>
        {detail.earlier !== null && (
          <p className="chat-log-source ui-text-secondary">{detail.earlier}</p>
        )}
      </Disclosure>
      {ways !== undefined && (
        <BlockWaysOut blocks={row.rows} who={name} tell={ways.tell} go={ways.go} />
      )}
    </>
  );
}

/**
 * **The chat's blocks, counted** (R-299): every block its log holds, the unverified ones apart,
 * which the sidebar never counts. Nothing where it has none.
 */
export function blocksCounted(counted: ChatBlocks | undefined): string | undefined {
  if (counted === undefined || counted.blocks + counted.unverified === 0) return undefined;
  const blocked = `${counted.blocks} blocked`;
  return counted.unverified === 0 ? blocked : `${blocked}, ${counted.unverified} unverified`;
}

/** The kinds of row a filter to blocks keeps: the block rows T1-29 draws (R-301). */
const BLOCK_ROWS: readonly ChatLogRow["kind"][] = ["blocked"];

/**
 * **The Chat log as it is drawn** (R-310, R-227, R-233; T1-28): the chat's own simple timeline,
 * apart from its transcript, one line per row, oldest first. Its first row is the start, its last
 * the end and its result; between them its protection changes, its blocks (T1-29), its asks with
 * their answers (one row each), its holds and its loop pauses. Above them, its honesty lines first
 * (R-235), then its allowed calls in one line (R-234). It only reads: a block opens its detail,
 * and nothing else in it is pressed in phase 1.
 */
export function ChatLogRows({
  name,
  rows,
  summary = QUIET,
  ways,
}: {
  name: string;
  rows: readonly ChatLogRow[];
  summary?: ChatLogSummary;
  /** A block row's ways out (T1-33); none where the chat is not one to tell or go to. */
  ways?: ChatLogWays;
}) {
  const allowed = allowedSaid(summary);
  return (
    <>
      {honestyLines(summary).map((line) => (
        <p key={line} className="chat-log-honesty ui-text-body">
          {line}
        </p>
      ))}
      {allowed !== null && <p className="chat-log-allowed ui-text-secondary">{allowed}</p>}
      <ol className="chat-log" aria-label={`Chat log of ${name}`}>
        {drawnRows(rows).map((row, at) => (
          <li
            key={`${row.kind}:${row.at ?? ""}:${at}`}
            className="chat-log-row"
            data-kind={row.kind}
          >
            <span className="chat-log-at ui-text-secondary">{atSaid(row.at)}</span>
            {row.kind === "blocked" ? (
              <BlockedRow row={row} name={name} ways={ways} />
            ) : row.kind === "qualified" ? (
              <QualifiedLine row={row} name={name} ways={ways} />
            ) : (
              <span className="chat-log-said ui-text-body">{rowSaid(row)}</span>
            )}
          </li>
        ))}
      </ol>
    </>
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
  onShowChat,
}: {
  plane: PlaneId;
  session?: number;
  name: string;
  /** Go to chat (T1-33, R-199): the chat in front. */
  onShowChat?: (session: number) => void;
}) {
  const [log, setLog] = useState<ChatLog>();
  const [said, setSaid] = useState<string>();
  // **Filtered to blocks where its marker opened it** (R-301), and the count of its blocks.
  const [filter, setFilter] = useChatLogFilter(plane, session);
  const [counted, setCounted] = useState<ChatBlocks>();
  // **Opened, its blocks are looked at** (R-175): its marker clears.
  useBlocksLookedAt(session);
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
    void commands
      .chatBlocks(plane, [{ session, after: null }])
      .then((done) => {
        if (gone || done.status === "error") return;
        setCounted(done.data.find((one) => one.session === session));
      })
      .catch(() => {});
    return () => {
      gone = true;
    };
  }, [plane, session]);

  // **Its block rows' ways out** (T1-33, R-300): Tell the chat… types through the core's one
  // way in, and answers why where nothing was typed.
  const ways: ChatLogWays | undefined =
    session === undefined
      ? undefined
      : {
          tell: () =>
            commands
              .tellChatBlocked(plane, session)
              .then((done): Told =>
                done.status === "error" ? { why: done.error } : { typed: TOLD },
              ),
          go: () => onShowChat?.(session),
        };
  if (said !== undefined)
    return <EmptyState headline="purlis could not read this chat's log" body={said} />;
  if (session === undefined || (log !== undefined && !log.known))
    return (
      <EmptyState headline="This chat is not open" body="Open it again to see its chat log." />
    );
  if (log === undefined) return <p className="chat-log-reading">Reading this chat&apos;s log…</p>;
  const blocksOnly = filter === "blocks";
  const rows = blocksOnly ? log.rows.filter((row) => BLOCK_ROWS.includes(row.kind)) : log.rows;
  const blocks = blocksCounted(counted);
  const head = (
    <div className="chat-log-head">
      <ToggleButton
        size="sm"
        isSelected={blocksOnly}
        onChange={(on) => setFilter(on ? "blocks" : "all")}
      >
        Blocks only
      </ToggleButton>
      {blocks !== undefined && <span className="ui-text-secondary">{blocks}</span>}
    </div>
  );
  if (blocksOnly)
    return (
      <>
        {head}
        {rows.length === 0 ? (
          <EmptyState
            headline="No block rows to show"
            body="Nothing was blocked in this chat on this machine."
          />
        ) : (
          <ChatLogRows name={name} rows={rows} ways={ways} />
        )}
      </>
    );
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
  return (
    <>
      {head}
      <ChatLogRows name={name} rows={log.rows} summary={log.summary} ways={ways} />
    </>
  );
}
