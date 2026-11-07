// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A check result in the head's gutter** (ADR 0117, #5407): a letter on each changed line a result
 * is on, whose title says its tool, severity and message. A pointer reads it there; the keyboard and
 * a screen reader read the same words in the list beside it (`CheckResults.tsx`), since nothing in a
 * gutter takes focus.
 */
import { RangeSet, type Extension } from "@codemirror/state";
import { gutter, GutterMarker } from "@codemirror/view";

/** One mark: the line, the worst severity on it, and what it says. */
export type CheckMark = { line: number; severity: string; said: string };

const LETTER: Record<string, string> = { error: "E", warning: "W", note: "N" };

class Mark extends GutterMarker {
  constructor(readonly mark: CheckMark) {
    super();
  }
  override eq(other: GutterMarker) {
    return (
      other instanceof Mark &&
      other.mark.said === this.mark.said &&
      other.mark.severity === this.mark.severity
    );
  }
  override toDOM() {
    const drawn = document.createElement("span");
    drawn.className = "cm-check-mark";
    drawn.dataset.severity = this.mark.severity;
    drawn.title = this.mark.said;
    drawn.textContent = LETTER[this.mark.severity] ?? "·";
    return drawn;
  }
}

/** The gutter holding `marks`, none past the end of the file. Nothing at all with none to draw. */
export function checkGutter(marks: readonly CheckMark[]): Extension {
  if (marks.length === 0) return [];
  return gutter({
    class: "cm-check-gutter",
    markers: (view) => {
      const lines = view.state.doc.lines;
      return RangeSet.of(
        [...marks]
          .filter((mark) => mark.line >= 1 && mark.line <= lines)
          .sort((a, b) => a.line - b.line)
          .map((mark) => new Mark(mark).range(view.state.doc.line(mark.line).from)),
      );
    },
    initialSpacer: () => new Mark({ line: 1, severity: "error", said: "" }),
  });
}
