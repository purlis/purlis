// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, type ReactNode } from "react";
import { Plus, Sparkles } from "lucide-react";
import { Button, ButtonRow, ToggleButton } from "./ui/button/Button";
import { List, ListRow, ListRowSection, type RowMenu } from "./ui/list/List";
import { ListFilter, noMatchSaid, type Noun } from "./ui/list/ListFilter";
import { RowLabel, RowTrail } from "./ui/list/Row";
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
  type SkillAct,
  type SkillGroup,
} from "./skillsListing";

/** What the view's filter calls its rows. */
const SKILLS_NOUN: Noun = { one: "skill", many: "skills" };

/** The words of the switch that narrows the list to the chat in front (the amendment's (c)). */
const ONLY_THIS_CHATS = "Only this chat's";

/**
 * **The Skills view, drawn** (SK-55, #3266): its header's way to the Skill studio and its New
 * skill, the switch to the chat in front's own set, the filter, and the skills by level, on the
 * list family (#2911). What it lists and does is the caller's (`SkillsView`); this draws it, so a
 * story draws it without a core.
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
}) {
  const listId = useId();
  const rows = groups.flatMap((group) => group.rows);
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
          {/* From the button's end, so at the side's width it opens inside the view (#3308). */}
          <Menu
            align="end"
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
      <List
        id={listId}
        aria-label="Skills"
        size="sm"
        className="skills-view-list"
        onAction={(key) => {
          const row = rows.find((one) => one.key === key);
          if (row !== undefined) onOpen(row);
        }}
        empty={
          query.trim() !== "" ? (
            <p className="honest">{noMatchSaid(SKILLS_NOUN, query)}</p>
          ) : starts ? undefined : (
            <p className="honest">No skill reaches a chat here.</p>
          )
        }
      >
        {groups.map((group) => (
          <ListRowSection key={group.id} id={group.id} title={group.title}>
            {group.rows.map((row) => {
              const mark = markOf(row);
              const tip = tipOf(row);
              return (
                <ListRow
                  key={row.key}
                  id={row.key}
                  textValue={row.skill.name}
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
                >
                  <RowLabel>{row.skill.name}</RowLabel>
                  {mark !== undefined && (
                    <RowTrail className="skills-view-mark">{MARK_SAID[mark]}</RowTrail>
                  )}
                </ListRow>
              );
            })}
          </ListRowSection>
        ))}
      </List>
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
