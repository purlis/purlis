// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useId, useState } from "react";
import { commands, type ProfileRow, type SkillDraft, type SkillDraftFile } from "./bindings";
import { counted } from "./dispatches";
import { Badge } from "./ui/badge/Badge";
import { Button } from "./ui/button/Button";
import { Disclosure } from "./ui/disclosure/Disclosure";
import { List, ListRow, type Selection } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ViewPageHeader } from "./ui/panel/ViewFrame";
import { ReviewPage } from "./ui/review-page/ReviewPage";
import { Select } from "./ui/select/Select";
import { Notice } from "./Notice";
import { askSkillChat } from "./skillDraftAsks";

/** The merge view, from the editor's own chunk (ADR 0086 row M2), loaded when a file opens. */
const MergeViewer = lazy(() =>
  import("./editor/LightEditor").then((module) => ({ default: module.MergeViewer })),
);

/** The harness profile of the draft's own level, in the Try it picker: the one its chats run on,
 *  else the project's default. */
const DEFAULT = "";

/** What a draft is, as its mark says it: out of date, a new skill, or a change to one. */
function markOf(draft: SkillDraft) {
  if (draft.stale) return <Badge variant="waiting">Out of date</Badge>;
  return <Badge variant="accent">{draft.new ? "A new skill" : "A change to it"}</Badge>;
}

/** Who drafted it: its chat by its number, where the core knows it. */
function draftedBy(draft: SkillDraft): string {
  return draft.chat === null ? "a chat" : `chat ${draft.chat}`;
}

/** **"Ask the chat to redo it"**, or "Ask a chat…" where the chat that drafted it is closed. */
export function redoLabel(draft: SkillDraft): string {
  return draft.chat_open ? "Ask the chat to redo it" : "Ask a chat to redo it";
}

/**
 * **The Drafts page's list** (S9, #3690; `prototype/studio-secondary-pages`, variant A): a row of
 * each skill a chat drafted, its name, its skill level and one mark (out of date, a new skill, a
 * change to it), with how many files it writes and who drafted it under them. The draft chosen
 * opens beside the list ({@link DraftPage}).
 */
export function DraftsList({
  drafts,
  labelOf,
  chosen,
  onChoose,
}: {
  drafts: readonly SkillDraft[];
  /** Its level as a person reads it. */
  labelOf: (draft: SkillDraft) => string;
  /** The draft chosen, by its id. */
  chosen: string | undefined;
  onChoose: (id: string) => void;
}) {
  return (
    <List
      aria-label="Draft skills"
      className="skill-library"
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={chosen === undefined ? new Set() : new Set([chosen])}
      onSelectionChange={(keys: Selection) => {
        const [first] = keys === "all" ? [] : [...keys];
        if (first !== undefined) onChoose(String(first));
      }}
    >
      {drafts.map((draft) => (
        <ListRow
          key={draft.id}
          id={draft.id}
          textValue={draft.name}
          data-testid={`draft-${draft.name}`}
          data-row={draft.id}
        >
          <RowLabel>{draft.name}</RowLabel>
          <RowTrail>
            <Badge>{labelOf(draft)}</Badge>
            {markOf(draft)}
          </RowTrail>
          <RowBelow clamp={1}>
            {`${counted(draft.files.length, "file", "files")} · drafted by ${draftedBy(draft)}`}
          </RowBelow>
        </ListRow>
      ))}
    </List>
  );
}

/** What one file of a draft is, beside its path. */
function fileMark(file: SkillDraftFile) {
  if (file.stale) return <Badge variant="waiting">Out of date</Badge>;
  return <Badge>{file.before === null ? "New" : "Changed"}</Badge>;
}

/**
 * **A draft's page** (S9, #3690; decisions 38, 50 and 52), beside the Drafts list: a review page
 * of what the draft writes, with its action fixed at the bottom.
 *
 * - **Its files**: each one it writes, against what the project holds, in the light editor's
 *   merge view, the first open.
 * - **Try it on**: a scratch chat with the built-in skills and this draft alone, in a throwaway
 *   folder, on the harness profile the draft's level runs on or the one picked.
 * - **Accept**: one commit of exactly its files (a local skill's is kept on this machine only).
 * - **Discard**: deletes the draft; the project is not changed.
 * - **Out of date** (a file it writes changed in the project since): said in the attention tone,
 *   with **Ask the chat to redo it** in place of Accept. The chat that drafted it is asked where
 *   it is still open, else a chat is opened to redo it, and what it writes is drafted on the
 *   project's copy (`drafts::take`).
 */
export function DraftPage({
  plane,
  draft,
  label,
  local,
  busy,
  onAccept,
  onDiscard,
  onBack,
}: {
  plane: string;
  draft: SkillDraft;
  /** Its level as a person reads it. */
  label: string;
  /** Whether its level is this machine's alone, so Accept commits nothing. */
  local: boolean;
  busy: boolean;
  onAccept: (draft: SkillDraft) => void;
  onDiscard: (draft: SkillDraft) => void;
  /** Back to the list of drafts. */
  onBack: () => void;
}) {
  const [profiles, setProfiles] = useState<readonly ProfileRow[]>([]);
  const [profile, setProfile] = useState(DEFAULT);
  const tryOn = useId();
  useEffect(() => {
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
  }, [plane]);
  const options = [
    { value: DEFAULT, label: "Where it was drafted" },
    ...profiles.map((one) => ({ value: one.name, label: one.name })),
  ];
  const files = counted(draft.files.length, "file", "files");
  const accept = local
    ? `Accept writes its ${files} into the ${label} skills, on this machine only.`
    : `Accept writes its ${files} into the ${label} skills, in one commit.`;
  const discard = "Discard deletes the draft. The project is not changed.";
  const redo = () =>
    askSkillChat({
      plane,
      kind: "redo",
      id: draft.id,
      name: draft.name,
      chat: draft.chat_open ? draft.chat : null,
    });
  return (
    <>
      <ViewPageHeader
        name={draft.name}
        tag={<Badge>{label}</Badge>}
        mark={markOf(draft)}
        description={`${draft.new ? "A new skill" : "A change to it"}, drafted by ${draftedBy(draft)}. A draft stays on this machine until you accept it.`}
      />
      <ReviewPage
        label={`The draft of ${draft.name}`}
        data-testid="skill-studio-draft"
        summary={draft.stale ? discard : accept}
        summaryReserve={[accept, discard]}
        onCancel={onBack}
        cancel={false}
        main={
          draft.stale
            ? {
                children: "Discard",
                variant: "danger",
                "aria-label": `Discard ${draft.name}`,
                isDisabled: busy,
                onPress: () => onDiscard(draft),
              }
            : {
                children: "Accept",
                "aria-label": `Accept ${draft.name}`,
                isDisabled: busy,
                byReturn: false,
                onPress: () => onAccept(draft),
              }
        }
        alternative={
          draft.stale
            ? undefined
            : {
                children: "Discard",
                variant: "danger",
                "aria-label": `Discard ${draft.name}`,
                isDisabled: busy,
                onPress: () => onDiscard(draft),
              }
        }
      >
        {draft.stale && (
          <Notice
            cause={`draft-out-of-date:${draft.id}`}
            tone="attention"
            label="Out of date"
            fixes={[{ label: redoLabel(draft), onPress: redo }]}
          >
            <span data-testid={`draft-stale-${draft.name}`}>
              {`The project's copy changed since ${draftedBy(draft)} drafted it, so it can't be accepted. ${
                draft.chat_open
                  ? "That chat is still open: it can write its change again on top of the project's copy."
                  : "That chat is closed: a new chat can write the change on top of the project's copy."
              }`}
            </span>
          </Notice>
        )}
        <section aria-label="What it writes" className="skill-draft-files">
          {draft.files.map((file, at) => (
            <Disclosure
              key={file.path}
              label={file.path}
              trail={fileMark(file)}
              defaultExpanded={at === 0}
              data-testid={`draft-file-${file.path}`}
            >
              <Suspense fallback={<p className="pending">Drawing the changes…</p>}>
                <MergeViewer
                  path={file.path}
                  base={file.before ?? ""}
                  head={file.after}
                  hunks={file.hunks}
                  label={`The draft's ${file.path} of ${draft.name}`}
                />
              </Suspense>
            </Disclosure>
          ))}
        </section>
        <div className="skill-draft-try">
          <span id={tryOn}>Try it on</span>
          <Select
            size="sm"
            aria-labelledby={tryOn}
            value={profile}
            options={options}
            onValueChange={setProfile}
          />
          <Button
            size="sm"
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
        </div>
      </ReviewPage>
    </>
  );
}
