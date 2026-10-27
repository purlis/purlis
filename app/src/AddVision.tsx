// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Field, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **Add a vision** (#3344): what a workspace is for, recorded in its `workspace.md` as
 * `purlis workspace vision "<the goal>"` records it, from the notice that says it has none.
 *
 * It validates nothing, for `NewWorkspace`'s reason: what a vision may hold is the core's
 * question (`workspace_vision` → `brokered::set_vision`), and its refusal comes back here, in
 * its own words, beside the box the person is still filling.
 */
export function AddVision({
  workspace,
  current,
  trouble,
  saving,
  onSave,
  onCancel,
}: {
  workspace: string;
  /** The vision recorded now, `""` for none: shown in the box, since a Save replaces it (a
   *  result kept under Results still offers Add a vision after one was recorded). */
  current: string;
  /** Why the last Save recorded nothing: the core's sentence, unchanged. */
  trouble?: string;
  saving: boolean;
  onSave: (vision: string) => void;
  onCancel: () => void;
}) {
  const [vision, setVision] = useState(current);
  const had = current.trim() !== "";
  const ready = vision.trim() !== "" && vision.trim() !== current.trim() && !saving;
  return (
    <Dialog
      onClose={onCancel}
      title={had ? `Change the vision of ${workspace}` : `Add a vision for ${workspace}`}
      titleId="add-vision"
      initialFocus={(dialog) => dialog.querySelector("textarea")}
      main={{
        children: saving ? "Saving…" : "Save vision",
        reserve: ["Save vision", "Saving…"],
        isDisabled: !ready,
        onPress: () => {
          if (ready) onSave(vision);
        },
      }}
    >
      <SettingRow
        label="What it is for"
        help={
          <>
            Recorded in <code>workspace.md</code>, the living charter its chats read and a fork
            inherits.{had && " It replaces the vision recorded now."}
          </>
        }
        control={(ids) => (
          <Field ids={ids} kind="list" minRows={3} value={vision} onChange={setVision} />
        )}
      />
      {/* Verbatim, beside the box: the person is still answering. */}
      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}
