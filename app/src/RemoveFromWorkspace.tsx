// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type PlaneId } from "./bindings";
import { forgetFailedClone } from "./repoClones";
import { AlertDialog } from "./ui/dialog/Dialog";

/**
 * **Take a repo the workspace names, and this machine has not cloned, out of the workspace**
 * (#1228): a yes before anything is written. Asked from the explorer's "Not cloned here" row,
 * its menu, the bottom bar's menu, the palette, and Settings › Workspace › Repos — one
 * question, so each says the same thing.
 *
 * Only the repo's row in `workspace.json` goes (`drop_repo_membership`). Nothing is deleted,
 * because nothing is here to delete, and the question says so. It is still a question: the
 * workspace stops naming the repo for everyone who shares it, and there is no Undo. A cloned
 * repo is never asked about here — its removal deletes the clone and goes through
 * `drop_repo`'s guard — and the core refuses one that got cloned in between, in its own words.
 */
export function RemoveFromWorkspace({
  plane,
  workspace,
  repo,
  onClose,
  onDone,
}: {
  plane: PlaneId;
  workspace: string;
  repo: string;
  onClose: () => void;
  /** It is out of the workspace: what the core said about it. */
  onDone: (said: string[]) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [trouble, setTrouble] = useState<string | null>(null);

  const confirm = async () => {
    setBusy(true);
    setTrouble(null);
    try {
      const got = await commands.dropRepoMembership(plane, workspace, repo);
      if (got.status === "ok") {
        // Named no more, a failure of it would otherwise stay as a row of its own (#4395).
        forgetFailedClone(plane, workspace, repo);
        onDone(got.data);
      } else setTrouble(got.error);
    } catch (err: unknown) {
      setTrouble(String(err));
    } finally {
      setBusy(false);
    }
  };

  // What had the keyboard when the question opened — the Remove… that asked it — gets it back
  // when the question closes, if it is still there: the dialog's own return.
  return (
    <AlertDialog
      onClose={() => {
        // Not while the core is at work, `NewVault`'s reason: a refusal would land where
        // nobody is looking (#630).
        if (!busy) onClose();
      }}
      title={`Remove ${repo} from ${workspace}?`}
      description={
        <p className="came-back">
          {`${workspace} stops naming ${repo}. Nothing is deleted: it is not cloned on this machine.`}
        </p>
      }
      cancel={{ isDisabled: busy, onPress: onClose }}
      main={{
        children: busy ? "Removing…" : "Remove from workspace",
        reserve: ["Remove from workspace", "Removing…"],
        variant: "danger",
        isDisabled: busy,
        onPress: () => void confirm(),
      }}
    >
      {trouble !== null && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </AlertDialog>
  );
}
