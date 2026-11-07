// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Field, SettingRow } from "./settings/components";
import type { TabRenaming } from "./tabRenaming";
import { Checkbox } from "./ui/choice/Checkbox";
import { Dialog } from "./ui/dialog/Dialog";

export type RenameTabProps = {
  /** What the tab's Rename renames, from what it holds (`tabRenaming.renameOf`). */
  renames: TabRenaming;
  /** What the tab is called now (`tabNameOf`), which the box opens on. */
  name: string;
  /** What the tab's one chat is called now, which "Only this tab" keeps. */
  chat: string;
  /** Whether the tab has a name of its own (`Tab.own`), which can be cleared. */
  own: boolean;
  /** Why the last attempt renamed nothing: **the core's sentence, unchanged**. */
  trouble?: string;
  /** Whether purlis is renaming the chat now, so the answer cannot be given twice. */
  busy: boolean;
  /** `only` is whether the name is the tab's own, and the chat keeps its name. */
  onRename: (answer: { typed: string; only: boolean }) => void;
  /** Clears the tab's own name, so it is called by its first chat again. */
  onClear: () => void;
  onCancel: () => void;
};

/**
 * **A tab's Rename, which says what it renames** (#5343, spec #5339). A double-click on a tab,
 * F2 on it and its menu's Rename… open it; a view's tab has none.
 *
 * - **A tab with one chat renames the chat**, since the tab shows the chat's name, and the tab,
 *   the list and the head change together. **"Only this tab"** gives the tab a name of its own
 *   instead, and the chat keeps its name. A tab that has one opens ticked, on that name.
 * - **A split names the tab only**, and says the chats keep theirs: each pane's ⋯ renames its
 *   own chat (`RenameChat`).
 * - **A tab with a name of its own** can go back to its chat's: "Use the chat name again".
 *
 * A dialog, and no longer a box in the tab's place, because what it renames has to be said
 * before Enter, and a tick has no room on the strip. **Enter renames, Esc changes nothing**,
 * and the keyboard goes back to the tab, as the family's dialogs hand it back.
 *
 * **It validates nothing a chat's name needs**, for `RenameChat`'s reason: the core trims it,
 * a blank is the chat's default back, and a refusal comes back in its words. A tab's own name
 * has no default, so a blank one is not offered: "Use the chat name again" is how it goes.
 */
export function RenameTab({
  renames,
  name,
  chat,
  own,
  trouble,
  busy,
  onRename,
  onClear,
  onCancel,
}: RenameTabProps) {
  const [typed, setTyped] = useState(name);
  const split = renames.kind === "tab";
  const [ticked, setTicked] = useState(own);
  const only = split || ticked;
  const changed = typed.trim() !== name || (!split && ticked !== own);
  const ready = changed && !busy && (!only || typed.trim() !== "");
  const rename = () => {
    if (ready) onRename({ typed, only });
  };
  const verb = only ? "Rename tab" : "Rename chat";
  return (
    <Dialog
      onClose={onCancel}
      title={split ? `Rename tab ${name}` : `Rename ${name}`}
      titleId="rename-tab"
      description={
        split
          ? `This tab shows ${renames.panes} panes, so this names the tab only. The chats keep their names, and each pane's ⋯ renames its own.`
          : "This tab shows one chat, so this renames the chat: on its tab, in the list and on its head."
      }
      // The box, with its name chosen, so what is typed replaces it.
      initialFocus={(dialog) => {
        const box = dialog.querySelector<HTMLInputElement>('input:not([type="checkbox"])');
        box?.select();
        return box;
      }}
      alternative={own ? { children: "Use the chat name again", onPress: onClear } : undefined}
      main={{
        children: busy ? "Renaming…" : verb,
        reserve: ["Rename chat", "Rename tab", "Renaming…"],
        isDisabled: !ready,
        onPress: rename,
      }}
    >
      <SettingRow
        label="New name"
        control={(ids) => <Field ids={ids} kind="text" value={typed} onChange={setTyped} />}
      />
      {!split && (
        <Checkbox
          isSelected={ticked}
          onChange={setTicked}
          description={`The chat keeps its name, ${chat}.`}
        >
          Only this tab
        </Checkbox>
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
