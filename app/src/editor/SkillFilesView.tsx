// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's folder as a file tree beside the preview** (#5183): what `SkillFiles.tsx` draws
 * once the core has walked the folder. It is the light editor's chunk, as a branch's Files tab is
 * (`PieceFiles.tsx`), and draws the same: the tree family's rows with the project's file icons,
 * a divider that can be dragged or moved from the keyboard, and PieceFiles' own preview
 * (`Shown`, with *Source* beside markdown).
 *
 * **Read only, or the project's own.** Every file is only read here, as a branch's are. Where the
 * page can change the skill, `SKILL.md` offers Edit, which opens it in the Text tab's editor as
 * the page's own Edit does, and every file offers *Open in a tab of its own*, the project's file
 * tab. A built-in or third-party skill's files offer neither, and the preview says why they are
 * only read.
 */
import { useMemo, useState } from "react";
import { Group, Panel } from "react-resizable-panels";
import { FileText } from "lucide-react";
import { commands, type SkillFolder, type StudioSkill } from "../bindings";
import { FileIcon } from "../FileIcon";
import { pieceFileTitle, pieceFileView } from "../pieceViews";
import { useFileIcons } from "../projectTheme";
import { skillFileOf } from "../skillFile";
import type { ViewRef } from "../tabs";
import { iconFor } from "../theme/icons";
import { Button } from "../ui/button/Button";
import { EmptyState } from "../ui/empty-state/EmptyState";
import { RowLabel, RowLead } from "../ui/list/Row";
import { Separator } from "../ui/panel/Separator";
import { editorToolbar, ToolbarFile } from "../ui/tabs/Tabs";
import { Tree, TreeRow, type Key } from "../ui/tree/Tree";
import { Shown, SourceToggle, useFileRead } from "./PieceFiles";

/** One row of the tree: a file, or a folder with what it holds. */
type Node = {
  path: string;
  name: string;
  folder: boolean;
  refused: string | null;
  under: Node[];
};

/** The core's walk, flat and in order, as the rows it nests into. */
export function nested(walked: SkillFolder): Node[] {
  const top: Node[] = [];
  const folders = new Map<string, Node>();
  for (const entry of walked.entries) {
    const cut = entry.path.lastIndexOf("/");
    const node: Node = {
      path: entry.path,
      name: entry.path.slice(cut + 1),
      folder: entry.kind === "folder",
      refused: entry.refused,
      under: [],
    };
    if (node.folder) folders.set(node.path, node);
    const parent = cut < 0 ? undefined : folders.get(entry.path.slice(0, cut));
    (parent?.under ?? top).push(node);
  }
  return top;
}

/** The tree's share of the tab, and the shares it is held between: PieceFiles' own. */
const SPLIT = { start: 30, least: 10, most: 70 };

export default function SkillFilesView({
  plane,
  skill,
  workspace,
  persona,
  walked,
  changed,
  editable,
  readOnly,
  onEditText,
  onOpenView,
}: {
  plane: string;
  skill: StudioSkill;
  workspace: string | null;
  persona: string | null;
  walked: SkillFolder;
  /** Bumped when the project's skills change: the file shown is read again. */
  changed: number;
  editable: boolean;
  readOnly: string | undefined;
  onEditText: () => void;
  onOpenView?: (view: ViewRef, title: string) => void;
}) {
  const { name, folder } = skill;
  const rows = useMemo(() => nested(walked), [walked]);
  const icons = useFileIcons(plane, workspace ?? undefined);
  const [open, setOpen] = useState<Set<Key>>(new Set());
  const [picked, setPicked] = useState<string>();
  const [source, setSource] = useState<{ path: string; on: boolean }>();
  const read = useFileRead(
    picked,
    (path) => commands.skillFile(plane, workspace, persona, folder, path),
    [plane, workspace, persona, folder, changed],
  );
  // Where the project's own file tab can open it: a skill the page can change, in the project.
  const own = editable && picked !== undefined ? skillFileOf(skill, picked) : undefined;
  const showsSource = source !== undefined && source.path === picked && source.on;

  const row = (node: Node) => (
    <TreeRow
      key={node.path}
      id={node.path}
      textValue={node.name}
      unavailable={node.refused !== null}
      tip={node.refused ?? undefined}
      chosen={node.folder ? undefined : node.path === picked}
      onAction={node.folder || node.refused !== null ? undefined : () => setPicked(node.path)}
      content={
        <>
          <RowLead>
            <FileIcon
              symbol={iconFor(icons, {
                name: node.name,
                folder: node.folder,
                open: open.has(node.path),
              })}
            />
          </RowLead>
          <RowLabel>{node.name}</RowLabel>
        </>
      }
    >
      {node.folder ? node.under.map(row) : undefined}
    </TreeRow>
  );

  return (
    <Group className="piece-files" orientation="horizontal">
      <Panel
        id="skill-files-tree"
        defaultSize={`${SPLIT.start}%`}
        minSize={`${SPLIT.least}%`}
        maxSize={`${SPLIT.most}%`}
      >
        <div className="explorer piece-files-tree ui-text-body">
          <Tree
            aria-label={`Files of ${name}`}
            data-testid="skill-files-tree"
            size="sm"
            fit="content"
            expandedKeys={open}
            onExpandedChange={setOpen}
            stopAt={picked}
          >
            {rows.map(row)}
          </Tree>
          {walked.more > 0 && (
            <p className="honest" data-testid="skill-files-more">
              {`At least ${walked.more} more past the first ${walked.entries.length} are not shown.`}
            </p>
          )}
        </div>
      </Panel>
      <Separator className="piece-files-split" aria-label="Resize the file tree" />
      <Panel id="skill-files-preview" minSize={`${100 - SPLIT.most}%`}>
        <section className="piece-files-shown" aria-label={picked ?? "No file picked"}>
          {picked === undefined || read === undefined ? (
            <EmptyState
              mark={FileText}
              headline="Pick a file to read it"
              body="Arrows move through the tree, Right opens a folder, and Enter shows a file here."
              variant="panel"
            />
          ) : (
            <>
              <header {...editorToolbar("piece-files-head")}>
                <ToolbarFile path={picked} />
                <span className="piece-files-actions">
                  <SourceToggle
                    path={picked}
                    read={read}
                    source={showsSource}
                    onSource={(on) => setSource({ path: picked, on })}
                  />
                  {editable && picked === "SKILL.md" && (
                    <Button size="sm" aria-label="Edit SKILL.md" onPress={onEditText}>
                      Edit
                    </Button>
                  )}
                  {own !== undefined && "place" in own && onOpenView !== undefined && (
                    <Button
                      size="sm"
                      onPress={() =>
                        onOpenView(
                          pieceFileView(own.place, own.path),
                          pieceFileTitle(own.place, own.path),
                        )
                      }
                    >
                      Open in a tab of its own
                    </Button>
                  )}
                </span>
              </header>
              {readOnly !== undefined && (
                <p className="honest" data-testid="skill-files-read-only">
                  {readOnly}
                </p>
              )}
              <Shown path={picked} read={read} source={showsSource} />
            </>
          )}
        </section>
      </Panel>
    </Group>
  );
}
