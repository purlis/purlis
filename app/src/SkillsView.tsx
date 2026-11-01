// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  commands,
  type ChatSkill,
  type HarnessKind,
  type PlaneUpdated,
  type SkillPinsRefused,
  type StudioFirst,
  type StudioLevel,
} from "./bindings";
import { Notice } from "./Notice";
import { INSTRUCTIONS, skillsIn, usePlaneChanged, useViewChanged } from "./planeChanged";
import { planeUpdatedAt } from "./PlaneUpdated";
import { SkillsNotRead, SkillsReading } from "./SkillStarts";
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
  branchFolderOf,
  branchFolderRowsOf,
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
import { chatNeedsOf, chatSetSaid, type ChatNeed, type ChatSkillsSaid } from "./chatSkillNeeds";
import { SKILLS_MISSING_PUT_AWAY, type ChatSkillsAsk } from "./chatSkillsAsk";

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
      /** What the chat in front's set says as a whole (S8, #3694): none with no chat in front. */
      said?: ChatSkillsSaid;
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
  /** What changed since each chat started: the answer the chats' panes read at this change. */
  updatedSinceStart: () => Promise<readonly PlaneUpdated[]> = async () => [],
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
  // And what its set says as a whole (S8): its "started without" notice's, and what changed
  // since it started, read with the rest so the view is drawn once.
  const [base, personas, set, updated, branchFolder] = await Promise.all([
    answered(commands.skillStudio(plane, chat.workspace ?? null, chat.persona)),
    answered(commands.skillPersonaSkills(plane, chat.workspace ?? null)),
    answered(commands.chatSkills(plane, chat.session)),
    updatedSinceStart(),
    // Its branch's repo skills (#4326), which the studio's rows of its clone do not speak for.
    answered(commands.chatBranchFolderSkills(plane, chat.session)),
  ]);
  if ("refused" in base) return base;
  const rows = viewRowsOf(base.ok, "ok" in personas ? personas.ok : undefined);
  const given: { skills: ChatSkill[]; repos: string[] } =
    "ok" in set ? set.ok : { skills: [], repos: [] };
  const said: ChatSkillsSaid = {
    budget: "ok" in set ? set.ok.budget : null,
    routing: "ok" in set ? set.ok.routing : null,
    // What it started without, from the same answer the Skills chip reads it from (#3953).
    missing: "ok" in set ? set.ok.missing : null,
    changed: updated.find((one) => one.session === chat.session && one.skills) ?? null,
    // Its program's own catalogue (#3324), from the studio answer read above: no read of its own.
    own: base.ok.harnesses.find((one) => one.harness === chat.harness)?.own ?? null,
  };
  return {
    rows: [
      ...narrowedTo(rows, given.skills, {
        persona: chat.persona,
        harness: chat.harness,
        repos: given.repos,
      }),
      ...branchFolderRowsOf("ok" in branchFolder ? branchFolder.ok : []),
    ],
    first: base.ok.first,
    levels: base.ok.levels,
    pinsRefused: base.ok.pins_refused,
    said,
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
  onRestart,
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
  /** Restarts chat `session` on its conversation: what a chat whose skills changed is offered. */
  onRestart?: (session: number) => void | Promise<void>;
}) {
  const [folded, onFold] = useFoldedGroups();
  // Where the rows are read for, and where the switch and the pin act: the chat's own workspace
  // while one is in front, else the title bar's.
  const place = chat === undefined ? workspace : chat.workspace;
  const persona = chat?.persona ?? null;
  const changed = useViewChanged([plane], skillsIn(place));
  // What changed since its chat started is asked once a change, with the chats' panes (S8).
  const instructions = usePlaneChanged([plane], INSTRUCTIONS);
  // Asked again after the chat in front is restarted from here: it then has what changed.
  const [restarted, setRestarted] = useState(0);
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
    void readSkills(plane, place, of, () => planeUpdatedAt(plane, instructions))
      .catch((err: unknown): Read => ({ refused: String(err) }))
      .then((answer) => {
        if (live) setRead({ of, read: answer });
      });
    return () => {
      live = false;
    };
    // The name is not read by: it only says whose set it is.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plane, place, session, chatWorkspace, persona, harness, changed, instructions, restarted]);

  // Its "started without" notice put away is put away in the core: its card goes as the Skills
  // chip's warning does (#3953), with no read again.
  useEffect(() => {
    if (session === undefined) return;
    const hear = (event: Event) => {
      const { detail } = event as CustomEvent<ChatSkillsAsk>;
      if (detail.plane !== plane || detail.session !== session) return;
      setRead((was) =>
        was === undefined || !("rows" in was.read) || was.read.said === undefined
          ? was
          : { ...was, read: { ...was.read, said: { ...was.read.said, missing: null } } },
      );
    };
    window.addEventListener(SKILLS_MISSING_PUT_AWAY, hear);
    return () => window.removeEventListener(SKILLS_MISSING_PUT_AWAY, hear);
  }, [plane, session]);

  const rows = read !== undefined && "rows" in read.read ? read.read.rows : NO_ROWS;
  const nameOf = useCallback((row: ListedSkill) => row.skill.name, []);
  const filter = useListFilter(rows, nameOf);
  const groups = useMemo(() => groupsOf(filter.shown), [filter.shown]);
  // The header counts the whole set, whatever the filter leaves (S7).
  const whole = useMemo(() => groupsOf(rows), [rows]);
  // A built-in skill a skill of the project has the name of stands out of the fold (S12).
  const clashing = useMemo(
    () =>
      new Set(rows.filter((row) => row.skill.level !== "built-in").map((row) => row.skill.name)),
    [rows],
  );

  // While another chat's set is read, the list stays as it was: it is replaced once it is known.
  if (read === undefined) return <SkillsReading />;
  if ("refused" in read.read)
    return <SkillsNotRead refused={read.read.refused} testid="skills-view-refused" />;
  const shown = read.read;
  // **What a card acts on is the chat its rows were read for** (the one shown), never a chat
  // put in front a moment ago whose set is still being read: Use types into that chat, and the
  // switch and the pin act on its workspace.
  const acting = read.of;
  const actsIn = acting === undefined ? workspace : acting.workspace;
  const actsAs = acting?.persona ?? null;
  // Whose set is shown: the one it was read for, which is the chat in front once it is read.
  const whose =
    read.of !== undefined
      ? read.of.session === session
        ? chatName
        : read.of.name
      : workspace === undefined
        ? "A new chat at the project's root"
        : `A new chat in ${workspace}`;

  const scope: StudioScope = actsIn === undefined ? AT_THE_ROOT : inWorkspace(actsIn);
  const studioAt = (page: StudioPage, skill?: string, newTab = false) =>
    onOpenView(skillStudioAt(scope, page, { skill, newTab }), SKILL_STUDIO_TITLE);
  /** The studio on this view's scope, at its Library (#3681); a tab of its own on ⌘-click. */
  const openStudio = (newTab = false) => studioAt("library", undefined, newTab);
  /** **Its `SKILL.md`** in a tab: Open the file, and the press of a branch folder's copy. */
  const openFile = (row: ListedSkill) => {
    const at = filePlaceOf(row.skill);
    if ("reason" in at) return;
    const of =
      at.repo === ""
        ? PLANE_ROOT
        : { workspace: at.workspace, repo: at.repo, piece: at.piece ?? null };
    onOpenView(pieceFileView(of, at.path), `SKILL.md · ${row.skill.name}`);
  };
  /** A card in the studio, at Library › it, for the view's scope (S7); a program's own or a
   *  repo's skill, which the Library does not list, on the Harness skills page. A branch folder's
   *  copy is on neither (#4376), so it opens as its file. */
  const openRow = (row: ListedSkill) => {
    if (branchFolderOf(row.skill) !== undefined) {
      openFile(row);
      return;
    }
    onOpenView(
      row.skill.level === "installed" || row.skill.level === "repo"
        ? skillStudioAt(scope, "harness")
        : skillStudioAt(scopeOfRow(row, actsIn, actsAs), "library", {
            skill: skillRowId(row.skill),
          }),
      SKILL_STUDIO_TITLE,
    );
  };
  /** **New skill…** (#3684): the studio's own dialog, which asks how; the studio goes to its
   *  Library first, and the dialog opens there. */
  const newSkill = () => {
    openStudio();
    askStudioStart("", "new");
  };
  const change = (row: ListedSkill, what: "turn" | "pin") => {
    const asked = what === "pin" ? pinOf(row, actsIn) : turnOf(row.skill, actsIn);
    const where = actsIn;
    if (!("label" in asked) || where === undefined) return;
    setBusy(true);
    // The pin is the studio's own command (SK-15d), for this workspace's chats: one commit.
    void (
      what === "pin"
        ? commands.skillPin(plane, { at: "workspace", name: where }, row.skill.name, asked.on)
        : commands.skillTurn(plane, where, row.skill.name, asked.on)
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
    if (acting !== undefined) onUseSkill(acting.session, row.skill.name);
  };
  const act = (row: ListedSkill, what: SkillAct) => {
    if (what === "use") typeIn(row);
    else if (what === "studio") openRow(row);
    else if (what === "explorer") {
      // Explorer shows the title bar's workspace, whichever chat is in front.
      const at = explorerPlaceOf(row.skill, workspace);
      if (!("reason" in at)) onShowInExplorer(at);
    } else if (what === "file") openFile(row);
    else change(row, what);
  };
  const fix = (row: ListedSkill, one: SkillFix) => {
    if (one.act === "turn") change(row, "turn");
    else if (one.act === "routing") studioAt("routing");
    else openRow(row);
  };
  // **The chat's own needs** (S8): its budget, pins, router, what it started without and what
  // changed, each with its one fix; for the chat its rows were read for alone.
  const chatNeeds: ChatNeed[] =
    acting === undefined || shown.said === undefined ? [] : chatNeedsOf(shown.said, scope);
  const chatFix = (need: ChatNeed) => {
    const one = need.fix;
    if (one.act === "restart") {
      if (acting === undefined || onRestart === undefined) return;
      void Promise.resolve(onRestart(acting.session)).then(() => setRestarted((was) => was + 1));
      return;
    }
    onOpenView(skillStudioAt(one.scope, one.page), SKILL_STUDIO_TITLE);
  };
  const usable = (row: ListedSkill) => {
    const mark = markOf(row);
    return acting !== undefined && mark !== "left-out" && mark !== "off" && mark !== "needs-review";
  };

  return (
    <SkillsList
      whose={whose}
      chat={read.of !== undefined}
      setSaid={shown.said === undefined ? [] : chatSetSaid(shown.said)}
      chatNeeds={filter.query.trim() === "" ? chatNeeds : []}
      onChatFix={chatFix}
      groups={groups}
      count={whole.needs.length + whole.inChat.length + whole.builtIns.length}
      clashes={(row) => clashing.has(row.skill.name)}
      total={rows.length}
      query={filter.query}
      onQuery={filter.setQuery}
      filtered={filter.filtered}
      starts={shown.first.starts}
      busy={busy}
      onNewSkill={newSkill}
      onOpenStudio={openStudio}
      onOpen={openRow}
      usable={usable}
      onUse={typeIn}
      fixOf={(row) => fixOf(row, actsIn)}
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
              workspace: actsIn,
              chat: acting !== undefined,
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
