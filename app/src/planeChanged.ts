// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { useOnShownAgain, useProjectShown } from "./projectShown";
import { listen } from "./here";

import type { MemoryScope, PlaneAnswer, PlaneChange, PlaneChanged, PlaneId } from "./bindings";

/**
 * The answer a reader of the plane holds, by its name (FD-10).
 *
 * **Which answer a change concerns is the core's question, never the window's**
 * (`purlis_core::planechange::answers`): the core reads the plane format's paths, so it is
 * the one place that knows a todo is part of the sidebar and a memory is not. `plane-changed`
 * carries the answers a batch concerns, and a reader here only names the one it holds.
 */
export type Interest = PlaneAnswer;

/** The sidebar (`plane_sidebar`). */
export const SIDEBAR: Interest = { answer: "sidebar" };

/** The instructions a chat read at its start, which mark a chat running on old ones
 *  (`chats_plane_updated`). */
export const INSTRUCTIONS: Interest = { answer: "instructions" };

/** The curation actions offered on each subject (`curation_offers`). */
export const CURATIONS: Interest = { answer: "curations" };

/** What a project has on, and the theme it draws. */
export const SETTINGS: Interest = { answer: "settings" };

/** The git standings: the alerts and the Saving rows. What auto-save did reaches these and
 *  no other reader (#933). */
export const GIT: Interest = { answer: "git" };

/** The plane root's panels (`plane_root_panels`). */
export const ROOT_PANELS: Interest = { answer: "rootPanels" };

/** The view tabs, which read any of the plane's stores. */
export const VIEWS: Interest = { answer: "views" };

/** One workspace's panels (`workspace_panels`). */
export function panelsOf(workspace: string): Interest {
  return { answer: "panels", workspace };
}

/**
 * Whether the answers the core said moved include the one a reader holds.
 *
 * **Not knowing concerns everyone.** `null` — or an event from a core too old to say — is the
 * core saying it cannot name what moved (a batch it could not place), and every reader reads
 * again, as each did before there were kinds. The core's `panels` with no workspace is every
 * workspace's: a persona or the project moved.
 */
export function concerns(
  answers: readonly PlaneAnswer[] | null | undefined,
  mine: Interest,
): boolean {
  if (answers === null || answers === undefined) return true;
  return answers.some((told) => {
    // Every workspace's panels moved, so the project's personas did: the plane root draws
    // them too (#1686).
    if (told.answer === "panels" && told.workspace === null && mine.answer === "rootPanels")
      return true;
    if (told.answer !== mine.answer) return false;
    if (told.answer !== "panels" || mine.answer !== "panels") return true;
    return told.workspace === null || told.workspace === mine.workspace;
  });
}

/**
 * **What one view tab shows, finer than the answer it is drawn from** (#1160 L3): a test of each
 * change the core placed. `VIEWS` is every store a view can read, so a todo closed asked every
 * open memory tab again; a tab that names what it shows is asked again for that alone.
 */
export type About = (change: PlaneChange) => boolean;

/**
 * Whether `change` is to the folder of workspace `name` itself (removed, renamed, its
 * `workspace.json`): what it holds is then read again too, so an item gone is shown gone.
 */
const workspaceFolder = (change: PlaneChange, name: string) =>
  change.kind === "workspace" && (change.workspace === null || change.workspace === name);

/** A memory store's tab — one memory, or the store's archive: its store's writes. */
export function memoryIn(scope: MemoryScope): About {
  if (scope.kind === "workspace")
    return (change) =>
      (change.kind === "memory" && change.workspace === scope.name) ||
      workspaceFolder(change, scope.name);
  // The shared store is `personas/_shared/memory/` (`planechange::Change::persona`).
  const owner = scope.kind === "persona" ? scope.name : "_shared";
  return (change) =>
    (change.kind === "memory" && change.persona === owner) ||
    (change.kind === "persona" && (change.persona === null || change.persona === owner));
}

/** A todo's tab: its workspace's todos, and its workspace's own folder. */
export function todosIn(workspace: string): About {
  return (change) =>
    (change.kind === "todos" && change.workspace === workspace) ||
    workspaceFolder(change, workspace);
}

/**
 * A session record's tab, by the record's plane-relative path (`sessions.sessionView`): its
 * place's `sessions/` — a workspace's, the plane root's — and its workspace's own folder. A
 * record kept under a persona follows that persona's folder, which is all the core says of it
 * (`planechange::classify`). `undefined`, so every view store, for a path it cannot place.
 */
export function sessionsAt(path: string): About | undefined {
  const parts = path.split("/");
  if (parts.length === 2 && parts[0] === "sessions")
    return (change) => change.kind === "sessions" && change.workspace === null;
  if (parts.length === 4 && parts[0] === "workspaces" && parts[2] === "sessions") {
    const name = parts[1];
    return (change) =>
      (change.kind === "sessions" && change.workspace === name) || workspaceFolder(change, name);
  }
  if (parts.length >= 3 && parts[0] === "personas") {
    const owner = parts[1];
    return (change) =>
      change.kind === "persona" && (change.persona === null || change.persona === owner);
  }
  return undefined;
}

/**
 * A persona's view (`panels::persona_view`): its memories, and what its facts are read from —
 * every persona's definition, because it may inherit from another one (its `Inherits` line),
 * and the project's files, which say the profiles it can start on.
 */
export function personaIn(name: string): About {
  return (change) =>
    (change.kind === "memory" && change.persona === name) ||
    change.kind === "persona" ||
    // Its skills, and any persona's it may inherit, or the project's (SK-29).
    (change.kind === "skills" && change.workspace === null) ||
    change.kind === "project";
}

/**
 * **What the Skills view lists** (#3266), for a chat in `workspace` or at the root (`undefined`):
 * the project's own files (its `skills/`, its settings), a workspace's, every persona's, which a
 * persona brings into every workspace, and each chat program's configuration. Not a todo, a
 * memory, a session record or a chat.
 */
export function skillsIn(workspace: string | undefined): About {
  return (change) =>
    change.kind === "project" ||
    change.kind === "harness" ||
    change.kind === "persona" ||
    // A skill of the project's or a persona's, or of this workspace (SK-29).
    (change.kind === "skills" &&
      (change.workspace === null || (workspace !== undefined && change.workspace === workspace))) ||
    (change.kind === "workspace" &&
      workspace !== undefined &&
      (change.workspace === null || change.workspace === workspace));
}

/**
 * **What any open chat's skill set is made of** (#4094), for the Skills chip on each chat's pane
 * (`useChatSkillSets`): {@link skillsIn} for every workspace at once, since the chats open may be
 * in any of them. A skill's folder moving, or a workspace's settings (its skills turned off or
 * pinned, in its `workspace.json`), moves a chat's budget as the Skills view's header shows it.
 */
export const skillSetsOfChats: About = (change) =>
  change.kind === "project" ||
  change.kind === "harness" ||
  change.kind === "persona" ||
  change.kind === "skills" ||
  change.kind === "workspace";

/**
 * Whether what the core said moved concerns a view tab that shows `about`, or any of the
 * views' stores with no `about`.
 *
 * **Not knowing still concerns everyone**: no answers, or no changes, is the core saying it
 * could not place a batch, and the tab reads again as every tab did.
 */
export function concernsAView(
  told: Pick<PlaneChanged, "answers" | "changes">,
  about: About | undefined,
): boolean {
  if (told.answers === null || told.answers === undefined) return true;
  if (!concerns(told.answers, VIEWS)) return false;
  if (about === undefined || told.changes === null || told.changes === undefined) return true;
  return told.changes.some(about);
}

/**
 * How many times the core has said one of `planes` changed on disk (charter-app#264).
 *
 * **A count, so it goes in a dependency array.** Every read of the plane that must follow the
 * disk — the workspace's panels, the sidebar, the alerts — lists it, and runs again exactly
 * when it moves. The core debounces (`app/src-tauri/src/planewatch.rs`), so one `git pull` is
 * one bump rather than ten.
 *
 * **Filtered on the plane, as `chat-moved` is.** The event is emitted on the app and a
 * process holds several projects, so a todo closed in one must not make another read itself
 * again.
 *
 * Listening starts at the mount, before any answer it would invalidate is asked for — the
 * reason `chatState.ts` gives for the same order. A window that cannot listen (a unit test,
 * a webview being torn down) is simply never bumped, and reads the plane when it is focused,
 * as it did before there was anything to listen to.
 *
 * **It moves only for a change that concerns the answer it names** ([`concerns`], FD-10), so
 * a panel reads again for what its answer is made of instead of for every write anywhere in
 * the plane — and which changes those are is the core's to say.
 *
 * **No interest follows nothing**, while still listening (#1160): a reader that holds its
 * answer only some of the time — the plane root's panels, while no workspace is in front — is
 * not counted, and so not drawn, for a change it would not read, and comes and goes without
 * listening again.
 */
export function usePlaneChanged(
  planes: readonly PlaneId[],
  interest: Interest | undefined,
): number {
  return useCounted(
    planes,
    interest,
    (told, mine) => mine !== undefined && concerns(told.answers, mine),
  );
}

/**
 * How many times the core has said one of `planes` changed in a way that concerns a view tab
 * showing `about` ([`concernsAView`], #1160 L3), or any of the views' stores with no `about`.
 */
export function useViewChanged(planes: readonly PlaneId[], about: About | undefined): number {
  return useCounted(planes, about, concernsAView);
}

/** The count both hooks above keep, of the events `wants` says concern what `mine` names. */
function useCounted<T>(
  planes: readonly PlaneId[],
  mine: T,
  wants: (told: PlaneChanged, mine: T) => boolean,
): number {
  const [count, setCount] = useState(0);
  // The list by value: a fresh array from the caller each render must not re-register.
  const holding = planes.join("\n");
  // What it follows is read when an event arrives, not listened under: focusing another
  // workspace changes it, and re-registering the listener then would be IPC per focus and a
  // gap in which a change could go unheard. Focusing reads the workspace anyway.
  const wanted = useRef({ mine, wants });
  useEffect(() => {
    wanted.current = { mine, wants };
  });

  // **A change heard while the caller's project is not in front is not counted**
  // (`projectShown.tsx`): a view nobody can see reads nothing again. **It is counted once as the
  // project comes back**, after it is painted, whether or not a change was heard: a view that
  // was built again on every switch read the disk again each time, and what it shows is still
  // owed that read. What listens outside any project's view, the window and each project's own
  // state, is counted at once as before.
  const shown = useProjectShown();
  const shownNow = useRef(shown);
  useEffect(() => {
    shownNow.current = shown;
  }, [shown]);
  useOnShownAgain(() => setCount((was) => was + 1));

  useEffect(
    () =>
      followTold(
        holding.split("\n"),
        (told) => wanted.current.wants(told, wanted.current.mine),
        () => {
          if (shownNow.current) setCount((was) => was + 1);
        },
      ),
    [holding],
  );

  return count;
}

/**
 * Calls `told` each time the core says one of `planes` changed in a way that concerns the
 * answer `interest` names now, until the returned stop is called: [`usePlaneChanged`] without
 * the component around it.
 *
 * **For a store that lives outside a view** (#1160): `workspacePanels.ts` reads the focused
 * workspace's panels again on it, so the view that holds the store is not drawn again just to
 * pass a count down. The interest is a getter, read when an event arrives, for the reason
 * `usePlaneChanged` gives.
 */
export function followPlaneChanged(
  planes: readonly PlaneId[],
  interest: () => Interest,
  told: () => void,
): () => void {
  return followTold(planes, (said) => concerns(said.answers, interest()), told);
}

function followTold(
  planes: readonly PlaneId[],
  wants: (told: PlaneChanged) => boolean,
  told: () => void,
): () => void {
  const mine = new Set(planes);
  let gone = false;
  let stop: (() => void) | undefined;
  void (async () => {
    try {
      const unlisten = await listen<PlaneChanged>("plane-changed", (event) => {
        if (gone || !mine.has(event.payload.plane)) return;
        if (wants(event.payload)) told();
      });
      if (gone) unlisten();
      else stop = unlisten;
    } catch {
      // No window to listen in. See `usePlaneChanged`.
    }
  })();
  return () => {
    gone = true;
    stop?.();
  };
}
