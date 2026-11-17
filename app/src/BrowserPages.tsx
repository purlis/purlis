// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Layers } from "lucide-react";
import type { PageListed } from "./bindings";
import { siteOf } from "./SecretMarks";
import { Button } from "./ui/button/Button";
import { Menu, MenuItem, MenuSection, MenuTrigger, MenuWords } from "./ui/menu/Menu";

/** What the count of a browser's pages says: which one is shown, of how many. */
export function pagesCounted(pages: readonly PageListed[]): string {
  const at = pages.findIndex((page) => page.current);
  return at < 0 ? `${pages.length} pages` : `Page ${at + 1} of ${pages.length}`;
}

/** What a page's row is named: its title, or its site where it has none, or that it is blank. */
export function pageNamed(page: PageListed): string {
  if (page.title !== null) return page.title;
  if (page.origin !== null) return siteOf(page.origin);
  return "A page with no web address";
}

/** The menu's hint: what picking a page does. */
export const WATCH_SAID =
  "Watch another of this browser's pages. Picking one takes the browser over; when you press Done, the agent goes on with its own page.";

/**
 * **The browser's pages, and a way to watch another** (#6989): "Page 2 of 3" in the view's first
 * row, after the page's title, once the browser has more than one page. Its menu lists each page
 * as `browser_pages` lists it to the chat (its title scrubbed there, and its site alone), the one
 * shown now ticked. Drawn by purlis, outside the page, so a page cannot fake it (#2024).
 *
 * Picking another page is the person's act: `onWatch` takes the browser over first, as Back does
 * (#6938), so it is never done for the agent, and the hand-back gives the agent its own page.
 */
export function BrowserPages({
  pages,
  onWatch,
}: {
  pages: readonly PageListed[];
  onWatch: (page: string) => void;
}) {
  if (pages.length < 2) return null;
  const current = pages.find((page) => page.current)?.id;
  return (
    <MenuTrigger>
      <Button
        size="sm"
        variant="ghost"
        icon={<Layers />}
        tooltip={WATCH_SAID}
        className="browser-pages"
        data-testid="browser-pages"
      >
        {pagesCounted(pages)}
      </Button>
      <Menu aria-label="This browser's pages" align="start">
        <MenuSection
          selectionMode="single"
          selectedKeys={current === undefined ? [] : [current]}
          shouldCloseOnSelect
        >
          {pages.map((page) => (
            <MenuItem
              key={page.id}
              id={page.id}
              textValue={pageNamed(page)}
              data-testid="browser-page"
              onAction={() => {
                // The page shown now, picked again, changes nothing.
                if (page.id !== current) onWatch(page.id);
              }}
            >
              <MenuWords
                said={pageNamed(page)}
                under={
                  page.title !== null && page.origin !== null ? siteOf(page.origin) : undefined
                }
              />
            </MenuItem>
          ))}
        </MenuSection>
      </Menu>
    </MenuTrigger>
  );
}
