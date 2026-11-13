// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What changed in one file** (FM-11, #1103 story 21): the file against the branch's base,
 * committed or not, in a view tab of its own — what the preview's *Show what changed* opens.
 *
 * The comparison is the core's one diff engine (RC-2, ADR 0084): `what_changed` answers both
 * sides and git's hunks, and the merge view draws the hunks it was handed and diffs nothing of
 * its own (`hunks.ts`). **Nothing is ever drawn as an empty diff**: a file the branch did not
 * change, a binary one, one past 2 MiB, one charter cannot read, or a path the core refuses is
 * said in a sentence.
 *
 * **It reads again when the branch moves** (#1189): the window's `branch-changed` for this
 * branch asks once more, one read at a time, and the comparison drawn stays until the new one
 * comes, marked busy with *Comparing … again…* said as a status; its *Open at line N* buttons
 * stay usable meanwhile (they land in the file tab, which reads the file as it is now). The tab
 * never asks the core to watch a branch (`branch_watch` sets the window's whole
 * watched set, which is the explorer's): it hears a branch the explorer watches, and a branch
 * nobody watches is compared again with *Compare again*.
 *
 * **A change opens in the file tab at its line** (#984): each hunk's first line on the head's
 * side is offered, and a press is the window's one jump to a file and line (`fileJump.ts`): the
 * branch's file tab picks the file and lands on the line. The path goes through the core's read
 * there, so one the branch does not offer is refused as the viewer refuses it. **And in your
 * editor at that line** (#1043): beside each, the same *Open in your editor* the header has,
 * handed the change's line instead of the first.
 *
 * **A hunk can be discarded** (#5406, ADR 0084 §8): each change's *Discard* puts it back as the
 * base has it, through the core's `discard_hunk`, and the hunk leaves at once, from the diff the
 * core answers with. A binary file's change goes whole, and a deleted file comes back. A chat
 * that is not idle in the folder is warned of by name and state, and the choice is the operator's;
 * a file that changed on disk since it was drawn offers Reload or Overwrite, and never merges.
 *
 * **Each sentence is a status** (#1189): the reading, the refusal and the change with no line in
 * it are said to a screen reader as they come, and the merge view is named after the file and
 * what it is compared against.
 *
 * **Against base or one turn** (#5405, #5408): the same view of one file across one turn that
 * ended on the branch ([`DiffAgainst`]), which the core reads from that turn's refs. It opens on
 * the latest turn, and the [`TurnPicker`] steps back through that chat's turns, newest first. No
 * turn to show, a turn that changed no file, and a file the turn left alone are each a sentence.
 *
 * **The branch's check results come with the comparison** (ADR 0117, #5407): each one on a line a
 * hunk shows is marked in the gutter, and every one is listed with its tool, severity and message,
 * so the keyboard and a screen reader reach them all (`CheckResults.tsx`).
 *
 * **On a turn, Discard puts a hunk back as the turn's start had it** (#5411), not the base, and a
 * file changed since the turn ended is drawn from the turn's start to the file now, which the
 * header says. The latest turn not yet undone offers [`UndoTurn`], which the core refuses while a
 * chat works in the folder, naming each one.
 */
import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { FileDiff as FileDiffMark, LoaderCircle } from "lucide-react";
import { EmptyState } from "../ui/empty-state/EmptyState";
import {
  commands,
  type BusyChat,
  type DiscardAgainst,
  type DiscardTarget,
  type PlaneId,
  type TurnChanged,
  type TurnPick,
  type WhatChanged,
} from "../bindings";
import type { Place } from "../pieceViews";
import { MergeViewer } from "./LightEditor";
import { DiffAgainst, TurnPicker, type Against } from "./DiffAgainst";
import { UndoTurn } from "./UndoTurn";
import { sized, ToYourEditor } from "./PieceFiles";
import { CheckResultsList, marksOf } from "./CheckResults";
import { jumpTo } from "../fileJump";
import { Notice } from "../Notice";
import { useBranchMoved } from "./branchMoved";
import { Button } from "../ui/button/Button";
import { AlertDialog } from "../ui/dialog/Dialog";
import { editorToolbar, ToolbarFile } from "../ui/tabs/Tabs";

/** The comparison, as the tab draws it: being read, refused with the core's sentence, or read. */
type Read =
  { kind: "reading" } | { kind: "refused"; why: string } | { kind: "read"; shown: WhatChanged };

/** The turn a read was of, the turns its chat has on the branch, newest first, and every path an
 *  undo would write where it is the turn offered. */
type OfTurn = { chat: string; number: number; turns: number[]; undo: string[] | null };

/** A discard the operator pressed, as it is asked again once they answer a warning. */
type Asked = {
  target: DiscardTarget;
  against: DiscardAgainst;
  head: string | null;
  busyToo: boolean;
};

/** What stands between a discard and the file: the operator's to answer. */
type Warning = (Asked & { kind: "busy"; chats: BusyChat[] }) | (Asked & { kind: "stale" });

/**
 * One file of a branch against the branch's base. *Compare again* reads it again, from
 * *Comparing…*; the branch moving reads it again behind the comparison drawn.
 */
export function PieceDiffTab({ plane, cut, path }: { plane: PlaneId; cut: Place; path: string }) {
  const [asked, setAsked] = useState(0);
  const [comparedTo, setComparedTo] = useState<Against>("base");
  /** The turn picked, or `null` for the latest: what one turn opens on, each time it is chosen. */
  const [pick, setPick] = useState<TurnPick | null>(null);
  /** The turn last drawn, so the picker holds its place while the next one is read. */
  const [ofTurn, setOfTurn] = useState<OfTurn>();
  const compareTo = (next: Against) => {
    if (next === "turn") {
      setPick(null);
      // The picker holds the newest it knows while the latest is read, never the turn left.
      setOfTurn((was) => was && { ...was, number: was.turns[0] ?? was.number });
    }
    setComparedTo(next);
  };
  /** How often the branch moved and was read again for it. */
  const [moved, setMoved] = useState(0);
  /** The last comparison told, with the asks and the moves it was read for. */
  const [read, setRead] = useState<{
    asked: number;
    moved: number;
    comparedTo: Against;
    pick: TurnPick | null;
    read: Read;
  }>();
  /** The read in flight, and whether the branch moved during it. */
  const flight = useRef<{ again: boolean }>(undefined);
  /** A discard on its way: the controls wait for it, and hold their places. */
  const [discarding, setDiscarding] = useState(false);
  const [warning, setWarning] = useState<Warning>();
  /** What the last discard could not do, or could not record. */
  const [trouble, setTrouble] = useState<string>();
  /** The chats the last edit made here is not told to, as a sentence ([`notTold`]). */
  const [untold, setUntold] = useState<string>();
  // The pick by its parts, so picking the turn already shown asks nothing again.
  const pickedChat = pick?.chat ?? null;
  const pickedNumber = pick?.number ?? null;
  useEffect(() => {
    let gone = false;
    const now = { again: false };
    const pick =
      pickedChat === null || pickedNumber === null
        ? null
        : { chat: pickedChat, number: pickedNumber };
    flight.current = now;
    const told = (got: Read) => {
      if (gone) return;
      flight.current = undefined;
      // A turn that did not read holds no Undo, and the picker holds the turn picked: never the
      // last turn read, whose number and files would be offered under the turn picked.
      if (comparedTo === "turn" && got.kind === "refused")
        setOfTurn((was) => was && { ...was, ...pick, undo: null });
      setRead({ asked, moved, comparedTo, pick, read: got });
      if (now.again) setMoved((n) => n + 1);
    };
    const asking =
      comparedTo === "base"
        ? commands
            .whatChanged(plane, cut.workspace, cut.repo, cut.piece, path)
            .then((answer) =>
              answer.status === "error"
                ? ({ kind: "refused", why: answer.error } as const)
                : ({ kind: "read", shown: answer.data } as const),
            )
        : commands
            .turnChanged(plane, cut.workspace, cut.repo, cut.piece, path, pick)
            .then((answer): Read => {
              if (answer.status === "error") return { kind: "refused", why: answer.error };
              const turn = answer.data;
              if (!gone)
                setOfTurn(
                  turn.kind === "of"
                    ? { chat: turn.chat, number: turn.number, turns: turn.turns, undo: turn.undo }
                    : undefined,
                );
              return turnRead(turn, path);
            });
    void asking.then(told).catch((err: unknown) => told({ kind: "refused", why: String(err) }));
    return () => {
      gone = true;
    };
  }, [
    plane,
    cut.workspace,
    cut.repo,
    cut.piece,
    path,
    asked,
    moved,
    comparedTo,
    pickedChat,
    pickedNumber,
  ]);
  useBranchMoved(plane, cut, () => {
    if (flight.current !== undefined) flight.current.again = true;
    else setMoved((n) => n + 1);
  });
  const current =
    read?.asked === asked &&
    read.comparedTo === comparedTo &&
    (comparedTo === "base" ||
      (read.pick?.chat === pick?.chat && read.pick?.number === pick?.number));
  const now: Read = current ? read.read : { kind: "reading" };
  // The branch moved since the comparison drawn was read, and it is being read again behind it.
  const again = current && read.moved !== moved;
  /** The turn drawn, which a discard puts a hunk back as its start had it; `null` on the base. */
  const onTurn: TurnPick | null =
    comparedTo === "turn" && ofTurn !== undefined
      ? { chat: ofTurn.chat, number: ofTurn.number }
      : null;
  const discard = (ask: Asked) => {
    setWarning(undefined);
    setTrouble(undefined);
    setUntold(undefined);
    setDiscarding(true);
    void commands
      .discardHunk(
        plane,
        cut.workspace,
        cut.repo,
        cut.piece,
        path,
        ask.target,
        ask.against,
        ask.busyToo,
        onTurn,
      )
      .then((answer) => {
        if (answer.status === "error") {
          setTrouble(answer.error);
          return;
        }
        const told = answer.data;
        if (told.kind === "busy") setWarning({ ...ask, kind: "busy", chats: told.chats });
        else if (told.kind === "stale") setWarning({ ...ask, kind: "stale" });
        else {
          if (told.now !== null)
            setRead({ asked, moved, comparedTo, pick, read: { kind: "read", shown: told.now } });
          else setAsked((n) => n + 1);
          if (told.unrecorded !== null)
            setTrouble(`Discarded, but it is not in the chat's history: ${told.unrecorded}`);
          setUntold(notTold(told.untold));
        }
      })
      .catch((err: unknown) => setTrouble(String(err)))
      .finally(() => setDiscarding(false));
  };
  const undo = (turn: TurnPick) => {
    setTrouble(undefined);
    setUntold(undefined);
    setDiscarding(true);
    void commands
      .undoTurn(plane, cut.workspace, cut.repo, cut.piece, turn)
      .then((answer) => {
        if (answer.status === "error") {
          setTrouble(answer.error);
          return;
        }
        const told = answer.data;
        if (told.kind === "busy") {
          const who = told.chats.map((chat) => `${chat.name} ${chat.state}`).join(", ");
          const until = told.chats.length === 1 ? "it stops" : "they stop";
          setTrouble(`Turn ${turn.number} was not undone, because ${who}. Undo it once ${until}.`);
          return;
        }
        setAsked((n) => n + 1);
        setUntold(notTold(told.untold));
        const unsaid = [told.unrecorded, told.unmarked].filter((why) => why !== null);
        if (unsaid.length > 0)
          setTrouble(`Turn ${turn.number} is undone, but ${unsaid.join(", and ")}`);
      })
      .catch((err: unknown) => setTrouble(String(err)))
      .finally(() => setDiscarding(false));
  };
  // Offered on the turn drawn, once it is read, never on the one the picker is leaving.
  const undoOffered =
    comparedTo === "turn" && current && ofTurn?.undo != null && onTurn !== null
      ? { turn: onTurn, files: ofTurn.undo }
      : null;
  return (
    <div className="piece-file" aria-busy={again}>
      <header {...editorToolbar("piece-files-head")}>
        {/* The tab is the file's name: the path alone, quiet (#3147). */}
        <ToolbarFile path={path} named={false} />
        {now.kind === "read" && <span>{against(now.shown)}</span>}
        {now.kind === "read" && now.shown.sinceTurnEnded && (
          <span>You changed this file since the turn ended.</span>
        )}
        <span className="piece-files-actions">
          <DiffAgainst value={comparedTo} onChange={compareTo} />
          {comparedTo === "turn" && ofTurn !== undefined && (
            <TurnPicker
              turns={ofTurn.turns}
              number={pick?.number ?? ofTurn.number}
              onPick={(number) =>
                // The newest is the latest, and follows a turn that ends after it.
                setPick(number === ofTurn.turns[0] ? null : { chat: ofTurn.chat, number })
              }
            />
          )}
          {undoOffered !== null && (
            <UndoTurn
              number={undoOffered.turn.number}
              files={undoOffered.files}
              undoing={discarding}
              onUndo={() => undo(undoOffered.turn)}
            />
          )}
          <ToYourEditor plane={plane} cut={cut} path={path} line={firstLine(now)} />
          <Button size="sm" onPress={() => setAsked((n) => n + 1)}>
            Compare again
          </Button>
        </span>
      </header>
      {again && (
        <p className="piece-files-trouble" role="status">
          {`Comparing ${path} again…`}
        </p>
      )}
      {trouble !== undefined && (
        <Notice
          cause={`discard-trouble:${path}`}
          tone="trouble"
          onDismiss={() => setTrouble(undefined)}
        >
          {trouble}
        </Notice>
      )}
      {untold !== undefined && (
        <Notice
          cause={`human-edit-untold:${path}`}
          tone="attention"
          onDismiss={() => setUntold(undefined)}
        >
          {untold}
        </Notice>
      )}
      <ChangedLines
        plane={plane}
        cut={cut}
        path={path}
        read={now}
        discarding={discarding}
        onDiscard={discard}
      />
      {now.kind === "read" && (
        <CheckResultsList plane={plane} cut={cut} path={path} results={now.shown.checks} />
      )}
      <Compared path={path} read={now} />
      {warning?.kind === "busy" && (
        <AlertDialog
          onClose={() => setWarning(undefined)}
          title={warning.chats.map((chat) => `${chat.name} ${chat.state}`).join(", ")}
          description={<p>{`Discarding now changes ${path} in the folder it works in.`}</p>}
          main={{
            children: "Discard anyway",
            variant: "danger",
            onPress: () => discard({ ...warning, busyToo: true }),
          }}
        />
      )}
      {warning?.kind === "stale" && (
        <AlertDialog
          onClose={() => setWarning(undefined)}
          title={`${path} changed on disk since this diff was drawn`}
          description={
            <p>
              Reload draws it again. Overwrite writes the file as drawn here, less this change, over
              what it holds now.
            </p>
          }
          cancel={{
            children: "Reload",
            onPress: () => {
              setWarning(undefined);
              setAsked((n) => n + 1);
            },
          }}
          main={{
            children: "Overwrite",
            variant: "danger",
            onPress: () =>
              discard({
                ...warning,
                against: { kind: "overwrite", head: warning.head },
              }),
          }}
        />
      )}
    </div>
  );
}

/**
 * **What the window says of the chats an edit is not told to** (#5477): a chat in the folder on a
 * harness purlis has no prompt hook into is never told of a Human edit at its next turn (ADR 0084
 * §8, as amended 2026-11-07), so the operator is, by the chat's name. `undefined` for none.
 */
export function notTold(chats: readonly string[]): string | undefined {
  if (chats.length === 0) return undefined;
  if (chats.length === 1)
    return (
      `${chats[0]} is not told of this change at its next turn, because purlis cannot add to ` +
      "its prompts. Tell it in its chat if it should know."
    );
  const names = `${chats.slice(0, -1).join(", ")} and ${chats[chats.length - 1]}`;
  return (
    `${names} are not told of this change at their next turn, because purlis cannot add to ` +
    "their prompts. Tell them in their chats if they should know."
  );
}

/** What the core said of one turn, as the tab draws it. */
function turnRead(turn: TurnChanged, path: string): Read {
  const name = path.slice(path.lastIndexOf("/") + 1);
  if (turn.kind === "no-turn") return { kind: "refused", why: turn.why };
  switch (turn.file.kind) {
    case "nothing":
      return { kind: "refused", why: `Turn ${turn.number} changed no file` };
    case "not-this-file":
      return { kind: "refused", why: `Turn ${turn.number} did not change ${name}` };
    case "changed":
      return { kind: "read", shown: turn.file.shown };
  }
}

/** How many changes are offered a line of their own: past it, the rest are counted, and each comes
 *  into the first ones as those are discarded. */
const LINES_OFFERED = 20;

/**
 * Each change's first line on the head's side, to open in the file tab (#984) or in your editor
 * at that line (#1043, through the header's own *Open in your editor*), and its *Discard* (#5406).
 * A file the branch deleted has no line on the head to land on, so only its *Discard*; so too a
 * binary file's change, which goes whole.
 */
function ChangedLines({
  plane,
  cut,
  path,
  read,
  discarding,
  onDiscard,
}: {
  plane: PlaneId;
  cut: Place;
  path: string;
  read: Read;
  discarding: boolean;
  onDiscard: (ask: Asked) => void;
}) {
  if (read.kind !== "read") return null;
  const { shown } = read;
  const { diff, stamp } = shown;
  const againstOpened: DiscardAgainst | null = stamp === null ? null : { kind: "opened", stamp };
  const discardButton = (label: string, target: DiscardTarget, head: string | null) =>
    againstOpened !== null && (
      <Button
        size="sm"
        aria-label={label}
        isDisabled={discarding}
        onPress={() => onDiscard({ target, against: againstOpened, head, busyToo: false })}
      >
        Discard
      </Button>
    );
  if (diff.kind === "binary")
    return (
      <nav className="piece-files-head" aria-label={`Changes in ${path}`}>
        <span>The whole change</span>
        <span className="piece-files-actions">
          {discardButton(`Discard the change to ${path}`, { kind: "whole" }, null)}
        </span>
      </nav>
    );
  if (diff.kind !== "text" || diff.hunks.length === 0) return null;
  const offered = diff.hunks.slice(0, LINES_OFFERED);
  const count = diff.hunks.length === 1 ? "1 change" : `${diff.hunks.length} changes`;
  return (
    <nav className="piece-files-head" aria-label={`Changes in ${path}`}>
      <span>
        {offered.length < diff.hunks.length ? `${count}, the first ${offered.length} here` : count}
      </span>
      <span className="piece-files-actions">
        {offered.map((hunk) => {
          const line = Math.max(1, hunk.newStart);
          const target: DiscardTarget = { kind: "hunk", hunk };
          return shown.mark === "deleted" ? (
            <Fragment key={hunk.oldStart}>
              {discardButton(`Discard the deletion of ${path}`, target, diff.head)}
            </Fragment>
          ) : (
            <Fragment key={`${hunk.oldStart}:${hunk.newStart}`}>
              <Button size="sm" onPress={() => jumpTo({ plane, place: cut, path, line })}>
                {`Open at line ${line}`}
              </Button>
              <ToYourEditor plane={plane} cut={cut} path={path} line={line} />
              {discardButton(`Discard the change at line ${line}`, target, diff.head)}
            </Fragment>
          );
        })}
      </span>
    </nav>
  );
}

/** The first line the change touches on the head's side, else the top: where *Open in your
 *  editor* opens the file. */
function firstLine(read: Read): number {
  if (read.kind !== "read" || read.shown.diff.kind !== "text") return 1;
  const first = read.shown.diff.hunks[0];
  if (first === undefined) return 1;
  return Math.max(1, first.newStart);
}

/** Where a sentence about a file the tab does not draw sends the operator. */
const TO_YOUR_EDITOR = "Open in your editor, at the top of this tab, shows it.";

/** The sentence for a change no line of which differs: what changed instead. */
function noLine(name: string, shown: WhatChanged, on: string): { headline: string; body?: string } {
  if (shown.mark === "renamed" && shown.from !== null)
    return { headline: `Only its name changed: moved from ${shown.from}` };
  if (shown.mark === "added") return { headline: `${name} was added, and it is empty` };
  if (shown.mark === "deleted") return { headline: `${name} was deleted, and it was empty` };
  if (shown.sinceTurnEnded) return { headline: `No line of ${name} differs from ${on}` };
  return {
    headline: `No line of ${name} differs from ${on}`,
    body: "What changed is not in its lines: its mode, or whether it is a link.",
  };
}

/** What the file is compared against, and where it came from when it moved. */
function against(shown: WhatChanged): string {
  const base = shown.base === null ? "its last commit" : shown.base;
  const moved = shown.from === null ? "" : `, moved from ${shown.from}`;
  return `against ${base}${moved}`;
}

/** The comparison drawn, or the sentence that says why it is not. */
function Compared({ path, read }: { path: string; read: Read }) {
  const marks = useMemo(() => (read.kind === "read" ? marksOf(read.shown.checks) : []), [read]);
  const name = path.slice(path.lastIndexOf("/") + 1);
  if (read.kind === "reading")
    return (
      <EmptyState
        mark={LoaderCircle}
        headline={`Comparing ${path}…`}
        variant="panel"
        role="status"
      />
    );
  if (read.kind === "refused")
    return (
      <EmptyState headline={read.why} variant="panel" role="status" testid="piece-diff-trouble" />
    );
  const { diff, base } = read.shown;
  const said = noLine(name, read.shown, base ?? "its last commit");
  switch (diff.kind) {
    case "binary":
      return (
        <EmptyState
          mark={FileDiffMark}
          headline={`${name} is a binary file, so its lines are not compared`}
          body={TO_YOUR_EDITOR}
          variant="panel"
          role="status"
          testid="piece-diff-trouble"
        />
      );
    case "too-large":
      return (
        <EmptyState
          mark={FileDiffMark}
          headline={`${name} is ${sized(diff.bytes)}, past what the comparison draws (2 MiB)`}
          body={TO_YOUR_EDITOR}
          variant="panel"
          role="status"
          testid="piece-diff-trouble"
        />
      );
    case "text":
      return diff.hunks.length === 0 ? (
        <EmptyState
          mark={FileDiffMark}
          headline={said.headline}
          body={said.body}
          variant="panel"
          role="status"
          testid="piece-diff-trouble"
        />
      ) : (
        <MergeViewer
          path={path}
          base={diff.base}
          head={diff.head}
          hunks={diff.hunks}
          marks={marks}
          label={`What changed in ${path} ${against(read.shown)}`}
        />
      );
  }
}
