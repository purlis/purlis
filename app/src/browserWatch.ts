// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, useSyncExternalStore, type RefObject } from "react";
import {
  commands,
  type BrowserFrame,
  type BrowserViewRole,
  type BrowserWatching,
} from "./bindings";
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
  | { kind: "ended"; closed?: string }
  | { kind: "refused"; why: string };

/**
 * **Where each browser's page is, by program** (#2300): the origin its views were last told
 * (`http://localhost:5173`), `null` for no page or one with no origin of its own. A browser no
 * view has been told of is not here. Its views write it; its rows and chip read it.
 */
const origins = new Map<string, string | null>();
const listeners = new Set<() => void>();

/** Browser `program`'s page is at `origin` now, as one of its views was told. */
export function originTold(program: string, origin: string | null): void {
  if (origins.has(program) && origins.get(program) === origin) return;
  origins.set(program, origin);
  for (const listener of listeners) listener();
}

/** Where browser `program`'s page is, as its views were last told: `undefined` if never. */
export function originOf(program: string): string | null | undefined {
  return origins.get(program);
}

/** Forgets where every browser's page is: for a test, which starts from none. */
export function forgetOrigins(): void {
  origins.clear();
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** **Where browser `program`'s page is** (#2300), following what its views are told. */
export function useBrowserOrigin(program: string): string | null | undefined {
  return useSyncExternalStore(subscribe, () => originOf(program));
}

/** What a watch tells its caller besides drawing: each frame's aim, the view attached. */
export type WatchHeard = {
  /** What the frame just drawn shows (`p1:5`): what the person's input is aimed at. */
  aimed?: (aim: string) => void;
  /** The view attached: the page's size, and whether the person drives it (#2014). */
  attached?: (watching: BrowserWatching) => void;
};

/** A watched browser: where it stands, and whether its runner's link is down (#2542). */
export type BrowserWatch = { watched: Watched; reconnecting: boolean };

/**
 * **A browser view of `program`, drawn on `canvas` while `seen`** (#2024): attached as `role`
 * (a thumbnail is sent about a frame a second), each whole frame drawn as it comes, the newest
 * only, and `across` pixels wide where it is given; closed when it cannot be seen, so a browser
 * nobody watches makes no frames, and the last frame stays drawn until it is seen again.
 *
 * Where its page is goes to the browsers' origins ({@link useBrowserOrigin}, #2300), what each
 * frame shows and the view's attachment to `heard`, and whether the link to the runner it is
 * drawn from is down (#2542) is in the answer.
 *
 * `of` `"devtools"` watches the browser's open DevTools instead of its page (#2028): their
 * frames, as the page's are, which name no page's origin.
 */
export function useBrowserWatch(
  program: string,
  role: BrowserViewRole,
  canvas: RefObject<HTMLCanvasElement | null>,
  seen: boolean,
  across?: number,
  heard?: WatchHeard,
  of: "page" | "devtools" = "page",
): BrowserWatch {
  const [watched, setWatched] = useState<Watched>({ kind: "opening" });
  const [reconnecting, setReconnecting] = useState(false);
  // Read at each frame, so a caller's new callbacks never attach the view again.
  const told = useRef(heard);
  useEffect(() => {
    told.current = heard;
  });
  useEffect(() => {
    if (!seen) return;
    let gone = false;
    let view: number | undefined;
    let closedWhy: string | undefined;
    const draw = latestOnly((jpeg) =>
      canvas.current === null ? Promise.resolve() : paint(canvas.current, jpeg, across),
    );
    const frames = new Channel<BrowserFrame>();
    frames.onmessage = (frame) => {
      if (gone) return;
      // Where the view stands while its runner's link is down and back: no image of its own.
      if (frame.reconnection !== null) {
        setReconnecting(frame.reconnection === "reconnecting");
        return;
      }
      // Why a browser's DevTools closed, just before their view ends (#2028).
      if (frame.closed !== null) {
        closedWhy = frame.closed;
        return;
      }
      // Where its page is, its origin alone (#2300): no image of its own.
      if (frame.page !== null) {
        originTold(program, frame.page.origin);
        return;
      }
      // An empty frame is the host's word that the view has ended for good.
      if (frame.jpeg === "") {
        setWatched(
          closedWhy === undefined ? { kind: "ended" } : { kind: "ended", closed: closedWhy },
        );
        return;
      }
      // A frame comes only over a link that is up.
      setReconnecting(false);
      told.current?.aimed?.(frame.aim);
      draw(frameBytes(frame.jpeg));
    };
    const watching =
      of === "devtools"
        ? commands.watchInspector(program, frames)
        : commands.watchBrowser(program, role, frames);
    void watching.then((answer) => {
      if (answer.status === "error") {
        if (!gone) setWatched({ kind: "refused", why: answer.error });
        return;
      }
      view = answer.data.view;
      if (gone) {
        void commands.unwatchBrowser(program, view);
        return;
      }
      told.current?.attached?.(answer.data);
      setWatched({ kind: "live", width: answer.data.width, height: answer.data.height });
    });
    return () => {
      gone = true;
      if (view !== undefined) void commands.unwatchBrowser(program, view);
    };
  }, [program, role, canvas, seen, across, of]);
  return { watched, reconnecting };
}
