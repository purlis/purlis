// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The keyboard shortcuts** (#2921, spec #2897 N25): the cheat sheet, generated from the key
 * registry (`registry.ts`), so it can never say a key the window does not answer, nor leave one
 * out. Opened from the palette's *Keyboard shortcuts* row and by its key (⌘/, Ctrl+/ off a Mac),
 * in a view tab of its own.
 *
 * - **No title of its own** (the operator's ruling of 2026-10-24, #3147): the tab is its name, so
 *   the view starts with its filter, on the ground its chosen tab joins.
 * - **The list family**: the registry's groups as the list's sections, one row an act, its words
 *   then where it applies and its key at the end (the `shortcut` part). Over seven rows, so a
 *   filter stands over it, which finds an act by its words or its key.
 * - **This keyboard's keys**: a Mac's on a Mac, the Ctrl ones elsewhere.
 */
import { useCallback, useMemo } from "react";
import { onAMac } from "../tabKeys";
import { List, ListRow, ListRowSection } from "../ui/list/List";
import { ListFilter, noMatchSaid, useListFilter } from "../ui/list/ListFilter";
import { RowDescription, RowLabel, RowShortcut } from "../ui/list/Row";
import { SHORTCUTS_TITLE } from "../tabs";
import { switcherKeySaid } from "../switcherKey";
import { GROUPS, SHORTCUTS, type Scope, type Shortcut } from "./registry";

/** Where an act applies, as its row says it: nothing for the whole window. */
const placeSaid = (mac: boolean): Readonly<Record<Scope, string>> => ({
  window: "",
  palette: "In the palette",
  finder: `In ${switcherKeySaid(mac)}'s files`,
  dialog: "In a dialog",
  menu: "In a menu",
  tab: "On a focused tab",
  row: "On a focused row",
  terminal: "In a chat",
  editor: "In an editor",
  form: "In a form",
});

/** One row of the sheet: an act, its key on this keyboard, and where it applies. */
export interface SheetRow {
  id: string;
  does: string;
  keys: string;
  where: string;
  group: Shortcut["group"];
}

/** **The sheet's rows**, from the registry, for this keyboard: every act it has a key for. */
export function sheetRows(mac: boolean, entries: readonly Shortcut[] = SHORTCUTS): SheetRow[] {
  const platform = mac ? "mac" : "other";
  return entries.flatMap((one) => {
    const said = one.said(mac);
    if (said === "" || (one.kind !== "path" && one.chords[platform].length === 0)) return [];
    const also = one.alsoSaid?.(mac) ?? [];
    const off = !mac && one.yields.includes("terminal-off-mac");
    const kept = one.yields.includes("terminal") || off ? "Not in a chat" : "";
    return [
      {
        id: one.id,
        does: one.does,
        keys: [said, ...also].join(" · "),
        where: placeSaid(mac)[one.where] || kept,
        group: one.group,
      },
    ];
  });
}

const SHORTCUT = { one: "shortcut", many: "shortcuts" };

/** **The view tab**: the filter, then the registry's groups. */
export function ShortcutsView({ mac = onAMac() }: { mac?: boolean }) {
  const rows = useMemo(() => sheetRows(mac), [mac]);
  const nameOf = useCallback((row: SheetRow) => `${row.does} ${row.keys} ${row.where}`, []);
  const { query, setQuery, shown } = useListFilter(rows, nameOf);
  return (
    <section className="shortcuts-view" aria-label={SHORTCUTS_TITLE} data-testid="shortcuts-view">
      <ListFilter
        noun={SHORTCUT}
        total={rows.length}
        query={query}
        onQuery={setQuery}
        controls="shortcuts-list"
      />
      {shown.length === 0 ? (
        <p className="none" role="status">
          {noMatchSaid(SHORTCUT, query)}
        </p>
      ) : (
        <List id="shortcuts-list" aria-label={SHORTCUTS_TITLE} className="shortcuts-list">
          {GROUPS.map((group) => {
            const inIt = shown.filter((row) => row.group === group);
            if (inIt.length === 0) return null;
            return (
              <ListRowSection key={group} id={group} title={group}>
                {inIt.map((row) => (
                  <ListRow key={row.id} id={row.id} textValue={row.does}>
                    <RowLabel>{row.does}</RowLabel>
                    {row.where !== "" && <RowDescription>{row.where}</RowDescription>}
                    <RowShortcut keys={row.keys} heard />
                  </ListRow>
                ))}
              </ListRowSection>
            );
          })}
        </List>
      )}
    </section>
  );
}
