// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { CloudDownload } from "lucide-react";
import { DoorPage } from "./DoorPage";
import { EmptyState } from "./EmptyState";

/**
 * **Get a project from your forge** (#1812's door; the flow is #1815's, then #1818's).
 *
 * The door is on the welcome from the first day, because ruling Q1 puts it there and a person
 * looking for "clone my team's project" should find where it will be. Until #1815 builds the
 * clone — a pasted URL, a folder under `~/Projects`, progress, and nothing left behind on a
 * cancel — this page says plainly that it is not here yet, and offers the way that works today:
 * clone it with git, then open its folder. Nothing on it reaches a forge.
 *
 * **This component is the seam #1815 fills in.** The welcome draws it behind the door and hands
 * it the folder dialog the Open door uses; the clone form replaces the empty state below.
 */
export function GetProject({
  onBack,
  onOpenFolder,
  said,
}: {
  /** Comes back to the welcome. */
  onBack: () => void;
  /** The Open door's own act: the folder dialog, then the open. */
  onOpenFolder: () => void;
  /** Why the folder dialog could not open, when it could not (#1291). */
  said?: string;
}) {
  return (
    <DoorPage title="Get a project from your forge" className="get-project" onBack={onBack}>
      <EmptyState
        mark={CloudDownload}
        headline="Getting a project from GitHub or GitLab comes in a later update"
        body="Until then, clone the project with git, then open its folder here."
        action={
          <button type="button" tabIndex={0} onClick={onOpenFolder}>
            Open a folder…
          </button>
        }
      />
      {said && (
        <p className="trouble" role="alert">
          {said}
        </p>
      )}
    </DoorPage>
  );
}
