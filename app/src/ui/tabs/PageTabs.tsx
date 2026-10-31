// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Page tabs** (#3668, spec #2897, the skill page prototype's variant A): the tabs inside a page
 * that switch its panels (a skill's Overview · Text · History · Grades · Runs), a member of the
 * tabs family on React Aria's `Tabs`, `TabList`, `Tab` and `TabPanel`. A screen draws a page's
 * tabs from these and never writes a tablist by hand (`purlis/tabs-family`). The segmented
 * control is a radio group, the wrong pattern for a page's panels.
 *
 * - **The keyboard is React Aria's**, the WAI-ARIA "Tabs" pattern with automatic activation: one
 *   Tab stop on the list, ← and → move to the tab beside and choose it, Home and End to the first
 *   and the last. Each panel is named by its tab.
 * - **A count after a tab's words** describes the tab and is no part of its name (as a chip's
 *   count is, #3484): heard as its sentence after the name. Its room is kept for three figures,
 *   so a count that changes (9 to 10) never widens the tab; past 99 it says 99+.
 * - **The chosen tab** is told by the indicator under it as well as by its words' colour
 *   (WCAG 1.4.1), and choosing a tab changes a colour and paints the indicator only, so no tab
 *   moves (#2800).
 * - **Where the room is narrow** the tabs scroll along their row; they never wrap.
 * - **The chosen tab is the owner's** (`selected`, `onSelect`), so a view's history can bring it
 *   back (#3667).
 * - **Drawn by its parts**: `data-scope="page-tabs"` with `data-part` `list`, `tab` (React Aria's
 *   state attributes on it), `label`, `count`, `indicator` and `panel`. `tabs.css` draws the
 *   defaults and a theme's parts reach the same attributes (`purlis theme parts`).
 */
import { useId, type ReactNode } from "react";
import {
  Tab as AriaTab,
  TabList as AriaTabList,
  TabPanel as AriaTabPanel,
  Tabs as AriaTabs,
  type Key,
} from "react-aria-components";

/** **One tab of a page**: its id, its words, and what its count says, if it has one. */
export interface PageTab {
  id: string;
  label: string;
  /** How many it holds (History 4, Grades 2), after its words. */
  count?: number;
  /** What a screen reader hears for the count ("4 versions"); the number when left out. */
  countSaid?: string;
}

/** The most a count shows before it says `99+`: three figures' room, kept on every tab. */
export const COUNT_MOST = 99;

/** A count as it is drawn. */
export function countShown(count: number): string {
  return count > COUNT_MOST ? `${COUNT_MOST}+` : String(count);
}

/** **One tab**: its words, its count after them, and the indicator under it. */
function PageTabItem({ tab }: { tab: PageTab }) {
  const said = useId();
  const counted = tab.count !== undefined;
  return (
    <AriaTab
      id={tab.id}
      data-scope="page-tabs"
      data-part="tab"
      aria-describedby={counted ? said : undefined}
    >
      <span data-scope="page-tabs" data-part="label">
        {tab.label}
      </span>
      {counted && (
        <>
          <span data-scope="page-tabs" data-part="count" aria-hidden="true">
            {countShown(tab.count ?? 0)}
          </span>
          <span id={said} hidden>
            {tab.countSaid ?? String(tab.count)}
          </span>
        </>
      )}
      <span data-scope="page-tabs" data-part="indicator" aria-hidden="true" />
    </AriaTab>
  );
}

/**
 * **A page's tabs and the chosen tab's panel.** `children` draws the panel of the tab it is
 * handed; only the chosen tab's is drawn.
 */
export function PageTabs({
  label,
  tabs,
  selected,
  onSelect,
  children,
}: {
  /** What the tabs are called (`aria-label`): "deploy's sections". */
  label: string;
  tabs: readonly PageTab[];
  /** The chosen tab's id: the owner's, so a history can bring it back. */
  selected: string;
  onSelect: (id: string) => void;
  /** The panel of the tab `id`. */
  children: (id: string) => ReactNode;
}) {
  return (
    <AriaTabs
      selectedKey={selected}
      onSelectionChange={(key: Key) => onSelect(String(key))}
      keyboardActivation="automatic"
    >
      <AriaTabList aria-label={label} data-scope="page-tabs" data-part="list">
        {tabs.map((tab) => (
          <PageTabItem key={tab.id} tab={tab} />
        ))}
      </AriaTabList>
      {tabs.map((tab) => (
        <AriaTabPanel key={tab.id} id={tab.id} data-scope="page-tabs" data-part="panel">
          {tab.id === selected ? children(tab.id) : null}
        </AriaTabPanel>
      ))}
    </AriaTabs>
  );
}
