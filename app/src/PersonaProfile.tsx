// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PersonaProfile as Read } from "./bindings";
import { Choice, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";

/** The value that stands for "names no profile": no profile is called the empty string. */
const NONE = "";

/**
 * The profile a persona's chats start on (#1445), set from the persona view.
 *
 * **Only a profile the project offers can be picked.** The rows are the project's own profiles
 * on this machine, read by the core, and the core refuses any other name whatever this dialog
 * sends: the line it writes is read back as what a chat starts on. A persona's definition is a
 * file a chat can edit, so a name there the project does not offer is shown here as what it
 * is, picked by nothing, and cleared by any choice made.
 *
 * "None" means none: a chat handed to the persona then starts on the asking chat's profile.
 * For a persona that names its own profile that removes the line. For one that inherits a
 * profile from the persona it extends, the dialog says whose it is, and the core writes
 * `profile: none` in this persona's own definition, so it opts out and its parent is untouched.
 *
 * It writes one `profile:` line in the persona's own `persona.md`, a change to commit like any
 * other edit of that file.
 */
export function PersonaProfile({
  persona,
  read,
  unreadable,
  trouble,
  saving,
  onSave,
  onCancel,
}: {
  persona: string;
  /** What the definition names and what the project offers; absent while it is being read. */
  read?: Read;
  /** Why it could not be read, in the core's words. */
  unreadable?: string;
  /** Why the last save wrote nothing, in the core's words. */
  trouble?: string;
  saving: boolean;
  /** The profile picked, or `null` for none. */
  onSave: (profile: string | null) => void;
  onCancel: () => void;
}) {
  // What the person picked here; until they pick, the row the definition names, where the
  // project offers it.
  const [picked, setPicked] = useState<string>();
  const named = read?.named ?? null;
  const offered = read?.offered ?? [];
  const unknown = named !== null && !offered.includes(named);
  const parent = read?.inherited_from ?? null;
  const value = picked ?? (named !== null && !unknown ? named : unknown ? undefined : NONE);
  // Cancel has the keyboard as it opens: a profile is a choice to read before it is made.
  return (
    <Dialog
      onClose={() => {
        if (!saving) onCancel();
      }}
      title={`Profile of ${persona}`}
      subtitle={
        <>
          Written as <code>profile:</code> in <code>personas/{persona}/persona.md</code>
        </>
      }
      initialFocus="cancel"
      cancel={{ isDisabled: saving && read !== undefined, onPress: onCancel }}
      main={{
        children: saving ? "Saving…" : "Save profile",
        reserve: ["Save profile", "Saving…"],
        isDisabled: !read || value === undefined || saving,
        onPress: () => {
          if (read && value !== undefined && !saving) onSave(value === NONE ? null : value);
        },
      }}
    >
      {!read && !unreadable && (
        <p className="pending" role="status" aria-busy="true">
          Reading the profile {persona} names…
        </p>
      )}
      {unreadable && (
        <p className="trouble" role="alert">
          {unreadable}
        </p>
      )}
      {unknown && (
        <p className="honest">
          Its definition names <code>{named}</code>. This project does not offer it, so no chat
          starts on it. Pick one of the project&apos;s profiles, or none.
        </p>
      )}

      {parent !== null && (
        <p className="honest">
          Inherited from <code>{parent}</code>, which this persona extends. A choice here is written
          in this persona&apos;s own definition, and <code>{parent}</code> keeps its profile.
        </p>
      )}

      {read && (
        <>
          <SettingRow
            label="Profile"
            grouped
            help={
              "The harness profile this persona's chats start on. With none, a chat handed " +
              "to it starts on the asking chat's profile."
            }
            control={(ids) => (
              <Choice
                ids={ids}
                kind="radio"
                options={[
                  { value: NONE, label: "None" },
                  ...offered.map((name) => ({ value: name, label: name })),
                ]}
                value={value}
                onValueChange={setPicked}
              />
            )}
          />

          {trouble && (
            <p className="trouble" role="alert">
              {trouble}
            </p>
          )}
        </>
      )}
    </Dialog>
  );
}
