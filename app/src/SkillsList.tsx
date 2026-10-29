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
import { Menu, MenuItem, MenuTrigger, MenuWords } from "./ui/menu/Menu";
import { SKILL_STARTS, NEW_SKILL_MENU, SkillStarts, type SkillStart } from "./SkillStarts";
import { BUILT_IN_FIXED, MAKE_MY_OWN_COPY } from "./SkillCopy";
import type { Offer } from "./actions";
import { counted } from "./dispatches";
import type { GroupDrawn } from "./menuModel";
import { Groups } from "./Menus";
import { OPEN_SKILL_STUDIO } from "./skillStudio";
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

/**
 * **The Skills view, drawn** (SK-55, #3266; spec #3679 S7, `prototype/skills-sidebar` variant
 * C): whose skills it shows and how many, its header's way to the Skill studio and its New
 * skill, the filter, and the skills as small cards in three groups that fold: Needs you (in the
 * waiting colour), In this chat (its built-in skills folded to one quiet line at its end, S12)
 * and Left out (folded at first). What it lists and does is the caller's (`SkillsView`); this
 * draws it, so a story draws it without a core.
 *
 * **A card** is a skill's name and its skill level, Use while the chat in front can use it, and
 * its ⋯; two lines of what it does, or why it is as it is in their place; and its one fix. Enter
 * or a click opens it in the Skill studio; its menu (the ⋯, a right-click, Shift+F10 or the menu
 * key) is the caller's.
 */
export function SkillsList({
  whose,
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
  groups: SkillGroups;
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
  // Every skill the chat gets: its built-in skills count, folded or not.
  const count = needs.length + inChat.length + builtIns.length;
  // **While a filter is typed every group is open**, so no match is folded away.
  const isFolded = (id: GroupId) => !filtering && folded.includes(id);
  const card = (row: ListedSkill) => {
    const reason = cardReasonOf(row);
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
  const rows = [...needs, ...inChat, ...builtIns, ...leftOut];
  return (
    <div className="skills-view" data-testid="skills-view">
      <p className="skills-view-whose" data-testid="skills-view-whose">
        <strong>{whose}</strong>{" "}
        <span className="honest">· {counted(count, "skill", "skills")}</span>
      </p>
      <ButtonRow className="skills-view-head">
        <Button
          variant="ghost"
          icon={<Sparkles aria-hidden="true" />}
          onPress={(event) => onOpenStudio(event.metaKey || event.ctrlKey)}
        >
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
      {/* The list's place in the view: it takes the rest, and scrolls. */}
      <div className="skills-view-list">
        <List
          id={listId}
          aria-label="Skills"
          size="sm"
          onAction={(key) => {
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
          {rows.length === 0 ? null : (
            <>
              {needs.length > 0 && (
                <ListFoldSection
                  id="needs-you"
                  title="Needs you"
                  count={needs.length}
                  tone="waiting"
                  folded={isFolded("needs-you")}
                  onFoldedChange={(to) => onFold("needs-you", to)}
                >
                  {needs.map(card)}
                </ListFoldSection>
              )}
              {inChat.length + builtIns.length > 0 && (
                <ListFoldSection
                  id="in-chat"
                  title="In this chat"
                  count={inChat.length + builtIns.length}
                  folded={isFolded("in-chat")}
                  onFoldedChange={(to) => onFold("in-chat", to)}
                >
                  {inChat.map(card)}
                  <ListRun
                    id="built-in"
                    items={builtIns}
                    standsOut={() => filtering}
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
                  {leftOut.map(card)}
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
