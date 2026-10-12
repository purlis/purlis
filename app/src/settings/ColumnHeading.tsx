// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import * as Tooltip from "@radix-ui/react-tooltip";

/**
 * **A settings table's column heading** (DS-9, #1841; spec #1832 stories 20-21): one line, in the
 * shared table's look (`.ui-table`, `docs/ui-primitives.md`). A heading longer than its column
 * shows a word or two; its whole words name the column for a screen reader, and they are in the
 * row's tooltip (the Radix one DS-8 gave Settings) with what the column holds. The words take the
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
      <Tooltip.Provider delayDuration={300}>
        <Tooltip.Root>
          <Tooltip.Trigger asChild>
            {/* Words, not a control: they press nothing, so they are no button. */}
            <span className="ui-table-tip" tabIndex={0}>
              {words}
            </span>
          </Tooltip.Trigger>
          <Tooltip.Portal>
            <Tooltip.Content
              className="ui-tooltip ui-setting-tip"
              side="top"
              align="start"
              sideOffset={4}
            >
              {about === undefined ? (
                whole
              ) : (
                <>
                  <strong>{whole}</strong> {about}
                </>
              )}
              <Tooltip.Arrow className="ui-overlay-arrow" />
            </Tooltip.Content>
          </Tooltip.Portal>
        </Tooltip.Root>
      </Tooltip.Provider>
    </th>
  );
}
