// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type { ISearchOptions, SearchAddon } from "@xterm/addon-search";
import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import { inForce, searchDecorations } from "./theme/theme";

/** Where a find is: the match it is on (from 0, or -1 for none or past what is marked), and how
 *  many there are. */
export type FoundAt = { index: number; count: number };

/**
 * **What the bar finds through** (#1088): a chat's terminal ({@link fromTheTerminal}) or a
 * file's light editor (`editor/find.ts`). Kept this small so the bar is one bar for both, and
 * so this module never imports CodeMirror (`tools/main-chunk-has-no-codemirror.mjs`).
 */
export type Finder = {
  /** Find `term` forward: as it is typed (`incremental`, which stays on the match it is on
   *  while that still matches), or to the next match. */
  next: (term: string, incremental: boolean) => void;
  previous: (term: string) => void;
  /** Leave nothing marked. */
  clear: () => void;
  /** Hear where the find is after each one; answers how to stop hearing. */
  onResults: (told: (at: FoundAt) => void) => () => void;
};

/** The terminal's search addon as a {@link Finder}: every match highlighted, in the theme in
 *  force when the find runs, so a theme switched while the bar is open colours the next find. */
export function fromTheTerminal(search: SearchAddon): Finder {
  const options = (incremental: boolean): ISearchOptions => ({
    incremental,
    decorations: searchDecorations(inForce()),
  });
  return {
    next: (term, incremental) => void search.findNext(term, options(incremental)),
    previous: (term) => void search.findPrevious(term, options(false)),
    clear: () => search.clearDecorations(),
    onResults: (told) => {
      const heard = search.onDidChangeResults(({ resultIndex, resultCount }) =>
        told({ index: resultIndex, count: resultCount }),
      );
      return () => heard.dispose();
    },
  };
}

/**
 * **The find bar over a chat's terminal** (SI-4): what `⌘F` opens on the focused pane. **And
 * over a file** (#946, #1088): the same bar, finding in the light editor through a
 * {@link Finder}, with what it finds in named by `label`.
 *
 * The finding is `@xterm/addon-search`'s, the terminal's own addon, so what it searches is what
 * the terminal holds — the screen and the scrollback the core keeps. Nothing here reads a
 * harness's output to decide anything; it only shows the operator where a word is.
 *
 * Native HTML and not a Radix primitive, by `docs/ui-primitives.md`'s own rule: a search field
 * and three buttons are elements the browser already has, and Radix has no primitive for a find
 * bar to take. `role="search"` is what names it as one.
 *
 * It is a sibling of the terminal and never inside it — the rule `.pane-frame` holds for
 * everything charter draws over a pane — so a key typed here is the bar's and not the chat's:
 * it is outside the element marked `CHAT_KEYBOARD`, and xterm never sees it.
 */
export function FindBar({
  search,
  finder: given,
  label = "Find in the chat",
  asked,
  onClose,
}: {
  /** A chat's terminal to find in. */
  search?: SearchAddon;
  /** Anything else to find in, when there is no terminal. */
  finder?: Finder;
  /** What the bar is named, for what it finds in. */
  label?: string;
  /** How many times the bar has been asked for: each time, the keyboard goes to its field. */
  asked: number;
  onClose: () => void;
}) {
  const field = useRef<HTMLInputElement>(null);
  const [term, setTerm] = useState("");
  const [results, setResults] = useState<FoundAt | null>(null);
  const finder = useMemo(
    () => given ?? (search === undefined ? undefined : fromTheTerminal(search)),
    [given, search],
  );

  // The finder says where it is after every find; that is the whole of the count.
  useEffect(() => {
    if (finder === undefined) return;
    const stop = finder.onResults(setResults);
    return () => {
      stop();
      // A closed bar leaves nothing drawn over what it found in.
      finder.clear();
    };
  }, [finder]);

  // Opened, or asked for again while open: the keyboard goes to the field, with what is in it
  // selected, so typing replaces it.
  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, [asked]);

  const next = () => {
    if (term !== "") finder?.next(term, false);
  };
  const previous = () => {
    if (term !== "") finder?.previous(term);
  };

  const typed = (value: string) => {
    setTerm(value);
    if (value === "") {
      finder?.clear();
      setResults(null);
      return;
    }
    finder?.next(value, true);
  };

  const key = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      if (event.shiftKey) previous();
      else next();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  };

  return (
    <div className="pane-find" role="search" aria-label={label}>
      <span className="pane-find-box ui-input-lead">
        <Search className="ui-input-lead-icon" aria-hidden="true" />
        <input
          ref={field}
          type="search"
          className="ui-input"
          aria-label="Find"
          placeholder="Find"
          value={term}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => typed(event.target.value)}
          onKeyDown={key}
        />
      </span>
      <span className="pane-find-count" role="status" aria-live="polite">
        {told(results)}
      </span>
      <button type="button" aria-label="Previous match" onClick={previous}>
        <ChevronUp size={14} aria-hidden />
      </button>
      <button type="button" aria-label="Next match" onClick={next}>
        <ChevronDown size={14} aria-hidden />
      </button>
      <button type="button" aria-label="Close find" onClick={onClose}>
        <X size={14} aria-hidden />
      </button>
    </div>
  );
}

/** The count, in words. The addon answers an index of -1 past its highlight limit, where it
 *  stops saying which match is selected; the count is still true then. */
function told(results: FoundAt | null): string {
  if (results === null) return "";
  if (results.count === 0) return "No matches";
  if (results.index < 0) return `${results.count} matches`;
  return `${results.index + 1} of ${results.count}`;
}
