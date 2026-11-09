// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ChartLine } from "lucide-react";
import {
  commands,
  type ArtifactAbout,
  type ArtifactOpened,
  type ArtifactPlace,
  type PlaneId,
} from "./bindings";
import {
  APPROVALS_CHANGED,
  ARTIFACT_KEY,
  approvalOf,
  coveredByAnOverlay,
  drawnTheme,
  followDrawnTheme,
  freshness,
  templateOf,
  type ArtifactApprovalChanged,
  type ArtifactKeyPressed,
} from "./artifacts";
import { artifactDataIn, artifactsIn, useViewChanged } from "./planeChanged";
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
 * - **Script runs only once approved, and only from this bar** (ADR 0120 §4, #5959): a page with
 *   script is shown without it, and the bar says so and offers Approve script. The frame's edge,
 *   which the page cannot draw, shows where it stands. An approval in any window, or a change to
 *   its template, tears the page down and opens it again under its new standing.
 * - **Live** (#5955): the page is loaded again in place when its data changes, as the plane's
 *   watch tells every window, and keeps where it was scrolled; it is drawn in the theme the
 *   window draws, and in each one it draws after; and the bar says when its data was last
 *   updated.
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
  const [opened, setOpened] = useState<ArtifactOpened | null>(null);
  // Each change of either loads the page again: the old webview closes before the new one opens.
  const approvals = useApprovalsHeard(workspace, folder);
  const template = useViewChanged([plane], templateOf(workspace, folder));
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
        {opened === null ? null : <ScriptStanding opened={opened} />}
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
        <div className="artifact-edge" data-standing={edgeOf(opened)} data-testid="artifact-edge">
          <ArtifactFrame
            key={`${approvals}:${template}`}
            plane={plane}
            workspace={workspace}
            folder={folder}
            name={name}
            onOpened={setOpened}
          />
        </div>
      )}
    </section>
  );
}

/** What the frame's edge shows: whether the page's script runs, is asked about, or there is none. */
function edgeOf(opened: ArtifactOpened | null): "plain" | "asks" | "runs" {
  if (opened === null) return "plain";
  if (opened.standing === "approved") return "runs";
  if (ASKS.includes(opened.standing)) return "asks";
  return "plain";
}

/** The standings in which the bar asks the person to approve the page's script. */
const ASKS = ["new", "changed", "pin-changed"];

/** What the bar says of the page's script, and its one way to approve it. */
function ScriptStanding({ opened }: { opened: ArtifactOpened }) {
  const [failed, setFailed] = useState<string | null>(null);
  const said = standingSaid(opened);
  if (said === null) return null;
  const approve = () => {
    setFailed(null);
    commands.artifactApprove(opened.label).then(
      (answer) => {
        if (answer.status === "error") setFailed(answer.error);
      },
      (why: unknown) => setFailed(String(why)),
    );
  };
  return (
    <>
      <span className="artifact-bar-standing" data-testid="artifact-standing">
        {failed ?? said}
      </span>
      {ASKS.includes(opened.standing) ? (
        <Button variant="grants" size="sm" onPress={approve}>
          Approve script
        </Button>
      ) : null}
    </>
  );
}

/** The bar's sentence for where the page's script stands, or nothing for a page with none. */
function standingSaid(opened: ArtifactOpened): string | null {
  switch (opened.standing) {
    case "approved":
      return "Script approved on this machine";
    case "new":
      return "Has script, shown without it until you approve it";
    case "changed":
      return "Its template changed since you approved it, so it is shown without its script";
    case "pin-changed":
      return `${opened.changed_by ?? "Someone"} changed its pin since you approved it, so it is shown without its script`;
    case "not-here":
      return "Shown without its script: scripted artifacts do not run on this system yet";
    default:
      return null;
  }
}

/** How many times this machine's approval of this artifact changed, as any window heard it. */
function useApprovalsHeard(workspace: string, folder: string): number {
  const [heard, setHeard] = useState(0);
  useEffect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    listen<ArtifactApprovalChanged>(APPROVALS_CHANGED, (event) => {
      if (approvalOf(event.payload, workspace, folder)) setHeard((n) => n + 1);
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
  }, [workspace, folder]);
  return heard;
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
  onOpened,
}: {
  plane: PlaneId;
  workspace: string;
  folder: string;
  name: string;
  /** Where the page it opened stands, or nothing once it is gone. */
  onOpened: (opened: ArtifactOpened | null) => void;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const label = useRef<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const hint = useId();
  // Read when the page opens or goes, not listened under: a new callback must not reopen it.
  const opened = useRef(onOpened);
  useEffect(() => {
    opened.current = onOpened;
  });

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
          void commands.artifactClose(answer.data.label).catch(() => {});
          return;
        }
        label.current = answer.data.label;
        shown = first;
        opened.current(answer.data);
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
      if (was !== null) {
        void commands.artifactClose(was).catch(() => {});
        opened.current(null);
      }
    };
  }, [plane, workspace, folder]);

  // Its data changed: the page is loaded again in place, where it was scrolled, every window
  // showing it alike, since each hears the plane's watch (#5955). The core reads where it
  // stands again with it, which the bar says. A change to its template is the tab's: it opens
  // the page again under its new standing (#5959).
  const changed = useViewChanged([plane], artifactDataIn(workspace, folder));
  const reloads = useRef(changed);
  useEffect(() => {
    if (changed === reloads.current) return;
    reloads.current = changed;
    const now = label.current;
    if (now === null) return;
    commands.artifactReload(now).then(
      (answer) => {
        if (answer.status === "ok" && label.current === now) opened.current(answer.data);
      },
      () => {},
    );
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
