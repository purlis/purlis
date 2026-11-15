// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState, type ReactNode } from "react";
import { commands, type PlaneId, type Shown } from "./bindings";
import { asksMoved } from "./asks";
import { CodeBlock } from "./ui/code/CodeBlock";
import { Dialog } from "./ui/dialog/Dialog";

/** The label of the press that shows a held merge's changes (R-302). */
export const VIEW_CHANGES = "View changes";

/**
 * **View changes on a held merge** (T1-41; R-302, R-122, Q277): the request's changes, read by
 * the core from the forge with the app's own login, never from a clone the chat can write, shown
 * here, and its head marked viewed, so its row says so. The core marks it only where it showed
 * the whole diff, while the forge says the request is at the head and base the row names; where
 * it isn't, or the forge can't give a diff, nothing is marked and its words say why. The changes
 * are the chat's own writing, so they are shown as text, never as markup.
 *
 * On the merge's pane and on its row in Asks alike: `view` is the press, `dialog` what it opens,
 * and `refused` why it showed nothing.
 */
export function useViewChanges(
  plane: PlaneId,
  ask: Shown,
): { view: () => void; busy: boolean; refused?: string; dialog: ReactNode } {
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  const [changes, setChanges] = useState<string>();
  const path = ask.answer;
  if (path.via !== "merge-hold") return { view: () => undefined, busy: false, dialog: null };

  const view = () => {
    if (busy) return;
    setBusy(true);
    setRefused(undefined);
    void commands
      .viewHeldMerge(plane, ask.session, path.repo, path.number, path.head)
      .then((read) => {
        if (read.status === "error") setRefused(String(read.error));
        else setChanges(read.data);
      })
      .catch((err: unknown) => setRefused(String(err)))
      .finally(() => {
        setBusy(false);
        // The row says the head is viewed once the list is read again.
        asksMoved(plane);
      });
  };

  const dialog =
    changes === undefined ? null : (
      <MergeChangesDialog
        number={path.number}
        repo={path.repo}
        head={path.head}
        base={path.facts.base}
        changes={changes}
        onClose={() => setChanges(undefined)}
      />
    );
  return { view, busy, refused, dialog };
}

/** **A held merge's changes**, as View changes shows them: at the row's head, into its base, as
 *  text the chat may have written. */
export function MergeChangesDialog({
  number,
  repo,
  head,
  base,
  changes,
  onClose,
}: {
  number: number;
  repo: string;
  head: string;
  base: string;
  changes: string;
  onClose: () => void;
}) {
  return (
    <Dialog
      title={`Changes of request ${number} of ${repo}`}
      size="wide"
      onClose={onClose}
      cancel={{ children: "Close" }}
    >
      <CodeBlock
        label={`At ${head.slice(0, 7)}, into ${base}`}
        text={changes === "" ? "No changes." : changes}
        copy="Copy changes"
      />
    </Dialog>
  );
}
