// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Bug } from "lucide-react";
import type { KeyboardEvent as ReactKeyboardEvent, PointerEvent, WheelEvent } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { CHAT_KEYBOARD } from "./actions";
import { type BrowserWatching, commands } from "./bindings";
import { keyInput, mouseInput, pagePoint, wheelInput } from "./browserDriving";
import { DROPPED, useSender } from "./BrowserView";
import { leavesTheSurface } from "./leaveKeys";
import { useBrowserWatch, useWindowSeen } from "./browserWatch";
import { INSPECTOR_SMALLEST } from "./inspector";
import { useProjectShown } from "./projectShown";
import { EmptyState } from "./ui/empty-state/EmptyState";

/** What the bar says while DevTools are open. */
export const DEVTOOLS_SAID =
  "DevTools of this browser's page. You drive the browser while they are open, and they close when you press Done on it. Press Ctrl+Tab to move out of DevTools.";

/** The keys that are only held with another: they reach DevTools with the key they are held
 *  for, never alone, so none is left held when the keyboard leaves. */
const ONLY_HELD = new Set(["Shift", "Control", "Alt", "Meta"]);

/** Why DevTools closed, as their view says it (#2028): what happened, and what to do. */
export const CLOSED_SAID: Record<string, string> = {
  handed_back:
    "They closed when the browser was handed back. Press Inspect on the browser to open them again.",
  page_gone:
    "The page they inspected closed. Press Inspect on the browser to open them on the page it shows now.",
  fell_behind:
    "They fell too far behind the page. Press Inspect on the browser to open them again.",
  too_large:
    "The page sent them more than they can show at once. Press Inspect on the browser to open them again.",
  moved:
    "They left their own page, so purlis closed them. Press Inspect on the browser to open them again.",
  ended: "They ended. Press Inspect on the browser to open them again.",
  stopped: "The browser stopped. Open a new browser to inspect a page.",
};

/** How long the pane's size settles before DevTools are sized to it, in milliseconds. */
const SIZED_AFTER = 200;

/**
 * **A browser's DevTools, drawn live** (#2028): the engine's own DevTools of the page browser
 * `program` shows, opened by the person's Inspect. Their frames come whole, as a browser's do,
 * and are drawn on a canvas; nothing of DevTools runs in the window. The person's clicks, wheel
 * and keys go to DevTools alone, aimed at what the frame they saw showed. They are sized to the
 * pane. Only the person drives them: they close as the browser is handed back, and the view
 * then says why they closed. Every key goes to DevTools but Ctrl+Tab (Ctrl+Shift+Tab back),
 * which leaves them as it leaves a chat's terminal (`leaveKeys.leavesTheSurface`), and the
 * palette's own keys.
 */
export function InspectorView({ program }: { program: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const surface = useRef<HTMLDivElement>(null);
  const [said, setSaid] = useState<string | undefined>();
  const aim = useRef("");
  const size = useRef({ width: 0, height: 0 });
  const inFront = useProjectShown();
  const windowSeen = useWindowSeen();
  const seen = inFront && windowSeen;
  const [dropped, setDropped] = useState(false);
  const { send, clear } = useSender(program, setSaid, setDropped, commands.inspectorInput);

  useEffect(() => {
    if (seen) clear();
  }, [program, seen, clear]);

  const attached = useCallback((watching: BrowserWatching) => {
    size.current = { width: watching.width, height: watching.height };
  }, []);
  const { watched: shown } = useBrowserWatch(
    program,
    "focused",
    canvas,
    seen,
    undefined,
    {
      aimed: (at) => {
        aim.current = at;
      },
      attached,
    },
    "devtools",
  );

  // DevTools are as large as their pane, once its size settles.
  useEffect(() => {
    const box = surface.current;
    if (shown.kind !== "live" || box === null || typeof ResizeObserver === "undefined") return;
    let settling: ReturnType<typeof setTimeout> | undefined;
    const sized = new ResizeObserver(([entry]) => {
      const width = Math.max(INSPECTOR_SMALLEST, Math.round(entry.contentRect.width));
      const height = Math.max(INSPECTOR_SMALLEST, Math.round(entry.contentRect.height));
      if (width === size.current.width && height === size.current.height) return;
      clearTimeout(settling);
      settling = setTimeout(() => {
        void commands.inspectBrowser(program, width, height).then((answer) => {
          if (answer.status === "error") setSaid(answer.error);
          else size.current = { width, height };
        });
      }, SIZED_AFTER);
    });
    sized.observe(box);
    return () => {
      clearTimeout(settling);
      sized.disconnect();
    };
  }, [program, shown.kind]);

  const pointAt = (e: { clientX: number; clientY: number }) => {
    const box = canvas.current?.getBoundingClientRect();
    if (box === undefined) return undefined;
    return pagePoint({ x: e.clientX, y: e.clientY }, box, size.current);
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    const point = pointAt(e);
    if (point === undefined) return;
    e.preventDefault();
    e.currentTarget.focus();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    send(mouseInput(aim.current, "pressed", point, e));
  };

  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    const point = pointAt(e);
    if (point !== undefined) send(mouseInput(aim.current, "released", point, e));
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const point = pointAt(e);
    if (point !== undefined) send(mouseInput(aim.current, "moved", point, e));
  };

  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    const point = pointAt(e);
    if (point !== undefined) send(wheelInput(aim.current, point, e));
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    if (ONLY_HELD.has(e.key)) return;
    send(keyInput(aim.current, true, e));
  };

  const onKeyUp = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    if (ONLY_HELD.has(e.key)) return;
    send(keyInput(aim.current, false, e));
  };

  if (shown.kind === "ended" || shown.kind === "refused")
    return (
      <EmptyState
        mark={Bug}
        headline="DevTools are closed"
        body={
          shown.kind === "refused"
            ? shown.why
            : (CLOSED_SAID[shown.closed ?? ""] ??
              "Press Inspect on the browser to open them again.")
        }
        testid="devtools-closed"
      />
    );
  return (
    <div className="browser-view" data-testid="devtools-view">
      <div className="browser-bar" data-driving="person">
        <p className="browser-bar-said" role="status">
          {DEVTOOLS_SAID}
          {dropped && <span className="browser-bar-dropped"> {DROPPED}</span>}
          {said !== undefined && <span className="browser-bar-refused"> {said}</span>}
        </p>
      </div>
      <div
        ref={surface}
        className="browser-surface"
        data-testid="devtools-surface"
        tabIndex={0}
        aria-label="DevTools: click or type to use them"
        {...{ [CHAT_KEYBOARD]: "" }}
        onPointerDown={onPointerDown}
        onPointerUp={onPointerUp}
        onPointerMove={onPointerMove}
        onWheel={onWheel}
        onKeyDownCapture={leavesTheSurface}
        onKeyDown={onKeyDown}
        onKeyUp={onKeyUp}
        onContextMenu={(e) => e.preventDefault()}
      >
        <canvas
          ref={canvas}
          className="browser-frame"
          aria-label="DevTools"
          width={shown.kind === "live" ? shown.width : undefined}
          height={shown.kind === "live" ? shown.height : undefined}
        />
      </div>
    </div>
  );
}
