// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Tooltip } from "../ui/tooltip/Tooltip";

/**
 * **A settings table's column heading** (DS-9, #1841; spec #1832 stories 20-21): one line, in the
 * shared table's look (`.ui-table`, `docs/ui-primitives.md`). A heading longer than its column
 * shows a word or two; its whole words name the column for a screen reader, and they are in the
 * row's tooltip (the shared one, #2908) with what the column holds. The words take the
 * keyboard so the tooltip can be read without a pointer, as a row's badge does. A heading that
 * says it all in its own words, with nothing to add, is a plain heading.
 */
export function ColumnHeading({
  words,
  full,
  about,
  className,
}: {
  /** What the heading shows, on one line. */
  words: string;
  /** Its whole words, where `words` shortens them. */
  full?: string;
  /** What the column holds, in a sentence. */
  about?: string;
  className?: string;
}) {
  const whole = full ?? words;
  const shortened = whole !== words;
  if (!shortened && about === undefined)
    return (
      <th scope="col" className={className}>
        {words}
      </th>
    );
  return (
    <th scope="col" className={className} aria-label={shortened ? whole : undefined}>
      {/* Words, not a control: they press nothing, so they are no button. */}
      <Tooltip
        tip={
          about === undefined ? (
            whole
          ) : (
            <>
              <strong>{whole}</strong> {about}
            </>
          )
        }
      >
        <span className="ui-table-tip">{words}</span>
      </Tooltip>
    </th>
  );
}
