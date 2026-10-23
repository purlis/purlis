// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, type ReactElement, type ReactNode } from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import {
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
  UsersRound,
  type LucideIcon,
} from "lucide-react";
import { RailIcon } from "./FileIcon";
import { useTabStop } from "./roving";
import { useRailIcons } from "./theme/railIcons";
import type { OwnViewId, PanelViewId, Side, ViewId } from "./regions";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** A view as its tab draws it: its id, what it is called and its mark. */
export type Tabbed = { view: ViewId; name: string; mark: LucideIcon };

/** An extension's panel as its tab draws it (#1678): the name and mark its panel declared. */
export type PanelTab = Tabbed & { view: PanelViewId };

/**
 * **A side's activity bar: a strip of icons, one per view, that switches the side between them**
 * (ADR 0038 as amended 2026-10-10, B-1, #1673).
 *
 * VS Code's, in the window's own primitives. **It is a tab list, vertical**: the tabs are the
 * views, the selected tab is the open view, and the arrows, Home and End move along it with one
 * Tab stop (`roving.ts`, the strips' own `RovingFocusGroup`). Enter or Space presses the tab the
 * keyboard is on, and so does a click: activation is manual, as an editor's is, so walking the
 * bar with the arrows opens nothing. **The tab list owns tabs and nothing else** (#1204): each
 * tab's badge is inside its own tab.
 *
 * **What a press does is the arrangement's** (`regions.picked`): a press of the open view puts
 * the side away, any other opens. This draws and says; it decides nothing.
 *
 * **Icons, and the words are still there** (`docs/design-system.md` §Icons): each tab is its
 * view's mark, the view's name is its `aria-label`, and the tooltip says what pressing does,
 * with the key. The mark is what the view IS, never which edge it is on, so it is right on the
 * other side too. **A theme may redraw a view's mark** with its icon set (#2337,
 * `theme/railIcons.ts`); a view it does not draw keeps its own.
 *
 * **Generic over the side.** It knows nothing of the left: the right side's bar (#1678) is this
 * component with the attention region's views, drawn by `RegionFrame` from the same data, and an
 * extension's panel is a tab like any other, with the name and mark it declared.
 */
export function ActivityBar({
  side,
  region,
  name,
  away,
  views,
  open,
  keys,
  badges,
  panelOf,
  tabOf,
  onPick,
  menu,
}: {
  /** Which edge of the window it is drawn on. */
  side: Side;
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
  /** How each view's key is spelled on this platform, for the tooltip. */
  keys?: Partial<Record<ViewId, string>>;
  /** A count or a mark beside a view's icon, drawn whether or not the side is out. */
  badges?: Partial<Record<ViewId, ReactNode>>;
  /** The id of each view's tab panel, which its tab controls. */
  panelOf: (view: ViewId) => string;
  /** The id of each view's tab, which names its panel. */
  tabOf: (view: ViewId) => string;
  onPick: (view: ViewId) => void;
  /** **A view's tab with its context menu around it** (#1740): the window's own, the one
   *  catalogue's (`Menus.tsx`), as every strip's tabs have theirs. Absent, no menu. */
  menu?: (view: ViewId, tab: ReactElement) => ReactNode;
}) {
  const stop = useTabStop(
    open,
    views.map((one) => one.view),
  );
  const iconOf = useRailIcons();
  return (
    <div className="activity-bar" data-side={side} data-region={region}>
      <RovingFocusGroup.Root asChild orientation="vertical" loop={false} {...stop}>
        <div role="tablist" aria-orientation="vertical" aria-label={name}>
          {views.map(({ view, name: called, mark: Mark }) => {
            const selected = view === open;
            const icon = iconOf(view);
            const key = keys?.[view];
            const badge = badges?.[view];
            const described = `${tabOf(view)}-badge`;
            const tab = (
              <RovingFocusGroup.Item asChild tabStopId={view} active={selected}>
                <Tooltip
                  tip={
                    (selected
                      ? (away ?? `Put the ${name} region away`)
                      : `Show the ${called} view`) + (key === undefined ? "" : ` (${key})`)
                  }
                >
                  <button
                    type="button"
                    role="tab"
                    id={tabOf(view)}
                    className="activity-tab"
                    aria-selected={selected}
                    aria-controls={panelOf(view)}
                    aria-label={called}
                    aria-describedby={badge === undefined ? undefined : described}
                    onClick={() => onPick(view)}
                  >
                    {icon ? (
                      <RailIcon view={view} paths={icon.paths} />
                    ) : (
                      <Mark aria-hidden="true" />
                    )}
                    {badge !== undefined && (
                      <span id={described} className="activity-badge">
                        {badge}
                      </span>
                    )}
                  </button>
                </Tooltip>
              </RovingFocusGroup.Item>
            );
            return <Fragment key={view}>{menu === undefined ? tab : menu(view, tab)}</Fragment>;
          })}
        </div>
      </RovingFocusGroup.Root>
    </div>
  );
}

/**
 * **A count on a view's tab** (B-8): the number drawn, and the sentence a screen reader is told
 * as the tab's description, since the tab's name is the view's. The needs-you tone is the one
 * the project and workspace tabs' counts are drawn in, and it is only ever the count's, never a
 * word's (`docs/design-system.md` §Icons). Nothing is drawn for nothing: a zero is not a badge.
 */
export function ActivityCount({
  count,
  said,
  tone,
  drawn,
}: {
  count: number;
  said: string;
  tone?: "needs-you";
  /** What is drawn in place of the bare number, where one number does not say it: Repos'
   *  commits behind and ahead (#1739). Still nothing at all for a zero `count`. */
  drawn?: string;
}) {
  if (count <= 0) return null;
  return (
    <span className="activity-count" data-tone={tone}>
      <span aria-hidden="true">{drawn ?? count}</span>
      <span className="activity-said">{said}</span>
    </span>
  );
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
  changes: GitBranch,
  // #1737: a repo is a folder git keeps, as the cockpit's repo mark has it.
  repos: FolderGit2,
  // The title bar's hand (#1692): the ✋ counts the asks and opens this view.
  inbox: Hand,
  todos: ListTodo,
  memory: Brain,
  personas: UsersRound,
  sessions: History,
  vaults: KeyRound,
};
