// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, type ReactNode } from "react";
import { useSyncExternalStoreWithSelector } from "use-sync-external-store/with-selector";
import {
  commands,
  type Panels as PanelsModel,
  type Piece,
  type PlaneId,
  type RepoStates,
} from "./bindings";
import { followPlaneChanged, panelsOf } from "./planeChanged";
import type { WorkspaceState } from "./workspaceState";

/**
 * **The focused workspace's panels, held outside the view that draws them** (#1160).
 *
 * `useWorkspaceState` lived in `PlaneView`, so every answer it took — a memory an agent saved,
 * a session record written — drew the whole project window again, twice: once for the count
 * `usePlaneChanged` bumped, and once for the answer. That is the render-count half of #935's
 * review. This store is the shape SC-3 gave chat state (#681, `chatState.ts`): the asks and
 * the answer live here, and each region reads the part it draws through
 * `useSyncExternalStoreWithSelector`, so an answer that moves only the memory panel's rows
 * draws the memory panel and nothing around it.
 *
 * **The asks are `workspaceState.ts`'s, unchanged** — that file argues their cost, and its
 * tests pin it: one `workspace_panels` and one `workspace_repos` per focus, one
 * `worktree_list` per clone when the clone NAMES change, and the plane read alone again when
 * the plane changes on disk.
 *
 * **An answer that says what was already known keeps what was known**, field by field, so a
 * reader comparing by identity sees no change where there is none. The core answers a whole
 * `Panels` for every write a workspace's panels are made of; most of it is the same as last
 * time, and only the store that answers again can tell.
 */
export class WorkspacePanels {
  readonly #plane: PlaneId;
  #listeners = new Set<() => void>();
  #snapshot: Held = { workspace: undefined, state: NOTHING_FOCUSED };
  #answer: Answer | undefined;
  /** What the asks below were made for: the workspace and the window's `again`. */
  #focus: { workspace: string; again: number } | undefined;
  /** Each kind of ask's latest, so an older answer arriving late is dropped. */
  #asks = { panels: 0, repos: 0, pieces: 0 };
  /** The clone names the pieces were last asked for, under this focus. */
  #piecesFor: string | undefined;

  constructor(plane: PlaneId) {
    this.#plane = plane;
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  };

  readonly snapshot = (): Held => this.#snapshot;

  /**
   * Reads `workspace`, or reads it again when `again` moved: the window's counter, bumped when
   * something it did makes the record wrong (charter-app#174) — a worktree removed from an
   * explorer row. The same pair twice asks nothing.
   */
  focus(workspace: string | undefined, again = 0): void {
    if (workspace === undefined) {
      this.drop();
      return;
    }
    const was = this.#focus;
    if (was?.workspace === workspace && was.again === again) return;
    this.#focus = { workspace, again };
    // Another workspace starts from nothing. The same one asked again keeps what it knows
    // until the answers replace it, so nothing on screen blinks out while it is read again.
    if (this.#answer?.workspace !== workspace) {
      this.#answer = { workspace, pieces: NO_PIECES, piecesRefused: NONE_REFUSED };
      this.#publish();
    }
    this.#askPanels(workspace);
    this.#askRepos(workspace);
    // Asked again: the pieces with the clones already known, as the hook's own effect did.
    const known = this.#answer?.panels?.repos.join("\n");
    this.#piecesFor = undefined;
    if (known !== undefined) this.#askPieces(workspace, known);
  }

  /** Reads the focused workspace's plane answer again, and never git (charter-app#264). */
  panelsChanged(): void {
    if (this.#focus !== undefined) this.#askPanels(this.#focus.workspace);
  }

  /** Forgets the focus and drops every answer still on its way. */
  drop(): void {
    this.#focus = undefined;
    this.#piecesFor = undefined;
    this.#asks = {
      panels: this.#asks.panels + 1,
      repos: this.#asks.repos + 1,
      pieces: this.#asks.pieces + 1,
    };
  }

  /** Follows the plane on disk for the focused workspace's panels, until the stop is called. */
  follow(): () => void {
    return followPlaneChanged(
      [this.#plane],
      () => panelsOf(this.#focus?.workspace ?? ""),
      () => this.panelsChanged(),
    );
  }

  #askPanels(workspace: string): void {
    const ask = ++this.#asks.panels;
    const mine = () => ask === this.#asks.panels && this.#focus?.workspace === workspace;
    void commands
      .workspacePanels(this.#plane, workspace)
      .then((said) => {
        if (!mine()) return;
        if (said.status === "error") this.#merge(workspace, { panelsTrouble: said.error });
        // An `ok` answer with no body reads as `ok` all the same: nothing to draw.
        else if (said.data?.workspace === workspace)
          this.#merge(workspace, { panels: said.data, panelsTrouble: undefined });
      })
      .catch((err: unknown) => {
        if (mine()) this.#merge(workspace, { panelsTrouble: String(err) });
      });
  }

  #askRepos(workspace: string): void {
    const ask = ++this.#asks.repos;
    const mine = () => ask === this.#asks.repos && this.#focus?.workspace === workspace;
    void commands
      .workspaceRepos(this.#plane, workspace)
      .then((said) => {
        if (!mine()) return;
        if (said.status === "error")
          this.#merge(workspace, { read: true, reposTrouble: said.error });
        else if (said.data?.workspace === workspace)
          this.#merge(workspace, { read: true, repos: said.data, reposTrouble: undefined });
      })
      .catch((err: unknown) => {
        if (mine()) this.#merge(workspace, { read: true, reposTrouble: String(err) });
      });
  }

  /**
   * The pieces of each clone, asked when the clone NAMES change and not when the plane answers
   * again with the same ones: `\n` cannot be in a clone's name, a directory name charter has
   * already checked. See `workspaceState.ts` on what each costs.
   */
  #askPieces(workspace: string, names: string): void {
    if (names === this.#piecesFor) return;
    this.#piecesFor = names;
    const ask = ++this.#asks.pieces;
    const mine = () => ask === this.#asks.pieces && this.#focus?.workspace === workspace;
    for (const repo of names === "" ? [] : names.split("\n")) {
      void commands
        .worktreeList(this.#plane, workspace, repo)
        .then((said) => {
          if (!mine()) return;
          this.#change(workspace, (was) => {
            if (said.status === "error")
              return { ...was, piecesRefused: { ...was.piecesRefused, [repo]: said.error } };
            // A listing that went through clears that clone's refusal (NO-4).
            const refused = Object.fromEntries(
              Object.entries(was.piecesRefused).filter(([one]) => one !== repo),
            );
            return {
              ...was,
              pieces: { ...was.pieces, [repo]: said.data ?? [] },
              piecesRefused: refused,
            };
          });
        })
        .catch((err: unknown) => {
          if (!mine()) return;
          this.#change(workspace, (was) => ({
            ...was,
            piecesRefused: { ...was.piecesRefused, [repo]: String(err) },
          }));
        });
    }
  }

  #merge(workspace: string, what: Partial<Answer>): void {
    this.#change(workspace, (was) => ({ ...was, ...what }));
    const names = this.#answer?.panels?.repos.join("\n");
    if (names !== undefined) this.#askPieces(workspace, names);
  }

  #change(workspace: string, to: (was: Answer) => Answer): void {
    const was = this.#answer;
    // Not this workspace's any more: dropped rather than merged onto another one's.
    if (was?.workspace !== workspace) return;
    this.#answer = to(was);
    this.#publish();
  }

  #publish(): void {
    const answer = this.#answer;
    if (answer === undefined) return;
    const was = this.#snapshot;
    // Kept against what a reader of THIS workspace was drawing: the nothing-yet state for one
    // just focused (`stateFor`), so its first publish is no change to any reader.
    const state = kept(
      was.workspace === answer.workspace ? was.state : NOTHING_YET,
      stateOf(answer),
    );
    if (was.workspace === answer.workspace && state === was.state) return;
    this.#snapshot = { workspace: answer.workspace, state };
    for (const listener of this.#listeners) listener();
  }
}

/** What the store holds: the state, and the workspace it is about. */
export type Held = { workspace: string | undefined; state: WorkspaceState };

/** Everything the core has said about ONE workspace, as `workspaceState.ts` keeps it. */
type Answer = {
  workspace: string;
  panels?: PanelsModel;
  repos?: RepoStates;
  pieces: Record<string, Piece[]>;
  piecesRefused: Record<string, string>;
  /** Each ask's own refusal, so one ask's success clears its own and never the other's (NO-4). */
  panelsTrouble?: string;
  reposTrouble?: string;
  /** Whether the repos ask has come back, however it came back. */
  read?: boolean;
};

const NO_PIECES: Record<string, Piece[]> = {};
const NONE_REFUSED: Record<string, string> = {};

/** The state while no workspace is focused, and while one has not answered yet. */
const NOTHING_FOCUSED: WorkspaceState = {
  pieces: NO_PIECES,
  piecesRefused: NONE_REFUSED,
  reading: false,
};
const NOTHING_YET: WorkspaceState = { ...NOTHING_FOCUSED, reading: true };

function stateOf(answer: Answer): WorkspaceState {
  return {
    panels: answer.panels,
    repos: answer.repos,
    pieces: answer.pieces,
    piecesRefused: answer.piecesRefused,
    trouble: answer.panelsTrouble ?? answer.reposTrouble,
    // Reading until the answer that runs git has come back for THIS workspace.
    reading: !answer.read,
  };
}

/** `now`, with every part that says what `was` said taken from `was`: `was` itself when
 *  nothing moved. */
function kept(was: WorkspaceState, now: WorkspaceState): WorkspaceState {
  const next: WorkspaceState = {
    panels: keptPanels(was.panels, now.panels),
    repos: same(was.repos, now.repos) ? was.repos : now.repos,
    pieces: keptBy(was.pieces, now.pieces),
    piecesRefused: same(was.piecesRefused, now.piecesRefused)
      ? was.piecesRefused
      : now.piecesRefused,
    trouble: now.trouble,
    reading: now.reading,
  };
  const moved = (Object.keys(next) as (keyof WorkspaceState)[]).some(
    (key) => next[key] !== was[key],
  );
  return moved ? next : was;
}

function keptPanels(
  was: PanelsModel | undefined,
  now: PanelsModel | undefined,
): PanelsModel | undefined {
  if (was === undefined || now === undefined || was.workspace !== now.workspace) return now;
  const next = { ...now } as Record<string, unknown>;
  const before = was as unknown as Record<string, unknown>;
  let moved = Object.keys(before).length !== Object.keys(next).length;
  for (const key of Object.keys(next)) {
    if (same(before[key], next[key])) next[key] = before[key];
    else moved = true;
  }
  return moved ? (next as unknown as PanelsModel) : was;
}

function keptBy<T>(was: Record<string, T>, now: Record<string, T>): Record<string, T> {
  const next: Record<string, T> = {};
  let moved = Object.keys(was).length !== Object.keys(now).length;
  for (const [key, value] of Object.entries(now)) {
    const before = was[key];
    if (before !== undefined && same(before, value)) next[key] = before;
    else {
      next[key] = value;
      moved = true;
    }
  }
  return moved ? next : was;
}

/** Whether two answers say the same thing. They are plain data off IPC, so their JSON is what
 *  they say, and a few kilobytes compared per answer is nothing beside a window redrawn. */
function same(a: unknown, b: unknown): boolean {
  return a === b || JSON.stringify(a) === JSON.stringify(b);
}

/**
 * The store for `plane`, for as long as the view that holds it, with `workspace` focused, read
 * again when `again` moves and when the plane changes on disk.
 */
export function useWorkspacePanelsStore(
  plane: PlaneId,
  workspace: string | undefined,
  again = 0,
): WorkspacePanels {
  const store = useMemo(() => new WorkspacePanels(plane), [plane]);
  useEffect(() => store.follow(), [store]);
  useEffect(() => {
    store.focus(workspace, again);
    // Unmounted, or focused elsewhere: what is still on its way is not wanted.
    return () => store.drop();
  }, [again, store, workspace]);
  return store;
}

/**
 * What `workspace` is to a reader: the store's state when it is about that workspace, and the
 * nothing-yet state otherwise, so a reader drawn for a workspace the store has not caught up
 * with never draws another one's answer under its heading.
 */
function stateFor(held: Held, workspace: string | undefined): WorkspaceState {
  if (workspace === undefined) return NOTHING_FOCUSED;
  return held.workspace === workspace ? held.state : NOTHING_YET;
}

/**
 * The part of `workspace`'s state a reader draws: it renders again only when `equal` says
 * that part moved (`identically` by default).
 */
export function useWorkspacePanels<T>(
  store: WorkspacePanels,
  workspace: string | undefined,
  select: (state: WorkspaceState) => T,
  equal: (a: T, b: T) => boolean = Object.is,
): T {
  return useSyncExternalStoreWithSelector(
    store.subscribe,
    store.snapshot,
    store.snapshot,
    (held) => select(stateFor(held, workspace)),
    equal,
  );
}

/** A `Panels` field a reader draws from. */
export type PanelsField = keyof PanelsModel;

/**
 * Two states the same for a reader of `fields` of the panels: every other part compared by
 * identity, which [`WorkspacePanels`] keeps where an answer did not move it.
 *
 * **A reader that draws a field not named here draws it stale** until something it does name
 * moves, so the list is each reader's whole read, by name. Read the region before trimming it.
 */
export function sameFor(fields: readonly PanelsField[]) {
  return (a: WorkspaceState, b: WorkspaceState): boolean =>
    a.repos === b.repos &&
    a.pieces === b.pieces &&
    a.piecesRefused === b.piecesRefused &&
    a.trouble === b.trouble &&
    a.reading === b.reading &&
    (a.panels === b.panels ||
      (a.panels !== undefined &&
        b.panels !== undefined &&
        fields.every((field) => a.panels?.[field] === b.panels?.[field])));
}

/** The panels' fields the git side reads — the explorer, the Changes view and the cockpit:
 *  the clones, where they are, which are not cloned and which were refused. */
export const GIT_SIDE: readonly PanelsField[] = [
  "workspace",
  "repos",
  "paths",
  "absent",
  "refused",
];

/**
 * A region drawn from the store: `draw` is called with its part of the state, and only this
 * component renders again when that part moves. `draw` is the view's own closure, so whatever
 * else it reads is as fresh as the view's last render.
 */
export function WorkspaceRegion({
  store,
  workspace,
  equal,
  draw,
}: {
  store: WorkspacePanels;
  workspace: string | undefined;
  equal?: (a: WorkspaceState, b: WorkspaceState) => boolean;
  draw: (state: WorkspaceState) => ReactNode;
}): ReactNode {
  return draw(useWorkspacePanels(store, workspace, (state) => state, equal));
}
