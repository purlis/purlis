// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { SecretFilled } from "./bindings";
import { Badge } from "./ui/badge/Badge";
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

/**
 * **Where purlis filled a vault secret into the page a browser's view shows** (#5119, SF-8):
 * "secret: <key>" beside the page's site, one mark a key, while the page is on the site it went
 * to. Drawn by purlis in the view's bar, outside the page, as the site's chip is (#2024), so a
 * page can neither fake a mark nor move one off its field. Each names the key and the site,
 * never a value, and its hint says which field the page labels.
 */
export function SecretMarks({ secrets }: { secrets: readonly SecretFilled[] }) {
  if (secrets.length === 0) return null;
  return (
    <span className="browser-secrets" role="group" aria-label="Secrets purlis filled on this site">
      {secrets.map((one) => (
        <Tooltip key={`${one.key} ${one.origin} ${one.page}`} tip={secretMarkSaid(one)}>
          <Badge
            variant="accent"
            role="img"
            aria-label={secretMarkName(one)}
            data-testid="secret-mark"
          >
            secret: {one.key}
          </Badge>
        </Tooltip>
      ))}
    </span>
  );
}
