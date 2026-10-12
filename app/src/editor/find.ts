// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Find in a file** (#946, #1088): the light editor's half of the find bar, with no package
 * beyond the CodeMirror the editor already has. `FindBar.tsx` draws the bar and holds the
 * keys; this finds, marks and moves, behind the bar's small {@link Finder} interface, as the
 * terminal's search addon does for a chat.
 *
 * A match is the text typed, in any case, as the terminal's find matches. Every match is marked
 * (`.cm-find-match`, the terminal's `terminal.find-match` colour, `look.ts`) and the one the
 * bar is on is marked apart (`.cm-find-match-active`), selected and scrolled into view. In the
 * merge view both sides are searched, the side before first, so the count is the whole diff's.
 * A match inside an unchanged stretch the merge view folded away is unfolded first, on both
 * sides, as a click on the fold does (#1797); a plain file has no folds, so nothing changes.
 */
import { getChunks, mergeViewSiblings, uncollapseUnchanged } from "@codemirror/merge";
import { RangeSetBuilder, StateEffect, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";
import type { Finder, FoundAt } from "../FindBar";

/** How many matches are marked at most, as the terminal's addon stops at its limit too. Past
 *  it the count is still true; only the marks stop. */
const MARKED_AT_MOST = 1000;

/** One match: which view, and where in its document. */
type Match = { view: EditorView; from: number; to: number };

/** The marks a view draws: every match in it, and which of them is the one the bar is on. */
const marked = StateEffect.define<{ all: readonly { from: number; to: number }[]; on?: number }>();

const MATCH = Decoration.mark({ class: "cm-find-match" });
const ACTIVE = Decoration.mark({ class: "cm-find-match-active" });

/** The field that holds a view's marks; the views of a light editor each carry it. */
const marks = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(set, tr) {
    let now = set.map(tr.changes);
    for (const effect of tr.effects) {
      if (!effect.is(marked)) continue;
      const built = new RangeSetBuilder<Decoration>();
      effect.value.all.forEach(({ from, to }, at) =>
        built.add(from, to, at === effect.value.on ? ACTIVE : MATCH),
      );
      now = built.finish();
    }
    return now;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/** What a view needs to be found in: the field its marks are kept in. */
export const findable: Extension = marks;

/**
 * Where each folded block in `view` that covers `at` starts. The merge view folds an unchanged
 * stretch as one block replacement, and unfolds it by that start (`uncollapseUnchanged`), so a
 * place inside the stretch is turned into the stretch's start here.
 */
function foldsOver(view: EditorView, at: number): number[] {
  const starts: number[] = [];
  for (const source of view.state.facet(EditorView.decorations)) {
    const set = typeof source === "function" ? source(view) : source;
    set.between(at, at, (from, to, deco) => {
      // A fold is a range; the merge view's spacers are points, and none of them folds.
      if (deco.spec.block === true && deco.spec.widget !== undefined && from < to) {
        starts.push(from);
      }
    });
  }
  return starts;
}

/**
 * `at` in `view` as the same place on the merge view's other side: an unchanged stretch is the
 * same text on both sides, so the offset into it carries over (the merge view's own fold click
 * maps a place the same way).
 */
function onTheOtherSide(view: EditorView, at: number): number {
  const { chunks, side } = getChunks(view.state) ?? { chunks: [], side: null };
  const ours = side === "a";
  let [mine, theirs] = [0, 0];
  for (const chunk of chunks) {
    if ((ours ? chunk.fromA : chunk.fromB) >= at) break;
    [mine, theirs] = ours ? [chunk.toA, chunk.toB] : [chunk.toB, chunk.toA];
  }
  return theirs + (at - mine);
}

/** The effects that unfold, in `view` and its merge-view sibling, the folds over `at`. */
function unfolding(view: EditorView, at: number): [EditorView, StateEffect<number>[]][] {
  const out: [EditorView, StateEffect<number>[]][] = [];
  const here = foldsOver(view, at);
  if (here.length === 0) return out;
  out.push([view, here.map((from) => uncollapseUnchanged.of(from))]);
  const pair = mergeViewSiblings(view);
  const other = pair === null ? undefined : pair.a === view ? pair.b : pair.a;
  if (other !== undefined) {
    const there = foldsOver(other, onTheOtherSide(view, at));
    if (there.length > 0) out.push([other, there.map((from) => uncollapseUnchanged.of(from))]);
  }
  return out;
}

/**
 * Every place `term` is in `text`, in any case, from the start. Matched in the text as it is,
 * never in a lowered copy: lowering can change a string's length (`İ` lowers to two code
 * units), and an offset from the copy would mark the wrong letters, or past the end.
 */
export function placesOf(text: string, term: string): { from: number; to: number }[] {
  const out: { from: number; to: number }[] = [];
  if (term === "") return out;
  const pattern = new RegExp(term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "giu");
  for (let found = pattern.exec(text); found !== null; found = pattern.exec(text)) {
    out.push({ from: found.index, to: found.index + found[0].length });
    // Overlapping matches too, as the terminal's find counts them: on from the next letter.
    pattern.lastIndex = found.index + 1;
  }
  return out;
}

/**
 * A {@link Finder} over `views` (one for a file, two for a merge view), read when each find
 * runs, so a view drawn again since is the one searched.
 */
export function finderOver(views: () => readonly EditorView[]): Finder {
  let term = "";
  let found: Match[] = [];
  let on = -1;
  const listeners = new Set<(at: FoundAt) => void>();

  const draw = () => {
    for (const view of views()) {
      const mine = found.filter((one) => one.view === view).slice(0, MARKED_AT_MOST);
      const active = on >= 0 && found[on]?.view === view ? mine.indexOf(found[on]) : undefined;
      view.dispatch({ effects: marked.of({ all: mine, on: active }) });
    }
    const here = found[on];
    if (here !== undefined) {
      for (const [view, effects] of unfolding(here.view, here.from)) view.dispatch({ effects });
      here.view.dispatch({
        selection: { anchor: here.from, head: here.to },
        effects: EditorView.scrollIntoView(here.from, { y: "center" }),
      });
    }
    for (const told of listeners) told({ index: on, count: found.length });
  };

  const search = (wanted: string) => {
    term = wanted;
    found = views().flatMap((view) =>
      placesOf(view.state.doc.toString(), wanted).map((at) => ({ view, ...at })),
    );
  };

  /** Where `one` stands among every view's text, for "at or after" across both sides. */
  const order = (one: Match) => [views().indexOf(one.view), one.from] as const;

  /** Whether `wanted` must be searched for afresh: a new word, nothing found yet, or a view
   *  drawn again since (the file moved under the bar), whose old matches point at nothing. */
  const afresh = (wanted: string) =>
    wanted !== term || found.length === 0 || found.some((one) => !views().includes(one.view));

  return {
    next(wanted, incremental) {
      if (afresh(wanted)) {
        // A word typed on stays where it was while that still matches, as the terminal's find
        // does; a new search starts from the top.
        const was = incremental && wanted !== term ? found[on] : undefined;
        search(wanted);
        const from = was === undefined ? undefined : order(was);
        const at =
          from === undefined
            ? 0
            : found.findIndex((one) => {
                const [view, offset] = order(one);
                return view > from[0] || (view === from[0] && offset >= from[1]);
              });
        on = found.length === 0 ? -1 : Math.max(0, at);
      } else if (!incremental) {
        on = (on + 1) % found.length;
      }
      draw();
    },
    previous(wanted) {
      if (afresh(wanted)) {
        search(wanted);
        on = found.length - 1;
      } else {
        on = (on - 1 + found.length) % found.length;
      }
      draw();
    },
    clear() {
      term = "";
      found = [];
      on = -1;
      for (const view of views()) view.dispatch({ effects: marked.of({ all: [] }) });
    },
    onResults(told) {
      listeners.add(told);
      return () => void listeners.delete(told);
    },
  };
}
