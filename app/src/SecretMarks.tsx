// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { SecretFilled } from "./bindings";
import { KeyRound } from "lucide-react";
import { Badge } from "./ui/badge/Badge";
import { Button } from "./ui/button/Button";
import { PopoverDialog, PopoverTrigger } from "./ui/popover/Popover";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** A site as a mark says it: its host, and the scheme where it isn't https. */
function siteOf(origin: string): string {
  return origin.startsWith("https://") ? origin.slice("https://".length) : origin;
}

/** What a mark is heard as: the key and the site, never a value. */
export function secretMarkName(one: SecretFilled): string {
  return `Secret ${one.key}, filled on ${siteOf(one.origin)}`;
}

/** What a mark's hint says: the key, or the code its seed made, the field the page labels and
 *  the site. */
export function secretMarkSaid(one: SecretFilled): string {
  const field = one.label.trim() === "" ? "a field" : `the field the page labels "${one.label}"`;
  const what = one.code ? `a 2FA code from ${one.key}` : `${one.key} from your vault`;
  const where =
    one.page !== null && one.page !== one.origin
      ? `${siteOf(one.origin)}, in a part of a page from ${siteOf(one.page)}`
      : siteOf(one.origin);
  return `purlis filled ${what} into ${field} on ${where}. The chat never saw the value.`;
}

/** The secrets of `secrets` filled on the site `origin` shows, in the page or a part of it from
 *  another site: what a view marks there. */
export function marksOn(
  secrets: readonly SecretFilled[],
  origin: string | null,
): readonly SecretFilled[] {
  return origin === null
    ? []
    : secrets.filter((one) => one.origin === origin || one.page === origin);
}

/** What the count of several marks says: how many, as digits, with the plural agreeing. */
export function secretsCounted(count: number): string {
  return count === 1 ? "1 secret" : `${count} secrets`;
}

/**
 * **Where purlis filled a vault secret into the page a browser's view shows** (#5119, SF-8):
 * beside the page's site, while the page is on the site it went to. Drawn by purlis in the view's
 * first row, outside the page, as the site is (#2024), so a page can neither fake a mark nor move
 * one off its field. Each names the key and the site, never a value, and its hint says which
 * field the page labels.
 *
 * **One mark, or a count** (#6937): one secret is "secret: <key>", cut to an ellipsis where the
 * key is long. Several are one button, "3 secrets", that opens the list of them, each key whole
 * with what its hint says, so a row of long keys never pushes the site or the row past the pane.
 */
export function SecretMarks({ secrets }: { secrets: readonly SecretFilled[] }) {
  if (secrets.length === 0) return null;
  return (
    <span className="browser-secrets" role="group" aria-label="Secrets purlis filled on this site">
      {secrets.length === 1 ? (
        <OneMark one={secrets[0]} />
      ) : (
        <PopoverTrigger>
          <Button
            size="sm"
            variant="ghost"
            icon={<KeyRound />}
            tooltip="Names only: purlis never shows a value."
            data-testid="secret-marks"
          >
            {secretsCounted(secrets.length)}
          </Button>
          <PopoverDialog aria-label="Secrets purlis filled on this site">
            <ul className="browser-secrets-list">
              {secrets.map((one) => (
                <li key={`${one.key} ${one.origin} ${one.page}`} data-testid="secret-mark">
                  <span className="browser-secrets-key">{one.key}</span>
                  <span className="ui-text-secondary">{secretMarkSaid(one)}</span>
                </li>
              ))}
            </ul>
          </PopoverDialog>
        </PopoverTrigger>
      )}
    </span>
  );
}

/** One secret's mark: its key, its hint saying the field and the site. */
function OneMark({ one }: { one: SecretFilled }) {
  return (
    <Tooltip tip={secretMarkSaid(one)}>
      <Badge
        variant="accent"
        role="img"
        aria-label={secretMarkName(one)}
        data-testid="secret-mark"
        className="browser-secret"
      >
        {/* The words in a box of their own: a badge lays out as a flex box, where an
            ellipsis is drawn only by a box around the words (#6937). */}
        <span className="browser-secret-said">secret: {one.key}</span>
      </Badge>
    </Tooltip>
  );
}
