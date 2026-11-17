// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Download } from "lucide-react";
import type { DownloadListed } from "./bindings";
import { marked } from "./invisible";
import { Button } from "./ui/button/Button";
import { PopoverDialog, PopoverTrigger } from "./ui/popover/Popover";

/** What the count of a browser's downloads says: how many, as digits, the plural agreeing. */
export function downloadsCounted(count: number): string {
  return count === 1 ? "1 download" : `${count} downloads`;
}

/** A size in bytes, as a person reads it: bytes, KB, MB or GB, one decimal past KB. */
export function sizeSaid(bytes: number): string {
  if (bytes < 1000) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = bytes / 1000;
  let unit = 0;
  while (size >= 1000 && unit < units.length - 1) {
    size /= 1000;
    unit += 1;
  }
  return `${size.toFixed(1)} ${units[unit]}`;
}

/** **How far a download came, in words** (#6990): going (with how much of how much, where the
 *  site said), done, cancelled, or cancelled by purlis for its size. */
export function downloadSaid(one: DownloadListed): string {
  // A size the core could not say as a number is none so far.
  const received = one.received ?? 0;
  switch (one.state) {
    case "going":
      return one.total === null
        ? `Downloading, ${sizeSaid(received)} so far`
        : `Downloading, ${sizeSaid(received)} of ${sizeSaid(one.total)}`;
    case "done":
      return `Downloaded, ${sizeSaid(received)}`;
    case "cancelled":
      return "Cancelled";
    case "too_large":
      return "Cancelled by purlis: larger than a browser may download";
  }
}

/** What the list says of saving one: that is the chat's to do. */
export const SAVED_BY_THE_CHAT = "Only the chat saves a download into the project. Ask it to.";

/**
 * **The browser's downloads** (#6990): a count in the view's first row, once its page has saved a
 * file, that opens the list of them: each by the name purlis shows it by (every character a
 * reader would not see marked, #2020), with how far it came. Drawn by purlis, outside the page,
 * so a page cannot fake it (#2024). Saving one stays the chat's (`browser_save_download`).
 */
export function BrowserDownloads({ downloads }: { downloads: readonly DownloadListed[] }) {
  if (downloads.length === 0) return null;
  const going = downloads.some((one) => one.state === "going");
  return (
    <PopoverTrigger>
      <Button
        size="sm"
        variant="ghost"
        icon={<Download />}
        tooltip={going ? "A download is under way." : "What this browser's pages downloaded."}
        className="browser-downloads"
        data-testid="browser-downloads"
      >
        {downloadsCounted(downloads.length)}
      </Button>
      <PopoverDialog aria-label="This browser's downloads">
        <ul className="browser-downloads-list">
          {downloads.map((one) => (
            <li key={one.id} data-testid="browser-download">
              <span>{marked(one.name, "marked")}</span>
              <span className="ui-text-secondary">{downloadSaid(one)}</span>
            </li>
          ))}
        </ul>
        <p className="ui-text-secondary">{SAVED_BY_THE_CHAT}</p>
      </PopoverDialog>
    </PopoverTrigger>
  );
}
