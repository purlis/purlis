// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type PlaneId, type SecretAt } from "./bindings";
import { Notice } from "./Notice";
import { askSettingsLink } from "./settings/links";
import { CHOOSE_EDITOR, NO_EDITOR, yourEditor } from "./yourEditor";

/** How many findings get a Notice each; the rest are counted in one line under them. */
export const SHOWN_FINDINGS = 5;

/**
 * **Where a save is blocked by what looks like a credential, and the ways out** (ADR 0051).
 *
 * The title bar and the Inbox said only that a staged file held a secret-shaped value, with a
 * link to this view, and this view said no more: the person had to ask a chat which file it
 * was. So each finding is a Notice here: the file, the line, the kind of value the save read
 * there, and what it matched with the value hidden. Never the value: the core sends none.
 *
 * - **Open the file** starts the person's editor on the file at that line. The core opens only
 *   a file its own reading of the project names, at the line it read, whatever this window
 *   sends.
 * - **Save again** is the save the Save button runs, for after the line is mended.
 *
 * A finding with no line is one the save read through the file's escapes: the core opens that
 * file at its top.
 */
export function SaveBlockedBySecret({
  plane,
  secrets,
  busy,
  onSaveAgain,
}: {
  plane: PlaneId;
  secrets: readonly SecretAt[];
  /** While a save runs: Save again says it is busy and a press sends nothing. */
  busy: boolean;
  onSaveAgain: () => void;
}) {
  /** What Open the file answered, for the file it answered about. */
  const [said, setSaid] = useState<{ path: string; trouble: string }>();
  const open = (one: SecretAt) => {
    const editor = yourEditor();
    if (editor === undefined) {
      setSaid({ path: one.path, trouble: NO_EDITOR });
      return;
    }
    setSaid(undefined);
    void commands
      .openBlockedFile(plane, one.path, editor)
      .then((done) => {
        if (done.status === "error") setSaid({ path: one.path, trouble: done.error });
      })
      .catch((err: unknown) => setSaid({ path: one.path, trouble: String(err) }));
  };
  const shown = secrets.slice(0, SHOWN_FINDINGS);
  const more = secrets.length - shown.length;
  return (
    <>
      {shown.map((one) => {
        const trouble = said?.path === one.path ? said.trouble : undefined;
        return (
          <Notice
            key={one.path}
            cause={`save-blocked-secret:${one.path}`}
            tone="trouble"
            label={`The save is blocked in ${one.path}`}
            fixes={[
              { label: "Open the file", onPress: () => open(one) },
              { label: "Save again", onPress: onSaveAgain, busy },
            ]}
            // With no editor chosen, Open the file's answer goes where one is chosen (SE-22).
            link={
              trouble === NO_EDITOR
                ? {
                    label: "Choose your editor",
                    onPress: () => askSettingsLink(plane, CHOOSE_EDITOR),
                  }
                : undefined
            }
          >
            {whereAndWhy(one)} If it is a credential, keep it in a vault and name the vault entry
            here. If it is not, reword the line. Then save again.
            {trouble !== undefined && ` ${trouble}`}
          </Notice>
        );
      })}
      {more > 0 && (
        <p className="saving-note">
          {`And ${more} more ${more === 1 ? "file" : "files"} the save is blocked in: each is named here once the ones above are settled.`}
        </p>
      )}
    </>
  );
}

/** "notes/a.md, line 5: a credential assignment (token = ********)." */
export function whereAndWhy(one: SecretAt): string {
  const where =
    one.line == null ? `${one.path}, read through its escapes` : `${one.path}, line ${one.line}`;
  const matched = one.shown == null ? "" : ` (${one.shown})`;
  return `${where}: ${one.kind}${matched}.`;
}
