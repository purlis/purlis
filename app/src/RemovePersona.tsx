// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { AlertDialog } from "./ui/dialog/Dialog";

/**
 * Deleting a persona (SI-3): what goes, what stays, and one answer.
 *
 * **The core decides.** `persona_remove` is `charter persona remove` without `--force`: it
 * refuses a persona another one still `extends:` or `uses:`, naming each, and that refusal is
 * drawn here verbatim. The window offers no force — repointing the dependents first is the
 * repair the refusal names, and forcing past it leaves a dangling reference, which is a
 * terminal's decision to make.
 *
 * **No name typed back**, unlike a vault. A persona is a directory in the plane's git history, so
 * a delete is a change to commit, not a destruction; a vault's keychain entries are neither.
 *
 * The family's `AlertDialog`, for `DeleteWorkspace`'s reasons: it starts on Cancel, Escape is
 * Cancel, and a stray Return is never what deletes.
 */
export function RemovePersona({
  persona,
  trouble,
  deleting,
  onDelete,
  onCancel,
}: {
  persona: string;
  /** The last delete's refusal, in the core's words. */
  trouble?: string;
  /** Whether a delete is running right now, so the answer cannot be given twice. */
  deleting: boolean;
  onDelete: () => void;
  onCancel: () => void;
}) {
  return (
    <AlertDialog
      onClose={() => {
        if (!deleting) onCancel();
      }}
      title={`Delete persona ${persona}?`}
      description={
        <p className="came-back">
          This deletes <code>personas/{persona}/</code> — its definition, its memory and its
          reference files — and the agent purlis generated for it. Its vault is left alone. Commit
          the deletion to share it.
        </p>
      }
      main={{
        children: deleting ? "Deleting…" : "Delete persona",
        reserve: ["Delete persona", "Deleting…"],
        variant: "danger",
        isDisabled: deleting,
        onPress: onDelete,
      }}
    >
      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </AlertDialog>
  );
}
