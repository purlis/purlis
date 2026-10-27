// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { LayoutGrid } from "lucide-react";
import { OUTSIDE, OUTSIDE_TITLE } from "./actions";
import { BROWSER_TITLE, addressChip, chatBrowserTitle, deviceOf } from "./browsers";
import {
  gridCounts,
  keepBrowsersScope,
  tilesIn,
  useBrowsersScope,
  useProjectsBrowsers,
  type GridTile,
  type ProjectBrowsers,
} from "./browsersGrid";
import { useBrowserOrigin } from "./browserWatch";
import { BrowserThumbnail, DRIVEN } from "./ChatBrowsersShown";
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
 * How many pixels across a tile's frame is drawn (#2027): twice a tile's width at its widest
 * at the default text, for a sharp picture on a high-density screen. Still a thumbnail view,
 * about a frame a second; only the picture it is drawn into is larger than a tab's.
 */
export const TILE_ACROSS = 640;

/** The key a tile goes by in the grid: two projects may each have a program of one name. */
const keyOf = (tile: GridTile): string => `${tile.plane}\u0000${tile.program}`;

/** What a tile is called: the chat's browser by its chat, or the person's own. */
export function tileTitle(tile: GridTile): string {
  return tile.name === null || tile.chat === null
    ? BROWSER_TITLE
    : chatBrowserTitle(tile.chat, tile.name);
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
export function BrowsersGrid({
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
                : [{ scope: one.scope, says: one.says, count, name: `${one.says}, ${count}` }];
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
            if (tile !== undefined) projectOf(tile)?.open(tile);
          }}
        >
          {listed.map((tile) => (
            <Tile
              key={keyOf(tile)}
              tile={tile}
              project={scope === "all" ? projectOf(tile)?.project : undefined}
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
function Tile({ tile, project }: { tile: GridTile; project: string | undefined }) {
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
      id={keyOf(tile)}
      textValue={title}
      data-program={tile.program}
      tip={tile.driver === null ? undefined : DRIVEN[tile.driver].said}
    >
      <BrowserThumbnail program={tile.program} driver={tile.driver} across={TILE_ACROSS} />
      <RowLabel>{title}</RowLabel>
      <RowDescription>{where}</RowDescription>
    </ListOption>
  );
}
