// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { openUrl } from "@tauri-apps/plugin-opener";
import type { SaveMerge } from "./bindings";
import { Notice } from "./Notice";

/**
 * **Where a save's request stands with merging, in the Saving view** (ADR 0051, amended
 * 2026-10-13), under its link.
 *
 * A red request, or one the forge will not let merge, is a Notice: something is wrong, purlis
 * did not merge it, and it stays open. A red one names each check that failed, linked to its
 * page on the forge where it has one. Its way out here is the request's own page; fixing it
 * from the window is a later change. Anything else is a note: nothing is wrong, and purlis
 * looks again by itself.
 *
 * A check's name and link came from whoever wrote the check. The core cuts the name, makes it
 * printable and keeps a link only when it is a plain `https://` address
 * (`purlis_core::forge::checks::FailedCheck`); here both are text and an `href`, never markup.
 */
export function RequestMerge({
  name,
  request,
  pr,
  merge,
}: {
  /** Whose request it is, as a sentence names it: `this project`, or a repo's name. */
  name: string;
  /** `pull request` or `merge request`. */
  request: string;
  /** The request's page. */
  pr: string;
  merge: SaveMerge;
}) {
  const open = {
    label: `Open the ${request}`,
    onPress: () => void openUrl(pr).catch(() => undefined),
  };
  if (merge.state === "red")
    return (
      <Notice cause={`save-request-red:${name}`} tone="trouble" link={open}>
        {`The ${request} of ${name} is red, so purlis did not merge it: ${merge.why}. It stays open, and the next save goes onto it.`}
        {merge.checks.length > 0 && (
          <ul className="saving-files" aria-label="Checks that failed">
            {merge.checks.map((check, n) => (
              <li key={`${check.name}-${n}`}>
                {check.url == null ? (
                  check.name
                ) : (
                  <a href={check.url} target="_blank" rel="noreferrer">
                    {check.name}
                  </a>
                )}
              </li>
            ))}
          </ul>
        )}
      </Notice>
    );
  if (merge.state === "cannot-merge")
    return (
      <Notice cause={`save-request-cannot-merge:${name}`} tone="trouble" link={open}>
        {`purlis cannot merge the ${request} of ${name} as it stands: ${merge.why}. It stays open.`}
      </Notice>
    );
  const said =
    merge.state === "waiting"
      ? `purlis merges the ${request} of ${name} itself once every check on it has passed: ${merge.why}. It looks again by itself.`
      : merge.state === "merged"
        ? `purlis merged the ${request} of ${name}; the project moves onto what merged with the next look.`
        : merge.why;
  return (
    <p className="saving-note" data-merge={merge.state}>
      {said}
    </p>
  );
}
