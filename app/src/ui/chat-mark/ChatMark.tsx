// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The chat mark family** (#5340, spec #5339, ADR 0104): a chat's mark, one component on its
 * tab, its row in the list, its pane's head and a folded pane's rail, so they cannot drift apart.
 * A screen imports this and never draws its own (`purlis/chat-mark-family` in `lint.js`).
 *
 * - **Drawn by its parts.** The root carries `data-scope="chat-mark"`, `data-part="root"` and the
 *   chat's one state as `data-state`. `who` holds who the chat runs as, `badge` the state's glyph
 *   on its lower end corner with the state as its `data-variant`, and `word` the state's word.
 *   `chat-mark.css` draws the defaults, and a theme's parts reach the same attributes (ADR 0103).
 * - **A shape as well as a colour** (WCAG 1.4.1): {@link MARK_GLYPHS} draws each state its own.
 * - **`who` is its own part so a later control can stand in it** (#5290's grip): what stands there
 *   changes, and the badge beside it does not.
 * - **It says what the place it stands sets**: `--chat-mark-size` and `--chat-mark-ground`, the
 *   ground its badge is ringed in. A state changes a colour and a glyph, never a size.
 * - **A chat that starts needing you knocks twice** (`useArrived`): on that change only, and the
 *   motion comes off when it ends, so a row put back into the document does not play it again.
 */
import {
  Circle,
  Hand,
  Hourglass,
  Minus,
  Pause,
  X,
  type LucideIcon,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { useArrived } from "../../lib/arrived";
import type { MarkState } from "../../shownState";

/** Each state's glyph. **No two share an outline** at the size of a badge. */
export const MARK_GLYPHS: Readonly<Record<MarkState, LucideIcon>> = {
  needs: Hand,
  failed: X,
  working: Circle,
  waiting: Hourglass,
  idle: Pause,
  ended: Minus,
};

export interface ChatMarkProps {
  /** The chat's one state (`shownState`), or nothing where none is known: `who` alone. */
  state: MarkState | null | undefined;
  /** Who the chat runs as: a persona's mark, a shell's, a task's. Nothing for the state alone. */
  who?: ReactNode;
  /** The state's word, heard and not seen. Left out where the place says it itself, after its
   *  name (a tab, `TabMarks`). */
  word?: string;
  /** Its state changed while it was not drawn, so it arrives in it. */
  changed?: boolean;
  /** A place for the mark (margin, flex, order), never a look. */
  className?: string;
}

/** A chat's mark: who it runs as, with its one state on the corner. */
export function ChatMark({ state, who, word, changed = false, className }: ChatMarkProps) {
  // A state this mark CHANGED to, not the one it was drawn in.
  const arrived = useArrived(state) || changed;
  /** The state whose arrival has been played: its motion is over until the next change. */
  const [played, setPlayed] = useState<MarkState | null | undefined>();
  // Another state since: the next time it comes to this one is a change again.
  if (played !== undefined && played !== state) setPlayed(undefined);
  const Glyph = state == null ? undefined : MARK_GLYPHS[state];
  return (
    <span
      data-scope="chat-mark"
      data-part="root"
      data-state={state ?? undefined}
      className={className}
    >
      {who !== undefined && (
        <span data-scope="chat-mark" data-part="who">
          {who}
        </span>
      )}
      {Glyph !== undefined && (
        <span
          data-scope="chat-mark"
          data-part="badge"
          data-variant={state ?? undefined}
          data-mark={state === "needs" ? "needs-you" : undefined}
          data-arrived={arrived && played !== state ? "" : undefined}
          onAnimationEnd={() => setPlayed(state)}
          aria-hidden="true"
        >
          <Glyph />
        </span>
      )}
      {word !== undefined && (
        <span data-scope="chat-mark" data-part="word" className="hidden-words">
          {word}
        </span>
      )}
    </span>
  );
}
