// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, type ReactNode } from "react";
import { Plus, Sparkles } from "lucide-react";
import { Button, ButtonRow, ToggleButton } from "./ui/button/Button";
import { List, ListRow, ListRowSection, type RowMenu } from "./ui/list/List";
import { ListFilter, noMatchSaid, type Noun } from "./ui/list/ListFilter";
import { RowLabel, RowTrail } from "./ui/list/Row";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "./ui/menu/Menu";
import { SKILL_STARTS, NEW_SKILL, SkillStarts, type SkillStart } from "./SkillStarts";
import { MARK_SAID, markOf, tipOf, type ListedSkill, type SkillGroup } from "./skillsListing";

/** What the view's filter calls its rows. */
export const SKILLS_NOUN: Noun = { one: "skill", many: "skills" };

/** The words of the view's way to the Skill studio, as the palette's row says it. */
export const OPEN_SKILL_STUDIO = "Open Skill studio";

/** The words of the switch that narrows the list to the chat in front (the amendment's (c)). */
export const ONLY_THIS_CHATS = "Only this chat's";

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
            {NEW_SKILL}
          </Button>
          <Menu
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
      {starts && <SkillStarts busy={busy} onStart={onStart} testid="skills-view-empty" />}
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
          <ListRowSection key={group.id} id={group.id} title={group.title} named={group.named}>
            {group.rows.map((row) => {
              const mark = markOf(row.skill);
              const tip = tipOf(row);
              return (
                <ListRow
                  key={row.key}
                  id={row.key}
                  textValue={row.skill.name}
                  tip={
                    tip.length === 0
                      ? undefined
                      : tip.map((line) => (
                          <span key={line} className="skills-view-tip">
                            {line}
                          </span>
                        ))
                  }
                  menu={menuOf(row)}
                  data-testid={`skills-row-${row.skill.name}`}
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
