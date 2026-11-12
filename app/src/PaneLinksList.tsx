// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import type { KindHint } from "./bindings";
import { onAMac } from "./mac";
import type { ScreenLink } from "./paneLinks";
import { sentence } from "./PaneLinkSurfaces";
import { Dialog } from "./ui/dialog/Dialog";
import { ListBox, ListOption } from "./ui/list/List";
import { ListFilter, noMatchSaid } from "./ui/list/ListFilter";
import { RowBelow, RowDescription, RowLabel } from "./ui/list/Row";

/**
 * **Links in this pane…** (#6220, spec #6208 D5; the prototype's S5): the links on the pane's
 * screen, for the keyboard and a screen reader, a quick picker as *Use skill…* is.
 *
 * - **Newest first**: the lowest on screen leads. The viewport's links, not the history's.
 * - **Each entry says its target and its kind**: the target is its name, so it is heard first,
 *   and its kind is its description. One that asks first says so, and why.
 * - **Enter opens it by the same rules as ⌘-click** (`pane_link_open`, asks included). Text the
 *   core refuses is listed with its reason and cannot be picked: it keeps the keyboard, so the
 *   reason is heard (`unavailable`).
 * - **⌘C (Ctrl+C off a Mac) copies the entry's target**, as the menu's *Copy link* does.
 */

/** What each kind of link is called in the list. A new kind says its word here. */
export const KIND_SAID: Record<KindHint, string> = { url: "Web address" };

/** What the list's items are called. */
const LINKS = { one: "link", many: "links" };

/** What the list says with no links on screen. */
export const NO_LINKS = "No links on screen.";

/** An entry's key in the list: its kind and its text, which the hook lists once each. */
const keyOf = (link: ScreenLink) => `${link.candidate.kind}:${link.candidate.text}`;

/** Whether `link` can be picked: the core opens it, or asks first. */
const pickable = (link: ScreenLink) => link.seen.does.does !== "refused";

/** The entries `query` leaves: a case-insensitive substring of the text or the target. */
function matching(links: readonly ScreenLink[], query: string): ScreenLink[] {
  const typed = query.trim().toLowerCase();
  if (typed === "") return [...links];
  return links.filter((link) =>
    `${link.candidate.text} ${link.seen.target}`.toLowerCase().includes(typed),
  );
}

/** An entry's line under its kind: why it asks first, or why it will not open. */
function whySaid(link: ScreenLink): string | undefined {
  const does = link.seen.does;
  if (does.does === "opens") return undefined;
  return `${sentence(does.reason)}.`;
}

export function PaneLinksList({
  links,
  onPick,
  onCopy,
  onClose,
}: {
  /** The links on screen, newest first, or `undefined` while the screen is read. */
  links: readonly ScreenLink[] | undefined;
  /** Opens `link` through the open command. */
  onPick: (link: ScreenLink) => void;
  /** Copies `link`'s target. */
  onCopy: (link: ScreenLink) => void;
  onClose: () => void;
}) {
  const mac = onAMac();
  const [query, setQuery] = useState("");
  const listId = useId();
  const content = useRef<HTMLDivElement>(null);
  const all = links ?? [];
  const shown = matching(all, query);

  // The filter takes the keys, as a quick picker's does, once the screen is read.
  const read = links !== undefined;
  useEffect(() => {
    content.current?.querySelector<HTMLElement>("input")?.focus();
  }, [read]);

  const byKey = (key: unknown) => all.find((link) => keyOf(link) === key);
  const pick = (link: ScreenLink | undefined) => {
    if (link !== undefined && pickable(link)) onPick(link);
  };

  /** **The filter's keys, as a palette's**: Enter takes the first match that opens, and the
   *  down arrow goes into the list. */
  const filterKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      pick(shown.find(pickable));
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      document.getElementById(listId)?.querySelector<HTMLElement>('[role="option"]')?.focus();
    }
  };

  /** ⌘C (Ctrl+C off a Mac) on an entry copies its target. */
  const copyKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const held = mac ? event.metaKey : event.ctrlKey;
    if (!held || event.key.toLowerCase() !== "c") return;
    const key = (event.target as Element).closest<HTMLElement>("[data-key]")?.dataset.key;
    const link = byKey(key);
    if (link === undefined) return;
    event.preventDefault();
    onCopy(link);
  };

  return (
    <Dialog
      onClose={onClose}
      title="Links in this pane"
      subtitle="On screen, newest first"
      summary={<>⏎ opens · {mac ? "⌘C" : "Ctrl+C"} copies its target</>}
      className="pane-links-list"
      data-testid="pane-links-list"
    >
      <div ref={content}>
        {all.length > 0 && (
          <ListFilter
            noun={LINKS}
            total={all.length}
            query={query}
            onQuery={setQuery}
            onKeyDown={filterKey}
            controls={listId}
          />
        )}
        <div id={listId} onKeyDown={copyKey}>
          {links === undefined ? (
            <p className="honest">Reading the links on screen…</p>
          ) : all.length === 0 ? (
            <p className="honest">{NO_LINKS}</p>
          ) : shown.length === 0 ? (
            <p className="honest">{noMatchSaid(LINKS, query)}</p>
          ) : (
            <ListBox aria-label="Links on screen" size="lg" onAction={(key) => pick(byKey(key))}>
              {shown.map((link) => {
                const why = whySaid(link);
                return (
                  <ListOption
                    key={keyOf(link)}
                    id={keyOf(link)}
                    textValue={link.seen.target}
                    unavailable={!pickable(link)}
                    data-key={keyOf(link)}
                  >
                    <RowLabel className="pane-link-target">{link.seen.target}</RowLabel>
                    <RowDescription>
                      {KIND_SAID[link.seen.kind]}
                      {link.seen.does.does === "asks" ? " · asks first" : ""}
                      {link.seen.does.does === "refused" ? " · won't open" : ""}
                    </RowDescription>
                    {why !== undefined && <RowBelow>{why}</RowBelow>}
                  </ListOption>
                );
              })}
            </ListBox>
          )}
        </div>
      </div>
    </Dialog>
  );
}
