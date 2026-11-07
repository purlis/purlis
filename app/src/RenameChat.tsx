// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Field, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **Rename chat** (#4753, #5343): its Rename chat… row, from its sidebar row's menu, its pane's
 * ⋯, the palette, or F2 on its row or its pane. It says it renames the chat everywhere: the
 * list, its head, and any tab named for it, which follows unless the tab has a name of its own
 * (`tabRenaming.chatRenamed`). A tab's own Rename is `RenameTab`. Renaming never changes what
 * is on screen.
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
      description="This renames the chat everywhere: in the list, on its head and on a tab named for it."
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
