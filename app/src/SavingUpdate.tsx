// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { ArrowDownToLine, LoaderCircle } from "lucide-react";
import type { ProjectUpdate } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { SettingActions } from "./settings/components";
import { Button } from "./ui/button/Button";

/** "1 incoming commit", "259 incoming commits". */
export function incomingSaid(n: number): string {
  return `${n} incoming ${n === 1 ? "commit" : "commits"}`;
}

/** What the Update button is named and says on hover, here and in the title bar. */
export function updateSaid(incoming: number): string {
  return `Update: bring in ${incomingSaid(incoming)} from the remote`;
}

/**
 * **Update from remote, in the Saving view** (qw348): the button that brings a project's
 * incoming commits in, drawn where the view says "N incoming".
 *
 * purlis runs the update itself, outside every chat's sandbox: a chat that runs git in the
 * project's root is stopped partway by its sandbox and is now refused, so before this the only
 * way was the person's own terminal. It is the person's press alone (`update_project` is
 * window-only).
 *
 * **A project with unsaved work is never touched**, and nothing is stashed: where a save can
 * run, the button is *Save, then update*, and says so before it is pressed; where it cannot,
 * Update answers "save first" as a Notice.
 */
export function UpdateFromRemote({
  incoming,
  busy,
  saveFirst,
  onUpdate,
  onSaveThenUpdate,
}: {
  /** Commits the last fetch found on the remote that are not here. */
  incoming: number;
  busy: boolean;
  /** Whether the project has unsaved work a save could take first. */
  saveFirst: boolean;
  onUpdate: () => void;
  onSaveThenUpdate: () => void;
}) {
  return (
    <SettingActions>
      <Button
        size="sm"
        data-testid="saving-update"
        tooltip={
          saveFirst
            ? `Save what changed here, then bring in ${incomingSaid(incoming)} from the remote`
            : updateSaid(incoming)
        }
        isDisabled={busy}
        onPress={saveFirst ? onSaveThenUpdate : onUpdate}
        className="panel-view"
      >
        {busy ? (
          <LoaderCircle className="node-icon spinning" aria-hidden="true" />
        ) : (
          <ArrowDownToLine className="node-icon" aria-hidden="true" />
        )}
        {saveFirst ? "Save, then update" : "Update"}
      </Button>
    </SettingActions>
  );
}

/** The reasons pressing Update again can settle by itself: nothing in the project is wrong. */
const TRY_AGAIN: ReadonlySet<string> = new Set([
  "busy",
  "not-fetched",
  "index-locked",
  "out-of-time",
  "failed",
]);

/**
 * **An update that left the project as it was, as a Notice with the reason** (qw348).
 *
 * The sentence is the core's, and says whether the project is exactly as it was. Its way out is
 * the one that settles the reason: *Save, then update* for unsaved work, *Update again* where
 * nothing in the project is wrong, and otherwise *Open terminal here*, since what is left is
 * git's to settle and a sandboxed chat in the project's root is refused it.
 */
export function UpdateLeftNotice({
  left,
  busy,
  canSave,
  onUpdate,
  onSaveThenUpdate,
  onTerminal,
  onDismiss,
}: {
  left: ProjectUpdate;
  busy: boolean;
  /** Whether a save of the project could run now. */
  canSave: boolean;
  onUpdate: () => void;
  onSaveThenUpdate: () => void;
  onTerminal: () => void;
  onDismiss: () => void;
}) {
  const reason = left.reason ?? "failed";
  const files = left.files.length > 0 && (
    <ul className="saving-files" aria-label="The files it is about">
      {left.files.map((path) => (
        <li key={path}>{path}</li>
      ))}
    </ul>
  );
  const cause = `saving-update-left:${reason}`;
  const tone = left.whole && (reason === "unsaved" || reason === "busy") ? "news" : "trouble";
  if (reason === "unsaved" && canSave) {
    const fix: NoticeAction = { label: "Save, then update", onPress: onSaveThenUpdate, busy };
    return (
      <Notice cause={cause} tone={tone} fixes={[fix]} onDismiss={onDismiss}>
        {left.said}
      </Notice>
    );
  }
  if (left.whole && TRY_AGAIN.has(reason)) {
    const fix: NoticeAction = { label: "Update again", onPress: onUpdate, busy };
    return (
      <Notice cause={cause} tone={tone} fixes={[fix]} onDismiss={onDismiss}>
        {left.said}
      </Notice>
    );
  }
  if (reason === "waits-on-request" || reason === "unsaved") {
    return (
      <Notice cause={cause} tone={tone} onDismiss={onDismiss}>
        {left.said}
      </Notice>
    );
  }
  return (
    <Notice
      cause={cause}
      tone={tone}
      link={{ label: "Open terminal here", onPress: onTerminal }}
      onDismiss={onDismiss}
      under={files || undefined}
    >
      {left.said}
    </Notice>
  );
}
