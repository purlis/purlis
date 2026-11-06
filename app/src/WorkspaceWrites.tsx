// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useState, type ReactNode } from "react";
import type { WorkspaceWrite } from "./actions";
import { commands, type PlaneId } from "./bindings";
import { Field, SettingRow } from "./settings/components";
import { touchSaid } from "./touching";
import { AlertDialog, Dialog } from "./ui/dialog/Dialog";

/**
 * **The person's own workspace files, written from Explorer** (#5376, ADR 0081 amended
 * 2026-11-05): what each write asks before it is made, and the making.
 *
 * - **New file… and New folder…** ask the name; **Rename…** asks the new one, its old one
 *   chosen in the box. A chat writing the entry right now is said first, and the rename waits
 *   for Rename anyway.
 * - **Delete…** asks once, saying it goes to the Trash, and which chat is writing it, if one is.
 * - **Import files…** asks nothing here: the system's file picker is its question.
 *
 * **This validates nothing**, for `RenameChat`'s reason: where a write may land, and what name
 * it may take, is the core's to say (`files::workspace`), and its sentence comes back here
 * unchanged, beside the box, while the person is still answering.
 */

/** The last step of `path`: what an entry is called in its folder. */
export function nameIn(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

/** Where a write stands while its dialog is up. */
export type Asking = {
  does: WorkspaceWrite;
  /** A rename of an entry a chat is writing: whether the person said Rename anyway. */
  goOn?: boolean;
  /** Why the last answer wrote nothing: the core's sentence, unchanged. */
  trouble?: string;
  busy: boolean;
};

/** The dialog a write asks with, for `asking`, or none when it asks nothing here (an import's
 *  picker is the system's). */
export function WorkspaceWriteDialog({
  asking,
  onName,
  onGoOn,
  onTrash,
  onClose,
}: {
  asking: Asking;
  onName: (name: string) => void;
  onGoOn: () => void;
  onTrash: () => void;
  onClose: () => void;
}) {
  const { does } = asking;
  switch (does.verb) {
    case "newWorkspaceEntry":
      return (
        <NameDialog
          title={does.folder ? "New folder" : "New file"}
          label={does.folder ? "Folder name" : "File name"}
          help={
            does.at.path === ""
              ? "Made at the top of the workspace's folder."
              : `Made in ${does.at.path}.`
          }
          act={does.folder ? "Create folder" : "Create file"}
          acting="Creating…"
          was=""
          asking={asking}
          onName={onName}
          onClose={onClose}
        />
      );
    case "renameWorkspaceEntry": {
      const name = nameIn(does.at.path);
      if (does.touchedBy.length > 0 && asking.goOn !== true)
        return (
          <AlertDialog
            onClose={onClose}
            title={`Rename ${name} while a chat works on it?`}
            description={
              <p>
                {touchSaid(does.touchedBy)}. A rename moves the file out from under it, and the chat
                may write it again under its old name.
              </p>
            }
            main={{ children: "Rename anyway…", onPress: onGoOn }}
          />
        );
      return (
        <NameDialog
          title={`Rename ${name}`}
          label="New name"
          help="It stays in the same folder."
          act={does.folder ? "Rename folder" : "Rename file"}
          acting="Renaming…"
          was={name}
          asking={asking}
          onName={onName}
          onClose={onClose}
        />
      );
    }
    case "trashWorkspaceEntry": {
      const name = nameIn(does.at.path);
      return (
        <AlertDialog
          onClose={onClose}
          title={`Delete ${name}?`}
          description={
            <>
              {does.touchedBy.length > 0 && (
                <p>{touchSaid(does.touchedBy)}, and the chat may write it again.</p>
              )}
              <p>
                {does.folder
                  ? `${name} and everything in it go to the Trash, where you can put them back.`
                  : `${name} goes to the Trash, where you can put it back.`}
              </p>
              {asking.trouble !== undefined && (
                <p className="trouble" role="alert">
                  {asking.trouble}
                </p>
              )}
            </>
          }
          main={{
            children: asking.busy ? "Deleting…" : "Move to Trash",
            reserve: ["Move to Trash", "Deleting…"],
            variant: "danger",
            isDisabled: asking.busy,
            onPress: onTrash,
          }}
        />
      );
    }
    case "importWorkspaceFiles":
      return asking.trouble === undefined ? null : (
        <AlertDialog
          onClose={onClose}
          title="Import files did not finish"
          description={
            <p className="trouble" role="alert">
              {asking.trouble}
            </p>
          }
          cancel={false}
          main={{ children: "Close", onPress: onClose }}
        />
      );
  }
}

/** The box a new entry's name, or a rename's, is typed in. */
function NameDialog({
  title,
  label,
  help,
  act,
  acting,
  was,
  asking,
  onName,
  onClose,
}: {
  title: string;
  label: string;
  help: string;
  act: string;
  acting: string;
  /** The name it has now: `""` for a new entry. */
  was: string;
  asking: Asking;
  onName: (name: string) => void;
  onClose: () => void;
}) {
  const [typed, setTyped] = useState(was);
  const ready = typed !== "" && typed !== was && !asking.busy;
  return (
    <Dialog
      onClose={onClose}
      title={title}
      // The box, its name up to its extension chosen, so what is typed replaces the name and
      // keeps the kind of file it is.
      initialFocus={(dialog) => {
        const box = dialog.querySelector("input");
        const dot = was.lastIndexOf(".");
        box?.setSelectionRange(0, dot > 0 ? dot : was.length);
        return box;
      }}
      main={{
        children: asking.busy ? acting : act,
        reserve: [act, acting],
        isDisabled: !ready,
        onPress: () => {
          if (ready) onName(typed);
        },
      }}
    >
      <SettingRow
        label={label}
        help={help}
        control={(ids) => <Field ids={ids} kind="text" value={typed} onChange={setTyped} />}
      />
      {asking.trouble !== undefined && (
        <p className="trouble" role="alert">
          {asking.trouble}
        </p>
      )}
    </Dialog>
  );
}

/**
 * **The writes, for a project's window**: `ask` takes a row's write and asks what it needs;
 * `dialog` is what to draw while it asks. `onWritten` hears each write made, by the path it
 * made, so the window can open a new file. The tree draws what changed by its own watch.
 */
export function useWorkspaceWrites(
  plane: PlaneId | undefined,
  onWritten?: (does: WorkspaceWrite, path: string) => void,
): { ask: (does: WorkspaceWrite) => void; dialog: ReactNode } {
  const [asking, setAsking] = useState<Asking>();
  const close = useCallback(() => setAsking(undefined), []);

  const run = useCallback(
    async (does: WorkspaceWrite, write: () => Promise<WriteAnswer>) => {
      if (plane === undefined) return;
      setAsking((was) => (was === undefined ? { does, busy: true } : { ...was, busy: true }));
      const answer = await write();
      if (answer.status === "error") {
        setAsking((was) => ({ ...(was ?? { does }), busy: false, trouble: answer.error }));
        return;
      }
      setAsking(undefined);
      for (const path of answer.data) onWritten?.(does, path);
    },
    [plane, onWritten],
  );

  const ask = useCallback(
    (does: WorkspaceWrite) => {
      if (plane === undefined) return;
      if (does.verb !== "importWorkspaceFiles") {
        setAsking({ does, busy: false });
        return;
      }
      // The system's file picker is the question; only a refusal is drawn here.
      void run(does, async () =>
        commands.workspaceFilesImport(plane, does.at.workspace, does.at.path),
      );
    },
    [plane, run],
  );

  const onName = (name: string) => {
    if (asking === undefined || plane === undefined) return;
    const { does } = asking;
    const ws = does.at.workspace;
    if (does.verb === "newWorkspaceEntry")
      void run(does, async () =>
        one(
          await (does.folder
            ? commands.workspaceFilesNewFolder(plane, ws, does.at.path, name)
            : commands.workspaceFilesNewFile(plane, ws, does.at.path, name)),
        ),
      );
    else if (does.verb === "renameWorkspaceEntry")
      void run(does, async () =>
        one(await commands.workspaceFilesRename(plane, ws, does.at.path, name)),
      );
  };

  const onTrash = () => {
    if (asking === undefined || plane === undefined) return;
    const { does } = asking;
    if (does.verb !== "trashWorkspaceEntry") return;
    void run(does, async () => {
      const answer = await commands.workspaceFilesTrash(plane, does.at.workspace, does.at.path);
      return answer.status === "error" ? answer : { status: "ok", data: [] };
    });
  };

  const dialog =
    asking === undefined ? null : (
      <WorkspaceWriteDialog
        // A new question is a new dialog, with a box of its own.
        key={`${asking.does.verb}:${asking.does.at.path}:${asking.goOn === true}`}
        asking={asking}
        onName={onName}
        onGoOn={() => setAsking((was) => was && { ...was, goOn: true })}
        onTrash={onTrash}
        onClose={close}
      />
    );
  return { ask, dialog };
}

/** What a write answers: the paths it made, or the core's refusal. */
type WriteAnswer = { status: "ok"; data: string[] } | { status: "error"; error: string };

/** One path made, as a list. */
function one(answer: { status: "ok"; data: string } | { status: "error"; error: string }) {
  return answer.status === "error"
    ? answer
    : ({ status: "ok", data: [answer.data] } satisfies WriteAnswer);
}
