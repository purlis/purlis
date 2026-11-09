// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ChartLine } from "lucide-react";
import { commands, type ArtifactAbout, type ArtifactPlace, type PlaneId } from "./bindings";
import {
  ARTIFACT_KEY,
  coveredByAnOverlay,
  drawnTheme,
  followDrawnTheme,
  freshness,
  type ArtifactKeyPressed,
} from "./artifacts";
import { artifactIn, artifactsIn, useViewChanged } from "./planeChanged";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";

/**
 * **A pinned artifact, rendered in its tab** (ADR 0120 §2–3, #5952).
 *
 * - **A bar purlis draws, outside the page**: its name and who pinned it, which nothing on the
 *   page can draw over or fake.
 * - **The frame**: the page itself is a native webview of its own (`artifacts.rs`), with no
 *   grant, no network and no script, laid over this element and moved with it. It is put away
 *   while the tab is behind another, and while a dialog or a menu purlis draws is over it, which
 *   it would otherwise cover.
 * - **A restored tab runs nothing until pressed** (`waits`): it draws the bar and an Open
 *   button, and makes no webview and reads nothing of the page's until the person presses it.
 * - **The keyboard**: the frame is a Tab stop named by the artifact's name. Enter puts the
 *   keyboard in the page; Esc there brings it back to the frame, and F6 moves on to the next
 *   region as it does anywhere in the window.
 * - **Live** (#5955): the page is loaded again when its folder's files or data change, as the
 *   plane's watch tells every window; it is drawn in the theme the window draws, and in each
 *   one it draws after; and the bar says when its data was last updated.
 */
export function ArtifactTab({
  plane,
  workspace,
  folder,
  title,
  waits,
  onAsk,
}: {
  plane: PlaneId;
  workspace: string;
  folder: string;
  /** What the tab is called: the pin's name when it was opened. */
  title: string;
  /** Put back by a launch and not pressed for yet (`tabs.Content.waits`). */
  waits: boolean;
  /** The person pressed Open on a tab that waits. */
  onAsk: () => void;
}) {
  const about = useAbout(plane, workspace, folder);
  const name = about.said?.name ?? title;
  const missing = about.said?.missing ?? about.failed;
  return (
    <section
      className="view-pane view-alone artifact-tab"
      aria-label={name}
      data-testid="view-pane-artifact"
    >
      <header className="artifact-bar" data-testid="artifact-bar">
        <ChartLine className="node-icon" aria-hidden="true" />
        <span className="artifact-bar-name">{name}</span>
        <span className="artifact-bar-by">{pinnedBy(about.said)}</span>
        <span className="artifact-bar-fresh">{freshness(about.said?.fresh)?.said ?? ""}</span>
      </header>
      {missing !== undefined && missing !== null ? (
        <EmptyState
          headline={`${name} cannot be opened`}
          body={missing}
          mark={ChartLine}
          role="status"
          testid="artifact-missing"
        />
      ) : waits ? (
        <EmptyState
          headline={`${name} has not run yet`}
          body="Opened again with this window, so nothing on it runs until you open it."
          mark={ChartLine}
          action={<Button onPress={onAsk}>Open {name}</Button>}
          testid="artifact-waits"
        />
      ) : about.said === undefined ? null : (
        <ArtifactFrame plane={plane} workspace={workspace} folder={folder} name={name} />
      )}
    </section>
  );
}

/** What the bar says of who pinned it. */
function pinnedBy(about: ArtifactAbout | undefined): string {
  if (about === undefined) return "";
  return about.pinned_by === null
    ? "Pinned here, not saved to the project yet"
    : `Pinned by ${about.pinned_by}`;
}

/** The pin's name, who pinned it and its freshness, as the core reads them, read again when
 *  the workspace's artifacts change; or why it could not. */
function useAbout(
  plane: PlaneId,
  workspace: string,
  folder: string,
): { said?: ArtifactAbout; failed?: string } {
  // Kept with the artifact it is of, so another's is never drawn for this one.
  const of = `${plane}\u0000${workspace}\u0000${folder}`;
  const [read, setRead] = useState<{ of: string; said?: ArtifactAbout; failed?: string }>({
    of: "",
  });
  const changed = useViewChanged([plane], artifactsIn(workspace));
  useEffect(() => {
    let gone = false;
    commands.artifactAbout(plane, workspace, folder).then(
      (answer) => {
        if (gone) return;
        setRead(answer.status === "ok" ? { of, said: answer.data } : { of, failed: answer.error });
      },
      (why: unknown) => {
        if (!gone) setRead({ of, failed: String(why) });
      },
    );
    return () => {
      gone = true;
    };
  }, [of, plane, workspace, folder, changed]);
  return read.of === of ? read : {};
}

/** Where `frame` is now, as the webview is laid: nothing while it is not on screen to be seen. */
function placeOf(frame: HTMLElement): ArtifactPlace | null {
  const at = frame.getBoundingClientRect();
  if (at.width < 1 || at.height < 1 || frame.closest("[hidden]") !== null) return null;
  if (coveredByAnOverlay(at)) return null;
  return {
    x: at.left,
    y: at.top,
    width: at.width,
    height: at.height,
    ratio: window.devicePixelRatio || 1,
  };
}

function same(one: ArtifactPlace | null, other: ArtifactPlace | null): boolean {
  if (one === null || other === null) return one === other;
  return (
    one.x === other.x &&
    one.y === other.y &&
    one.width === other.width &&
    one.height === other.height &&
    one.ratio === other.ratio
  );
}

/**
 * **The frame the page's webview is laid over**: made when this mounts, moved with it each
 * frame it moves, put away while it is not to be seen, and closed when it goes.
 */
function ArtifactFrame({
  plane,
  workspace,
  folder,
  name,
}: {
  plane: PlaneId;
  workspace: string;
  folder: string;
  name: string;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const label = useRef<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const hint = useId();

  // The webview, made once for this frame's life and closed as it goes.
  useEffect(() => {
    const element = frame.current;
    if (element === null) return;
    let gone = false;
    let shown: ArtifactPlace | null = null;
    let tick = 0;
    const follow = () => {
      tick = requestAnimationFrame(follow);
      const now = label.current;
      if (now === null) return;
      const place = placeOf(element);
      if (same(place, shown)) return;
      shown = place;
      void commands.artifactPlace(now, place).catch(() => {});
    };
    const first = placeOf(element) ?? {
      x: 0,
      y: 0,
      width: 1,
      height: 1,
      ratio: window.devicePixelRatio || 1,
    };
    commands.artifactOpen(plane, workspace, folder, first, drawnTheme()).then(
      (answer) => {
        if (answer.status === "error") {
          if (!gone) setFailed(answer.error);
          return;
        }
        if (gone) {
          void commands.artifactClose(answer.data).catch(() => {});
          return;
        }
        label.current = answer.data;
        shown = first;
        follow();
      },
      (why: unknown) => {
        if (!gone) setFailed(String(why));
      },
    );
    return () => {
      gone = true;
      cancelAnimationFrame(tick);
      const was = label.current;
      label.current = null;
      if (was !== null) void commands.artifactClose(was).catch(() => {});
    };
  }, [plane, workspace, folder]);

  // Its folder's files or data changed: the page is loaded again from them, every window
  // showing it alike, since each hears the plane's watch (#5955).
  const changed = useViewChanged([plane], artifactIn(workspace, folder));
  const reloads = useRef(changed);
  useEffect(() => {
    if (changed === reloads.current) return;
    reloads.current = changed;
    const now = label.current;
    if (now !== null) void commands.artifactReload(now).catch(() => {});
  }, [changed]);

  // The window draws another theme: so does the page (#5955).
  useEffect(
    () =>
      followDrawnTheme((theme) => {
        const now = label.current;
        if (now !== null) void commands.artifactTheme(now, theme).catch(() => {});
      }),
    [],
  );

  // Esc and F6 pressed in the page come back here (`artifacts.rs`).
  useEffect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    listen<ArtifactKeyPressed>(ARTIFACT_KEY, (event) => {
      const element = frame.current;
      if (element === null || event.payload.label !== label.current) return;
      element.focus();
      if (event.payload.key !== "escape") {
        // F6 goes on from the frame, as the window's own F6 does from anywhere.
        element.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "F6",
            code: "F6",
            shiftKey: event.payload.key === "f6-back",
            bubbles: true,
            cancelable: true,
          }),
        );
      }
    }).then(
      (unlisten) => {
        if (gone) unlisten();
        else stop = unlisten;
      },
      () => {},
    );
    return () => {
      gone = true;
      try {
        stop?.();
      } catch {
        // The window's event bridge went first: nothing is listening any more.
      }
    };
  }, []);

  if (failed !== null)
    return (
      <EmptyState
        headline={`${name} cannot be opened`}
        body={failed}
        mark={ChartLine}
        role="status"
        testid="artifact-missing"
      />
    );
  return (
    <div
      ref={frame}
      className="artifact-frame"
      role="region"
      aria-label={name}
      aria-describedby={hint}
      tabIndex={0}
      data-testid="artifact-frame"
      onKeyDown={(event) => {
        if (event.key !== "Enter" || label.current === null) return;
        event.preventDefault();
        void commands.artifactEnter(label.current).catch(() => {});
      }}
    >
      <span id={hint} className="sr-only">
        Press Enter to read the page. Esc comes back here, and F6 moves on.
      </span>
    </div>
  );
}
