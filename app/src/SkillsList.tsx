// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, type ReactNode } from "react";
import { Plus, Sparkle, Sparkles } from "lucide-react";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { List, type RowMenu } from "./ui/list/List";
import { ListCardRow } from "./ui/list/CardRow";
import { ListFoldSection, ListRun } from "./ui/list/Fold";
import { ListFilter, noMatchSaid, type Noun } from "./ui/list/ListFilter";
import { Menu } from "./ui/menu/Menu";
import { NEW_SKILL_MENU, SkillStarts } from "./SkillStarts";
import { BUILT_IN_FIXED, MAKE_MY_OWN_COPY } from "./SkillCopy";
import type { Offer } from "./actions";
import { counted } from "./dispatches";
import type { GroupDrawn } from "./menuModel";
import { Groups } from "./Menus";
import { OPEN_SKILL_STUDIO } from "./skillStudio";
import type { ChatNeed } from "./chatSkillNeeds";
import type { PluginOffShown } from "./bindings";
import {
  cardReasonOf,
  menuRowsOf,
  type GroupId,
  type ListedSkill,
  type MenuAt,
  type SkillAct,
  type SkillFix,
  type SkillGroups,
} from "./skillsListing";

/** What the view's filter calls its rows. */
const SKILLS_NOUN: Noun = { one: "skill", many: "skills" };

/** The middle group's heading while a chat is in front: the skills it gets. */
export const IN_THIS_CHAT = "In this chat";
/** The same group while no chat is in front: what a new chat in the workspace would get, as the
 *  header says ("A new chat in ops"). */
export const A_NEW_CHAT_GETS = "A new chat gets";

/**
 * **The Skills view, drawn** (SK-55, #3266; spec #3679 S7, `prototype/skills-sidebar` variant
 * C): whose skills it shows and how many, its header's way to the Skill studio and its New
 * skill, the filter, and the skills as small cards in three groups that fold: Needs you (in the
 * waiting colour), In this chat (its built-in skills folded to one quiet line at its end, S12;
 * headed A new chat gets while no chat is in front, #3778) and Left out (folded at first). What it lists and does is the caller's (`SkillsView`); this
 * draws it, so a story draws it without a core.
 *
 * **A card** is a skill's name and its skill level, Use while the chat in front can use it, and
 * its ⋯; two lines of what it does, or why it is as it is in their place; and its one fix. Enter
 * or a click opens it in the Skill studio; its menu (the ⋯, a right-click, Shift+F10 or the menu
 * key) is the caller's.
 */
export function SkillsList({
  whose,
  chat,
  setSaid = [],
  chatNeeds = [],
  onChatFix,
  groups,
  count,
  clashes,
  total,
  query,
  onQuery,
  filtered,
  starts,
  busy,
  onNewSkill,
  onOpenStudio,
  onOpen,
  menuOf,
  usable,
  onUse,
  fixOf,
  onFix,
  onCopy,
  notice,
  folded,
  onFold,
}: {
  /** Whose skills these are: the chat in front's name, or "A new chat in ops". */
  whose: string;
  /** Whether they are a chat's, one in front: its group is In this chat, and else the set a new
   *  chat would get is headed A new chat gets (#3778). */
  chat: boolean;
  /** What the chat's set says as a whole, under whose it is: its budget and its routing (S8). */
  setSaid?: readonly string[];
  /** **What the chat needs of the person as a whole** (S8, #3694), first in Needs you, each with
   *  its one fix: its budget, pins, router, what it started without and what changed. */
  chatNeeds?: readonly ChatNeed[];
  onChatFix?: (need: ChatNeed) => void;
  groups: SkillGroups;
  /** How many skills the chat gets, whatever the filter leaves: the header's count. */
  count: number;
  /** Whether a built-in skill shares its name with a skill of the project's, so it stands out
   *  of the fold (S12). */
  clashes: (row: ListedSkill) => boolean;
  /** How many skills the list holds before the filter. */
  total: number;
  query: string;
  onQuery: (query: string) => void;
  /** Whether the list is long enough to take a filter (`ui/list/ListFilter`). */
  filtered: boolean;
  /** Whether the project has no skill of its own, so its three starts are offered (SK-22). */
  starts: boolean;
  busy: boolean;
  /** New skill…: the studio's own dialog, which asks how (#3684). */
  onNewSkill: () => void;
  /** Opens the studio; `newTab` where ⌘ (Ctrl off a Mac) was held, for a tab of its own. */
  onOpenStudio: (newTab: boolean) => void;
  /** Opens the skill in the Skill studio, at Library › it. */
  onOpen: (row: ListedSkill) => void;
  menuOf: (row: ListedSkill) => RowMenu | undefined;
  /** Whether Use is drawn on a card: a chat is in front, and gets the skill. */
  usable: (row: ListedSkill) => boolean;
  onUse: (row: ListedSkill) => void;
  /** A card's one fix, where it has one. */
  fixOf: (row: ListedSkill) => SkillFix | undefined;
  onFix: (row: ListedSkill, fix: SkillFix) => void;
  /** Make my own copy of a built-in skill, in the Skill studio. */
  onCopy: () => void;
  /** What the last change from a card's menu left to say. */
  notice?: ReactNode;
  /** The groups the person folded, kept for the window (`skillsViewPrefs.ts`). */
  folded: readonly GroupId[];
  /** Folds (`true`) or unfolds the group `id`. */
  onFold: (id: GroupId, folded: boolean) => void;
}) {
  const listId = useId();
  const filtering = query.trim() !== "";
  const { needs, inChat, builtIns, leftOut } = groups;
  // **While a filter is typed every group is open**, so no match is folded away.
  const isFolded = (id: GroupId) => !filtering && folded.includes(id);
  const card = (row: ListedSkill) => {
    // A plugin's run says why once, on its line and its closing line (#4477).
    const reason = row.pluginOff === undefined ? cardReasonOf(row) : undefined;
    const fix = fixOf(row);
    return (
      <ListCardRow
        key={row.key}
        id={row.key}
        textValue={row.skill.name}
        name={row.skill.name}
        mark={<Badge>{row.skill.label}</Badge>}
        actions={
          usable(row) ? (
            <Button
              size="sm"
              variant="ghost"
              aria-label={`Use ${row.skill.name} in this chat`}
              onPress={() => onUse(row)}
            >
              Use
            </Button>
          ) : undefined
        }
        description={reason === undefined ? row.skill.description : undefined}
        reason={reason}
        fix={
          fix === undefined ? undefined : (
            <Button size="sm" isDisabled={busy} onPress={() => onFix(row, fix)}>
              {fix.label}
            </Button>
          )
        }
        menu={menuOf(row)}
        data-testid={`skills-row-${row.key}`}
      />
    );
  };
  /** A need of the chat's own: a card with its title, what it says in the waiting colour, and its
   *  fix. Enter or a click goes where the fix goes, as a skill's card opens it; a restart, which
   *  ends what the chat is doing, is only ever its button. */
  const needCard = (need: ChatNeed) => (
    <ListCardRow
      key={`need:${need.id}`}
      id={`need:${need.id}`}
      textValue={need.title}
      name={need.title}
      reason={{ says: need.says, tone: "warning" }}
      fix={
        <Button size="sm" isDisabled={busy} onPress={() => onChatFix?.(need)}>
          {need.fix.label}
        </Button>
      }
      data-testid={`skills-need-${need.id}`}
    />
  );
  const rows = [...needs, ...inChat, ...builtIns, ...leftOut];
  // The plugins this chat lacks, each in the order its first skill comes, then the rest.
  const pluginsOff = [
    ...new Map(
      leftOut.flatMap((row) =>
        row.pluginOff === undefined ? [] : [[row.pluginOff.name, row.pluginOff]],
      ) as [string, PluginOffShown][],
    ).values(),
  ];
  const needing = chatNeeds.length + needs.length;
  return (
    <div className="skills-view" data-testid="skills-view">
      <p className="skills-view-whose" data-testid="skills-view-whose">
        <strong>{whose}</strong>{" "}
        <span className="honest">· {counted(count, "skill", "skills")}</span>
      </p>
      {setSaid.length > 0 && (
        <p className="skills-view-said" data-testid="skills-view-said">
          {setSaid.join(" · ")}
        </p>
      )}
      <ButtonRow className="skills-view-head">
        <Button
          variant="ghost"
          icon={<Sparkles aria-hidden="true" />}
          onPress={(event) => onOpenStudio(event.metaKey || event.ctrlKey)}
        >
          {OPEN_SKILL_STUDIO}
        </Button>
        <Button
          variant="ghost"
          icon={<Plus aria-hidden="true" />}
          isDisabled={busy}
          onPress={onNewSkill}
        >
          {NEW_SKILL_MENU}
        </Button>
      </ButtonRow>
      {notice}
      {starts && <SkillStarts testid="skills-view-empty" />}
      {filtered && (
        <ListFilter
          noun={SKILLS_NOUN}
          total={total}
          query={query}
          onQuery={onQuery}
          controls={listId}
        />
      )}
      {/* The list's place in the view: it takes the rest, and scrolls. */}
      <div className="skills-view-list">
        <List
          id={listId}
          aria-label="Skills"
          size="sm"
          onAction={(key) => {
            const need = chatNeeds.find((one) => `need:${one.id}` === String(key));
            if (need !== undefined) {
              // A press on the card goes where its fix goes; a restart is only ever its button.
              if (need.fix.act !== "restart" && !busy) onChatFix?.(need);
              return;
            }
            const row = rows.find((one) => one.key === String(key));
            if (row !== undefined) onOpen(row);
          }}
          empty={
            filtering ? (
              <p className="honest">{noMatchSaid(SKILLS_NOUN, query)}</p>
            ) : starts ? undefined : (
              <p className="honest">No skill reaches a chat here.</p>
            )
          }
        >
          {rows.length + chatNeeds.length === 0 ? null : (
            <>
              {needing > 0 && (
                <ListFoldSection
                  id="needs-you"
                  title="Needs you"
                  count={needing}
                  tone="waiting"
                  folded={isFolded("needs-you")}
                  onFoldedChange={(to) => onFold("needs-you", to)}
                >
                  {chatNeeds.map(needCard)}
                  {needs.map(card)}
                </ListFoldSection>
              )}
              {inChat.length + builtIns.length > 0 && (
                <ListFoldSection
                  id="in-chat"
                  title={chat ? IN_THIS_CHAT : A_NEW_CHAT_GETS}
                  count={inChat.length + builtIns.length}
                  folded={isFolded("in-chat")}
                  onFoldedChange={(to) => onFold("in-chat", to)}
                >
                  {inChat.map(card)}
                  <ListRun
                    id="built-in"
                    items={builtIns}
                    standsOut={(row) => filtering || clashes(row)}
                    says={(n) => counted(n, "built-in skill", "built-in skills")}
                    mark={<Sparkle aria-hidden="true" />}
                    folded={isFolded("built-in")}
                    onFoldedChange={(to) => onFold("built-in", to)}
                    closing={{
                      says: BUILT_IN_FIXED,
                      children: (
                        <Button size="sm" variant="ghost" onPress={onCopy}>
                          {`${MAKE_MY_OWN_COPY}…`}
                        </Button>
                      ),
                    }}
                  >
                    {card}
                  </ListRun>
                </ListFoldSection>
              )}
              {leftOut.length > 0 && (
                <ListFoldSection
                  id="left-out"
                  title="Left out"
                  count={leftOut.length}
                  folded={isFolded("left-out")}
                  onFoldedChange={(to) => onFold("left-out", to)}
                >
                  {pluginsOff.map((plugin) => (
                    <ListRun
                      key={plugin.name}
                      id={`plugin-off:${plugin.name}`}
                      items={leftOut.filter((row) => row.pluginOff?.name === plugin.name)}
                      says={(n) =>
                        `${plugin.name}: ${counted(n, "skill", "skills")}. ${plugin.line}`
                      }
                      // While a filter is typed no match is folded away.
                      {...(filtering ? { folded: false } : {})}
                      closing={{
                        says: plugin.said,
                        children:
                          plugin.command === null ? undefined : (
                            <Button
                              size="sm"
                              variant="ghost"
                              onPress={() =>
                                void navigator.clipboard
                                  ?.writeText(plugin.command ?? "")
                                  .catch(() => undefined)
                              }
                            >
                              Copy command
                            </Button>
                          ),
                      }}
                    >
                      {card}
                    </ListRun>
                  ))}
                  {leftOut.filter((row) => row.pluginOff === undefined).map(card)}
                </ListFoldSection>
              )}
            </>
          )}
        </List>
      </div>
    </div>
  );
}

/**
 * **A card's menu** (spec #3679 S7): Use in this chat, the switch, Pin to this workspace, Show
 * in the Explorer view, Open the file and Open in Skill studio, drawn as every menu is
 * (`Menus.Groups`), each that cannot run saying why. While a change is on its way the switch and
 * the pin wait.
 */
export function SkillMenu({
  row,
  at,
  busy,
  onAct,
}: {
  row: ListedSkill;
  at: MenuAt;
  busy: boolean;
  onAct: (what: SkillAct) => void;
}) {
  const rows = menuRowsOf(row, at);
  const group: GroupDrawn = {
    entries: rows.map((one) => {
      const waits = busy && (one.act === "turn" || one.act === "pin");
      const available = one.available && !waits;
      return {
        kind: "row",
        offer: {
          id: `skills.${one.act}`,
          title: one.title,
          available,
          reason: available ? "" : waits ? "A change is on its way." : one.reason,
          does: { verb: "nothing" },
        } satisfies Offer,
        said: one.title,
        danger: false,
        // A pin no chat reads (#3432): said under the row while it can run, and as its
        // description, never only on hover.
        ...(available && one.note !== undefined ? { warns: one.note } : {}),
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
