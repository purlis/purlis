// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Field, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * Renaming a workspace (charter#367), asked where the answer is given.
 *
 * **This dialog validates nothing**, for `NewWorkspace`'s reason: what a workspace may be
 * called, whether the name is taken, and whether a chat is running in it are the core's
 * questions (`workspace_rename` → `wscmd::rename`, which is `charter workspace rename`), and the
 * core's sentence comes back here unchanged. The button asks only that the box holds a name
 * other than the one the workspace already has — which is not a rule about names, it is the
 * difference between a question answered and one that was not.
 */
export function RenameWorkspace({
  workspace,
  /** The core's sentence naming the chats that will start a fresh conversation after the
   *  rename (charter#367, D10), `null` for none, and `undefined` while it is being asked —
   *  the answer waits for it, so it is always read first. The rename still goes ahead. */
  startsFresh,
  /** Why the last attempt renamed nothing — **the core's sentence, unchanged**. */
  trouble,
  /** Whether charter is renaming it right now, so the answer cannot be given twice. */
  renaming,
  onRename,
  onCancel,
}: {
  workspace: string;
  startsFresh?: string | null;
  trouble?: string;
  renaming: boolean;
  onRename: (name: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(workspace);
  const next = name.trim();
  const ready = next !== "" && next !== workspace && !renaming && startsFresh !== undefined;
  const rename = () => {
    if (ready) onRename(next);
  };
  return (
    <Dialog
      onClose={onCancel}
      title={`Rename workspace ${workspace}`}
      titleId="rename-workspace"
      // The box, with its name chosen, so what is typed replaces it. The row draws the box, and
      // a setting's field takes no ref, so it is found in the dialog.
      initialFocus={(dialog) => {
        const box = dialog.querySelector("input");
        box?.select();
        return box;
      }}
      main={{
        children: renaming ? "Renaming…" : "Rename workspace",
        reserve: ["Rename workspace", "Renaming…"],
        isDisabled: !ready,
        onPress: rename,
      }}
    >
      <SettingRow
        label="New name"
        help={
          <>
            Its folder under <code>workspaces/</code> moves, its branches&apos; folders are
            repaired, and everything that names it follows. Not while a chat is running in it.
          </>
        }
        control={(ids) => <Field ids={ids} kind="text" value={name} onChange={setName} />}
      />
      {/* Rename waits for this answer, so say that it is being asked (#1719): a button greyed
          out with no reason reads as a dialog that refuses. */}
      {startsFresh === undefined && (
        <p className="pending" aria-busy="true">
          Asking which chats would start fresh…
        </p>
      )}
      {/* The core's sentence, unchanged, as `trouble` is. */}
      {startsFresh && (
        <p className="came-back" aria-label="Chats that will start fresh">
          {startsFresh}
        </p>
      )}
      {/* Verbatim, beside the box: the operator is still answering. */}
      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}
