// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import {
  commands,
  type ChatSkill,
  type HarnessKind,
  type StudioFirst,
  type StudioLevel,
} from "./bindings";
import { Notice } from "./Notice";
import { skillsIn, useViewChanged } from "./planeChanged";
import { skillView } from "./skillRuns";
import { askDraft } from "./skillDraftAsks";
import { SkillsNotRead, SkillsReading, type SkillStart } from "./SkillStarts";
import {
  AT_THE_ROOT,
  askStudioStart,
  inWorkspace,
  skillStudioTitle,
  skillStudioView,
  type StudioScope,
} from "./skillStudio";
import { SkillMenu, SkillsList } from "./SkillsList";
import {
  groupsOf,
  listedOf,
  narrowedTo,
  personaRowsOf,
  pinOf,
  turnOf,
  type ListedSkill,
  type SkillAct,
} from "./skillsListing";
import type { ViewRef } from "./tabs";
import { useListFilter } from "./ui/list/ListFilter";

/** The chat in front, as the Skills view reads its set: its session, where it works, and the
 *  persona it runs as. */
export type FrontChat = {
  session: number;
  /** Its workspace, or `undefined` for a chat at the project's root. */
  workspace: string | undefined;
  persona: string | null;
  /** The program it runs, by the word the project calls it: whose own skills it gets. */
  harness: HarnessKind | undefined;
};

/** **Whether the list is narrowed to the chat in front** (the amendment's (c)): one switch for
 *  the window, whichever project is in front, kept while the window is open. */
let onlyThisChat = false;
const onlyListeners = new Set<() => void>();

function useOnlyThisChat(): [boolean, (only: boolean) => void] {
  const only = useSyncExternalStore(
    (listener) => {
      onlyListeners.add(listener);
      return () => void onlyListeners.delete(listener);
    },
    () => onlyThisChat,
  );
  const set = useCallback((to: boolean) => {
    onlyThisChat = to;
    for (const one of onlyListeners) one();
  }, []);
  return [only, set];
}

/** What the view read: its rows, what the studio offers to start from, and the levels a new
 *  skill can land at; or the core's refusal. */
type Read =
  { rows: ListedSkill[]; first: StudioFirst; levels: StudioLevel[] } | { refused: string };

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

/**
 * **Reads what the view lists** from the Skill studio's own answer (the amendment's (f)): the
 * workspace's chats' skills, each program's own and each repo's, then every persona's own, which
 * a persona brings into every workspace, in one more read.
 */
async function readSkills(plane: string, workspace: string | undefined): Promise<Read> {
  const [base, personas] = await Promise.all([
    answered(commands.skillStudio(plane, workspace ?? null, null)),
    answered(commands.skillPersonaSkills(plane, workspace ?? null)),
  ]);
  if ("refused" in base) return base;
  return {
    rows: [...listedOf(base.ok), ...("ok" in personas ? personaRowsOf(personas.ok) : [])],
    first: base.ok.first,
    levels: base.ok.levels,
  };
}

/**
 * **The Skills view** (SK-55, #3266; decisions amendment 2026-10-26): a sidebar view of this
 * workspace's skills by level, behind the Skill studio's preview. It follows the title bar's
 * workspace, and reads again whenever the project's skills change on disk, as the studio does.
 *
 * Every act is one the window already has: a row opens the skill's tab, Use in this chat is
 * Use skill… opened on it, the switch and the pin are the studio's, Show in Explorer opens
 * Explorer on its folder, and New skill is the studio's three starts.
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
  /** Opens Explorer on `folder`, one of the project's own files. */
  onShowInExplorer: (folder: string) => void;
}) {
  const [only, setOnly] = useOnlyThisChat();
  const narrowed = only && chat !== undefined ? chat : undefined;
  // Where the rows are read for, and where the switch and Explorer act: the chat's own place
  // while narrowed to it, else the title bar's.
  const place = narrowed === undefined ? workspace : narrowed.workspace;
  const changed = useViewChanged([plane], skillsIn(place));
  const [read, setRead] = useState<Read>();
  // The chat in front's real set, what Use skill… types from, while the list is narrowed to it.
  const [chatSet, setChatSet] = useState<{ session: number; skills: ChatSkill[] }>();
  const [said, setSaid] = useState<{ label: string; text: string }>();
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let live = true;
    void readSkills(plane, place)
      .catch((err: unknown): Read => ({ refused: String(err) }))
      .then((answer) => {
        if (live) setRead(answer);
      });
    return () => {
      live = false;
    };
  }, [plane, place, changed]);
  const session = narrowed?.session;
  useEffect(() => {
    let live = true;
    if (session === undefined) return;
    void answered(commands.chatSkills(plane, session)).then((answer) => {
      if (live) setChatSet({ session, skills: "ok" in answer ? answer.ok.skills : [] });
    });
    return () => {
      live = false;
    };
  }, [plane, session, changed]);

  const base = read !== undefined && "rows" in read ? read.rows : NO_ROWS;
  const persona = narrowed?.persona ?? null;
  const harness = narrowed?.harness;
  const set = chatSet !== undefined && chatSet.session === session ? chatSet.skills : undefined;
  const rows = useMemo(
    () =>
      session === undefined || set === undefined
        ? base
        : narrowedTo(base, set, { persona, harness }),
    [base, session, set, persona, harness],
  );
  const nameOf = useCallback((row: ListedSkill) => row.skill.name, []);
  const filter = useListFilter(rows, nameOf);
  const groups = useMemo(
    () => groupsOf(filter.shown, { root: place === undefined, chat: narrowed !== undefined }),
    [filter.shown, place, narrowed],
  );

  // While the chat's set is read, the list stays as it was: it is narrowed once it is known.
  if (read === undefined) return <SkillsReading />;
  if ("refused" in read)
    return <SkillsNotRead refused={read.refused} testid="skills-view-refused" />;

  const scope: StudioScope = workspace === undefined ? AT_THE_ROOT : inWorkspace(workspace);
  const openStudio = () => onOpenView(skillStudioView(scope), skillStudioTitle(scope));
  /** One of the studio's three starts (SK-22): a chat's draft here, the others in the studio. */
  const startWith = (start: SkillStart) => {
    if (start === "draft") {
      askDraft(plane, scope, read.levels);
      return;
    }
    askStudioStart(scope, start === "blank" ? "new" : "sources");
    openStudio();
  };
  const act = (row: ListedSkill, what: SkillAct) => {
    if (what === "use") {
      if (chat !== undefined) onUseSkill(chat.session, row.skill.name);
      return;
    }
    if (what === "explorer") {
      onShowInExplorer(row.skill.folder);
      return;
    }
    const change = what === "pin" ? pinOf(row.skill, place) : turnOf(row.skill, place);
    if (!("label" in change) || place === undefined) return;
    setBusy(true);
    // The pin is the studio's own command (SK-15d), for this workspace's chats: one commit.
    void (
      what === "pin"
        ? commands.skillPin(plane, { at: "workspace", name: place }, row.skill.name, change.on)
        : commands.skillTurn(plane, place, row.skill.name, change.on)
    )
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((answer) => {
        // Done, it is read again as the project's skills change on disk: one read, not two.
        if (answer.status === "error") setSaid({ label: change.refused, text: answer.error });
        else setSaid(undefined);
      })
      .finally(() => setBusy(false));
  };

  return (
    <SkillsList
      groups={groups}
      total={rows.length}
      query={filter.query}
      onQuery={filter.setQuery}
      filtered={filter.filtered}
      starts={read.first.starts && narrowed === undefined}
      busy={busy}
      onStart={startWith}
      onOpenStudio={openStudio}
      onOpen={(row) => onOpenView(skillView(row.skill.name), row.skill.name)}
      menuOf={(row) => ({
        menu: () => (
          <SkillMenu
            row={row}
            at={{ workspace: place, chat: chat !== undefined }}
            busy={busy}
            onAct={(what) => act(row, what)}
          />
        ),
      })}
      chat={chat === undefined ? undefined : { only, onOnly: setOnly }}
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
