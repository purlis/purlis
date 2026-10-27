// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { LayoutGrid } from "lucide-react";
import { OUTSIDE, OUTSIDE_TITLE } from "./actions";
import { BROWSER_TITLE, addressChip, chatBrowserTitle, deviceOf } from "./browsers";
import {
  gridCounts,
  keepBrowsersScope,
  openTile,
  tilesIn,
  useBrowsersScope,
  useProjectsBrowsers,
  type GridTile,
  type ProjectBrowsers,
} from "./browsersGrid";
import { useBrowserOrigin } from "./browserWatch";
import { BrowserThumbnail, DRIVEN, browsersSaid } from "./ChatBrowsersShown";
import { INBOX_SCOPES, scopeDrawn, scopeOf, widerWaiting } from "./inboxScope";
import { ScopeSwitch } from "./ScopeSwitch";
import { onAMac } from "./tabKeys";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { ListBox, ListOption } from "./ui/list/List";
import { RowDescription, RowLabel } from "./ui/list/Row";

/** What the grid's scope switch is called. */
export const GRID_SCOPE_LABEL = "Show the browsers of";

/**
 * How many pixels across a tile's frame is drawn (#2027): over twice the widest a tile is drawn,
 * which is under twice its least width (28rem, 392 pixels at the default text), so the picture
 * is sharp on a screen of two pixels to a point. Still a thumbnail view, about a frame a second;
 * only the picture it is drawn into is larger than a tab's.
 */
export const TILE_ACROSS = 800;

/**
 * **How many tiles are live at once, at most** (#2027): each live tile is a view the browser
 * makes frames for. Past it, and for a tile scrolled out of sight, the tile keeps the last frame
 * it drew and asks for no more until it is among the first this many in sight again.
 */
export const LIVE_AT_MOST = 24;

/** The key a tile goes by in the grid: two projects may each have a program of one name. */
const keyOf = (tile: GridTile): string => `${tile.plane}\u0000${tile.program}`;

/**
 * What a tile is called: the person's own by that alone, else the chat's browser by its chat,
 * or by its name alone where no name of its chat was kept.
 */
export function tileTitle(tile: GridTile): string {
  if (tile.own || tile.name === null) return BROWSER_TITLE;
  return tile.chat === null || tile.chat === ""
    ? `${BROWSER_TITLE} ${tile.name}`
    : chatBrowserTitle(tile.chat, tile.name);
}

/**
 * **Which tiles are in sight**, by key, as an `IntersectionObserver` sees them: `null` where the
 * window has none, and every tile counts as in sight. `observe` is each tile's ref.
 */
function useInSight(): {
  inSight: ReadonlySet<string> | null;
  observe: (key: string) => (element: HTMLElement | null) => void;
} {
  // Nothing is in sight until the observer says so: a tile asks for nothing before.
  const [inSight, setInSight] = useState<ReadonlySet<string> | null>(() =>
    typeof IntersectionObserver === "undefined" ? null : new Set(),
  );
  const keys = useRef(new Map<Element, string>());
  const observer = useRef<IntersectionObserver | null>(null);
  useEffect(() => {
    if (typeof IntersectionObserver === "undefined") return;
    const seen = new Set<string>();
    const watching = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const key = keys.current.get(entry.target);
        if (key === undefined) continue;
        if (entry.isIntersecting) seen.add(key);
        else seen.delete(key);
      }
      setInSight(new Set(seen));
    });
    observer.current = watching;
    for (const element of keys.current.keys()) watching.observe(element);
    return () => {
      watching.disconnect();
      observer.current = null;
    };
  }, []);
  // One ref a key, kept, so drawing the grid again observes nothing again.
  const refs = useRef(new Map<string, (element: HTMLElement | null) => void>());
  const observe = useCallback((key: string) => {
    const held = refs.current.get(key);
    if (held !== undefined) return held;
    let at: HTMLElement | null = null;
    const ref = (element: HTMLElement | null) => {
      if (at !== null) {
        keys.current.delete(at);
        observer.current?.unobserve(at);
      }
      at = element;
      if (element === null) return;
      keys.current.set(element, key);
      observer.current?.observe(element);
    };
    refs.current.set(key, ref);
    return ref;
  }, []);
  return { inSight, observe };
}

/**
 * **The Browsers grid** (#2027, spec #2008): every browser in scope as a live tile, about a
 * frame a second, drawn while the tab can be seen. The switch at the top is the Inbox's
 * (`ScopeSwitch`, `inboxScope.ts`): every project this window holds, this project (the
 * default), or the workspace whose strip the tab is on, each with its count, the pick kept for
 * the project. A scope with nothing in it says how many run in the nearest wider one, with one
 * press to widen.
 *
 * The tiles are one list to act on (`ListBox`, `layout="grid"`): one Tab stop, the arrows walk
 * the grid, and a press or Enter opens the browser's view in its own project, on the strip of
 * its chat, which comes to the front where it is another project.
 */
export function BrowsersGridTab({
  plane,
  workspace,
}: {
  plane: string;
  /** The workspace whose strip the tab is on, by the strip's key: This workspace's. */
  workspace?: string;
}) {
  const projects = useProjectsBrowsers();
  const picked = useBrowsersScope(plane);
  const counts = gridCounts(projects, plane, workspace);
  const scope = scopeDrawn(picked, counts);
  const listed = tilesIn(scope, projects, plane, workspace);
  const wider = widerWaiting(scope, counts);
  const projectOf = (tile: GridTile): ProjectBrowsers | undefined =>
    projects.find((one) => one.plane === tile.plane);
  const { inSight, observe } = useInSight();
  /** The tiles that are live: the first {@link LIVE_AT_MOST} in sight. */
  const live = new Set(
    listed
      .map(keyOf)
      .filter((key) => inSight === null || inSight.has(key))
      .slice(0, LIVE_AT_MOST),
  );
  return (
    <div className="browsers-grid-tab">
      {(counts.all ?? 0) > 0 && (
        <div className="inbox-scope">
          <ScopeSwitch
            label={GRID_SCOPE_LABEL}
            scopes={INBOX_SCOPES.flatMap((one) => {
              const count = counts[one.scope];
              return count === undefined
                ? []
                : [
                    {
                      scope: one.scope,
                      says: one.says,
                      count,
                      name: `${one.says}, ${browsersSaid(count)}`,
                    },
                  ];
            })}
            scope={scope}
            onPick={(to) => keepBrowsersScope(plane, to)}
          />
        </div>
      )}
      {listed.length === 0 ? (
        <EmptyState
          mark={LayoutGrid}
          headline={`No browser runs in ${scopeOf(scope)}`}
          body={
            wider === undefined
              ? "A browser shows here, live, once a chat starts one or you open one with New browser."
              : `${wider.count} ${wider.count === 1 ? "browser runs" : "browsers run"} in ${scopeOf(wider.scope)}.`
          }
          action={
            wider === undefined ? undefined : (
              <Button size="sm" onPress={() => keepBrowsersScope(plane, wider.scope)}>
                {`Show ${scopeOf(wider.scope)}`}
              </Button>
            )
          }
          testid="browsers-grid-empty"
        />
      ) : (
        <ListBox
          aria-label="Browsers"
          layout="grid"
          className="browsers-grid"
          onAction={(key) => {
            const tile = listed.find((one) => keyOf(one) === key);
            if (tile !== undefined) openTile(tile);
          }}
        >
          {listed.map((tile) => (
            <Tile
              key={keyOf(tile)}
              tile={tile}
              project={scope === "all" ? projectOf(tile)?.project : undefined}
              live={live.has(keyOf(tile))}
              observe={observe}
            />
          ))}
        </ListBox>
      )}
    </div>
  );
}

/** What the strip calls the workspace with that key. */
const workspaceNamed = (key: string): string => (key === OUTSIDE ? OUTSIDE_TITLE : key);

/**
 * **One browser's tile**: its live picture, what it is called, and under that where it is:
 * its project among every project's, its workspace, its page and device, as the chip above its
 * page says them (#2300), and who drives it where its agent does not.
 */
function Tile({
  tile,
  project,
  live,
  observe,
}: {
  tile: GridTile;
  project: string | undefined;
  /** It may ask for frames: among the first {@link LIVE_AT_MOST} in sight. */
  live: boolean;
  observe: (key: string) => (element: HTMLElement | null) => void;
}) {
  const origin = useBrowserOrigin(tile.program);
  const title = tileTitle(tile);
  const where = [
    project,
    workspaceNamed(tile.workspace),
    addressChip(origin ?? null, deviceOf(tile.pool, onAMac())).said,
    tile.driver === null ? undefined : DRIVEN[tile.driver].mark,
  ]
    .filter((part): part is string => part !== undefined && part !== "")
    .join(" · ");
  return (
    <ListOption
      ref={observe(keyOf(tile))}
      id={keyOf(tile)}
      textValue={title}
      data-program={tile.program}
      tip={tile.driver === null ? undefined : DRIVEN[tile.driver].said}
    >
      <BrowserThumbnail
        program={tile.program}
        driver={tile.driver}
        across={TILE_ACROSS}
        live={live}
      />
      <RowLabel>{title}</RowLabel>
      <RowDescription>{where}</RowDescription>
    </ListOption>
  );
}
