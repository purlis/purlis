// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Field, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **New branch**, from a repo's row (GL-1, ADR 0072 §4).
 *
 * Cuts a branch of its own in the repo — a folder and a branch beside the repo's shared clone,
 * off whatever the clone has checked out — and makes it where the next chat starts. It starts
 * nothing: the next New tab's picker still asks.
 *
 * **It validates nothing**, for `NewWorkspace`'s reason: what a branch may be called is the
 * core's rule, reached through `worktree_add`, which is the path `charter worktree add` takes,
 * so the window refuses exactly what a terminal refuses and says the same sentence. A name left
 * empty is charter's next free `chat-<n>`; a name typed is used exactly, or refused when the
 * repo already has that branch.
 *
 * On screen it says branch and never the words charter keeps for its own internals (ADR 0072
 * §3): the folder is the branch's, and only where a path is needed is it shown.
 */
export function NewBranch({
  repo,
  trouble,
  making,
  onCut,
  onCancel,
}: {
  /** The repo the branch is cut in. */
  repo: string;
  /** Why the last attempt cut nothing — **the core's sentence, unchanged**. */
  trouble?: string;
  /** Whether charter is cutting it right now, so the answer cannot be given twice. */
  making: boolean;
  /** `branch` is the name typed, or `null` for charter's own. */
  onCut: (branch: string | null) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const cut = () => {
    if (!making) onCut(name.trim() === "" ? null : name.trim());
  };
  // The keyboard starts in the box, the dialog's first field; ⏎ there is Create branch.
  return (
    <Dialog
      onClose={onCancel}
      title="New branch"
      titleId="new-branch"
      // Where the branch is cut, said with the title (#1719).
      subtitle={
        <>
          In <code>{repo}</code>
        </>
      }
      main={{
        children: making ? "Creating…" : "Create branch",
        reserve: ["Create branch", "Creating…"],
        isDisabled: making,
        onPress: cut,
      }}
    >
      <SettingRow
        label="Name"
        help={
          <>
            Optional. Left empty, purlis names it chat-1, chat-2 and on. It is cut from what{" "}
            <code>{repo}</code> has checked out, in a folder of its own, and new chats start on it
            until you pick somewhere else.
          </>
        }
        control={(ids) => <Field ids={ids} kind="text" value={name} onChange={setName} />}
      />

      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}
