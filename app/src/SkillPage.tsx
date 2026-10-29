// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's page in the Skill studio** (#3682, spec #3679, the amendment's S3, the skill page
 * prototype's variant A): what opens beside the Library's list when a skill is chosen there, or
 * alone with a way back where the tab is narrow. It is the skill's one place: the skill's own
 * view tab is retired.
 *
 * - **The header**: the skill's name, its skill level and its state, what it does, and its
 *   actions: Use in this chat, Edit and Grade, then a "…" menu of Rename, Move to another level,
 *   Deprecate, Archive, Open the file and Delete. A built-in skill's page says it can't be
 *   edited, moved or turned off, and offers only what it can do.
 * - **The banner**: where the skill isn't plainly on (off, left out, a change to review, an update
 *   waiting, hidden by the router, a clash, deprecated or archived), why in one sentence, and the
 *   one fix.
 * - **Its tabs**, each with its count where it has one: Overview (who gets it, where it lives,
 *   who looks after it, its grade, its runs, and how each chat program takes it), Text (its
 *   `SKILL.md` as it is now), History (each version with its diff and a revert, and a change
 *   waiting for review), Grades and Runs.
 *
 * What a change asks of the person (Edit, Rename, Move, Delete) is the studio's, handed in as
 * `doing` and drawn where it belongs: an editor in the Text tab, the rest under the header. Each
 * read is in place: one that fails says so where its data would be, with Try again, and leaves
 * the rest of the page as it is.
 */
import { useContext, useId, useState, type ReactNode } from "react";
import { Ellipsis, RotateCw } from "lucide-react";
import {
  commands,
  SKILL_RUNS_PANEL,
  type SkillChanged,
  type SkillCopy,
  type SkillDetails,
  type SkillDraft,
  type SkillLevel,
  type SkillLifecycle,
  type SkillPanel,
  type Studio,
  type StudioSkill,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice, type NoticeAction } from "./Notice";
import { Badge, type BadgeVariant } from "./ui/badge/Badge";
import { Button, IconButton } from "./ui/button/Button";
import { Fact, Facts } from "./ui/card/Facts";
import { Dialog } from "./ui/dialog/Dialog";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { TextField } from "./ui/field/Field";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "./ui/menu/Menu";
import { ViewPageHeader } from "./ui/panel/ViewFrame";
import { PageTabs, type PageTab } from "./ui/tabs/PageTabs";
import { SkillCompatBadges } from "./SkillCompatBadges";
import {
  SkillHistory,
  SkillUses,
  gradeSaid,
  lifecycleSaid,
  runsSaid,
  UNREAD,
} from "./SkillDetails";
import { SkillGrade } from "./SkillGrade";
import { SkillPinSwitch, SkillPinnedBy } from "./SkillPins";
import { GrantPicker } from "./SkillLevelPickers";
import {
  SKILL_GRADES_PANEL,
  SkillPanelSection,
  SkillTotals,
  keyOf,
  runsOf,
  useRead,
  type Read,
} from "./SkillPanels";
import { gradeTitle, gradeView } from "./GradeScorecard";
import { ownerShown } from "./SkillStudioHarnesses";
import { levelOf, sameLevel, whoseOf } from "./skillChanges";
import {
  ChatToUseHere,
  SKILL_PAGE_TAB_NAMES,
  pinnerOf,
  scopeChat,
  skillRowId,
  whoseChats,
  type SkillPageTab,
  type StudioPage,
  type StudioScope,
} from "./skillStudio";
import { PINNABLE, turnOf } from "./skillsListing";
import { useOpenUseSkill } from "./UseSkill";
import { PLANE_ROOT, pieceFileTitle, pieceFileView, type Place } from "./pieceViews";
import type { ViewRef } from "./tabs";
import type { Offer } from "./actions";

/** One change's answer, as the core's commands give it. */
export type Answer = { status: "ok"; data: SkillChanged } | { status: "error"; error: string };

/** What the studio says after a change: a refusal, or what the change left to say. */
export type Said = { tone: "news" | "trouble"; label: string; text: string; restore?: string };

/** **Runs one change and says what came of it**: the studio's, so its notice and its reads are
 *  one for every page. */
export type Change = (
  label: string,
  run: () => Promise<Answer>,
  then?: () => Promise<Said | undefined>,
) => void;

/** What a change answers, made one: a command that answers nothing beyond being done. */
export function changed(answer: { status: "ok" } | { status: "error"; error: string }): Answer {
  return answer.status === "ok" ? { status: "ok", data: { notes: [] } } : answer;
}

// --- What the page says of a skill -------------------------------------------------------------

/**
 * **A row's one mark** (S2): what most needs saying of a skill, or nothing for a skill a chat
 * simply gets. Off where the workspace turned it off, left out where a chat does not get it,
 * hidden where a router left it off a chat's list, pinned where a pin keeps it on that list.
 */
export function markOf(skill: StudioSkill): { said: string; variant: BadgeVariant } | undefined {
  if (skill.turned_off) return { said: "Off", variant: "waiting" };
  if (!skill.on) return { said: "Left out", variant: "waiting" };
  if (skill.hidden.length > 0) return { said: "Hidden by the router", variant: "neutral" };
  if (skill.pinned) return { said: "Pinned", variant: "accent" };
  return undefined;
}

/** What the page's one fix does, as the banner offers it. */
export type Fix =
  | { kind: "turn"; label: string; on: boolean; refused: string }
  | { kind: "accept"; folder: string }
  | { kind: "pin" }
  | { kind: "go"; label: string; page: StudioPage }
  | { kind: "rename" }
  | { kind: "lifecycle"; label: string; to: SkillLifecycle };

/** **Why a skill isn't plainly on**, in one sentence, its tone and its one fix. */
export type Banner = {
  /** What it is about, stably: what a test names it by. */
  cause: string;
  tone: "news" | "attention";
  text: string;
  /** A second sentence: a grant that does not reach the chats shown, where it is that. */
  more?: { text: string; testid: string };
  fix?: Fix;
};

/** What the page knows beyond the studio's answer that a banner may say. */
export type Beyond = {
  /** The copy's state at its level, once read. */
  copy?: SkillCopy;
  /** A change a chat made to it, waiting for the person's review. */
  review?: SkillDraft;
  /** A newer version of it at its source, by its short commit. */
  update?: string;
};

/**
 * **The banner of a skill's page** (S3): why the skill isn't plainly on and its one fix, the most
 * pressing first. Archived, then a change to review, an update waiting, off in the workspace,
 * left out, hidden by the router, a clash with a skill from outside the project, deprecated.
 * Nothing for a skill a chat simply gets, and nothing for a built-in skill, which is always on.
 */
export function bannerOf(
  skill: StudioSkill,
  studio: Pick<Studio, "clashes">,
  scope: StudioScope,
  beyond: Beyond,
): Banner | undefined {
  if (skill.level === "built-in") return undefined;
  const { workspace } = whoseChats(scope);
  const level = levelOf(skill, scope);
  const lifecycle = beyond.copy?.lifecycle;
  if (lifecycle?.state === "archived")
    return {
      cause: "archived",
      tone: "news",
      text: lifecycleSaid(skill.name, lifecycle),
      fix: { kind: "lifecycle", label: "Unarchive", to: { state: "active" } },
    };
  if (beyond.review !== undefined)
    return {
      cause: "review",
      tone: "attention",
      text: `A chat changed ${skill.name}. Chats keep the version you accepted until you review this change.`,
      fix: { kind: "go", label: "Review the change", page: "drafts" },
    };
  if (beyond.update !== undefined)
    return {
      cause: "update",
      tone: "attention",
      text: `Its source has a newer version (${beyond.update}). Nothing changes until you accept it.`,
      fix: { kind: "go", label: "Review the update", page: "sources" },
    };
  if (skill.turned_off) {
    const turn = turnOf(skill, workspace ?? undefined);
    return {
      cause: "off",
      tone: "news",
      text: `It is off in ${workspace ?? "this workspace"}, so ${lowerFirst(scopeChat(scope))} doesn't get it.`,
      ...("label" in turn ? { fix: { kind: "turn" as const, ...turn } } : {}),
    };
  }
  if (!skill.on) {
    const more =
      skill.not_granted === null
        ? undefined
        : { text: skill.not_granted, testid: `not-granted-${skill.name}` };
    return {
      cause: "left-out",
      tone: "news",
      text: `Left out: ${skill.left_out ?? "no chat here gets it"}.`,
      ...(more === undefined ? {} : { more }),
      ...(skill.accept === null ? {} : { fix: { kind: "accept" as const, folder: skill.accept } }),
    };
  }
  const pinner = pinnerOf(scope);
  if (skill.hidden.length > 0 && !skill.pinned) {
    const on = skill.hidden.map((one) => one.title).join(", ");
    return {
      cause: "hidden",
      tone: "news",
      text: `Hidden by the router on ${on}: ${skill.hidden[0].hidden.said}. The chat can still load it.`,
      ...(pinner !== null && PINNABLE.has(skill.level) ? { fix: { kind: "pin" as const } } : {}),
    };
  }
  const clash = studio.clashes.find(
    (one) => one.name === skill.name && one.level === skill.level && one.local === skill.local,
  );
  if (clash !== undefined)
    return {
      cause: "clash",
      tone: "news",
      text: `Also: ${clash.said}.`,
      ...(level === null ? {} : { fix: { kind: "rename" as const } }),
    };
  if (lifecycle?.state === "deprecated")
    return {
      cause: "deprecated",
      tone: "news",
      text: lifecycleSaid(skill.name, lifecycle),
      fix: { kind: "lifecycle", label: "Undo deprecation", to: { state: "active" } },
    };
  return undefined;
}

/** **Who gets a skill**, as its Overview says it in one line. */
export function whoGetsIt(skill: StudioSkill, scope: StudioScope): string {
  if (!skill.on) return `Left out: ${skill.left_out ?? ""}`;
  if (skill.unchecked !== null) return skill.unchecked;
  const chat = lowerFirst(scopeChat(scope));
  if (skill.hidden.length > 0)
    return `On, ${skill.hidden[0].hidden.said} (on ${skill.hidden.map((one) => one.title).join(", ")}): ${chat} can still load it.`;
  if (skill.pinned) return `On, pinned: ${chat} gets it, and no router hides it.`;
  return `On: ${chat} gets it.`;
}

/** **The file a skill's text is in**, as a tab opens it, or why there is none to open. */
export function skillFileOf(
  skill: StudioSkill,
): { place: Place; path: string } | { reason: string } {
  const { folder } = skill;
  if (skill.level === "built-in")
    return {
      reason: "It ships with purlis, inside the app, and is not one of the project's files.",
    };
  if (folder === "")
    return {
      reason: "It was installed from a source: its files are kept with the project's pins.",
    };
  if (folder.startsWith("/")) return { reason: "Its file is outside the project." };
  if (skill.level === "repo") {
    // `workspaces/<workspace>/<clone>/…`, as the core says a clone's folder.
    const [top, workspace, repo, ...rest] = folder.split("/");
    if (top !== "workspaces" || workspace === undefined || repo === undefined || rest.length === 0)
      return { reason: "Its file is outside the project." };
    return { place: { workspace, repo, piece: null }, path: `${rest.join("/")}/SKILL.md` };
  }
  return { place: PLANE_ROOT, path: `${folder}/SKILL.md` };
}

/**
 * **The row of each name that carries its Pin switch** (SK-15d): of its rows at one of the
 * project's levels, the copy a chat gets, or else the first, where a pin of it may still be
 * written.
 */
export function pinRowsOf(skills: readonly StudioSkill[]): Map<string, StudioSkill> {
  const rows = new Map<string, StudioSkill>();
  for (const skill of skills) {
    if (!PINNABLE.has(skill.level)) continue;
    const was = rows.get(skill.name);
    if (was === undefined || (!was.on && skill.on)) rows.set(skill.name, skill);
  }
  return rows;
}

/** `read` where its data has the shape `fits` checks, else as if it were still being read. */
function shaped<T>(read: Read<T>, fits: (data: T) => boolean): Read<T> {
  return read.data === undefined || read.data === null || fits(read.data)
    ? read
    : { ...read, data: undefined };
}

/** **Why a skill can't be changed from this page**, where it can't: whose it is, or where it is
 *  changed instead. */
export function cannotChangeSaid(skill: StudioSkill): string {
  switch (skill.level) {
    case "built-in":
      return "Built-in skills can't be changed.";
    case "workspace":
      return "It is a workspace's skill: open the studio on that workspace's chats to change it.";
    case "persona":
      return "It is a persona's skill: open the studio on that persona's chats to change it.";
    default: {
      const owner = skill.owner ?? null;
      if (owner?.kind === "you")
        return "It is in your own skills folder, which purlis never changes.";
      const whose = ownerShown(owner);
      return whose === ""
        ? "It is not one of the project's skills, so purlis never changes it."
        : `It belongs to ${whose.charAt(0).toLowerCase()}${whose.slice(1)}, which purlis never changes.`;
    }
  }
}

/** The copy of the skill at `level`, of the copies its details hold. */
function copyAt(copies: readonly SkillCopy[], level: SkillLevel | null): SkillCopy | undefined {
  return level === null ? undefined : copies.find((one) => sameLevel(one.level, level));
}

/** `text` with its first letter lower-cased, to go inside a sentence. */
export function lowerFirst(text: string): string {
  return text.charAt(0).toLowerCase() + text.slice(1);
}

/** A read that failed, said where its data would be, with Try again. */
function NotRead({
  what,
  read,
  onAgain,
}: {
  what: string;
  read: Read<unknown>;
  onAgain: () => void;
}) {
  return (
    <EmptyState
      variant="panel"
      headline={`purlis could not read ${what}`}
      body={read.refused}
      testid="skill-page-not-read"
      action={
        <Button icon={<RotateCw aria-hidden="true" />} onPress={onAgain}>
          Try again
        </Button>
      }
    />
  );
}

/** A read still on its way: the layout is kept, and a line says so. */
function Reading({ what }: { what: string }) {
  return (
    <p className="pending" aria-busy="true" data-testid="skill-page-reading">
      {`Reading ${what}…`}
    </p>
  );
}

// --- The page -----------------------------------------------------------------------------------

/** A change on the skill that the studio draws for it: the editor, or Rename, Move or Delete. */
export type SkillDoing = { what: "edit" | "rename" | "move" | "delete"; drawn: ReactNode };

/**
 * **A skill's page** (#3682): see the module. The studio holds where it is (`tab`, the run marked
 * on Runs) and the change in progress (`doing`); the page reads what only it shows.
 */
export function SkillPage({
  plane,
  studio,
  skill,
  scope,
  drafts,
  tab,
  run,
  busy,
  changed: given,
  change,
  doing,
  onTab,
  onGo,
  onEdit,
  onRename,
  onMove,
  onDelete,
  onPersona,
  offerFor,
  onPress,
  onOpenView,
}: {
  plane: string;
  studio: Studio;
  skill: StudioSkill;
  scope: StudioScope;
  /** The chats' drafts: one that changes this skill waits for review. */
  drafts: readonly SkillDraft[];
  tab: SkillPageTab;
  /** The run its Runs mark, from a run's chip. */
  run?: string;
  busy: boolean;
  /** Bumped when the project's skills change: what the page reads is read again. */
  changed: number;
  change: Change;
  /** The change in progress on this skill, as the studio draws it. */
  doing?: SkillDoing;
  onTab: (tab: SkillPageTab) => void;
  /** Goes to another page of the studio, on the same chats: a fix's way to it. */
  onGo: (page: StudioPage) => void;
  onEdit: (level: SkillLevel) => void;
  onRename: (level: SkillLevel) => void;
  onMove: (level: SkillLevel) => void;
  onDelete: (level: SkillLevel) => void;
  onPersona: (who: string, written: string, how: "add" | "remove") => void;
  /** The catalogue, for the ways out its Runs offer (a new chat where it has none). */
  offerFor?: (id: string) => Offer | undefined;
  onPress?: (offer: Offer) => void;
  /** Opens a tab: the skill's file, a grade's scorecard. */
  onOpenView?: (view: ViewRef, title: string) => void;
}) {
  const [again, setAgain] = useState(0);
  const [graded, setGraded] = useState(0);
  const [asked, setAsked] = useState<"deprecate" | "archive" | "owners">();
  const [dismissed, setDismissed] = useState<string>();
  const level = levelOf(skill, scope);
  const { workspace } = whoseChats(scope);
  const builtIn = skill.level === "built-in";
  const name = skill.name;
  const reread = [plane, name, given, again, graded];
  // Each answer is taken only in its own shape: a core that answers another leaves it unread.
  const details = shaped(
    useRead(() => commands.skillDetails(plane, name), reread),
    (data) => Array.isArray(data.copies),
  );
  const stats = shaped(
    useRead(() => commands.skillStats(plane, name), reread),
    (data) => Array.isArray(data.by_harness),
  );
  const panels = shaped(
    useRead(() => commands.skillPanels(plane, name, level), [...reread, level?.at]),
    Array.isArray,
  );
  // An installed skill alone has a source to ask for a newer version.
  const installed = level !== null && skill.folder === "";
  const updates = useRead(
    () =>
      installed
        ? commands.skillUpdates(plane)
        : Promise.resolve({ status: "ok" as const, data: null }),
    // Once a page: asking a source is a network call, never made on every change on disk.
    [plane, name, installed],
  );
  const copy = copyAt(details.data?.copies ?? [], level);
  const review =
    level === null
      ? undefined
      : drafts.find((one) => !one.new && one.name === name && sameLevel(one.level, level));
  const offer =
    level === null
      ? undefined
      : (Array.isArray(updates.data?.offers) ? (updates.data?.offers ?? []) : []).find(
          (one) => one.name === name && sameLevel(one.level, level),
        );
  const banner = bannerOf(skill, studio, scope, {
    ...(copy === undefined ? {} : { copy }),
    ...(review === undefined ? {} : { review }),
    ...(offer === undefined ? {} : { update: offer.newer.short }),
  });
  const runs = stats.data === undefined ? undefined : runsOf(stats.data);
  const versions = copy?.versions.length;

  const lifecycle = (to: SkillLifecycle, label: string) => {
    if (level !== null) change(label, () => commands.skillLifecycle(plane, level, name, to));
  };
  const pinner = pinnerOf(scope);
  const pin = (to: boolean) => {
    if (pinner !== null)
      change(to ? "The skill was not pinned" : "The skill was not unpinned", () =>
        commands.skillPin(plane, pinner, name, to),
      );
  };
  const press = (fix: Fix): NoticeAction => {
    switch (fix.kind) {
      case "turn":
        return {
          label: fix.label,
          busy,
          onPress: () => {
            if (workspace !== null)
              change(fix.refused, () => commands.skillTurn(plane, workspace, name, fix.on));
          },
        };
      case "accept":
        return {
          label: "Accept its folder",
          busy,
          onPress: () =>
            change("The folder was not accepted", () =>
              commands.skillLevelAccept(plane, fix.folder),
            ),
        };
      case "pin":
        return {
          label: `Pin it to ${scope.name}`,
          busy,
          onPress: () => pin(true),
        };
      case "go":
        return { label: fix.label, onPress: () => onGo(fix.page) };
      case "rename":
        return {
          label: "Rename the project's copy",
          busy,
          onPress: () => level !== null && onRename(level),
        };
      case "lifecycle":
        return {
          label: fix.label,
          busy,
          onPress: () => lifecycle(fix.to, "The skill's state was not changed"),
        };
    }
  };

  const stateMark = markOf(skill);
  const stateBadge =
    stateMark === undefined ? undefined : (
      <Badge variant={stateMark.variant}>{stateMark.said}</Badge>
    );
  const chat = useContext(ChatToUseHere);
  const openUseSkill = useOpenUseSkill();
  const useReason = !skill.on
    ? `${name} is left out of these chats, so a chat can't use it.`
    : chat === undefined
      ? "No chat is open to use it in."
      : undefined;
  const editing = doing?.what === "edit";
  // Its state: the menu offers the way back from it, whatever the banner says first.
  const state = copy?.lifecycle.state;
  const deprecateSaid = state === "deprecated" ? "Undo deprecation" : "Deprecate…";
  const archiveSaid = state === "archived" ? "Unarchive" : "Archive…";
  const file = skillFileOf(skill);
  const cannotChange = cannotChangeSaid(skill);

  const actions = (
    <>
      <Button
        // The suggested action only while it can be taken: a disabled fill reads as none.
        variant={useReason === undefined ? "primary" : "secondary"}
        isDisabled={useReason !== undefined}
        tooltip={useReason ?? `Types ${name} into ${chat?.name ?? "the chat"}, and sends nothing.`}
        onPress={() => chat !== undefined && openUseSkill(chat.session, name)}
      >
        Use in this chat
      </Button>
      {level !== null && (
        <>
          <Button
            aria-label={`Edit ${name}`}
            isDisabled={busy || editing}
            onPress={() => onEdit(level)}
          >
            Edit
          </Button>
          <Button aria-label={`Grade ${name}`} onPress={() => onTab("grades")}>
            Grade
          </Button>
        </>
      )}
    </>
  );
  const moreMenu = (
    <MenuTrigger>
      <IconButton
        variant="ghost"
        icon={<Ellipsis aria-hidden="true" />}
        aria-label={`More for ${name}`}
        isDisabled={busy}
      />
      <Menu
        align="end"
        aria-label={`More for ${name}`}
        disabledKeys={[
          ...(level === null ? ["rename", "move", "deprecate", "archive", "delete"] : []),
          ...(state === "archived" ? ["deprecate"] : []),
          ...("reason" in file || onOpenView === undefined ? ["file"] : []),
        ]}
        onAction={(key) => {
          if (key === "file" && "place" in file)
            onOpenView?.(
              pieceFileView(file.place, file.path),
              pieceFileTitle(file.place, file.path),
            );
          if (level === null) return;
          if (key === "rename") onRename(level);
          else if (key === "move") onMove(level);
          else if (key === "delete") onDelete(level);
          else if (key === "deprecate") {
            if (state === "deprecated")
              lifecycle({ state: "active" }, "The skill's state was not changed");
            else setAsked("deprecate");
          } else if (key === "archive") {
            if (state === "archived")
              lifecycle({ state: "active" }, "The skill's state was not changed");
            else setAsked("archive");
          }
        }}
      >
        <MenuItem id="rename" textValue="Rename…">
          <MenuWords said="Rename…" under={level === null ? cannotChange : undefined} />
        </MenuItem>
        <MenuItem id="move" textValue="Move to another level…">
          <MenuWords
            said="Move to another level…"
            under={level === null ? cannotChange : undefined}
          />
        </MenuItem>
        <MenuItem id="deprecate" textValue={deprecateSaid}>
          <MenuWords
            said={deprecateSaid}
            under={
              level === null
                ? cannotChange
                : state === "archived"
                  ? "It is archived: unarchive it first."
                  : undefined
            }
          />
        </MenuItem>
        <MenuItem id="archive" textValue={archiveSaid}>
          <MenuWords said={archiveSaid} under={level === null ? cannotChange : undefined} />
        </MenuItem>
        <MenuItem id="file" textValue="Open the file">
          <MenuWords said="Open the file" under={"reason" in file ? file.reason : undefined} />
        </MenuItem>
        <MenuItem id="delete" textValue="Delete…" variant="danger">
          <MenuWords said="Delete…" under={level === null ? cannotChange : undefined} />
        </MenuItem>
      </Menu>
    </MenuTrigger>
  );

  const tabs: PageTab[] = [
    { id: "overview", label: SKILL_PAGE_TAB_NAMES.overview },
    { id: "text", label: SKILL_PAGE_TAB_NAMES.text },
    {
      id: "history",
      label: SKILL_PAGE_TAB_NAMES.history,
      ...(versions === undefined
        ? {}
        : { count: versions, countSaid: counted(versions, "version", "versions") }),
    },
    { id: "grades", label: SKILL_PAGE_TAB_NAMES.grades },
    {
      id: "runs",
      label: SKILL_PAGE_TAB_NAMES.runs,
      ...(runs === undefined ? {} : { count: runs, countSaid: counted(runs, "run", "runs") }),
    },
  ];

  const tryAgain = () => setAgain((n) => n + 1);
  const panel = (id: string): ReactNode => {
    switch (id as SkillPageTab) {
      case "overview":
        return (
          <Overview
            plane={plane}
            studio={studio}
            skill={skill}
            scope={scope}
            level={level}
            busy={busy}
            change={change}
            details={details}
            copy={copy}
            runs={runs}
            panels={panels.data}
            onPersona={onPersona}
            onPin={pin}
            onOwners={() => setAsked("owners")}
            onAgain={tryAgain}
          />
        );
      case "text":
        return editing ? doing.drawn : <SkillText plane={plane} skill={skill} level={level} />;
      case "history":
        if (details.refused !== undefined)
          return <NotRead what={`the history of ${name}`} read={details} onAgain={tryAgain} />;
        if (details.data === undefined) return <Reading what={`the history of ${name}`} />;
        if (copy === undefined)
          return (
            <EmptyState
              variant="panel"
              headline={`purlis keeps no history of ${name}`}
              body={
                builtIn
                  ? "Built-in skills change with purlis itself: its release notes say what changed."
                  : "Only the project's own skills have a history: each saved change is a version."
              }
              testid="skill-history-none"
            />
          );
        return (
          <SkillHistory
            plane={plane}
            name={name}
            copy={copy}
            busy={busy}
            waiting={
              review !== undefined && (
                <Notice
                  cause={`skill-review:${name}`}
                  tone="attention"
                  label="A change waits for your review"
                  fixes={[{ label: "Review the change", onPress: () => onGo("drafts") }]}
                >
                  <span data-testid="skill-history-review">
                    {review.chat === null
                      ? `A chat changed ${name}, and the change is not accepted yet.`
                      : `Chat ${review.chat} changed ${name}, and the change is not accepted yet.`}
                  </span>
                </Notice>
              )
            }
            onRevert={(commit) => {
              if (level !== null)
                change("The skill was not reverted", () =>
                  commands.skillRevert(plane, level, name, commit),
                );
            }}
          />
        );
      case "grades":
        return (
          <div className="skill-page-panel">
            {level === null ? (
              <EmptyState
                variant="panel"
                headline={`${name} isn't graded here`}
                body={
                  builtIn
                    ? "Built-in skills are graded by purlis's own tests."
                    : "Only the project's own skills are graded in the studio."
                }
                testid="skill-grades-none"
              />
            ) : (
              <SkillGrade
                plane={plane}
                name={name}
                level={level}
                onGraded={() => setGraded((n) => n + 1)}
              />
            )}
            {(panels.data ?? [])
              .filter((one) => one.key.kind === SKILL_GRADES_PANEL)
              .map((one) => (
                <SkillPanelSection
                  key={keyOf(one.key)}
                  panel={one}
                  name={name}
                  rowAction={
                    onOpenView === undefined
                      ? undefined
                      : {
                          column: "Scorecard",
                          label: "Open scorecard",
                          onOpen: (grade) => onOpenView(gradeView(name, grade), gradeTitle(name)),
                        }
                  }
                />
              ))}
          </div>
        );
      case "runs":
        return (
          <div className="skill-page-panel">
            <SkillTotals name={name} stats={stats.data} refused={stats.refused} />
            {panels.refused !== undefined ? (
              <NotRead what={`the runs of ${name}`} read={panels} onAgain={tryAgain} />
            ) : panels.data === undefined ? (
              <Reading what={`the runs of ${name}`} />
            ) : (
              panels.data
                .filter((one) => one.key.kind === SKILL_RUNS_PANEL)
                .map((one) => (
                  <SkillPanelSection
                    key={keyOf(one.key)}
                    panel={one}
                    name={name}
                    current={run}
                    offerFor={offerFor}
                    onPress={onPress}
                  />
                ))
            )}
          </div>
        );
    }
  };

  return (
    <div
      className="skill-studio-item skill-page"
      data-testid="skill-studio-skill"
      data-row={skillRowId(skill)}
    >
      <ViewPageHeader
        name={name}
        tag={<Badge>{skill.label}</Badge>}
        mark={stateBadge}
        description={skill.description === "" ? undefined : skill.description}
        actions={actions}
        more={moreMenu}
      />
      {builtIn && (
        <p className="honest" data-testid="skill-built-in">
          It ships with purlis, so it can't be edited, moved or turned off.
        </p>
      )}
      {banner !== undefined && dismissed !== banner.cause && (
        <BannerNotice
          banner={banner}
          fix={banner.fix === undefined ? undefined : press(banner.fix)}
          onDismiss={() => setDismissed(banner.cause)}
        />
      )}
      {doing !== undefined && doing.what !== "edit" && (
        <div className="skill-page-doing" data-testid="skill-studio-doing">
          {doing.drawn}
        </div>
      )}
      <PageTabs
        label={`${name}'s sections`}
        tabs={tabs}
        selected={tab}
        onSelect={(id) => onTab(id as SkillPageTab)}
      >
        {panel}
      </PageTabs>
      {asked === "deprecate" && level !== null && (
        <DeprecateDialog
          name={name}
          busy={busy}
          onClose={() => setAsked(undefined)}
          onDeprecate={(replacement) => {
            setAsked(undefined);
            lifecycle({ state: "deprecated", replacement }, "The skill was not deprecated");
          }}
        />
      )}
      {asked === "archive" && level !== null && (
        <Dialog
          onClose={() => setAsked(undefined)}
          title={`Archive ${name}?`}
          titleId="skill-archive"
          description="It stays in the project, with its history, but no chat loads it until you unarchive it."
          main={{
            children: "Archive",
            isDisabled: busy,
            onPress: () => {
              setAsked(undefined);
              lifecycle({ state: "archived" }, "The skill was not archived");
            },
          }}
        />
      )}
      {asked === "owners" && level !== null && copy !== undefined && (
        <OwnersDialog
          name={name}
          owners={copy.owners}
          busy={busy}
          onClose={() => setAsked(undefined)}
          onSave={(owners) => {
            setAsked(undefined);
            change("The skill's owners were not changed", () =>
              commands.skillOwners(plane, level, name, owners),
            );
          }}
        />
      )}
    </div>
  );
}

/** **The banner** (S3): why the skill isn't plainly on, and its one fix, or else a way to put
 *  the line away. */
function BannerNotice({
  banner,
  fix,
  onDismiss,
}: {
  banner: Banner;
  fix: NoticeAction | undefined;
  onDismiss: () => void;
}) {
  const sentence = (
    <>
      <span data-testid={`skill-banner-${banner.cause}`}>{banner.text}</span>
      {banner.more !== undefined && (
        <span className="honest" data-testid={banner.more.testid}>
          {` ${banner.more.text}`}
        </span>
      )}
    </>
  );
  const cause = `skill-banner:${banner.cause}`;
  return fix === undefined ? (
    <Notice cause={cause} tone={banner.tone} label={banner.text} onDismiss={onDismiss}>
      {sentence}
    </Notice>
  ) : (
    <Notice cause={cause} tone={banner.tone} label={banner.text} fixes={[fix]}>
      {sentence}
    </Notice>
  );
}

/** **A skill's Overview** (S3): who gets it, where it lives, who looks after it, its grade, its
 *  runs, how each chat program takes it, the skills it names, who pins it, and each panel an
 *  extension adds. */
function Overview({
  plane,
  studio,
  skill,
  scope,
  level,
  busy,
  change,
  details,
  copy,
  runs,
  panels,
  onPersona,
  onPin,
  onOwners,
  onAgain,
}: {
  plane: string;
  studio: Studio;
  skill: StudioSkill;
  scope: StudioScope;
  level: SkillLevel | null;
  busy: boolean;
  change: Change;
  details: Read<SkillDetails>;
  copy: SkillCopy | undefined;
  runs: number | undefined;
  panels: readonly SkillPanel[] | undefined;
  onPersona: (who: string, written: string, how: "add" | "remove") => void;
  onPin: (to: boolean) => void;
  onOwners: () => void;
  onAgain: () => void;
}) {
  const name = skill.name;
  const { workspace, persona } = whoseChats(scope);
  const pinner = pinnerOf(scope);
  const refusedPins = pinner === null ? null : (studio.pins_refused ?? null);
  // The Pin switch (SK-15d): on the copy of a name a chat could have pinned (one of the
  // project's levels), where the chat gets it or a pin of it is still written.
  const pinned =
    pinner === null || pinRowsOf(studio.skills).get(name) !== skill
      ? null
      : skill.pinned_by.some((by) => by.at === pinner.at && by.name === pinner.name);
  const pinShown = pinner !== null && pinned !== null && (skill.on || pinned);
  const turn = workspace === null ? undefined : turnOf(skill, workspace);
  // A persona's own list (SK-5, #2768): its own skill is in its skills already.
  const personaAct =
    persona === null || skill.level === "persona"
      ? null
      : skill.named
        ? {
            label: "Remove",
            name: `Remove ${skill.written} from ${persona}`,
            how: "remove" as const,
          }
        : {
            label: `Add to ${persona}`,
            name: `Add ${skill.written} to ${persona}`,
            how: "add" as const,
          };
  const newest = copy?.versions[0];
  const ownersSaid =
    copy !== undefined
      ? copy.owners.length === 0
        ? "No one is named."
        : copy.owners.join(", ")
      : skill.level === "built-in"
        ? "purlis"
        : ownerShown(skill.owner ?? null) || "Not named";
  return (
    <div className="skill-page-panel">
      <Facts aria-label={`About ${name}`} className="skill-studio-facts">
        <Fact label="Who gets it">
          <span data-testid="skill-in-a-chat">{whoGetsIt(skill, scope)}</span>
          {turn !== undefined && "label" in turn && (
            <>
              {" "}
              <Button
                size="sm"
                isDisabled={busy}
                onPress={() => {
                  if (workspace !== null)
                    change(turn.refused, () => commands.skillTurn(plane, workspace, name, turn.on));
                }}
              >
                {turn.label}
              </Button>
            </>
          )}
          {skill.accept !== null && (
            <>
              {" "}
              <Button
                size="sm"
                isDisabled={busy}
                onPress={() => {
                  const folder = skill.accept;
                  if (folder !== null)
                    change("The folder was not accepted", () =>
                      commands.skillLevelAccept(plane, folder),
                    );
                }}
              >
                Accept its folder
              </Button>
            </>
          )}
        </Fact>
        {personaAct !== null && persona !== null && (
          <Fact label={`In ${persona}'s list`}>
            {skill.named ? `${persona} names it.` : `${persona} doesn't name it.`}{" "}
            <Button
              size="sm"
              aria-label={personaAct.name}
              isDisabled={busy}
              onPress={() => onPersona(persona, skill.written, personaAct.how)}
            >
              {personaAct.label}
            </Button>
          </Fact>
        )}
        <Fact label="Where it lives">
          {skill.folder === "" || skill.folder.startsWith("/") ? (
            skill.label
          ) : (
            <>
              <code>{skill.folder}</code>
              {` · ${skill.label}`}
            </>
          )}
          {skill.local && level !== null && (
            <>
              {" "}
              <Button
                size="sm"
                isDisabled={busy}
                tooltip="Give it to the team: it is committed with the project"
                onPress={() =>
                  change("The skill was not shared", () =>
                    commands.skillShare(plane, whoseOf(level), name).then(changed),
                  )
                }
              >
                Share
              </Button>
            </>
          )}
        </Fact>
        {skill.granted && (
          <Fact label="Reaches">
            <GrantPicker
              label={`Where ${name} reaches`}
              workspaces={studio.workspaces}
              now={skill.granted}
              disabled={busy}
              onPick={(grant) =>
                change("The grant was not changed", () => commands.skillGrant(plane, name, grant))
              }
            />
          </Fact>
        )}
        {pinShown && pinner !== null && (
          <Fact label="Pinned">
            <SkillPinSwitch
              name={name}
              by={pinner}
              pinned={pinned === true}
              refused={refusedPins}
              busy={busy}
              onPin={onPin}
            />
          </Fact>
        )}
        <Fact label="Owner">
          <span data-testid="skill-owners">{ownersSaid}</span>
          {copy !== undefined && level !== null && (
            <>
              {" "}
              <Button size="sm" isDisabled={busy} onPress={onOwners}>
                Change owners…
              </Button>
            </>
          )}
          {copy?.unread === true && <span className="honest">{` ${UNREAD}`}</span>}
        </Fact>
        <Fact label="Grade">{newest === undefined ? "Not graded yet." : gradeSaid(newest)}</Fact>
        <Fact label="Runs">
          {runs === undefined
            ? "Reading…"
            : runs === 0
              ? "No chat has run it yet."
              : newest === undefined
                ? counted(runs, "run", "runs")
                : `${counted(runs, "run", "runs")}; on this version: ${runsSaid(newest.runs)}`}
        </Fact>
        <Fact label="Works on">
          <SkillCompatBadges
            name={name}
            compat={skill.compat}
            lint={skill.lint}
            notSupported={studio.not_supported}
          />
        </Fact>
      </Facts>
      {copy?.unused_days !== null &&
        copy?.unused_days !== undefined &&
        copy.lifecycle.state === "active" && (
          <p className="honest" data-testid="skill-unused">
            No chat has run {name} for {counted(copy.unused_days, "day", "days")}. Deprecate it, so
            chats are pointed to something else?
          </p>
        )}
      {details.refused !== undefined ? (
        <NotRead what={`what ${name} names`} read={details} onAgain={onAgain} />
      ) : details.data === undefined ? (
        <Reading what={`what ${name} names`} />
      ) : (
        <>
          <SkillUses name={name} details={details.data} />
          <SkillPinnedBy
            name={name}
            pinners={details.data.pinned_by}
            refused={details.data.pins_refused}
            busy={busy}
            onUnpin={(by) =>
              change("The skill was not unpinned", () => commands.skillPin(plane, by, name, false))
            }
            onRemove={(by, bad) =>
              change("The pin was not removed", () => commands.skillPin(plane, by, bad, false))
            }
          />
        </>
      )}
      {(panels ?? [])
        .filter((one) => one.key.kind === "extension")
        .map((one) => (
          <SkillPanelSection key={keyOf(one.key)} panel={one} name={name} />
        ))}
    </div>
  );
}

/** **A skill's Text** (S3): its `SKILL.md` as it is now, read only. Edit turns it into the editor
 *  in place (Skills design 04). */
function SkillText({
  plane,
  skill,
  level,
}: {
  plane: string;
  skill: StudioSkill;
  level: SkillLevel | null;
}) {
  const [again, setAgain] = useState(0);
  const text = useRead(
    () =>
      level === null
        ? Promise.resolve({ status: "ok" as const, data: null })
        : commands.skillText(plane, level, skill.name),
    [plane, skill.name, level?.at, again],
  );
  const id = useId();
  if (level === null)
    return (
      <EmptyState
        variant="panel"
        headline={`purlis doesn't keep the text of ${skill.name}`}
        body={
          skill.level === "built-in"
            ? "It ships with purlis, inside the app."
            : `It is ${ownerShown(skill.owner ?? null).toLowerCase() || "from outside the project"}'s: its program reads it where it is.`
        }
        testid="skill-text-none"
      />
    );
  if (text.refused !== undefined)
    return (
      <NotRead
        what={`the text of ${skill.name}`}
        read={text}
        onAgain={() => setAgain((n) => n + 1)}
      />
    );
  if (text.data === undefined || text.data === null)
    return <Reading what={`the text of ${skill.name}`} />;
  return (
    <figure className="skill-page-text">
      <figcaption id={id} className="honest">
        SKILL.md, as chats started from now get it.
      </figcaption>
      <pre tabIndex={0} aria-labelledby={id} data-testid="skill-text">
        {text.data}
      </pre>
    </figure>
  );
}

/** **Deprecate** (decision 62): chats still get it, pointed to the skill to use instead. */
export function DeprecateDialog({
  name,
  busy,
  onClose,
  onDeprecate,
}: {
  name: string;
  busy: boolean;
  onClose: () => void;
  onDeprecate: (replacement: string | null) => void;
}) {
  const [instead, setInstead] = useState("");
  return (
    <Dialog
      onClose={onClose}
      title={`Deprecate ${name}?`}
      titleId="skill-deprecate"
      description="Chats still get it, listed after the other skills, with a line that points them to what replaces it."
      initialFocus="field"
      main={{
        children: "Deprecate",
        isDisabled: busy,
        onPress: () => onDeprecate(instead.trim() === "" ? null : instead.trim()),
      }}
    >
      <TextField
        label="Use instead (optional)"
        placeholder="another skill's name"
        value={instead}
        onChange={setInstead}
      />
    </Dialog>
  );
}

/** **Who looks after a skill** (decision 63): logins or emails, for people to know. */
export function OwnersDialog({
  name,
  owners,
  busy,
  onClose,
  onSave,
}: {
  name: string;
  owners: readonly string[];
  busy: boolean;
  onClose: () => void;
  onSave: (owners: string[]) => void;
}) {
  const [typed, setTyped] = useState(owners.join(", "));
  return (
    <Dialog
      onClose={onClose}
      title={`Who looks after ${name}`}
      titleId="skill-owners"
      description="Named for people to know whom to ask. On this project, owners are informational."
      initialFocus="field"
      main={{
        children: "Save owners",
        isDisabled: busy || typed === owners.join(", "),
        onPress: () =>
          onSave(
            typed
              .split(",")
              .map((one) => one.trim())
              .filter((one) => one !== ""),
          ),
      }}
    >
      <TextField
        label="Owners"
        placeholder="logins or emails, separated by commas"
        value={typed}
        onChange={setTyped}
      />
    </Dialog>
  );
}
