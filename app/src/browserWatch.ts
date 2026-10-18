// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Channel } from "@tauri-apps/api/core";
import { useEffect, useState, type RefObject } from "react";
import { commands, type BrowserFrame, type BrowserViewRole } from "./bindings";
import { frameBytes, latestOnly, paint } from "./browserFrames";

/** Whether the window can be seen now: not minimised, hidden or behind a full-screen app. */
export function useWindowSeen(): boolean {
  const [seen, setSeen] = useState(() => typeof document === "undefined" || !document.hidden);
  useEffect(() => {
    const look = () => setSeen(!document.hidden);
    document.addEventListener("visibilitychange", look);
    return () => document.removeEventListener("visibilitychange", look);
  }, []);
  return seen;
}

/**
 * How many pixels across a thumbnail's frame is drawn (#2024): over twice the thumbnail's
 * width at the default text, for a sharp picture on a high-density screen.
 */
export const THUMBNAIL_ACROSS = 64;

/** Where a watched browser stands: opening, drawing, ended, or refused with the host's words. */
export type Watched =
  | { kind: "opening" }
  | { kind: "live"; width: number; height: number }
  | { kind: "ended" }
  | { kind: "refused"; why: string };

/**
 * **A browser view of `program`, drawn on `canvas` while `seen`** (#2024): attached as `role`
 * (a thumbnail is sent about a frame a second), each whole frame drawn as it comes, the newest
 * only, and `across` pixels wide where it is given; closed when it cannot be seen, so a browser nobody watches makes no frames, and the
 * last frame stays drawn until it is seen again.
 */
export function useBrowserWatch(
  program: string,
  role: BrowserViewRole,
  canvas: RefObject<HTMLCanvasElement | null>,
  seen: boolean,
  across?: number,
): Watched {
  const [watched, setWatched] = useState<Watched>({ kind: "opening" });
  useEffect(() => {
    if (!seen) return;
    let gone = false;
    let view: number | undefined;
    const draw = latestOnly((jpeg) =>
      canvas.current === null ? Promise.resolve() : paint(canvas.current, jpeg, across),
    );
    const frames = new Channel<BrowserFrame>();
    frames.onmessage = (frame) => {
      if (gone) return;
      // Where the view stands while its runner's link is down (#2542) is said over the focused
      // view; a thumbnail keeps its last frame as it is.
      if (frame.reconnection !== null) return;
      // An empty frame is the host's word that the view has ended for good. What it shows
      // (`aim`) is for the person's input, which a thumbnail takes none of.
      if (frame.jpeg === "") setWatched({ kind: "ended" });
      else draw(frameBytes(frame.jpeg));
    };
    void commands.watchBrowser(program, role, frames).then((answer) => {
      if (answer.status === "error") {
        if (!gone) setWatched({ kind: "refused", why: answer.error });
        return;
      }
      view = answer.data.view;
      if (gone) {
        void commands.unwatchBrowser(program, view);
        return;
      }
      setWatched({ kind: "live", width: answer.data.width, height: answer.data.height });
    });
    return () => {
      gone = true;
      if (view !== undefined) void commands.unwatchBrowser(program, view);
    };
  }, [program, role, canvas, seen, across]);
  return watched;
}
