// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState, type ReactNode } from "react";
import { Plus, Sparkles } from "lucide-react";
import { Button, ButtonRow, ToggleButton } from "./ui/button/Button";
import type { RowMenu } from "./ui/list/List";
import { ListFilter, noMatchSaid, type Noun } from "./ui/list/ListFilter";
import { RowLabel, RowTrail } from "./ui/list/Row";
import { Tree, TreeRow, type Key } from "./ui/tree/Tree";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "./ui/menu/Menu";
import { SKILL_STARTS, NEW_SKILL_MENU, SkillStarts, type SkillStart } from "./SkillStarts";
import type { Offer } from "./actions";
import type { GroupDrawn } from "./menuModel";
import { Groups } from "./Menus";
import { OPEN_SKILL_STUDIO } from "./skillStudio";
import {
  MARK_SAID,
  markOf,
  menuRowsOf,
  tipOf,
  type ListedSkill,
  type MenuAt,
  type GroupId,
  type SkillAct,
  type SkillGroup,
} from "./skillsListing";

/** What the view's filter calls its rows. */
const SKILLS_NOUN: Noun = { one: "skill", many: "skills" };

/** A group's row's key, apart from every skill's. */
const groupKey = (id: GroupId): Key => `group:${id}`;

/** How many skills a group holds, to a reader: its count is drawn, and heard after its name. */
const countSaid = (count: number): string => `${count} ${count === 1 ? "skill" : "skills"}`;

/** The words of the switch that narrows the list to the chat in front (the amendment's (c)). */
const ONLY_THIS_CHATS = "Only this chat's";

/**
 * **The Skills view, drawn** (SK-55, #3266): its header's way to the Skill studio and its New
 * skill, the switch to the chat in front's own set, the filter, and the skills by level, on the
 * tree family (#2911, #3281). What it lists and does is the caller's (`SkillsView`); this draws
 * it, so a story draws it without a core.
 *
 * **Each level is a branch of the tree**, its name and how many skills it holds, which folds and
 * opens with ← and → or its twist, Enter or a click; the folds are the caller's to keep. One Tab
 * stop, ↑ and ↓, and typing finds a skill or a level by its name (the WAI-ARIA tree's keys).
 *
 * **A row is a skill's name and at most one mark**, its description and the mark's reason on
 * hover and while the keyboard is on it (`ui/list/RowTip`). Enter or a click opens its tab; its
 * menu (a right-click, Shift+F10 or the menu key) is the caller's.
 */
export function SkillsList({
  groups,
  total,
  query,
  onQuery,
  filtered,
  starts,
  busy,
  onStart,
  onOpenStudio,
  onOpen,
  menuOf,
  chat,
  notice,
  folded,
  onFold,
}: {
  groups: readonly SkillGroup[];
  /** How many skills the list holds before the filter. */
  total: number;
  query: string;
  onQuery: (query: string) => void;
  /** Whether the list is long enough to take a filter (`ui/list/ListFilter`). */
  filtered: boolean;
  /** Whether the project has no skill of its own, so its three starts are offered (SK-22). */
  starts: boolean;
  busy: boolean;
  onStart: (start: SkillStart) => void;
  onOpenStudio: () => void;
  onOpen: (row: ListedSkill) => void;
  menuOf: (row: ListedSkill) => RowMenu | undefined;
  /** The switch to the chat in front's own set, drawn while a chat tab is in front. */
  chat?: { only: boolean; onOnly: (only: boolean) => void };
  /** What the last change from a row's menu left to say. */
  notice?: ReactNode;
  /** The groups the person folded, kept for the window (`skillsViewPrefs.ts`). */
  folded: readonly GroupId[];
  /** The groups `shown` were left with `open` open, by the arrows or a twist. */
  onFold: (shown: readonly GroupId[], open: ReadonlySet<GroupId>) => void;
}) {
  const listId = useId();
  const filtering = query.trim() !== "";
  // **While a filter is typed every group is open**, so no match is folded out of sight; a group
  // folded meanwhile is folded for that filter alone, and the kept folds wait for it to clear.
  const [filterFolds, setFilterFolds] = useState<{ query: string; groups: readonly GroupId[] }>();
  const shown = groups.map((group) => group.id);
  const closed = filtering ? (filterFolds?.query === query ? filterFolds.groups : []) : folded;
  const open = new Set(shown.filter((id) => !closed.includes(id)).map((id): Key => groupKey(id)));
  return (
    <div className="skills-view" data-testid="skills-view">
      <ButtonRow className="skills-view-head">
        <Button variant="ghost" icon={<Sparkles aria-hidden="true" />} onPress={onOpenStudio}>
          {OPEN_SKILL_STUDIO}
        </Button>
        <MenuTrigger>
          <Button variant="ghost" icon={<Plus aria-hidden="true" />} isDisabled={busy}>
            {NEW_SKILL_MENU}
          </Button>
          {/* From the button's end, and narrow, so it opens inside the view (#3308). */}
          <Menu
            align="end"
            className="skills-view-starts"
            aria-label="Ways to start a skill"
            onAction={(key) => onStart(String(key) as SkillStart)}
          >
            {SKILL_STARTS.map(({ start, label, about }) => (
              <MenuItem key={start} id={start} textValue={label}>
                <MenuWords said={label} under={about} />
              </MenuItem>
            ))}
          </Menu>
        </MenuTrigger>
      </ButtonRow>
      {chat !== undefined && (
        <ToggleButton className="skills-view-only" isSelected={chat.only} onChange={chat.onOnly}>
          {ONLY_THIS_CHATS}
        </ToggleButton>
      )}
      {notice}
      {starts && (
        <SkillStarts
          busy={busy}
          onStart={onStart}
          from={NEW_SKILL_MENU}
          testid="skills-view-empty"
        />
      )}
      {filtered && (
        <ListFilter
          noun={SKILLS_NOUN}
          total={total}
          query={query}
          onQuery={onQuery}
          controls={listId}
        />
      )}
      {/* The tree's place in the view: it takes the rest, and scrolls. */}
      <div className="skills-view-list">
        <Tree
          id={listId}
          aria-label="Skills"
          size="sm"
          expandedKeys={open}
          onExpandedChange={(keys) => {
            const opened = new Set(
              groups.filter((group) => keys.has(groupKey(group.id))).map((group) => group.id),
            );
            if (filtering) setFilterFolds({ query, groups: shown.filter((id) => !opened.has(id)) });
            else onFold(shown, opened);
          }}
          empty={
            filtering ? (
              <p className="honest">{noMatchSaid(SKILLS_NOUN, query)}</p>
            ) : starts ? undefined : (
              <p className="honest">No skill reaches a chat here.</p>
            )
          }
        >
          {groups.map((group) => (
            <TreeRow
              key={group.id}
              id={groupKey(group.id)}
              textValue={group.title}
              description={countSaid(group.rows.length)}
              data-testid={`skills-group-${group.id}`}
              content={
                <>
                  <RowLabel>{group.title}</RowLabel>
                  <RowTrail>{group.rows.length}</RowTrail>
                </>
              }
            >
              {group.rows.map((row) => {
                const mark = markOf(row);
                const tip = tipOf(row);
                return (
                  <TreeRow
                    key={row.key}
                    id={row.key}
                    textValue={row.skill.name}
                    onAction={() => onOpen(row)}
                    tip={
                      tip.length === 0
                        ? undefined
                        : tip.map((line, at) => (
                            <span key={at} className="skills-view-tip">
                              {line}
                            </span>
                          ))
                    }
                    menu={menuOf(row)}
                    data-testid={`skills-row-${row.key}`}
                    data-mark={mark}
                    content={
                      <>
                        <RowLabel>{row.skill.name}</RowLabel>
                        {mark !== undefined && (
                          <RowTrail className="skills-view-mark">{MARK_SAID[mark]}</RowTrail>
                        )}
                      </>
                    }
                  />
                );
              })}
            </TreeRow>
          ))}
        </Tree>
      </div>
    </div>
  );
}

/**
 * **A row's menu** (the amendment's (b)): Use in this chat, the switch, Pin to this workspace and
 * Show in Explorer, drawn as every menu is (`Menus.Groups`), each that cannot run saying why.
 * While a change is on its way the switch and the pin wait.
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
