import { useEffect, useMemo, useRef, useState } from "react";
import {
  commands,
  type BranchFolder,
  type FilesChanged,
  type FolderEntry,
  type PlaneId,
} from "./bindings";
import { listen } from "./here";

/**
 * **The folders of branches the explorer has expanded, read one level at a time** (FM-1,
 * #1103).
 *
 * A branch is named, never given as a directory: a workspace, a repo and a piece, or no piece
 * for the repo's own folder (#948). A folder is its path inside the branch, `""` for its top.
 * What each holds is `purlis_core::files::tree`'s answer, through `branch_tree`.
 *
 * **What it costs.** One `branch_tree` per folder when it is expanded, and nothing for a
 * folder that is not: a branch of a hundred thousand files costs what its open folders hold.
 * Each expanded folder is watched, non-recursively (`files_watch`), and read again when the
 * core says it moved — an agent adding or removing a file — so the tree never needs a manual
 * refresh. A folder closed and opened again is read again, because it was not watched while
 * closed.
 *
 * **Watched first, then read** (#1427). A change is told only once its folder is watched, so a
 * folder read before the watch held could miss a file made in between and never hear of it: the
 * tree drew it without the file until something else moved there. A newly opened folder is read
 * once the core has answered the watch that names it, so whatever changed before that is in the
 * read and whatever changes after it is told — or after {@link WATCH_WAIT_MS}, so a watch that is
 * slow to answer delays the folder and never hides it. A folder read that way, before its watch
 * held, is read once more when the watch does answer: a file made between the first read and
 * the watch is told by nothing else, and the tree would go without it until something else
 * moved there. Finding a branch's folder for the watch waits on the bounded reader's child
 * (#1189), which a busy reader gate can hold past the bound.
 */

/** One folder of a branch, as the explorer names it. */
export type BranchFolderRef = {
  repo: string;
  /** The piece, or `null` for the repo's own folder. */
  piece: string | null;
  /** Its path inside the branch, `""` for the branch's top. */
  folder: string;
};

/** What a folder holds — its first entries and how many more — or the sentence the core
 *  refused it with. Absent while it is read. `unwatched` while the core is not watching it for
 *  changes (#1727): the reader paused its branch, or had no answer for it yet. */
export type FolderRead = {
  entries?: FolderEntry[];
  more?: number;
  trouble?: string;
  unwatched?: boolean;
};

/** A folder's key within one workspace. A repo and a piece cannot hold a `/` or a `:`
 *  (`worktree::path_for` refuses both), so the key splits back exactly. */
export function folderKey(ref: BranchFolderRef): string {
  return `${ref.repo}/${ref.piece ?? ""}:${ref.folder}`;
}

/**
 * What each of `open` holds, by {@link folderKey}: read when a folder first appears in `open`,
 * and again whenever the core says it moved.
 *
 * @param open The folders the explorer has expanded and draws, in this workspace.
 */
export function useBranchFolders(
  plane: PlaneId | undefined,
  workspace: string | undefined,
  open: readonly BranchFolderRef[],
): ReadonlyMap<string, FolderRead> {
  const [held, setHeld] = useState<{ workspace?: string; reads: Map<string, FolderRead> }>({
    reads: new Map(),
  });
  /** The keys read already, so a re-render asks for nothing and a newly opened one is read. */
  const asked = useRef<{ workspace?: string; keys: Set<string> }>({ keys: new Set() });
  /** The folders open now, by key: what a `files-changed` is matched against. */
  const byKey = useRef(new Map<string, BranchFolderRef>());
  // Joined with NUL, which no name can hold: a folder an agent named with a line break stays
  // one folder.
  const keys = open.map(folderKey).join("\0");

  /** The workspace focused now: an answer for another one is dropped, not merged. */
  const focused = useRef(workspace);
  const read = useRef<(ref: BranchFolderRef) => void>(() => {});
  useEffect(() => {
    focused.current = workspace;
    read.current = (ref) => {
      if (plane === undefined || workspace === undefined) return;
      const key = folderKey(ref);
      const told = (got: FolderRead) => {
        if (focused.current !== workspace) return;
        setHeld((was) => {
          const reads = new Map(was.workspace === workspace ? was.reads : []);
          reads.set(key, got);
          return { workspace, reads };
        });
      };
      void commands
        .branchTree(plane, workspace, ref.repo, ref.piece, ref.folder)
        .then((said) =>
          told(
            said.status === "error"
              ? { trouble: said.error }
              : { entries: said.data.entries, more: said.data.more },
          ),
        )
        .catch((err: unknown) => told({ trouble: String(err) }));
    };
  });

  /** What this tree has the core watch: its share of the window's set (`watchFor`). */
  const owner = useRef(Symbol("branch folders"));
  const watch = useRef((folders: BranchFolder[]) => watchFor(owner.current, folders));

  // The whole open set is what the core watches, and each folder newly open is read once the
  // watch naming it holds.
  useEffect(() => {
    if (plane === undefined || workspace === undefined) {
      void watch.current([]);
      return;
    }
    const refs = keys === "" ? [] : keys.split("\0").map(unkey);
    if (asked.current.workspace !== workspace) asked.current = { workspace, keys: new Set() };
    const now = new Set(refs.map(folderKey));
    byKey.current = new Map(refs.map((ref) => [folderKey(ref), ref]));
    const fresh = refs.filter((ref) => !asked.current.keys.has(folderKey(ref)));
    // A folder closed is forgotten, so opening it again reads it again.
    asked.current.keys = now;
    const held = watch.current(refs.map((ref) => ({ plane, workspace, ...ref })));
    if (fresh.length === 0) return;
    // Only what is still open: one closed meanwhile is read when it is opened again.
    const readFresh = () => {
      for (const ref of fresh) {
        if (byKey.current.has(folderKey(ref))) read.current(ref);
      }
    };
    void held.then((inTime) => {
      readFresh();
      // Read before its watch held: read again once it holds, so a change in between is drawn.
      if (!inTime) void answered().then(readFresh);
    });
  }, [keys, plane, workspace]);

  // Nothing watched once the explorer is gone.
  useEffect(() => {
    const unwatch = watch.current;
    return () => void unwatch([]);
  }, []);

  // A folder that moved on disk is read again. Listened under the plane alone: focusing another
  // workspace changes what is matched, read when an event arrives, not the listener.
  useEffect(() => {
    if (plane === undefined) return;
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<FilesChanged>("files-changed", (event) => {
          if (gone) return;
          for (const moved of event.payload.folders) {
            if (moved.plane !== plane || moved.workspace !== focused.current) continue;
            const ref = byKey.current.get(folderKey(moved));
            if (ref !== undefined) read.current(ref);
          }
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);

  // The folders the core could not watch for this window, by key (#1727): every `files_watch`
  // and every later watch of one of them says the whole list again.
  const [unwatched, setUnwatched] = useState<ReadonlySet<string>>(NONE);
  useEffect(() => {
    if (plane === undefined) return;
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<FilesChanged>(UNWATCHED, (event) => {
          if (gone) return;
          const keys = event.payload.folders
            .filter((one) => one.plane === plane)
            .map((one) => `${one.workspace}\0${folderKey(one)}`);
          setUnwatched(keys.length === 0 ? NONE : new Set(keys));
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);

  const reads = held.workspace === workspace ? held.reads : EMPTY;
  return useMemo(() => {
    if (unwatched.size === 0 || workspace === undefined) return reads;
    const marked = new Map(reads);
    for (const [key, read] of reads) {
      if (unwatched.has(`${workspace}\0${key}`)) marked.set(key, { ...read, unwatched: true });
    }
    return marked;
  }, [reads, unwatched, workspace]);
}

/** The event naming every folder of this window the core is not watching now (#1727): what
 *  `files-changed` carries, under its own name. */
const UNWATCHED = "files-unwatched";

const NONE: ReadonlySet<string> = new Set();

const EMPTY: ReadonlyMap<string, FolderRead> = new Map();

/** The most folders one `files_watch` takes (`purlis_core::files::WATCHED`). */
const WATCHED = 256;

/** Every tree's open folders in this window, by tree: the explorer's and each file tab's
 *  (FM-2). */
const watchedBy = new Map<symbol, BranchFolder[]>();

/** Whether the core is watching anything for this window, so a window that never opened a
 *  folder never asks it to watch nothing. */
let watchingAny = false;

/** The newest `files_watch` this window has sent, answered or not. */
let newestWatch: Promise<unknown> = Promise.resolve();

/**
 * Tells the core what this window watches now that `owner`'s open folders are `folders`, and
 * settles `true` once that is watched, or `false` when {@link WATCH_WAIT_MS} passed first.
 *
 * **The core keeps one set per window, and each call replaces it** (FM-1's D-6). Two trees in
 * one window — the explorer and a file tab — would each replace the other's, so the core is told
 * their union, once per folder, up to what one call takes.
 *
 * Settles once the newest set sent by then, from any tree, has been answered — the core has
 * watched it, or refused. Not just this call's: the core sets a window's newest set and passes
 * over an older one, so an older call can answer before the set that names the folder holds.
 */
function watchFor(owner: symbol, folders: BranchFolder[]): Promise<boolean> {
  if (folders.length === 0) watchedBy.delete(owner);
  else watchedBy.set(owner, folders);
  const union = new Map<string, BranchFolder>();
  for (const one of [...watchedBy.values()].flat()) {
    union.set(`${one.plane}\0${one.workspace}\0${folderKey(one)}`, one);
  }
  const all = [...union.values()].slice(0, WATCHED);
  if (all.length === 0 && !watchingAny) return newest();
  watchingAny = all.length > 0;
  newestWatch = commands.filesWatch(all).catch(() => undefined);
  return newest();
}

/**
 * How long a folder's first read waits for its watch, in milliseconds. The watch is what makes
 * the read complete, but a read that waits for good draws nothing: a `files_watch` the core
 * never answers, or newer ones sent faster than they are answered while folders are toggled,
 * would hold the folder back. Past this the folder is read anyway, and read once more when the
 * watch answers, so a change made between the two is not missed — as it was before #1427, and
 * again after the watch began finding its branch through the bounded reader's child (#1189),
 * whose answer on a busy machine can take longer than this.
 */
export const WATCH_WAIT_MS = 2_000;

/** Settles once the newest `files_watch` has answered, however many are sent meanwhile, or
 *  after {@link WATCH_WAIT_MS}, whichever is first: `true` when the watch answered in time. */
function newest(): Promise<boolean> {
  let bound: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<boolean>((go) => {
    bound = setTimeout(() => go(false), WATCH_WAIT_MS);
  });
  return Promise.race([answered().then(() => true), late]).finally(() => clearTimeout(bound));
}

/** Settles once the newest `files_watch` has answered, however many are sent meanwhile. */
async function answered(): Promise<void> {
  let waited: Promise<unknown>;
  do {
    waited = newestWatch;
    await waited;
  } while (waited !== newestWatch);
}

/** A key back into the folder it names. */
function unkey(key: string): BranchFolderRef {
  const slash = key.indexOf("/");
  const colon = key.indexOf(":", slash);
  const piece = key.slice(slash + 1, colon);
  return {
    repo: key.slice(0, slash),
    piece: piece === "" ? null : piece,
    folder: key.slice(colon + 1),
  };
}
