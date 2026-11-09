// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ChartLine } from "lucide-react";
import { commands, type ArtifactAbout, type ArtifactPlace, type PlaneId } from "./bindings";
import { ARTIFACT_KEY, coveredByAnOverlay, type ArtifactKeyPressed } from "./artifacts";
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
 * - **An artifact that cannot be opened says why, with its way out** (#5954): a missing one
 *   names the path its page belongs at, a too-large one both sizes, and each offers Repin
 *   (pinned again as it is, so its folder is checked anew) and Unpin (its files stay). A
 *   refused Repin says why and leaves the state as it was. Nothing of its page is read.
 * - **The keyboard**: the frame is a Tab stop named by the artifact's name. Enter puts the
 *   keyboard in the page; Esc there brings it back to the frame, and F6 moves on to the next
 *   region as it does anywhere in the window.
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
  // Bumped by a repin, so the core is asked again what it now finds.
  const [reread, setReread] = useState(0);
  const [unpinned, setUnpinned] = useState(false);
  const about = useAbout(plane, workspace, folder, reread);
  const name = about.said?.name ?? title;
  const missing = about.said?.missing ?? about.failed;
  const tooLarge = about.said?.too_large ?? null;
  const remedies = (
    <Remedies
      plane={plane}
      workspace={workspace}
      folder={folder}
      name={name}
      onRepinned={() => setReread((n) => n + 1)}
      onUnpinned={() => setUnpinned(true)}
    />
  );
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
      </header>
      {unpinned ? (
        <EmptyState
          headline={`${name} is unpinned`}
          body={`Its files stay in artifacts/${folder}/. Close this tab, or pin it again from the command line.`}
          mark={ChartLine}
          role="status"
          testid="artifact-unpinned"
        />
      ) : missing !== undefined && missing !== null ? (
        <EmptyState
          headline={`${name} is missing`}
          body={`${sentence(missing)}. Its page belongs at artifacts/${folder}/index.html in this workspace: repin it once it is there, or unpin it.`}
          mark={ChartLine}
          role="status"
          action={remedies}
          testid="artifact-missing"
        />
      ) : tooLarge !== null ? (
        <EmptyState
          headline={`${name} is too large to open`}
          body={`${sentence(tooLarge)}. Make its folder smaller and repin it, or unpin it.`}
          mark={ChartLine}
          role="status"
          action={remedies}
          testid="artifact-too-large"
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
        <ArtifactFrame
          plane={plane}
          workspace={workspace}
          folder={folder}
          name={name}
          wayOut={remedies}
        />
      )}
    </section>
  );
}

/** A sentence the core said, as the window starts one: its first letter capital. */
function sentence(said: string): string {
  return said.charAt(0).toUpperCase() + said.slice(1);
}

/**
 * **Repin and Unpin**, the way out of an artifact that cannot be opened. A refusal is said
 * under them, as an alert, and nothing else changes.
 */
function Remedies({
  plane,
  workspace,
  folder,
  name,
  onRepinned,
  onUnpinned,
}: {
  plane: PlaneId;
  workspace: string;
  folder: string;
  name: string;
  onRepinned: () => void;
  onUnpinned: () => void;
}) {
  const [refused, setRefused] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const act = (verb: "repin" | "unpin") => {
    setBusy(true);
    setRefused(null);
    const ask = verb === "repin" ? commands.artifactRepin : commands.artifactUnpin;
    ask(plane, workspace, folder).then(
      (answer) => {
        setBusy(false);
        if (answer.status === "error")
          setRefused(`purlis could not ${verb} ${name}: ${answer.error}`);
        else if (verb === "repin") onRepinned();
        else onUnpinned();
      },
      (why: unknown) => {
        setBusy(false);
        setRefused(`purlis could not ${verb} ${name}: ${String(why)}`);
      },
    );
  };
  return (
    <>
      <Button variant="primary" isDisabled={busy} onPress={() => act("repin")}>
        Repin
      </Button>
      <Button isDisabled={busy} onPress={() => act("unpin")}>
        Unpin
      </Button>
      {refused !== null && (
        <p role="alert" className="artifact-refused">
          {refused}
        </p>
      )}
    </>
  );
}

/** What the bar says of who pinned it. */
function pinnedBy(about: ArtifactAbout | undefined): string {
  if (about === undefined) return "";
  return about.pinned_by === null
    ? "Pinned here, not saved to the project yet"
    : `Pinned by ${about.pinned_by}`;
}

/** The pin's name and who pinned it, as the core reads them; or why it could not. */
function useAbout(
  plane: PlaneId,
  workspace: string,
  folder: string,
  reread: number,
): { said?: ArtifactAbout; failed?: string } {
  // Kept with the artifact it is of, so another's is never drawn for this one.
  const of = `${plane}\u0000${workspace}\u0000${folder}\u0000${reread}`;
  const [read, setRead] = useState<{ of: string; said?: ArtifactAbout; failed?: string }>({
    of: "",
  });
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
  }, [of, plane, workspace, folder]);
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
  wayOut,
}: {
  plane: PlaneId;
  workspace: string;
  folder: string;
  name: string;
  /** Repin and Unpin, for a page the core would not open. */
  wayOut: ReactNode;
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
    commands.artifactOpen(plane, workspace, folder, first).then(
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
        body={sentence(failed)}
        mark={ChartLine}
        role="status"
        action={wayOut}
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
