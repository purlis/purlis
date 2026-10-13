// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What an edit ask would write, in its Inbox row** (#1146).
 *
 * The core reads a Claude Code edit's change from the tool's own input and offers Allow only
 * where it is shown whole (`hooked::shown_in_full`); this draws it, each change in the light
 * editor's merge view (`MergeViewer`), the one the comparison tab draws a file's change in, and
 * never a view of its own. Its answers are drawn inside the same wait for that view's chunk
 * (`children`), so no Allow is on the screen before the change it lets run.
 *
 * A change not shown is said in its sentence, with nothing that lets it run: the person
 * answers it in its pane.
 */
import { lazy, Suspense, useEffect, type ReactNode } from "react";
import type { WritesShown } from "./bindings";

/** The merge view, from the editor's own chunk (ADR 0086 row M2), loaded when an edit asks. */
const MergeViewer = lazy(() =>
  import("./editor/LightEditor").then((module) => ({ default: module.MergeViewer })),
);

/** What an edit's row says while its change is being drawn. */
export const DRAWING = "Drawing the change…";

/** Tells `onDrawn` once it is drawn: inside the wait, so once the change is on the screen. */
function Drawn({ onDrawn }: { onDrawn?: () => void }) {
  useEffect(() => onDrawn?.(), [onDrawn]);
  return null;
}

/**
 * An edit's change above its answers (`children`). `onDrawn` hears when the change is on the
 * screen, so its row can wait to settle from then before an Allow under it counts (#1695).
 */
export function AskWrites({
  writes,
  onDrawn,
  children,
}: {
  writes: WritesShown;
  onDrawn?: () => void;
  children: ReactNode;
}) {
  if (writes.kind === "not-shown")
    return (
      <>
        <p className="inbox-refused" data-testid="ask-writes-not-shown">
          {writes.why}
        </p>
        {children}
      </>
    );
  return (
    <Suspense fallback={<p className="inbox-refused">{DRAWING}</p>}>
      <div className="inbox-writes" data-testid="ask-writes">
        {writes.changes.map((change, n) => (
          <section key={n} className="inbox-change">
            <p className="inbox-source">{change.says}</p>
            <MergeViewer
              path={writes.path}
              base={change.before}
              head={change.after}
              hunks={change.hunks}
              label={`${change.says}, in ${writes.path}`}
            />
          </section>
        ))}
      </div>
      <Drawn onDrawn={onDrawn} />
      {children}
    </Suspense>
  );
}
