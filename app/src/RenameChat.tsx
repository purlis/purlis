// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Field, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **Renaming a chat that has no tab of its own** (#4753): its Rename… row, from its sidebar
 * row's menu, its pane's ⋯ or the palette. A chat that is its tab's own is renamed in place on
 * the strip instead (`TabRename`), and renaming never changes what is on screen.
 *
 * **This dialog validates nothing**, for `RenameWorkspace`'s reason: the core trims the name,
 * gives a blank back as the default, and refuses a name it will not draw in its own words, which
 * come back here unchanged (`rename_chat`). A blank answer is allowed: it is how a chat's own
 * name comes back.
 */
export function RenameChat({
  name,
  trouble,
  renaming,
  onRename,
  onCancel,
}: {
  /** What the chat is called now. */
  name: string;
  /** Why the last attempt renamed nothing — **the core's sentence, unchanged**. */
  trouble?: string;
  /** Whether purlis is renaming it right now, so the answer cannot be given twice. */
  renaming: boolean;
  onRename: (name: string) => void;
  onCancel: () => void;
}) {
  const [typed, setTyped] = useState(name);
  const ready = typed.trim() !== name && !renaming;
  const rename = () => {
    if (ready) onRename(typed);
  };
  return (
    <Dialog
      onClose={onCancel}
      title={`Rename chat ${name}`}
      titleId="rename-chat"
      // The box, with its name chosen, so what is typed replaces it.
      initialFocus={(dialog) => {
        const box = dialog.querySelector("input");
        box?.select();
        return box;
      }}
      main={{
        children: renaming ? "Renaming…" : "Rename chat",
        reserve: ["Rename chat", "Renaming…"],
        isDisabled: !ready,
        onPress: rename,
      }}
    >
      <SettingRow
        label="New name"
        help="What purlis calls it here. Its harness keeps its own. Leave it empty for its own name."
        control={(ids) => <Field ids={ids} kind="text" value={typed} onChange={setTyped} />}
      />
      {/* Verbatim, beside the box: the operator is still answering. */}
      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}
