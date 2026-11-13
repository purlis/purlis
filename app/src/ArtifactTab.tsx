// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import { ChartLine } from "lucide-react";
import {
  commands,
  type ArtifactAbout,
  type ArtifactOpened,
  type ArtifactOpenedAsOf,
  type ArtifactPlace,
  type ArtifactTheme,
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
import { ArtifactHistory, AsOfStanding, openAsOf, type AsOf } from "./ArtifactHistory";
import { ArtifactLinks } from "./ArtifactLinks";
import { ArtifactOffers } from "./ArtifactOffers";
import type { Offer } from "./actions";
import { ArtifactQueries } from "./ArtifactQueries";
import { ArtifactRegenerate } from "./ArtifactRegenerate";
import { artifactDataIn, artifactsIn, useViewChanged } from "./planeChanged";
import { listen, untilStopped } from "./here";
import { DiscussButton } from "./artifactDiscuss";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Notice } from "./Notice";

/**
 * **A pinned artifact, rendered in its tab** (ADR 0122 §2–3, #5952).
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
 * - **Script runs only once approved, and only from this bar** (ADR 0122 §4, #5959): a page with
 *   script is shown without it, and the bar says so and offers Approve script. The frame's edge,
 *   which the page cannot draw, shows where it stands. An approval in any window, or a change to
 *   its template, tears the page down and opens it again under its new standing.
 * - **Live** (#5955): the page is loaded again in place when its data changes, as the plane's
 *   watch tells every window, and keeps where it was scrolled; it is drawn in the theme the
 *   window draws, and in each one it draws after; and the bar says when its data was last
 *   updated.
 * - **A page only offers actions** (ADR 0122 §8, #5972): the catalogue rows its page offers are
 *   drawn on this bar in purlis's words, and only the person's press here does one.
 * - **Its data is each person's** (ADR 0122 §1, #5961): a page that declares data files of
 *   which none is on this machine still renders, and the bar says it was not generated here.
 *   The page reads its data itself, through the port purlis hands each load (`artifacts.rs`).
 */
export function ArtifactTab({
  plane,
  workspace,
  folder,
  title,
  waits,
  onAsk,
  offerFor,
  onPress,
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
  /** The catalogue, by row id: what the page's offers are looked up in (#5972). */
  offerFor?: (id: string) => Offer | undefined;
  /** Runs a catalogue row, as the palette does: a press on an offer. */
  onPress?: (offer: Offer) => void;
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
  const [opened, setOpened] = useState<ArtifactOpened | null>(null);
  // Each change of either loads the page again: the old webview closes before the new one opens.
  const approvals = useApprovalsHeard(workspace, folder);
  const template = useViewChanged([plane], templateOf(workspace, folder));
  const data = useViewChanged([plane], artifactDataIn(workspace, folder));
  // Shown as of a version from its History (#5966): nothing on it reloads or runs live.
  const [asOf, setAsOf] = useState<AsOf | null>(null);
  const [then, setThen] = useState<ArtifactOpenedAsOf | null>(null);
  const showAsOf = (to: AsOf | null) => {
    setThen(null);
    setAsOf(to);
  };
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
        {opened === null ? null : <ArtifactQueries opened={opened} />}
        {opened === null ? null : (
          <ArtifactOffers
            key={opened.label}
            label={opened.label}
            workspace={workspace}
            name={name}
            offerFor={offerFor}
            onPress={onPress}
          />
        )}
        {opened === null || opened.generated ? null : (
          <span className="artifact-bar-standing" data-testid="artifact-data">
            Not generated on this machine
          </span>
        )}
        {asOf === null ? null : <AsOfStanding then={then} onBack={() => showAsOf(null)} />}
        <ArtifactRegenerate plane={plane} workspace={workspace} folder={folder} name={name} />
        {missing === undefined || missing === null ? (
          <ArtifactHistory
            plane={plane}
            workspace={workspace}
            folder={folder}
            name={name}
            onAsOf={showAsOf}
          />
        ) : null}
        {!unpinned && about.said?.missing === null && about.said.too_large === null ? (
          <DiscussButton plane={plane} workspace={workspace} folder={folder} />
        ) : null}
      </header>
      {opened === null ? null : <ArtifactLinks label={opened.label} onPress={onPress} />}
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
        <div className="artifact-edge" data-standing={edgeOf(opened)} data-testid="artifact-edge">
          <ArtifactFrame
            key={asOf === null ? `${approvals}:${template}` : `as-of:${asOf.id}`}
            name={name}
            opens={`${plane}\u0000${workspace}\u0000${folder}\u0000${asOf?.id ?? ""}`}
            open={
              asOf === null
                ? (first, theme) => commands.artifactOpen(plane, workspace, folder, first, theme)
                : openAsOf(plane, workspace, folder, asOf, setThen)
            }
            dataChanged={asOf === null ? data : 0}
            onOpened={setOpened}
            wayOut={remedies}
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
    case "as-of":
      return "This version's script is not the one you approved, so it is shown without it";
    default:
      return null;
  }
}

/** How many times this machine's approval of this artifact changed, as any window heard it. */
function useApprovalsHeard(workspace: string, folder: string): number {
  const [heard, setHeard] = useState(0);
  useEffect(
    () =>
      untilStopped(
        listen<ArtifactApprovalChanged>(APPROVALS_CHANGED, (event) => {
          if (approvalOf(event.payload, workspace, folder)) setHeard((n) => n + 1);
        }),
      ),
    [workspace, folder],
  );
  return heard;
}

/** A sentence the core said, as the window starts one: its first letter capital. */
function sentence(said: string): string {
  return said.charAt(0).toUpperCase() + said.slice(1);
}

/**
 * **Repin and Unpin**, the way out of an artifact that cannot be opened. A refusal is said
 * beside them as a Notice, saying what to do, and nothing else changes: Repin and Unpin
 * beside it are its retry, and Dismiss puts it away.
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
  const [refused, setRefused] = useState<{ verb: "repin" | "unpin"; why: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const act = (verb: "repin" | "unpin") => {
    setBusy(true);
    setRefused(null);
    const ask = verb === "repin" ? commands.artifactRepin : commands.artifactUnpin;
    ask(plane, workspace, folder).then(
      (answer) => {
        setBusy(false);
        if (answer.status === "error") setRefused({ verb, why: answer.error });
        else if (verb === "repin") onRepinned();
        else onUnpinned();
      },
      (why: unknown) => {
        setBusy(false);
        setRefused({ verb, why: String(why) });
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
        <Notice
          cause={`artifact-${refused.verb}-refused:${folder}`}
          tone="trouble"
          label={`Could not ${refused.verb} ${name}`}
          onDismiss={() => setRefused(null)}
        >
          {remedyRefused(refused.verb, name, refused.why)}
        </Notice>
      )}
    </>
  );
}

/** A refused Repin or Unpin in the core's sentence, and what to do about it. */
function remedyRefused(verb: "repin" | "unpin", name: string, why: string): string {
  const todo =
    verb === "repin"
      ? "Put its folder back and repin it again, or unpin it."
      : "Unpin it again once the project can be written.";
  return `purlis could not ${verb} ${name}: ${why}. ${todo}`;
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
  reread: number,
): { said?: ArtifactAbout; failed?: string } {
  // Kept with the artifact it is of, so another's is never drawn for this one.
  const of = `${plane}\u0000${workspace}\u0000${folder}\u0000${reread}`;
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
 * frame it moves, put away while it is not to be seen, and closed when it goes. A pinned
 * artifact's and an `.html` file's private preview (#5953) differ only in what `open` asks.
 */
export function ArtifactFrame({
  name,
  opens,
  open,
  dataChanged = 0,
  onOpened = () => {},
  wayOut,
}: {
  name: string;
  /** What `open` opens, as a key: the webview is made again only when it changes. */
  opens: string;
  /** Makes the webview at its first place: where the page stands, or why not. */
  open: (
    first: ArtifactPlace,
    theme: ArtifactTheme,
  ) => Promise<{ status: "ok"; data: ArtifactOpened } | { status: "error"; error: string }>;
  /**
   * A count that moves each time the page's data changes, which reloads it in place (#5955); a
   * preview has no data of its own and never moves it.
   */
  dataChanged?: number;
  /** Where the page it opened stands, or nothing once it is gone. */
  onOpened?: (opened: ArtifactOpened | null) => void;
  /** Repin and Unpin, for a pinned page the core would not open (#5954); a preview has none. */
  wayOut?: ReactNode;
}) {
  // The latest `open`, read when the webview is made: a new function for the same page opens
  // nothing new.
  const asks = useRef(open);
  useEffect(() => {
    asks.current = open;
  });
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
    asks.current(first, drawnTheme()).then(
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
  }, [opens]);

  // Its data changed: the page is loaded again in place, where it was scrolled, every window
  // showing it alike, since each hears the plane's watch (#5955). The core reads where it
  // stands again with it, which the bar says. A change to its template is the tab's: it opens
  // the page again under its new standing (#5959).
  const reloads = useRef(dataChanged);
  useEffect(() => {
    if (dataChanged === reloads.current) return;
    reloads.current = dataChanged;
    const now = label.current;
    if (now === null) return;
    commands.artifactReload(now).then(
      (answer) => {
        if (answer.status === "ok" && label.current === now) opened.current(answer.data);
      },
      () => {},
    );
  }, [dataChanged]);

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
  useEffect(
    () =>
      untilStopped(
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
        }),
      ),
    [],
  );

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
      data-native-page=""
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
