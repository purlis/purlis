// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **PROTOTYPE, never merged** (#7998, map #7990, decision #7994): the narrowed file list a
 * branch click opens. Three variants of the sidebar's Explorer view, switched by the floating
 * ← → bar (or the `variant` arg, so a link keeps it), around the REAL `Explorer` and the real
 * `CockpitHeader`, over a window drawn just enough to click from: the rail, the sidebar, the
 * chat area and the Panel's Repos view, with 160 branch folders in two repos.
 *
 * **The one mode it proposes** (what every variant shares): a window holds one *narrowed*
 * branch, or none. It is today's `pickedSpot` and today's cockpit `focus` as one value: what
 * Files lists, where the next chat starts, and what the branch's header is of. A click on a
 * branch row in Repos sets it (opening the sidebar on Files if it was hidden or on another
 * view); a click on another branch replaces it (no stack); Esc, the variant's own way out, or the
 * workspace's row in Repos clears it. It is kept with the window's views (Machine tier,
 * device-bound, as `focus` is today), so a relaunch opens narrowed again.
 *
 * **What the variants disagree on**: how the scope is shown (a standing line, a chip in the
 * title, a frame round the tree), where the branch's facts and Merge/Done sit, what Esc puts
 * back, how a new chat says where it starts, and what happens when the folder goes.
 *
 * Actions are stubs: Merge, Done, New chat and the rest print what they would do in the
 * prototype's log beside the window. Nothing is written.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useMemo, useReducer, useRef, useState, type ReactNode } from "react";
import { useArgs } from "storybook/preview-api";
import { expect, waitFor } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  ChevronLeft,
  ChevronRight,
  CircleCheck,
  FolderOpen,
  Files,
  FolderX,
  GitBranch,
  GitMerge,
  MessageSquare,
  MessagesSquare,
  Plus,
  ScanSearch,
  X,
} from "lucide-react";
import { Explorer } from "./Explorer";
import { CockpitHeader, commitsSaid, useAheadBehind, type AheadBehindRead } from "./Cockpit";
import type { Offer } from "./actions";
import type { StatusRead } from "./branchStatus";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { FolderEntry, Panels as PanelsModel, Piece, PlaneId } from "./bindings";
import type { WorkspaceState } from "./workspaceState";
import { Panel, PanelTitle } from "./ui/panel/Panel";
import { HeadActions } from "./viewHead";
import { Button, IconButton, ToggleButton } from "./ui/button/Button";
import { ListBox, ListOption } from "./ui/list/List";
import { RowLabel, RowTrail } from "./ui/list/Row";
import { EmptyState } from "./ui/empty-state/EmptyState";
import "./Explorer.narrowed.prototype.css";

// ---------------------------------------------------------------------------------------------
// Data: two repos, 160 branch folders, long names, mixed states.

const PLANE = "/plane" as unknown as PlaneId;
const WS = "smart-ide";
const REPOS = ["purlis", "theme-lab"] as const;

type Branch = {
  repo: string;
  piece: string;
  /** The owning task's or chat's title, when one exists (#7994's rows). */
  title?: string;
  changes: number;
  ahead: number;
  behind: number;
};

const TITLES = [
  "Prototype the narrowed file list",
  "Repos view: the needs-you badge",
  "Clear build outputs of landed folders",
  "Fault suite B1 proves the negative",
  "Run grant: execline's programs refused",
  "Hibernate the longest-rested chat",
  "Hold chat starts while a prerequisite is open",
  "Guard: a path is no program",
  "Measure folder sizes with du",
  "Closed-PR list per repo, ETag-conditional",
  "Theme specimen: dark contrast floors",
  "Settings: the proxy page",
  "Inbox: Dismiss all with Undo",
  "Chat pane band rows",
  undefined,
  undefined,
];

/** A stable small number from a name. */
function hash(text: string): number {
  let h = 7;
  for (const c of text) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return h;
}

const slug = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");

const tail = (n: number) => (n.toString(36) + "k3m9q2zt").slice(0, 8);

const BRANCH_X: Branch = {
  repo: "purlis",
  piece: "prototype-the-narrowed-file-list-7998-2pk32emk",
  title: "Prototype the narrowed file list",
  changes: 4,
  ahead: 3,
  behind: 1,
};
const BRANCH_Y: Branch = {
  repo: "theme-lab",
  piece: "theme-specimen-dark-contrast-floors-7871-q8r2v6sd",
  title: "Theme specimen: dark contrast floors",
  changes: 12,
  ahead: 9,
  behind: 27,
};
const BRANCH_LONG: Branch = {
  repo: "purlis",
  piece:
    "when-a-person-clicks-a-branch-in-the-repos-view-the-sidebar-file-list-opens-narrowed-to-that-branch-folder-and-says-so-7998-zz81mm4q",
  changes: 1,
  ahead: 1,
  behind: 0,
};

const BRANCHES: Branch[] = (() => {
  const made: Branch[] = [BRANCH_X, BRANCH_Y, BRANCH_LONG];
  for (let i = 0; made.length < 160; i++) {
    const repo = i % 4 === 0 ? "theme-lab" : "purlis";
    const title = TITLES[i % TITLES.length];
    const words = title ?? `chat-${i}-scratch-branch`;
    const piece = `${slug(words)}-${7000 + i * 7}-${tail(hash(words) + i)}`;
    const h = hash(piece);
    made.push({
      repo,
      piece,
      title: i % 3 === 0 ? undefined : title,
      changes: h % 9,
      ahead: h % 14,
      behind: (h >> 4) % 40,
    });
  }
  return made;
})();

const byKey = new Map(BRANCHES.map((b) => [`${b.repo}/${b.piece}`, b]));
const branchOf = (n: Narrowed | undefined) =>
  n === undefined ? undefined : byKey.get(`${n.repo}/${n.piece}`);

/** A branch row's name (#7994): its title, or the branch without its 8-character id tail. */
function rowName(b: Branch): string {
  return b.title ?? b.piece.replace(/-[a-z0-9]{8}$/, "");
}

/** Middle-ellipsised to `most` characters, the whole in its tooltip. */
function middle(text: string, most: number): string {
  if (text.length <= most) return text;
  const keep = most - 1;
  return `${text.slice(0, Math.ceil(keep / 2))}…${text.slice(text.length - Math.floor(keep / 2))}`;
}

const pathOf = (b: { repo: string; piece: string }) =>
  `/plane/workspaces/${WS}/.worktrees/${b.repo}/${b.piece}`;

const entry = (name: string, folder = false): FolderEntry => ({
  name,
  kind: folder ? "folder" : "file",
  ignored: false,
  refused: null,
});
const TOP: FolderEntry[] = [
  entry("app", true),
  entry("crates", true),
  entry("docs", true),
  entry("tools", true),
  entry(".gitignore"),
  entry("Cargo.toml"),
  entry("CLAUDE.md"),
  entry("README.md"),
];
const UNDER: Record<string, FolderEntry[]> = {
  app: [entry("src", true), entry("package.json"), entry("vite.config.ts")],
  "app/src": [
    entry("ui", true),
    entry("App.tsx"),
    entry("Explorer.tsx"),
    entry("Cockpit.tsx"),
    entry("PlaneView.tsx"),
  ],
  docs: [entry("adr", true), entry("spec.md"), entry("plane-format.md")],
};
const OWN: FolderEntry[] = [
  { ...entry("memory", true), managed: true },
  entry("notes", true),
  { ...entry("sessions", true), managed: true },
  { ...entry("workspace.md"), managed: true },
];

/** The core, as the views ask it. A folder in `gone` is refused, as a removed one is. */
function core(gone: () => ReadonlySet<string>) {
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      const key = `${String(a.repo)}/${String(a.piece ?? "")}`;
      const b = byKey.get(key);
      if (cmd === "workspace_tree") {
        const folder = String(a.folder ?? "");
        return {
          entries: folder === "" ? OWN : [],
          more: 0,
          repos: folder === "" ? REPOS.map((name) => ({ name, inside_repos: true })) : [],
        };
      }
      if (cmd === "branch_tree") {
        if (gone().has(key)) throw new Error("The branch's folder is gone.");
        const folder = String(a.folder ?? "");
        return { entries: folder === "" ? TOP : (UNDER[folder] ?? []), more: 0 };
      }
      if (cmd === "repos_changed")
        return (a.repos as string[]).map((repo) => ({ repo, changes: null, trouble: null }));
      if (cmd === "branch_status") {
        const count = b?.changes ?? 0;
        return {
          refused: [],
          changes: [],
          folders: count === 0 ? [] : [{ folder: "", mark: "changed", count }],
          more: 0,
          base: "main",
        };
      }
      if (cmd === "branch_ahead_behind")
        return b === undefined
          ? { ahead: 0, behind: 0, base: null }
          : { ahead: b.ahead, behind: b.behind, base: "main" };
      if (cmd === "find_files") return { files: [], branches: 0, refused: [], partial: [] };
      return null;
    },
    { shouldMockEvents: true },
  );
}

function stateOf(gone: ReadonlySet<string>): WorkspaceState {
  const panels = {
    workspace: WS,
    repos: [...REPOS],
    paths: {},
    absent: [],
    refused: [],
    todos: [],
    done_todos: [],
    todos_refused: null,
    personas: [],
    persona: null,
    sessions: [],
    contributed: [],
  } as PanelsModel;
  const pieces: Record<string, Piece[]> = Object.fromEntries(
    REPOS.map((repo) => [
      repo,
      BRANCHES.filter((b) => b.repo === repo && !gone.has(`${b.repo}/${b.piece}`)).map(
        (b) =>
          ({
            piece: b.piece,
            path: pathOf(b),
            branch: b.piece,
            wired: true,
            stale: false,
            said: "",
            unclaimed: null,
            changed: b.changes,
          }) as unknown as Piece,
      ),
    ]),
  );
  return {
    panels,
    repos: {
      workspace: WS,
      repos: REPOS.map((name) => ({ name, branch: "main" })),
      cache_refused: null,
    } as unknown as WorkspaceState["repos"],
    pieces,
    piecesRefused: {},
    reading: false,
  };
}

// ---------------------------------------------------------------------------------------------
// The window's state: one narrowed branch or none, the sidebar, the chats, and a log.

type Variant = "A" | "B" | "C";
type Narrowed = { repo: string; piece: string };
type SideView = "files" | "chats" | "hidden";
type Chat = { id: number; title: string; where: string; branch?: Narrowed };

type Win = {
  narrowed?: Narrowed;
  /** What the sidebar showed before a branch click opened it on Files (variant A puts it back). */
  before?: SideView;
  side: SideView;
  chats: Chat[];
  gone: ReadonlySet<string>;
  /** A branch whose folder went while it was narrowed, as the variant says it. */
  wentAway?: Narrowed;
  /** Variant C: narrowed to the gone branch's repo instead. */
  atRepo?: string;
  said: string;
  log: string[];
};

/** What a relaunch reads back: the window's views file (Machine tier, device-bound). */
type Kept = { narrowed?: Narrowed; side: SideView };
let ON_DISK: Kept = { side: "files" };

type Act =
  | { type: "narrow"; to: Narrowed; variant: Variant }
  | { type: "widen"; how: string; variant: Variant }
  | { type: "side"; to: SideView }
  | { type: "newChat"; variant: Variant }
  | { type: "removeFolder"; variant: Variant }
  | { type: "log"; line: string }
  | { type: "dismissGone" };

const short = (n: Narrowed) => rowName(branchOf(n) ?? { ...n, changes: 0, ahead: 0, behind: 0 });

function reduce(win: Win, act: Act): Win {
  const log = (line: string) => [...win.log.slice(-11), line];
  switch (act.type) {
    case "narrow": {
      const same = win.narrowed?.repo === act.to.repo && win.narrowed.piece === act.to.piece;
      const opened = win.side !== "files";
      return {
        ...win,
        narrowed: act.to,
        atRepo: undefined,
        wentAway: undefined,
        // Only the first click from a hidden or other sidebar remembers what was there.
        before: win.narrowed === undefined ? (opened ? win.side : undefined) : win.before,
        side: "files",
        said: `Files shows only ${short(act.to)}. Esc for the whole workspace.`,
        log: log(
          same
            ? `click ${act.to.piece}: already narrowed, nothing changes`
            : `narrow to ${act.to.repo}/${act.to.piece}${opened ? ` (sidebar was ${win.side}: opened on Files)` : ""}`,
        ),
      };
    }
    case "widen": {
      if (win.narrowed === undefined && win.atRepo === undefined) return win;
      const back = act.variant === "A" && act.how.startsWith("Esc") ? win.before : undefined;
      return {
        ...win,
        narrowed: undefined,
        atRepo: undefined,
        before: undefined,
        side: back ?? win.side,
        said: "Files shows the whole workspace.",
        log: log(`widen by ${act.how}${back !== undefined ? ` (sidebar back to ${back})` : ""}`),
      };
    }
    case "side":
      return { ...win, side: act.to, log: log(`sidebar: ${act.to}`) };
    case "newChat": {
      const id = win.chats.length + 1;
      const n = win.narrowed;
      const where =
        n !== undefined
          ? pathOf(n)
          : win.atRepo !== undefined
            ? `/plane/workspaces/${WS}/repos/${win.atRepo}`
            : `${WS} (the workspace's folder)`;
      return {
        ...win,
        chats: [...win.chats, { id, title: `New chat ${id}`, where, branch: n }],
        log: log(`would start a chat in ${where}`),
      };
    }
    case "removeFolder": {
      const n = win.narrowed;
      if (n === undefined) return win;
      const gone = new Set(win.gone).add(`${n.repo}/${n.piece}`);
      const base = { ...win, gone, log: log(`the folder of ${n.piece} is removed (tidy)`) };
      if (act.variant === "A")
        return {
          ...base,
          narrowed: undefined,
          wentAway: n,
          said: `The folder of ${short(n)} is gone, so Files shows the whole workspace.`,
        };
      if (act.variant === "C")
        return {
          ...base,
          narrowed: undefined,
          atRepo: n.repo,
          wentAway: n,
          said: `The folder of ${short(n)} is gone. Files shows its repo, ${n.repo}.`,
        };
      return { ...base, wentAway: n, said: `The folder of ${short(n)} is gone.` };
    }
    case "dismissGone":
      return { ...win, wentAway: undefined, log: log("dismiss the gone line") };
    case "log":
      return { ...win, log: log(act.line) };
  }
}

// ---------------------------------------------------------------------------------------------
// The variants: what the sidebar draws around the real Explorer.

type SideProps = {
  variant: Variant;
  win: Win;
  dispatch: (act: Act) => void;
  /** The real Explorer, drawn by the shell with the narrowed branch as its pick. */
  explorer: ReactNode;
  /** The element the Explorer's head actions portal into. */
  setHead: (el: HTMLElement | null) => void;
  apart: AheadBehindRead | undefined;
  offers: ReadonlyMap<string, Offer>;
  onPress: (offer: Offer) => void;
};

function statusOf(b: Branch | undefined): StatusRead | undefined {
  if (b === undefined) return undefined;
  return {
    status: {
      refused: [],
      changes: [],
      folders: b.changes === 0 ? [] : [{ folder: "", mark: "changed", count: b.changes }],
      more: 0,
      base: "main",
    },
  } as unknown as StatusRead;
}

/** **A: a standing line above the tree, then the branch's header** (ahead/behind, Merge, Done). */
function SideA({ win, dispatch, explorer, setHead, apart, offers, onPress }: SideProps) {
  const n = win.narrowed;
  const b = branchOf(n);
  return (
    <>
      <div className="side-head" data-side="left">
        <PanelTitle className="side-title">{WS}</PanelTitle>
        <span className="view-actions" ref={setHead} />
      </div>
      {n !== undefined && b !== undefined && (
        <>
          <div className="np-a-line ui-text-meta" role="group" aria-label="Files is narrowed">
            <ScanSearch className="node-icon" aria-hidden="true" />
            <span className="ui-text-body">
              Only <strong>{rowName(b)}</strong> · {n.repo}
            </span>
            <span className="np-a-way">
              <span>Esc for the whole workspace</span>
              <Button
                size="sm"
                variant="ghost"
                onPress={() => dispatch({ type: "widen", how: "the line's button", variant: "A" })}
              >
                Whole workspace
              </Button>
            </span>
          </div>
          <div className="explorer np-a-head">
            <CockpitHeader
              name={b.piece}
              repo={n.repo}
              piece={n.piece}
              apart={apart}
              status={statusOf(b)}
              offers={offers}
              onPress={onPress}
            />
          </div>
        </>
      )}
      {win.wentAway !== undefined && n === undefined && (
        <div className="np-a-line ui-text-meta" data-gone role="status">
          <FolderX className="node-icon" aria-hidden="true" />
          <span className="ui-text-body">
            The folder of <strong>{short(win.wentAway)}</strong> is gone
          </span>
          <span className="np-a-way">
            <span>Files shows the whole workspace.</span>
            <Button size="sm" variant="ghost" onPress={() => dispatch({ type: "dismissGone" })}>
              Dismiss
            </Button>
          </span>
        </div>
      )}
      <div className="region-view">{explorer}</div>
    </>
  );
}

/** **B: the scope as a chip in the title, removed like a filter; the header a strip.** */
function SideB({ win, dispatch, explorer, setHead, apart, offers, onPress }: SideProps) {
  const n = win.narrowed;
  const b = branchOf(n);
  const gone = n !== undefined && win.gone.has(`${n.repo}/${n.piece}`);
  const action = (id: string, label: string, Icon: typeof GitMerge) => {
    const offer = offers.get(id);
    if (offer === undefined) return null;
    return (
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={label}
        tooltip={offer.title}
        isDisabled={gone}
        onPress={() => onPress(offer)}
        icon={<Icon className="node-icon" aria-hidden="true" />}
      />
    );
  };
  return (
    <div className="np-b" style={{ display: "contents" }}>
      <div className="side-head" data-side="left">
        <span className="np-b-title">
          <PanelTitle className="side-title">{WS}</PanelTitle>
          {n !== undefined && b !== undefined && (
            <>
              <ChevronRight className="node-icon" aria-hidden="true" />
              <span
                className="np-b-chip ui-text-meta"
                data-gone={gone || undefined}
                title={`${n.repo} · ${b.piece}`}
              >
                <GitBranch className="node-icon" aria-hidden="true" />
                <span className="np-b-chip-name">
                  <span className="np-said">Files is narrowed to </span>
                  {middle(rowName(b), 22)}
                  {gone && <span className="np-said"> (its folder is gone)</span>}
                </span>
                <IconButton
                  size="sm"
                  variant="ghost"
                  aria-label={`Show the whole workspace, not only ${rowName(b)}`}
                  tooltip="Esc"
                  onPress={() => dispatch({ type: "widen", how: "the chip's ×", variant: "B" })}
                  icon={<X className="node-icon" aria-hidden="true" />}
                />
              </span>
            </>
          )}
        </span>
        <span className="view-actions" ref={setHead} />
      </div>
      {n !== undefined && b !== undefined && (
        <div className="np-b-strip ui-text-meta" role="group" aria-label={`Branch ${b.piece}`}>
          {gone ? (
            <span>Folder gone</span>
          ) : (
            <span>
              {apart?.apart === undefined
                ? "Counting…"
                : `${commitsSaid(apart.apart.ahead)} ahead, ${commitsSaid(apart.apart.behind)} behind ${apart.apart.base ?? "base"}`}
              {" · "}
              {b.changes === 0 ? "No changes" : `${b.changes} changes`}
            </span>
          )}
          <span className="np-b-strip-actions">
            {action(`worktree.merge:${n.repo}/${n.piece}`, "Merge", GitMerge)}
            {action(`worktree.done:${n.repo}/${n.piece}`, "Done", CircleCheck)}
          </span>
        </div>
      )}
      <div className="region-view">
        {gone && b !== undefined ? (
          <EmptyState
            variant="panel"
            role="status"
            mark={FolderX}
            headline="This branch's folder is gone"
            body={`${rowName(b)} was put away while Files showed it. Its branch is kept.`}
            action={
              <Button
                size="sm"
                onPress={() => dispatch({ type: "widen", how: "the empty state", variant: "B" })}
              >
                Show the whole workspace
              </Button>
            }
          />
        ) : (
          explorer
        )}
      </div>
    </div>
  );
}

/** **C: the scope is a container**: a framed, tinted tree, with the branch's actions in its foot. */
function SideC({ win, dispatch, explorer, setHead, apart, offers, onPress }: SideProps) {
  const n = win.narrowed;
  const b = branchOf(n);
  const here = n === undefined ? [] : win.chats.filter((c) => c.branch?.piece === n.piece);
  const framed = (n !== undefined && b !== undefined) || win.atRepo !== undefined;
  const head = (
    <div className="side-head" data-side="left">
      <PanelTitle className="side-title">{WS}</PanelTitle>
      <span className="view-actions" ref={setHead} />
    </div>
  );
  if (!framed)
    return (
      <>
        {head}
        <div className="region-view">{explorer}</div>
      </>
    );
  const offer = (id: string) => offers.get(id);
  return (
    <>
      {head}
      <section
        className="np-c-frame"
        data-at={win.atRepo !== undefined ? "repo" : "branch"}
        aria-label={
          b !== undefined ? `Only the branch ${b.piece}` : `Only the repo ${win.atRepo ?? ""}`
        }
      >
        <div className="np-c-label">
          {b !== undefined ? (
            <GitBranch className="node-icon" aria-hidden="true" />
          ) : (
            <FolderX className="node-icon" aria-hidden="true" />
          )}
          <span>
            <span className="np-c-name ui-text-body ui-text-strong" title={b?.piece}>
              {b !== undefined ? rowName(b) : win.atRepo}
            </span>
            <span className="ui-text-meta">
              {b !== undefined
                ? `Only this branch · ${n?.repo}`
                : `${win.wentAway === undefined ? "" : short(win.wentAway)}'s folder is gone, so this is its repo`}
            </span>
          </span>
        </div>
        {here.length > 0 && (
          <ul className="np-c-chats ui-text-meta" aria-label="Chats in this branch">
            {here.map((c) => (
              <li key={c.id}>
                <MessageSquare className="node-icon" aria-hidden="true" />
                {c.title}
              </li>
            ))}
          </ul>
        )}
        <div className="region-view">{explorer}</div>
        <div className="np-c-foot">
          {b !== undefined && n !== undefined && (
            <>
              <span className="ui-text-meta">
                {apart?.apart === undefined
                  ? "Counting…"
                  : `${commitsSaid(apart.apart.ahead)} ahead, ${commitsSaid(apart.apart.behind)} behind ${apart.apart.base ?? "base"}`}
                {" · "}
                {b.changes === 0 ? "No changes" : `${b.changes} changes`}
              </span>
              <span className="np-c-foot-actions">
                {[
                  [`worktree.merge:${n.repo}/${n.piece}`, "Merge", GitMerge] as const,
                  [`worktree.done:${n.repo}/${n.piece}`, "Done", CircleCheck] as const,
                ].map(([id, label, Icon]) => {
                  const o = offer(id);
                  return o === undefined ? null : (
                    <Button
                      key={id}
                      size="sm"
                      onPress={() => onPress(o)}
                      icon={<Icon className="node-icon" aria-hidden="true" />}
                    >
                      {label}
                    </Button>
                  );
                })}
              </span>
            </>
          )}
          <Button
            size="sm"
            variant="ghost"
            shortcut="Esc"
            onPress={() => dispatch({ type: "widen", how: "the frame's foot", variant: "C" })}
          >
            Whole workspace
          </Button>
        </div>
      </section>
    </>
  );
}

const VARIANTS: Record<Variant, { name: string; Side: (p: SideProps) => ReactNode }> = {
  A: { name: "Standing line + branch header", Side: SideA },
  B: { name: "Scope chip in the title + strip", Side: SideB },
  C: { name: "Framed tree, actions in its foot", Side: SideC },
};
const ORDER: Variant[] = ["A", "B", "C"];

// ---------------------------------------------------------------------------------------------
// The window.

function offersFor(n: Narrowed | undefined): ReadonlyMap<string, Offer> {
  if (n === undefined) return new Map();
  const at = `${n.repo}/${n.piece}`;
  const row = (id: string, title: string): [string, Offer] => [
    id,
    { id, title, available: true, reason: "", does: { verb: "stub" } } as unknown as Offer,
  ];
  return new Map([
    row(`worktree.merge:${at}`, `Merge ${n.piece} into main`),
    row(`worktree.done:${at}`, `Mark ${n.piece} done`),
  ]);
}

function Window({
  variant,
  dispatch,
  win,
}: {
  variant: Variant;
  dispatch: (act: Act) => void;
  win: Win;
}) {
  const [head, setHead] = useState<HTMLElement | null>(null);
  const state = useMemo(() => stateOf(win.gone), [win.gone]);
  const n = win.narrowed;
  const apart = useAheadBehind(PLANE, WS, n, undefined);
  const offers = useMemo(() => offersFor(n), [n]);
  const reposRef = useRef<HTMLDivElement>(null);
  const Side = VARIANTS[variant].Side;

  // The narrowed branch is what Files is of: the Explorer's pick (today's `pickedSpot`).
  const spot =
    n !== undefined && !win.gone.has(`${n.repo}/${n.piece}`)
      ? { repo: n.repo, piece: n.piece, path: pathOf(n) }
      : win.atRepo !== undefined
        ? { repo: win.atRepo, path: `/plane/workspaces/${WS}/repos/${win.atRepo}` }
        : undefined;

  /** After a way back out, the keyboard lands on the branch's row in Repos. */
  const widen = (how: string) => {
    const was = n;
    dispatch({ type: "widen", how, variant });
    if (was !== undefined)
      requestAnimationFrame(() =>
        reposRef.current
          ?.querySelector<HTMLElement>(`[data-key="${was.repo}/${was.piece}"]`)
          ?.focus(),
      );
  };
  const onEsc = (event: React.KeyboardEvent, how: string) => {
    if (event.key !== "Escape" || event.defaultPrevented) return;
    if (n === undefined && win.atRepo === undefined) return;
    event.preventDefault();
    widen(how);
  };

  const explorer = (
    <HeadActions.Provider value={head}>
      <ChatsHere.Provider value={fixedChats(nothingKnown)}>
        <Explorer
          part="explorer"
          plane={PLANE}
          workspace={WS}
          state={state}
          chats={[]}
          spot={spot}
          onPick={(to) =>
            dispatch({ type: "log", line: `Explorer asked to pick ${JSON.stringify(to)}` })
          }
          offers={offers}
          onPress={(o) => dispatch({ type: "log", line: `would run ${o.id}` })}
          onReadAgain={() => {}}
          onOpenFile={(_, path) => dispatch({ type: "log", line: `would open ${path}` })}
          onSearchContents={() => dispatch({ type: "log", line: "would open Search" })}
        />
      </ChatsHere.Provider>
    </HeadActions.Provider>
  );

  const groups = REPOS.map((repo) => ({
    repo,
    rows: BRANCHES.filter((b) => b.repo === repo && !win.gone.has(`${b.repo}/${b.piece}`)),
  }));
  const selected = n === undefined ? "ws" : `${n.repo}/${n.piece}`;

  return (
    <div className="np-window" data-variant={variant}>
      <nav className="np-rail" aria-label="Views">
        <ToggleButton
          size="md"
          variant="ghost"
          aria-label="Files"
          isSelected={win.side === "files"}
          onChange={(on) => dispatch({ type: "side", to: on ? "files" : "hidden" })}
          icon={<Files className="node-icon" aria-hidden="true" />}
        />
        <ToggleButton
          size="md"
          variant="ghost"
          aria-label="Chats"
          isSelected={win.side === "chats"}
          onChange={(on) => dispatch({ type: "side", to: on ? "chats" : "hidden" })}
          icon={<MessagesSquare className="node-icon" aria-hidden="true" />}
        />
      </nav>

      <Panel
        variant="side"
        aria-label="Sidebar"
        role="region"
        className="np-side"
        hidden={win.side === "hidden"}
      >
        <div
          style={{ display: "flex", flexDirection: "column", flex: "1 1 0", minBlockSize: 0 }}
          onKeyDown={(e) => onEsc(e, "Esc")}
        >
          {win.side === "files" && (
            <Side
              variant={variant}
              win={win}
              dispatch={(act) => (act.type === "widen" ? widen(act.how) : dispatch(act))}
              explorer={explorer}
              setHead={setHead}
              apart={apart}
              offers={offers}
              onPress={(o) => dispatch({ type: "log", line: `would run ${o.id}` })}
            />
          )}
          {win.side === "chats" && (
            <>
              <div className="side-head" data-side="left">
                <PanelTitle className="side-title">Chats</PanelTitle>
              </div>
              <EmptyState
                variant="panel"
                headline="The Chats view"
                body="Stands for any other view."
              />
            </>
          )}
        </div>
      </Panel>

      <main className="np-main" aria-label="Chats">
        <div className="np-main-head">
          <ul className="np-tabs" aria-label="Open chats">
            {win.chats.length === 0 && <li className="ui-text-meta">No chats open.</li>}
            {win.chats.map((c) => (
              <li key={c.id} className="np-tab ui-text-meta" title={c.where}>
                <MessageSquare className="node-icon" aria-hidden="true" />
                <span className="ui-text-body">{c.title}</span>
                {/* B and C say on the tab where it started; A said it on the button. */}
                {variant !== "A" && c.branch !== undefined && (
                  <span className="np-tab-where">
                    <GitBranch className="node-icon" aria-hidden="true" /> {short(c.branch)}
                  </span>
                )}
              </li>
            ))}
          </ul>
          <Button
            size="sm"
            variant="primary"
            onPress={() => dispatch({ type: "newChat", variant })}
            tooltip={
              n === undefined ? "Starts in the workspace's folder" : `Starts in ${n.piece}'s folder`
            }
            icon={<Plus className="node-icon" aria-hidden="true" />}
          >
            {variant === "A" && n !== undefined
              ? `New chat in ${middle(short(n), 40)}`
              : "New chat"}
          </Button>
        </div>
        <p className="ui-text-meta" style={{ margin: 0 }}>
          {n !== undefined
            ? `The next chat starts in ${n.piece}'s folder.`
            : win.atRepo !== undefined
              ? `The next chat starts in ${win.atRepo}'s own folder.`
              : `The next chat starts in ${WS}'s own folder.`}
        </p>
      </main>

      <section className="np-panel" aria-label="Panel">
        <div className="np-panel-head">
          <FolderOpen className="node-icon" aria-hidden="true" />
          <PanelTitle>Repos</PanelTitle>
          <span className="ui-text-meta">{BRANCHES.length - win.gone.size} branch folders</span>
        </div>
        <div
          className="np-panel-body"
          ref={reposRef}
          onKeyDownCapture={(e) => onEsc(e, "Esc in Repos")}
        >
          <ListBox aria-label={WS} size="sm" onAction={() => widen("the workspace's row")}>
            <ListOption id="ws" textValue={`${WS}, the whole workspace`} data-key="ws">
              <RowLabel>{WS}</RowLabel>
              <RowTrail>
                {selected === "ws" ? "In Files · whole workspace" : "whole workspace"}
              </RowTrail>
            </ListOption>
          </ListBox>
          {groups.map(({ repo, rows }) => (
            <div key={repo}>
              <h3 className="np-group-head ui-text-meta ui-text-strong">{repo}</h3>
              <ListBox
                aria-label={`Branches of ${repo}`}
                size="sm"
                onAction={(key) => {
                  const [r, ...p] = String(key).split("/");
                  dispatch({ type: "narrow", to: { repo: r, piece: p.join("/") }, variant });
                }}
              >
                {rows.map((b) => (
                  <ListOption
                    key={`${b.repo}/${b.piece}`}
                    id={`${b.repo}/${b.piece}`}
                    data-key={`${b.repo}/${b.piece}`}
                    textValue={rowName(b)}
                    tip={`${b.piece}\n${pathOf(b)}`}
                  >
                    <RowLabel>{middle(rowName(b), 60)}</RowLabel>
                    <RowTrail>
                      {selected === `${b.repo}/${b.piece}`
                        ? "In Files"
                        : b.changes === 0
                          ? ""
                          : `${b.changes} changed`}
                    </RowTrail>
                  </ListOption>
                ))}
              </ListBox>
            </div>
          ))}
        </div>
      </section>

      <div className="np-said" role="status" aria-live="polite">
        {win.said}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// The prototype's chrome: the switcher, the scenario's steps, the state and the log.

function Switcher({ variant, onTo }: { variant: Variant; onTo: (v: Variant) => void }) {
  const step = (by: number) =>
    onTo(ORDER[(ORDER.indexOf(variant) + by + ORDER.length) % ORDER.length]);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const at = document.activeElement;
      // Only while nothing in the window holds the keyboard: a tree's arrows are its own.
      if (at !== document.body && !ref.current?.contains(at)) return;
      if (e.key === "ArrowLeft") step(-1);
      if (e.key === "ArrowRight") step(1);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });
  return (
    <div className="np-switcher" ref={ref} role="group" aria-label="Prototype variant">
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Previous variant"
        onPress={() => step(-1)}
        icon={<ChevronLeft className="node-icon" aria-hidden="true" />}
      />
      <span className="np-switcher-label ui-text-body ui-text-strong" aria-live="polite">
        {variant} · {VARIANTS[variant].name}
      </span>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Next variant"
        onPress={() => step(1)}
        icon={<ChevronRight className="node-icon" aria-hidden="true" />}
      />
    </div>
  );
}

type Scenario = {
  title: string;
  /** What to look at, one or two sentences. */
  look: string;
};

/** What each variant does differently, said beside the window. */
const VARIANT_SAYS: Record<Variant, string[]> = {
  A: [
    "Scope: a standing line above the tree, the whole name wrapped, never cut.",
    "Header: the real CockpitHeader (ahead/behind, changes, Merge, Done) under the line.",
    "Esc: widens AND puts the sidebar back as it was before the click (hidden, or Chats). The line's button only widens.",
    "New chat: the button says where (“New chat in …”).",
    "Folder gone: widens at once; the line turns into “The folder of … is gone” with Dismiss.",
  ],
  B: [
    "Scope: a chip after the workspace in the title, middle-ellipsised, full name in its tooltip; × removes it like a filter.",
    "Header: one strip, words then Merge and Done as icon buttons.",
    "Esc: widens; the sidebar stays on Files.",
    "New chat: the button says nothing; the chat's tab carries the branch.",
    "Folder gone: stays narrowed; the chip is struck, the tree an empty state with a way out.",
  ],
  C: [
    "Scope: the tree sits in a tinted frame named for the branch (two lines at most).",
    "Header: facts and Merge / Done / Whole workspace in the frame's foot.",
    "Esc: widens; the sidebar stays on Files.",
    "New chat: listed inside the frame, above the tree, and its tab carries the branch.",
    "Folder gone: narrows to the nearest folder still there, its repo, in a dashed frame.",
  ],
};

type PrototypeProps = {
  variant: Variant;
  scenario: Scenario;
  start?: Partial<Win>;
  onVariant: (v: Variant) => void;
};

/** A relaunch is a new session: nothing in memory survives it, only what was kept on disk. */
function Prototype(props: PrototypeProps) {
  const [launch, setLaunch] = useState(0);
  return (
    <Session key={launch} {...props} launch={launch} onRelaunch={() => setLaunch((n) => n + 1)} />
  );
}

function Session({
  variant,
  scenario,
  start,
  onVariant,
  launch,
  onRelaunch,
}: PrototypeProps & { launch: number; onRelaunch: () => void }) {
  const fresh = (): Win => ({
    side: ON_DISK.side,
    narrowed: ON_DISK.narrowed,
    chats: [],
    gone: new Set(),
    said: "",
    log: launch === 0 ? ["launch"] : [`relaunch #${launch}: read ${JSON.stringify(ON_DISK)}`],
    ...(launch === 0 ? start : {}),
  });
  const [win, dispatch] = useReducer(reduce, undefined, fresh);
  // Kept on every change, as the window's views are.
  useEffect(() => {
    ON_DISK = { narrowed: win.narrowed, side: win.side };
  }, [win.narrowed, win.side]);
  const kept = { narrowed: win.narrowed, side: win.side };
  return (
    <div className="np-stage">
      <Window variant={variant} win={win} dispatch={dispatch} />
      <aside className="np-notes" aria-label="Prototype notes">
        <section>
          <h2 className="ui-text-lg ui-text-strong">{scenario.title}</h2>
          <p className="ui-text-body">{scenario.look}</p>
          <div className="np-steps">
            <Button size="sm" onPress={() => dispatch({ type: "narrow", to: BRANCH_X, variant })}>
              Click branch X
            </Button>
            <Button size="sm" onPress={() => dispatch({ type: "narrow", to: BRANCH_Y, variant })}>
              Click branch Y
            </Button>
            <Button
              size="sm"
              onPress={() => dispatch({ type: "narrow", to: BRANCH_LONG, variant })}
            >
              Click the long one
            </Button>
            <Button
              size="sm"
              isDisabled={win.narrowed === undefined}
              onPress={() => dispatch({ type: "removeFolder", variant })}
            >
              Remove its folder
            </Button>
            <Button size="sm" onPress={onRelaunch}>
              Relaunch the app
            </Button>
          </div>
        </section>
        <section>
          <h3 className="ui-text-body ui-text-strong">
            {variant} · {VARIANTS[variant].name}
          </h3>
          <ul className="ui-text-meta">
            {VARIANT_SAYS[variant].map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
        <section>
          <h3 className="ui-text-body ui-text-strong">State</h3>
          <pre className="ui-text-meta" data-testid="np-state">
            {JSON.stringify(
              {
                narrowed: win.narrowed ?? null,
                sidebar: win.side,
                before: win.before ?? null,
                atRepo: win.atRepo ?? null,
                kept,
                chats: win.chats.map((c) => `${c.title} in ${c.where}`),
              },
              null,
              1,
            )}
          </pre>
        </section>
        <section>
          <h3 className="ui-text-body ui-text-strong">What it would do</h3>
          <pre className="ui-text-meta" data-testid="np-log">
            {win.log.join("\n")}
          </pre>
        </section>
      </aside>
      <Switcher variant={variant} onTo={onVariant} />
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// Stories: one per scenario; each walks its step in `play`, every variant behind the switcher.

type Args = { variant: Variant; scenario: Scenario; start?: Partial<Win> };

let GONE: ReadonlySet<string> = new Set();

const meta = {
  title: "Prototypes/Narrowed file list (#7998)",
  parameters: { layout: "fullscreen" },
  // A story function, where Storybook's hooks may be called: the bar writes the `variant` arg,
  // so the URL keeps it.
  render: function Render(args) {
    const [, setArgs] = useArgs<Args>();
    return <Prototype {...args} onVariant={(v) => setArgs({ variant: v })} />;
  },
  args: { variant: "A" },
  argTypes: { variant: { control: "inline-radio", options: ORDER } },
  beforeEach: () => {
    ON_DISK = { side: "files" };
    GONE = new Set();
    core(() => GONE);
    return () => clearMocks();
  },
  // Free play and the walks: no picture is drawn for a prototype.
  tags: ["!test"],
} satisfies Meta<Args>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The row of `b` in the Repos view. */
const rowOf = (root: HTMLElement, b: Branch) =>
  waitFor(() => {
    const row = root.querySelector<HTMLElement>(`[data-key="${b.repo}/${b.piece}"]`);
    if (row === null) throw new Error(`no row for ${b.piece}`);
    return row;
  });

export const FreePlay: Story = {
  args: {
    scenario: {
      title: "Free play",
      look: "Click any branch in the Repos view below the chats, or use the steps here. ← → (or the bar at the bottom) switches the variant and keeps the state.",
    },
  },
};

export const FromAHiddenSidebar: Story = {
  args: {
    scenario: {
      title: "The sidebar was hidden when the person clicked",
      look: "The sidebar was closed. The click on X opens it on Files, narrowed. Press Esc in it: in A the sidebar closes again, in B and C it stays open on the whole workspace.",
    },
    start: { side: "hidden" },
  },
  play: async ({ canvasElement, userEvent }) => {
    await userEvent.click(await rowOf(canvasElement, BRANCH_X));
    await waitFor(() =>
      expect(canvasElement.querySelector(".np-side")).not.toHaveAttribute("hidden"),
    );
  },
};

export const FromAnotherView: Story = {
  args: {
    scenario: {
      title: "The sidebar showed another view (Chats)",
      look: "The sidebar showed Chats. The click on X switches it to Files, narrowed. In A, Esc goes back to Chats.",
    },
    start: { side: "chats" },
  },
  play: async ({ canvasElement, userEvent }) => {
    await userEvent.click(await rowOf(canvasElement, BRANCH_X));
  },
};

export const FromXToY: Story = {
  args: {
    scenario: {
      title: "From branch X to branch Y",
      look: "X, then Y in the other repo. Y replaces X: no stack, so Esc goes to the whole workspace, never back to X. The header reads Y's counts again.",
    },
  },
  play: async ({ canvasElement, userEvent }) => {
    await userEvent.click(await rowOf(canvasElement, BRANCH_X));
    await userEvent.click(await rowOf(canvasElement, BRANCH_Y));
  },
};

export const WaysBackOut: Story = {
  args: {
    scenario: {
      title: "Esc and the other ways back out",
      look: "Narrowed to X, the keyboard in the file tree, then Esc. The ways out: Esc in Files; Esc in Repos; the variant's own control (A's line button, B's chip ×, C's foot button); and the workspace's own row at the top of Repos. Each lands the keyboard on X's row in Repos.",
    },
    start: { narrowed: BRANCH_X },
  },
  play: async ({ canvas, userEvent }) => {
    const row = await canvas.findByRole("row", { name: /^README\.md/ });
    row.focus();
    await userEvent.keyboard("{Escape}");
  },
};

export const ANewChatWhileNarrowed: Story = {
  args: {
    scenario: {
      title: "A new chat started while narrowed",
      look: "Narrowed to X, the person presses New chat. It starts in X's folder (the log says the path). A says so on the button before the press; B and C on the chat's tab after; C also lists it inside the frame.",
    },
    start: { narrowed: BRANCH_X },
  },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(await canvas.findByRole("button", { name: /^New chat/ }));
  },
};

export const Relaunch: Story = {
  args: {
    scenario: {
      title: "Relaunch: what is remembered",
      look: "Narrowed to Y, then the app relaunches. The narrowed branch and the sidebar's view are kept with the window's views (Machine tier, device-bound), so it opens narrowed again; the chats and the log are not. Press Relaunch again after a change to see it.",
    },
  },
  play: async ({ canvasElement, canvas, userEvent }) => {
    await userEvent.click(await rowOf(canvasElement, BRANCH_Y));
    await userEvent.click(canvas.getByRole("button", { name: "Relaunch the app" }));
  },
};

export const TheFolderDisappears: Story = {
  args: {
    scenario: {
      title: "The branch's folder disappears while narrowed",
      look: "Narrowed to X, and tidy removes X's folder (its branch is kept). A widens at once and says why; B stays and says the folder is gone, with a way out; C narrows to the nearest folder still there, X's repo.",
    },
    start: { narrowed: BRANCH_X },
  },
  play: async ({ canvas, userEvent }) => {
    await canvas.findByRole("row", { name: /^README\.md/ });
    GONE = new Set([`${BRANCH_X.repo}/${BRANCH_X.piece}`]);
    await userEvent.click(canvas.getByRole("button", { name: "Remove its folder" }));
  },
};

export const KeyboardOnly: Story = {
  args: {
    scenario: {
      title: "Keyboard only",
      look: "Into Repos, ↓ to a branch, Enter narrows: the keyboard stays in Repos and a screen reader hears “Files shows only …”. Shift+Tab reaches the sidebar. Esc widens from either and brings the keyboard back to the row, but in Repos the row's tooltip is up while the keyboard is on it, and the first Esc is the tooltip's: it takes two (a finding for the view's keyboard model).",
    },
  },
  play: async ({ canvasElement, userEvent }) => {
    const first = await rowOf(canvasElement, BRANCH_X);
    first.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvasElement.querySelector('[role="status"].np-said')).toHaveTextContent(
        /Files shows only/,
      ),
    );
  },
};

export const AVeryLongBranchName: Story = {
  args: {
    scenario: {
      title: "A very long branch name, with no title",
      look: "A branch of 130 characters and no task title: its row drops the id tail and cuts in the middle. A wraps the whole name; B cuts the chip in the middle with the full name in its tooltip; C clamps to two lines.",
    },
  },
  play: async ({ canvasElement, userEvent }) => {
    await userEvent.click(await rowOf(canvasElement, BRANCH_LONG));
  },
};
