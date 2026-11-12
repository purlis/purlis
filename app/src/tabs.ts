// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { activityView, isActivity } from "./activity";
import { chatNetworkView, isChatNetwork } from "./chatNetwork";
import { archiveTitle } from "./memories";

/**
 * The tabs the window shows, and how each one is split into panes.
 *
 * **A tab is a layout of panes, and a pane holds a session or a view** (ADR 0043, as
 * amended 2026-09-23). A session is a chat's terminal; a view is anything else a tab can show —
 * a persona, an extension's statistics — named by data ({@link ViewRef}) rather than by a
 * component, so charter's own views and an extension's take the same path. The operator's
 * words, choosing a tab for the persona card: *"we dont have other tabs then sessions, and this
 * can be good example for us - that in tabs we can have what we want - not only harnesses"*.
 *
 * **The pane is what was generalised, not the tab.** A tab stays one thing — a layout — so a
 * view can be split beside a chat, and every rule a tab already had (the fixed order, pinning,
 * the overflow menu, closing the pane in focus) holds for a tab that shows no chat without a
 * second copy of it. What a tab does NOT have any more is a chat of its own by definition: the
 * chat's name lives on the session a pane shows, so a tab holding only a view has no chat to
 * pretend about ({@link chatOf}).
 *
 * Only the tab in front has panes on screen, which is what keeps fifty sessions cheap: the core
 * holds every session's terminal, and the panes that are visible are the only ones drawing.
 *
 * **A tab belongs to a workspace, and the strip shows one workspace's tabs** (ADR 0036). It is
 * not a field on the tab: which workspace a chat is in is the plane's answer, read off the
 * sidebar, so the functions that need it take a `FiledIn` and ask.
 *
 * **`order` is fixed, and that is now a decision rather than an accident** (ADR 0039). A tab
 * is appended when it opens, taken out when it closes, and nothing else ever touches its
 * place. A tab that moves under the cursor breaks aiming: an operator going back to the chat
 * that was third from the left goes there with their hand, not by reading, and a strip that
 * re-sorted on activity would turn every click into a read. This is what browsers do, and it
 * is the one interaction convention in this window every user already has.
 *
 * The one place activity DOES order anything is the overflow menu — `byLastActivity` below,
 * and nowhere else. The rule and its opposite are the same rule from two sides: the menu is a
 * list you read, the strip is a surface you aim at, and the boundary is whether the thing
 * moves under your hand.
 *
 * **One rule says where a chat opens** (#5289): `opensAt`, which `press` and `openNew` land by,
 * and `back` is the way to the chat a pane showed before. Who asked whom is the core's, handed
 * in as `AskedBy`. A tab switched to a task in its chat's place (`Tab.shows`, #1486) is only
 * what a record from before layouts were kept puts back, until #4758's upgrade reads those.
 *
 * Everything here is a plain value, so the window's whole arrangement is one state to test.
 */

/** Which way a split divides its two children: `row` side by side, `column` one above the other. */
export type Direction = "row" | "column";

/**
 * A view, named by data: **who draws it, which of theirs, and what it is about.**
 *
 * `from` is `null` for a view charter draws itself and an approved extension's id for one it
 * offers, which is `purlis_core::panel::By` on the wire; `view` is which of theirs; `key` is
 * what it is about inside that — a persona's name, or `""` for the whole plane. The persona view
 * is `{ from: null, view: "persona", key: "steward" }` and persona statistics is
 * `{ from: "persona-statistics", view: "statistics", key: "" }`: **the same shape, the same
 * command (`open_view`), the same renderer** — nothing about a built-in view is a path an
 * extension's cannot take.
 */
export type ViewRef = { from: string | null; view: string; key: string };

/**
 * **The Saving view** (charter-app#294, ADR 0051): where a plane's unsaved work sits, what the
 * next save takes, the save button and the last saves. One per plane.
 */
export const SAVING_VIEW: ViewRef = { from: null, view: "saving", key: "" };

/** What the Saving tab is called. */
export const SAVING_TITLE = "Saving";

/**
 * **The Licences view** (#1719): what purlis ships that others wrote, and their licences, which
 * About opens in the project in front. One per project, about nothing in it.
 */
export const LICENCES_VIEW: ViewRef = { from: null, view: "licences", key: "" };

/**
 * **The keyboard shortcuts** (#2921, N25): the cheat sheet the key registry draws
 * (`keys/ShortcutsView.tsx`), opened from the palette and by its key. One per project, about
 * nothing in it.
 */
export const SHORTCUTS_VIEW: ViewRef = { from: null, view: "shortcuts", key: "" };

/** What the shortcuts' tab is called. */
export const SHORTCUTS_TITLE = "Keyboard shortcuts";

/**
 * **Devices** (#2052, spec #2041): this machine and every runner it keeps, with what runs on
 * each, and each pool's use (`DevicesView.tsx`). One per project, about nothing in it: what it
 * shows is the machine's.
 */
export const DEVICES_VIEW: ViewRef = { from: null, view: "devices", key: "" };

/** What the Devices tab is called. */
export const DEVICES_TITLE = "Devices";

/**
 * **Agent identity** (#4485, spec #4469): what a chat acts as on a forge, set up, inspected and
 * removed. One per project, whose tool lines and personas it reads.
 */
export const AGENT_IDENTITY_VIEW: ViewRef = { from: null, view: "agent-identity", key: "" };

/** What the Agent identity tab is called. */
export const AGENT_IDENTITY_TITLE = "Agent identity";

/**
 * **Settings at a workspace's level** (SE-20, #1170; first a page of its own, charter-app#280):
 * the Settings tab keyed by the level and its target — the workspace — so there is one per
 * workspace, deduplicated by the same `viewKey` and filed on that workspace's strip. It keeps the
 * address the old Workspace settings page had (D-SE20a): a launch's record, a pin and a rename
 * (`followRename`, and the core's `wscmd::rename`) already follow it, so a tab put back from
 * before SE-20 opens the Settings tab at that workspace's level.
 */
export function workspaceSettingsView(workspace: string): ViewRef {
  return { from: null, view: "workspace-settings", key: workspace };
}

/**
 * **A workspace's cross-repo changes** (charter#470, ADR 0060): charter's own view, answered in
 * panel blocks by `change::view`. Keyed by the workspace, like its settings, so there is one tab
 * per workspace and it is filed on that workspace's strip.
 */
export function changesView(workspace: string): ViewRef {
  return { from: null, view: "changes", key: workspace };
}

/** What a workspace's changes tab is called. */
export function changesTitle(workspace: string): string {
  return `Changes · ${workspace}`;
}

/**
 * **The agent instructions a workspace's repo carries** (FR-18a, #612): `CLAUDE.md`, `AGENTS.md`
 * and `.cursor/rules`, previewed whole and added to the workspace's memory only on the tab's
 * press. Keyed by the workspace, like its changes, so there is one tab per workspace.
 */
export function repoInstructionsView(workspace: string): ViewRef {
  return { from: null, view: "repo-instructions", key: workspace };
}

/** What a workspace's repo instructions tab is called. */
export function repoInstructionsTitle(workspace: string): string {
  return `Memory from the repo · ${workspace}`;
}

/**
 * **The first task** (FR-28, #621): the guided task FR-1 measures, run twice on the repo the first
 * run opened — each run a chat on a branch of its own, with the task typed and unsent — and both
 * diffs one press away. Keyed by the repo's clone, which is where each run's branch is cut, and
 * filed on its workspace's strip.
 */
export function firstTaskView(clone: string): ViewRef {
  return { from: null, view: "first-task", key: clone };
}

/** What the first task's tab is called. */
export function firstTaskTitle(workspace: string): string {
  return `First task · ${workspace}`;
}

/**
 * **Setting up a harness** (FR-29, W10's "no harness found"): each harness's official installer,
 * run in a shell tab on a press, and a local model server already on the machine. Keyed by the
 * directory the first chat would start in — the repo's clone — so the shell and the chat it
 * leads to start there, and filed on that workspace's strip.
 */
export function harnessSetupView(cwd: string): ViewRef {
  return { from: null, view: "harness-setup", key: cwd };
}

/**
 * **Pool setup** (spec #4469): one guided tab from nothing to a working chat on a pool. One tab per
 * project for now; a pool of its own has one with its Open setup (#4491).
 */
export const POOL_SETUP_VIEW: ViewRef = { from: null, view: "pool-setup", key: "" };
/** What the Pool setup tab is called. */
export const POOL_SETUP_TITLE = "Pool setup";

/** What the harness setup tab is called. */
export function harnessSetupTitle(workspace: string): string {
  return `Set up a harness · ${workspace}`;
}

/**
 * **A harness's capability card** (HP-19, W10): what one harness can do here, each thing it lacks
 * said in a line, drawn by the core off the harness's declaration and adapter
 * (`purlis_core::harness_card`). Keyed by the harness's name, so there is one tab per harness,
 * opened from a chat's header and called what the card is labelled, *What Codex can do here*.
 */
export function harnessCardView(harness: string): ViewRef {
  return { from: null, view: "harness", key: harness };
}

/**
 * What Settings at a workspace's level is called (the core's `wscmd::rename` says the same):
 * "Settings", as at the You and Project levels, and the workspace it is at (D-1192-1). The name
 * tells the tab apart from the Project level's Settings tab on the same strip.
 */
export function workspaceSettingsTitle(workspace: string): string {
  return `Settings · ${workspace}`;
}

/** What Settings at a workspace's level was called before D-1192-1, as an older record holds it. */
function workspaceSettingsTitleBefore(workspace: string): string {
  return `Workspace settings · ${workspace}`;
}

/**
 * **The Settings tab** (SE-16, #1166; V89b): every setting, one level at a time. **Keyed by the
 * level it shows** — `you`, `project` (SE-17); a workspace's is keyed by the workspace too, and
 * is {@link workspaceSettingsView} (SE-20) — so opening
 * Settings at a level that already has its tab brings that tab forward. The project is the one
 * whose tabs hold it. Its level switcher moves the tab itself to another level
 * ({@link showInstead}, D-SE17a), so the key always says what the tab shows. It is the view
 * `settings` because it is what Settings means from here on. The old Project settings page was
 * the same view with the empty key; SE-19 retired it, and a tab of it put back from an older
 * launch is Settings at the Project level ({@link viewNamedNow}).
 *
 * The You level is the machine's and not the project's, so it is the same tab whichever project's
 * strip it was opened on: what it edits is the machine's layout file, never a file in the project.
 */
export function settingsView(level: Exclude<SettingsLevel, "workspace">): ViewRef {
  return { from: null, view: "settings", key: level };
}

/** The levels a Settings tab can be at (`settings/groups.ts`'s `Level`, as far as it is offered). */
export type SettingsLevel = "you" | "project" | "workspace";

/** The level a view is the Settings tab at, or `undefined` for any other view. */
export function settingsLevelOf(view: ViewRef): SettingsLevel | undefined {
  if (view.from !== null) return undefined;
  if (view.view === "workspace-settings") return "workspace";
  if (view.view !== "settings") return undefined;
  return view.key === "you" || view.key === "project" ? view.key : undefined;
}

/** What the Settings tab is called. */
export const SETTINGS_TAB_TITLE = "Settings";

/**
 * A view tab as a record from an older charter names it, as it is named now, so a launch that
 * put one back puts Settings back instead of a tab nothing draws:
 *
 * - the Preferences tab (charter-app#283) is Settings at the You level since SE-16;
 * - the Project settings page (charter-app#252, the view `settings` keyed `""`) is Settings at
 *   the Project level since SE-19 retired it (D-SE19b);
 * - Settings at a workspace's level, titled "Workspace settings · <ws>" until D-1192-1, is
 *   titled {@link workspaceSettingsTitle} — only that exact title, so nothing else is renamed.
 *
 * Every other view is itself.
 */
export function viewNamedNow(view: ViewRef, title: string): { view: ViewRef; title: string } {
  if (view.from !== null) return { view, title };
  if (view.view === "preferences") return { view: settingsView("you"), title: SETTINGS_TAB_TITLE };
  if (view.view === "settings" && view.key === "")
    return { view: settingsView("project"), title: SETTINGS_TAB_TITLE };
  if (view.view === "workspace-settings" && title === workspaceSettingsTitleBefore(view.key))
    return { view, title: workspaceSettingsTitle(view.key) };
  return { view, title };
}

/** **What a view is called by itself** (#5577): its own title, or its view's word where it was
 *  opened with none. */
export function viewTitleOf(content: { view: ViewRef; title?: string }): string {
  return content.title ?? content.view.view;
}

/** What a pane shows. */
export type Content =
  /**
   * A chat's terminal. `chat` is the chat's own name, as the plane records it and as the core
   * was told it — separate from the tab's `name`, because a split starts a second chat under
   * the first one's name and the core must be told that name, not the sentence the tab draws.
   */
  | {
      kind: "session";
      session: number;
      chat: string;
      /**
       * **A one-job program** (#4761; ADR 0062 and 0081 as amended 2026-11-02): a terminal
       * editor opened from a file, or a forge login, opened beside the pane `returnTo` with the
       * keyboard ({@link openOneJob}). Its split goes when its program exits and the keyboard
       * goes back to `returnTo` ({@link oneJobEnded}). It has no grip, close or swap, nothing a
       * press replaces, and the core never records it. Absent on every other chat's pane.
       */
      oneJob?: { returnTo?: number };
    }
  /**
   * A view. **It carries its workspace, because it has nothing else to be filed by**: a chat is
   * on the strip of the workspace it works in (the plane's answer, `FiledIn`), and a view works
   * nowhere. So it is on the strip it was opened from — the one in front — and says so.
   */
  | {
      kind: "view";
      view: ViewRef;
      workspace: string;
      /**
       * Put back by a launch, and **not asked anything until the operator presses for it.**
       *
       * An extension's view runs that extension's program when it is asked (ADR 0041 stage 2,
       * *one round trip per deliberate human action*), and a tab the record put back was
       * opened by nobody at this launch — the record is a file in the plane, and a line in it
       * must not become a program run at every start. So a put-back tab waits, says so, and
       * asks on a press; one the operator opens is asked at once. charter's own views run no
       * program and ignore it, save a vault's: reading one that is more than files takes its
       * token, which macOS may ask about, so it waits too (`VaultTab`, #1660).
       */
      waits?: boolean;
      /**
       * **Its own title** (#5577): what it was opened as, such as a file's name or a settings
       * page's, which names it in a split, where the tab's name is its first pane's, and names
       * the tab once it is first. None where it was opened with none, as DevTools beside its
       * browser, which is then called what its tab is.
       */
      title?: string;
      /**
       * **The strip's preview tab** (SI-9b, ADR 0065 Q1), VS Code's model: what a single click
       * on a memory row shows, replaced in place by the next single click. A double-click or
       * starting an edit keeps it ({@link keepView}), and a kept tab is an ordinary tab. Drawn
       * in italics on the strip, so the operator can tell which tab the next click reuses.
       */
      preview?: boolean;
    }
  /**
   * **Nothing** (#4755): an empty pane, drawn with New chat and how many chats run here. A pane
   * shows a chat, a view, or this; a tab can be all empty panes, and the record keeps them. It
   * carries its workspace for a view's reason: nothing else files it on a strip.
   */
  | { kind: "empty"; workspace: string };

export type Layout =
  | { kind: "pane"; pane: number; content: Content }
  | {
      kind: "split";
      direction: Direction;
      children: [Layout, Layout];
      /**
       * The first child's share of the split, in tenths of a percent (#4751): what a person
       * dragged its handle to, kept per tab and recorded with it, so a relaunch draws the split
       * as it was. Absent for a split never moved, which is even (`laidOut.ts` `EVEN`).
       */
      share?: number;
    };

export type Tab = {
  id: number;
  /**
   * What the tab bar shows: **the name the operator gave the chat, or its default** — or the
   * view's title for a tab that opened on a view.
   *
   * The default is the persona the chat adopted and then its number, `steward 3` rather than
   * `3` (charter-app#130), persona first because that is what the operator reads the strip
   * for (charter-app#254). A number identifies a chat to charter and tells the operator
   * nothing, and the strip is where an operator with fifty of them works out which is which.
   * What goes before the number is known at the moment a tab opens on every path — the picker
   * carries the operator's choice, and a chat put back at a launch carries its own — so this
   * is never filled in later.
   *
   * **A name given is charter's label and nothing else** (charter-app#254). The chat's own
   * name — what its harness was started with, and what a split's chat is started under — is
   * `Content.chat`, and a rename never touches it: a running harness is not disturbed.
   */
  name: string;
  /** What {@link name} goes back to when a given name is taken off: the default above. */
  defaultName: string;
  /**
   * **A name the person gave the tab itself** (#5342, spec #5339), never a chat's: what the tab
   * is called whatever its panes hold ({@link tabNameOf}), which no split, close or drop
   * replaces. Window state, as a tab's name always was: the record keeps none. Absent on a tab
   * named for its first pane, which is every tab until Rename sets one (#5343).
   */
  own?: string;
  layout: Layout;
  /** The pane a split or a close acts on. */
  focused: number;
  /**
   * **What a pane shows in place of its own chat** (#1486), by pane: a task below that chat,
   * at any depth. Absent for a tab whose every pane shows its own chat, which is every tab of
   * a session with no tasks.
   *
   * **The layout is not touched.** A pane's content stays the session's own chat, so what a
   * tab IS — whose close it is, where it is filed, what it is named after — is read exactly as
   * before ({@link panesOf}, {@link chatOf}); only what is DRAWN in the pane reads this
   * ({@link shownIn}, {@link layoutShown}). It is the tab's, so it is remembered while another
   * tab is in front and goes with the tab when it closes.
   */
  shows?: Readonly<Record<number, number>>;
  /**
   * **Whether the operator pinned the tab** (#4935): the tab's own, never its first pane's chat
   * or view, so a tab whose only pane is empty can be pinned and the pin stays whatever that
   * pane goes on to show. The record keeps it as the tab's `pinned` (`laidOut.layoutOf`).
   * Absent on a tab that is not pinned.
   */
  pinned?: true;
  /** **The pane zoomed to fill the tab for now** (#4765): drawn alone while the tree stays
   *  whole, in memory only and never told to the core (`tiling.ts` `zoomedOf`). */
  zoomed?: number;
  /**
   * **The strip this tab is on** (#4757; spec #4750, "A tab records its workspace"), once a
   * press has shown a chat in it. Absent until then, and the tab is on its first pane's strip
   * ({@link workspaceOf}). A pane may show a chat of another workspace (a breadcrumb walks up to
   * an asker filed elsewhere, ⌘⇧H goes back to one), and the tab does not move strip for it: a
   * chat is still filed by the plane's answer, never by the tab it sits in.
   */
  workspace?: string;
  /**
   * **The one chat each pane showed before**, by pane (#4757): what ⌘⇧H goes back to
   * ({@link back}). Written by a press that replaces a chat, and only then.
   */
  previous?: Readonly<Record<number, number>>;
};

/**
 * **What Undo puts back** (#4763, #4765): the window's tabs before its last layout change and
 * after it, and what the change did, in the past tense. One for the window: a drop can change
 * two tabs at once, or make one. In memory only (`layoutUndo.ts`).
 */
export type LayoutUndo = { before: Tabs; after: Tabs; did: string };

/**
 * Which workspace a chat is filed under — the strip its tab appears on.
 *
 * A function rather than a field on the tab, because **the plane is what files a chat**: what
 * relates a chat to a workspace is the directory it works in, and the sidebar the core reads
 * off the plane is the answer to that. A copy on the tab would be a second answer that
 * nothing invalidates when the plane changes under it.
 */
export type FiledIn = (session: number) => string;

export type Tabs = {
  byId: Record<number, Tab>;
  /** Left to right, as the tab bar shows them. */
  order: number[];
  /** The tab on screen, or none when every tab has closed. */
  inFront?: number;
  /** Ids already handed out, so a new tab or pane never reuses one, and the chats the window
   *  has named ({@link newChatNumber}); without it, a window counts its chats by its tabs. */
  named: { tabs: number; panes: number; chats?: number };
  /**
   * Where each view's divider was, by {@link viewKey}: its first side's share of the pane in
   * percent — a file tab's tree beside its preview (FM-2). **Kept for a view whose tab closed**,
   * so opening it again brings the divider back where it was; the record keeps it for the tabs
   * open at a quit (`reopen::View::split`).
   */
  splits?: Readonly<Record<string, number>>;
  /**
   * **When each pane was last the one with the keyboard**, by pane (#4757): bigger is more
   * recent. What a press reads with a view alone in the front tab, to find the pane the person
   * used last that could take it ({@link opensAt}). Stamped by {@link noteUsed}.
   */
  used?: Readonly<Record<number, number>>;
  /** **The last layout change, which Undo and ⌘Z put back** (#4763), while the tabs are still
   *  as it left them. Never told to the core. */
  undo?: LayoutUndo;
};

/** Where `view`'s divider was, or `undefined` when it never moved. */
export function splitOf(tabs: Tabs, view: ViewRef): number | undefined {
  return tabs.splits?.[viewKey(view)];
}

/** `view`'s divider was moved to `split`. Answers `tabs` itself when it was already there. */
export function setSplit(tabs: Tabs, view: ViewRef, split: number): Tabs {
  if (splitOf(tabs, view) === split) return tabs;
  return { ...tabs, splits: { ...tabs.splits, [viewKey(view)]: split } };
}

export function noTabs(): Tabs {
  return { byId: {}, order: [], named: { tabs: 0, panes: 0, chats: 0 } };
}

/**
 * Opens a tab with one pane showing `session`, in front.
 *
 * `who` is what the default name puts before the chat's number: the persona it adopted, or —
 * with none — the program it runs. `label` is the name the operator gave it, where they gave
 * one, and the tab says that instead.
 */
export function openTab(
  tabs: Tabs,
  session: number,
  chat = "",
  who: string | null = null,
  label: string | null = null,
): Tabs {
  const named = chat || String(tabs.named.tabs + 1);
  return withTab(
    tabs,
    who ? `${who} ${named}` : named,
    { kind: "session", session, chat: named },
    tabs.order.length,
    label,
  );
}

/**
 * **The strip's `+`: a new tab holding one empty pane** (#4758; spec story 13), in front, after
 * the strip's last tab, on `workspace`'s strip. New chat then fills it, as it fills any empty
 * pane ({@link opensAt}): "new tab" still means a tab.
 */
export function openEmptyTab(tabs: Tabs, workspace: string): Tabs {
  const opened = withTab(tabs, EMPTY_TAB, { kind: "empty", workspace }, tabs.order.length);
  const id = opened.named.tabs;
  return { ...opened, byId: { ...opened.byId, [id]: { ...opened.byId[id], workspace } } };
}

/**
 * **The number a chat the window starts now is named with** (#4758): one more than the chats it
 * has named. Not the next tab's, as when every chat opened a tab: every strip keeps a tab, so a
 * fresh window's empty tab is tab 1, and a chat that fills an empty pane opens none, so two
 * chats would share a number. A window put back from a layout counts from its tabs, as before.
 */
export function newChatNumber(tabs: Tabs): number {
  return (tabs.named.chats ?? tabs.named.tabs) + 1;
}

/** `tabs` with chat number `number` taken, so {@link newChatNumber} gives the one after it. */
export function chatNamed(tabs: Tabs, number: number): Tabs {
  const chats = Math.max(tabs.named.chats ?? tabs.named.tabs, number);
  return { ...tabs, named: { ...tabs.named, chats } };
}

/**
 * **The empty tabs of workspaces the plane no longer has, gone** (#4758): a workspace renamed
 * or removed on disk takes its strip with it, and an empty tab, which holds nothing but its
 * strip, would keep the window on a workspace that is not there. `onPlane` says which are,
 * and says yes for the strip outside every workspace. The same `tabs` where none goes. An
 * empty tab shows no chat, so the strip its chats are filed on is never asked.
 */
export function dropEmptyTabsOfGoneWorkspaces(
  tabs: Tabs,
  onPlane: (workspace: string) => boolean,
): Tabs {
  const gone = tabs.order.filter((id) => {
    const layout = tabs.byId[id]?.layout;
    return (
      layout?.kind === "pane" &&
      layout.content.kind === "empty" &&
      !onPlane(layout.content.workspace)
    );
  });
  return gone.reduce((left, id) => closeTab(left, id, () => ""), tabs);
}

/**
 * Gives a chat's tab the name `label`, or its default back with `null` (charter-app#254).
 *
 * **The tab's name and nothing else**: the chat's own name, which its harness was started
 * with, stays what it was. A tab that opened on a view is named after what it shows, and is
 * left as it is; so is a tab that is not there.
 *
 * **A split tab's name is its own chat's** — the first pane's, {@link chatOf} — whichever pane
 * has the keyboard, because that is the chat the tab is (its state mark and its pin are that
 * chat's too). A relaunch brings each chat back as a tab of its own, so the name comes back on
 * that chat's tab and the chat that was split beside it comes back under its default.
 */
export function renameTab(tabs: Tabs, id: number, label: string | null): Tabs {
  const tab = tabs.byId[id];
  const name = label ?? tab?.defaultName;
  if (!tab || chatOf(tabs, id) === undefined || tab.name === name) return tabs;
  return { ...tabs, byId: { ...tabs.byId, [id]: { ...tab, name } } };
}

/**
 * The one place a tab is minted: `content` in one pane, at `at` in the order, in front, named
 * `label` where the operator gave it one and `name` otherwise.
 */
function withTab(
  tabs: Tabs,
  name: string,
  content: Content,
  at: number,
  label: string | null = null,
): Tabs {
  const id = tabs.named.tabs + 1;
  const pane = tabs.named.panes + 1;
  const order = [...tabs.order];
  order.splice(Math.max(0, Math.min(at, order.length)), 0, id);
  return {
    ...tabs,
    byId: {
      ...tabs.byId,
      [id]: {
        id,
        name: label ?? name,
        defaultName: name,
        layout: { kind: "pane", pane, content },
        focused: pane,
      },
    },
    order,
    inFront: id,
    named: { ...tabs.named, tabs: id, panes: pane },
  };
}

/** What one view is called as a key: unique per plane, and the same for the same view. */
export function viewKey(view: ViewRef): string {
  // Namespaced as `purlis_core::panel::Panel::key` is, so an extension that calls itself
  // `charter` cannot answer to charter's own view's name.
  return view.from === null
    ? `charter/${view.view}/${view.key}`
    : `ext/${view.from}/${view.view}/${view.key}`;
}

/** The tab and pane already showing `view`, where one is. */
export function findView(tabs: Tabs, view: ViewRef): { tab: number; pane: number } | undefined {
  const wanted = viewKey(view);
  for (const tab of tabs.order) {
    const found = contents(tabs.byId[tab].layout).find(
      (one) => one.content.kind === "view" && viewKey(one.content.view) === wanted,
    );
    if (found) return { tab, pane: found.pane };
  }
  return undefined;
}

/**
 * Opens `view` in a tab of its own, in front — **or brings forward the one already showing it.**
 *
 * One view, one surface: a second tab for the persona that already has one is two answers to
 * "where is steward" that can drift apart (one scrolled, one searched), and the operator asked
 * for a tab, not a tab per click. The pane showing it takes the focus too, so a view that is
 * one side of a split is the side that answers the keyboard.
 *
 * `workspace` is the strip it goes on: the one in front, which is where it was opened from.
 */
export function openView(tabs: Tabs, view: ViewRef, name: string, workspace: string): Tabs {
  const open = findView(tabs, view);
  if (open) {
    const tab = tabs.byId[open.tab];
    const shown = panes(tab.layout).find((one) => one.pane === open.pane)?.content;
    const waits = shown?.kind === "view" && shown.waits === true;
    // In front, focused and not waiting already: nothing changes, so `tabs` itself.
    if (!waits && tab.focused === open.pane && tabs.inFront === tab.id) return tabs;
    // Opening it is the operator asking for it, so a tab that was waiting for a press has had
    // one (`Content.waits`).
    const layout = replace(tab.layout, open.pane, (found) =>
      found.content.kind === "view" && found.content.waits
        ? { ...found, content: { ...found.content, waits: false } }
        : found,
    );
    return {
      ...tabs,
      byId: { ...tabs.byId, [tab.id]: { ...tab, layout, focused: open.pane } },
      inFront: tab.id,
    };
  }
  return withTab(tabs, name, { kind: "view", view, workspace, title: name }, tabs.order.length);
}

/**
 * Opens `view` in a tab of its own **behind the tab in front** — an offer beside what the operator
 * is doing, which asks them nothing until they go to it (FR-18a, W10's interrupt budget). With
 * nothing in front it is in front, as there is nothing else to show. A view already open is left
 * where it is.
 */
export function offerView(tabs: Tabs, view: ViewRef, name: string, workspace: string): Tabs {
  if (findView(tabs, view)) return tabs;
  const opened = withTab(
    tabs,
    name,
    { kind: "view", view, workspace, title: name },
    tabs.order.length,
  );
  return tabs.inFront === undefined ? opened : { ...opened, inFront: tabs.inFront };
}

/**
 * Shows `view` in the strip's preview tab (SI-9b, ADR 0065 Q1) — **replacing what that tab
 * previewed**, in its place on the strip — or opens a preview tab when the strip has none. A
 * view already open anywhere is brought forward instead, as {@link openView} does, and a kept
 * tab is never replaced: previewing is what a single click does, and it must not take away a
 * tab the operator chose to keep.
 *
 * One per strip: `workspace` is the strip in front, and a preview on another strip is that
 * strip's.
 */
export function openPreview(tabs: Tabs, view: ViewRef, name: string, workspace: string): Tabs {
  if (findView(tabs, view)) return openView(tabs, view, name, workspace);
  const reused = previewOf(tabs, workspace);
  if (reused === undefined) {
    return withTab(
      tabs,
      name,
      { kind: "view", view, workspace, title: name, preview: true },
      tabs.order.length,
    );
  }
  const tab = tabs.byId[reused];
  const layout = replace(tab.layout, tab.focused, (found) => ({
    ...found,
    content: { kind: "view", view, workspace, title: name, preview: true },
  }));
  return {
    ...tabs,
    byId: { ...tabs.byId, [reused]: { ...tab, layout, name, defaultName: name } },
    inFront: reused,
  };
}

/**
 * The strip's preview tab, where it has one: a tab showing one view and nothing else, marked
 * as a preview. A tab split beside a chat is not one, because replacing it would replace the
 * chat's tab.
 */
export function previewOf(tabs: Tabs, workspace: string): number | undefined {
  return tabs.order.find((id) => {
    const layout = tabs.byId[id].layout;
    return (
      layout.kind === "pane" &&
      layout.content.kind === "view" &&
      layout.content.preview === true &&
      layout.content.workspace === workspace
    );
  });
}

/**
 * Keeps the tab showing `view` (SI-9b): it stops being the preview, so the next single click
 * previews in a tab of its own. A double-click on the row or the tab, and starting an edit,
 * keep it. Answers `tabs` itself when nothing changed.
 */
export function keepView(tabs: Tabs, view: ViewRef): Tabs {
  const open = findView(tabs, view);
  if (!open) return tabs;
  const tab = tabs.byId[open.tab];
  let changed = false;
  const layout = replace(tab.layout, open.pane, (found) => {
    if (found.content.kind !== "view" || !found.content.preview) return found;
    changed = true;
    return { ...found, content: { ...found.content, preview: false } };
  });
  return changed ? { ...tabs, byId: { ...tabs.byId, [tab.id]: { ...tab, layout } } } : tabs;
}

/**
 * The pane showing `from` shows `to` instead, and its tab is called `name` — a new memory's tab
 * once it is saved (it shows the memory it made), and a memory's tab once it is retitled. The
 * tab keeps its place and whether it is kept. Answers `tabs` itself when `from` is not shown.
 *
 * **One view, one surface**, as {@link openView} keeps it: when another pane already shows `to`,
 * that one is brought forward and `from` is left as it was — the Settings tab moved to a level
 * whose tab is open (SE-17), a Search asked what another Search tab already shows.
 */
export function showInstead(tabs: Tabs, from: ViewRef, to: ViewRef, name: string): Tabs {
  const open = findView(tabs, from);
  if (!open) return tabs;
  const there = findView(tabs, to);
  if (there && (there.tab !== open.tab || there.pane !== open.pane))
    return openView(tabs, to, name, "");
  const tab = tabs.byId[open.tab];
  const layout = replace(tab.layout, open.pane, (found) =>
    // Its own title goes with it (#5577): the pane is called what it shows now.
    found.content.kind === "view"
      ? { ...found, content: { ...found.content, view: to, title: name } }
      : found,
  );
  // The tab's name is its lead view's: a view on the far side of a split names nothing.
  const leads = contents(tab.layout)[0]?.pane === open.pane;
  const renamed = leads ? { name, defaultName: name } : {};
  return { ...tabs, byId: { ...tabs.byId, [tab.id]: { ...tab, layout, ...renamed } } };
}

/**
 * Puts back a view tab the last launch recorded, at the place it had — **behind** whatever is in
 * front, because a launch decides what is in front once, from the whole record.
 *
 * `at` is where it was on the strip, over chats and views together. It is a place and not a
 * promise: a chat that did not come back moves it by one, and a place past the end is the end.
 * A view already open is left where it is rather than drawn twice.
 */
export function putViewBack(
  tabs: Tabs,
  view: ViewRef,
  name: string,
  workspace: string,
  at: number,
  split?: number,
): Tabs {
  if (findView(tabs, view)) return tabs;
  const opened = withTab(
    tabs,
    name,
    { kind: "view", view, workspace, title: name, waits: true },
    at,
  );
  const back = { ...opened, inFront: tabs.inFront };
  return split === undefined ? back : setSplit(back, view, split);
}

/**
 * The operator pressed to have a waiting view asked (`Content.waits`): that pane of the tab in
 * front stops waiting. Anything else is left as it is.
 */
export function stopWaiting(tabs: Tabs, pane: number): Tabs {
  const tab = frontTab(tabs);
  if (!tab) return tabs;
  let changed = false;
  const layout = replace(tab.layout, pane, (found) => {
    if (found.content.kind !== "view" || !found.content.waits) return found;
    changed = true;
    return { ...found, content: { ...found.content, waits: false } };
  });
  return changed ? { ...tabs, byId: { ...tabs.byId, [tab.id]: { ...tab, layout } } } : tabs;
}

/**
 * The tab's own chat: **the session in its first pane, and nothing when that pane is a view.**
 *
 * What a tab's state mark and its pin are about. The core's "chat in front" is
 * {@link chatInFrontOf}, which also names a chat beside a view. A tab that opened
 * on a view and was split to start a chat beside it is still the view's tab — its first pane
 * says what it is — so it draws no chat state and has no chat to pin.
 */
export function chatOf(tabs: Tabs, id: number): number | undefined {
  const first = contentsOf(tabs, id)[0]?.content;
  return first?.kind === "session" ? first.session : undefined;
}

/**
 * **The chat a tab puts in front of the person**: its own chat ({@link chatOf}), or, in a tab
 * whose first pane is a view, the first chat beside it. What the core is told is in front, so
 * that a task such a chat shows is one the person is taken to be looking at (#1486). Nothing
 * for a tab of views only.
 */
export function chatInFrontOf(tabs: Tabs, id: number): number | undefined {
  return chatOf(tabs, id) ?? panesOf(tabs, id)[0]?.session;
}

/**
 * The name a new chat started beside this tab's panes is given: the name of the first chat in
 * it, or nothing when it has none — a view's tab split to start a chat starts one with a name of
 * its own, not the view's title.
 */
export function chatNameOf(tabs: Tabs, id: number): string | undefined {
  const tab = tabs.byId[id];
  if (!tab) return undefined;
  const first = contents(tab.layout).find((one) => one.content.kind === "session");
  return first?.content.kind === "session" ? first.content.chat : undefined;
}

/** What each pane of a tab shows, left to right and top to bottom. */
export function contentsOf(tabs: Tabs, id: number): { pane: number; content: Content }[] {
  const tab = tabs.byId[id];
  return tab ? contents(tab.layout) : [];
}

/** **Whether tab `id` has one pane**, so that its own close does what the pane's would (#5318). A
 *  pane zoomed to fill its tab is not its only one: `layout` keeps the whole tree (#4765). */
export function hasOnePane(tabs: Tabs, id: number): boolean {
  return contentsOf(tabs, id).length === 1;
}

/** What the focused pane of the tab in front shows, when anything is in front. */
export function focusedContent(tabs: Tabs): Content | undefined {
  const tab = frontTab(tabs);
  return tab && contents(tab.layout).find((one) => one.pane === tab.focused)?.content;
}

/**
 * **Every view keyed by a workspace's name, and how its key spells that name** (#1248): the one
 * rule a rename follows, here for the window's tabs and pins and in the core for the saved
 * record (`wscmd::rename::Move::view`, tested against the same keys).
 *
 * `whole` is a key that is the spelling and nothing more; otherwise the spelling is the key's
 * first part, and what follows its `/` is kept as it is — a todo's slug, a memory's, a piece and
 * a file, a session record's file.
 */
const BY_WORKSPACE: { view: string; spell: (workspace: string) => string; whole: boolean }[] = [
  { view: "workspace-settings", spell: (ws) => ws, whole: true },
  { view: "changes", spell: (ws) => ws, whole: true },
  { view: "repo-instructions", spell: (ws) => ws, whole: true },
  { view: "todo", spell: (ws) => ws, whole: false },
  { view: "piece-files", spell: (ws) => ws, whole: false },
  { view: "piece-file", spell: (ws) => ws, whole: false },
  { view: "piece-diff", spell: (ws) => ws, whole: false },
  { view: "memory", spell: (ws) => `workspace/${ws}`, whole: false },
  { view: "memory-archive", spell: (ws) => `workspace/${ws}`, whole: true },
  { view: "session", spell: (ws) => `workspaces/${ws}`, whole: false },
];

/** What a view keyed by a workspace's name is titled, where the title says the name. */
const TITLED_BY_WORKSPACE: Record<string, (workspace: string) => string> = {
  "workspace-settings": workspaceSettingsTitle,
  changes: changesTitle,
  "repo-instructions": repoInstructionsTitle,
  "memory-archive": (ws) => archiveTitle({ kind: "workspace", name: ws }),
  "first-task": firstTaskTitle,
  "harness-setup": harnessSetupTitle,
};

/**
 * `view` after its workspace is renamed from `from` to `to`, or `undefined` when its key does
 * not name that workspace (#1248).
 *
 * The views keyed by a path rather than a name — the first task's and the harness setup's, keyed
 * by a clone's directory — follow when the plane's `root` is given, by the folder that moved:
 * `<root>/workspaces/<from>`.
 */
export function renamedView(
  view: ViewRef,
  from: string,
  to: string,
  root?: string,
): ViewRef | undefined {
  if (view.from !== null) return undefined;
  const swap = (before: string, after: string, whole: boolean): ViewRef | undefined => {
    if (view.key === before) return { ...view, key: after };
    if (!whole && view.key.startsWith(`${before}/`))
      return { ...view, key: after + view.key.slice(before.length) };
    return undefined;
  };
  if (view.view === "first-task" || view.view === "harness-setup")
    return root === undefined
      ? undefined
      : swap(`${root}/workspaces/${from}`, `${root}/workspaces/${to}`, false);
  const rule = BY_WORKSPACE.find((one) => one.view === view.view);
  return rule && swap(rule.spell(from), rule.spell(to), rule.whole);
}

/**
 * Every view tab follows a workspace renamed from `from` to `to` (charter#367): a view on its
 * strip moves to the new strip, and **every view keyed by the workspace's name is keyed by the
 * new one** ({@link renamedView}, #1248) — its settings, its changes, a todo, a memory, a piece's
 * files — so it reads the workspace where it is now, and opening the same thing again finds it.
 * A tab still called what its view is titled under the old name is titled by the new one. The
 * core does the same to the record (`wscmd::rename::Move::view`), so the two agree on what
 * comes back at the next launch.
 *
 * Answers `tabs` itself when nothing moved, so a caller can tell. A chat needs nothing here:
 * its strip is where it works, which the plane answers (`FiledIn`).
 */
export function followRename(tabs: Tabs, from: string, to: string, root?: string): Tabs {
  let moved = false;
  const follow = (layout: Layout): Layout => {
    if (layout.kind === "split")
      return { ...layout, children: layout.children.map(follow) as [Layout, Layout] };
    const content = layout.content;
    if (content.kind !== "view") return layout;
    const strip = content.workspace === from;
    const view = renamedView(content.view, from, to, root);
    if (!strip && !view) return layout;
    moved = true;
    // A title that says the workspace's name says its new one (#5577), as its tab's does below.
    const titled =
      view && content.view.from === null ? TITLED_BY_WORKSPACE[content.view.view] : undefined;
    const title =
      titled !== undefined && content.title === titled(from) ? titled(to) : content.title;
    return {
      ...layout,
      content: {
        ...content,
        workspace: strip ? to : content.workspace,
        view: view ?? content.view,
        ...(title === undefined ? {} : { title }),
      },
    };
  };
  const byId = Object.fromEntries(
    tabs.order.map((id) => {
      const tab = tabs.byId[id];
      const layout = follow(tab.layout);
      const lead = contents(layout)[0]?.content;
      const title = lead?.kind === "view" ? TITLED_BY_WORKSPACE[lead.view.view] : undefined;
      const retitle =
        lead?.kind === "view" &&
        lead.view.from === null &&
        title !== undefined &&
        lead !== contents(tab.layout)[0]?.content &&
        tab.name === title(from);
      const name = retitle ? title(to) : tab.name;
      // A tab's own strip follows its workspace's new name, as a view's does (#4757).
      const strip = tab.workspace === from;
      if (strip) moved = true;
      return [
        id,
        {
          ...tab,
          layout,
          name,
          defaultName: retitle ? name : tab.defaultName,
          ...(strip ? { workspace: to } : {}),
        },
      ];
    }),
  );
  return moved ? { ...tabs, byId } : tabs;
}

/**
 * Every view tab whose strip is not a workspace any more, moved to `outside`.
 *
 * A chat's strip is re-read off the plane every time (`FiledIn`), so a workspace that goes takes
 * its chats' strip with it. A view carries its own, so nothing would move it: it would be on a
 * strip that is never drawn, unreachable. This is the one place that says what a view's strip is
 * when the plane stops having it — the same strip a chat that works in no workspace is on.
 *
 * Answers `tabs` itself when nothing moved, so a caller can tell.
 */
export function refileViews(
  tabs: Tabs,
  stands: (workspace: string) => boolean,
  outside: string,
): Tabs {
  let moved = false;
  const refile = (layout: Layout): Layout => {
    if (layout.kind === "split")
      return { ...layout, children: layout.children.map(refile) as [Layout, Layout] };
    const content = layout.content;
    if (content.kind !== "view" || content.workspace === outside || stands(content.workspace))
      return layout;
    moved = true;
    return { ...layout, content: { ...content, workspace: outside } };
  };
  // A tab's own strip goes the same way (#4757).
  const strip = (tab: Tab): Tab => {
    if (tab.workspace === undefined || tab.workspace === outside || stands(tab.workspace))
      return tab;
    moved = true;
    return { ...tab, workspace: outside };
  };
  const byId = Object.fromEntries(
    tabs.order.map((id) => [id, strip({ ...tabs.byId[id], layout: refile(tabs.byId[id].layout) })]),
  );
  return moved ? { ...tabs, byId } : tabs;
}

/**
 * Closes a tab. **The tab beside it IN ITS OWN WORKSPACE** comes to the front if it was the
 * one in front, and nothing is in front when its workspace held nothing else.
 *
 * The workspace is what the strip shows (ADR 0036), so a close that reached across to a tab
 * on another strip would move the operator to a workspace they did not ask for — and leave
 * the strip they are looking at with no selected tab. A plane charter has not read yet files
 * every chat the same way, and then this is the old rule exactly: the tab to the left.
 *
 * **"Beside" means beside ON THE STRIP**, which is why this takes `pinned` too: a pin draws a
 * tab first, and an operator closing the third tab means the tab they can see to its left,
 * not the one that happens to be before it in the order the chats were opened.
 */
export function closeTab(
  tabs: Tabs,
  id: number,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
): Tabs {
  if (!(id in tabs.byId)) return tabs;
  const order = tabs.order.filter((tab) => tab !== id);
  const byId = Object.fromEntries(order.map((tab) => [tab, tabs.byId[tab]]));
  if (tabs.inFront !== id) return { ...tabs, byId, order };
  return { ...tabs, byId, order, inFront: frontWithout(tabs, id, filedIn, pinned) };
}

/** What a tab whose first pane is empty is called (#4755): **Empty tab**, until it holds
 *  something (#5292; spec #5288 story 86), so the strip says what is there. It was "New chat",
 *  the empty pane's button, until `+` stopped starting a chat. */
export const EMPTY_TAB = "Empty tab";

/** Whether `tab` is one empty pane of `workspace`'s strip: what a close leaves on a strip's last
 *  tab (#4756). */
function isEmptyTab(tab: Tab | undefined, workspace: string): boolean {
  const layout = tab?.layout;
  return (
    layout?.kind === "pane" &&
    layout.content.kind === "empty" &&
    layout.content.workspace === workspace
  );
}

/**
 * **Close tab, the person's** (#4756): the layout closes, and its chats go on in the sidebar.
 * **Unless it is its strip's last tab**, which keeps one empty pane in place of its whole
 * layout, so a strip the person tidied is never a blank window. {@link closeTab} is the bare
 * close, for a tab that goes with its workspace or with the chat the core closed.
 */
export function closeLayout(
  tabs: Tabs,
  id: number,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
): Tabs {
  const tab = tabs.byId[id];
  if (tab === undefined) return tabs;
  const workspace = workspaceOf(tabs, id, filedIn);
  const strip = tabsIn(tabs, workspace, filedIn, pinned);
  if (workspace === undefined || strip.some((other) => other !== id))
    return closeTab(tabs, id, filedIn, pinned);
  if (isEmptyTab(tab, workspace)) return tabs;
  const pane = tabs.named.panes + 1;
  const emptied: Tab = {
    id,
    name: EMPTY_TAB,
    defaultName: EMPTY_TAB,
    layout: { kind: "pane", pane, content: { kind: "empty", workspace } },
    focused: pane,
  };
  return {
    ...tabs,
    byId: { ...tabs.byId, [id]: emptied },
    named: { ...tabs.named, panes: pane },
  };
}

/**
 * **Where the front goes when tab `id` leaves it**: the tab before it on its strip as the strip
 * draws it, or the first one after it when it was first, or nothing when it was the only one.
 */
function frontWithout(
  tabs: Tabs,
  id: number,
  filedIn: FiledIn,
  pinned: Pinned,
): number | undefined {
  const strip = tabsIn(tabs, workspaceOf(tabs, id, filedIn), filedIn, pinned);
  const at = strip.indexOf(id);
  const beside = strip.filter((tab) => tab !== id);
  return at > 0 ? strip[at - 1] : beside[0];
}

/**
 * Which workspace a tab belongs to: **its own** once a press gave it one ({@link Tab.workspace}),
 * and otherwise **its first pane's.** A chat is filed where it works — the
 * plane's answer — and a view where it was opened, which it carries.
 */
export function workspaceOf(tabs: Tabs, id: number, filedIn: FiledIn): string | undefined {
  const own = tabs.byId[id]?.workspace;
  if (own !== undefined) return own;
  const first = contentsOf(tabs, id)[0]?.content;
  if (first === undefined) return undefined;
  return first.kind === "session" ? filedIn(first.session) : first.workspace;
}

/**
 * **The workspace a chat in tab `id` is filed in, where it is not the tab's** (#4982; #4583
 * rule 2): what its pane head names with that workspace's badge. A drag may put another
 * workspace's chat in a pane, and a press may leave one there ({@link Tab.workspace}); the chat
 * stays filed in its own workspace, and the head says which. `undefined` for a chat of the
 * tab's own workspace, and for a tab with no workspace to differ from.
 */
export function filedElsewhere(
  tabs: Tabs,
  id: number,
  session: number,
  filedIn: FiledIn,
): string | undefined {
  const own = workspaceOf(tabs, id, filedIn);
  if (own === undefined) return undefined;
  const filed = filedIn(session);
  return filed === own ? undefined : filed;
}

/**
 * Whether a tab is pinned, which is a fact about the operator and not about the chat.
 *
 * A function, as `FiledIn` and `LastMoved` are, so the strip's rules ask one question whatever
 * answers it: the window answers it from the tab's own pin ({@link pinsOf}).
 */
export type Pinned = (id: number) => boolean;

/** Each tab's own pin (#4935), as {@link Pinned} asks it. */
export function pinsOf(tabs: Tabs): Pinned {
  return (id) => tabs.byId[id]?.pinned === true;
}

/** Tab `id` pinned, or unpinned (#4935). Answers `tabs` itself when it already was, or when
 *  there is no such tab. */
export function setPinned(tabs: Tabs, id: number, pinned: boolean): Tabs {
  const tab = tabs.byId[id];
  if (tab === undefined || (tab.pinned === true) === pinned) return tabs;
  const next: Tab = { ...tab, pinned: true };
  if (!pinned) delete next.pinned;
  return { ...tabs, byId: { ...tabs.byId, [id]: next } };
}

/** Nothing pinned, which is what every caller that does not care about pins passes. */
export const nothingPinned: Pinned = () => false;

/**
 * The tabs on one workspace's strip, left to right — **pinned ones first** (ADR 0039).
 *
 * **This is not the strip re-ordering itself.** The rule ADR 0039 fixed is that a tab does not
 * MOVE under the cursor: an operator going back to the chat that was third from the left goes
 * there with their hand, and a strip that re-sorted on activity would turn every click into a
 * read. A pin is the opposite of that — it is the operator putting a tab where they want it,
 * once, deliberately, and it is what pinning means in every browser that has it. Nothing
 * moves that the operator did not move.
 *
 * Within each group the order is `tabs.order`'s, which is the order the chats were opened and
 * is never touched. Pinning two tabs does not sort them against each other.
 *
 * **And this is the whole of what a pin does to the overflow**, which ADR 0039 left open: a
 * pinned tab is first, so it is the last thing the strip scrolls away, and it needs no
 * exemption of its own. An exemption — a tab held out of the scroller — would be a second
 * mechanism deciding what is on screen, and the one thing the strip owes is that nothing in
 * it is unreachable.
 *
 * **No tab goes into the background any more** (#4760, spec #4750): a chat wrapping up keeps its
 * pane, where its head breathes, so the Smart close chip that went before the pinned tabs
 * (SI-8f) is gone.
 */
export function tabsIn(
  tabs: Tabs,
  workspace: string | undefined,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
): number[] {
  const here = tabs.order.filter((id) => workspaceOf(tabs, id, filedIn) === workspace);
  return [...here.filter(pinned), ...here.filter((id) => !pinned(id))];
}

/**
 * **What Close other tabs closes** (#4762): every other tab on the strip tab `keep` is on, in
 * the strip's order, except the pinned ones. A pin is the operator saying a tab stays, and a
 * sweep of the strip is the act most likely to take it with the rest. The tab it was asked on
 * stays, pinned or not.
 */
export function otherTabsToClose(
  tabs: Tabs,
  keep: number,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
): number[] {
  const strip = workspaceOf(tabs, keep, filedIn);
  return tabs.order.filter(
    (id) => id !== keep && !pinned(id) && workspaceOf(tabs, id, filedIn) === strip,
  );
}

/**
 * When a chat last moved, as the core counts moves across every plane. Bigger is more recent.
 *
 * A function rather than a field, for the same reason `FiledIn` is: the count is the core's,
 * it arrives on `chat-moved`, and a copy on the tab would be a second answer that nothing
 * invalidates when the next event lands.
 */
export type LastMoved = (session: number) => number;

/** When a TAB last moved: the most recent of the chats in its panes. */
export function movedAt(tabs: Tabs, id: number, lastMoved: LastMoved): number {
  return panesOf(tabs, id).reduce((most, pane) => Math.max(most, lastMoved(pane.session)), 0);
}

/**
 * `ids` most recently moved first — the order the overflow menu lists them in (ADR 0039).
 *
 * **Ties keep the order they came in**, which is the strip's, because `Array.sort` is stable
 * in every engine this app runs on. That matters more than it looks: a chat nothing has been
 * heard about reads `0`, so at a launch every tab ties and the menu is the strip's order
 * rather than a shuffle. A menu whose rows moved between two openings for no reason the
 * operator can see is the aiming defect ADR 0039 refuses, re-introduced in the one surface
 * that was allowed to sort.
 *
 * It answers a new array and never touches `tabs.order`. The strip's order is fixed.
 */
export function byLastActivity(ids: readonly number[], tabs: Tabs, lastMoved: LastMoved): number[] {
  return [...ids].sort(
    (one, other) => movedAt(tabs, other, lastMoved) - movedAt(tabs, one, lastMoved),
  );
}

/**
 * The arrangement after a workspace is focused: one of ITS tabs in front, and none at all
 * when it holds none.
 *
 * `prefer` is the tab that was in front there last, which is what an operator coming back to
 * a workspace means by it. A workspace with nothing in it puts nothing in front rather than
 * leaving another workspace's chat on screen under this workspace's empty strip.
 */
export function showWorkspace(
  tabs: Tabs,
  workspace: string | undefined,
  filedIn: FiledIn,
  prefer?: number,
  pinned: Pinned = nothingPinned,
): Tabs {
  // The strip's order, so "its first" is the tab the operator can see first — which is a
  // pinned one where there is one.
  const here = tabsIn(tabs, workspace, filedIn, pinned);
  const inFront =
    here.length === 0
      ? undefined
      : prefer !== undefined && here.includes(prefer)
        ? prefer
        : here[0];
  // `tabs` itself when that tab is in front already, so a caller can tell nothing moved.
  return inFront === tabs.inFront ? tabs : { ...tabs, inFront };
}

/** Brings tab `id` to the front: `tabs` itself when it is in front already, as {@link focusPane}. */
export function selectTab(tabs: Tabs, id: number): Tabs {
  return id in tabs.byId && tabs.inFront !== id ? { ...tabs, inFront: id } : tabs;
}

/** Focuses a pane of the tab in front, so the next split or close acts on it. The very tabs
 *  come back where that pane already has the focus: it is asked on every focus inside a pane,
 *  and new tabs draw the whole window again and read the sidebar again (#3136). */
export function focusPane(tabs: Tabs, pane: number): Tabs {
  const tab = frontTab(tabs);
  if (!tab || tab.focused === pane || !panes(tab.layout).some((each) => each.pane === pane))
    return tabs;
  return { ...tabs, byId: { ...tabs.byId, [tab.id]: { ...tab, focused: pane } } };
}

/**
 * Divides the focused pane in two, the new one showing `session` and focused.
 *
 * `chat` is the new chat's name. It defaults to the name of the tab's own chat, which is what a
 * split has always meant; a tab showing only a view has none, and the caller names it.
 */
export function splitFocusedPane(
  tabs: Tabs,
  direction: Direction,
  session: number,
  chat?: string,
): Tabs {
  const tab = frontTab(tabs);
  if (!tab) return tabs;
  return insertPane(tabs, {
    tab: tab.id,
    pane: tab.focused,
    direction,
    side: "after",
    content: {
      kind: "session",
      session,
      chat: chat ?? chatNameOf(tabs, tab.id) ?? String(session),
    },
  });
}

/** Which side of a pane a new one goes: `before` is the left or the top, `after` the right or
 *  the bottom. */
export type Side = "before" | "after";

/** Where {@link insertPane} puts a new pane, and what it shows. */
export type Insert = {
  tab: number;
  /** The pane the new one is put beside. */
  pane: number;
  direction: Direction;
  side: Side;
  content: Content;
};

/**
 * **A new pane beside pane `pane` of tab `tab`, on either side** (#4755): the two become a split
 * at an even share, the new pane its first child for `before` (left or top) and its second for
 * `after` (right or bottom), and the new pane is the tab's focused one. The tab need not be in
 * front, nor the pane focused, which is what a drop needs (#4763).
 *
 * Answers `tabs` itself when there is no such pane, or when `content` is a chat another pane
 * already shows: a chat is on screen in one pane at a time, and the core refuses a layout that
 * names one twice.
 */
export function insertPane(tabs: Tabs, at: Insert): Tabs {
  const tab = tabs.byId[at.tab];
  if (tab === undefined || !panes(tab.layout).some((each) => each.pane === at.pane)) return tabs;
  if (at.content.kind === "session" && shownElsewhere(tabs, at.content.session)) return tabs;
  const pane = tabs.named.panes + 1;
  const added: Layout = { kind: "pane", pane, content: at.content };
  const layout = replace(tab.layout, at.pane, (found) => ({
    kind: "split",
    direction: at.direction,
    children: at.side === "before" ? [added, found] : [found, added],
  }));
  return {
    ...tabs,
    byId: { ...tabs.byId, [tab.id]: { ...tab, layout, focused: pane } },
    named: { ...tabs.named, panes: pane },
  };
}

/**
 * **Pane `pane` of tab `id` shows `content`** (#4755): any open chat, not only its tab's own or
 * a task of it; a view; or nothing. A task it showed in place of its chat (`Tab.shows`) is
 * forgotten, since what it shows now is what it was given.
 *
 * Answers `tabs` itself when there is no such pane, when it shows `content` already, or when
 * `content` is a chat another pane shows: a chat is on screen in one pane at a time.
 */
export function showIn(tabs: Tabs, id: number, pane: number, content: Content): Tabs {
  const tab = tabs.byId[id];
  const found = tab === undefined ? undefined : panes(tab.layout).find((one) => one.pane === pane);
  if (tab === undefined || found === undefined) return tabs;
  const switched = tab.shows?.[pane] !== undefined;
  if (!switched && sameContent(found.content, content)) return tabs;
  if (content.kind === "session" && shownElsewhere(tabs, content.session, id, pane)) return tabs;
  const layout = replace(tab.layout, pane, (one) => ({ ...one, content }));
  const shows = Object.fromEntries(
    Object.entries(tab.shows ?? {}).filter(([at]) => Number(at) !== pane),
  ) as Record<number, number>;
  return { ...tabs, byId: { ...tabs.byId, [id]: showing({ ...tab, layout }, shows) } };
}

/**
 * Whether a pane other than pane `pane` of tab `tab` shows chat `session`: what the core refuses
 * twice is what each pane SHOWS (`layoutOf`), so a chat with a task drawn over it in its own pane
 * is not counted there.
 */
function shownElsewhere(tabs: Tabs, session: number, tab?: number, pane?: number): boolean {
  return tabs.order.some((id) =>
    shownIn(tabs, id).some((one) => one.session === session && !(id === tab && one.pane === pane)),
  );
}

/** Why a one-job program's pane has no close, and cannot be picked up (#4761). */
export const ONE_JOB_STAYS = "This pane goes when its program exits. Quit the program to close it.";

/** Why a tab holding a one-job program has no close (#4761): it would leave the program running
 *  with no pane to draw it. */
export const ONE_JOB_IN_TAB =
  "A program in this tab goes when it exits. Quit it first, then close the tab.";

/**
 * **A one-job program opens beside the focused pane** (#4761; spec #4750 story 66): a terminal
 * editor opened from a file, or a forge login, as a split to the right of the pane with the
 * keyboard, in the same tab and with the keyboard, so what the person was reading stays on
 * screen. That pane is where the keyboard goes back to when it exits ({@link oneJobEnded}).
 * With no tab in front it opens in a tab of its own, which goes with it.
 */
export function openOneJob(tabs: Tabs, session: number, chat: string): Tabs {
  const tab = frontTab(tabs);
  if (tab === undefined) {
    const opened = openTab(tabs, session, chat);
    const id = opened.inFront;
    if (id === undefined) return opened;
    const own = opened.byId[id];
    const layout = replace(own.layout, own.focused, (found) =>
      found.content.kind === "session"
        ? { ...found, content: { ...found.content, oneJob: {} } }
        : found,
    );
    return { ...opened, byId: { ...opened.byId, [id]: { ...own, layout } } };
  }
  return insertPane(tabs, {
    tab: tab.id,
    pane: tab.focused,
    direction: "row",
    side: "after",
    content: { kind: "session", session, chat, oneJob: { returnTo: tab.focused } },
  });
}

/**
 * **A one-job program exited: its split goes** (#4761; spec #4750 story 66). The pane it was
 * beside takes the split's place, and the keyboard goes back to the pane it opened from, or,
 * where that has gone, to the first pane left. A tab it was alone in closes. Answers `tabs`
 * itself where no pane shows it.
 */
export function oneJobEnded(
  tabs: Tabs,
  session: number,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
): Tabs {
  const found = oneJobAt(tabs, session);
  if (found === undefined) return tabs;
  const tab = tabs.byId[found.tab];
  const left = without(tab.layout, found.pane);
  if (!left) return closeTab(tabs, tab.id, filedIn, pinned);
  const kept = panes(left);
  const focused =
    tab.focused !== found.pane
      ? tab.focused
      : kept.some((one) => one.pane === found.returnTo)
        ? (found.returnTo as number)
        : kept[0].pane;
  return {
    ...tabs,
    byId: { ...tabs.byId, [tab.id]: showing({ ...tab, layout: left, focused }, tab.shows) },
  };
}

/** Where one-job program `session` is drawn, and the pane it opened from. */
function oneJobAt(
  tabs: Tabs,
  session: number,
): { tab: number; pane: number; returnTo?: number } | undefined {
  for (const id of tabs.order) {
    for (const { pane, content } of contents(tabs.byId[id].layout))
      if (content.kind === "session" && content.session === session && content.oneJob)
        return { tab: id, pane, returnTo: content.oneJob.returnTo };
  }
  return undefined;
}

/** Whether `content` is a one-job program ({@link openOneJob}): the one test of it (#4761). */
export function isOneJob(content: Content | undefined): boolean {
  return content?.kind === "session" && content.oneJob !== undefined;
}

/** Whether pane `pane`, in any tab, shows a one-job program ({@link openOneJob}). */
export function isOneJobPane(tabs: Tabs, pane: number): boolean {
  return tabs.order.some((id) =>
    contents(tabs.byId[id].layout).some((one) => one.pane === pane && isOneJob(one.content)),
  );
}

/**
 * **A shell whose program exited leaves its pane empty** (#4761; spec #4750 story 64, ADR 0062
 * as amended 2026-11-02): the pane stays in its place, showing nothing, and the keyboard stays
 * where it was. The core closes the shell, so its row goes. Answers `tabs` itself where no pane
 * shows it.
 */
export function shellExited(tabs: Tabs, session: number, filedIn: FiledIn): Tabs {
  for (const id of tabs.order) {
    const at = contents(tabs.byId[id].layout).find(
      (one) => one.content.kind === "session" && one.content.session === session,
    );
    if (at !== undefined)
      return showIn(tabs, id, at.pane, { kind: "empty", workspace: filedIn(session) });
  }
  return tabs;
}

/** A chat a press shows: its number, its own name, and what a tab named after it says. */
export type PressedChat = {
  session: number;
  /** The chat's own name ({@link Content}'s `chat`). */
  chat: string;
  /** What its default name puts first: its persona, or its program. */
  who?: string | null;
  /** The name the person gave it, where they gave one. */
  label?: string | null;
};

/** Each strip's front tab, by workspace, as the window last left it. */
export type Fronts = Readonly<Record<string, number>>;

/** What a press needs that the tabs do not hold. */
export type Pressing = {
  filedIn: FiledIn;
  /** The tab last in front on a workspace's strip, where the window remembers one. */
  frontOf: (workspace: string) => number | undefined;
};

/** What a press leaves: the tabs, and the strip in front now, which the window draws. */
export type Press = { tabs: Tabs; workspace: string | undefined };

/** A pane of a tab. */
export type PaneAt = { tab: number; pane: number };

/**
 * **What is opening** (#5289): a chat that has just started, or one the person pressed.
 *
 * - `new.strip` is the strip a start named ("New shell in beta", a curation, a Fix), and its
 *   front tab is the one read. With none, the tab in front of the window is, whichever strip
 *   the chat is filed on.
 * - `new.asked` is the empty pane New chat was pressed for, kept while it stays empty, since the
 *   picker can stay open for a while.
 * - `new.ownTab` is a start from the strip's `+` list (#5292), each of whose rows says "a new tab
 *   with this": it gets a tab of its own even where the front tab has an empty pane.
 * - `pressed.into` is a pane the press itself names: a breadcrumb's, or ⌘⇧J's pick.
 */
export type Opening =
  | { kind: "new"; strip?: string; asked?: PaneAt; ownTab?: boolean }
  | { kind: "pressed"; session: number; into?: PaneAt };

/** **Where it goes:** to its pane on screen, into an empty pane, in place of the chat a one-pane
 *  tab shows, or to a tab of its own. */
export type Opens = Lands | { how: "new-tab" };

/** An opening that lands in a pane there is already. */
type Lands = { how: "goes-to" | "fills" | "shows"; at: PaneAt };

/**
 * **One rule says where a chat opens** (#5289; spec #5288 stories 66 to 72), and the new chat
 * button, the keyboard, the palette and a pressed row all ask it.
 *
 * - **A new chat never replaces anything.** It fills an empty pane of the front tab, and
 *   otherwise gets a tab of its own, in a split or a single pane.
 * - **A pressed chat on screen** is gone to, in its tab and pane, so it never shows twice.
 * - **A pressed chat off screen** fills an empty pane of the front tab. In a one-pane tab it
 *   shows there, and the chat it replaces keeps running in the list. In a split it gets a tab of
 *   its own, so the layout survives.
 * - **A view or a one-job program is never replaced.** With a view alone in the front tab, the
 *   press goes to the pane used last on the strip that a press could take (spec #4750 story 5),
 *   or else to a tab of its own. With a program alone there, it goes to a tab of its own.
 */
export function opensAt(tabs: Tabs, opening: Opening, pressing: Pressing): Opens {
  if (opening.kind === "new") return newChatOpensAt(tabs, opening, pressing);
  const on = whereShown(tabs, opening.session);
  if (on !== undefined) return { how: "goes-to", at: on };
  if (opening.into !== undefined) return { how: "shows", at: opening.into };
  const strip = pressing.filedIn(opening.session);
  const front = frontOn(tabs, strip, pressing);
  if (front === undefined) return { how: "new-tab" };
  const here = takenBy(tabs, front);
  if (here !== undefined) return here;
  // A split keeps its layout, and a lone program is no reason to replace a chat elsewhere.
  const [only, ...others] = contents(tabs.byId[front].layout);
  if (others.length > 0 || isOneJob(only.content)) return { how: "new-tab" };
  // Stable, so panes never used keep the strip's order.
  const used = tabs.used ?? {};
  const [last] = tabsIn(tabs, strip, pressing.filedIn)
    .flatMap((id) => {
      const opens = takenBy(tabs, id);
      return opens === undefined ? [] : [opens];
    })
    .sort((one, other) => (used[other.at.pane] ?? 0) - (used[one.at.pane] ?? 0));
  return last ?? { how: "new-tab" };
}

/** {@link opensAt} for a chat that has just started. */
function newChatOpensAt(
  tabs: Tabs,
  { strip, asked, ownTab = false }: Extract<Opening, { kind: "new" }>,
  pressing: Pressing,
): Opens {
  if (ownTab) return { how: "new-tab" };
  const stillEmpty =
    asked !== undefined &&
    tabs.byId[asked.tab] !== undefined &&
    contentsOf(tabs, asked.tab).some(
      (one) => one.pane === asked.pane && one.content.kind === "empty",
    );
  if (stillEmpty) return { how: "fills", at: asked };
  const front = strip === undefined ? tabs.inFront : frontOn(tabs, strip, pressing);
  const empty = front === undefined ? undefined : emptyPaneOf(tabs, front);
  return empty === undefined ? { how: "new-tab" } : { how: "fills", at: empty };
}

/** The tab a press reads on `workspace`'s strip: the one in front of the window where that is on
 *  the strip, else the one last left in front, else its first. None on a strip with no tabs. */
function frontOn(tabs: Tabs, workspace: string, pressing: Pressing): number | undefined {
  const here = tabsIn(tabs, workspace, pressing.filedIn);
  const remembered = pressing.frontOf(workspace);
  if (tabs.inFront !== undefined && here.includes(tabs.inFront)) return tabs.inFront;
  return remembered !== undefined && here.includes(remembered) ? remembered : here[0];
}

/** Tab `id`'s empty pane: the focused one where that is empty, else its first. */
function emptyPaneOf(tabs: Tabs, id: number): PaneAt | undefined {
  const tab = tabs.byId[id];
  const empties = contents(tab.layout).filter((one) => one.content.kind === "empty");
  const pane = empties.find((one) => one.pane === tab.focused) ?? empties[0];
  return pane === undefined ? undefined : { tab: id, pane: pane.pane };
}

/** How tab `id` takes a pressed chat itself: in an empty pane, or in place of the chat it holds
 *  alone. Nothing for a split with no empty pane, a lone view and a lone one-job program. */
function takenBy(tabs: Tabs, id: number): Lands | undefined {
  const empty = emptyPaneOf(tabs, id);
  if (empty !== undefined) return { how: "fills", at: empty };
  const [only, ...others] = contents(tabs.byId[id].layout);
  if (others.length > 0 || only.content.kind !== "session" || isOneJob(only.content))
    return undefined;
  return { how: "shows", at: { tab: id, pane: only.pane } };
}

/**
 * **The pane in front now was used** (#4757): the focused pane of the tab in front is stamped
 * as the most recent, which is what a press with a view focused reads ({@link press}). The
 * window calls it on every change; it answers `tabs` itself when that pane is already the
 * latest, so a change that moved nothing stays one.
 */
export function noteUsed(tabs: Tabs): Tabs {
  const tab = frontTab(tabs);
  if (tab === undefined) return tabs;
  const used = tabs.used ?? {};
  const latest = Math.max(0, ...Object.values(used));
  if (latest > 0 && used[tab.focused] === latest) return tabs;
  return { ...tabs, used: { ...used, [tab.focused]: latest + 1 } };
}

/**
 * **A press shows chat `chat`** (#4757, #5289): a sidebar row, a task, a Go, a name in a
 * breadcrumb, where {@link opensAt} says. The chat a pane showed before keeps running: nothing
 * here ends anything, and the pane remembers it for ⌘⇧H ({@link back}). `into` is the pane a
 * breadcrumb or ⌘⇧J's pick was pressed in.
 *
 * A tab whose pane is given a chat keeps its strip ({@link Tab.workspace}), and a tab whose
 * first pane is given one is named after it, as a tab opened on it would be.
 */
export function press(tabs: Tabs, chat: PressedChat, pressing: Pressing, into?: PaneAt): Press {
  return opened(
    tabs,
    chat,
    pressing,
    opensAt(tabs, { kind: "pressed", session: chat.session, into }, pressing),
  );
}

/**
 * **A new chat opens** (#5289) where {@link opensAt} says, and replaces nothing: `named` for a
 * start that named its strip, so the chat's own is the one read, `asked` for the empty pane
 * New chat was pressed for, and `ownTab` for a start from the strip's `+` list (#5292).
 */
export function openNew(
  tabs: Tabs,
  chat: PressedChat,
  pressing: Pressing,
  {
    named = false,
    asked,
    ownTab = false,
  }: { named?: boolean; asked?: PaneAt; ownTab?: boolean } = {},
): Press {
  const strip = named ? pressing.filedIn(chat.session) : undefined;
  return opened(
    tabs,
    chat,
    pressing,
    opensAt(tabs, { kind: "new", strip, asked, ownTab }, pressing),
  );
}

/** `chat` where `opens` says, stamped as used, and the strip in front now. */
function opened(tabs: Tabs, chat: PressedChat, pressing: Pressing, opens: Opens): Press {
  const next =
    opens.how === "new-tab"
      ? openTab(tabs, chat.session, chat.chat, chat.who ?? null, chat.label ?? null)
      : opens.how === "goes-to"
        ? focusAt(tabs, opens.at)
        : showChatAt(tabs, opens.at, chat, pressing.filedIn);
  const used = noteUsed(next);
  return { tabs: used, workspace: stripInFront(used, pressing.filedIn) };
}

/**
 * **Whether a press on chat `session` would move this window to another strip** (#5330): the
 * chat is in no pane here, and it is filed on a strip other than the one in front. A tab window
 * holds only the tabs torn off into it, so it takes no such press: the rule would give the chat
 * a tab there and the window would show another strip. The project window, which holds every
 * strip, takes it. A chat on screen here is gone to wherever it is, as anywhere.
 */
export function wouldLeaveStrip(tabs: Tabs, session: number, filedIn: FiledIn): boolean {
  if (whereShown(tabs, session) !== undefined) return false;
  const shown = stripInFront(tabs, filedIn);
  return shown !== undefined && filedIn(session) !== shown;
}

/** The strip of the tab in front, which the window draws, or none with nothing in front. */
function stripInFront(tabs: Tabs, filedIn: FiledIn): string | undefined {
  return tabs.inFront === undefined ? undefined : workspaceOf(tabs, tabs.inFront, filedIn);
}

/**
 * **⌘⇧H: the focused pane goes back to the chat it showed before** (#4757; spec story 86), and
 * a second press comes back again. Where that chat is on screen in another pane now, that pane
 * is jumped to instead. `chat` names a chat that is still open, and nothing for one that has
 * ended; answers `tabs` itself where the pane showed nothing else, or that chat has ended.
 */
export function back(
  tabs: Tabs,
  at: Pressing,
  chat: (session: number) => PressedChat | undefined,
): Press {
  const tab = frontTab(tabs);
  const before = tab?.previous?.[tab.focused];
  const named = before === undefined ? undefined : chat(before);
  if (tab === undefined || named === undefined)
    return { tabs, workspace: stripInFront(tabs, at.filedIn) };
  return press(tabs, named, at, { tab: tab.id, pane: tab.focused });
}

/**
 * **The chats the tab in front shows in its other panes** (#4757; spec story 15): what the
 * sidebar marks faintly, beside the focused pane's chat ({@link focusedChat}) it highlights.
 */
export function shownBeside(tabs: Tabs): number[] {
  const tab = frontTab(tabs);
  if (tab === undefined) return [];
  return shownIn(tabs, tab.id)
    .filter((one) => one.pane !== tab.focused)
    .map((one) => one.session);
}

/** The pane that shows chat `session`, in any tab. */
function whereShown(tabs: Tabs, session: number): PaneAt | undefined {
  for (const id of tabs.order) {
    const one = shownIn(tabs, id).find((each) => each.session === session);
    if (one !== undefined) return { tab: id, pane: one.pane };
  }
  return undefined;
}

/** Tab `at.tab` in front, with pane `at.pane` focused. */
function focusAt(tabs: Tabs, at: PaneAt): Tabs {
  const tab = tabs.byId[at.tab];
  if (tab.focused === at.pane && tabs.inFront === at.tab) return tabs;
  return {
    ...tabs,
    byId: { ...tabs.byId, [at.tab]: { ...tab, focused: at.pane } },
    inFront: at.tab,
  };
}

/** Pane `at.pane` of tab `at.tab` shows `chat`, has the keyboard, and its tab is in front. */
function showChatAt(tabs: Tabs, at: PaneAt, chat: PressedChat, filedIn: FiledIn): Tabs {
  const tab = tabs.byId[at.tab];
  if (tab === undefined) return tabs;
  const before = shownIn(tabs, at.tab).find((one) => one.pane === at.pane)?.session;
  const strip = workspaceOf(tabs, at.tab, filedIn);
  const shown = showIn(tabs, at.tab, at.pane, {
    kind: "session",
    session: chat.session,
    chat: chat.chat,
  });
  const after = shown.byId[at.tab];
  const previous =
    before !== undefined && before !== chat.session
      ? { ...after.previous, [at.pane]: before }
      : after.previous;
  const lead = contents(after.layout)[0]?.pane === at.pane;
  const workspace = after.workspace ?? strip;
  const named = chat.who ? `${chat.who} ${chat.chat}` : chat.chat;
  return {
    ...shown,
    byId: {
      ...shown.byId,
      [at.tab]: {
        ...after,
        focused: at.pane,
        ...(workspace !== undefined ? { workspace } : {}),
        ...(previous !== undefined ? { previous } : {}),
        ...(lead && shown !== tabs ? { name: chat.label ?? named, defaultName: named } : {}),
      },
    },
    inFront: at.tab,
  };
}

/** What a tab is called when a content is its first pane: the name it says, and what that goes
 *  back to when a given name is taken off ({@link Tab.defaultName}). */
export type TabName = { name: string; defaultName: string };

/**
 * **A tab's name** (#5342, spec #5339, amending #4762): its own name if the person gave it one,
 * otherwise its first pane's, the layout's first leaf at top left. Which pane has the keyboard
 * plays no part, so splitting and moving between panes never rename a tab.
 * {@link namedForFirstPanes} keeps the first pane's name current as the layout changes.
 */
export function tabNameOf(tab: Tab): string {
  return tab.own ?? tab.name;
}

/** Which tabs {@link namedForFirstPanes} renames beyond those whose first pane is another
 *  pane: `inPlace` also renames one whose first pane shows something else, as a drop's swap
 *  does. Every other change that puts something in a first pane in place names the tab itself. */
export type Renaming = { inPlace?: boolean };

/**
 * **`after`, a change made to `before`, with each tab whose first pane is another pane named
 * for what that pane shows** (#5342, #5308), by `nameOf`, read against `before`. The first chat
 * closed or moved out, so the tab takes the next one's name; or a chat was put left of it, and
 * is first now.
 *
 * A change that named the tab itself, as Undo does, keeps the name it gave. A tab's own name
 * ({@link Tab.own}) is kept: only the name under it moves. Answers `after` itself where no tab
 * is renamed.
 */
export function namedForFirstPanes(
  before: Tabs,
  after: Tabs,
  nameOf: (content: Content, before: Tabs) => TabName,
  { inPlace = false }: Renaming = {},
): Tabs {
  let byId: Record<number, Tab> | undefined;
  for (const id of after.order) {
    const was = before.byId[id] && panes(before.byId[id].layout)[0];
    const is = after.byId[id] && panes(after.byId[id].layout)[0];
    if (was === undefined || is === undefined) continue;
    const other = was.pane !== is.pane || (inPlace && !sameContent(was.content, is.content));
    if (!other) continue;
    const tab = after.byId[id];
    // A change that named the tab itself, as Undo does, knew what to call it.
    const kept = before.byId[id];
    if (tab.name !== kept.name || tab.defaultName !== kept.defaultName) continue;
    const { name, defaultName } = nameOf(is.content, before);
    if (tab.name === name && tab.defaultName === defaultName) continue;
    byId ??= { ...after.byId };
    byId[id] = { ...tab, name, defaultName };
  }
  return byId === undefined ? after : { ...after, byId };
}

/** Whether two contents show the same thing. */
export function sameContent(a: Content, b: Content): boolean {
  if (a.kind === "session" && b.kind === "session") return a.session === b.session;
  if (a.kind === "view" && b.kind === "view") return viewKey(a.view) === viewKey(b.view);
  return a.kind === "empty" && b.kind === "empty";
}

/**
 * **Close pane** (#4756): what shared its split takes the split's place; when it was the tab's
 * only pane, the tab closes with it by {@link closeLayout}'s rule. Its chat ends nothing and goes
 * on in the sidebar: which chats end is no part of a close.
 */
export function closeFocusedPane(
  tabs: Tabs,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
): Tabs {
  const tab = frontTab(tabs);
  if (!tab) return tabs;
  return closePane(tabs, tab.id, tab.focused, filedIn, pinned);
}

/**
 * **Closes pane `pane` of tab `id`**, focused or not, as Close pane closes the focused one: what
 * shared its split takes its place, and a tab's only pane closes the tab by `closeLayout`'s rule,
 * so the strip's last tab keeps one empty pane. A pane the core moved to another window (#4768)
 * goes this way, and so does Clear pane on an ended chat's band (#5019). The keyboard stays
 * where it was, unless it was on that pane. Answers `tabs` itself where tab `id` has no pane
 * `pane`.
 */
export function closePane(
  tabs: Tabs,
  id: number,
  pane: number,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
): Tabs {
  const tab = tabs.byId[id];
  if (tab === undefined || !panes(tab.layout).some((one) => one.pane === pane)) return tabs;
  const left = without(tab.layout, pane);
  if (!left) return closeLayout(tabs, tab.id, filedIn, pinned);
  // **The keyboard goes to the pane that takes the space** (spec #4750): the nearest pane of
  // the sibling that takes the closed pane's place. A pane closed while another had the
  // keyboard leaves the keyboard where it was.
  const focused =
    tab.focused === pane ? (takesTheSpace(tab.layout, pane) ?? panes(left)[0].pane) : tab.focused;
  const kept = showing({ ...tab, layout: left, focused }, tab.shows);
  return { ...tabs, byId: { ...tabs.byId, [id]: kept } };
}

/**
 * Takes chat `session`'s pane away, wherever it is — **the window catching up with a chat the core
 * has already closed**: a smart close whose record landed (ADR 0064). What shared its split takes
 * its place; when it was its tab's only pane, the tab closes by `closeTab`'s rule. A session no
 * tab shows changes nothing, and nor does one `keeps` says stays in its pane as ended.
 */
export function closeChat(
  tabs: Tabs,
  session: number,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
  keeps: (session: number) => boolean = () => false,
): Tabs {
  const found = tabs.order
    .flatMap((id) => panesOf(tabs, id).map((one) => ({ id, ...one })))
    .find((one) => one.session === session);
  // **A task shown in the pane it returns to is not taken off the screen by its end** (#1486,
  // V100-37): a pane never changes what it shows by itself. The tab keeps showing it, as
  // ended ({@link shownLive}), until the person leaves it.
  if (found === undefined) return tabs;
  // **A chat `keeps` says stays in its pane, as ended** (#4757): a task a press put there. The
  // pane does not change what it shows by itself (V100-37); the person leaves it.
  if (keeps(session)) return tabs;
  const { id, pane } = found;
  const tab = tabs.byId[id];
  const left = without(tab.layout, pane);
  if (!left) return closeTab(tabs, id, filedIn, pinned);
  const focused = tab.focused === pane ? panes(left)[0].pane : tab.focused;
  const kept = showing({ ...tab, layout: left, focused }, tab.shows);
  return { ...tabs, byId: { ...tabs.byId, [id]: kept } };
}

/**
 * The panes of a tab **that show a chat**, left to right and top to bottom.
 *
 * Every caller of this is asking about chats — which to end when the tab closes, which one a
 * queue row brings forward, when the tab last moved — and a pane showing a view has none of
 * those. {@link contentsOf} is every pane.
 */
export function panesOf(tabs: Tabs, id: number): { pane: number; session: number }[] {
  return contentsOf(tabs, id).flatMap(({ pane, content }) =>
    content.kind === "session" ? [{ pane, session: content.session }] : [],
  );
}

/**
 * **The chat `from` was showing, now `to`, in the same pane** (NO-3's Start fresh): the same
 * chat started again in a new session. Its tab, its place on the strip, its split and the
 * pane's focus all stay as they were; every other pane is left alone. Answers `tabs` itself
 * when no pane shows `from`.
 *
 * **Its Activity and Network tabs go with it** (#1495, #1662): those views are keyed by the
 * chat's number, which is all the window knows a chat by, so one open on `from` is about `to`
 * from here on. It would otherwise say the chat is not open, of a chat that is.
 */
export function replaceSession(tabs: Tabs, from: number, to: number): Tabs {
  const moved = viewsFollow(tabs, from, to);
  for (const id of moved.order) {
    const tab = moved.byId[id];
    const found = contents(tab.layout).find(
      ({ content }) => content.kind === "session" && content.session === from,
    );
    if (!found || found.content.kind !== "session") continue;
    const content: Content = { ...found.content, session: to };
    const layout = replace(tab.layout, found.pane, (pane) => ({ ...pane, content }));
    return { ...moved, byId: { ...moved.byId, [id]: { ...tab, layout } } };
  }
  // A task shown in the pane it returns to, started again: the pane shows the new one (#1486).
  for (const id of moved.order) {
    const tab = moved.byId[id];
    const at = Object.entries(tab.shows ?? {}).find(([, shown]) => shown === from);
    if (at === undefined) continue;
    const kept = showing(tab, { ...tab.shows, [Number(at[0])]: to });
    return { ...moved, byId: { ...moved.byId, [id]: kept } };
  }
  return moved;
}

/** `tabs` with every Activity and Network view of chat `from` about chat `to`; `tabs` itself
 *  where none is. */
function viewsFollow(tabs: Tabs, from: number, to: number): Tabs {
  const was = String(from);
  let byId = tabs.byId;
  for (const id of tabs.order) {
    let layout = byId[id].layout;
    for (const { pane, content } of contents(layout)) {
      if (content.kind !== "view" || content.view.key !== was) continue;
      const view = isActivity(content.view)
        ? activityView(to)
        : isChatNetwork(content.view)
          ? chatNetworkView(to)
          : undefined;
      if (view === undefined) continue;
      const now: Content = { ...content, view };
      layout = replace(layout, pane, (found) => ({ ...found, content: now }));
    }
    if (layout !== byId[id].layout) byId = { ...byId, [id]: { ...byId[id], layout } };
  }
  return byId === tabs.byId ? tabs : { ...tabs, byId };
}

/**
 * **The chats open in this window, as one key** (#4729): every session a tab's pane holds, in
 * ascending order. Equal for two `Tabs` that differ only in which tab is in front, how they are
 * ordered or split, or what a pane shows in place of its own chat, so what is read again when a
 * chat is opened or closed is not read again for a switch.
 */
export function openChatsKey(tabs: Tabs): string {
  const open = new Set<number>();
  for (const id of tabs.order) for (const { session } of panesOf(tabs, id)) open.add(session);
  return [...open].sort((one, other) => one - other).join(",");
}

/** The sessions with a pane on screen: the only ones a terminal is drawing. */
export function visibleSessions(tabs: Tabs): number[] {
  return tabs.inFront === undefined ? [] : shownIn(tabs, tabs.inFront).map((one) => one.session);
}

/**
 * **Which chat started a chat**, by number, or nothing for one a person opened (#1486).
 *
 * A function rather than a field, for {@link FiledIn}'s reason: who asked whom is the core's
 * answer, and the tabs keep only what each tab shows.
 */
export type AskedBy = (session: number) => number | undefined;

/**
 * **A chat's home: the tab and pane of the nearest chat, itself or above it, that has a pane
 * of its own** (#1486, #5667). A chat in a pane is at home in it. A task with no pane of its
 * own is a row in the Chats list under the chat that asked, and is at home in that chat's pane,
 * or the pane of the nearest chat above that one: where its Notices are drawn and its tab's
 * chip counts it. A task given a pane of its own is at home there, and so is everything below it.
 *
 * Nothing when no chat above it has a pane in this window: the chat that asked was closed and
 * left its tasks, or is in another window.
 */
export function homeOf(
  tabs: Tabs,
  chat: number,
  askedBy: AskedBy,
): { tab: number; pane: number; own: number } | undefined {
  const owned = ownedIn(tabs);
  // A lineage that loops back on itself is walked once and no further.
  const seen = new Set<number>();
  for (let at: number | undefined = chat; at !== undefined && !seen.has(at); at = askedBy(at)) {
    seen.add(at);
    const home = owned.get(at);
    if (home !== undefined) return { ...home, own: at };
  }
  return undefined;
}

/** Each chat that has a pane of its own, by number: built once for each value of the tabs,
 *  since every row of a list and every tab's menu asks for a chat's home. */
const OWNED = new WeakMap<Tabs, Map<number, { tab: number; pane: number }>>();

function ownedIn(tabs: Tabs): Map<number, { tab: number; pane: number }> {
  const held = OWNED.get(tabs);
  if (held !== undefined) return held;
  const owned = new Map<number, { tab: number; pane: number }>();
  for (const tab of tabs.order)
    for (const { pane, session } of panesOf(tabs, tab))
      if (!owned.has(session)) owned.set(session, { tab, pane });
  OWNED.set(tabs, owned);
  return owned;
}

/**
 * **What each chat pane of a tab shows, and whether that chat is there to be shown** (#1486).
 *
 * `live` is the one invariant a pane's drawing rests on: **a pane draws another chat's
 * terminal only while that chat is open and at home in that very pane** ({@link homeOf}). A
 * pane's own chat is always live. A shown task is not when it has ended, when a chat between
 * it and the session has ended, when the session was started again and the lineage has not
 * caught up, or when it was given a tab of its own. Then the pane still shows it, as the
 * person left it, and draws no terminal for it: nothing typed can reach a chat the pane cannot
 * say the path to.
 */
export function shownLive(
  tabs: Tabs,
  id: number,
  askedBy: AskedBy,
  open: (session: number) => boolean,
): { pane: number; own: number; session: number; live: boolean }[] {
  return shownIn(tabs, id).map((one) => {
    if (one.session === one.own) return { ...one, live: true };
    const home = open(one.session) ? homeOf(tabs, one.session, askedBy) : undefined;
    return { ...one, live: home !== undefined && home.tab === id && home.pane === one.pane };
  });
}

/**
 * **Pane `pane` of tab `id` shows its own chat again** (#1486): the way out of a task that has
 * ended. Answers `tabs` itself when it showed its own already.
 */
export function showOwnIn(tabs: Tabs, id: number, pane: number): Tabs {
  const tab = tabs.byId[id];
  if (tab?.shows?.[pane] === undefined) return tabs;
  const rest = Object.fromEntries(
    Object.entries(tab.shows).filter(([at]) => Number(at) !== pane),
  ) as Record<number, number>;
  return { ...tabs, byId: { ...tabs.byId, [id]: showing(tab, rest) } };
}

/**
 * **Switches a tab to chat `chat`** (#1486): the tab that is the chat's home ({@link homeOf})
 * comes to the front, the pane of the session that asked shows the chat in its own chat's
 * place (`Tab.shows`) and takes the keyboard. Answers `tabs` itself when the chat has no home
 * in this window, or when it is already what is on screen.
 *
 * **No press calls it since #4757**, which shows a chat in the pane itself ({@link press}).
 * It stays as the one way to build the state a record from before layouts were kept puts back
 * (`restoreShown`), which the readers of `Tab.shows` are tested against; it goes with that
 * field once #4758's upgrade reads those records.
 */
export function switchTabTo(tabs: Tabs, chat: number, askedBy: AskedBy): Tabs {
  const home = homeOf(tabs, chat, askedBy);
  if (home === undefined) return tabs;
  const tab = tabs.byId[home.tab];
  const shown = tab.shows?.[home.pane] ?? home.own;
  if (shown === chat && tab.focused === home.pane && tabs.inFront === tab.id) return tabs;
  const next = showing({ ...tab, focused: home.pane }, { ...tab.shows, [home.pane]: chat });
  return { ...tabs, byId: { ...tabs.byId, [tab.id]: next }, inFront: tab.id };
}

/**
 * **Tab `id` shows its session's own chat again**, in every pane (#1486): what pressing the
 * tab does when it is already in front. Answers `tabs` itself for a tab that shows no task.
 */
export function showOwn(tabs: Tabs, id: number): Tabs {
  const tab = tabs.byId[id];
  if (tab?.shows === undefined) return tabs;
  return { ...tabs, byId: { ...tabs.byId, [id]: showing(tab, undefined) } };
}

/**
 * **What each chat pane of a tab shows**, left to right and top to bottom (#1486): `own` is
 * the pane's own chat, the session the tab is, and `session` is the chat drawn in it, which is
 * the same chat unless the pane shows a task. {@link panesOf} is the tab's own chats.
 */
export function shownIn(tabs: Tabs, id: number): { pane: number; own: number; session: number }[] {
  const shows = tabs.byId[id]?.shows;
  return panesOf(tabs, id).map(({ pane, session }) => ({
    pane,
    own: session,
    session: shows?.[pane] ?? session,
  }));
}

/**
 * **The task tab `id` shows in place of its own chat**, or nothing when it shows that chat
 * (#1486). The tab's own chat is its first pane's ({@link chatOf}), so this is what the tab's
 * label says after the session's name.
 */
export function taskShownIn(tabs: Tabs, id: number): number | undefined {
  const [lead] = contentsOf(tabs, id);
  if (lead?.content.kind !== "session") return undefined;
  return tabs.byId[id].shows?.[lead.pane];
}

/**
 * **The chat tab `id` shows** (#1486): the task its first pane is switched to, and otherwise
 * the tab's own chat. Nothing for a tab that opened on a view. What a thing dropped on the tab
 * is handed to, since the tab reads as the chat it shows, and the number its tooltip says and
 * the palette finds it by (#6212, #6213).
 */
export function chatShownBy(tabs: Tabs, id: number): number | undefined {
  return taskShownIn(tabs, id) ?? chatOf(tabs, id);
}

/**
 * **A tab's layout as it is drawn** (#1486): each pane holding the chat it shows. The tab's
 * own layout, untouched, for a tab that shows no task.
 */
export function layoutShown(tabs: Tabs, id: number): Layout | undefined {
  const tab = tabs.byId[id];
  if (tab === undefined) return undefined;
  const shows = tab.shows;
  if (shows === undefined) return tab.layout;
  const draw = (layout: Layout): Layout => {
    if (layout.kind === "split")
      return { ...layout, children: layout.children.map(draw) as [Layout, Layout] };
    const shown = shows[layout.pane];
    return shown === undefined || layout.content.kind !== "session"
      ? layout
      : { ...layout, content: { ...layout.content, session: shown } };
  };
  return draw(tab.layout);
}

/**
 * **The chat that has the keyboard**: the one shown in the focused pane of the tab in front,
 * or nothing when that pane shows a view or nothing is in front (#1486).
 */
export function focusedChat(tabs: Tabs): number | undefined {
  const tab = frontTab(tabs);
  return tab && shownIn(tabs, tab.id).find((one) => one.pane === tab.focused)?.session;
}

/**
 * **Every tab stops showing a task that is not `open` any more** (#1486): it shows its
 * session's own chat, which is what was under the task all along. Answers `tabs` itself when
 * every shown task is still open, so a caller can tell.
 */
export function keepShown(tabs: Tabs, open: (session: number) => boolean): Tabs {
  let dropped = false;
  const byId = Object.fromEntries(
    tabs.order.map((id) => {
      const tab = tabs.byId[id];
      if (tab.shows === undefined) return [id, tab];
      const kept = Object.entries(tab.shows).filter(([, session]) => open(session));
      if (kept.length === Object.keys(tab.shows).length) return [id, tab];
      dropped = true;
      return [id, showing(tab, Object.fromEntries(kept))];
    }),
  );
  return dropped ? { ...tabs, byId } : tabs;
}

/**
 * **Puts back what the tabs showed** (#1486): each chat of `shown` in its home, as the saved
 * layout recorded it. Nothing comes to the front and no pane takes the keyboard: a launch
 * decides what is in front once, from the whole record. A task whose session did not come
 * back is left out. Answers `tabs` itself when nothing changed.
 */
export function restoreShown(tabs: Tabs, shown: readonly number[], askedBy: AskedBy): Tabs {
  return shown.reduce((now, chat) => {
    const home = homeOf(now, chat, askedBy);
    if (home === undefined || home.own === chat) return now;
    const tab = now.byId[home.tab];
    if (tab.shows?.[home.pane] === chat) return now;
    const next = showing(tab, { ...tab.shows, [home.pane]: chat });
    return { ...now, byId: { ...now.byId, [tab.id]: next } };
  }, tabs);
}

/**
 * **Where a task with a pane of its own is** (#1489): in a `tab` of its own, which the person
 * asked for, or `beside` the chat it returns to ({@link returnsTo}), split from that chat's pane.
 */
export type Placed = "tab" | "beside";

/**
 * **Whether a task has a pane of its own, and where** (#1489): nothing for a task shown in the
 * pane it returns to or in no pane at all, which is every task until the person asks, and
 * nothing for a chat nobody asked for. `tab` where it is its tab's own chat ({@link chatOf}),
 * `beside` where its pane is in another chat's tab.
 *
 * Read off the layout, as everything about a tab is: a task with a pane is its own home
 * ({@link homeOf}), and nothing else records it.
 */
export function placedOf(tabs: Tabs, chat: number, askedBy: AskedBy): Placed | undefined {
  if (askedBy(chat) === undefined) return undefined;
  const at = ownedIn(tabs).get(chat);
  if (at === undefined) return undefined;
  return chatOf(tabs, at.tab) === chat ? "tab" : "beside";
}

/**
 * **Where a task returns to: the home of the chat that asked for it** (#1489, #5667). A task
 * that has no pane of its own, or loses the one it had, is a row in the Chats list under the
 * chat that asked, and this is the pane that answers for it: its Notices and its hand are drawn
 * there, its tab's chip counts it, and a pane beside it opens there. Nothing for a chat nobody
 * asked for, and for a task none of whose askers has a pane in this window.
 */
export function returnsTo(
  tabs: Tabs,
  task: number,
  askedBy: AskedBy,
): { tab: number; pane: number; own: number } | undefined {
  const asker = askedBy(task);
  return asker === undefined || asker === task ? undefined : homeOf(tabs, asker, askedBy);
}

/**
 * **Every task below chat `session` that has a pane of its own**, at any depth, by number
 * (#1489): what a close of that chat's tab has to say, and to send back.
 */
export function tasksPlacedBelow(
  tabs: Tabs,
  session: number,
  askedBy: AskedBy,
): { session: number; placed: Placed }[] {
  const below = (chat: number): boolean => {
    const seen = new Set<number>([chat]);
    for (let at = askedBy(chat); at !== undefined && !seen.has(at); at = askedBy(at)) {
      if (at === session) return true;
      seen.add(at);
    }
    return false;
  };
  return [...ownedIn(tabs).keys()]
    .filter(below)
    .sort((one, other) => one - other)
    .flatMap((chat) => {
      const placed = placedOf(tabs, chat, askedBy);
      return placed === undefined ? [] : [{ session: chat, placed }];
    });
}

/**
 * **`tabs` as a task leaves the pane it was shown in** (#1489): no pane goes on showing the
 * task, **or any chat below it**. Those are at home wherever the task goes (`homeOf`), so a
 * pane left showing one could not draw it, and the core would go on being told it is on
 * screen there: its end held and its notifications withheld for a chat nobody can see.
 */
function leftBy(tabs: Tabs, task: number, askedBy: AskedBy): Tabs {
  const goes = (shown: number): boolean => {
    const seen = new Set<number>();
    for (let at: number | undefined = shown; at !== undefined && !seen.has(at); at = askedBy(at)) {
      if (at === task) return true;
      seen.add(at);
    }
    return false;
  };
  return keepShown(tabs, (shown) => !goes(shown));
}

/**
 * `tabs` without chat `session`'s pane, wherever it is, and without its tab where that was its
 * only pane. **Nothing is put in front**: the caller says what is. Answers `tabs` itself for a
 * chat with no pane.
 */
function withoutPaneOf(tabs: Tabs, session: number): Tabs {
  const at = ownedIn(tabs).get(session);
  if (at === undefined) return tabs;
  const tab = tabs.byId[at.tab];
  const left = without(tab.layout, at.pane);
  if (!left) {
    const order = tabs.order.filter((id) => id !== tab.id);
    const byId = Object.fromEntries(order.map((id) => [id, tabs.byId[id]]));
    return { ...tabs, byId, order, inFront: tabs.inFront === tab.id ? undefined : tabs.inFront };
  }
  const focused = tab.focused === at.pane ? panes(left)[0].pane : tab.focused;
  const kept = showing({ ...tab, layout: left, focused }, tab.shows);
  return { ...tabs, byId: { ...tabs.byId, [tab.id]: kept } };
}

/**
 * **The panes a chat moved into** (#3271): each pane of `after` that `before` did not have,
 * with the pane its chat was at home in before ({@link homeOf}), where its Notices were drawn: a
 * task given a tab of its own ({@link moveToOwnTab}) or a pane beside its session
 * ({@link placeBeside}). A pane whose chat had no home before is new, and is not listed.
 */
export function panesMovedInto(
  before: Tabs,
  after: Tabs,
  askedBy: AskedBy,
): { from: number; to: number }[] {
  if (before === after) return [];
  const had = new Set(before.order.flatMap((id) => contentsOf(before, id).map((one) => one.pane)));
  return after.order.flatMap((id) =>
    panesOf(after, id).flatMap(({ pane, session }) => {
      if (had.has(pane)) return [];
      const home = homeOf(before, session, askedBy);
      return home === undefined || home.pane === pane ? [] : [{ from: home.pane, to: pane }];
    }),
  );
}

/**
 * **Moves a task to a tab of its own** (#1489, V100-38): at the strip's end, in front. It is
 * taken out of any pane that was showing it, so one chat is in one place, and out of a split
 * it was opened beside its session in. Its own tasks go with it: they are at home in its tab
 * ({@link homeOf}). A task that already has its tab has it brought forward.
 *
 * **Nothing ends, and the pane it returns to is left on what it showed.** `chat`, `who` and
 * `label` name the tab as {@link openTab} names any.
 */
export function moveToOwnTab(
  tabs: Tabs,
  task: number,
  chat = "",
  who: string | null = null,
  label: string | null = null,
  /** Who asked whom: what says which shown chats are below the task, and go with it. */
  askedBy: AskedBy = () => undefined,
): Tabs {
  const at = ownedIn(tabs).get(task);
  if (at !== undefined && chatOf(tabs, at.tab) === task) return selectTab(tabs, at.tab);
  const front = tabs.inFront;
  const cleared = withoutPaneOf(leftBy(tabs, task, askedBy), task);
  return openTab({ ...cleared, inFront: cleared.inFront ?? front }, task, chat, who, label);
}

/**
 * **Sends a task back out of its pane** (#1489): the minimise. Its tab leaves the strip, or
 * its pane leaves the split and what shared the split takes the room. **Nothing ends**: the
 * task is a row in the Chats list under the chat that asked, at home where it returns to
 * ({@link returnsTo}), whose pane shows what it showed and whose tab's chip counts it. The
 * front goes where closing the task's tab would send it ({@link closeTab}). Answers `tabs`
 * itself for a task with no pane.
 */
export function sendBack(
  tabs: Tabs,
  task: number,
  filedIn: FiledIn,
  pinned: Pinned = nothingPinned,
): Tabs {
  return closeChat(tabs, task, filedIn, pinned);
}

/**
 * **Gives a task a pane beside the chat it returns to** (#1489, V100-38): that chat's pane
 * ({@link returnsTo}) is split side by side, that chat stays where it was and the task is on
 * the far side. The pane stops showing the task in place of its own
 * chat, and a tab the task had of its own goes. **Nothing comes to the front and no pane takes
 * the keyboard**: this is what a launch puts back ({@link openBeside} is the person's press).
 *
 * Answers `tabs` itself for a task whose session has no tab in this window, and for one that
 * is beside its session already.
 */
export function placeBeside(tabs: Tabs, task: number, askedBy: AskedBy, chat?: string): Tabs {
  const before = returnsTo(tabs, task, askedBy);
  if (before === undefined) return tabs;
  const had = ownedIn(tabs).get(task);
  if (had !== undefined && had.tab === before.tab) return tabs;
  const cleared = withoutPaneOf(leftBy(tabs, task, askedBy), task);
  const home = returnsTo(cleared, task, askedBy);
  if (home === undefined) return tabs;
  const tab = cleared.byId[home.tab];
  const pane = cleared.named.panes + 1;
  const content: Content = {
    kind: "session",
    session: task,
    chat: chat ?? chatNameOf(cleared, tab.id) ?? String(task),
  };
  const layout = replace(tab.layout, home.pane, (found) => ({
    kind: "split",
    direction: "row",
    children: [found, { kind: "pane", pane, content }],
  }));
  return {
    ...cleared,
    byId: { ...cleared.byId, [tab.id]: { ...tab, layout } },
    inFront: cleared.inFront ?? (tabs.inFront === undefined ? undefined : home.tab),
    named: { ...cleared.named, panes: pane },
  };
}

/**
 * **Opens a task beside its session** (#1489): {@link placeBeside}, and then that chat's tab
 * is in front and the task's pane has the keyboard. A task already beside its session is
 * focused there.
 */
export function openBeside(tabs: Tabs, task: number, askedBy: AskedBy, chat?: string): Tabs {
  const placed = placeBeside(tabs, task, askedBy, chat);
  const at = ownedIn(placed).get(task);
  const home = returnsTo(placed, task, askedBy);
  if (at === undefined || home === undefined || at.tab !== home.tab) return tabs;
  const tab = placed.byId[at.tab];
  if (tab.focused === at.pane && placed.inFront === tab.id) return placed;
  return {
    ...placed,
    byId: { ...placed.byId, [tab.id]: { ...tab, focused: at.pane } },
    inFront: tab.id,
  };
}

/**
 * **Opens `view` beside chat `session`'s pane, in that chat's tab** (#2024): the pane is split
 * side by side, the view on the right. Nothing else moves: the tab in front and the pane with
 * the keyboard stay where they were, so a view opened for the person while they work elsewhere
 * takes nothing from them. A view some tab shows already, or a chat no pane shows, is left as
 * it is (answers `tabs` itself).
 */
export function openViewBeside(
  tabs: Tabs,
  session: number,
  view: ViewRef,
  workspace: string,
): Tabs {
  if (findView(tabs, view)) return tabs;
  for (const id of tabs.order) {
    const tab = tabs.byId[id];
    const home = contents(tab.layout).find(
      ({ content }) => content.kind === "session" && content.session === session,
    );
    if (home === undefined) continue;
    const pane = tabs.named.panes + 1;
    const layout = replace(tab.layout, home.pane, (found) => ({
      kind: "split",
      direction: "row",
      children: [found, { kind: "pane", pane, content: { kind: "view", view, workspace } }],
    }));
    return {
      ...tabs,
      byId: { ...tabs.byId, [id]: { ...tab, layout } },
      named: { ...tabs.named, panes: pane },
    };
  }
  return tabs;
}

/**
 * **Opens `view` beside the pane showing `beside`, in that pane's tab** (#2028): the pane is
 * split side by side, the view on the right, and the tab comes forward with the keyboard in the
 * new pane: the person asked for it there. A view some tab shows already is brought forward
 * instead, and where no tab shows `beside` the view opens in a tab of its own named `name`.
 */
export function openViewBesideView(
  tabs: Tabs,
  beside: ViewRef,
  view: ViewRef,
  name: string,
  workspace: string,
): Tabs {
  if (findView(tabs, view)) return openView(tabs, view, name, workspace);
  const at = findView(tabs, beside);
  if (at === undefined) return openView(tabs, view, name, workspace);
  return openViewBesidePane(tabs, at.pane, view, name, workspace, "row");
}

/**
 * **Opens `view` beside pane `pane`, in that pane's tab** (#4769, Open beside): the pane is split
 * in `direction` (the pane's shape, `viewBeside.ts`), the view second, and the tab comes forward
 * with the keyboard in the new pane: the person pressed for it there. **A view some pane shows
 * already is brought forward instead, never opened twice** ({@link openView}), and where no tab
 * has `pane` any more the view opens in a tab of its own named `name`. Whether the split leaves
 * each side room is the caller's to judge before the press (`besideOf`).
 */
export function openViewBesidePane(
  tabs: Tabs,
  pane: number,
  view: ViewRef,
  name: string,
  workspace: string,
  direction: Direction,
): Tabs {
  if (findView(tabs, view)) return openView(tabs, view, name, workspace);
  const id = tabs.order.find((one) => panes(tabs.byId[one].layout).some((it) => it.pane === pane));
  if (id === undefined) return openView(tabs, view, name, workspace);
  const tab = tabs.byId[id];
  const made = tabs.named.panes + 1;
  const layout = replace(tab.layout, pane, (found) => ({
    kind: "split",
    direction,
    children: [
      found,
      { kind: "pane", pane: made, content: { kind: "view", view, workspace, title: name } },
    ],
  }));
  return {
    ...tabs,
    byId: { ...tabs.byId, [tab.id]: { ...tab, layout, focused: made } },
    named: { ...tabs.named, panes: made },
    inFront: tab.id,
  };
}

/**
 * `tab` showing `shows`, kept true to the tab: only a chat pane the tab still has, and only a
 * chat other than the pane's own. A tab showing no task carries no `shows` at all, so it is
 * the value it was before any task was shown.
 */
function showing(tab: Tab, shows: Readonly<Record<number, number>> | undefined): Tab {
  const kept = Object.fromEntries(
    contents(tab.layout).flatMap(({ pane, content }) => {
      const shown = shows?.[pane];
      return content.kind === "session" && shown !== undefined && shown !== content.session
        ? [[pane, shown]]
        : [];
    }),
  ) as Record<number, number>;
  if (Object.keys(kept).length > 0) return { ...tab, shows: kept };
  const plain = { ...tab };
  delete plain.shows;
  return plain;
}

function frontTab(tabs: Tabs): Tab | undefined {
  return tabs.inFront === undefined ? undefined : tabs.byId[tabs.inFront];
}

/** One pane of a layout. */
export type Pane = Extract<Layout, { kind: "pane" }>;

/** Every pane of `layout`, in reading order. */
export function panes(layout: Layout): Pane[] {
  return layout.kind === "pane" ? [layout] : layout.children.flatMap(panes);
}

function contents(layout: Layout): { pane: number; content: Content }[] {
  return panes(layout).map(({ pane, content }) => ({ pane, content }));
}

/** `layout` with the pane `pane` put through `change`. */
export function replace(layout: Layout, pane: number, change: (found: Pane) => Layout): Layout {
  if (layout.kind === "pane") return layout.pane === pane ? change(layout) : layout;
  return {
    ...layout,
    children: layout.children.map((child) => replace(child, pane, change)) as [Layout, Layout],
  };
}

/**
 * The pane that takes pane `pane`'s space when it closes: in the split it is a side of, the
 * other side's pane nearest to it (its last pane where `pane` came second, its first where it
 * came first). Nothing for a layout of one pane, or a pane it does not hold.
 */
function takesTheSpace(layout: Layout, pane: number): number | undefined {
  if (layout.kind === "pane") return undefined;
  const [first, second] = layout.children;
  if (first.kind === "pane" && first.pane === pane) return panes(second)[0]?.pane;
  if (second.kind === "pane" && second.pane === pane) return panes(first).at(-1)?.pane;
  return takesTheSpace(first, pane) ?? takesTheSpace(second, pane);
}

/** `layout` without the pane `pane`, or nothing when it was the only one. */
export function without(layout: Layout, pane: number): Layout | undefined {
  if (layout.kind === "pane") return layout.pane === pane ? undefined : layout;
  const [first, second] = layout.children.map((child) => without(child, pane));
  if (!first) return second;
  if (!second) return first;
  return { ...layout, children: [first, second] };
}
