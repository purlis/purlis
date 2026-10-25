// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { commands, type Studio, type StudioFirst, type StudioLevel } from "./bindings";
import type { Offer } from "./actions";
import type { GroupDrawn } from "./menuModel";
import { Groups } from "./Menus";
import { Notice } from "./Notice";
import { useViewChanged } from "./planeChanged";
import { skillView } from "./skillRuns";
import { levelFrom, levelsFor } from "./skillChanges";
import { askSkillChat } from "./skillDraftAsks";
import { SkillsNotRead, SkillsReading, type SkillStart } from "./SkillStarts";
import {
  AT_THE_ROOT,
  askStudioStart,
  inWorkspace,
  skillStudioTitle,
  skillStudioView,
  type StudioScope,
} from "./skillStudio";
import { SkillsList } from "./SkillsList";
import {
  groupsOf,
  listedOf,
  menuRowsOf,
  personaRowsOf,
  personasOf,
  turnOf,
  type ListedSkill,
  type SkillAct,
} from "./skillsListing";
import type { ViewRef } from "./tabs";
import { useListFilter } from "./ui/list/ListFilter";
import { Menu } from "./ui/menu/Menu";

/** The chat in front, as the Skills view reads its set: its tab's name, where it works, and the
 *  persona it runs as. */
export type FrontChat = {
  session: number;
  name: string;
  /** Its workspace, or `undefined` for a chat at the project's root. */
  workspace: string | undefined;
  persona: string | null;
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

/** One studio answer, or its refusal as a sentence. */
async function studioOf(
  plane: string,
  workspace: string | undefined,
  persona: string | null,
): Promise<{ ok: Studio } | { refused: string }> {
  const answer = await commands
    .skillStudio(plane, workspace ?? null, persona)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  return answer.status === "ok" ? { ok: answer.data } : { refused: answer.error };
}

/**
 * **Reads what the view lists** from the Skill studio's own answer (the amendment's (f)): the
 * workspace's chats' skills, each program's own and each repo's, and each persona's own, which a
 * persona brings into every workspace; or, narrowed to a chat, that chat's set alone.
 */
async function readSkills(
  plane: string,
  workspace: string | undefined,
  chat: FrontChat | undefined,
): Promise<Read> {
  if (chat !== undefined) {
    const one = await studioOf(plane, chat.workspace, chat.persona);
    if ("refused" in one) return one;
    return {
      rows: listedOf(one.ok, chat.persona ?? undefined),
      first: one.ok.first,
      levels: one.ok.levels,
    };
  }
  const base = await studioOf(plane, workspace, null);
  if ("refused" in base) return base;
  const personas = await Promise.all(
    personasOf(base.ok).map(async (persona) => {
      const theirs = await studioOf(plane, workspace, persona);
      return "ok" in theirs ? personaRowsOf(persona, theirs.ok) : [];
    }),
  );
  return {
    rows: [...listedOf(base.ok), ...personas.flat()],
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
 * Use skill… opened on it, the switch is the studio's, Show in Explorer opens Explorer on its
 * folder, and New skill is the studio's three starts.
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
  const changed = useViewChanged([plane], undefined);
  const [done, setDone] = useState(0);
  const [only, setOnly] = useOnlyThisChat();
  const narrowed = only && chat !== undefined ? chat : undefined;
  const [read, setRead] = useState<Read>();
  const [said, setSaid] = useState<{ label: string; text: string }>();
  const [busy, setBusy] = useState(false);
  // What the chat's set is read by: its session's place and persona, not its tab's name.
  const chatKey =
    narrowed === undefined
      ? ""
      : JSON.stringify([narrowed.session, narrowed.workspace, narrowed.persona]);
  useEffect(() => {
    let live = true;
    void readSkills(plane, workspace, narrowed)
      .catch((err: unknown): Read => ({ refused: String(err) }))
      .then((answer) => {
        if (live) setRead(answer);
      });
    return () => {
      live = false;
    };
    // `chatKey` stands for the chat it reads.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plane, workspace, chatKey, changed, done]);

  const rows = read !== undefined && "rows" in read ? read.rows : NO_ROWS;
  const nameOf = useCallback((row: ListedSkill) => row.skill.name, []);
  const filter = useListFilter(rows, nameOf);
  // Where the rows' switch and Explorer act: the chat's own place while narrowed to it.
  const place = narrowed === undefined ? workspace : narrowed.workspace;
  const groups = useMemo(
    () => groupsOf(filter.shown, { root: place === undefined, chat: narrowed !== undefined }),
    [filter.shown, place, narrowed],
  );

  if (read === undefined) return <SkillsReading />;
  if ("refused" in read)
    return <SkillsNotRead refused={read.refused} testid="skills-view-refused" />;

  const scope: StudioScope = workspace === undefined ? AT_THE_ROOT : inWorkspace(workspace);
  const openStudio = () => onOpenView(skillStudioView(scope), skillStudioTitle(scope));
  /** One of the studio's three starts (SK-22): a chat's draft here, the others in the studio. */
  const startWith = (start: SkillStart) => {
    if (start === "draft") {
      askSkillChat({
        plane,
        kind: "draft",
        level: levelFrom(scope, levelsFor(read.levels, scope)),
      });
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
    const turn = turnOf(row.skill, place);
    if (!("label" in turn) || place === undefined) return;
    setBusy(true);
    void commands
      .skillTurn(plane, place, row.skill.name, turn.on)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((answer) => {
        if (answer.status === "error") setSaid({ label: turn.refused, text: answer.error });
        else setSaid(undefined);
      })
      .finally(() => {
        setBusy(false);
        setDone((n) => n + 1);
      });
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

/**
 * **A row's menu** (the amendment's (b)): Use in this chat, the switch and Show in Explorer,
 * drawn as every menu is (`Menus.Groups`), each that cannot run saying why.
 */
function SkillMenu({
  row,
  at,
  busy,
  onAct,
}: {
  row: ListedSkill;
  at: { workspace: string | undefined; chat: boolean };
  busy: boolean;
  onAct: (what: SkillAct) => void;
}) {
  const rows = menuRowsOf(row, at);
  const group: GroupDrawn = {
    entries: rows.map((one) => {
      const available = one.available && !(busy && one.act === "turn");
      return {
        kind: "row",
        offer: {
          id: `skills.${one.act}`,
          title: one.title,
          available,
          reason: available ? "" : one.reason || "A change is on its way.",
          does: { verb: "nothing" },
        } satisfies Offer,
        said: one.title,
        danger: false,
      };
    }),
  };
  return (
    <Menu aria-label={row.skill.name} className="item-menu">
      <Groups
        groups={[group]}
        onPress={(offer) => {
          const one = rows.find((r) => `skills.${r.act}` === offer.id);
          if (one !== undefined) onAct(one.act);
        }}
      />
    </Menu>
  );
}
