// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  commands,
  type ChatSkill,
  type HarnessKind,
  type SkillPinsRefused,
  type StudioFirst,
  type StudioLevel,
} from "./bindings";
import { Notice } from "./Notice";
import { skillsIn, useViewChanged } from "./planeChanged";
import { askDraft } from "./skillDraftAsks";
import { SkillsNotRead, SkillsReading, type SkillStart } from "./SkillStarts";
import {
  AT_THE_ROOT,
  SKILL_STUDIO_TITLE,
  asPersona,
  askStudioStart,
  inWorkspace,
  skillRowId,
  skillStudioAt,
  type StudioPage,
  type StudioScope,
} from "./skillStudio";
import { SkillMenu, SkillsList } from "./SkillsList";
import {
  explorerPlaceOf,
  filePlaceOf,
  fixOf,
  groupsOf,
  listedOf,
  markOf,
  narrowedTo,
  pinOf,
  turnOf,
  viewRowsOf,
  type ExplorerPlace,
  type ListedSkill,
  type SkillAct,
  type SkillFix,
} from "./skillsListing";
import { useFoldedGroups } from "./skillsViewPrefs";
import { PLANE_ROOT, pieceFileView } from "./pieceViews";
import type { ViewRef } from "./tabs";
import { useListFilter } from "./ui/list/ListFilter";

/** The chat in front, as the Skills view reads its set: its session, its name, where it works,
 *  and the persona it runs as. */
export type FrontChat = {
  session: number;
  /** What the chat is called here, as its row names it: whose set the view's header says. */
  name: string;
  /** Its workspace, or `undefined` for a chat at the project's root. */
  workspace: string | undefined;
  persona: string | null;
  /** The program it runs, by the word the project calls it: whose own skills it gets. */
  harness: HarnessKind | undefined;
};

/** What the view read: its rows, what the studio offers to start from, and the levels a new
 *  skill can land at; or the core's refusal. */
type Read =
  | {
      rows: ListedSkill[];
      first: StudioFirst;
      levels: StudioLevel[];
      /** Why the workspace's chats read none of its pins, where they read none (#3432). */
      pinsRefused: SkillPinsRefused | null;
    }
  | { refused: string };

/** A command's answer, or its refusal, thrown or said, as a sentence. */
async function answered<T>(
  ask: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
): Promise<{ ok: T } | { refused: string }> {
  const answer = await ask.catch((err: unknown) => ({
    status: "error" as const,
    error: String(err),
  }));
  return answer.status === "ok" ? { ok: answer.data } : { refused: answer.error };
}

/** The levels of the plane's and the built-in skills: what a new chat gets whichever program it
 *  runs and wherever it works. A program's own and a repo's wait on the chat. */
const A_NEW_CHATS: readonly string[] = ["built-in", "project", "workspace", "persona"];

/**
 * **Reads what the view lists** from the Skill studio's own answer (the amendment's (f)).
 *
 * - **For the chat in front** (`chat`): the studio read for its workspace as its persona, each
 *   persona's own (which a persona brings into every workspace), then narrowed to the chat's
 *   real set (`narrowedTo`, what Use skill… types from).
 * - **With no chat in front**: what a new chat in `workspace` gets, the studio read for it on no
 *   persona, its plane's and built-in skills alone: which program it runs and which clone it
 *   works in are not known until it starts.
 */
async function readSkills(
  plane: string,
  workspace: string | undefined,
  chat: FrontChat | undefined,
): Promise<Read> {
  if (chat === undefined) {
    const base = await answered(commands.skillStudio(plane, workspace ?? null, null));
    if ("refused" in base) return base;
    return {
      rows: listedOf(base.ok).filter((row) => A_NEW_CHATS.includes(row.skill.level)),
      first: base.ok.first,
      levels: base.ok.levels,
      pinsRefused: base.ok.pins_refused,
    };
  }
  const [base, personas, set] = await Promise.all([
    answered(commands.skillStudio(plane, chat.workspace ?? null, chat.persona)),
    answered(commands.skillPersonaSkills(plane, chat.workspace ?? null)),
    answered(commands.chatSkills(plane, chat.session)),
  ]);
  if ("refused" in base) return base;
  const rows = viewRowsOf(base.ok, "ok" in personas ? personas.ok : undefined);
  const given: { skills: ChatSkill[]; repos: string[] } =
    "ok" in set ? set.ok : { skills: [], repos: [] };
  return {
    rows: narrowedTo(rows, given.skills, {
      persona: chat.persona,
      harness: chat.harness,
      repos: given.repos,
    }),
    first: base.ok.first,
    levels: base.ok.levels,
    pinsRefused: base.ok.pins_refused,
  };
}

/** **The studio's scope a card opens at** (S7): a persona's own skill on that persona's chats,
 *  any other on the workspace the view reads, or the project's root. */
function scopeOfRow(row: ListedSkill, place: string | undefined, persona: string | null) {
  if (row.skill.level === "persona") {
    const whose = row.persona ?? persona;
    if (whose !== undefined && whose !== null) return asPersona(whose);
  }
  return place === undefined ? AT_THE_ROOT : inWorkspace(place);
}

/**
 * **The Skills view** (SK-55, #3266; spec #3679 S7): one answer to what a chat gets and why. It
 * follows the chat in front, its workspace, persona and program, and with no chat in front shows
 * what a new chat in the title bar's workspace gets; its header names whose set it is. It reads
 * again whenever the project's skills change on disk, as the studio does.
 *
 * Every act is one the window already has: a card opens the Skill studio at Library › it, the
 * chat keeping its pane; Use types it into the chat through Use skill…; the switch and the pin
 * are the studio's; Show in the Explorer view opens Explorer on its folder; Open the file opens
 * its `SKILL.md` in a tab; New skill and Make my own copy are the studio's own. Only the folded
 * groups are kept, in the layout file (`skillsViewPrefs.ts`).
 */
export function SkillsView({
  plane,
  workspace,
  chat,
  onOpenView,
  onUseSkill,
  onShowInExplorer,
}: {
  plane: string;
  /** The title bar's workspace, or `undefined` at the project's root. */
  workspace: string | undefined;
  /** The chat in front, while a chat tab is in front. */
  chat?: FrontChat;
  onOpenView: (view: ViewRef, title: string) => void;
  /** Opens Use skill… for chat `session` on `skill`. */
  onUseSkill: (session: number, skill: string) => void;
  /** Opens Explorer on a skill's folder: one of the project's own files, or in a clone of the
   *  title bar's workspace. */
  onShowInExplorer: (place: ExplorerPlace) => void;
}) {
  const [folded, onFold] = useFoldedGroups();
  // Where the rows are read for, and where the switch and the pin act: the chat's own workspace
  // while one is in front, else the title bar's.
  const place = chat === undefined ? workspace : chat.workspace;
  const persona = chat?.persona ?? null;
  const changed = useViewChanged([plane], skillsIn(place));
  const [read, setRead] = useState<{ of: FrontChat | undefined; read: Read }>();
  const [said, setSaid] = useState<{ label: string; text: string }>();
  const [busy, setBusy] = useState(false);
  // The chat by what it reads: a new name or program of the same chat reads nothing again.
  const session = chat?.session;
  const harness = chat?.harness;
  const chatWorkspace = chat?.workspace;
  const chatName = chat?.name ?? "";
  useEffect(() => {
    let live = true;
    const of: FrontChat | undefined =
      session === undefined
        ? undefined
        : { session, name: chatName, workspace: chatWorkspace, persona, harness };
    void readSkills(plane, place, of)
      .catch((err: unknown): Read => ({ refused: String(err) }))
      .then((answer) => {
        if (live) setRead({ of, read: answer });
      });
    return () => {
      live = false;
    };
    // The name is not read by: it only says whose set it is.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plane, place, session, chatWorkspace, persona, harness, changed]);

  const rows = read !== undefined && "rows" in read.read ? read.read.rows : NO_ROWS;
  const nameOf = useCallback((row: ListedSkill) => row.skill.name, []);
  const filter = useListFilter(rows, nameOf);
  const groups = useMemo(() => groupsOf(filter.shown), [filter.shown]);

  // While another chat's set is read, the list stays as it was: it is replaced once it is known.
  if (read === undefined) return <SkillsReading />;
  if ("refused" in read.read)
    return <SkillsNotRead refused={read.read.refused} testid="skills-view-refused" />;
  const shown = read.read;
  // Whose set is shown: the one it was read for, which is the chat in front once it is read.
  const whose =
    read.of !== undefined
      ? read.of.session === session
        ? chatName
        : read.of.name
      : workspace === undefined
        ? "A new chat at the project's root"
        : `A new chat in ${workspace}`;

  const scope: StudioScope = place === undefined ? AT_THE_ROOT : inWorkspace(place);
  const studioAt = (page: StudioPage, skill?: string, newTab = false) =>
    onOpenView(skillStudioAt(scope, page, { skill, newTab }), SKILL_STUDIO_TITLE);
  /** The studio on this view's scope, at its Library (#3681); a tab of its own on ⌘-click. */
  const openStudio = (newTab = false) => studioAt("library", undefined, newTab);
  /** A card in the studio, at Library › it, for the view's scope (S7). */
  const openRow = (row: ListedSkill) =>
    onOpenView(
      skillStudioAt(scopeOfRow(row, place, persona), "library", {
        skill: skillRowId(row.skill),
      }),
      SKILL_STUDIO_TITLE,
    );
  /** One of the studio's three starts (SK-22): a chat's draft here, the others in the studio. */
  const startWith = (start: SkillStart) => {
    if (start === "draft") {
      askDraft(plane, scope, shown.levels);
      return;
    }
    if (start === "sources") {
      studioAt("sources");
      return;
    }
    // The studio goes to its Library first, and the start opens there.
    openStudio();
    askStudioStart("", "new");
  };
  const change = (row: ListedSkill, what: "turn" | "pin") => {
    const asked = what === "pin" ? pinOf(row, place) : turnOf(row.skill, place);
    if (!("label" in asked) || place === undefined) return;
    setBusy(true);
    // The pin is the studio's own command (SK-15d), for this workspace's chats: one commit.
    void (
      what === "pin"
        ? commands.skillPin(plane, { at: "workspace", name: place }, row.skill.name, asked.on)
        : commands.skillTurn(plane, place, row.skill.name, asked.on)
    )
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((answer) => {
        // Done, it is read again as the project's skills change on disk: one read, not two.
        if (answer.status === "error") setSaid({ label: asked.refused, text: answer.error });
        else setSaid(undefined);
      })
      .finally(() => setBusy(false));
  };
  const typeIn = (row: ListedSkill) => {
    if (chat !== undefined) onUseSkill(chat.session, row.skill.name);
  };
  const act = (row: ListedSkill, what: SkillAct) => {
    if (what === "use") typeIn(row);
    else if (what === "studio") openRow(row);
    else if (what === "explorer") {
      // Explorer shows the title bar's workspace, whichever chat is in front.
      const at = explorerPlaceOf(row.skill, workspace);
      if (!("reason" in at)) onShowInExplorer(at);
    } else if (what === "file") {
      const at = filePlaceOf(row.skill);
      if ("reason" in at) return;
      const of =
        at.repo === "" ? PLANE_ROOT : { workspace: at.workspace, repo: at.repo, piece: null };
      onOpenView(pieceFileView(of, at.path), `SKILL.md · ${row.skill.name}`);
    } else change(row, what);
  };
  const fix = (row: ListedSkill, one: SkillFix) => {
    if (one.act === "turn") change(row, "turn");
    else if (one.act === "routing") studioAt("routing");
    else openRow(row);
  };
  const usable = (row: ListedSkill) => {
    const mark = markOf(row);
    return chat !== undefined && mark !== "left-out" && mark !== "off" && mark !== "needs-review";
  };

  return (
    <SkillsList
      whose={whose}
      groups={groups}
      total={rows.length}
      query={filter.query}
      onQuery={filter.setQuery}
      filtered={filter.filtered}
      starts={shown.first.starts}
      busy={busy}
      onStart={startWith}
      onOpenStudio={openStudio}
      onOpen={openRow}
      usable={usable}
      onUse={typeIn}
      fixOf={(row) => fixOf(row, place)}
      onFix={fix}
      onCopy={() => {
        openStudio();
        askStudioStart("", "copy");
      }}
      menuOf={(row) => ({
        menu: () => (
          <SkillMenu
            row={row}
            at={{
              workspace: place,
              chat: chat !== undefined,
              explorer: workspace,
              pinsRefused: shown.pinsRefused,
            }}
            busy={busy}
            onAct={(what) => act(row, what)}
          />
        ),
      })}
      folded={folded}
      onFold={(id, to) => onFold([id], to ? new Set() : new Set([id]))}
      notice={
        said !== undefined && (
          <Notice
            cause="skill-change"
            tone="trouble"
            label={said.label}
            onDismiss={() => setSaid(undefined)}
          >
            {said.text}
          </Notice>
        )
      }
    />
  );
}

/** No rows yet. */
const NO_ROWS: ListedSkill[] = [];
