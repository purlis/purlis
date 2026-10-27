// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";
import {
  CircleCheck,
  CircleDashed,
  CircleSlash,
  CircleX,
  Clock,
  FileDiff,
  FolderGit2,
  GitBranch,
  GitFork,
  Hand,
  LoaderCircle,
  Puzzle,
  Server,
  ShieldOff,
  SkipForward,
  TriangleAlert,
} from "lucide-react";
import type { FactCell, FactColumn, Piece, RepoState } from "./bindings";
import { inertly } from "./sandboxBlocks";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { catalogueMenu, Menued } from "./Menus";
import type { Catalogued, Offer } from "./actions";
import type { WorkspaceState } from "./workspaceState";
import type { Cloning } from "./NotCloned";
import type { CloneState } from "./repoClones";
import { useArrived } from "./lib/arrived";
import { Tree, TreeRow } from "./ui/tree/Tree";

/**
 * The Changes view: what the focused workspace's repos are doing (ADR 0038 as amended
 * 2026-10-10, #1676).
 *
 * Repo git state, worktrees and pipelines, which used to be two sections of the right-hand
 * side. They moved because the right-hand side is what is asking for you and this is not: it
 * is what is true, and nothing in it can be pressed. **They were the bottom region until
 * #1676** (B-7): the bottom took height from the terminals, so it became a view on the left
 * side's activity bar, beside Chats, Explorer and Search, and the bottom of the window is the
 * terminals'. Its tab counts the files git has uncommitted ({@link uncommitted}).
 *
 * ## It is a tree, an editor's source-control view (#1701)
 *
 * In a side the bottom bar's table scrolled sideways past every column it had. So each repo is
 * a heading — the repo and the branch its checkout is on — and what is true of it are rows
 * under it, in the order a person asks: what is uncommitted, which branches are cut off it (and
 * under that row, each branch), and what its pipeline last said; then a row per column an
 * extension adds. Drawn as the tree family (`ui/tree`, #2911): each level one step in, its guide
 * a straight line, so the worktrees under a clone lost the bottom bar's elbows (#1682). **Nothing
 * folds** (`fixed`): a fold is a press, and every row is always drawn.
 *
 * **A row that names something is one line**, the explorer's rule, asked of both sides by the
 * operator: the view scrolls sideways rather than fold a name. A row that holds a SENTENCE — a
 * tree purlis could not read, a pipeline nobody fetched — still wraps, because held on one line
 * it would push the scroll out past everything else. `regions.e2e.ts` holds both halves, in a
 * real WebView, because jsdom lays nothing out. The one-line rule is `App.css`'s `.tree-row`, on
 * the row, and the sentence's own rule there still wins on the sentence, which it inherits from.
 *
 * ## The keyboard reads it; nothing in it is pressed
 *
 * **One Tab stop, and the tree's keys** (#1701): the WAI-ARIA "Tree View" pattern, the tree
 * family's — Up, Down, Home and End, Right into a row's first child, Left back to its parent, and
 * type-ahead on a row's words. The stop at rest is the tree, which hands the keyboard to its
 * first row, and is where ⌃⇧G puts it (`giveViewTheKeyboard` lands on a tree's stop). A row the keyboard is on is read,
 * never run: Enter and Space do nothing here. **A row is named by the whole of its words**, its
 * kind first (`changes: 3 changed`): a tree's row is named by what typing finds it by.
 *
 * **Read-only, and that is asserted rather than described** — `regions.e2e.ts` presses on
 * every control in here and expects to find none. A focusable row is not a control: it is what
 * lets the view be read and scrolled by keys. It is the one half of ADR 0038's reading ("the
 * bottom is where you read what is true and do not touch it", where "the bottom" is now this
 * view) that a test can hold.
 *
 * **A repo's rows have a context menu, and a menu is not a control** (charter-app#174). It is
 * the explorer's clone menu — `New tab in <repo>` and `Start new chats in <repo>` — drawn from
 * the same catalogue rows, now that the core says where a clone is (`Panels.paths`). It is on
 * the heading and on each row that is about the clone (its changes, its branches' count, its
 * pipeline, an extension's value); the menu is drawn in a portal outside this region, and none
 * of its rows touches the repo the region is reading: they are about where the next chat
 * starts. The keyboard opens it on the row it is on (Shift+F10 or the menu key, the tree
 * family's).
 *
 * **The worktree rows under each repo still have none, and that is a decision.** Their verbs
 * exist — `worktree.merge:<repo>/<piece>` and `worktree.remove:<repo>/<piece>`, drawn on the
 * explorer's rows — but putting `Remove worktree` under the pointer in the region ADR 0038
 * says is for reading is an amendment to ADR 0038, argued on its own, and not a defect fix.
 * A right-click there answers with nothing rather than with the browser's own menu, which
 * `useNoBrowserMenu` covers for the whole window.
 *
 * **A repo the workspace names and nobody cloned here has the explorer's Clone row as its menu**
 * (#1215), by the same argument as a clone's: a menu, drawn in a portal, and a clone adds a
 * folder beside the others without touching any repo this region reads. Its heading says what
 * became of the clone — under way, or failed in the core's words — as text, never a control.
 *
 * **Nothing here waits on a network.** The pipeline row is what a forge refresher last wrote
 * into `.charter/cache/glstate.json`; charter-app reads that file and never fetches. A row with
 * nothing to show says why, because a blank one reads as "green" to a person in a hurry.
 *
 * **The worktrees are a count and a list of branches, not the explorer's rows again.** They
 * are in two regions — ADR 0038 names that as the visible crack in its own rule — and the
 * least dishonest way to have them in both is to make each answer its own question: on the
 * left a piece is a thing you pick, here it is a branch that exists and may be unwired.
 */
export function ChangesView({
  workspace,
  state,
  offers,
  onPress,
  columns = [],
  cloning = new Map(),
}: {
  workspace: string | undefined;
  state: WorkspaceState;
  /**
   * The columns the extensions on in this project and workspace add (charter-app#340), filled
   * per repo from each one's facts file by the core — `extension_facts`, which never starts a
   * program. A row under each repo the file names: a value, never a control.
   */
  columns?: readonly FactColumn[];
  /** The catalogue by id, which is what a repo row's menu is drawn out of. */
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  /** What this window is cloning into the workspace (#1215): a repo not cloned here says
   *  what became of its clone, in words, never as a control. */
  cloning?: Cloning;
}) {
  const { panels, repos, pieces, piecesRefused, reading, trouble } = state;
  const names = panels?.repos ?? [];
  const absent = panels?.absent ?? [];
  if (workspace === undefined) {
    return (
      <section className="state-bar" aria-label="Repository state" data-testid="changes-view">
        <EmptyState
          headline="No workspace focused"
          body="Focus a workspace, and the branch, changes and pipeline of each of its repos are listed here."
          mark={GitFork}
          variant="panel"
          testid="changes-empty"
        />
      </section>
    );
  }

  const byName = new Map((repos?.repos ?? []).map((repo) => [repo.name, repo]));
  // Every row with rows under it is open, always: a repo's heading, and its Branches row when
  // branches are cut off it.
  const open = names.flatMap((name) =>
    (pieces[name] ?? []).length > 0 ? [repoRow(name), branchesRow(name)] : [repoRow(name)],
  );
  const at: At = { offers, onPress };

  return (
    <section className="state-bar" aria-label="Repository state" data-testid="changes-view">
      {trouble && (
        <Trouble offers={offers} onPress={onPress}>
          {trouble}
        </Trouble>
      )}
      {repos?.cache_refused && (
        <Trouble offers={offers} onPress={onPress}>
          {repos.cache_refused}
        </Trouble>
      )}

      {panels === undefined ? (
        <Pending>Reading the project…</Pending>
      ) : names.length === 0 && absent.length === 0 ? (
        <EmptyState
          headline="No repos in this workspace"
          body="Each repo the workspace names is listed here: the branch it is on, its changes, the branches cut off it and its pipeline."
          mark={FolderGit2}
          variant="panel"
          testid="changes-empty"
        />
      ) : (
        <Tree
          aria-label={`Repos of ${workspace}`}
          data-testid="changes-tree"
          size="sm"
          fixed
          expandedKeys={open}
        >
          {names.map((name) =>
            repoRows({
              name,
              state: byName.get(name),
              pieces: pieces[name],
              piecesRefused: piecesRefused[name],
              reading,
              columns,
              at,
            }),
          )}
          {absent.map((name) => (
            // Its menu is the explorer's Clone row (#1215): a menu, not a control, so the
            // region stays unpressable. Membership without a clone is said, because a repo the
            // workspace means to hold and nobody has cloned is not the same as one that is not
            // listed — and, while one is under way, what became of the clone.
            <TreeRow
              key={`absent-${name}`}
              id={absentRow(name)}
              textValue={`${name}: ${absentSaid(cloning.get(name))}`}
              className="repo-row absent tree-row"
              data-testid={`repo-${name}`}
              data-repo={name}
              menu={catalogueMenu({ on: "absent", repo: name }, offers, onPress)}
              content={
                <>
                  <FolderGit2 className="node-icon" />
                  <span className="repo">{name}</span>{" "}
                  <span className="none">{absentSaid(cloning.get(name))}</span>
                </>
              }
            />
          ))}
        </Tree>
      )}

      {/* A refusal is drawn, never swallowed: a row that is simply missing looks like a
          workspace with fewer repos than it has. */}
      {panels?.refused.map(([name, why]) => (
        <Trouble key={`refused-${name}`} offers={offers} onPress={onPress}>
          purlis will not read <code>{name}</code>: {why}
        </Trouble>
      ))}

      <p className="note">
        CI was last fetched by a refresher. charter-app reads this, never fetches it.
      </p>
    </section>
  );
}

/** What every row is drawn with: the clone menu's catalogue. */
type At = {
  offers: Catalogued;
  onPress: (offer: Offer) => void;
};

const repoRow = (repo: string) => `repo:${repo}`;
const absentRow = (repo: string) => `absent:${repo}`;
const changesRow = (repo: string) => `changes:${repo}`;
const branchesRow = (repo: string) => `branches:${repo}`;
const pieceRow = (repo: string, piece: string) => `piece:${repo}/${piece}`;
const ciRow = (repo: string) => `ci:${repo}`;
const runnerRow = (repo: string, pool: string, branch: string) =>
  `runner:${repo}:${pool}:${branch}`;
const leftOutRow = (repo: string) => `left-out:${repo}`;

/** How many left-out files a row names before it says how many more there are. */
const LEFT_OUT_NAMED = 5;

/** What the last send to the runner left out, in one line: the first few paths, drawn inertly,
 *  and how many more, the words agreeing with how many there are (#2046). */
export function leftOutSaid(paths: readonly string[]): string {
  const named = paths.slice(0, LEFT_OUT_NAMED).map(inertly).join(", ");
  const more = paths.length - LEFT_OUT_NAMED;
  const them = paths.length === 1 ? "it" : "them";
  return `Not sent to the runner, as a save would refuse ${them}: ${named}${more > 0 ? `, and ${more} more` : ""}`;
}
const factRow = (repo: string, column: FactColumn) =>
  `fact:${repo}/${column.extension}/${column.id}`;

/** What a repo that is not cloned here says in its row: that, or what its clone is doing. */
function absentSaid(clone: CloneState | undefined): string {
  switch (clone?.state) {
    case "waiting":
      return "waiting to be cloned";
    case "cloning":
      return "cloning…";
    case "cloned":
      return "cloned";
    case "failed":
      return `not cloned here — the clone failed: ${clone.said}`;
    default:
      return "not cloned here";
  }
}

/** A refusal, with the mark that says it is one. Lucide hides a nameless icon from a screen
 *  reader itself, so the alert reads exactly as it did before.
 *
 *  **Its menu is Read again** (#1244): the explorer's own row, so a refusal down here has its
 *  way out with the explorer hidden. A menu, not a control (charter-app#174), so the region
 *  stays unpressable. */
function Trouble({
  offers,
  onPress,
  children,
}: {
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  children: ReactNode;
}) {
  return (
    <Menued on={{ on: "refusal" }} offers={offers} onPress={onPress}>
      <p className="trouble" role="alert">
        <TriangleAlert className="node-icon" />
        <span>{children}</span>
      </p>
    </Menued>
  );
}

/** Something charter is still reading, which is a state no colour tells from a stopped one. */
function Pending({ children }: { children: ReactNode }) {
  return (
    <p className="pending">
      <LoaderCircle className="node-icon spinning" />
      <span>{children}</span>
    </p>
  );
}

/**
 * **One repo: its heading, and under it what is true of it**, in the order a person asks. Every
 * row about the clone carries the clone's menu (charter-app#174), the heading and the rows under
 * it alike; a branch cut off it carries none (see the docstring), so a right-click there answers
 * with nothing. Each row is named by the whole of its words, its kind first.
 */
function repoRows({
  name,
  state,
  pieces,
  piecesRefused,
  reading,
  columns,
  at,
}: {
  name: string;
  state: RepoState | undefined;
  pieces: Piece[] | undefined;
  piecesRefused: string | undefined;
  reading: boolean;
  columns: readonly FactColumn[];
  at: At;
}) {
  const unread = reading ? "reading…" : "not read";
  const cut = pieces ?? [];
  const menu = catalogueMenu({ on: "clone", repo: name }, at.offers, at.onPress);
  const head =
    state !== undefined && state.unreadable === null
      ? [headOf(state), state.upstream, gapOf(state)].filter(Boolean).join(", ")
      : "";
  return (
    <TreeRow
      key={name}
      id={repoRow(name)}
      textValue={head === "" ? name : `${name}, on ${head}`}
      className="repo-row tree-row"
      data-testid={`repo-${name}`}
      data-repo={name}
      menu={menu}
      content={
        <>
          <FolderGit2 className="node-icon" />
          <span className="repo">{name}</span>
          {state !== undefined && state.unreadable === null && (
            <span className="branch">
              {" "}
              <GitBranch className="node-icon" />
              <span>{headOf(state)}</span>
              {state.upstream && <span className="upstream">{state.upstream}</span>}
              {gapOf(state) && <span className="gap">{gapOf(state)}</span>}
            </span>
          )}
        </>
      }
    >
      <TreeRow
        id={changesRow(name)}
        textValue={`changes: ${state === undefined ? unread : (state.unreadable ?? dirtOf(state))}`}
        className="changes tree-row"
        data-testid={`changes-${name}`}
        data-repo={name}
        menu={menu}
        content={
          <>
            <FileDiff className="node-icon" />
            {state === undefined ? (
              <span className="pending">{unread}</span>
            ) : state.unreadable ? (
              // Never "clean". A tree charter could not read is the one thing this view must not
              // round down, because the round-down says everything is fine. A sentence, so it
              // wraps (`.branch.unreadable`).
              <span className="branch unreadable" role="alert">
                <TriangleAlert className="node-icon" />
                {state.unreadable}
              </span>
            ) : (
              <span className="dirt">{dirtOf(state)}</span>
            )}
          </>
        }
      />
      <TreeRow
        id={branchesRow(name)}
        textValue={`branches: ${worktreesSaid(pieces, piecesRefused)}`}
        className="worktrees tree-row"
        data-testid={`worktrees-${name}`}
        data-repo={name}
        menu={menu}
        content={
          <>
            <GitBranch className="node-icon" />
            <Worktrees pieces={pieces} refused={piecesRefused} />
          </>
        }
      >
        {/* The branches' own level, each its own line and guide (#1682). No menu on these rows
            (see the docstring). */}
        {cut.map((piece) => (
          <TreeRow
            key={piece.piece}
            id={pieceRow(name, piece.piece)}
            textValue={pieceSaid(piece)}
            className="piece tree-row"
            data-testid={`changes-piece-${name}-${piece.piece}`}
            data-repo={name}
            data-piece-of={name}
            content={<PieceWords piece={piece} />}
          />
        ))}
      </TreeRow>
      {(state?.on_a_runner ?? []).map((one) => (
        // A runner chat's branch, as this machine last fetched it (#2046): a name, so one
        // line, and how much of it is new here.
        <TreeRow
          key={`${one.pool}/${one.branch}`}
          id={runnerRow(name, one.pool, one.branch)}
          textValue={[
            `${inertly(one.branch)} on pool ${inertly(one.pool)}`,
            ...(one.ahead > 0 ? [aheadSaid(one.ahead)] : []),
          ].join(", ")}
          className="runner-branch tree-row"
          data-testid={`runner-${name}-${one.pool}-${one.branch}`}
          data-repo={name}
          menu={menu}
          content={
            <>
              <Server className="node-icon" />
              {/* A runner names its branches: each is drawn inertly, so none turns its line
                  around or hides what follows. */}
              <code className="branch">{inertly(one.branch)}</code>
              <span className="upstream">on pool {inertly(one.pool)}</span>
              {one.ahead > 0 && <span className="gap">{aheadSaid(one.ahead)}</span>}
            </>
          }
        />
      ))}
      {(state?.left_out_of_send ?? []).length > 0 && (
        // What the last send to the runner left out (#2046, the operator's ruling of
        // 2026-10-25): named by path, drawn inertly, so the person knows what stayed here.
        <TreeRow
          id={leftOutRow(name)}
          textValue={`not sent: ${leftOutSaid(state?.left_out_of_send ?? [])}`}
          className="runner-left-out tree-row"
          data-testid={`left-out-${name}`}
          data-repo={name}
          menu={menu}
          content={
            <>
              <ShieldOff className="node-icon" />
              <span className="upstream">{leftOutSaid(state?.left_out_of_send ?? [])}</span>
            </>
          }
        />
      )}
      <TreeRow
        id={ciRow(name)}
        textValue={`pipeline: ${state === undefined ? unread : ciSaid(state)}`}
        className="ci tree-row"
        data-testid={`ci-${name}`}
        data-repo={name}
        menu={menu}
        content={
          state === undefined ? (
            <span className="pending">{unread}</span>
          ) : (
            <CiWords state={state} />
          )
        }
      />
      {columns.map((column) => {
        const cell = column.cells.find((one) => one.repo === name);
        // A repo the facts file named no value for has no row — never another repo's value.
        if (cell === undefined) return null;
        return (
          <TreeRow
            key={`${column.extension}/${column.id}`}
            id={factRow(name, column)}
            textValue={`${column.title}: ${cell.value}${cell.stale ? `, ${ago(cell.age_seconds)}` : ""}`}
            className={cell.stale ? "fact stale tree-row" : "fact tree-row"}
            data-testid={`fact-${column.extension}-${column.id}-${name}`}
            data-repo={name}
            menu={menu}
            // Where the value comes from, beside the row: under the pointer and on the keyboard.
            tip={`From the extension ${column.extension}`}
            content={<FactOf column={column} cell={cell} />}
          />
        );
      })}
    </TreeRow>
  );
}

/** How many commits of a runner's branch this machine has not fetched. */
function aheadSaid(ahead: number): string {
  return `${ahead} ${ahead === 1 ? "commit" : "commits"} not on this machine`;
}

/** A branch cut off a clone, in words: its name, its branch where that says more, its state. */
function pieceSaid(piece: Piece): string {
  return [
    piece.piece,
    ...(piece.branch && piece.branch !== piece.piece ? [piece.branch] : []),
    ...(piece.stale ? ["stale"] : piece.wired ? [] : ["unwired"]),
    ...(piece.unread ? [`changes unreadable: ${piece.unread}`] : []),
  ].join(", ");
}

/** A branch cut off a clone, as its row draws it. */
function PieceWords({ piece }: { piece: Piece }) {
  return (
    <>
      <GitBranch className="node-icon" />
      <span className="piece-name">{piece.piece}</span>
      {/* The branch only when it says something the name does not. charter cuts a piece on a
          branch of its own name by default, and `perf perf` down a whole column is the same
          word twice on every row. */}
      {piece.branch && piece.branch !== piece.piece && (
        <>
          {" "}
          <code className="branch">{piece.branch}</code>
        </>
      )}
      {/* The same two states the count above totals, said here of the one tree they are true
          of. A total answers "is anything wrong in this clone"; a row answers "which one". */}
      {piece.stale ? (
        <>
          {" "}
          <span className="label stale">stale</span>
        </>
      ) : (
        !piece.wired && (
          <>
            {" "}
            <span className="label unwired">unwired</span>
          </>
        )
      )}
      {/* Never counted as clean: a folder purlis could not read says why, in the core's
          sentence (#1476), as a clone's row does. A sentence, so it wraps. */}
      {piece.unread && (
        <>
          {" "}
          <span className="branch unreadable">
            <TriangleAlert className="node-icon" />
            changes unreadable: {piece.unread}
          </span>
        </>
      )}
    </>
  );
}

/** One extension's value for one repo, named by its column. A stale value is dimmed and says
 *  how old it is. */
function FactOf({ column, cell }: { column: FactColumn; cell: FactCell }) {
  return (
    <>
      <Puzzle className="node-icon" />
      <span className="stamp">{column.title}</span> {cell.value}
      {cell.stale && <span className="stamp"> · {ago(cell.age_seconds)}</span>}
    </>
  );
}

/** What {@link Worktrees} says, in words. */
function worktreesSaid(pieces: Piece[] | undefined, refused: string | undefined): string {
  if (refused !== undefined) return `branches unreadable: ${refused}`;
  if (pieces === undefined) return "asking git…";
  if (pieces.length === 0) return "no branches";
  const stale = pieces.filter((piece) => piece.stale).length;
  const unwired = pieces.filter((piece) => !piece.wired && !piece.stale).length;
  return [
    `${pieces.length} ${pieces.length === 1 ? "branch" : "branches"}`,
    ...(unwired > 0 ? [`${unwired} unwired`] : []),
    ...(stale > 0 ? [`${stale} stale`] : []),
  ].join(", ");
}

/** What git says about this clone's pieces, in one phrase. */
function Worktrees({ pieces, refused }: { pieces: Piece[] | undefined; refused?: string }) {
  // Never "no branches". A listing charter could not run says so, for the same reason an
  // unreadable tree is never drawn as clean, and says why in the core's own sentence (#1476).
  if (refused !== undefined) return <span className="none">branches unreadable: {refused}</span>;
  if (pieces === undefined)
    return (
      <span className="pending">
        <LoaderCircle className="node-icon spinning" />
        branches: asking git…
      </span>
    );
  if (pieces.length === 0) return <span className="none">no branches</span>;
  const stale = pieces.filter((piece) => piece.stale).length;
  const unwired = pieces.filter((piece) => !piece.wired && !piece.stale).length;
  return (
    <>
      <span className="count">
        {pieces.length} {pieces.length === 1 ? "branch" : "branches"}
      </span>
      {/* The two states that change what starting a chat in one would mean. Counted here
          rather than listed: the row a person acts on is the explorer's. */}
      {unwired > 0 && <span className="label unwired"> {unwired} unwired</span>}
      {stale > 0 && <span className="label stale"> {stale} stale</span>}
    </>
  );
}

/**
 * The mark for each of the seven words a pipeline may be in.
 *
 * `CI_STATES` in `crates/purlis-core/src/cistate.rs` is the closed list — both forges map
 * their own vocabulary onto it — so this is exhaustive rather than a guess, and anything the
 * cache holds that is not one of the seven gets the dashed circle, which is what charter
 * already draws for "there is a fetch here and it names nothing".
 *
 * **`running` and `pending` are the ones that move.** That is the whole of the operator's
 * "show pipelines with animation": those two are the states where nothing else on the row
 * distinguishes *this is happening now* from *this stopped and nobody said so* — amber and
 * the word "running" are equally true of a job that died an hour ago. Every other mark is a
 * settled answer and sits still, because motion beside a settled answer is only something to
 * look at.
 *
 * **And the two move differently, because they are different claims** (M7.2). `running` spins:
 * work is being done. `pending` is a clock that breathes: the run is alive and queued, and
 * nothing is being done yet. Both used to spin, which drew a queued pipeline as a working one —
 * the same lie the paragraph above is about, told the other way round.
 *
 * **A mark that arrives at an answer settles into it, once.** A pipeline that finishes while
 * the operator is looking has its new mark drawn in over `duration.settle`; one that was already
 * finished when the row was drawn is simply there (`useArrived`). Every one of these motions is
 * a theme token, and every one of them stops under `prefers-reduced-motion` in the one place
 * the motion layer handles it (`src/theme/motion.ts`); the word and the shape do not.
 */
const CI_MARK: Record<string, { Mark: typeof CircleCheck; moving?: "spinning" | "breathing" }> = {
  success: { Mark: CircleCheck },
  failed: { Mark: CircleX },
  running: { Mark: LoaderCircle, moving: "spinning" },
  pending: { Mark: Clock, moving: "breathing" },
  manual: { Mark: Hand },
  canceled: { Mark: CircleSlash },
  skipped: { Mark: SkipForward },
};

/** What {@link CiWords} says, in words. */
function ciSaid(state: RepoState): string {
  if (state.ci)
    return [
      state.ci,
      ...(state.change !== null ? [`${state.sigil ?? "#"}${state.change}`] : []),
      ago(state.fetched_seconds_ago),
    ].join(", ");
  if (state.not_fetched) return `not fetched — ${state.not_fetched}`;
  return `no pipeline recorded, ${ago(state.fetched_seconds_ago)}`;
}

function CiWords({ state }: { state: RepoState }) {
  const arrived = useArrived(state.ci);
  if (state.ci) {
    const { Mark, moving } = CI_MARK[state.ci] ?? { Mark: CircleDashed };
    const motion = moving ?? (arrived ? "settling" : undefined);
    return (
      <span className={`ci-state ci-${state.ci}`}>
        <Mark className={motion ? `node-icon ${motion}` : "node-icon"} />
        {state.ci}
        {state.change !== null && (
          <span className="change">
            {" "}
            {state.sigil ?? "#"}
            {state.change}
          </span>
        )}
        <span className="stamp"> · {ago(state.fetched_seconds_ago)}</span>
      </span>
    );
  }
  if (state.not_fetched) return <span className="none">not fetched — {state.not_fetched}</span>;
  // An entry inside the window that names no pipeline. The cache cannot tell "there is none"
  // from "the call failed", so neither can this — but it is still a fetch, with an age.
  return (
    <span className="none">
      <CircleDashed className="node-icon" />
      no pipeline recorded · {ago(state.fetched_seconds_ago)}
    </span>
  );
}

/** Where HEAD is, in words. */
function headOf(state: RepoState): string {
  if (state.detached !== null) return `detached at ${state.detached || "an unnamed commit"}`;
  if (state.branch === null) return "no branch";
  return state.unborn ? `${state.branch} (no commits yet)` : state.branch;
}

/**
 * **The files git has uncommitted in the focused workspace's clones and their branch folders**:
 * what the Changes tab counts (B-8, #1676, #1718), changed and untracked alike, as
 * {@link dirtOf} says them per repo and each branch's row counts its own folder. A tree purlis
 * could not read adds nothing: its count means "not known", and the view says so on its row.
 */
export function uncommitted(state: WorkspaceState): number {
  const clones = (state.repos?.repos ?? [])
    .filter((repo) => repo.unreadable === null)
    .reduce((sum, repo) => sum + repo.tracked + repo.untracked, 0);
  const branches = Object.values(state.pieces)
    .flat()
    .reduce((sum, piece) => sum + (piece.uncommitted ?? 0), 0);
  return clones + branches;
}

/**
 * **How far the focused workspace's clones are from their upstreams, once one is behind**: what
 * the Repos icon draws (P-9, #1739), the commits behind and ahead summed over the clones. Nothing
 * while no clone is behind, however far ahead: being ahead is the person's own work waiting to be
 * pushed, which the icon does not need to say. A tree purlis could not read adds nothing, as in
 * {@link uncommitted}.
 */
export function reposGap(state: WorkspaceState): { behind: number; ahead: number } | undefined {
  const read = (state.repos?.repos ?? []).filter((repo) => repo.unreadable === null);
  const behind = read.reduce((sum, repo) => sum + repo.behind, 0);
  if (behind === 0) return undefined;
  return { behind, ahead: read.reduce((sum, repo) => sum + repo.ahead, 0) };
}

/** Whether there is anything uncommitted, counted the way git counts it. */
function dirtOf(state: RepoState): string {
  const parts: string[] = [];
  if (state.tracked > 0) parts.push(`${state.tracked} changed`);
  if (state.untracked > 0) parts.push(`${state.untracked} untracked`);
  return parts.length === 0 ? "clean" : parts.join(", ");
}

/** How far the branch is from its upstream, or nothing when it is level. */
function gapOf(state: RepoState): string {
  const parts: string[] = [];
  if (state.ahead > 0) parts.push(`${state.ahead} ahead`);
  if (state.behind > 0) parts.push(`${state.behind} behind`);
  return parts.join(", ");
}

/** An age a person reads. The bar says how old an answer is, because a two-hour-old
 *  "success" is not the same claim as one from a minute ago. */
export function ago(seconds: number | null): string {
  if (seconds === null) return "at an unknown time";
  if (seconds < 90) return `${seconds}s ago`;
  if (seconds < 5400) return `${Math.round(seconds / 60)}m ago`;
  return `${Math.round(seconds / 3600)}h ago`;
}
