// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type PlaneSaving } from "./bindings";
import { type Offer, type Ran, type Said } from "./actions";
import { type Place } from "./pieceViews";
import { type Asking } from "./NeedsYou";
import { type QuietChat } from "./chatState";
import { type Ending } from "./QuitWarning";
import { type SandboxSaid } from "./SandboxMark";

/**
 * **What a project and the window tell each other** (#1035, split out of `PlaneView.tsx`): what
 * a project reports about itself (`PlaneReport`), what it asks the window to do
 * (`WindowDoing`), and a first chat asked for in a repository opened into it (`FirstChat`).
 */

/** A first chat asked for in a repository opened into a project (FR-4). */
export type FirstChat = {
  /** The workspace named after the repository. */
  workspace: string;
  /** The repo's clone in it, where the chat starts. */
  cwd: string;
  /** The one harness signed in on this machine, which the chat starts on without the picker;
   *  `null` when there is a choice to make. */
  harness: string | null;
  /** Whether no harness is installed on this machine: the first chat is then the harness
   *  setup tab instead of the picker (FR-29). */
  noneInstalled: boolean;
  /** How many of the repo's agent instruction files can go into the workspace's memory
   *  (FR-18a): offered in a tab beside the chat when there are any. */
  instructions: number;
  /** Which ask this is, so each is answered once. */
  at: number;
};

/** What this project told the window about itself. */
export type PlaneReport = {
  /** Every chat it has open, with what each one is doing — what a quit would end. */
  ending: Ending[];
  /** Everything this project can do, for the window's one palette to list. */
  offers: Offer[];
  /** How the window carries a row out: this project's own dispatcher, so a row the palette
   *  runs reaches this project's live arrangement and no other's. */
  run: (offer: Offer) => Promise<Ran>;
  /** What its last action answered, drawn by the window beside the palette. */
  said?: Said;
  /** Its chats asking for the operator: for its own tab to count, and for the title bar's
   *  list (charter-app#249). */
  asking: Asking[];
  /** Its chats that can be waiting without saying so (charter-app#52): the title bar's faint
   *  hand, and the Notices each Inbox lists for them (#1695). */
  quiet: readonly QuietChat[];
  /** Whether the core has answered what it already had open. Until it has, "no tabs" is
   *  "not yet", and a quit that read it as "nothing is running" would end the lot. */
  settled: boolean;
  /** Whether its plane has been read at all: until it has, which workspace it is on is not
   *  known, and the window waits before drawing that workspace's theme. */
  read: boolean;
  /** The workspace it is on by its name, `undefined` outside every workspace: whose
   *  `workspace.json` is a layer of the theme the window draws (charter-app#281). */
  workspace?: string;
  /** That workspace's colour, a palette name or `#rrggbb`, or `null`: what the window's accent
   *  and focus ring are tinted with while it is in front (charter-app#281). */
  colour?: string | null;
  /** How much the sandbox of the chat in front holds (#2071), as its tab says it: what the
   *  title bar shows while this project is in front. `null` where no chat is in front. */
  sandbox?: SandboxSaid | null;
  /** Where this project's unsaved work sits (charter-app#302), once read. */
  saving?: PlaneSaving;
  /** Why this purlis opens it read-only, or undefined (#826): the title bar's save is off with
   *  it while the project is in front. */
  readOnly?: string;
  /** The branch the explorer has picked in the workspace in front, or none: the narrowest
   *  scope ⌘P finds files in (FM-7). */
  branch?: Place;
  /**
   * When anything in it last moved: the newest `movedAt` among its chats, `0` when nothing
   * has been heard. What the project strip's show-more menu orders its rows by after the ones
   * that need you (ADR 0054, charter#401). The core counts moves across every project's
   * board with one count, so this compares across projects.
   */
  moved: number;
  /** Its workspaces, once its plane is read: the title bar's chips and switchers (#2916). */
  workspaces?: ReportedWorkspaces;
};

/**
 * **A project's workspaces, as the title bar draws them** (#2916): every one but the plane root,
 * in the order the palette lists them (the pins first, in the order they were pinned), its pins,
 * the one in front (the root's own name, `actions.OUTSIDE`, when it is the root), and how many
 * things wait on the person in each, the root's under its name too.
 */
export type ReportedWorkspaces = {
  names: readonly string[];
  pins: readonly string[];
  focused?: string;
  waiting: Readonly<Record<string, number>>;
};

/** What a project asks the WINDOW to do, because the window is what holds projects. */
export type WindowDoing = {
  openProject: () => void;
  /** Shows the dialog that makes a new project. The window's, like the opener: what it ends in
   *  is another project tab, and the open it ends in is the gated one (ADR 0035). */
  createProject: () => void;
  /** Shows what has contributed what to this window. The window's and not a project's: an
   *  extension is machine state (ADR 0041), so it is the same list behind every tab. */
  showExtensions: () => void;
  /** Puts the app's `charter` on a terminal's PATH. The window's: it is about the machine. */
  installCli: () => Promise<Ran>;
  selectProject: (plane: string) => void;
  /** Opens the project switcher (FR-27). The window's: the palette and the projects are. */
  switchProject: () => void;
  closeProject: (plane: string) => Promise<Ran>;
  /** Moves a project into another window, or a new one (charter#126). The window's, because
   *  the window is what holds projects. */
  moveProject: (plane: string, to: string | null) => Promise<Ran>;
  /** Brings a project to the front and opens Settings at its Project level (SE-19). The
   *  window's, because the project may not be the one in front, and only the window can bring
   *  it there. */
  openSettings: (plane: string) => void;
  /** Brings a project to the front and opens its Saving tab (charter-app#294). */
  openSaving: (plane: string) => void;
  /** Opens the Settings tab (SE-16) on the project in front, at its focused level (SE-23), or
   *  draws it at You where the opener is when there is none. The window's, because which
   *  project is in front is. */
  openSettingsTab: () => void;
  /** Opens the Settings tab at the You level (SE-23): on the project in front, or where the
   *  opener is when there is none. */
  openYourSettings: () => void;
  /** Pinning a PROJECT is the window's, because the project strip is: a project that is not
   *  in front draws nothing, and its pin still has to be on that strip (ADR 0039). */
  pinProject: (plane: string, pinned: boolean) => Promise<Ran>;
  quit: () => void;
};
