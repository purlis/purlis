// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PlaneId } from "./bindings";
import { RepoPicker } from "./RepoPicker";
import { Choice, Field, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * Making a workspace, asked where the answer is given.
 *
 * **The prompt IS the prompt** (ADR 0035, and `ApprovePlane` says the same thing about
 * the trust ask). The CLI's shape here is `charter workspace create <name> --vision "…"`; the
 * window has a person looking at it, so it asks for the two things that command takes and
 * nothing is copied from the printed-command shape.
 *
 * **This dialog validates nothing.** What a workspace may be called is
 * `contain::workspace_name_ok`, reached through `workspace_create` → `wscmd::create` →
 * `wscmd::ensure`, which is the same path `charter workspace create` takes — so the app refuses
 * exactly the names a terminal refuses and says the same sentence about them. A check written
 * here would be a second answer to "what may a workspace be called", and the two would drift
 * the first time either moved. The only thing the button asks of the box is that it has
 * something in it, which is not a rule about names: it is the difference between a question
 * that has been answered and one that has not.
 *
 * A vision is optional and says so. It is the one field that is easier to fill now than later —
 * `workspace.md` is the living charter a fork inherits, and charter nags about an empty one on
 * every command — but a workspace with no vision is a workspace, and the core's own line
 * explains how to add one afterwards.
 *
 * **Its repos are picked from what the operator's own forge login reaches** (ADR 0055), and
 * cloned after the workspace is made — the dialog closes at once and each repo lands on its
 * own. Picking none is a workspace with no repos, which is still a workspace.
 *
 * Its form is drawn from the settings set (DS-3e): a row per answer, and its answers the
 * dialog's footer.
 */
export function NewWorkspace({
  /** What the plane is called, so the dialog says where the workspace is going. */
  plane,
  /** Which plane, for the repo picker to ask about. */
  planeId,
  /** Why the last attempt made nothing — **the core's sentence, unchanged**. */
  trouble,
  /** Whether charter is making it right now, so the answer cannot be given twice. */
  making,
  onCreate,
  onCancel,
}: {
  plane: string;
  planeId: PlaneId;
  trouble?: string;
  making: boolean;
  /** `live`: born LIVE, its charter, memory and todos published with the plane (charter-app#301). */
  onCreate: (name: string, vision: string, live: boolean, repos: string[]) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [vision, setVision] = useState("");
  const [live, setLive] = useState(false);
  const [repos, setRepos] = useState<ReadonlySet<string>>(new Set());
  const ready = name.trim() !== "" && !making;
  const create = () => {
    if (ready) onCreate(name.trim(), vision, live, [...repos].sort());
  };
  // The name box has the keyboard as it opens, the dialog's first field: it is the one thing
  // that has to be answered, and a dialog that opens with the keyboard somewhere else is a
  // dialog you have to click at. A click outside answers nothing (`docs/ui-primitives.md`).
  // Escape is Cancel, and nothing is made.
  return (
    <Dialog
      onClose={onCancel}
      title="New workspace"
      titleId="new-workspace"
      subtitle={
        <>
          in <code>{plane}</code>
        </>
      }
      main={{
        children: making ? "Creating…" : "Create workspace",
        reserve: ["Create workspace", "Creating…"],
        isDisabled: !ready,
        onPress: create,
      }}
    >
      {/* charter's own alphabet, said once and not enforced here. It is guidance for somebody
          typing, and the core is what refuses. */}
      <SettingRow
        label="Name"
        help={
          <>
            Letters, digits, <code>.</code>, <code>_</code> and <code>-</code>. It becomes a
            directory under <code>workspaces/</code>.
          </>
        }
        control={(ids) => <Field ids={ids} kind="text" value={name} onChange={setName} />}
      />

      <SettingRow
        label="What it is for (optional)"
        help={
          <>
            Recorded in <code>workspace.md</code>, the living charter a fork inherits. You can add
            it later with <code>purlis workspace vision</code>.
          </>
        }
        control={(ids) => (
          <Field ids={ids} kind="list" minRows={3} value={vision} onChange={setVision} />
        )}
      />

      <SettingRow
        label="Repos"
        grouped
        help={
          "Optional. Each one is cloned into the workspace after it is made, and you can " +
          "add or remove repos later in its settings."
        }
        control={(ids) => (
          <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
            {/* A login typed in a shell tab closes the dialog, so the tab is not under it
                (NO-8). */}
            <RepoPicker plane={planeId} picked={repos} onPicked={setRepos} onLeave={onCancel} />
          </div>
        )}
      />

      {/* LOCAL unless ticked: publishing is the operator's choice, never a default. */}
      <SettingRow
        label="Live"
        help={
          "Its charter, memory and todos are committed with the project and published by " +
          "every save. Ticked, the project is saved as soon as it is made, as Saving says. " +
          "Left unticked, they stay on this machine."
        }
        control={(ids) => (
          <Choice ids={ids} kind="toggle" checked={live} onCheckedChange={setLive} />
        )}
      />

      {/* Verbatim, and in the dialog rather than behind it: the operator is still answering,
          and a refusal they cannot see beside the box is one they cannot act on. */}
      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}
