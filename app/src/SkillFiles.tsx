// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's Files** (#5183): the tab of a skill's page, after Text, that shows the skill's
 * whole folder (its `SKILL.md`, `references/`, `evals/`, `scripts/` …) as a file tree beside the
 * light editor's preview of the file picked.
 *
 * The core walks the folder and opens a file in it (`skill_files`, `skill_file`), named by the
 * studio row's own `folder`, and answers only a folder a skill the studio lists is in. So a
 * built-in skill's folder in the app, and a third-party one's in a harness's folder, are shown
 * as the project's own are. A skill with no folder purlis knows (one installed from a source)
 * says why in place of a tree.
 *
 * The tree and the preview are the light editor's chunk (`editor/SkillFilesView.tsx`), loaded
 * the first time the tab is drawn, so the window's main bundle carries no CodeMirror.
 */
import { lazy, Suspense, useState } from "react";
import { LoaderCircle } from "lucide-react";
import { commands, type SkillLevel, type StudioSkill } from "./bindings";
import { NotRead, ReadingRows, useRead } from "./SkillPanels";
import { skillFileOf } from "./SkillPage";
import { whoseChats, type StudioScope } from "./skillStudio";
import type { ViewRef } from "./tabs";
import { EmptyState } from "./ui/empty-state/EmptyState";

const SkillFilesView = lazy(() => import("./editor/SkillFilesView"));

export function SkillFiles({
  plane,
  skill,
  scope,
  level,
  readOnly,
  onEditText,
  onOpenView,
}: {
  plane: string;
  skill: StudioSkill;
  scope: StudioScope;
  /** Its level where the page can change it; `null` for a built-in or third-party skill. */
  level: SkillLevel | null;
  /** Why its files are only read here, where they are; said above the preview. */
  readOnly: string | undefined;
  /** Edit: its `SKILL.md` in the Text tab's editor, as the page's own Edit opens it. */
  onEditText: () => void;
  onOpenView?: (view: ViewRef, title: string) => void;
}) {
  const [again, setAgain] = useState(0);
  const { workspace, persona } = whoseChats(scope);
  const { name, folder } = skill;
  const walked = useRead(
    () =>
      folder === ""
        ? Promise.resolve({ status: "ok" as const, data: null })
        : commands.skillFiles(plane, workspace, persona, folder),
    [plane, workspace, persona, folder, again],
  );
  if (folder === "") {
    const none = skillFileOf(skill);
    return (
      <EmptyState
        variant="panel"
        headline={`purlis has no folder of ${name} to show`}
        body={"reason" in none ? none.reason : undefined}
        testid="skill-files-none"
      />
    );
  }
  if (walked.refused !== undefined)
    return (
      <NotRead
        what={`the files of ${name}`}
        why={walked.refused}
        onAgain={() => setAgain((n) => n + 1)}
        testid="skill-page-not-read"
      />
    );
  if (walked.data === undefined || walked.data === null)
    return <ReadingRows what={`the files of ${name}`} testid="skill-page-reading" />;
  return (
    <Suspense fallback={<EmptyState mark={LoaderCircle} headline="Opening the light editor…" />}>
      <SkillFilesView
        plane={plane}
        skill={skill}
        workspace={workspace}
        persona={persona}
        walked={walked.data}
        editable={level !== null}
        readOnly={readOnly}
        onEditText={onEditText}
        onOpenView={onOpenView}
      />
    </Suspense>
  );
}
