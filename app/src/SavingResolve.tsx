// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState } from "react";
import { commands, type PlaneId, type PlaneSaving, type SaveKey } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { Radio, RadioGroup } from "./ui/choice/Radio";
import { askWayOut, FIX_STARTING_SIZE, tellFixStarted, tellSaved } from "./saving";

/** What the conflict Notice is busy with, while a press runs. */
type Busy = "resolve" | "chat" | "continue" | "skip" | "abort";

/**
 * **A save's conflict, resolved from the window** (qw382; ADR 0051, noted 2026-10-14).
 *
 * - On a save that conflicted, *Resolve* has the window bring the remote's commits in, and where
 *   what a person wrote conflicts, cut a **resolve folder** off to the side (#5824 F7): the
 *   project itself never stops, and chats go on writing in it.
 * - While the resolve is open, the Notice names the folder and the files, offers a choice of
 *   value for each settings key both machines changed (#5852, never a marker in the file), and
 *   offers *Resolve in a chat* (a chat that edits the files in the folder and runs no git),
 *   *Continue* (the window checks no marker is left, commits the merge and moves the project onto
 *   it at once) and *Abort* (the folder goes, and nothing here changes). Settled as the remote has
 *   it, *Skip* drops this machine's side.
 * - When it finishes, what is committed is pushed and nothing new is committed: a file changed
 *   meanwhile is named and left for the person.
 * - A rebase or merge an older purlis left stopped in the project itself is offered *Abort*.
 *
 * Every git command is the window's own press: a chat may not rewrite the project root's tree,
 * and nothing it writes decides which files are staged or what runs.
 */
export function SaveConflict({
  plane,
  saving,
  onSaved,
}: {
  plane: PlaneId;
  saving: PlaneSaving;
  onSaved?: () => void;
}) {
  const [busy, setBusy] = useState<Busy>();
  const [said, setSaid] = useState<string>();
  /** The last Continue found the commit being replayed has nothing left to commit: Skip is
   *  offered, which drops it. */
  const [empty, setEmpty] = useState(false);
  /** The files git stopped in, as the last answer named them: Continue stages them with what
   *  git still calls unmerged, so a file settled and staged by hand is still checked. */
  const shown = useRef<string[]>([]);
  const sending = useRef(false);
  /** The person's choice for each settings key in conflict, by `file` and key. */
  const [picks, setPicks] = useState<Record<string, "ours" | "theirs">>({});

  const step = (which: "start" | "continue" | "skip" | "abort") => {
    if (sending.current) return;
    sending.current = true;
    setBusy(which === "start" ? "resolve" : which);
    setSaid(undefined);
    const files = [...new Set([...shown.current, ...saving.conflicts])];
    const keys = (saving.resolving?.keys ?? []).flatMap((one) => {
      const pick = picks[keyId(one)];
      return pick === undefined
        ? []
        : [{ file: one.file, key: one.key, theirs: pick === "theirs" }];
    });
    void commands
      .settleSaveConflict(plane, which, files, keys)
      .then((answer) => {
        if (answer.status === "error") {
          setSaid(answer.error);
          return;
        }
        const got = answer.data;
        setEmpty(got.outcome === "empty");
        if (got.outcome === "stopped") shown.current = got.files;
        if (got.outcome === "saved" || got.outcome === "aborted") {
          shown.current = [];
          setPicks({});
        }
        setSaid(got.said);
        if (got.outcome === "saved") onSaved?.();
      })
      .catch((err: unknown) => setSaid(String(err)))
      .finally(() => {
        sending.current = false;
        setBusy(undefined);
        tellSaved();
      });
  };

  const inAChat = () => {
    if (sending.current) return;
    sending.current = true;
    setBusy("chat");
    setSaid(undefined);
    void commands
      .resolveSaveInAChat(plane, FIX_STARTING_SIZE.columns, FIX_STARTING_SIZE.rows)
      .then((answer) => {
        if (answer.status === "ok") tellFixStarted(plane, answer.data);
        else setSaid(answer.error);
      })
      .catch((err: unknown) => setSaid(String(err)))
      .finally(() => {
        sending.current = false;
        setBusy(undefined);
      });
  };

  const files = saving.conflicts.length > 0 && (
    <ul className="saving-files" aria-label="Where it conflicts">
      {saving.conflicts.map((path) => {
        // The settings file is merged key by key: the keys both machines changed are named
        // beside it, and only the file is ever handed to Continue (#5824).
        const keys = saving.conflictKeys[path];
        return <li key={path}>{keys?.length ? `${path}: ${keys.join(", ")}` : path}</li>;
      })}
    </ul>
  );
  const outcome = said !== undefined && <span className="alert-said">{said}</span>;
  const terminal: NoticeAction = {
    label: "Open terminal here",
    onPress: () => askWayOut(plane, "terminal"),
  };

  if (saving.stoppedBy === "resolve") {
    const marked = saving.conflicts.filter(
      (path) => !(saving.resolving?.keys ?? []).some((one) => one.file === path),
    );
    const chat: NoticeAction = { label: "Resolve in a chat", onPress: inAChat, busy: !!busy };
    const carryOn: NoticeAction = {
      label: "Continue",
      onPress: () => step("continue"),
      busy: !!busy,
    };
    const abort: NoticeAction = { label: "Abort", onPress: () => step("abort"), busy: !!busy };
    const skip: NoticeAction = { label: "Skip", onPress: () => step("skip"), busy: !!busy };
    const keys = saving.resolving?.keys ?? [];
    const pick = (one: SaveKey, picked: "ours" | "theirs") =>
      setPicks((was) => ({ ...was, [keyId(one)]: picked }));
    const choices = (
      <>
        {files}
        {keys.map((one) => (
          <KeyChoice
            key={keyId(one)}
            choice={one}
            picked={picks[keyId(one)]}
            onPick={(picked) => pick(one, picked)}
          />
        ))}
      </>
    );
    return (
      <Notice
        cause="saving-resolving"
        tone="trouble"
        fixes={
          empty ? [skip, abort] : marked.length > 0 ? [chat, carryOn, abort] : [carryOn, abort]
        }
        link={terminal}
        under={choices}
      >
        {`These files conflict. Settle them in the resolve folder ${saving.resolving?.folder ?? ""}, in a chat or by hand, then Continue. Abort removes the folder and changes nothing here.`}
        {outcome}
      </Notice>
    );
  }
  if (saving.stoppedBy === "rebase" || saving.stoppedBy === "merge") {
    const stop = saving.stoppedBy === "merge" ? "merge" : "replay";
    const chat: NoticeAction = { label: "Resolve in a chat", onPress: inAChat, busy: !!busy };
    const carryOn: NoticeAction = {
      label: "Continue",
      onPress: () => step("continue"),
      busy: !!busy,
    };
    const abort: NoticeAction = { label: "Abort", onPress: () => step("abort"), busy: !!busy };
    const skip: NoticeAction = { label: "Skip", onPress: () => step("skip"), busy: !!busy };
    return (
      <Notice
        cause="saving-resolving"
        tone="trouble"
        fixes={
          empty
            ? [skip, abort]
            : saving.conflicts.length > 0
              ? [chat, carryOn, abort]
              : [carryOn, abort]
        }
        link={terminal}
        under={files || undefined}
      >
        {saving.conflicts.length > 0
          ? `The ${stop} is stopped on a conflict. Settle the files below, in a chat or by hand, then Continue. Abort puts the project back where it was.`
          : `The ${stop} is stopped, with nothing left in conflict. Continue carries it on, and Abort puts the project back where it was.`}
        {outcome}
      </Notice>
    );
  }
  if (saving.byHand == null) return null;
  const resolve: NoticeAction = {
    label: "Resolve",
    onPress: () => step("start"),
    busy: busy === "resolve",
  };
  return (
    <Notice
      cause="saving-rebase-by-hand"
      tone="trouble"
      fixes={[resolve]}
      link={terminal}
      under={files || undefined}
    >
      {`Resolve brings the remote's commits and this project's own together and stops where they conflict, for you or a chat to settle. ${saving.byHand}`}
      {outcome}
    </Notice>
  );
}

/** Which settings key a choice is for, as one string. */
function keyId(one: SaveKey): string {
  return `${one.file}\u0000${one.key}`;
}

/** A value as the choice says it: the TOML, or that the side removed the key. */
function valueSaid(value: string | null): string {
  return value ?? "removed";
}

/**
 * **One settings key both machines changed, chosen by its value** (#5852): this machine's or the
 * remote's, as the TOML says it. The file never holds a marker; Continue writes the choice.
 */
export function KeyChoice({
  choice,
  picked,
  onPick,
}: {
  choice: SaveKey;
  picked: "ours" | "theirs" | undefined;
  onPick: (pick: "ours" | "theirs") => void;
}) {
  return (
    <RadioGroup
      aria-label={`${choice.key} in ${choice.file}`}
      value={picked ?? null}
      onChange={(value) => onPick(value === "theirs" ? "theirs" : "ours")}
      className="saving-key"
    >
      <span className="saving-key-name">{`${choice.key} in ${choice.file}`}</span>
      <Radio value="ours">{`This machine's: ${valueSaid(choice.ours)}`}</Radio>
      <Radio value="theirs">{`The remote's: ${valueSaid(choice.theirs)}`}</Radio>
    </RadioGroup>
  );
}
