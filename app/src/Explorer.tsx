// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { LiveMark } from "./LiveDialog";
import { NamedNotReadWords, namedNotRead, namedNotReadSaid } from "./ExplorerRefused";
import { Notice } from "./Notice";
import { useEffect, useContext, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import {
  ArrowUpRight,
  ChevronRight,
  ChevronsDownUp,
  Files,
  FileSymlink,
  FileX,
  FolderGit2,
  Folders,
  GitBranch,
  ListFilter,
  LoaderCircle,
  TriangleAlert,
} from "lucide-react";
import type { FileScope, FolderEntry, FoundFile, OpenChat, Piece, PlaneId } from "./bindings";
import { FileIcon } from "./FileIcon";
import { useFileIcons } from "./projectTheme";
import { iconFor, type IconTheme } from "./theme/icons";
import {
  folderKey,
  useBranchFolders,
  type BranchFolderRef,
  type FolderRead,
} from "./branchFolders";
import {
  branchKey,
  indexed,
  useBranchStatus,
  type BranchRef,
  type Indexed,
  type Marked,
  type StatusRead,
} from "./branchStatus";
import { PLANE_ROOT, isPlaneRoot, type Place } from "./pieceViews";
import { catalogueMenu } from "./Menus";
import { NotClonedHere, notClonedRows, type Cloning } from "./NotCloned";
import { stopRepoClones } from "./repoClones";
import { WorktreeMark } from "./Worktree";
import { OUTSIDE_TITLE } from "./actions";
import type { BranchPath, Catalogued, FileOn, FileRef, Offer } from "./actions";
import { setSectionOpen, useClosedSections, type SectionId } from "./explorerSections";
import { keepTreeFolds, keptTreeFolds } from "./explorerFolds";
import type { WorkspaceState } from "./workspaceState";
import { Breadcrumb, CockpitHeader, focusStands, useAheadBehind } from "./Cockpit";
import { carriedReference, type Referenced } from "./references";
import { pickable, useFilePicks, type Pickable } from "./filePicks";
import { useLastRead } from "./editor/lastRead";
import { touchingIn, touchSaid, useTouching, type Touching } from "./touching";
import { Button, IconButton, ToggleButton } from "./ui/button/Button";
import { TextField } from "./ui/field/Field";
import {
  PanelSection as ViewSection,
  PanelSectionHead,
  PanelSectionTitle,
  PanelTitle,
} from "./ui/panel/Panel";
import { Count } from "./ui/badge/Badge";
import { HeadActions } from "./viewHead";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { Tree, TreeLine, TreePending, TreeRow, type Key, type PressEvent } from "./ui/tree/Tree";
import { List, ListRow, ListRowSection, type RowMenu } from "./ui/list/List";
import { RowBelow, RowLabel, RowLead } from "./ui/list/Row";
import { ScopeSwitch } from "./ScopeSwitch";
import { hitSaid, placeOf, useFileFind, type FilesFoundNow } from "./fileFind";
import { searchKeySaid } from "./searchKey";
import { onAMac } from "./tabKeys";

/**
 * The left region: the repo and worktree **explorer** (ADR 0038).
 *
 * **It replaces the workspace listing, it does not extend it.** What used to be here was
 * every workspace with its vision text and its chats — the axis ADR 0036 had just given the
 * strip above, answered a second time in more words. That duplication is what made the
 * operator ask what the left sidebar was for.
 *
 * What the left gets instead is the level the three strips do not reach. A workspace holds
 * several clones and several worktrees, and until now nothing in the window selected one.
 *
 * **Picking something here decides where the next chat starts**, which is what makes this a
 * selector rather than a second listing. `New tab` starts in the focused workspace's own
 * directory until a piece is picked, and in that piece afterwards. Nothing is started by
 * picking: the picker still asks which profile and which persona, and the core still decides
 * whether that directory can be started in — it writes the harness layer there or refuses
 * with a sentence naming what stopped it.
 *
 * **A clone is a heading, and it is picked from its menu rather than by a click.** A click on
 * the heading opens and closes it, which is what a heading does; the pick is `Start new chats
 * in <repo>` on the clone's context menu, beside `New tab in <repo>` (charter-app#174). Both
 * carry the path the core spelled (`Panels.paths`), so nothing here joins one together, and a
 * picked clone is marked the way a picked piece is.
 *
 * **Every row that is a place has a context menu, and each is the catalogue filtered to that
 * place** (charter-app#174). A piece's is merge above the line and remove below it; a clone's
 * is the two rows above and nothing below, because nothing in this window writes to a clone.
 * Nothing here says what those rows mean — `Menus.tsx` draws whatever `actions.ts` has, and
 * Shift+F10 or the menu key opens the same menu on the row the arrows are on.
 *
 * **It is drawn as a tree, and the lines are drawn by the rows rather than by the lists.**
 * The nesting was always here — workspace, clone, piece, chat — and nothing said so: four
 * levels of `padding-inline-start` and no line to follow, which is what the operator meant by
 * *"trees are not looking like tree"*. Each row draws its own vertical segment and its own
 * elbow (now the tree family's guides, `ui/tree/tree.css`), so the last row's segment simply
 * stops at the elbow. The usual trick — a line on the list, masked at the bottom by a rectangle in
 * the background colour — cannot be used here, because **a region moves** (ADR 0038): the
 * explorer put in the bottom slot sits on `surface.deep`, and a mask painted in `surface.base`
 * would be a visible block. A row that draws its own line has no background to know.
 *
 * **A row never folds, and the region scrolls sideways instead.** The operator's words:
 * *"all trees components texts should not be breakable to new line — it should be horizontal
 * scrollable."* A row names one thing, and a name broken across two lines takes the tree with
 * it — the guides, the indent and the eye all read down a column of first lines. The rule is
 * in `App.css` beside the guides, because it had to be written without touching the padding
 * and the font size those elbows are tuned to. Sentences charter says about a FAILURE still
 * wrap; they are not rows. Neither half of this can be asserted in jsdom, which lays nothing
 * out: `workspace-explorer.e2e.ts` measures the rows and the elbows in the real WebView.
 *
 * **It is a tree, the WAI-ARIA "Tree View" pattern, whole** (charter-app#238), and since #2911
 * each of its trees is the tree family's (`ui/tree`, on React Aria's tree):
 *
 * - **Every row a row of a `treegrid`** with its `aria-level` and its place among its siblings
 *   (`aria-posinset` / `aria-setsize`), from the rows the tree is given.
 * - **`aria-expanded` on every row with children**, as the pattern asks of a parent: a clone or
 *   a folder says whether it is open, the workspace's own row `true`, as a parent that is always
 *   open. A leaf says nothing.
 * - **Right** opens a closed clone or folder, or moves to a row's first child. **Left** closes an
 *   open one, or moves to the row's parent. The fold is the same state a press on the row
 *   changes.
 * - **Up, Down, Home and End**, and ONE Tab stop for each tree: in *Repos and branches* the stop
 *   at rest is the current row — the picked worktree, or the workspace itself (`stopAt`).
 * - **Type-ahead**: typing moves to the next row whose name starts with it.
 * - **Enter** does what a press does: it picks a worktree, opens a file, and on a clone opens
 *   or closes it.
 *
 * Left and Right are taken from the region's sideways scroll while a row has the keyboard. A
 * focused row is scrolled into view by the engine, so nothing the keyboard can reach is lost.
 *
 * **A branch expands into its files** (FM-1, #1103). Under each branch, and under each repo for
 * the repo's own folder (#948), a *Files* row folds open onto that folder's first level, and
 * each folder onto the next — read only when it is opened (`branchFolders.ts`), and read again
 * when an agent adds or removes a file in it. They are rows of the same tree: the same levels,
 * the same arrows, Home and End, and Enter on a file opens it in its file tab. Folders come
 * first, in the order a person reads names. What git ignores is hidden until *Show ignored
 * files* is pressed, and then drawn dimmed and never opened; a file charter will not open — a
 * link out of the branch, git's own folder, a FIFO — is drawn with the reason. The *Files* row
 * is a child of the branch rather than the branch row folding itself, because a click on a
 * branch picks where the next chat starts.
 *
 * **What a branch changed is marked on it** (FM-4, #1107): each file it changed, added,
 * deleted or renamed against the branch it was cut from, committed or not, and each folder
 * holding such a file with how many, read again as agents write (`branchStatus.ts`). A file
 * the branch deleted is drawn where it was, and does not open. A path it changed that purlis
 * names and does not read (a `.git` folder on its path, a name that is not a plain path) is a
 * line of text under the branch's top, with its mark and why, never a row (`ExplorerRefused`,
 * #704). *Changed only* collapses every
 * open branch's files to what it changed, every folder of it open. The *Filter files* box
 * narrows the file rows drawn to names holding what is typed, keeping the folders on the way to
 * one; Esc clears it.
 *
 * **A file or folder row has a menu that changes nothing** (FM-10, #1113): copy its path,
 * relative or absolute, reveal it in the file manager, your editor for a file and a shell tab
 * for a folder (`actions.fileRows`). Never create, rename, move or delete (ADR 0081, V86 F8):
 * charter does not race an agent writing the same tree. Each row names the branch and the path
 * inside it; the core places it (`files::place`), so no path the window joined leaves it.
 *
 * **Focused on one branch, it is that branch's cockpit** (FM-5, #1108; V86 F2): a breadcrumb
 * back out, the branch's state — how far it is from its base, how many files it changed, and
 * Merge and Done — then its files, as rows of a tree of their own
 * with the same keys. Esc, or the breadcrumb, steps back out to the whole workspace, and the
 * keyboard lands on the branch's row. Which branch is the window's (`PlaneView.tsx`), and it is
 * remembered with the window's views.
 *
 * **It draws no chats** (#1673, B-12). It drew a workspace's chats where each worked (#1490),
 * and the Chats view drew them again; with the two now views of one side, one at a time, the
 * Chats view is the one place a chat is listed and this is the place: workspace, repos,
 * branches and files. A branch still answers "is anything already running here" in the chats'
 * own words, in the Chats view, and by the live marks below.
 *
 * **It is in two sections, each folding on its heading** (#1677, spec #1671 B-12), an
 * editor's explorer: *Repos and branches*, the focused workspace's own tree; and *Files*, of
 * where the next chat starts — the picked branch or repo, or each repo's own folder while the
 * workspace itself is picked. Each section's tree is its own Tab stop. A folded section is
 * hidden and stays mounted, so its folds and the place the keyboard was in it come back with it;
 * which ones are folded is kept in the layout file (`explorerSections.ts`).
 *
 * **It lists no workspaces** (qw358). It had a *Workspaces* section, the strip's list a second
 * time, and the operator called it what it was: redundant. The strip is where a workspace is
 * selected (ADR 0036), with its live mark and its menu on its tab; this is the files of the one
 * selected there, and it follows the strip. The *Files* heading names whose files they are.
 *
 * **On the plane root's tab, *Files* is the whole project's** (issue 1916; rulings G3, F1 and F2
 * of 2026-10-13): the project's own folder as the tree's first level, read one folder at a time
 * through the window's own command (`plane_tree`, `branchFolders.ts`). The core leaves out what
 * purlis keeps to itself — the vaults, their keys, its own records — so no row is drawn for
 * them. A workspace's repo is one row with the workspace's mark, and pressing it selects that
 * workspace on the strip: its files are that tab's, and the row never opens here. The rows are
 * read and nothing more: no menu, no drag onto a chat, no change marks — each of those names a
 * branch, which the plane root is not.
 *
 * **In the window it is two views** (#1737, spec #1736 P-2): the Explorer view, *Files*, in the
 * left sidebar, and the Repos view, *Repos and branches* alone, in the left half of the Panel
 * ({@link ExplorerPart}). Both are this component, drawn twice with the window's one pick, so
 * picking a repo or branch in Repos is what Files lists, exactly as when they were one. Each
 * keeps the rows of its own trees: Repos the clones folded, Explorer the folders opened.
 *
 * **The Explorer view opens straight to the files** (#3343, the sidebar-views prototype's Q2 A,
 * "Repos as folders"). *Files* is no fold: the view's title is the workspace (its region's head,
 * #3330) and the tree is the view, one scroll on the panel family's view frame (#3340). One repo
 * is its tree, open at once, with the repo and its branch beside the title; several are a
 * collapsed row each (its name, how many files it changed, its branch) that opens in place. The
 * filter stays in the view's top: it finds files by name (`find_files`, ⌘P's) in the selected
 * repo or in all of them, Search's *Branch* and *Workspace*, and *Search contents* opens Search
 * on the same scope.
 *
 * **What a chat is touching right now is marked live** (FM-6, #1109; V86 F6): a file a chat's
 * tool reads or edits, and every folder above it up to the branch's *Files* row, carry a dot
 * naming the chat on hover, which fades a few seconds after the chat goes quiet on it
 * (`touching.ts`). The core confined the path to the chat's own folder, and it was written
 * nowhere (D-86a).
 */
export function Explorer({
  plane,
  workspace,
  live = false,
  state,
  chats,
  spot,
  onPick,
  offers,
  onPress,
  onOpenFile,
  focus,
  onFocus,
  cloning,
  onReadAgain,
  part = "whole",
  onAddToChat,
  onGoToWorkspace,
  reveal,
  onSearchContents,
  onSearchKey,
}: {
  /** The project, for reading a branch's folders. Without one no folder is read. */
  plane?: PlaneId;
  /** The workspace selected on the strip, whose files and repos these are, or nothing when the
   *  strip is on the plane root. */
  workspace: string | undefined;
  /** Whether that workspace is LIVE (charter-app#301): its own row in *Repos and branches*
   *  carries the mark. */
  live?: boolean;
  state: WorkspaceState;
  /** The chats working in this workspace, for what each is touching right now. They are not
   *  rows here: the Chats view lists them (#1673). */
  chats: readonly OpenChat[];
  /** The piece picked, or nothing for the workspace's own directory. */
  spot: Spot | undefined;
  onPick: (spot: Spot | undefined) => void;
  /** The catalogue by id, which is what a piece row's menu is drawn out of. */
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  /** A file of a branch was opened from the tree: its file tab comes forward. */
  onOpenFile?: (place: Place, path: string) => void;
  /** The branch the explorer is focused on, its cockpit (FM-5), or nothing for the whole
   *  workspace. Drawn only while the workspace lists it. */
  focus?: Place;
  /** Focus on a branch, or step back out with nothing. */
  onFocus?: (focus: Place | undefined) => void;
  /** What this window is cloning into the workspace, so a repo that is not cloned here shows
   *  its clone under way, or why it failed (#1215). */
  cloning?: Cloning;
  /** Asks the workspace's reads again: the way out of a refused one (NO-4). It is this
   *  region's to offer, for the bottom bar's lines too, since that region is not pressed. */
  onReadAgain: () => void;
  /** Which of it this draws (#1737): the whole, or the window's Explorer view or Repos view. */
  part?: ExplorerPart;
  /** **Several file and folder rows, picked, to a chat at once** (#1151): their references, in
   *  the order the rows are drawn, all of one branch. No way to hand them without it. */
  onAddToChat?: (refs: Referenced[]) => void;
  /** A workspace's repo was pressed in the plane root's files (issue 1916, ruling F2): that
   *  workspace's tab is selected on the strip. */
  onGoToWorkspace?: (workspace: string) => void;
  /**
   * **A folder to bring into view** (#3266, #3308, the Skills view's Show in Explorer): one of
   * the project's own files (`repo` `""`), at the plane root, or one inside a clone (`repo`) of
   * the workspace shown, by its path in the clone. Each time `asked` moves, the folders above it
   * are opened and the keyboard lands on its row once it is drawn; in a workspace whose *Files*
   * are of another branch, the workspace itself is picked first, which lists every clone's own,
   * or, for a folder in a branch folder (#4326), that branch folder.
   * A folder of the other kind waits until Explorer is where it is.
   */
  reveal?: { repo: string; piece?: string; folder: string; asked: number };
  /**
   * **Search contents** (#3343): the Search view, opened on the filter's scope — `branch`, the
   * selected repo's own folder or the picked branch (`at`), or `workspace`, all its repos — with
   * what the filter holds. No link without it.
   */
  onSearchContents?: (kind: "branch" | "workspace", at: Place, query: string) => void;
  /**
   * **⌘⇧F does what the Search contents link says** (#3343): while the keyboard is in the files
   * view (`inside`), the window's key runs `run`, Search on the filter's scope with its words,
   * instead of Search on the window's focus. Handed over after each render, and taken back.
   */
  onSearchKey?: (here: SearchKeyHere | undefined) => void;
}) {
  const has = (section: SectionId) => PART_SECTIONS[part].includes(section);
  /** Whether this draws the plane root's files (issue 1916): the strip is on the plane root,
   *  and this is a view with a *Files* section, of a project it can read. */
  const atRoot = workspace === undefined && plane !== undefined && has("files");
  /** Whose folders the file trees are keyed and read by: the workspace, or the plane root's
   *  empty name (`pieceViews.PLANE_ROOT`), which no workspace has. */
  const filesWorkspace = workspace ?? (atRoot ? PLANE_ROOT.workspace : undefined);
  /** Whether this draws the trees whose rows it keeps: the clones', the files'. */
  const keepsClones = has("repos");
  const keepsFiles = has("files");
  /** The rows folded and opened as this project's Explorer was left, on this machine (B-11,
   *  #1686): read once, as the view is first drawn, and kept on every change. */
  const [kept] = useState(() => keptTreeFolds(plane));
  /** The clones the operator folded, by workspace and name: a row inside one is not drawn, so
   *  it cannot be where the keyboard comes back in. */
  const [folded, setFolded] = useState<ReadonlySet<string>>(kept.folded);
  /** The folders of branches the operator opened, by {@link fileFold}: closed until opened. */
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(kept.opened);
  /** Whether what git ignores is drawn, dimmed. */
  const [showIgnored, setShowIgnored] = useState(false);
  /** Whether a branch's files are collapsed to what it changed (FM-4). */
  const [changedOnly, setChangedOnly] = useState(false);
  /** What the file rows are narrowed to: names holding it, any case. */
  const [filter, setFilter] = useState("");
  /** The cockpit's *Files* rows the operator closed: open until they do (FM-5). */
  const [shut, setShut] = useState<ReadonlySet<string>>(kept.shut);
  // Written only when they differ from what is kept (`projectViews.keepFacet`), so the first
  // draw writes nothing.
  // Only what this part draws: Repos and the Explorer view are both drawn, and each would
  // otherwise write the other's rows back as they were when it was first drawn.
  useEffect(
    () =>
      keepTreeFolds(plane, {
        ...(keepsClones ? { folded } : {}),
        ...(keepsFiles ? { opened: expanded, shut } : {}),
      }),
    [plane, folded, expanded, shut, keepsClones, keepsFiles],
  );
  // The cockpit is Files' shape: the Repos view draws its tree whatever Explorer is focused on.
  const cockpit = has("files") ? cockpitOf(workspace, state, focus) : undefined;
  /** The workspace's repos: one is its tree, open at once; several a row each (#3343). */
  const repos = workspace === undefined ? [] : (state.panels?.repos ?? []);
  /** Where the *Files* tree is of: the picked branch or repo, or the workspace's one repo; or
   *  nothing for a workspace of several, whose repos' own folders it lists. */
  const filesTop: BranchFolderRef | undefined = atRoot
    ? { repo: PLANE_ROOT.repo, piece: PLANE_ROOT.piece, folder: "" }
    : cockpit !== undefined
      ? { ...cockpit.ref, folder: "" }
      : spot !== undefined
        ? { repo: spot.repo, piece: spot.piece ?? null, folder: "" }
        : repos.length === 1
          ? { repo: repos[0], piece: null, folder: "" }
          : undefined;
  /** The repo row pressed last, which the filter's *Selected* scope is of (#3343). */
  const [chosenRepo, setChosenRepo] = useState<string>();
  /** Whether the filter finds in all the workspace's repos rather than the selected one. */
  const [allRepos, setAllRepos] = useState(false);
  /** Whether the filter finds files by name (#3343), as it does on a workspace's own Files; on a
   *  branch or repo picked in Repos, in the cockpit and at the plane root it narrows the tree's
   *  rows, with *Changed only* and *Show ignored files* beside it. */
  const finds =
    has("files") && workspace !== undefined && cockpit === undefined && spot === undefined;
  const topFold =
    filesTop === undefined || filesWorkspace === undefined
      ? undefined
      : fileFold(filesWorkspace, filesTop);
  // The top of the files drawn is always open: the section's heading, or the cockpit's *Files*
  // row until the operator closes it, is what stands for it.
  const expandedNow =
    topFold !== undefined && !shut.has(topFold) && !expanded.has(topFold)
      ? new Set([...expanded, topFold])
      : expanded;
  const tops: BranchFolderRef[] = !has("files")
    ? // No files drawn, so none read: the Repos view asks git nothing of a folder.
      []
    : filesTop !== undefined
      ? [filesTop]
      : (state.panels?.repos ?? []).map((repo) => ({ repo, piece: null, folder: "" }));
  const open = openFolders(filesWorkspace, tops, expandedNow);
  // Nothing of the plane root is compared (issue 1916), so no status is read for it.
  const branches: BranchRef[] = atRoot
    ? []
    : open.filter((ref) => ref.folder === "").map(({ repo, piece }) => ({ repo, piece }));
  // The cockpit's branch is read whether or not its files are open: its header counts them.
  if (cockpit !== undefined && !branches.some((one) => branchKey(one) === branchKey(cockpit.ref)))
    branches.push(cockpit.ref);
  // A closed repo's row says how many files it changed (#3343): each repo's is read, open or not.
  if (has("files") && filesTop === undefined)
    for (const repo of repos)
      if (!branches.some((one) => one.repo === repo && one.piece === null))
        branches.push({ repo, piece: null });
  const statuses = useBranchStatus(plane, workspace, branches);
  const cockpitStatus = cockpit === undefined ? undefined : statuses.get(branchKey(cockpit.ref));
  const apart = useAheadBehind(plane, workspace, cockpit?.ref, cockpitStatus);
  // Indexed once per answer, not per folder per keystroke: a branch of 10,000 changes is walked
  // when its status arrives, and each folder after that is one lookup.
  const indexes = useMemo(
    () =>
      new Map(
        [...statuses].flatMap(([key, read]) =>
          // A status with nothing in it indexes nothing. The core always answers one; the check is
          // for a test's mocked core that answers every command it was not told about with `null`
          // (`theme/chrome.test.tsx`), which the closed repo rows' reads (#3343) now reach.
          read.status === undefined || read.status === null
            ? []
            : [[key, indexed(read.status)] as const],
        ),
      ),
    [statuses],
  );
  const reads = useBranchFolders(plane, filesWorkspace, open);
  const touches = useTouching(plane);
  // What the chats are touching in each branch read here: at most a few hundred paths over the
  // open branches, so worked out each render rather than remembered.
  const touching = new Map<string, Touching>();
  for (const ref of touches.length === 0 ? [] : branches) {
    const folder =
      ref.piece === null
        ? state.panels?.paths[ref.repo]
        : state.pieces[ref.repo]?.find((one) => one.piece === ref.piece)?.path;
    const marks = touchingIn(touches, chats, folder);
    if (marks.size > 0) touching.set(branchKey(ref), marks);
  }
  const icons = useFileIcons(plane, workspace);
  /** The file rows picked to hand a chat at once (#1151): let go when the files drawn are of
   *  somewhere else. */
  const picks = useFilePicks(
    JSON.stringify([workspace, filesTop?.repo, filesTop?.piece, cockpit !== undefined]),
  );
  const files: FilesOf = {
    workspace: filesWorkspace ?? "",
    expanded: expandedNow,
    reads,
    showIgnored,
    statuses,
    indexes,
    changedOnly,
    // Where the filter finds by name, the tree is not narrowed: the files found are drawn instead.
    filter: finds ? "" : filter.trim().toLocaleLowerCase(),
    levels: new Map(),
    touching,
  };
  const closed = useClosedSections(plane);
  /** Whether a section is folded on its heading: never the Repos view's, which has none, though
   *  a file from before #1737 says *Repos and branches* was folded; and never *Files*, which is
   *  no fold since #3343, though a file from before says it was. */
  const folds = (section: SectionId) =>
    section === "repos" && part !== "repos" && closed.has(section);
  /** The branch the filter's *Selected* scope is of (#3343): the tree's own, else the repo row
   *  pressed last, else the first repo. */
  const selected: { repo: string; piece: string | null } | undefined =
    filesTop !== undefined && !atRoot
      ? { repo: filesTop.repo, piece: filesTop.piece }
      : chosenRepo !== undefined && repos.includes(chosenRepo)
        ? { repo: chosenRepo, piece: null }
        : repos.length > 0
          ? { repo: repos[0], piece: null }
          : undefined;
  /** Whether the filter finds in all the repos: only offered where there are several. */
  const wide = repos.length > 1 && allRepos;
  const findIn: FileScope | undefined =
    plane === undefined || workspace === undefined || selected === undefined
      ? undefined
      : wide
        ? { kind: "workspace", plane, workspace, near: { workspace, ...selected } }
        : { kind: "branch", plane, workspace, ...selected };
  const found = useFileFind(finds && filter.trim() !== "", findIn, filter);
  /** The rows of each section's tree, and of the cockpit's: ids never repeat across them, so
   *  one lookup answers for every row. A folded section draws none of its rows. */
  const shownIn = (rows: Row[], section: SectionId) =>
    folds(section) ? rows.map((row) => ({ ...row, drawn: false })) : rows;
  const fileRows =
    workspace === undefined
      ? atRoot
        ? shownIn(filesTreeOf(PLANE_ROOT.workspace, tops, true, files), "files")
        : []
      : cockpit !== undefined
        ? cockpitTreeOf(workspace, cockpit, files)
        : shownIn(filesTreeOf(workspace, tops, filesTop !== undefined, files), "files");
  const drawnOf = (rows: Row[]) => rows.filter((row) => row.drawn);
  const picked =
    cockpit !== undefined
      ? undefined
      : spot === undefined
        ? ROOT
        : spot.piece === undefined
          ? cloneRow(spot.repo)
          : pieceRow(spot.repo, spot.piece);
  /** What a file tree hands the tree family: the folders open in it, and a fold it asks for. */
  const fileFolds = {
    expandedKeys: openFileRows(fileRows),
    onExpandedChange: foldFileRows(fileRows, (key, open) => fold(key, open)),
  };

  /** Opens or closes a clone or a branch's folder: the one fold state of each, whether a click
   *  or a key asked. A clone is open until folded; a folder is closed until opened. */
  const fold = (key: string, open: boolean) => {
    // A repo of several opened or closed, by a press, a twist or an arrow, is the one the
    // filter's *Selected* scope is of (#3343), as every open selects in the prototype.
    if (filesTop === undefined && workspace !== undefined) {
      const repo = repos.find(
        (one) => key === fileFold(workspace, { repo: one, piece: null, folder: "" }),
      );
      if (repo !== undefined) setChosenRepo(repo);
    }
    if (key === topFold)
      setShut((was) => {
        if (open !== was.has(key)) return was;
        const now = new Set(was);
        if (open) now.delete(key);
        else now.add(key);
        return now;
      });
    foldHeld(key, open);
  };
  const foldHeld = (key: string, open: boolean) =>
    key.startsWith(FILE_FOLD)
      ? setExpanded((was) => {
          if (open === was.has(key)) return was;
          const now = new Set(was);
          if (open) now.add(key);
          else now.delete(key);
          return now;
        })
      : setFolded((was) => {
          if (open === !was.has(key)) return was;
          const now = new Set(was);
          if (open) now.delete(key);
          else now.add(key);
          return now;
        });

  // Going in or coming back out of the cockpit moves the
  // keyboard, when it was in the explorer or nowhere, to where the explorer now is: the
  // cockpit's first row, or the branch's own row in the whole workspace.
  const cockpitKey = cockpit === undefined ? undefined : branchKey(cockpit.ref);
  const was = useRef(cockpitKey);
  const navRef = useRef<HTMLElement>(null);
  useEffect(() => {
    if (workspace === undefined) return;
    const left = was.current;
    was.current = cockpitKey;
    if (left === cockpitKey) return;
    const nav = navRef.current;
    const active = document.activeElement;
    if (nav === null || (active !== document.body && active !== null && !nav.contains(active)))
      return;
    const wanted =
      cockpit !== undefined
        ? drawnOf(fileRows)[0]?.id
        : left === undefined
          ? undefined
          : pieceRow(left.slice(0, left.indexOf("/")), left.slice(left.indexOf("/") + 1));
    // The tree family draws a row a moment after it knows of it: looked for a few frames.
    let tries = 0;
    let soon = 0;
    const land = () => {
      const row = [...nav.querySelectorAll<HTMLElement>("[data-row]")].find(
        (el) => el.dataset.row === wanted,
      );
      if (row !== undefined) row.focus();
      else if (wanted !== undefined && (tries += 1) < 20) soon = window.setTimeout(land, 16);
    };
    land();
    return () => window.clearTimeout(soon);
    // Only when the cockpit comes or goes; what it draws is read as it is then.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cockpitKey, workspace]);

  // **Show in Explorer** (#3266, #3308): on each ask, the folders above the one asked for are
  // opened (as React has state follow a prop, while drawing), then its row is given the keyboard
  // as soon as it is drawn: a folder is read as it opens. A clone's folder is asked of a
  // workspace, a project's own of the plane root.
  const [revealSeen, setRevealSeen] = useState(reveal?.asked);
  const [revealing, setRevealing] = useState<{
    row: string;
    repo: string;
    piece: string | null;
    workspace: string;
  }>();
  /** The ask whose row has had the keyboard: each ask lands once. */
  const revealed = useRef(reveal?.asked);
  /** The ask Files was moved to the workspace itself for: once each, so a row the ask never
   *  reaches (filtered out, gone) never holds the person's next pick back. */
  const movedFor = useRef(reveal?.asked);
  const revealHere =
    reveal !== undefined &&
    filesWorkspace !== undefined &&
    has("files") &&
    (reveal.repo === "") === atRoot;
  if (reveal !== undefined && revealHere && reveal.asked !== revealSeen) {
    setRevealSeen(reveal.asked);
    const parts = reveal.folder.split("/").filter((part) => part !== "");
    const top = atRoot
      ? { repo: PLANE_ROOT.repo, piece: PLANE_ROOT.piece }
      : { repo: reveal.repo, piece: reveal.piece ?? null };
    // A clone's own folder is one of several tops in a workspace, each closed until opened.
    const above = [
      ...(atRoot ? [] : [""]),
      ...parts.slice(0, -1).map((_, at) => parts.slice(0, at + 1).join("/")),
    ];
    setExpanded((was) => {
      const now = new Set(was);
      for (const folder of above) now.add(fileFold(filesWorkspace, { ...top, folder }));
      return now;
    });
    // A picked branch's filter narrows its tree, which would hide the row asked for (#4326).
    if (reveal.piece !== undefined) setFilter("");
    setRevealing({
      row: fileRow({ ...top, folder: parts.join("/") }),
      repo: top.repo,
      piece: top.piece,
      workspace: filesWorkspace,
    });
  }
  useEffect(() => {
    const wanted = revealing;
    if (wanted === undefined || revealed.current === revealSeen) return;
    // An ask of another workspace's (or the root's) folder, left behind by a switch, lapses.
    if (wanted.workspace !== filesWorkspace) {
      revealed.current = revealSeen;
      return;
    }
    // A folded *Files* section draws none of its rows: it is opened first.
    if (closed.has("files")) {
      setSectionOpen("files", true, plane);
      return;
    }
    // *Files* of another branch than the one asked for: the workspace itself lists a clone's own
    // folder, and only its own pick lists a branch folder (#4326).
    if (
      !atRoot &&
      (filesTop === undefined
        ? wanted.piece !== null
        : filesTop.repo !== wanted.repo || filesTop.piece !== wanted.piece)
    ) {
      if (movedFor.current === revealSeen) {
        // Moved once already, and the person went elsewhere: the ask is theirs to drop.
        revealed.current = revealSeen;
        return;
      }
      if (wanted.piece === null) {
        movedFor.current = revealSeen;
        if (cockpit !== undefined) onFocus?.(undefined);
        else onPick(undefined);
        return;
      }
      if (cockpit !== undefined) {
        movedFor.current = revealSeen;
        onFocus?.({ workspace: wanted.workspace, repo: wanted.repo, piece: wanted.piece });
        return;
      }
      // A pick holds the branch folder's path, where the next chat starts: it waits for the
      // clone's branch folders to be read, and a branch folder that is gone lapses the ask.
      const pieces = state.pieces[wanted.repo];
      if (pieces === undefined) return;
      const path = pieces.find((one) => one.piece === wanted.piece)?.path;
      if (path === undefined) {
        revealed.current = revealSeen;
        return;
      }
      movedFor.current = revealSeen;
      onPick({ repo: wanted.repo, piece: wanted.piece, path });
      return;
    }
    const nav = navRef.current;
    if (nav === null) return;
    /** Gives the row the keyboard once it is drawn: whether it was. */
    const land = (): boolean => {
      const row = [...nav.querySelectorAll<HTMLElement>("[data-row]")].find(
        (el) => el.dataset.row === wanted.row,
      );
      if (row === undefined) return false;
      revealed.current = revealSeen;
      row.scrollIntoView?.({ block: "nearest" });
      row.focus();
      return true;
    };
    if (land()) return;
    // The tree family draws a folder's rows in a commit of its own, after this view's, once
    // the folder is read: the row is waited for in the DOM, not on this view's next draw.
    const drawn = new MutationObserver(() => {
      if (land()) drawn.disconnect();
    });
    drawn.observe(nav, { childList: true, subtree: true });
    return () => drawn.disconnect();
  });

  // ⌘⇧F in the files view searches the filter's scope with its words (#3343): what the link
  // under the filter does, kept current for the window's key.
  useEffect(() => {
    if (onSearchKey === undefined) return;
    const run =
      onSearchContents !== undefined &&
      has("files") &&
      cockpit === undefined &&
      selected !== undefined &&
      workspace !== undefined
        ? () =>
            onSearchContents(
              wide ? "workspace" : "branch",
              { workspace, ...selected },
              filter.trim(),
            )
        : undefined;
    onSearchKey(
      run === undefined
        ? undefined
        : {
            inside: navRef.current?.querySelector<HTMLElement>('[data-section="files"]') ?? null,
            run,
          },
    );
    return () => onSearchKey(undefined);
  });

  /** Steps back out of the cockpit to the whole workspace. */
  const leave = () => onFocus?.(undefined);

  /** The `nav` each shape is drawn in, named for the view it is. */
  const named = PART_NAMES[part];

  if (workspace === undefined) {
    const rootLevel = filesTop === undefined ? undefined : levelOf(files, filesTop);
    /** The plane root's rows: read, and nothing more. No menu, no picks and no drag, each of
     *  which names a branch (issue 1916). */
    const rootRows: FileRows = {
      plane,
      place: PLANE_ROOT,
      files,
      fold,
      onOpenFile,
      icons,
      onGoToWorkspace,
    };
    return (
      // The same `nav` as the full explorer's below, so that it is the SAME element when a
      // workspace arrives: everything holding the old one — a scenario, a screen reader's place —
      // would otherwise lose it.
      <nav
        ref={navRef}
        className={has("files") ? "explorer explorer-view ui-text-body" : "explorer ui-text-body"}
        aria-label={named.label}
        data-testid={named.testid}
      >
        {/* The plane root is not a workspace: it has no repos. Its files are the whole
            project's (issue 1916), and the view's title says where the strip is (#3343). */}
        {has("files") ? (
          <ViewSection
            view
            data-section="files"
            top={
              <>
                <ViewHeadLine title={OUTSIDE_TITLE} />
                {filesTop !== undefined && <FilesFilter filter={filter} onFilter={setFilter} />}
              </>
            }
          >
            {filesTop !== undefined && rootLevel !== undefined && (
              <>
                <Tree
                  fit="content"
                  aria-label={`Files of ${OUTSIDE_TITLE}`}
                  data-testid="files"
                  size="sm"
                  // Its rows never drag, but it is the same tree as a workspace's files once a
                  // workspace is brought forward, which may (#3532).
                  mayDrag
                  {...fileFolds}
                >
                  <TopItems
                    top={filesTop}
                    level={rootLevel}
                    reading="Reading the project’s files…"
                    at={rootRows}
                  />
                </Tree>
                <div className="files-toggles">
                  <ToggleButton
                    size="sm"
                    isSelected={showIgnored}
                    onPress={() => setShowIgnored((was) => !was)}
                    className="ignored-toggle"
                  >
                    Show ignored files
                  </ToggleButton>
                </div>
              </>
            )}
          </ViewSection>
        ) : (
          <p className="empty ui-text-secondary">
            No workspace selected, so there are no repos to list.
          </p>
        )}
      </nav>
    );
  }

  const { panels, pieces } = state;
  const clones = panels?.repos ?? [];
  /** A file row's menu pressed. *Add to a chat's context* on a picked row hands the picks in
   *  the order the tree draws them, as the picked rows' own button does (#1151): the menu was
   *  drawn from them in the order they were picked. */
  const pressFile = (offer: Offer) =>
    onPress(
      offer.does.verb === "addToChat" && offer.does.rows.length > 1
        ? { ...offer, does: { ...offer.does, rows: picks.inOrder(navRef.current).map(refOf) } }
        : offer,
    );
  const at: FileRows = {
    plane,
    place: { workspace, repo: filesTop?.repo ?? "", piece: filesTop?.piece ?? null },
    files,
    fold,
    onOpenFile,
    onPress: pressFile,
    icons,
    pick: onAddToChat === undefined ? undefined : picks,
  };
  /** What a file tree of a workspace's branches takes beside its folds: its rows dragged onto a
   *  chat (FM-9), and picked several at once where they can be handed (#1151). */
  const fileTree = {
    ...fileFolds,
    draggable: plane !== undefined,
    // The same tree before the plane is known as after (#3532).
    mayDrag: true,
    choosesMany: onAddToChat !== undefined,
    onRowKey: onAddToChat === undefined ? undefined : picks.key,
  };
  /** A branch's *Files* row, and its folders under it as they are opened. */
  const filesOf = (repo: string, piece: string | null, testid: string, name?: string) => (
    <FilesRow
      key={testid}
      branch={{ repo, piece, folder: "" }}
      name={name}
      repo={name !== undefined}
      at={{ ...at, place: { workspace, repo, piece } }}
      data-testid={testid}
    />
  );
  /** **A repo of several, as a folder** (#3343): its own folder's row, closed until opened, with
   *  how many files it changed and its branch; a press opens it in place and selects it as the
   *  filter's *Selected* scope. */
  const repoRowOf = (repo: string) => {
    const top: BranchFolderRef = { repo, piece: null, folder: "" };
    const branch = repoBranch(state, repo);
    return (
      <FilesRow
        key={repo}
        branch={top}
        name={repo}
        repo
        // The repo the filter's *Selected* scope is of, drawn so (the scope switch names it):
        // never `aria-current`, which is where the next chat starts.
        data-in-scope={selected?.repo === repo && selected.piece === null ? "" : undefined}
        trail={
          branch === undefined ? undefined : (
            <span className="repo-branch ui-text-meta">{branch}</span>
          )
        }
        at={{ ...at, place: { workspace, repo, piece: null } }}
        data-testid={`files-${repo}`}
      />
    );
  };
  const filterBox = open.length > 0 && (
    // Only while some branch's files are open: it narrows the file rows and nothing else.
    <FilesFilter filter={filter} onFilter={setFilter} />
  );
  const toggles = open.length > 0 && (
    // Only while some branch's files are open: with none, there is nothing they change.
    <div className="files-toggles">
      <ToggleButton
        size="sm"
        isSelected={changedOnly}
        onPress={() => setChangedOnly((was) => !was)}
        className="changed-toggle"
      >
        Changed only
      </ToggleButton>
      <ToggleButton
        size="sm"
        isSelected={showIgnored}
        onPress={() => setShowIgnored((was) => !was)}
        className="ignored-toggle"
      >
        Show ignored files
      </ToggleButton>
    </div>
  );

  /** **The rows picked, and what can be done with them** (#1151): handed to a chat at once, or
   *  let go. Only while some are picked. */
  const pickedBar = onAddToChat !== undefined && (
    <>
      {/* How many are picked, read out as it changes (#1151): always there, so a screen
          reader hears the first pick too, and empty while none is. */}
      <span className="hidden-words" role="status">
        {picks.picks.length > 0 ? `${picks.picks.length} picked` : ""}
      </span>
      {picks.picks.length > 0 && (
        <div className="files-toggles" role="group" aria-label="Picked rows">
          <span
            className="ui-text-secondary"
            aria-hidden="true"
          >{`${picks.picks.length} picked`}</span>
          <Button
            size="sm"
            aria-label={`Add ${picks.picks.length} picked to a chat's context`}
            onPress={() => onAddToChat(picks.inOrder(navRef.current))}
          >
            Add to a chat&apos;s context
          </Button>
          <Button size="sm" aria-label="Clear the picked rows" onPress={picks.clear}>
            Clear
          </Button>
        </div>
      )}
    </>
  );

  if (cockpit !== undefined) {
    const { ref, name, on } = cockpit;
    return (
      <nav
        ref={navRef}
        className="explorer cockpit ui-text-body"
        aria-label="Explorer"
        data-testid="explorer"
        onKeyDown={(event) => {
          // Esc steps back out; one something inside already took (the filter box clearing
          // itself, a menu closing) has done its job.
          if (event.key !== "Escape" || event.defaultPrevented) return;
          event.preventDefault();
          leave();
        }}
      >
        <Breadcrumb
          workspace={workspace}
          repo={ref.repo}
          name={ref.piece === null ? undefined : name}
          onLeave={leave}
        />
        <CockpitHeader
          name={on}
          repo={ref.repo}
          piece={ref.piece}
          apart={apart}
          status={cockpitStatus}
          offers={offers}
          onPress={onPress}
        />
        {/* The workspace's read trouble is the Repos view's in the window (#1737), which is
            drawn whatever this is focused on: said here too, the Inbox would list it twice. */}
        {state.trouble && part === "whole" && (
          <ReadRefused cause={`workspace-read:${workspace}`} onReadAgain={onReadAgain}>
            {state.trouble}
          </ReadRefused>
        )}
        {filterBox}
        <Tree aria-label={`Files of ${name}`} size="sm" fit="content" {...fileTree}>
          {filesOf(ref.repo, ref.piece, `files-${ref.repo}-${ref.piece}`)}
        </Tree>
        {pickedBar}
        {toggles}
      </nav>
    );
  }

  /** What the *Files* section is of, as its heading and its tree say it. */
  const filesName =
    spot === undefined
      ? workspace
      : spot.piece === undefined
        ? spot.repo
        : pieceName(pieces[spot.repo], spot.repo, spot.piece);
  const topLevel = filesTop === undefined ? undefined : levelOf(files, filesTop);
  // What the one tree's branch changed in all, and whether a chat is in it now: the marks its
  // own folder's row would carry, beside the title that stands for that folder.
  const topKey = filesTop === undefined ? undefined : branchKey(filesTop);
  const topMarks = topKey !== undefined && (
    <>
      {" "}
      <TouchMark names={touching.get(topKey)?.get("")} />
      <ChangeBadge marked={indexes.get(topKey)?.marks.get("")} folder />
    </>
  );
  /** What stands beside the view's title (#3343): the one tree's repo and its branch. */
  const beside =
    filesTop === undefined
      ? undefined
      : [
          filesTop.repo,
          filesTop.piece === null
            ? repoBranch(state, filesTop.repo)
            : branchName(pieces[filesTop.repo], filesTop.piece),
        ]
          .filter((part) => part !== undefined && part !== "")
          .join(" · ");
  const selectedName =
    selected === undefined
      ? ""
      : selected.piece === null
        ? selected.repo
        : branchName(pieces[selected.repo], selected.piece);
  const finding = finds && filter.trim() !== "";
  /** Collapse all (#3343): every folder of this workspace's files closed. */
  const collapseAll = () =>
    setExpanded((was) => {
      const mine = `${FILE_FOLD}${workspace}\u0000`;
      const now = new Set([...was].filter((key) => !key.startsWith(mine)));
      return now.size === was.size ? was : now;
    });

  return (
    <nav
      ref={navRef}
      className={has("files") ? "explorer explorer-view ui-text-body" : "explorer ui-text-body"}
      aria-label={named.label}
      data-testid={named.testid}
    >
      {has("repos") && (
        <ReposSection
          bare={part === "repos"}
          workspace={workspace}
          state={state}
          live={live}
          spot={spot}
          picked={picked}
          onPick={onPick}
          folded={folded}
          fold={fold}
          closed={folds("repos")}
          project={plane}
          offers={offers}
          onPress={onPress}
          cloning={cloning}
          onReadAgain={onReadAgain}
        />
      )}

      {/* **Files, of where the next chat starts** (#1677), and since #3343 the view itself: no
          fold, its title the workspace's, its filter held at the top while the tree scrolls.
          The picked branch's or the one repo's files are the tree's first level; several repos
          are a row each, closed, that opens in place. */}
      {has("files") && (
        <ViewSection
          view
          data-section="files"
          top={
            <>
              <ViewHeadLine
                title={workspace}
                beside={beside}
                marks={topMarks}
                actions={
                  <IconButton
                    size="sm"
                    variant="ghost"
                    aria-label="Collapse all folders"
                    tooltip="Collapse all folders"
                    onPress={collapseAll}
                    icon={<ChevronsDownUp className="node-icon" aria-hidden="true" />}
                  />
                }
              />
              {repos.length > 0 && (
                <FilesFilter
                  filter={filter}
                  onFilter={setFilter}
                  onArrowDown={
                    finding
                      ? () =>
                          navRef.current
                            ?.querySelector<HTMLElement>('.found-files [role="row"]')
                            ?.focus()
                      : undefined
                  }
                  placeholder={
                    repos.length === 1 || !finds
                      ? "Filter files"
                      : wide
                        ? "Filter all repos"
                        : `Filter ${selectedName}`
                  }
                />
              )}
              {finds && repos.length > 1 && (
                <ScopeSwitch
                  label="Filter the files of"
                  scopes={[
                    { scope: "branch", says: `Selected: ${selectedName}` },
                    { scope: "workspace", says: "All repos" },
                  ]}
                  scope={wide ? "workspace" : "branch"}
                  onPick={(to) => setAllRepos(to === "workspace")}
                />
              )}
              {repos.length === 1 && beside !== undefined && (
                <p className="files-scope-note ui-text-meta">
                  <span className="files-scope-chip">
                    <FolderGit2 className="node-icon" aria-hidden="true" />
                    {beside}
                  </span>{" "}
                  One repo, so the filter covers all of it.
                </p>
              )}
              {onSearchContents !== undefined && selected !== undefined && (
                <p className="files-search-link ui-text-meta">
                  Names only.{" "}
                  <Button
                    variant="link"
                    size="sm"
                    onPress={() =>
                      onSearchContents(
                        wide ? "workspace" : "branch",
                        { workspace, ...selected },
                        filter.trim(),
                      )
                    }
                  >
                    {`Search contents in ${wide ? "Workspace" : "Branch"}`}
                    <ArrowUpRight className="node-icon" aria-hidden="true" />
                  </Button>{" "}
                  <kbd className="files-search-key" aria-hidden="true">
                    {searchKeySaid(onAMac())}
                  </kbd>
                </p>
              )}
            </>
          }
        >
          {finding ? (
            <FoundFiles
              found={found}
              query={filter.trim()}
              grouped={wide}
              pieces={pieces}
              icons={icons}
              onOpen={(hit) => onOpenFile?.(placeOf(hit), hit.path)}
            />
          ) : (
            <>
              <Tree
                aria-label={`Files of ${filesName}`}
                data-testid="files"
                size="sm"
                fit="content"
                {...fileTree}
              >
                {filesTop === undefined || topLevel === undefined
                  ? clones.map(repoRowOf)
                  : [
                      <TopItems
                        // Keyed by its folder: another workspace's one repo is other rows, whose
                        // ids the tree family never changes in place.
                        key={`${workspace}\u0000${folderKey(filesTop)}`}
                        top={filesTop}
                        level={topLevel}
                        reading={`Reading the files of ${filesName}…`}
                        at={at}
                      />,
                    ]}
              </Tree>
              {pickedBar}
              {toggles}
            </>
          )}
        </ViewSection>
      )}
    </nav>
  );
}

/**
 * **Which of Explorer a drawing of it is** (#1737): the `whole` of it, as it was until the Panel
 * (a component drawn on its own); the window's `explorer` view, *Files*, in the left sidebar; or
 * its `repos` view, *Repos and branches*, in the left half of the Panel.
 */
export type ExplorerPart = "whole" | "explorer" | "repos";

/** What ⌘⇧F runs while the keyboard is `inside` Explorer's files view (#3343). */
export type SearchKeyHere = { inside: HTMLElement | null; run: () => void };

const PART_SECTIONS: Record<ExplorerPart, readonly SectionId[]> = {
  whole: ["repos", "files"],
  explorer: ["files"],
  repos: ["repos"],
};

const PART_NAMES: Record<ExplorerPart, { label: string; testid: string }> = {
  whole: { label: "Explorer", testid: "explorer" },
  explorer: { label: "Explorer", testid: "explorer" },
  repos: { label: "Repos", testid: "repos-view" },
};

/** The *Filter files* box: it finds files by name in a workspace's Files (#3343), else narrows
 *  the file rows drawn to names holding what is typed; Esc clears it. */
function FilesFilter({
  filter,
  onFilter,
  onArrowDown,
  placeholder = "Filter files",
}: {
  filter: string;
  onFilter: (to: string) => void;
  /** ↓ from the box: into the files it found (#3343), as the palette's box does. */
  onArrowDown?: () => void;
  /** What it finds in, said in the box while it is empty: "Filter svc", "Filter all repos". */
  placeholder?: string;
}) {
  return (
    <TextField
      className="files-filter"
      type="search"
      icon={<ListFilter aria-hidden="true" />}
      keepsFocus="files-filter"
      value={filter}
      aria-label="Filter files"
      placeholder={placeholder}
      onChange={onFilter}
      onKeyDown={(event) => {
        if (event.key === "ArrowDown" && onArrowDown !== undefined) {
          event.preventDefault();
          onArrowDown();
          return;
        }
        if (event.key !== "Escape" || filter === "") return;
        // Taken here: an Esc that cleared the box has done its job, and nothing behind
        // the explorer should act on it too.
        event.preventDefault();
        event.stopPropagation();
        onFilter("");
      }}
    />
  );
}

/**
 * **The view's heading line** (#3343): its title, what stands beside it, and its actions. In a
 * region the title is the region's head (#3330), so what stands beside it and the actions go on
 * that line ({@link HeadActions}); drawn on its own (a story, a test) it is the top's first line.
 */
function ViewHeadLine({
  title,
  beside,
  marks,
  actions,
}: {
  title: string;
  beside?: string;
  /** The marks of what stands beside it: what its branch changed, a chat in it now. */
  marks?: ReactNode;
  actions?: ReactNode;
}) {
  const head = useContext(HeadActions);
  const besideIt = beside !== undefined && beside !== "" && (
    <span className="head-beside ui-text-meta">
      {beside}
      {marks}
    </span>
  );
  if (head === null) return null;
  if (head !== undefined)
    return createPortal(
      <>
        {besideIt}
        {actions}
      </>,
      head,
    );
  return (
    <PanelSectionHead>
      <PanelTitle>{title}</PanelTitle>
      {besideIt}
      {actions}
    </PanelSectionHead>
  );
}

/** A repo's own branch, as the workspace read it; nothing until it has. */
function repoBranch(state: WorkspaceState, repo: string): string | undefined {
  return state.repos?.repos.find((one) => one.name === repo)?.branch || undefined;
}

/** A branch of a repo by its name, or its folder's where it has none (yet). */
function branchName(pieces: readonly Piece[] | undefined, piece: string): string {
  const found = pieces?.find((one) => one.piece === piece);
  return found === undefined ? piece : branchOf(found);
}

/**
 * **The files the filter found by name** (#3343): best first, under their repo's name where the
 * workspace has several, each with the folder it is in under its name, the letters the filter
 * matched marked. A press opens the file in its tab. What the core could not list is said.
 */
function FoundFiles({
  found,
  query,
  grouped,
  pieces,
  icons,
  onOpen,
}: {
  found: FilesFoundNow;
  query: string;
  grouped: boolean;
  /** Each repo's branches, so a group of a branch is headed by its name. */
  pieces?: WorkspaceState["pieces"];
  icons: IconTheme;
  onOpen: (hit: FoundFile) => void;
}) {
  const refused = found.refused.map((why) => (
    // No role of its own (#3917): a role's colour wins over App.css's, and none is the danger
    // colour. It takes the view's size and `.trouble`'s `danger.text` on `danger.surface`.
    <p key={why} className="trouble" role="alert">
      {why}
    </p>
  ));
  if (found.files.length === 0)
    return (
      <>
        {refused}
        {found.looking ? (
          <Pending>Finding files…</Pending>
        ) : (
          <p className="none ui-text-secondary">
            {`No file names match “${query}”. Search file contents instead.`}
          </p>
        )}
      </>
    );
  const groups = new Map<string, FoundFile[]>();
  for (const hit of found.files) {
    const at =
      hit.piece === null ? hit.repo : `${hit.repo} · ${branchName(pieces?.[hit.repo], hit.piece)}`;
    groups.set(at, [...(groups.get(at) ?? []), hit]);
  }
  const rowOf = (hit: FoundFile) => {
    const said = hitSaid(hit, () => "");
    const folder = said.marks.folder.map((part) => part.text).join("");
    return (
      <ListRow
        key={`${hit.repo}/${hit.piece ?? ""}:${hit.path}`}
        id={`${hit.repo}/${hit.piece ?? ""}:${hit.path}`}
        textValue={said.name}
        className="found-file"
        onAction={() => onOpen(hit)}
      >
        <RowLead>
          <FileIcon symbol={iconFor(icons, { name: said.name, folder: false })} />
        </RowLead>
        <RowLabel>
          {said.marks.name.map((part, at) =>
            part.matched ? (
              <mark key={at} className="found-match">
                {part.text}
              </mark>
            ) : (
              <span key={at}>{part.text}</span>
            ),
          )}
        </RowLabel>
        <RowBelow className="ui-text-meta">{folder === "" ? hit.repo : folder}</RowBelow>
      </ListRow>
    );
  };
  return (
    <>
      {refused}
      <List aria-label="Files found" size="sm" className="found-files">
        {grouped
          ? [...groups].map(([at, hits]) => (
              <ListRowSection key={at} id={at} title={at} named count={hits.length}>
                {hits.map(rowOf)}
              </ListRowSection>
            ))
          : found.files.map(rowOf)}
      </List>
    </>
  );
}

/**
 * **The *Repos and branches* section** (#1677): the workspace itself, each clone with its
 * branches under it, and what purlis could not read or has not cloned here. Picking a row is
 * where the next chat starts, and what *Files* lists.
 *
 * **A tree of the tree family** (#2911): the workspace's own row always open, a clone's folding
 * on a press of it, a twist or the arrows, and a branch's picking where the next chat starts.
 * The row picked is the current one (`aria-current`, never a choice the arrows move: ADR 0036)
 * and the tree's Tab stop until the keyboard has been in.
 */
function ReposSection({
  bare = false,
  workspace,
  state,
  live,
  spot,
  picked,
  onPick,
  folded,
  fold,
  closed,
  project,
  offers,
  onPress,
  cloning,
  onReadAgain,
}: {
  /** Drawn as a view of its own (#1737): its title is the view's, and it does not fold. */
  bare?: boolean;
  workspace: string;
  state: WorkspaceState;
  live: boolean;
  spot: Spot | undefined;
  /** The row picked, by its id, or nothing while a branch is focused. */
  picked: string | undefined;
  onPick: (spot: Spot | undefined) => void;
  /** The clones the operator folded, by {@link foldKey}. */
  folded: ReadonlySet<string>;
  fold: (key: string, open: boolean) => void;
  closed: boolean;
  project: string | undefined;
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  cloning: Cloning | undefined;
  onReadAgain: () => void;
}) {
  const { panels, pieces, piecesRefused } = state;
  const clones = panels?.repos ?? [];
  const absent = notClonedRows(panels?.absent ?? [], clones, cloning);
  /** A repo is cloning that the workspace does not name yet: it has one, on its way. */
  const coming = absent.length > (panels?.absent ?? []).length;
  const open = (repo: string) => !folded.has(foldKey(workspace, repo));
  return (
    <Section
      id="repos"
      title={bare ? "Repos" : "Repos and branches"}
      closed={closed}
      project={project}
      bare={bare}
    >
      {state.trouble && (
        <ReadRefused cause={`workspace-read:${workspace}`} onReadAgain={onReadAgain}>
          {state.trouble}
        </ReadRefused>
      )}

      {/* The tree is the rows and what holds them. The sentences about the whole region — the
          trouble above, the pending and empty notes and what purlis would not read below —
          are outside it. The ones about ONE clone (its worktrees could not be listed, are
          still coming, or are none) are lines of the tree under that clone's row, beside the
          row they explain. */}
      <Tree
        aria-label="Repos and branches"
        fit="content"
        size="sm"
        stopAt={picked}
        expandedKeys={[ROOT, ...clones.filter(open).map(cloneRow)]}
        onExpandedChange={(keys) => {
          for (const repo of clones)
            if (keys.has(cloneRow(repo)) !== open(repo))
              fold(foldKey(workspace, repo), keys.has(cloneRow(repo)));
        }}
      >
        <TreeRow
          id={ROOT}
          textValue={workspace}
          description="the workspace itself"
          className="spot spot-root"
          data-row={ROOT}
          alwaysOpen
          // Not `aria-selected`, even in a tree: the three tablists in this window are where
          // it selects (ADR 0036), and a picked spot is not a selection the keyboard moves but
          // the place the next chat starts — the current item, which is what `aria-current` is
          // for and what the old sidebar's workspace rows used.
          current={spot === undefined}
          onAction={() => onPick(undefined)}
          content={
            <>
              <Folders className="node-icon" />
              <span className="spot-name">{workspace}</span>
              {live && <LiveMark />}
              <span className="spot-what ui-text-secondary">the workspace itself</span>
            </>
          }
        >
          {clones.map((repo) => (
            <TreeRow
              key={repo}
              id={cloneRow(repo)}
              textValue={repo}
              description={pieceCountSaid(pieces[repo], piecesRefused[repo])}
              className="clone"
              data-row={cloneRow(repo)}
              data-testid={`clone-${repo}`}
              data-clone={repo}
              current={picked === cloneRow(repo)}
              // A press on a clone opens and closes it, which is what a heading does; it is
              // picked from its menu (charter-app#174).
              onAction={() => fold(foldKey(workspace, repo), !open(repo))}
              // The clone's menu: a new tab in it, and picking it as where new chats start
              // (charter-app#174).
              menu={catalogueMenu({ on: "clone", repo }, offers, onPress)}
              // Why its branches could not be listed, beside its row (#1476).
              tip={piecesRefused[repo]}
              busy={pieces[repo] === undefined && piecesRefused[repo] === undefined}
              hasChildItems
              content={
                <>
                  <FolderGit2 className="node-icon" />
                  <span className="repo ui-text-body ui-text-strong">{repo}</span>
                  <PieceCount pieces={pieces[repo]} refused={piecesRefused[repo]} />
                </>
              }
            >
              {cloneItems({ workspace, repo, state, spot, offers, onPress, onPick, onReadAgain })}
            </TreeRow>
          ))}
        </TreeRow>
      </Tree>

      {panels === undefined ? (
        <Pending>Reading the project…</Pending>
      ) : (
        clones.length === 0 &&
        !coming && <p className="none ui-text-secondary">No repos in this workspace</p>
      )}

      {/* Membership without a clone. There is nothing to explore in it and nothing to start
            a chat in, so it is named and not made a heading — and it can be cloned (#1215). */}
      <NotClonedHere
        absent={absent}
        cloning={cloning}
        offers={offers}
        onPress={onPress}
        onStop={
          project === undefined ? undefined : (repo) => stopRepoClones(project, workspace, [repo])
        }
      />

      {panels?.refused.map(([name, why]) => (
        <ReadRefused
          key={`refused-${name}`}
          cause={`repo-refused:${workspace}/${name}`}
          onReadAgain={onReadAgain}
        >
          purlis will not read <code>{name}</code>: {why}
        </ReadRefused>
      ))}
    </Section>
  );
}

/** Why a branch is unclaimed, whole (#835). */
function unclaimedSaid(age: string): string {
  return `purlis cut this branch ${age} ago for a chat that never started in it. Nothing removes it on its own: start a chat in it, or remove its folder from its menu.`;
}

/** What a clone's count says, in words, after its name. */
function pieceCountSaid(pieces: readonly unknown[] | undefined, refused: string | undefined) {
  if (refused !== undefined) return "branches unreadable";
  if (pieces === undefined) return "";
  return `${pieces.length} ${pieces.length === 1 ? "branch" : "branches"}`;
}

/**
 * **What a clone holds, a row each**: its branches, or the line that says they could not be
 * listed (with *Read again*), are still being asked for, or are none.
 */
function cloneItems({
  workspace,
  repo,
  state,
  spot,
  offers,
  onPress,
  onPick,
  onReadAgain,
}: {
  workspace: string;
  repo: string;
  state: WorkspaceState;
  spot: Spot | undefined;
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  onPick: (spot: Spot | undefined) => void;
  onReadAgain: () => void;
}): ReactNode {
  const { pieces, piecesRefused } = state;
  const line = (what: string) => `${cloneRow(repo)}#${what}`;
  const refused = piecesRefused[repo];
  if (refused)
    // Said, never swallowed: a clone with no rows otherwise reads as a clone nobody has cut a
    // branch in. Its *Read again* is reached by → from the line.
    return (
      <TreeLine
        key="refused"
        id={line("refused")}
        textValue={`purlis could not list the branches of ${repo}: ${refused}`}
        className="clone-refused"
        data-clone={repo}
      >
        <ReadRefused cause={`branches-read:${workspace}/${repo}`} onReadAgain={onReadAgain}>
          purlis could not list the branches of <code>{repo}</code>: {refused}
        </ReadRefused>
      </TreeLine>
    );
  if (pieces[repo] === undefined)
    return (
      <TreePending key="pending" className="pending" data-clone={repo}>
        <LoaderCircle className="node-icon spinning" />
        <span>Asking git…</span>
      </TreePending>
    );
  if (pieces[repo].length === 0)
    return (
      <TreeLine
        key="none"
        id={line("none")}
        textValue="No branches cut here"
        className="none"
        data-clone={repo}
      >
        <span>No branches cut here</span>
      </TreeLine>
    );
  return pieces[repo].map((piece) => {
    const isPicked = spot?.repo === repo && spot.piece === piece.piece;
    // What the row reads first, so a typed letter finds it by its branch (#1102); its marks'
    // words after it, as its description.
    const said = piece.branch ? `${piece.branch} in ${repo}` : piece.piece;
    const marks = [
      ...(piece.stale ? ["stale"] : piece.wired ? [] : ["unwired"]),
      ...(piece.said ? [piece.said] : []),
      ...(piece.unread ? ["unreadable"] : []),
      ...(piece.unclaimed ? [`unclaimed ${piece.unclaimed}`] : []),
    ].join(", ");
    // Its folder, and why its marks say what they say: the row's own tooltip, under the pointer
    // and on the keyboard (a mark inside a row takes no keyboard of its own).
    const tip = [
      piece.path,
      ...(piece.unread ? [piece.unread] : []),
      ...(piece.unclaimed ? [unclaimedSaid(piece.unclaimed)] : []),
    ].join("\n");
    return (
      <TreeRow
        key={piece.piece}
        id={pieceRow(repo, piece.piece)}
        textValue={said}
        description={marks}
        className="spot"
        data-row={pieceRow(repo, piece.piece)}
        data-testid={`piece-${repo}-${piece.piece}`}
        data-clone={repo}
        current={isPicked}
        tip={tip}
        onAction={() => onPick({ repo, piece: piece.piece, path: piece.path })}
        // **Right-click is what these rows were missing** (charter-app#174). The menu is the
        // catalogue filtered to this piece — merge above the line, remove below it, and the
        // discard row that only exists while the core has refused THIS removal. Nothing here
        // says what those rows mean; `Menus.tsx` draws whatever `actions.ts` has.
        menu={catalogueMenu({ on: "worktree", repo, piece: piece.piece }, offers, onPress)}
        content={
          <>
            <GitBranch className="node-icon" />
            <BranchLabel repo={repo} piece={piece} />
            {/* The two states the operator has to see BEFORE they start a chat in a tree:
                `unwired` and `stale`. The same component the palette's worktree rows are
                written against, without the branch the row already reads (#1102). */}
            <WorktreeMark
              withBranch={false}
              worktree={{
                workspace,
                repo,
                piece: piece.piece,
                branch: piece.branch,
                wired: piece.wired,
                stale: piece.stale,
              }}
            />
            {/* What the piece said about itself — `done`, `abandoned: <why>` or `silent 3d` —
                so a finished piece and a quiet one do not look alike (charter#368). An age,
                never a verdict. */}
            {piece.said && (
              <span className="label said" data-testid="piece-said">
                {piece.said}
              </span>
            )}
            {/* A folder purlis could not read: said, with the core's sentence for why (#1476),
                never drawn as one with nothing to commit. The row is one line, so the
                sentence is the label's tooltip. */}
            {piece.unread && (
              <span className="label said" data-testid="piece-unread">
                unreadable
              </span>
            )}
            {/* A branch purlis cut for a chat that never started in it — a crash between the
                cut and the start leaves one (#835). Said, with its age, and never swept: its
                folder's removal stays the row's menu's. */}
            {piece.unclaimed && (
              <span className="label said" data-testid="piece-unclaimed">
                unclaimed {piece.unclaimed}
              </span>
            )}
          </>
        }
      />
    );
  });
}

/** One of Explorer's sections (#1677): a heading that folds it, and what it holds. **Folding
 *  hides and never unmounts**, so a section opened again has its folds and its stop as it left
 *  them. The heading is a disclosure button, the pattern an editor's explorer uses. */
function Section({
  id,
  title,
  closed,
  project,
  bare = false,
  children,
}: {
  id: SectionId;
  title: string;
  closed: boolean;
  /** The project whose sections these are: a fold is kept for it (B-11). */
  project: string | undefined;
  /** A view's own title rather than a fold (#1737): the Repos view's, in the Panel. Its region's
   *  head is that title where there is one (#3330), so it draws none of its own there. */
  bare?: boolean;
  children: ReactNode;
}) {
  const body = useId();
  const headed = useContext(HeadActions) !== undefined;
  if (bare) {
    return (
      <div className="explorer-section" data-section={id}>
        {!headed && (
          <PanelSectionTitle className="explorer-section-head" mark={FolderGit2}>
            {title}
          </PanelSectionTitle>
        )}
        <div id={body} className="explorer-section-body">
          {children}
        </div>
      </div>
    );
  }
  return (
    <div className="explorer-section" data-section={id}>
      <PanelSectionTitle className="explorer-section-head">
        {/* The heading's whole line is the disclosure: a row of the set (`.ui-row`, #1857), so
            its title starts where the section's rows and its filter's text do. */}
        <button
          type="button"
          className="ui-row ui-text-label"
          // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
          tabIndex={0}
          aria-expanded={!closed}
          aria-controls={body}
          onClick={() => setSectionOpen(id, closed, project)}
        >
          <ChevronRight className="twisty" data-open={!closed || undefined} />
          {title}
        </button>
      </PanelSectionTitle>
      <div id={body} className="explorer-section-body" hidden={closed}>
        {children}
      </div>
    </div>
  );
}

/** A branch's name as the explorer reads it, by its folder; the folder itself when the
 *  workspace has not listed it (yet). */
function pieceName(pieces: readonly Piece[] | undefined, repo: string, piece: string): string {
  const found = pieces?.find((one) => one.piece === piece);
  return found === undefined ? piece : `${branchOf(found)} in ${repo}`;
}

/** **A read this region was refused, as a Notice** (NO-4, V91b): the sentence, and **Read
 *  again**, which asks the workspace's reads again and clears the line once one goes through. */
function ReadRefused({
  cause,
  onReadAgain,
  children,
}: {
  cause: string;
  onReadAgain: () => void;
  children: ReactNode;
}) {
  return (
    <Notice cause={cause} tone="trouble" fixes={[{ label: "Read again", onPress: onReadAgain }]}>
      {children}
    </Notice>
  );
}

/** Something charter is still reading.
 *
 *  **The one place in this region an animation earns its place.** A spinner here means "this
 *  is still happening", which is a state no colour and no word can distinguish from "this
 *  stopped and nothing came back" — the reason the operator asked for motion on the pipelines.
 *  It is a state that ends, and it is drawn at most twice. `prefers-reduced-motion` stops the
 *  spin and leaves the mark. */
function Pending({ children }: { children: ReactNode }) {
  return (
    <p className="pending ui-text-secondary">
      <LoaderCircle className="node-icon spinning" />
      <span>{children}</span>
    </p>
  );
}

/** Where the next chat starts, when it is not the workspace's own directory: a piece, or — with
 *  no `piece` — the clone itself, picked from its menu (charter-app#174).
 *
 *  It carries the path the CORE spelled — `worktree_list` answers with a piece's, and
 *  `Panels.paths` with a clone's — so nothing here ever joins one together. */
export type Spot = { repo: string; piece?: string; path: string };

/** How many pieces a clone has, on its heading, so a closed one still says whether there is
 *  anything in it. */
function PieceCount({ pieces, refused }: { pieces?: readonly unknown[]; refused?: string }) {
  // Why is the clone row's own tooltip (`tip`), under the pointer and on the keyboard.
  if (refused !== undefined)
    return <span className="piece-count none ui-text-meta">unreadable</span>;
  if (pieces === undefined) return <span className="piece-count pending ui-text-meta">…</span>;
  return (
    // Its number is the row's name's last word, as it always was: the tree row says the rest.
    <Count className="piece-count">{pieces.length}</Count>
  );
}

/** What the tree knows about branches' folders: which are open, what each holds, and whether
 *  what git ignores is drawn. */
export type FilesOf = {
  workspace: string;
  expanded: ReadonlySet<string>;
  reads: ReadonlyMap<string, FolderRead>;
  showIgnored: boolean;
  /** What each open branch changed, by {@link branchKey}. */
  statuses: ReadonlyMap<string, StatusRead>;
  /** The same, indexed by path and by folder, by {@link branchKey}. */
  indexes: ReadonlyMap<string, Indexed>;
  /** Whether each branch's files are collapsed to what it changed. */
  changedOnly: boolean;
  /** What the file rows are narrowed to, lower-cased; `""` narrows nothing. */
  filter: string;
  /** Each folder's {@link levelOf}, worked out once per render. */
  levels: Map<string, Level>;
  /** What the chats are touching in each open branch, by {@link branchKey} (FM-6). */
  touching?: ReadonlyMap<string, Touching>;
};

/** One row under a folder of a branch, as both the render and {@link folderNode} walk it. */
export type Child = {
  name: string;
  kind: FolderEntry["kind"];
  ignored: boolean;
  refused: string | undefined;
  /** A folder that opens onto what it holds. */
  expands: boolean;
  /** A folder that is always open: one of *Changed only*'s. */
  always: boolean;
  marked: Marked | undefined;
  /** At the plane root, the workspace whose repo this folder is (issue 1916, ruling F2): a
   *  row that leads to that workspace's tab, and never one that opens. */
  workspace?: string;
};

/** What a folder of a branch draws under it: still being read, refused, or its rows. */
export type Level =
  { pending: true } | { trouble: string } | { children: Child[]; more: number; narrowed: boolean };

/** What a file the branch deleted says, drawn where it was. */
const DELETED = "deleted on this branch";

/** Whether a folder of a branch is open: one of *Changed only*'s always is. */
function isOpen(files: FilesOf, ref: BranchFolderRef, always: boolean): boolean {
  return always || files.expanded.has(fileFold(files.workspace, ref));
}

/**
 * What an OPEN folder of a branch draws under it: its entries — or, under *Changed only*, the
 * paths below it the branch changed — each with its mark, narrowed to the filter.
 *
 * **The one answer the render and {@link folderNode} both read**, so a row drawn is a row the
 * keyboard and a screen reader know, and the other way round.
 */
export function levelOf(files: FilesOf, ref: BranchFolderRef): Level {
  const cached = files.levels.get(folderKey(ref));
  if (cached !== undefined) return cached;
  const level = levelOfUncached(files, ref);
  files.levels.set(folderKey(ref), level);
  return level;
}

function levelOfUncached(files: FilesOf, ref: BranchFolderRef): Level {
  const key = branchKey(ref);
  const index = files.indexes.get(key);
  let children: Child[];
  let more = 0;
  if (files.changedOnly) {
    const read = files.statuses.get(key);
    if (read === undefined) return { pending: true };
    if (read.trouble !== undefined) return { trouble: read.trouble };
    children = changedUnder(index, ref.folder);
    // Past the most the core marks, the rest are counted at the branch's top.
    if (ref.folder === "") more = index?.status.more ?? 0;
  } else {
    const read = files.reads.get(folderKey(ref));
    if (read === undefined) return { pending: true };
    if (read.trouble !== undefined) return { trouble: read.trouble };
    more = read.more ?? 0;
    const entries = shown(read.entries ?? [], files.showIgnored);
    children = entries.map((entry): Child => ({
      name: entry.name,
      kind: entry.kind,
      ignored: entry.ignored,
      refused: entry.refused ?? undefined,
      expands: expands(entry),
      always: false,
      marked: index?.marks.get(joined(ref.folder, entry.name)),
      workspace: entry.workspace ?? undefined,
    }));
    // What the branch deleted here is not on disk to list: it is drawn where it was.
    const listed = new Set(entries.map((entry) => entry.name));
    const gone = changedUnder(index, ref.folder).filter(
      (child) => child.marked?.mark === "deleted" && !child.expands && !listed.has(child.name),
    );
    children = [...children, ...gone];
  }
  if (files.filter === "") return { children, more, narrowed: false };
  const kept = children.filter((child) => {
    if (child.name.toLocaleLowerCase().includes(files.filter)) return true;
    if (!child.expands) return false;
    const here = { ...ref, folder: joined(ref.folder, child.name) };
    if (!isOpen(files, here, child.always)) return false;
    const below = levelOf(files, here);
    return "children" in below && below.children.length > 0;
  });
  return { children: kept, more, narrowed: true };
}

/** The rows directly under `folder` that lead to a change: each changed file there, and each
 *  folder holding one, folders first, in the order a person reads names. */
function changedUnder(index: Indexed | undefined, folder: string): Child[] {
  return (index?.under.get(folder) ?? []).map((seed) => ({
    name: seed.name,
    kind: seed.folder ? "folder" : "file",
    ignored: false,
    refused: !seed.folder && seed.mark === "deleted" ? DELETED : undefined,
    expands: seed.folder,
    always: seed.folder,
    marked: index?.marks.get(joined(folder, seed.name)),
  }));
}

/** The words a mark is said in. */
const MARK_SAID: Record<Marked["mark"], string> = {
  changed: "changed",
  added: "added",
  deleted: "deleted",
  renamed: "renamed",
};

/** A file's or a folder's mark: a letter for a file, how many for a folder, and the words for a
 *  screen reader and a hover. */
function ChangeBadge({ marked, folder }: { marked: Marked | undefined; folder: boolean }) {
  if (marked === undefined) return null;
  const said = markSaid(marked, folder);
  return (
    <Tooltip tip={said}>
      <span className="change-mark" data-mark={marked.mark} aria-label={said}>
        {folder ? marked.count : marked.mark[0].toUpperCase()}
      </span>
    </Tooltip>
  );
}

/** What a file's or a folder's mark says, in words. */
function markSaid(marked: Marked, folder: boolean): string {
  return folder
    ? `${marked.count ?? 0} ${marked.count === 1 ? "change" : "changes"}`
    : marked.from
      ? `renamed from ${marked.from}`
      : MARK_SAID[marked.mark];
}

/**
 * **A file or folder row's name to a reader**: its name, then what a chat is doing in it and
 * what the branch changed in it, as its marks say them; typing still finds it by its name.
 */
function rowSaid(
  name: string,
  touchedBy: readonly string[] | undefined,
  marked: Marked | undefined,
  folder: boolean,
  refused?: string,
): string {
  return [
    name,
    ...(touchedBy !== undefined && touchedBy.length > 0 ? [touchSaid(touchedBy)] : []),
    ...(marked === undefined ? [] : [markSaid(marked, folder)]),
    ...(refused === undefined ? [] : [refused]),
  ].join(" ");
}

/** The live mark on a file or folder a chat is touching right now (FM-6): a dot, and on hover
 *  and to a screen reader which chats. */
function TouchMark({ names }: { names: readonly string[] | undefined }) {
  if (names === undefined || names.length === 0) return null;
  const said = touchSaid(names);
  return (
    <Tooltip tip={said}>
      <span className="touch-mark" role="img" aria-label={said} />
    </Tooltip>
  );
}

/** Everything a branch's file rows share: the branch they are of, what the tree knows about
 *  its folders, and what a row does. The file tab (FM-2) draws its tree out of the same rows. */
export type FileRows = {
  /** The project the rows are of: what a row dragged onto a chat names (FM-9). No drag
   *  without it. */
  plane?: PlaneId;
  place: Place;
  files: FilesOf;
  fold: (key: string, open: boolean) => void;
  onOpenFile?: (place: Place, path: string) => void;
  /** The project's icon theme (FM-3), which every file and folder row is drawn from. */
  icons: IconTheme;
  /** A file or folder row's menu was used (FM-10): copy its path, reveal it, your editor or a
   *  shell tab there. No menu without it. */
  onPress?: (offer: Offer) => void;
  /** The rows picked to hand a chat at once (#1151); none where they cannot be handed. */
  pick?: ReturnType<typeof useFilePicks>;
  /** A workspace's repo row was pressed in the plane root's files (issue 1916). */
  onGoToWorkspace?: (workspace: string) => void;
  /** The file a file tab shows (FM-2), by its path: its row is the tree's chosen one. */
  shown?: string;
};

/** What a file row's menu is looked up in: nothing, because its rows are its own
 *  (`actions.fileRows`). */
const NO_CATALOGUE: Catalogued = new Map();

/**
 * **A file or folder row's menu** (FM-10): the rows `actions.fileRows` builds for it — never
 * listed in the catalogue — and nothing that creates, renames, moves or deletes (ADR 0081).
 * None where the tree has no way to carry a row out. `line` is where its preview was last read
 * (#1143), so *Open in your editor* opens there.
 */
function fileMenu(on: FileOn, at: FileRows, line?: number): RowMenu | undefined {
  if (at.onPress === undefined) return undefined;
  // The rows picked with it, where two or more are (#1151): its *Add to a chat's context* hands
  // them all when it is one of them (`actions.fileRows`).
  const picks = at.pick?.picks ?? [];
  const picked = picks.length > 1 ? { picked: picks.map(refOf) } : {};
  return catalogueMenu(
    on.kind === "file" ? { ...on, line, ...picked } : { ...on, ...picked },
    NO_CATALOGUE,
    at.onPress,
  );
}

/**
 * What a file or folder row carries when it is dragged onto a chat (FM-9): the row's branch and
 * path, which the core places and renders in the chat's own syntax. Nothing on a refused row, or
 * where the tree does not know its project.
 */
function carriedBy(at: FileRows, path: string, folder: boolean, refused?: string) {
  const plane = at.plane;
  // A reference names a branch and a path in it, which a file of the plane root has not.
  if (isPlaneRoot(at.place)) return undefined;
  if (plane === undefined || refused !== undefined || path === "") return undefined;
  return carriedReference({ plane, ...at.place, path, folder });
}

/** A row as it can be picked (#1151): nothing on a refused row, the branch's own *Files* row, or
 *  where the tree hands no rows to a chat. */
function pickOf(
  at: FileRows,
  path: string,
  folder: boolean,
  refused?: string,
): Pickable | undefined {
  const plane = at.plane;
  if (at.pick === undefined || plane === undefined || refused !== undefined || path === "")
    return undefined;
  return { plane, ...at.place, path, folder };
}

/** A picked row as a menu hands it to a chat (#1151). */
function refOf({ workspace, repo, piece, path, folder }: Pickable): FileRef {
  return { at: { workspace, repo, piece, path }, folder };
}

/** What a row says of its pick (#1151): the reference it is picked by, and whether it is. */
function pickedAs(at: FileRows, r: Pickable | undefined) {
  if (at.pick === undefined || r === undefined) return {};
  return { ...pickable(r), chosen: at.pick.isPicked(r) };
}

/** A file row's path in the branch, as its menu names it. */
function pathOn(at: FileRows, path: string): BranchPath {
  return { ...at.place, path };
}

/**
 * **A press on a row picks it, where Cmd, Ctrl or Shift is held** (#1151), else runs it: its
 * modifier keys are heard as the press begins, before its action runs.
 */
function usePickingPress(at: FileRows, picked: Pickable | undefined, run: () => void) {
  const held = useRef<PressEvent>(undefined);
  return {
    onPressStart: (event: PressEvent) => {
      held.current = event;
    },
    onAction: () => {
      const press = held.current;
      held.current = undefined;
      if (press !== undefined && at.pick?.press(press, picked)) return;
      run();
    },
  };
}

/**
 * **A branch folder's *Files* row, or a folder's row**, with what it holds under it once opened:
 * a row of the tree family that folds (`fold`), unless it is one of *Changed only*'s, which is
 * always open.
 */
function FilesRow({
  branch,
  name = "Files",
  always = false,
  repo = false,
  trail,
  at,
  ...data
}: {
  branch: BranchFolderRef;
  /** At the row's end: a repo's branch (#3343). */
  trail?: ReactNode;
  /** A repo's own folder, drawn as the repo, in the *Files* section. */
  repo?: boolean;
  /** Its name: *Files* for the branch's own folder in the cockpit, the repo's for a repo's own
   *  folder in the *Files* section, the folder's name below either. */
  name?: string;
  /** Always open: a folder of *Changed only*'s, which does not fold. */
  always?: boolean;
  at: FileRows;
  /** `data-*` attributes reach the row as they are. */
  [attribute: `data-${string}`]: string | undefined;
}) {
  const id = fileRow(branch);
  const key = fileFold(at.place.workspace, branch);
  const open = isOpen(at.files, branch, always);
  const level = open ? levelOf(at.files, branch) : undefined;
  const marked = at.files.indexes.get(branchKey(branch))?.marks.get(branch.folder);
  const touchedBy = at.files.touching?.get(branchKey(branch))?.get(branch.folder);
  const picked = pickOf(at, branch.folder, true);
  const press = usePickingPress(at, picked, () => {
    if (!always) at.fold(key, !open);
  });
  return (
    <TreeRow
      {...data}
      {...press}
      {...pickedAs(at, picked)}
      id={id}
      textValue={rowSaid(name, touchedBy, marked, true)}
      className="file-node"
      data-row={id}
      data-mark={marked?.mark}
      busy={level !== undefined && "pending" in level}
      hasChildItems
      // A folder's menu (FM-10). The branch's own *Files* row has one too (#1143): its folder
      // placed whole, the rows its branch's row offers (`actions.fileRows`).
      menu={fileMenu({ on: "file", at: pathOn(at, branch.folder), kind: "folder" }, at)}
      carries={carriedBy(at, branch.folder, true)}
      content={
        <>
          {/* The branch's own folder is the *Files* row, or a repo's own folder by its name in
              the *Files* section; a folder under either is drawn by name. */}
          {branch.folder === "" ? (
            repo ? (
              <FolderGit2 className="node-icon" />
            ) : (
              <Files className="node-icon" />
            )
          ) : (
            <FileIcon symbol={iconFor(at.icons, { name, folder: true, open })} />
          )}
          <span className="spot-name">{name}</span>
          {/* A space, so a screen reader says the mark as its own word; flex drops it. */}{" "}
          <TouchMark names={touchedBy} />
          <ChangeBadge marked={marked} folder />
          {trail}
        </>
      }
    >
      {level === undefined ? null : <FolderItems branch={branch} level={level} at={at} />}
    </TreeRow>
  );
}

/**
 * **What one opened folder holds, a row each**: folders that open in turn, and files; the lines
 * that say it is still being read, could not be, holds nothing or more than the core lists; and
 * what its branch names and does not read, under its top. Each a row of the tree family, in
 * the order it is drawn.
 */
export function FolderItems({
  branch,
  level,
  at,
}: {
  branch: BranchFolderRef;
  level: Level;
  at: FileRows;
}) {
  return <>{folderItems(branch, level, at)}</>;
}

function folderItems(branch: BranchFolderRef, level: Level, at: FileRows): ReactNode[] {
  const line = (what: string) => `${fileRow(branch)}#${what}`;
  if ("pending" in level)
    return [
      <TreePending key="pending" className="pending">
        <LoaderCircle className="node-icon spinning" />
        <span>Reading…</span>
      </TreePending>,
    ];
  if ("trouble" in level)
    return [
      // A folder of a branch's tree that could not be read, said in the tree where its entries
      // would be: a row's own refusal, not a standing line about the window (folding the folder
      // and opening it again reads it again).
      <TreeLine key="trouble" id={line("trouble")} textValue={level.trouble} className="trouble">
        <TriangleAlert className="node-icon" />
        <span role="alert">{level.trouble}</span>
      </TreeLine>,
    ];
  const { children: entries, more, narrowed } = level;
  // What the branch changed and purlis names and does not read (#704), under its top only.
  const refused =
    branch.folder === "" ? (at.files.indexes.get(branchKey(branch))?.status.refused ?? []) : [];
  const named = namedNotRead(refused, at.files.filter);
  const namedLines = [
    ...named.drawn.map((one) => (
      <TreeLine
        key={`named-${one.path}`}
        id={line(`named:${one.path}`)}
        textValue={namedNotReadSaid(one, MARK_SAID)}
        className="none"
        data-mark={one.mark}
        data-testid="explorer-refused-row"
      >
        <NamedNotReadWords one={one} said={MARK_SAID} />
      </TreeLine>
    )),
    ...(named.past > 0
      ? [
          <TreeLine
            key="named-more"
            id={line("named-more")}
            textValue={`${named.past.toLocaleString("en")} more named and not read`}
            className="none"
          >
            <span>{`${named.past.toLocaleString("en")} more named and not read`}</span>
          </TreeLine>,
        ]
      : []),
  ];
  const unwatched = at.files.changedOnly ? undefined : unwatchedHere(at.files.reads, branch);
  const unwatchedSaid =
    unwatched === "branch"
      ? "Changes on disk are not shown yet: purlis watches this branch again once it can read it."
      : "Changes on disk in this folder are not shown: the system would not watch it. Close and open it to try again.";
  const note =
    unwatched === undefined
      ? []
      : [
          <TreeLine
            key="unwatched"
            id={line("unwatched")}
            textValue={unwatchedSaid}
            className="none"
            data-testid="unwatched"
          >
            <span>{unwatchedSaid}</span>
          </TreeLine>,
        ];
  if (entries.length === 0 && more === 0) {
    // A folder the filter emptied says nothing: the rows that match say where to look.
    if (narrowed) return [...note, ...namedLines];
    // A branch whose only changes are paths it names and does not read did change something.
    const nothing = !(at.files.changedOnly && refused.length > 0);
    const said = at.files.changedOnly && branch.folder === "" ? "Nothing changed" : "Nothing here";
    return [
      ...note,
      ...(nothing
        ? [
            <TreeLine key="nothing" id={line("nothing")} textValue={said} className="none">
              <span>{said}</span>
            </TreeLine>,
          ]
        : []),
      ...namedLines,
    ];
  }
  const rows = entries.map((entry) => {
    const path = joined(branch.folder, entry.name);
    const here = { ...branch, folder: path };
    if (entry.expands)
      return (
        <FilesRow
          key={`d${entry.name}`}
          branch={here}
          name={entry.name}
          always={entry.always}
          at={at}
          data-ignored={entry.ignored ? "" : undefined}
        />
      );
    if (entry.workspace !== undefined)
      return (
        <WorkspaceRow
          key={`w${entry.name}`}
          here={here}
          name={entry.name}
          of={entry.workspace}
          at={at}
        />
      );
    return <FileRow key={`f${entry.name}`} branch={branch} entry={entry} at={at} />;
  });
  // A folder of tens of thousands of generated files is drawn as its first entries; the rest
  // are counted, never drawn.
  const counted = `${more.toLocaleString("en")} ${at.files.changedOnly ? "more changes" : "more"} not shown`;
  return [
    ...note,
    ...rows,
    ...(more > 0
      ? [
          <TreeLine key="more" id={line("more")} textValue={counted} className="none">
            <span>{counted}</span>
          </TreeLine>,
        ]
      : []),
    ...namedLines,
  ];
}

/**
 * **The first level of a file tree whose top is no row** (the *Files* section of one branch, the
 * plane root's, a file tab's): the top folder's own rows, or the line that says it is still
 * being read, in the tree's own words.
 */
export function TopItems({
  top,
  level,
  reading,
  at,
}: {
  top: BranchFolderRef;
  level: Level;
  reading: string;
  at: FileRows;
}) {
  if (!("pending" in level)) return <FolderItems branch={top} level={level} at={at} />;
  return (
    <TreePending className="pending">
      <LoaderCircle className="node-icon spinning" />
      <span>{reading}</span>
    </TreePending>
  );
}

/**
 * A workspace's repo, at the plane root (issue 1916, ruling F2): one row with the workspace's
 * mark, which selects that workspace's tab and never opens here.
 */
function WorkspaceRow({
  here,
  name,
  of,
  at,
}: {
  here: BranchFolderRef;
  name: string;
  of: string;
  at: FileRows;
}) {
  const id = fileRow(here);
  return (
    <TreeRow
      // Its own key, never a folder's: the same path drawn as a folder before the core said
      // whose repo it is is another row.
      id={`${id}#workspace`}
      textValue={`${name}, workspace ${of}`}
      className="file-node"
      data-row={id}
      data-workspace={of}
      tip={`A repo of workspace ${of}: its files are on that workspace's tab`}
      onAction={() => at.onGoToWorkspace?.(of)}
      content={
        <>
          <Folders className="node-icon" />
          <span className="spot-name">{name}</span>{" "}
          <span className="spot-what ui-text-secondary">{`workspace ${of}`}</span>
        </>
      }
    />
  );
}

/** A file's row: it opens in its file tab, unless purlis will not open it, which it says. */
function FileRow({ branch, entry, at }: { branch: BranchFolderRef; entry: Child; at: FileRows }) {
  const path = joined(branch.folder, entry.name);
  const id = fileRow({ ...branch, folder: path });
  const refused = entry.refused;
  // Where its preview was last read, so *Open in your editor* opens there (#1143).
  const line = useLastRead(at.plane, at.place, path);
  const touchedBy = at.files.touching?.get(branchKey(branch))?.get(path);
  // Refused and linked files keep their own marks: what they say is that this row is not an
  // ordinary file, which matters more than what kind of file it would be.
  const Mark = refused !== undefined ? FileX : entry.kind === "link" ? FileSymlink : null;
  const picked = pickOf(at, path, false, refused);
  const press = usePickingPress(at, picked, () => {
    if (refused === undefined) at.onOpenFile?.(at.place, path);
  });
  return (
    <TreeRow
      {...press}
      {...(at.shown === undefined ? pickedAs(at, picked) : { chosen: path === at.shown })}
      id={id}
      textValue={rowSaid(entry.name, touchedBy, entry.marked, false, refused)}
      className="file-node"
      data-row={id}
      data-refused={refused !== undefined || undefined}
      data-ignored={entry.ignored ? "" : undefined}
      data-mark={entry.marked?.mark}
      unavailable={refused !== undefined}
      // Why it does not open is said on the keyboard too; its path, which only repeats the row,
      // is the pointer's alone, so no arrow step pops it and no Esc is spent on it.
      tip={refused ?? path}
      tipToPointer={refused === undefined}
      menu={fileMenu({ on: "file", at: pathOn(at, path), kind: entry.kind, refused }, at, line)}
      carries={carriedBy(at, path, false, refused)}
      content={
        <>
          {Mark === null ? (
            <FileIcon symbol={iconFor(at.icons, { name: entry.name, folder: false })} />
          ) : (
            <Mark className="node-icon" />
          )}
          <span className="spot-name">{entry.name}</span> <TouchMark names={touchedBy} />
          <ChangeBadge marked={entry.marked} folder={false} />
          {/* Why it does not open, said beside it — an ignored file's too, which is drawn only
              once the operator asked to see what git ignores. */}
          {refused !== undefined && <span className="spot-what ui-text-secondary">{refused}</span>}
        </>
      }
    />
  );
}

/**
 * **The folders open in a file tree, as the tree family names them**: each folder row whose
 * fold is open, whatever it holds yet (a folder still being read holds its "Reading…" line), and
 * every folder of *Changed only*'s, which is always open.
 */
export function openFileRows(rows: readonly Row[]): string[] {
  return rows.filter((row) => row.fold?.open ?? row.parents).map((row) => row.id);
}

/**
 * **A fold the tree family asks for** (→, ←, a twist pressed), handed to `fold` by the row's
 * fold key: each folder row whose open state `keys` changes.
 */
export function foldFileRows(
  rows: readonly Row[],
  fold: (key: string, open: boolean) => void,
): (keys: Set<Key>) => void {
  return (keys) => {
    for (const row of rows)
      if (row.fold !== undefined && keys.has(row.id) !== row.fold.open)
        fold(row.fold.key, keys.has(row.id));
  };
}

/** Whether an entry is a folder that opens onto what it holds: one charter does not refuse. */
function expands(entry: FolderEntry): boolean {
  return entry.kind === "folder" && entry.refused === null;
}

/** The entries drawn: every one when ignored files are shown, else those git does not ignore. */
export function shown(entries: readonly FolderEntry[], showIgnored: boolean): FolderEntry[] {
  return showIgnored ? [...entries] : entries.filter((entry) => !entry.ignored);
}

/** Whether `ref` is the topmost open folder of its branch that the core is not watching for
 *  changes, and why, as the core says it: `"folder"` when the platform would not watch it
 *  (#1160), else `"branch"`, as when the reader has not found the branch yet (#1727). Undefined
 *  when it is watched, or under a folder that already says so: the tree says it once, where the
 *  unwatched part begins, and not on every open folder under it. */
function unwatchedHere(
  reads: ReadonlyMap<string, FolderRead>,
  ref: BranchFolderRef,
): "branch" | "folder" | undefined {
  if (reads.get(folderKey(ref))?.unwatched !== true) return undefined;
  if (ref.folder !== "") {
    const slash = ref.folder.lastIndexOf("/");
    const parent = slash === -1 ? "" : ref.folder.slice(0, slash);
    if (reads.get(folderKey({ ...ref, folder: parent }))?.unwatched === true) return undefined;
  }
  return reads.get(folderKey(ref))?.refused === true ? "folder" : "branch";
}

/** A folder's path and a name in it, as a path inside the branch. */
function joined(folder: string, name: string): string {
  return folder === "" ? name : `${folder}/${name}`;
}

/**
 * The folders the operator opened that are drawn: each one under one of `tops` — the folders
 * the *Files* section or the cockpit draws — whose every folder above it is open too. These are
 * what is read and watched: a folder inside a closed one is neither, and nor is a branch's that
 * is not drawn.
 */
function openFolders(
  workspace: string | undefined,
  tops: readonly BranchFolderRef[],
  expanded: ReadonlySet<string>,
): BranchFolderRef[] {
  if (workspace === undefined) return [];
  const out: BranchFolderRef[] = [];
  const walk = (ref: BranchFolderRef) => {
    if (!expanded.has(fileFold(workspace, ref))) return;
    out.push(ref);
    for (const key of expanded) {
      const child = childOf(key, workspace, ref);
      if (child !== undefined) walk(child);
    }
  };
  for (const top of tops) walk(top);
  return out;
}

/** The folder a fold key names when it is a folder directly inside `parent`. */
export function childOf(
  key: string,
  workspace: string,
  parent: BranchFolderRef,
): BranchFolderRef | undefined {
  const prefix = fileFold(workspace, parent);
  const into = parent.folder === "" ? prefix : `${prefix}/`;
  if (!key.startsWith(into) || key === prefix) return undefined;
  const rest = key.slice(into.length);
  if (rest === "" || rest.includes("/")) return undefined;
  return { ...parent, folder: joined(parent.folder, rest) };
}

/** The workspace's own row, as a stop in the explorer's roving focus. */
const ROOT = "root";
const cloneRow = (repo: string) => `clone:${repo}`;
const pieceRow = (repo: string, piece: string) => `piece:${repo}/${piece}`;
/** A folded clone, by workspace as well as name: two workspaces can each clone `svc`. */
const foldKey = (workspace: string, repo: string) => `${workspace}/${repo}`;
/** A branch's folder, or one of its files, as a row. */
export const fileRow = (ref: BranchFolderRef) => `file:${folderKey(ref)}`;
/** What starts every opened folder's fold key, so `fold` knows it from a clone's. */
const FILE_FOLD = "files:";
/** An opened folder of a branch, by workspace as well: two workspaces can each clone `svc`. */
export const fileFold = (workspace: string, ref: BranchFolderRef) =>
  `${FILE_FOLD}${workspace}\u0000${folderKey(ref)}`;

/** The branch the explorer is focused on, while the workspace still lists it: its name as a
 *  branch, and as the explorer names branches. */
/** What the explorer is narrowed to: a branch's folder, or a repo's own (`piece` null, #1152).
 *  `name` is what its tree is called, and `on` the branch its header names. */
type Cockpit = {
  ref: { repo: string; piece: string | null };
  name: string;
  on: string;
  path: string | undefined;
};

/**
 * The cockpit to draw for `focus` (FM-5), or none: a focus on a branch of this workspace that
 * still stands by {@link focusStands}, the rule the window reads too.
 */
function cockpitOf(
  workspace: string | undefined,
  state: WorkspaceState,
  focus: Place | undefined,
): Cockpit | undefined {
  if (workspace === undefined || focus === undefined || focus.workspace !== workspace) return;
  const stands = focusStands(state, focus);
  if (stands === undefined) return;
  // A repo's own folder (#1152): named by the repo, its header by the branch it has checked out.
  if (focus.piece === null)
    return {
      ref: { repo: focus.repo, piece: null },
      name: focus.repo,
      on: state.repos?.repos.find((one) => one.name === focus.repo)?.branch ?? "",
      path: state.panels?.paths[focus.repo],
    };
  const { piece } = stands;
  const name = piece?.branch || focus.piece;
  return { ref: { repo: focus.repo, piece: focus.piece }, name, on: name, path: piece?.path };
}

/** What a branch's row is called: its branch, or the folder's name when the folder has none
 *  checked out (a detached HEAD, or a branch git could not name). */
function branchOf(piece: Piece): string {
  return piece.branch || piece.piece;
}

/** A branch's row reads *`fix/login` in svc* (ADR 0072 §4, #1102): the branch, then the repo.
 *  A folder with no branch to name reads as the folder, and the tooltip holds its path. */
function BranchLabel({ repo, piece }: { repo: string; piece: Piece }) {
  if (!piece.branch) return <span className="spot-name">{piece.piece}</span>;
  return (
    <>
      <span className="spot-name">{piece.branch}</span>{" "}
      <span className="spot-in ui-text-secondary">in {repo}</span>
    </>
  );
}

/** The cockpit's rows (FM-5): the chats with a tab working in the branch, the line for the
 *  tasks there no row here counts, then its *Files* row and what it holds, each a top-level
 *  row of the cockpit's own tree. */
function cockpitTreeOf(workspace: string, cockpit: Cockpit, files: FilesOf): Row[] {
  const kids: TreeNode[] = [folderNode(workspace, { ...cockpit.ref, folder: "" }, "Files", files)];
  const rows: Row[] = [];
  kids.forEach((kid, i) => walkRows(kid, undefined, true, i, kids.length, rows));
  return rows;
}

/** One row of the tree, as the keyboard and a screen reader know it (#238). */
export type Row = {
  id: string;
  /** What type-ahead matches: the row's own name, which is its first word on screen. */
  name: string;
  level: number;
  parent: string | undefined;
  /** Its place among its siblings, from 1, and how many siblings there are. */
  posinset: number;
  setsize: number;
  /** A clone's fold, by {@link foldKey}, and whether it is open. Nothing on a row that cannot
   *  fold. */
  fold?: { key: string; open: boolean };
  /** Whether it is drawn: a row inside a folded clone is not, so it cannot be the stop and
   *  no key moves to it. */
  drawn: boolean;
  /** Whether it has children, drawn or not: a parent says `aria-expanded`, a leaf does not. */
  parents: boolean;
};

/** `shows` is whether its children are drawn when it is: not in a folded clone or a closed
 *  folder. A clone whose branches could not be listed has none to draw, and says why. */
type TreeNode = { id: string; name: string; fold?: Row["fold"]; kids: TreeNode[]; shows: boolean };

/** A branch's folder and, once opened, what it holds: the same walk `FilesRow` draws, through
 *  {@link levelOf}. A folder of *Changed only* is `always` open and does not fold. */
function folderNode(
  workspace: string,
  ref: BranchFolderRef,
  name: string,
  files: FilesOf,
  always = false,
): TreeNode {
  const key = fileFold(workspace, ref);
  const open = isOpen(files, ref, always);
  const level = open ? levelOf(files, ref) : undefined;
  const children = level !== undefined && "children" in level ? level.children : [];
  const kids = children.map((child): TreeNode => {
    const here = { ...ref, folder: joined(ref.folder, child.name) };
    return child.expands
      ? folderNode(workspace, here, child.name, files, child.always)
      : { id: fileRow(here), name: child.name, kids: [], shows: true };
  });
  return {
    id: fileRow(ref),
    name,
    fold: always ? undefined : { key, open },
    kids,
    shows: open,
  };
}

/**
 * The *Files* section's rows: the entries of the one folder it is of as its first level, when
 * `flat`; else each of `tops` — the repos' own folders — as a row that opens.
 */
function filesTreeOf(
  workspace: string,
  tops: readonly BranchFolderRef[],
  flat: boolean,
  files: FilesOf,
): Row[] {
  if (flat && tops.length === 1) return fileTreeRows(workspace, tops[0], files);
  const rows: Row[] = [];
  const kids = tops.map((top) => folderNode(workspace, top, top.repo, files));
  kids.forEach((kid, i) => walkRows(kid, undefined, true, i, kids.length, rows));
  return rows;
}

/** A node and everything under it, as rows, appended to `rows` in the order they are drawn. */
function walkRows(
  node: TreeNode,
  parent: Row | undefined,
  drawn: boolean,
  at: number,
  of: number,
  rows: Row[],
) {
  const row: Row = {
    id: node.id,
    name: node.name,
    level: (parent?.level ?? 0) + 1,
    parent: parent?.id,
    posinset: at + 1,
    setsize: of,
    fold: node.fold,
    drawn,
    parents: node.kids.length > 0,
  };
  rows.push(row);
  node.kids.forEach((kid, i) => walkRows(kid, row, drawn && node.shows, i, node.kids.length, rows));
}

/**
 * The rows of one branch's tree with its own folder as the root, which is not a row itself: what
 * the file tab draws (FM-2). Its folder's entries are the first level.
 */
export function fileTreeRows(workspace: string, top: BranchFolderRef, files: FilesOf): Row[] {
  const rows: Row[] = [];
  const root = folderNode(workspace, top, "", files);
  root.kids.forEach((kid, i) => walkRows(kid, undefined, root.shows, i, root.kids.length, rows));
  return rows;
}
