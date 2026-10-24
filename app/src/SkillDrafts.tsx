// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useState } from "react";
import { commands, type ProfileRow, type SkillDraft, type StudioLevel } from "./bindings";
import { counted } from "./dispatches";
import { Button, ButtonRow } from "./ui/button/Button";
import { Select } from "./primitives/Select";
import { askSkillChat } from "./skillDraftAsks";
import { labelOf } from "./skillChanges";

/** The merge view, from the editor's own chunk (ADR 0086 row M2), loaded when a diff opens. */
const MergeViewer = lazy(() =>
  import("./editor/LightEditor").then((module) => ({ default: module.MergeViewer })),
);

/** The harness profile of the draft's own level, in the Try it picker: the one its chats run on,
 *  else the project's default. */
const DEFAULT = "";

/**
 * **The Skill studio's draft skills** (SK-9, #2772; decisions 38, 50 and 52): each skill a chat
 * drafted, kept on this machine and never committed until it is accepted. Each one shows
 *
 * - **Changes**: every file the draft writes, against what the project holds, in the light
 *   editor's merge view;
 * - **Try it**: a scratch chat with the built-in skills and this draft alone, in a throwaway
 *   folder, on the harness profile the draft's level runs on or the one picked; closing it leaves
 *   only the skill runs it recorded;
 * - **Accept**: one commit of exactly the draft's files (a local skill's is kept on this machine
 *   only), refused where the project changed one of them since;
 * - **Reject**: deletes the draft.
 */
export function SkillDrafts({
  plane,
  drafts,
  levels,
  busy,
  onAccept,
  onReject,
}: {
  plane: string;
  drafts: readonly SkillDraft[];
  /** The project's levels, with their words. */
  levels: readonly StudioLevel[];
  busy: boolean;
  onAccept: (draft: SkillDraft) => void;
  onReject: (draft: SkillDraft) => void;
}) {
  const [open, setOpen] = useState<string>();
  const [profiles, setProfiles] = useState<readonly ProfileRow[]>([]);
  const [picked, setPicked] = useState<Readonly<Record<string, string>>>({});
  const some = drafts.length > 0;
  useEffect(() => {
    if (!some) return;
    let live = true;
    void commands
      .startOptions(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setProfiles(answer.data.profiles);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, some]);
  if (!some) return null;
  const options = [
    { value: DEFAULT, label: "Where it was drafted" },
    ...profiles.map((one) => ({ value: one.name, label: one.name })),
  ];
  return (
    <section className="skill-drafts" aria-label="Draft skills" data-testid="skill-drafts">
      <h3>Draft skills</h3>
      <p className="honest">
        A chat's skill is kept here, on this machine, until you accept it. Accept is one commit.
      </p>
      <ul>
        {drafts.map((draft) => {
          const profile = picked[draft.id] ?? DEFAULT;
          return (
            <li key={draft.id} data-testid={`draft-${draft.name}`}>
              <p>
                <code>{draft.name}</code> · {labelOf(levels, draft.level)} ·{" "}
                {draft.new ? "a new skill" : "a change to it"} ·{" "}
                {counted(draft.files.length, "file", "files")}
                {draft.chat !== null && ` · from chat ${draft.chat}`}
              </p>
              {draft.stale && (
                <p className="honest" data-testid={`draft-stale-${draft.name}`}>
                  The project's copy changed since the chat drafted it, so it cannot be accepted:
                  reject it, or ask the chat to draft it again.
                </p>
              )}
              <ButtonRow>
                <Button
                  size="sm"
                  aria-expanded={open === draft.id}
                  aria-label={`Changes in ${draft.name}`}
                  onPress={() => setOpen(open === draft.id ? undefined : draft.id)}
                >
                  Changes
                </Button>
                <Select
                  aria-label={`Try ${draft.name} on`}
                  value={profile}
                  options={options}
                  onValueChange={(to) => setPicked((was) => ({ ...was, [draft.id]: to }))}
                />
                <Button
                  size="sm"
                  aria-label={`Try ${draft.name}`}
                  isDisabled={busy}
                  onPress={() =>
                    askSkillChat({
                      plane,
                      kind: "try",
                      id: draft.id,
                      profile: profile === DEFAULT ? null : profile,
                    })
                  }
                >
                  Try it
                </Button>
                <Button
                  size="sm"
                  variant="primary"
                  aria-label={`Accept ${draft.name}`}
                  isDisabled={busy || draft.stale}
                  onPress={() => onAccept(draft)}
                >
                  Accept
                </Button>
                <Button
                  size="sm"
                  variant="danger"
                  aria-label={`Reject ${draft.name}`}
                  isDisabled={busy}
                  onPress={() => onReject(draft)}
                >
                  Reject
                </Button>
              </ButtonRow>
              {open === draft.id && (
                <Suspense fallback={<p className="pending">Drawing the changes…</p>}>
                  {draft.files.map((file) => (
                    <section
                      key={file.path}
                      className="inbox-change"
                      data-testid={`draft-file-${file.path}`}
                    >
                      <p className="inbox-source">
                        <code>{file.path}</code>
                        {file.before === null ? " · new" : ""}
                      </p>
                      <MergeViewer
                        path={file.path}
                        base={file.before ?? ""}
                        head={file.after}
                        hunks={file.hunks}
                        label={`The draft's ${file.path} of ${draft.name}`}
                      />
                    </section>
                  ))}
                </Suspense>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
