// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from "react";
import { ExternalLink, FileText, MessageSquare } from "lucide-react";
import type { KindHint } from "./bindings";
import { sentence } from "./coreSentence";
import { onAMac } from "./mac";
import { linkKey, type ScreenLink } from "./paneLinks";
import { copiesALink, linkCopyKeySaid } from "./paneLinksKey";
import { Dialog } from "./ui/dialog/Dialog";
import { ListBox, ListOption } from "./ui/list/List";
import { ListFilter, noMatchSaid, quickPickKeys, useListFilter } from "./ui/list/ListFilter";
import { RowBelow, RowDescription, RowLabel, RowLead } from "./ui/list/Row";

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
export const KIND_SAID: Record<KindHint, string> = {
  url: "Web address",
  file: "File",
  chat: "Chat",
};

/** Each kind's icon, at the row's lead (the prototype's S5). Its word is the row's description,
 *  so the icon is not heard. */
const KIND_ICON: Record<KindHint, ReactNode> = {
  url: <ExternalLink aria-hidden="true" />,
  file: <FileText aria-hidden="true" />,
  chat: <MessageSquare aria-hidden="true" />,
};

/** What the list's items are called. */
const LINKS = { one: "link", many: "links" };

/** What the list says with no links on screen. */
export const NO_LINKS = "No links on screen.";

/** An entry's key in the list: the hook's, which lists each once. */
const keyOf = (link: ScreenLink) => linkKey(link.candidate);

/** Whether `link` can be picked: the core opens it, or asks first. */
const pickable = (link: ScreenLink) => link.seen.does.does !== "refused";

/** What the filter matches an entry by: its text and its target. */
const nameOf = (link: ScreenLink) => `${link.candidate.text} ${link.seen.target}`;

/** No links yet: the same empty list each time, so the filter keeps its answer. */
const NONE: readonly ScreenLink[] = [];

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
  const listId = useId();
  const content = useRef<HTMLDivElement>(null);
  const all = links ?? NONE;
  // A quick picker's filter: typing is how it is used, however short the list.
  const { query, setQuery, shown } = useListFilter(all, nameOf, { always: true });

  // The filter takes the keys, as a quick picker's does, once the screen is read.
  const read = links !== undefined;
  useEffect(() => {
    content.current?.querySelector<HTMLElement>("input")?.focus();
  }, [read]);

  const byKey = (key: unknown) => all.find((link) => keyOf(link) === key);
  const pick = (link: ScreenLink | undefined) => {
    if (link !== undefined && pickable(link)) onPick(link);
  };

  /** **The filter's keys, as a quick picker's** (`ui/list`): Enter takes the first match that
   *  opens. */
  const filterKey = quickPickKeys(listId, () => shown.find(pickable), pick);

  /** ⌘C (Ctrl+C off a Mac) on an entry copies its target (`links.copy`). */
  const copyKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!copiesALink(event, mac)) return;
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
      // What the keys do, only where there is something to do it to.
      description={
        all.length > 0
          ? `Enter opens a link. ${linkCopyKeySaid(mac)} copies its target.`
          : undefined
      }
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
                    <RowLead>{KIND_ICON[link.seen.kind]}</RowLead>
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
