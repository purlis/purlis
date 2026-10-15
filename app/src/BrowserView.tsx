// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Channel } from "@tauri-apps/api/core";
import { Globe } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { commands } from "./bindings";
import { frameBytes, latestOnly, paint } from "./browserFrames";
import { EmptyState } from "./EmptyState";
import { useProjectShown } from "./projectShown";

/** Where a browser view stands: opening, drawing, ended, or refused with the host's words. */
type Shown =
  | { kind: "opening" }
  | { kind: "live"; width: number; height: number }
  | { kind: "ended" }
  | { kind: "refused"; why: string };

/** Whether the window can be seen now: not minimised, hidden or behind a full-screen app. */
function useWindowSeen(): boolean {
  const [seen, setSeen] = useState(() => typeof document === "undefined" || !document.hidden);
  useEffect(() => {
    const look = () => setSeen(!document.hidden);
    document.addEventListener("visibilitychange", look);
    return () => document.removeEventListener("visibilitychange", look);
  }, []);
  return seen;
}

/**
 * **A browser, drawn live** (#2012, spec #2008): the browser view of `program`, a browser the
 * session host runs. Its frames come whole, as images, and are drawn on a canvas, scaled to the
 * pane; nothing of the page runs in the window. While it is on screen it is the focused view,
 * sent every frame. When nobody can see it (its project is not the one in front, or the window
 * is hidden) its view closes and the last frame stays drawn, and a browser nobody watches makes
 * no frames; it is attached again, from its latest frame, when it can be seen.
 */
export function BrowserView({ program }: { program: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [shown, setShown] = useState<Shown>({ kind: "opening" });
  const inFront = useProjectShown();
  const windowSeen = useWindowSeen();
  const seen = inFront && windowSeen;

  useEffect(() => {
    if (!seen) return;
    let gone = false;
    let view: number | undefined;
    const draw = latestOnly((jpeg) =>
      canvas.current === null ? Promise.resolve() : paint(canvas.current, jpeg),
    );
    const frames = new Channel<string>();
    frames.onmessage = (frame) => {
      if (gone) return;
      // An empty frame is the host's word that the view has ended for good.
      if (frame === "") setShown({ kind: "ended" });
      else draw(frameBytes(frame));
    };
    void commands.watchBrowser(program, "focused", frames).then((watched) => {
      if (watched.status === "error") {
        if (!gone) setShown({ kind: "refused", why: watched.error });
        return;
      }
      view = watched.data.view;
      if (gone) {
        void commands.unwatchBrowser(program, view);
        return;
      }
      setShown({ kind: "live", width: watched.data.width, height: watched.data.height });
    });
    return () => {
      gone = true;
      if (view !== undefined) void commands.unwatchBrowser(program, view);
    };
  }, [program, seen]);

  if (shown.kind === "ended")
    return (
      <EmptyState
        mark={Globe}
        headline="This browser has ended"
        body="Its pages are gone. Open a new browser from the palette to start again."
        testid="browser-ended"
      />
    );
  if (shown.kind === "refused")
    return (
      <EmptyState
        mark={Globe}
        headline="This browser cannot be shown"
        body={shown.why}
        testid="browser-refused"
      />
    );
  return (
    <div className="browser-view" data-testid="browser-view">
      <canvas
        ref={canvas}
        className="browser-frame"
        aria-label="The browser's page"
        width={shown.kind === "live" ? shown.width : undefined}
        height={shown.kind === "live" ? shown.height : undefined}
      />
    </div>
  );
}
