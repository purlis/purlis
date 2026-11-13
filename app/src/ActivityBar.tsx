// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, type ReactElement, type ReactNode } from "react";
import {
  Bell,
  Brain,
  FolderGit2,
  FolderTree,
  GitBranch,
  Hand,
  History,
  KeyRound,
  ListTodo,
  MessagesSquare,
  Search,
  Sparkles,
  TriangleAlert,
  UsersRound,
  type LucideIcon,
} from "lucide-react";
import { RailIcon } from "./FileIcon";
import {
  ActivityCount,
  ActivityDot,
  ActivityNew,
  ActivityGroup,
  ActivityItem,
  type ActivityVariant,
} from "./ui/activity/ActivityItem";
import { AsStripTab, useTabStrip, useTabStripRow } from "./ui/tabs/TabStrip";
import { useRailIcons } from "./theme/railIcons";
import type { OwnViewId, PanelViewId, Side, ViewId } from "./regions";

/** A view as its tab draws it: its id, what it is called and its mark. */
export type Tabbed = { view: ViewId; name: string; mark: LucideIcon };

/** An extension's panel as its tab draws it (#1678): the name and mark its panel declared. */
export type PanelTab = Tabbed & { view: PanelViewId };

/**
 * **A side's activity bar: a strip of icons, one per view, that switches the side between them**
 * (ADR 0038 as amended 2026-10-10, B-1, #1673).
 *
 * Each item is the activity family's (`ui/activity`, #2917). **It is a tab list**: a column of
 * icons on each side (#3144), the left one beside the project rail and the right one at the
 * window's edge, the tabs the views and the selected tab the open view. Its keyboard is the tabs
 * family's (`ui/tabs/TabStrip`, React Aria's tabs, #3134): the arrows along it, Home and End
 * move, wrapping at its ends, with one Tab stop. Enter or Space presses the tab the keyboard is
 * on, and so does a click: activation is manual, so walking the bar opens nothing. **The tab
 * list owns tabs and nothing else** (#1204): each tab's count is inside its own tab, so the
 * group is itself the tablist.
 *
 * **What a press does is the arrangement's** (`regions.picked`): a press of the open view puts
 * its region away, on every bar (R121 as amended 2026-10-26, #3329); any other opens.
 * This draws and says; it decides nothing.
 *
 * **Icons, and the words are still there** (`docs/design-system.md` §Icons): each tab is its
 * view's mark, the view's name is its `aria-label`, its count follows in its accessible name,
 * and the tooltip says what pressing does, with the key, or for an attention view the question it
 * answers (`VIEW_QUESTIONS`). The mark is what the view IS, never which edge it is on, so it is right on the
 * other side too. **A theme may redraw a view's mark** with its icon set (#2337,
 * `theme/railIcons.ts`); a view it does not draw keeps its own.
 *
 * **Generic over the side.** It knows nothing of the left: the right side's bar (#1678) is this
 * component with the attention region's views, drawn by `RegionFrame` from the same data, and an
 * extension's panel is a tab like any other, with the name and mark it declared.
 */
export function ActivityBar({
  side,
  variant,
  region,
  name,
  away,
  views,
  open,
  current,
  keys,
  badges,
  bounce,
  onBounced,
  panelOf,
  tabOf,
  onPick,
  menu,
}: {
  /** Which edge of the window it is drawn on. */
  side: Side;
  /** How its items are drawn (`ui/activity`): icons in a side's column (#3144), or tabs across a
   *  side panel's top. A column is walked by the up and down arrows, a row of tabs by left and
   *  right. */
  variant: ActivityVariant;
  /** The region whose bar it is, said on the bar for the stylesheet and the scenarios. */
  region?: string;
  /** The region's name, which is the tab list's. */
  name: string;
  /** What the open view's tooltip says a press does, where "Put the <name> region away" does
   *  not read: a half of the Panel's (#1737). */
  away?: string;
  /** The views, in the bar's order, each with its name and mark. */
  views: readonly Tabbed[];
  /** The open view, or nothing while the side is put away. */
  open: ViewId | undefined;
  /** **The view its region shows when it is out**, open or not (#3329): its tab says whether
   *  the region is out (`aria-expanded`), so a press that puts it away is heard as collapsed. */
  current?: ViewId;
  /** How each view's key is spelled on this platform, for the tooltip. */
  keys?: Partial<Record<ViewId, string>>;
  /** A count or a mark beside a view's icon, drawn whether or not the side is out. */
  badges?: Partial<Record<ViewId, ReactNode>>;
  /**
   * **The view whose icon bounces** (#6343, ADR 0121 amended 2026-11-12): the ✋, the one
   * movement in the window that calls the person. Its item is marked `data-bounce` and the
   * stylesheet plays {@link HAND_BOUNCE} on its mark, 3 times a duration token's length, so
   * reduced motion plays none. `onBounced` is told as it ends.
   */
  bounce?: ViewId;
  onBounced?: () => void;
  /** The id of each view's tab panel, which its tab controls. */
  panelOf: (view: ViewId) => string;
  /** The id of each view's tab, which names its panel. */
  tabOf: (view: ViewId) => string;
  onPick: (view: ViewId) => void;
  /** **A view's tab with its context menu around it** (#1740): the window's own, the one
   *  catalogue's (`Menus.tsx`), as every strip's tabs have theirs. Absent, no menu. */
  menu?: (view: ViewId, tab: ReactElement) => ReactNode;
}) {
  const strip = useTabStrip({ keys: views.map((one) => one.view), selected: open });
  const orientation = variant === "tab" ? "horizontal" : "vertical";
  const { row, tablist } = useTabStripRow({ strip, name, orientation });
  const iconOf = useRailIcons();
  return (
    <ActivityGroup
      {...row}
      {...tablist}
      className="activity-bar"
      data-side={side}
      data-region={region}
    >
      {views.map(({ view, name: called, mark: Mark }) => {
        const selected = view === open;
        const icon = iconOf(view);
        const key = keys?.[view];
        const badge = badges?.[view];
        const described = `${tabOf(view)}-badge`;
        // A view that answers a question says it, open or not (ADR 0121 decisions 1 and 24).
        const said =
          VIEW_QUESTIONS[view as OwnViewId] ??
          (selected ? (away ?? `Put the ${name} region away`) : `Show the ${called} view`);
        const tab = (
          <AsStripTab
            strip={strip}
            tabKey={view}
            selected={selected}
            // Its press here, with whatever a menu around it joins (`AsStripTab`).
            onClick={() => onPick(view)}
          >
            {(drawn) => (
              <ActivityItem
                {...drawn}
                variant={variant}
                selected={selected}
                tip={said}
                shortcut={key}
                id={tabOf(view)}
                aria-expanded={view === current ? open !== undefined : undefined}
                aria-controls={panelOf(view)}
                data-bounce={view === bounce || undefined}
                onAnimationEnd={
                  view === bounce
                    ? (event) => {
                        if (event.animationName === HAND_BOUNCE) onBounced?.();
                      }
                    : undefined
                }
                aria-label={called}
                // **Its count is in its name** (ADR 0121 decision 25): the view's name, then the
                // badge's sentence. The tab's own `aria-label` stays the view's alone, so the
                // panel it names and a scenario that finds it read "Asks" still (a name taken
                // through `aria-labelledby` never follows a second one).
                aria-labelledby={badge === undefined ? undefined : `${tabOf(view)} ${described}`}
                icon={icon ? <RailIcon view={view} paths={icon.paths} /> : <Mark />}
                count={
                  badge === undefined ? undefined : (
                    // Hidden as drawn, and read in the item's name through `aria-labelledby`.
                    <span id={described} className="activity-badge" aria-hidden="true">
                      {badge}
                    </span>
                  )
                }
              />
            )}
          </AsStripTab>
        );
        return <Fragment key={view}>{menu === undefined ? tab : menu(view, tab)}</Fragment>;
      })}
    </ActivityGroup>
  );
}

/** The animation the ✋'s bounce plays (`App.css`), told apart from any other on the item. */
export const HAND_BOUNCE = "purlis-hand-bounce";

/**
 * **The question each attention view answers** (ADR 0121 decision 1), its icon's tooltip in
 * place of what a press does: there is no tour, and the tooltips carry the questions (decision
 * 24). The wording is the ADR's.
 */
export const VIEW_QUESTIONS: Partial<Record<OwnViewId, string>> = {
  asks: "Do I need to answer something?",
  updates: "Did my work finish?",
  notices: "Is something about my setup wrong or worth knowing?",
};

/** A count on a view's tab (B-8, R119, R120): the activity family's own (`ui/activity`). */
export { ActivityCount, ActivityDot };

/**
 * **A "new" pip beside each view's badge** (#6002, ADR 0121 decision 10): `fresh` is how many
 * rows of each view are New. A view with none keeps its badge as it is; one with some has the
 * pip after it, and "N new" said after the badge's own sentence.
 */
export function withNew(
  badges: Partial<Record<ViewId, ReactNode>>,
  fresh: Partial<Record<ViewId, number>>,
): Partial<Record<ViewId, ReactNode>> {
  const out = { ...badges };
  for (const [view, count] of Object.entries(fresh) as [ViewId, number][]) {
    if (count <= 0) continue;
    const badge = badges[view];
    const drawn = badge !== undefined && badge !== null && badge !== false;
    out[view] = (
      <>
        {badge}
        <ActivityNew said={`${drawn ? ", " : ""}${count} new`} />
      </>
    );
  }
  return out;
}

/**
 * Each of purlis's own views' mark, by what it IS. A `Record` over `OwnViewId`, so a view added
 * without a mark is a type error rather than a tab with a hole in it. An extension's panel brings
 * its own, from the vocabulary its panel declares (`Panels.MARKS`).
 *
 * The right side's are the marks their tabs already have where one exists (`Views.OWN_MARKS`:
 * Memory's brain, a session record's history, a vault's key); Todos is the list, where one todo
 * is the dashed circle, and Personas is people, where one persona is a person.
 */
export const VIEW_MARKS: Record<OwnViewId, LucideIcon> = {
  chats: MessagesSquare,
  explorer: FolderTree,
  // #1676: a magnifier for finding in files, and a branch for what git says of the repos.
  search: Search,
  // #3266: the Skill studio's own mark (`SkillStudioTab`'s empty states).
  skills: Sparkles,
  changes: GitBranch,
  // #1737: a repo is a folder git keeps, as the cockpit's repo mark has it.
  repos: FolderGit2,
  // The title bar's hand (#1692): the ✋ counts the asks and opens this view.
  asks: Hand,
  // ADR 0119: the bell, for what happened rather than what asks.
  updates: Bell,
  // ADR 0119 (#5645): a warning sign, for what stands wrong until it is fixed.
  notices: TriangleAlert,
  todos: ListTodo,
  memory: Brain,
  personas: UsersRound,
  sessions: History,
  vaults: KeyRound,
};
