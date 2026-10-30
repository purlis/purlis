// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { saidOfLate, useLateAnswers } from "./answerWatch";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Menued } from "./Menus";
import { listen } from "./here";
import { afterDrop } from "./reorder";
import "./styles.css";
import {
  commands,
  type Ask,
  type AwayRefusal,
  type ForgeRow,
  type ForgeWord,
  type FoundFile,
  type GoneProject,
  type OpenedRepo,
  type PlaneId,
  type RelaunchChoice,
  type RelaunchQuestion,
  type SlowStart,
  type TemplateChoice,
} from "./bindings";
import type { ForgeAsk } from "./ForgeQuestion";

/** What trying to open a repo came to: refused, a question about its forge, or opened. */
type RepoTried = { refused?: string; asksForge?: string };
import { SessionBusNotice } from "./SessionBusNotice";
import { EngineDownloadNotice } from "./EngineDownloadNotice";
import { EngineSandboxNotice } from "./EngineSandboxNotice";
import { HostFailingNotice } from "./HostFailingNotice";
import { VaultsWaitingNotice } from "./VaultsWaitingNotice";
import { Notice, NoticeList } from "./Notice";
import { ForgeSignInEnded } from "./ForgeSignInEnded";
import { AddDockerPoolHost } from "./AddDockerPool";
import { ActionResultNotice, ResultsGroup } from "./ActionResultNotice";
import {
  AT_STATUS,
  dormantCause,
  dormantSays,
  placeOfSaid,
  useChipNotices,
  type ChipNotices,
  type StandingNotice,
} from "./ChipNotices";
import { chipsOf } from "./noticePlace";
import { dismissGone, GoneProjectNotice, settleGone, useGoneDismissed } from "./GoneProjectNotice";
import {
  goneCause,
  goneKept,
  keepGoneDismissed,
  letGoneGo,
  settleGoneKept,
  useDismissals,
} from "./dismissals";
import { askUpdate, tellSaved, useRepoSaving } from "./saving";
import { OPEN_LICENCES } from "./LicencesView";
import {
  catalogue,
  catalogued,
  OUTSIDE,
  perform,
  projectRows,
  saidOf,
  type Doing,
  type Offer,
  type Project,
  type Ran,
  type Said,
} from "./actions";
import { useScreenReaderMode } from "./screenReader";
import { useLatestSaid } from "./latestSaid";
import { resultsFor, useKeptResults } from "./keptResults";
import { useAlerts } from "./alerts";
import type { InboxAlertsDo, WindowAlerts } from "./InboxAlerts";
import { waitingAsk, type Elsewhere } from "./InboxElsewhere";
import { ApprovePlane } from "./ApprovePlane";
import { drawThemeFor, Extensions } from "./Extensions";
import { Onboarding } from "./Onboarding";
import { useGitPresence } from "./gitPresence";
import { useOnboarding } from "./useOnboarding";
import { SettingsTab } from "./settings/SettingsTab";
import {
  askSettingsLink,
  levelOf,
  linkToGroup,
  settingsPlace,
  type SettingsLink,
} from "./settings/links";
import { enterSettings } from "./settings/entering";
import { machineChanged, whenRecentSettled } from "./settings/thisMachine";
import { useExtensionsOn } from "./extensionsOn";
import { useProjectTheme } from "./projectTheme";
import { drawTint, facesAfterTheFirstFrame } from "./theme/theme";
import { useContributedPanels } from "./Panels";
import { useExtensionCommands, useExtensionViews } from "./Views";
import { Palette } from "./Palette";
import { placeOf, scopeLadder } from "./fileFind";
import { useJumpAsks, type Pending } from "./fileJump";
import type { Place } from "./pieceViews";
import { ClosingProject } from "./ClosingProject";
import { discardUnsavedIn, unsavedIn, unsavedNow } from "./unsavedWork";
import { UnsavedWorkQuestion, type UnsavedAsk } from "./UnsavedWorkQuestion";
import { QuitWarning, type Ending } from "./QuitWarning";
import { RelaunchAsk } from "./RelaunchAsk";
import { useNoBrowserMenu } from "./Menus";
import { NewProject } from "./NewProject";
import { PlaneView } from "./PlaneView";
import { keptOf } from "./projectShown";
import { switched } from "./switchClock";
import { withSessionRows } from "./sessionOffers";
import type { FirstChat, PlaneReport, WindowDoing } from "./planeReport";
import { TitleBar, useTitleBarRoom } from "./TitleBar";
import { TitleBarSwitch, type SwitchDoes, type SwitchProject } from "./TitleBarSwitch";
import { slideTowards } from "./workspaceSlide";
import { pinsChanged } from "./workspacePins";
import type { Needing, Quiet } from "./NeedsYou";
import { useAsks } from "./asks";
import { giveKeyboardTo } from "./paneKeyboard";
import { landsOn, useNotificationLanding, type Landing } from "./askNotices";
import { aQuestionIsUp, useCloseHanded } from "./closeHanded";
import { useAwayRefusals } from "./dispatchAway";
import { useUpdates } from "./Updates";
import { noTabs, SETTINGS_TAB_TITLE } from "./tabs";
import { MAIN, runElsewhere, thisWindow, useOtherWindows, useRunHere } from "./windows";
import { Button } from "./ui/button/Button";
import { IconButton } from "./ui/button/Button";
import { SidebarIcon } from "./ui/rail/Rail";
import { ProjectRail } from "./ProjectRail";
import { FOCUS_BACK, FOCUS_MODE, useFocusMode } from "./keys/focusMode";
import { unsavedOf } from "./SavingView";
import { useRailMode } from "./railMode";

/**
 * Puts the app's own `charter` on a terminal's `PATH`, and answers what the core said — the
 * link it made, or why it made none. Nothing else in the window is involved, so it is not a
 * hook: the core decides and the operating system asks for the password.
 */
async function installCli(): Promise<Ran> {
  const answer = await commands
    .installCliOnPath()
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  return answer.status === "ok"
    ? { ok: true, said: answer.data }
    : { ok: false, refused: answer.error };
}

/** What the window says while an open waits for its own close of the same project (#1949). */
const STILL_CLOSING =
  "Waiting for purlis to finish letting go of this project before it opens again…";

/**
 * The window, which holds projects.
 *
 * **A project is a plane and a window may hold several** (ADR 0033, spec decision 23).
 * The operator asked for Zed's shape by name and gave the reason Zed has it: eight projects is
 * eight things to arrange, and the thing an operating system gives you to arrange is a window.
 * So the top-level tabs here are projects; everything inside one is `PlaneView`'s, which is
 * the extraction that made a second project possible at all.
 *
 * **Switching projects is navigation, never a teardown.** The project left behind stays
 * mounted, keeps its tabs, its splits and its focused workspace, and goes on being told what
 * its chats are doing — fifty chats in project A are not torn down because the operator
 * glanced at project B, which is the whole reason he wanted one window per project.
 *
 * What lives up here is what belongs to the WINDOW: which projects it holds, which one is in
 * front, the opener and the trust ask in front of every open, the cold-launch restore, a
 * second launch handing its directory over, and the quit that ends every project's chats at
 * once.
 */
function App() {
  // The WebView's own menu, taken away from the whole window (`Menus.tsx`). One listener, on
  // the window, so it covers every surface including the ones with no charter menu of their
  // own — a shipped app that answers a right-click with `Reload` and `Inspect Element` is
  // showing the operator the browser it is built on.
  useNoBrowserMenu();
  /** Which window this is (charter#126): the main window, or a split window a project tab was
   *  moved into. Tauri's label, read once — a window never becomes another. */
  const own = useMemo(() => thisWindow(), []);
  const split = own !== MAIN;
  /** What the launch resolved, asked once. `undefined` while the core has not answered. */
  const [launch, setLaunch] = useState<{ plane: PlaneId | null; here: boolean; reason: string }>();
  /** The projects this window holds, left to right as the strip shows them. */
  const [planes, setPlanes] = useState<PlaneId[]>([]);
  /** The same, as of the last render, for a verb that is kept stable across renders. */
  const planesNow = useRef(planes);
  useLayoutEffect(() => {
    planesNow.current = planes;
  });
  /** What is on screen: one of the projects, or the opener. The opener is not only the state
   *  of an empty window — it is also how a window holding eight gets a ninth. */
  const [showing, setShowing] = useState<{ at: "opener" } | { at: "plane"; plane: PlaneId }>({
    at: "opener",
  });
  /** **Git first** (#1813): the window's gate, which holds an act that needs git until git is
   *  there. The guided steps put each project act behind it. */
  const gitGate = useGitPresence();
  /** **The guided steps** (#1817): where the person is in setting up, kept on this machine. A
   *  project they asked for is held behind the harness step, and arrives through the window's
   *  own ways of opening one. */
  const steps = useOnboarding({ gateOpen: gitGate.open, adding: planes.length > 0 });
  /** The same, for the stable verbs below that a project arrives through. */
  const stepsNow = useRef(steps);
  useLayoutEffect(() => {
    stepsNow.current = steps;
  });
  /** A repo the steps opened that waits on the trust question: where its first chat starts,
   *  for the harness step once the answer opens it. */
  const madeAsking = useRef<{ asking: string; cwd: string; workspace: string }>(undefined);
  /** What each project has open and whether it has found out yet. Reported by the project. */
  const [reports, setReports] = useState<Record<string, PlaneReport>>({});
  /** Whether the operator is being asked about quitting. */
  const [asking, setAsking] = useState(false);
  /** The trust asks waiting to be answered, oldest first. A restore can raise several at
   *  once — one project's committed settings changed while charter was not running — and
   *  they are answered one at a time rather than drawn on top of one another. */
  const [approving, setApproving] = useState<Ask[]>([]);
  /** Why the last attempt to open a project opened nothing. Shown on the opener, which is
   *  where the operator is standing when it happens. */
  const [openTrouble, setOpenTrouble] = useState<string>();
  /** The projects this window asked the core to let go of, until it has (#1949). */
  const closingNow = useRef(new Map<string, Promise<unknown>>());
  /** How many of them are still letting go: on the project strip, for whoever waits on it. */
  const [lettingGo, setLettingGo] = useState(0);
  /** What a window-level action answered, when it had something to say. */
  const [report, setReport] = useState<Said>();
  // A command the core has not answered for a while is said on the window's line (#1949): a
  // dialog waiting on it would otherwise wait in silence (`answerWatch.ts`).
  // Said once per command while it is late, however often it is asked meanwhile, and taken
  // away once its answer comes (#2029's review).
  const lateAnswers = useLateAnswers();
  const saidLate = useRef(new Set<string>());
  useEffect(() => {
    const waiting = new Set(
      lateAnswers.filter((one) => one.ms === undefined).map((one) => one.command),
    );
    for (const command of [...saidLate.current])
      if (!waiting.has(command)) {
        saidLate.current.delete(command);
        setReport((now) => (now?.from === `answer.late.${command}` ? undefined : now));
      }
    for (const one of lateAnswers) {
      if (one.ms !== undefined || saidLate.current.has(one.command)) continue;
      saidLate.current.add(one.command);
      setReport({ from: `answer.late.${one.command}`, refused: true, words: saidOfLate(one) });
    }
  }, [lateAnswers]);
  /** Whether the palette is up, so what an action answered is said in one place rather than
   *  two: the palette is modal and draws over the line below it. */
  const [paletteOpen, setPaletteOpen] = useState(false);
  /** Whether the new-project dialog is up, and why the last attempt made nothing. The
   *  window's, like the opener: what it ends in is a project this window holds. */
  const [creating, setCreating] = useState(false);
  const [createTrouble, setCreateTrouble] = useState<string>();
  /** Whether charter is scaffolding one right now, so the answer cannot be given twice. */
  const [makingProject, setMakingProject] = useState(false);
  /** Whether the extension list is up. The window's, not a project's: an extension is machine
   *  state, so it is the same list whichever project is in front. */
  const [extensions, setExtensions] = useState(false);
  /** Focus mode (#2921, N32): the work alone, held for this launch and written nowhere. */
  const focusMode = useFocusMode();
  const toggleFocusMode = focusMode.toggle;
  /**
   * The last ask for a project's Settings tab at the Project level (SE-19, #1213; the retired
   * Project settings page before it, charter-app#252): which project, and a count so
   * that asking twice opens it twice — the second ask brings forward a tab the operator may
   * have left behind. The project's own `PlaneView` opens it, because its tabs are its own.
   */
  const [settingsAsk, setSettingsAsk] = useState<{ plane: PlaneId; at: number }>();
  /**
   * How many times the project switcher has been asked for from outside the palette (FR-27) —
   * the title bar's button, or the row run from a menu. A count, `settingsAsk`'s shape, so that
   * asking twice opens it twice; the palette opens on each new value.
   */
  const [switcherAsk, setSwitcherAsk] = useState(0);
  /**
   * The last file ⌘P was asked to open (FM-7): its project, its branch, its path and a count,
   * the shape `settingsAsk` has. The project's own `PlaneView` opens its file tab, because its
   * tabs are its own; the window brings the project to the front first.
   */
  const [fileAsk, setFileAsk] = useState<{
    plane: PlaneId;
    place: Place;
    path: string;
    /** A jump to a line (a search hit, FM-8): the branch's file tab lands on it. */
    line?: number;
    at: number;
  }>();
  /**
   * A jump to a file at a line (FM-8, `fileJump.ts`): a search hit, possibly another project's.
   * That project comes to the front and opens the branch's file tab, which lands on the line.
   */
  const jumped = useCallback((jump: Pending) => {
    setShowing({ at: "plane", plane: jump.plane });
    setFileAsk((was) => ({
      plane: jump.plane,
      place: jump.place,
      path: jump.path,
      line: jump.line,
      at: (was?.at ?? 0) + 1,
    }));
  }, []);
  useJumpAsks(jumped);
  /** The last ask for a project's Saving tab (charter-app#294), the same shape as `settingsAsk`. */
  const [savingAsk, setSavingAsk] = useState<{ plane: PlaneId; at: number }>();
  /** The last ask for the Licences tab from About (#1719): the project in front when it was
   *  asked, which opens it, as Saving's ask is. */
  const [licencesAsk, setLicencesAsk] = useState<{ plane: PlaneId; at: number }>();
  /**
   * The last ask for the Settings tab (SE-16), the same shape as `settingsAsk`: the project in
   * front when it was asked, whose strip the tab opens on. It opens at that project's focused
   * level (SE-23) — its focused workspace's, else the project's — which only that project knows,
   * so the project answers the ask (`PlaneView`). With no project open it is You, at the opener.
   */
  const [settingsTabAsk, setSettingsTabAsk] = useState<{ plane: PlaneId; at: number }>();
  /** The same, for the Settings tab at the You level (SE-23's Your settings…): on the project in
   *  front's strip, since You is the machine's and any strip will do. */
  const [yourSettingsAsk, setYourSettingsAsk] = useState<{ plane: PlaneId; at: number }>();
  /** The Inbox the title bar's ✋ asked for (#1692, I-2): the project it opens on, and a count;
   *  and the chat whose group it opens at, for a clicked notification (#1694). */
  const [inboxAsk, setInboxAsk] = useState<{ plane: PlaneId; at: number; session?: number }>();
  /**
   * The last link into a Settings group the window followed (SE-22): the project in front, the
   * link, and a count. Its own ask and not `settingsTabAsk`, because a link names its level and
   * Settings… opens at whatever level ⌘, decides.
   */
  const [settingsLinkAsk, setSettingsLinkAsk] = useState<{
    plane: PlaneId;
    link: SettingsLink;
    at: number;
  }>();
  /**
   * The first chat a repo opened into the local project asks for (FR-4): the workspace named
   * after the repo, the clone the chat starts in, and a count, the shape `settingsAsk` has.
   * `plane` is the project once it is open; until the trust question about it is answered it
   * is `null` and `asking` names the path that question is about. The project's own
   * `PlaneView` starts the chat, because the picker and the tabs are its own.
   */
  const [firstChat, setFirstChat] = useState<
    FirstChat & { plane: PlaneId | null; asking: string | null }
  >();
  /**
   * A shell tab asked for with a command already typed in it — `gh auth login` or `glab auth
   * login`, from the first run's "Sign in to GitHub" or "Sign in to GitLab" (FR-4, W10) — in
   * the same two stages as `firstChat`.
   */
  const [shellAsk, setShellAsk] = useState<{
    plane: PlaneId | null;
    asking: string | null;
    typed: string;
    at: number;
  }>();
  /** Whether a repo is being opened right now, and why the last one opened nothing. */
  const [openingRepo, setOpeningRepo] = useState(false);
  const [repoTrouble, setRepoTrouble] = useState<string>();
  /** The first run's question about the forge, when the repo's remote did not say (#839). */
  const [repoForgeAsk, setRepoForgeAsk] = useState<ForgeAsk>();
  /** The New project dialog's, for either of its forms. */
  const [createForgeAsk, setCreateForgeAsk] = useState<ForgeAsk>();
  /** Settings asked for with no project in front: drawn where the opener is, because there is
   *  no strip to open a tab on and a text size is still worth changing. */
  const [settingsAlone, setSettingsAlone] = useState(false);
  /** Where the window's own lines are listed while no project is in front: the top of the
   *  opener's page (D-LB-1). Unset, they stand under the title bar. */
  const [windowLinesAt, setWindowLinesAt] = useState<HTMLDivElement | null>(null);
  /** Where they are listed while a project is in front: the top of its Inbox's Notices
   *  (D-LB-1, #1695), the place its PlaneView gives them. Unset, under the title bar. */
  const [inboxLinesAt, setInboxLinesAt] = useState<HTMLDivElement | null>(null);
  /** How many lines the window lists now: counted in the front project's status line. */
  const [windowLinesListed, setWindowLinesListed] = useState(0);
  /** Where the window's kept results are drawn: under the front project's Inbox's Notices
   *  (#3344). */
  const [resultsAt, setResultsAt] = useState<HTMLDivElement | null>(null);
  const onWindowLines = useCallback((_counted: number, listed: number) => {
    setWindowLinesListed(listed);
  }, []);
  /** The project in front, for a verb that is kept stable across renders. */
  const inFrontNow = useRef<PlaneId | undefined>(undefined);
  /** Why this launch took longer than the limit, when it did — and nothing when it did not
   *  (charter-app#24). The core decides that; the window only draws it. */
  const [slowStart, setSlowStart] = useState<SlowStart>();
  /** Projects the last quit had open that charter would not take back, each with its line.
   *  Never an error dialog: a restore is a convenience (ADR 0033). */
  const [notRestored, setNotRestored] = useState<string[]>([]);
  /** The projects the last quit had open that have moved or gone: Locate… and Forget (NO-5). */
  const [goneAtLaunch, setGoneAtLaunch] = useState<GoneProject[]>([]);
  /** The gone projects the opener is drawing now. It owns those, so they are drawn once. */
  const [openerGone, setOpenerGone] = useState<readonly string[] | "unread">("unread");
  /** Bumped when a gone project is located or forgotten up here, so an opener reads again. */
  const [goneChanged, setGoneChanged] = useState(0);
  const goneSettled = useCallback(
    (path: string) => setGoneAtLaunch((was) => was.filter((one) => one.path !== path)),
    [],
  );
  /** The gone projects dismissed, this run or kept from an earlier one (#1291). */
  const goneDismissed = useGoneDismissed();
  // Forgotten or located from Settings › You › This machine settles this window's line about it
  // too, and an opener drawing it reads again (#1291).
  useEffect(
    () =>
      whenRecentSettled((path) => {
        goneSettled(path);
        letGoneGo(path);
        setGoneChanged((n) => n + 1);
      }),
    [goneSettled],
  );
  /** Whether the cold-launch restore is still going. Until it is done the window has not
   *  finished saying which projects it holds, so neither the quit nor the arrangement it
   *  writes down may act on what it holds so far. */
  const [restoring, setRestoring] = useState(true);
  /** The launch's question, while it waits for the operator (charter-app#250): what would be
   *  put back, and how the answer reaches the restore that is waiting on it. */
  const [relaunchAsking, setRelaunchAsking] = useState<{
    question: RelaunchQuestion;
    answer: (choice: RelaunchChoice) => void;
  }>();
  /** Settled when the cold-launch restore is over, so a second launch's directory waits for
   *  it: opening a project starts its chats, and nothing may start while the launch's
   *  question is up. */
  const restoreOver = useMemo(() => {
    let settle = () => {};
    const done = new Promise<void>((resolve) => (settle = resolve));
    return { done, settle };
  }, []);
  /** Whether this window has ever held a project.
   *
   *  After it has, the opener is no longer the launch reporting what it could not resolve:
   *  the operator closed what was open, and "charter found no project here" would be charter
   *  answering a question nobody asked about a directory nobody is standing in. */
  const [heldSomething, setHeldSomething] = useState(false);
  useEffect(() => {
    if (planes.length > 0) setHeldSomething(true);
  }, [planes.length]);

  /**
   * **Every open project's alerts, read once for the window** (`alerts.ts`): an alert is about a
   * plane, and the plane that matters is often not the one on screen. Each project's Inbox lists
   * its own as Notices and names each other project that has some (#1695): the Alerts drawer
   * that listed them all folded into the Inbox (spec #1688, I-4).
   */
  const { reading: alertsRead, reread: rereadAlerts } = useAlerts(planes);

  /**
   * The panels approved extensions contribute to every project's side region.
   *
   * **Here and not in each `PlaneView`, for the reason the alerts are here**: this is about the
   * MACHINE and not about a project. An extension is installed per machine (ADR 0041 —
   * *an extension never travels in a plane*), so a window holding eight projects would
   * otherwise take the same survey eight times — and a survey re-hashes every installed
   * extension's whole directory, which is 0041's named cost of fingerprinting code.
   *
   * Asked once, after the first frame. Nothing waits on it: a window with no extensions gets an
   * empty list and charter's own two panels, which is every window until one is installed.
   */
  const contributedPanels = useContributedPanels();
  /** The views approved extensions offer — the persona statistics button is one — asked once
   *  per window for the same reason as the panels above. */
  const extensionViews = useExtensionViews();
  const extensionCommands = useExtensionCommands();

  /** The projects, as the strip and the palette name them. */
  const projects = useMemo<Project[]>(
    () => planes.map((plane) => ({ plane, name: calledOn(plane) })),
    [planes],
  );

  /**
   * The projects this operator has pinned, by root (ADR 0039).
   *
   * **The window's and not a project's**, because the project strip is the window's: a
   * project that is not in front draws nothing, and its own pin still has to be on the strip.
   * Each `PlaneView` reports its own, and this is where they meet.
   *
   * It is a list of roots rather than a flag per project for the reason `actions.ts` gives:
   * a pin is the operator's arrangement of the projects and not a property of one, so it is
   * held once, here, instead of copied onto each.
   */
  const [pinnedProjects, setPinnedProjects] = useState<string[]>([]);

  // What this machine already remembers as pinned, asked once per project it holds. A pin
  // outlives the app, so a window that did not ask would draw an operator's arrangement as
  // if they had never made it. Asked per plane rather than as one list, because the machine
  // store's answer for a plane is what `plane_pins` gives and there is no second reader of
  // that file in the app.
  useEffect(() => {
    let gone = false;
    for (const plane of planes) {
      void commands
        .planePins(plane)
        .then((answer) => {
          if (gone || answer.status !== "ok" || !answer.data.project) return;
          setPinnedProjects((was) => (was.includes(plane) ? was : [...was, plane]));
        })
        // A window that cannot ask simply draws nothing pinned. Every project is still there.
        .catch(() => undefined);
    }
    return () => {
      gone = true;
    };
  }, [planes]);

  /**
   * Pins or unpins one project. The core's refusal travels back whole — the store is
   * bounded, and "unpin one first" is a sentence the operator can act on.
   *
   * **`early` draws the pin before the core has written it**, and takes it back if the core
   * refuses. A drop across the pinned boundary asks for it (SI-6b): the strip's order changes
   * with the drop, and a pin that waited for the core would draw the tab in its old group
   * until the answer came — a jump under the pointer. Every other way in waits, so a mark is
   * never drawn that the store does not have.
   */
  const pinProject = useCallback(
    async (plane: string, pinned: boolean, { early = false } = {}): Promise<Ran> => {
      const mark = (on: boolean) =>
        setPinnedProjects((was) =>
          on ? (was.includes(plane) ? was : [...was, plane]) : was.filter((one) => one !== plane),
        );
      if (early) mark(pinned);
      const answer = await commands
        .pinProject(plane, pinned)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (answer.status === "error") {
        if (early) mark(!pinned);
        return { ok: false, refused: answer.error };
      }
      mark(pinned);
      // Settings › You › This machine lists the pins, and may be on screen (#1240).
      machineChanged();
      return { ok: true };
    },
    [],
  );

  /**
   * Takes a project into this window, as a tab.
   *
   * **The one path for all five ways in**: a recents row, a picked folder, a typed path, a
   * second launch handing its directory over, and the cold-launch restore. Every one of them
   * reaches `open_plane`, so the trust gate (ADR 0035) is the same gate — the core reads this
   * machine's record against the project as it is on disk at that instant and either opens it
   * or answers with the question. **A window that formed its own opinion about trust would be
   * a second gate beside the one that bites.**
   *
   * Answers with the project when one opened, so a restore can decide which of several ends
   * up in front rather than landing on whichever answered last.
   *
   * **A project another window holds is not taken in twice** (charter#126): one project is
   * one tab in one window, so opening it here brings that window to the front instead. And
   * `into: false` opens it without taking it into this window at all, for a restore that is
   * about to move it into a window of its own.
   */
  const openInto = useCallback(
    async (path: string, andShow: boolean, into = true): Promise<PlaneId | undefined> => {
      setOpenTrouble(undefined);
      // A project this window is still letting go of opens once that is done (#1949): the
      // core would make it wait anyway, and the window says why in the meantime.
      const closing = closingNow.current.get(path);
      if (closing !== undefined) {
        setOpenTrouble(STILL_CLOSING);
        await closing;
        setOpenTrouble((said) => (said === STILL_CLOSING ? undefined : said));
      }
      const answer = await commands
        .openPlane(path)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      // Verbatim: the sentence names the path and what was wrong with it, and an operator shown
      // a reworded version of it can neither act on it nor search for it.
      if (answer.status === "error") {
        setOpenTrouble(answer.error);
        return undefined;
      }
      if (answer.data.plane === null) {
        // The operator has to be asked first. Nothing was attached and nothing was started.
        const ask = answer.data.ask;
        if (ask)
          setApproving((queue) =>
            queue.some((q) => q.path === ask.path) ? queue : [...queue, ask],
          );
        return undefined;
      }
      const plane = answer.data.plane;
      if (!planesNow.current.includes(plane)) {
        const holder = await commands.showWindowHolding(plane).catch(() => null);
        if (holder !== null && holder !== own) return undefined;
      }
      if (!into) return plane;
      // A project already in the strip keeps its place: "open it" for one this window holds
      // means "show me that project", which is what a recents row and a second launch both
      // mean when they name one that is already a tab.
      setPlanes((was) => (was.includes(plane) ? was : [...was, plane]));
      if (andShow) {
        setShowing({ at: "plane", plane });
        // A project the guided steps asked for is held behind their harness step (#1817).
        if (stepsNow.current.awaiting)
          stepsNow.current.arrived({ plane, cwd: plane, workspace: OUTSIDE });
      }
      return plane;
    },
    [own],
  );

  /**
   * The operator said yes. The approval carries back the contribution they were shown, so it
   * is an answer to the question that was asked and not to whatever the project says by the
   * time the button is pressed.
   *
   * A refusal closes the dialog rather than keeping it up with a message: the one refusal
   * this command gives is "it changed while you were reading it", and the only honest repair
   * is to ask again about what it says now — which is what opening it again does.
   */
  const approveProject = useCallback(async (ask: Ask) => {
    const answer = await commands
      .approvePlane(ask.path, ask.contributes)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setApproving((queue) => queue.filter((q) => q.path !== ask.path));
    if (answer.status === "error") {
      setOpenTrouble(answer.error);
      return;
    }
    setOpenTrouble(undefined);
    const plane = answer.data;
    setPlanes((was) => (was.includes(plane) ? was : [...was, plane]));
    setShowing({ at: "plane", plane });
    // A project the guided steps asked for is held behind their harness step (#1817): a repo's
    // where its first chat starts, any other at its root.
    if (stepsNow.current.awaiting) {
      const repo = madeAsking.current?.asking === ask.path ? madeAsking.current : undefined;
      stepsNow.current.arrived({
        plane,
        cwd: repo?.cwd ?? plane,
        workspace: repo?.workspace ?? OUTSIDE,
      });
    }
    // A repo opened into this project was waiting on this yes for its first chat.
    setFirstChat((was) =>
      was?.asking === ask.path ? { ...was, plane, asking: null, at: was.at + 1 } : was,
    );
    setShellAsk((was) =>
      was?.asking === ask.path ? { ...was, plane, asking: null, at: was.at + 1 } : was,
    );
  }, []);

  /** Lets go of one project. Its chats end, its record is written into it, and its tab goes.
   *  Nothing of the project on disk goes. Reached through {@link closeProject}, which asks
   *  first when there is anything to end. */
  /** Takes one project's tab out of this window, and nothing else: closing it and moving it to
   *  another window both end here. */
  const takeOut = useCallback((plane: string) => {
    setPlanes((was) => {
      const at = was.indexOf(plane);
      // Twice is once: the window that asked the close takes the tab out, and then hears the
      // core say `plane-closed` about the same project (#1242).
      if (at < 0) return was;
      const left = was.filter((held) => held !== plane);
      // The tab beside it comes to the front, which is `closeTab`'s rule one scope up. With
      // nothing left, the opener — the window has to stay useful with no project (#111).
      setShowing((on) =>
        on.at === "plane" && on.plane === plane
          ? left.length === 0
            ? { at: "opener" }
            : { at: "plane", plane: left[Math.max(at - 1, 0)] }
          : on,
      );
      return left;
    });
    // And the window forgets what that project had open: its report is about chats that have
    // just been ended, or that another window now draws, and a quit warning listing them here
    // would list them twice or list nothing.
    setReports((was) => {
      const { [plane]: gone, ...rest } = was;
      void gone;
      return rest;
    });
    // And the unsaved work its views held (#3929): asked about first where the close was this
    // window's, and never drawn here again.
    discardUnsavedIn(plane);
  }, []);

  const letGoOf = useCallback(
    async (plane: string): Promise<Ran> => {
      // The core takes the project out of what it holds as it is asked, then writes its record
      // and ends its chats off the window's thread (#1949), which can take a while. Its tab goes
      // now, so nothing in it is pressed again meanwhile: a second close of a project already
      // let go of is refused.
      const asked = commands
        .closePlane(plane)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      closingNow.current.set(plane, asked);
      setLettingGo((now) => now + 1);
      void asked.finally(() => {
        if (closingNow.current.get(plane) === asked) closingNow.current.delete(plane);
        setLettingGo((now) => now - 1);
      });
      takeOut(plane);
      const answer = await asked;
      if (answer.status === "error") return { ok: false, refused: answer.error };
      return { ok: true, said: `purlis let go of ${plane}. Nothing in it was changed.` };
    },
    [takeOut],
  );

  /**
   * Moves one project into another window — a new one when `to` is null (charter#126).
   *
   * **Nothing is closed.** Its chats go on running in the core, and the window it arrives in
   * draws them from what the core has open, as a window that reloaded would. This window only
   * takes its tab out. A split window left holding nothing goes (`windows.rs`), and the main
   * window left holding nothing shows the opener.
   */
  /** **A project's close, its move or a quit waiting on the answer about the unsaved work it
   *  would lose** (#3929, `unsavedWork.ts`), asked before what it ends is. */
  const [unsavedAsk, setUnsavedAsk] = useState<UnsavedAsk>();
  const unsavedAskNow = useRef(unsavedAsk);
  /** Asks about `ask`'s work, unless a question about unsaved work is up already: that one is
   *  answered first, and this ask is one not taken (`kept`). */
  const askUnsaved = useCallback((ask: UnsavedAsk) => {
    if (unsavedAskNow.current !== undefined) return ask.kept?.();
    unsavedAskNow.current = ask;
    setUnsavedAsk(ask);
  }, []);
  const unsavedAnswered = useCallback(() => {
    unsavedAskNow.current = undefined;
    setUnsavedAsk(undefined);
  }, []);

  const moveProject = useCallback(
    async (plane: string, to: string | null, unsavedAsked = false): Promise<Ran> => {
      // **Its views' unsaved work stays in this window** (#3929, #4028): asked about first.
      const work = unsavedAsked ? [] : unsavedIn(plane);
      if (work.length > 0) {
        askUnsaved({ work, then: () => void moveProject(plane, to, true) });
        return { ok: true };
      }
      const answer = await commands
        .moveProjects([plane], plane, to)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (answer.status === "error") return { ok: false, refused: answer.error };
      takeOut(plane);
      return { ok: true };
    },
    [askUnsaved, takeOut],
  );

  /** The project whose close is waiting on the operator's answer (`ClosingProject`). */
  const [closing, setClosing] = useState<string>();

  /** Every project's report as of the last render, read when a close arrives rather than
   *  closed over, so the verb below stays one function. */
  const reportsNow = useRef(reports);
  useLayoutEffect(() => {
    reportsNow.current = reports;
  });

  /**
   * The `project.close:<plane>` row's verb: **ask first when the project has chats open**, and
   * let go of it at once when it has none — the operator's ruling on charter-app#239, where
   * Delete on a project's tab put "end every chat in it" one key away.
   *
   * Here, where the row is carried out, and not on any one button, so the `×`, Delete, the
   * tab's menu and the palette all ask it (`EndingChat` is the same rule one scope down). A
   * project that has not yet said what it has open is asked about too: "no tabs" there is
   * "not yet", the quit warning's rule. It answers `{ ok: true }` for a row it has only asked
   * about, as `PlaneView`'s `run` does.
   */
  const closeProject = useCallback(
    async (plane: string, unsavedAsked = false): Promise<Ran> => {
      // **Its views' unsaved work is asked about first** (#3929): then what the close ends. The
      // work goes with the project once it is let go of (`takeOut`).
      const work = unsavedAsked ? [] : unsavedIn(plane);
      if (work.length > 0) {
        askUnsaved({ work, then: () => void closeProject(plane, true) });
        return { ok: true };
      }
      const said = reportsNow.current[plane];
      if (said?.settled && said.ending.length === 0) return letGoOf(plane);
      setClosing(plane);
      return { ok: true };
    },
    [askUnsaved, letGoOf],
  );

  /**
   * Scaffolds a new project and takes it into this window — **through the same gate**.
   *
   * `create_project` writes the plane and then answers with `open_if_approved`'s own answer,
   * so this handles it exactly as `openInto` handles a recents row: a project, or the question
   * to ask first. A plane charter has just made is still one this machine has approved nothing
   * about (ADR 0035), so the ordinary end of this dialog is the trust dialog.
   *
   * A refusal stays IN the dialog rather than behind it: `init`'s refusal in a repository is
   * four lines naming what to do instead, and the operator is still standing at the box.
   */
  const makeProject = useCallback(
    async (
      path: string,
      planeIsThisRepo: boolean,
      adopt: string,
      forge: ForgeWord | null = null,
    ) => {
      setMakingProject(true);
      setCreateForgeAsk(undefined);
      const answer = await commands
        .createProject(path, planeIsThisRepo, adopt === "" ? null : adopt, forge)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      setMakingProject(false);
      if (answer.status === "error") {
        setCreateTrouble(answer.error);
        return;
      }
      const made = answer.data.opened;
      if (made === null) {
        // The repo's remote did not say which forge (#839): asked in the dialog, and the
        // answer makes the project.
        const why = answer.data.asks_forge ?? "";
        setCreateForgeAsk({
          why,
          answer: (named) => void makeProject(path, planeIsThisRepo, adopt, named),
        });
        return;
      }
      setCreating(false);
      setCreateTrouble(undefined);
      if (made.plane === null) {
        const ask = made.ask;
        if (ask)
          setApproving((queue) =>
            queue.some((q) => q.path === ask.path) ? queue : [...queue, ask],
          );
        return;
      }
      const opened = made.plane;
      setPlanes((was) => (was.includes(opened) ? was : [...was, opened]));
      setShowing({ at: "plane", plane: opened });
    },
    [],
  );

  /** What an opened repo puts in the window: its first chat, through the trust gate, and what
   *  taking it in did to the project's forges, where there is something to say (#1669). */
  const openedRepo = useCallback((repo: OpenedRepo) => {
    const { opened, workspace, cwd, harness, none_installed, instructions, forge_said } = repo;
    // Said once, on the window's line: the repo is open either way, and a forge that was not
    // added names its own fix (Settings › Project › Forges).
    if (forge_said) setReport({ from: "repo.forge", refused: false, words: forge_said });
    const plane = opened.plane;
    const ask = opened.ask;
    setFirstChat((was) => ({
      workspace,
      cwd,
      harness,
      noneInstalled: none_installed,
      instructions,
      plane,
      asking: plane === null ? (ask?.path ?? null) : null,
      at: (was?.at ?? 0) + 1,
    }));
    if (plane === null) {
      if (ask) {
        madeAsking.current = { asking: ask.path, cwd, workspace };
        setApproving((queue) => (queue.some((q) => q.path === ask.path) ? queue : [...queue, ask]));
      }
      return;
    }
    setPlanes((was) => (was.includes(plane) ? was : [...was, plane]));
    setShowing({ at: "plane", plane });
    // The repo the guided steps asked for: held behind their harness step (#1817).
    if (stepsNow.current.awaiting) stepsNow.current.arrived({ plane, cwd, workspace });
  }, []);

  /**
   * Opens a repo into this machine's local project and asks for the first chat in it
   * (FR-4, #603). Nothing asks where the project goes: the core makes it in charter's own
   * directory when there is none, clones the repo into a workspace named after it, and
   * answers as `create_project` does — a project, or the trust question to ask first.
   *
   * Answers why nothing was opened (`refused`), or that the forge has to be asked first
   * (`asksForge`, #839), or neither when something was opened, so the first run and the New
   * project dialog can each keep the refusal or the question where the operator is standing.
   * `forge` is the answer to that question, on the call after it.
   */
  const openRepo = useCallback(
    async (
      path: string,
      template: TemplateChoice,
      forge: ForgeWord | null = null,
    ): Promise<RepoTried> => {
      setOpeningRepo(true);
      const answer = await commands
        .openRepo(path, template, forge)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      setOpeningRepo(false);
      if (answer.status === "error") return { refused: answer.error };
      if (answer.data.opened === null) return { asksForge: answer.data.asks_forge ?? "" };
      openedRepo(answer.data.opened);
      return {};
    },
    [openedRepo],
  );

  /**
   * The first run's repo, opened, with its refusal or its forge question kept on the first-run
   * screen. The question's answer opens it again, with the forge named.
   */
  const firstRunRepo = useCallback(
    (path: string, template: TemplateChoice, forge: ForgeWord | null = null) => {
      setRepoTrouble(undefined);
      setRepoForgeAsk(undefined);
      void openRepo(path, template, forge).then(({ refused, asksForge }) => {
        setRepoTrouble(refused);
        if (asksForge !== undefined)
          setRepoForgeAsk({
            why: asksForge,
            answer: (named) => firstRunRepo(path, template, named),
          });
      });
    },
    [openRepo],
  );

  /** The same, from the New project dialog, which closes once the repo is open. */
  const dialogRepo = useCallback(
    (path: string, forge: ForgeWord | null = null) => {
      setCreateTrouble(undefined);
      setCreateForgeAsk(undefined);
      // The New project dialog lays out no template: the first run is where one is chosen
      // (FR-17).
      void openRepo(path, { kind: "no-template" }, forge).then(({ refused, asksForge }) => {
        if (asksForge !== undefined)
          setCreateForgeAsk({ why: asksForge, answer: (named) => dialogRepo(path, named) });
        else if (refused === undefined) setCreating(false);
        else setCreateTrouble(refused);
      });
    },
    [openRepo],
  );

  /**
   * "Sign in to GitHub" or "Sign in to GitLab" on the first run (FR-4, W10's "detected and
   * offered"): the local project is opened — made first when there is none, and through the
   * trust gate — and `<cli> auth login` is typed into a shell tab at its root. The forge CLI's
   * own login, in a tab the operator can leave; nothing here asks anything. `cli` is the one the
   * core's first-run row names (`gh`, `glab`). With no repo to read, the forge whose sign-in
   * was pressed is the one a project made here tracks (#839).
   */
  const signInToForge = useCallback(async (row: ForgeRow): Promise<string | undefined> => {
    const cli = row.cli;
    const answer = await commands
      .openLocalProject(row.forge)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    if (answer.status === "error") return answer.error;
    const { plane, ask } = answer.data;
    setShellAsk((was) => ({
      plane,
      asking: plane === null ? (ask?.path ?? null) : null,
      typed: `${cli} auth login`,
      at: (was?.at ?? 0) + 1,
    }));
    if (plane === null) {
      if (ask)
        setApproving((queue) => (queue.some((q) => q.path === ask.path) ? queue : [...queue, ask]));
      return undefined;
    }
    setPlanes((was) => (was.includes(plane) ? was : [...was, plane]));
    setShowing({ at: "plane", plane });
    return undefined;
  }, []);

  const onReport = useCallback((plane: PlaneId, mine: PlaneReport) => {
    setReports((was) => (was[plane] === mine ? was : { ...was, [plane]: mine }));
  }, []);
  /** Each project's session rows, which travel apart from its report (#1160): a record written
   *  hands up its two rows without the project view being drawn to build its catalogue. */
  const [sessionRows, setSessionRows] = useState<Record<string, readonly Offer[]>>({});
  const onSessionRows = useCallback((plane: PlaneId, rows: readonly Offer[]) => {
    setSessionRows((was) => {
      if ((was[plane] ?? []).length === 0 && rows.length === 0) return was;
      return was[plane] === rows ? was : { ...was, [plane]: rows };
    });
  }, []);

  const windowDoes = useMemo<WindowDoing>(
    () => ({
      openProject: () => setShowing({ at: "opener" }),
      createProject: () => {
        setCreateTrouble(undefined);
        setCreating(true);
      },
      showExtensions: () => setExtensions(true),
      toggleFocusMode,
      installCli,
      selectProject: (plane: string) => setShowing({ at: "plane", plane }),
      switchProject: () => setSwitcherAsk((was) => was + 1),
      closeProject,
      moveProject,
      pinProject,
      openSettings: (plane: string) => {
        setShowing({ at: "plane", plane });
        setSettingsAsk((was) => ({ plane, at: (was?.at ?? 0) + 1 }));
      },
      openSaving: (plane: string) => {
        setShowing({ at: "plane", plane });
        setSavingAsk((was) => ({ plane, at: (was?.at ?? 0) + 1 }));
      },
      openSettingsTab: () => {
        const plane = inFrontNow.current;
        if (plane === undefined) showSettingsAlone(setSettingsAlone);
        else setSettingsTabAsk((was) => ({ plane, at: (was?.at ?? 0) + 1 }));
      },
      openYourSettings: () => {
        const plane = inFrontNow.current;
        if (plane === undefined) showSettingsAlone(setSettingsAlone);
        else setYourSettingsAsk((was) => ({ plane, at: (was?.at ?? 0) + 1 }));
      },
      quit: () => void commands.askToQuit().catch(() => undefined),
    }),
    [closeProject, moveProject, pinProject, toggleFocusMode],
  );

  /**
   * **Follows a link into a You group, or into a project's** (SE-22, `settings/links.ts`). A You
   * group opens on the strip of the project in front (`PlaneView`'s `settingsLinkAsked`); with
   * none, the group is shown at You's place and Settings is drawn where the opener is. A
   * project's group — what an alert about that project names in its Inbox (NO-6) — brings that
   * project to the front and opens its Settings there. Never through Settings… (⌘,), which
   * opens at the level that is focused rather than the one the link names.
   */
  const openSettingsAt = useCallback((group: string, about?: PlaneId, setting?: string) => {
    const level = levelOf(group);
    // The setting itself, where the row names one: the link focuses its control (#1289).
    const link: SettingsLink = setting === undefined ? { group } : { group, setting };
    if (level === "project" && about !== undefined) {
      setShowing({ at: "plane", plane: about });
      setSettingsLinkAsk((was) => ({ plane: about, link, at: (was?.at ?? 0) + 1 }));
      return;
    }
    if (level !== "you") return;
    const plane = inFrontNow.current;
    if (plane === undefined) {
      linkToGroup(settingsPlace("you"), group, setting);
      showSettingsAlone(setSettingsAlone);
    } else setSettingsLinkAsk((was) => ({ plane, link, at: (was?.at ?? 0) + 1 }));
  }, []);

  /**
   * **Follows a refusal's way to its setting** (#1201): the link beside the last action's words,
   * on the line under the title bar or in the palette. Into the project in front's window, which
   * opens that level's Settings tab at the group; with none, a You group is shown where the
   * opener is.
   */
  const followSaid = useCallback((link: SettingsLink) => {
    const plane = inFrontNow.current;
    if (plane !== undefined) {
      askSettingsLink(plane, link);
      return;
    }
    if (levelOf(link.group) !== "you") return;
    linkToGroup(settingsPlace("you"), link.group, link.setting);
    showSettingsAlone(setSettingsAlone);
  }, []);

  /** What the alerts' ways out do in the window (NO-6), from whichever project's Inbox. */
  const alertsDo = useMemo<InboxAlertsDo>(
    () => ({
      openSettings: openSettingsAt,
      openProject: (path: string) => void openInto(path, true),
      openSaving: windowDoes.openSaving,
      reread: rereadAlerts,
      openInboxOf: (plane: PlaneId) => {
        setShowing({ at: "plane", plane });
        setInboxAsk((was) => ({ plane, at: (was?.at ?? 0) + 1 }));
      },
    }),
    [openInto, openSettingsAt, rereadAlerts, windowDoes],
  );
  const alerts = useMemo<WindowAlerts>(
    () => ({ reading: alertsRead, planes, nameOf: calledOn, does: alertsDo }),
    [alertsRead, planes, alertsDo],
  );

  // Which plane this launch opened — asked once, and the answer the first tab is built from.
  // The core resolved the working directory once to get it; nothing asks again.
  useEffect(() => {
    // A split window is not a launch: it was made to hold what was moved into it, and the
    // working directory was resolved once, for the main window (ADR 0034).
    if (split) {
      setLaunch({ plane: null, here: false, reason: "" });
      return;
    }
    void commands
      .planeAtLaunch()
      .then((it) =>
        setLaunch({
          plane: it.plane,
          // A directory it could read, that is in no plane, versus nothing to go on.
          here: it.from !== null,
          reason: it.why ?? "purlis has no plane open.",
        }),
      )
      // A command can also fail outright, with no answer of its own to give.
      .catch((err: unknown) => setLaunch({ plane: null, here: true, reason: String(err) }));
  }, [split]);

  /**
   * The window set the last quit left behind, put back (ADR 0033, spec decision 28).
   *
   * **Each project goes through `openInto`, which is the gate.** Opening one starts the
   * programs its reopen record names, so a restore may not be a way past the ask — a project
   * that has started contributing more than what was approved raises the same dialog it would
   * have raised from a recents row, and the rest of the restore carries on around it.
   *
   * **A project that has moved or is gone is dropped with a line saying so, never an error
   * dialog.** The core decides that; this draws the lines.
   *
   * Run once, after the launch has answered, and only then — the launch's own project is
   * attached by the core before there is a window, and its tab has to be the first one.
   *
   * **And not before the operator has answered the launch's question** (charter-app#250):
   * reopen every session, or start fresh. The core holds the launch's own project back until
   * `relaunch` has the answer, and the projects below are opened only after it, so nothing
   * starts while the question is up. A launch with nothing to put back asks nothing and
   * answers "Reopen all" itself — which is also what a question the core could not read
   * answers, because the other answer is the one that throws work away.
   */
  const restored = useRef(false);
  useEffect(() => {
    if (launch === undefined || restored.current) return;
    restored.current = true;
    // **A split window restores nothing and asks nothing.** It draws what was moved into it,
    // which the core already holds for it (`projects_handed`) — and asks again after a reload,
    // which is how a reloaded split window comes back with its tabs.
    if (split) {
      void (async () => {
        try {
          const held = await commands.projectsHanded().catch(() => null);
          if (held) {
            setPlanes(held.planes);
            const front = held.active === null ? undefined : held.planes[held.active];
            if (front !== undefined) setShowing({ at: "plane", plane: front });
          }
        } finally {
          setRestoring(false);
          restoreOver.settle();
        }
      })();
      return;
    }
    void (async () => {
      try {
        const asked = await commands
          .relaunchAsk()
          .catch(() => ({ status: "error" as const, error: "" }));
        const question = asked.status === "ok" ? asked.data : null;
        const choice: RelaunchChoice = question
          ? await new Promise<RelaunchChoice>((answer) =>
              setRelaunchAsking({
                question,
                answer: (chosen) => {
                  setRelaunchAsking(undefined);
                  answer(chosen);
                },
              }),
            )
          : "ReopenAll";
        // Awaited, so the launch's project has its chats registered before its tab asks what it
        // holds: each is drawn at once, as restoring, and starts behind the window
        // (`restoring.rs`).
        await commands.relaunch(choice).catch(() => undefined);
        // The launch's own project, which the core already opened and has now put the record
        // back for. Its tab is first and it is the one in front: the operator ran charter THERE.
        const opened = launch.plane;
        if (opened !== null) {
          setPlanes((was) => (was.includes(opened) ? was : [...was, opened]));
          setShowing({ at: "plane", plane: opened });
        }
        const answer = await commands
          .planesToRestore()
          .catch(() => ({ status: "error" as const, error: "" }));
        const back = answer.status === "ok" ? answer.data : undefined;
        setNotRestored(back?.dropped ?? []);
        setGoneAtLaunch(back?.gone ?? []);
        // A gone project dismissed at an earlier launch stays dismissed only while the core
        // still says it is gone: asked once, whether or not the opener is drawn (#1291).
        if (goneKept().length > 0)
          void commands
            .recentPlanes()
            .then((recents) => {
              if (recents.status !== "ok") return;
              const stillGone = (recents.data?.gone ?? []).map((one) => one.path);
              settleGone(stillGone);
              settleGoneKept(stillGone);
            })
            .catch(() => undefined);
        const [first, ...splits] = back?.windows ?? [];
        let front: PlaneId | undefined;
        for (const [at, path] of (first?.planes ?? []).entries()) {
          const plane = await openInto(path, false);
          if (plane !== undefined && at === first?.active) front = plane;
        }
        // **Every other remembered window comes back as a window of its own** (ADR 0033,
        // amended 2026-09-26). Each project is still opened here, through the same gate — a
        // project that raises the trust question asks it in this window, and lands here once
        // approved — and only then moved into its window. The launch's own project stays here.
        for (const window of splits) {
          const opened: PlaneId[] = [];
          let inFrontThere: PlaneId | null = null;
          for (const [at, path] of window.planes.entries()) {
            const plane = await openInto(path, false, false);
            if (plane === undefined || plane === launch.plane) continue;
            opened.push(plane);
            if (at === window.active) inFrontThere = plane;
          }
          if (opened.length > 0)
            await commands.moveProjects(opened, inFrontThere, null).catch(() => undefined);
        }
        // The remembered front tab, unless the launch already named one: a terminal launch
        // inside a project is the operator saying which project he means, and it outranks an
        // arrangement from yesterday.
        if (opened === null && front !== undefined) setShowing({ at: "plane", plane: front });
      } finally {
        // Whatever happened, the restore is over. A window that stayed `restoring` would
        // never write its arrangement down and would warn on every quit for the rest of the
        // day, which is a worse failure than the one that caused it.
        setRestoring(false);
        restoreOver.settle();
      }
    })();
  }, [launch, openInto, restoreOver, split]);

  // Nothing is ever drawn on a project this window does not hold. `showing` is set from
  // several places — a close, a restore, an approval — and a plane that went in between
  // would otherwise leave the window pointed at a project with no `PlaneView` behind it.
  // **And a project the guided steps hold behind their harness step is not drawn yet** (#1817):
  // the steps are, until the person picks a harness or skips it.
  const inFront =
    showing.at === "plane" && planes.includes(showing.plane) && showing.plane !== steps.holding
      ? showing.plane
      : undefined;
  /** Whether the opener — and the window's own palette with it — is what this window draws.
   *  Only once the core has said what the launch resolved and the restore has finished
   *  opening what it remembered: both are about to decide whether there is a project here. */
  const openerUp = inFront === undefined && launch !== undefined && !restoring;
  useLayoutEffect(() => {
    inFrontNow.current = inFront;
    // A project arriving ends the stand-in: from here Settings is a tab, and the opener a later
    // close brings back must be the opener rather than a Settings left behind.
    if (inFront !== undefined) setSettingsAlone(false);
  }, [inFront]);
  // **Each switch is timed by the window itself** (`switchClock.ts`): once it has drawn the
  // project that came to the front, which is here, and the doctor's dialog lists what it took.
  useLayoutEffect(() => {
    if (inFront !== undefined) switched(inFront);
  }, [inFront]);
  /** About's Licences (#1719): opened as a tab of the project in front, which takes the ask.
   *  With none in front the ask is left, and About draws them in place. */
  useEffect(() => {
    const asked = (event: Event) => {
      const plane = inFrontNow.current;
      if (plane === undefined) return;
      event.preventDefault();
      setLicencesAsk((was) => ({ plane, at: (was?.at ?? 0) + 1 }));
    };
    window.addEventListener(OPEN_LICENCES, asked);
    return () => window.removeEventListener(OPEN_LICENCES, asked);
  }, []);
  /** What the project in front last said about itself, when it has said anything yet. The
   *  palette lists its catalogue and runs its rows, so a row reaches that project's live
   *  arrangement and no other's. */
  const saying = inFront === undefined ? undefined : reports[inFront];
  /** What the palette lists for the project in front: its catalogue, with its session rows. */
  const rowsInFront = inFront === undefined ? undefined : sessionRows[inFront];
  const paletteOffers = useMemo(
    () => (saying === undefined ? undefined : withSessionRows(saying.offers, rowsInFront ?? [])),
    [saying, rowsInFront],
  );
  /** The workspace the project in front is on, when it is on one (charter-app#281): its
   *  `workspace.json` is a layer of the theme the window draws, between the project's two files. */
  const workspaceInFront = saying?.workspace;
  /** The project in front once it knows which workspace it is on — its plane read — and not
   *  before: asked for the project alone first, the window would draw the project's theme and
   *  then repaint in the workspace's a moment later, a flash at every launch. */
  const settledInFront = saying?.read === true ? inFront : undefined;
  /** What the project in front has on in that workspace (charter-app#253, #280), for the one
   *  thing that is the window's and not a project's to draw: the theme. */
  const onInFront = useExtensionsOn(settledInFront, workspaceInFront);
  /** The theme the project in front picked there (charter-app#273, #281): `null` when nothing
   *  picked one. */
  const pickInFront = useProjectTheme(settledInFront, workspaceInFront);
  // The theme the project and workspace in front have, drawn once they have said what they have
  // on and what they picked — and every approved extension's with no project in front, which is
  // what the window drew before projects had a say (ADR 0048). After the first frame, like every
  // extension theme (`Extensions.tsx`). A project or workspace switch redraws it, and
  // `theme.onDrawn` hands it to every terminal on screen (#216). **Not while the launch is still
  // putting its projects back**: the first frame drew the levels of the project it opens on
  // (#2602), and drawing the window with no project in front meanwhile would repaint it twice.
  useEffect(() => {
    if (inFront === undefined) {
      if (!restoring) void drawThemeFor("every");
    } else if (onInFront !== undefined && pickInFront !== undefined)
      void drawThemeFor(onInFront, pickInFront, {
        plane: inFront,
        workspace: workspaceInFront,
      });
  }, [inFront, onInFront, pickInFront, workspaceInFront, restoring]);
  // And the workspace in front's colour on the window's accent and focus ring (charter-app#281),
  // live on every switch. The terminal is not told: nothing it draws is tinted.
  const colourInFront = saying?.colour ?? null;
  useEffect(() => {
    drawTint(colourInFront);
  }, [colourInFront]);
  /** What the last action answered: the newer of the project in front's answer and this
   *  window's own (#1669), so a window sentence is not hidden by an older project one. */
  const said = useLatestSaid(inFront, saying?.said, report);
  /** The result whose notice was closed: it stays closed until the next result (#3344). */
  const [closedSaid, setClosedSaid] = useState<Said>();
  /** Every result said, kept for the Inbox's Notices under Results (#3344). */
  const { kept: keptResults, drop: dropResult } = useKeptResults(said, inFront);
  const resultsInFront = useMemo(() => resultsFor(keptResults, inFront), [keptResults, inFront]);
  /** The title bar's chips of the project in front, which a notice about a workspace hangs from
   *  (#3648, `noticePlace.ts`). */
  const workspacesInFront = inFront === undefined ? undefined : reports[inFront]?.workspaces;
  const chipsInFront = useMemo(
    () =>
      workspacesInFront === undefined
        ? []
        : chipsOf({
            pins: workspacesInFront.pins,
            dormant: workspacesInFront.dormant,
            focused: workspacesInFront.focused,
          }),
    [workspacesInFront],
  );
  /** **"X is gone, kept dormant", on its dormant chip** (#3651): the same line the Notices
   *  centre lists, with its Forget, and dismissed in the one per-project record both read. */
  const {
    dismissed: dismissedInFront,
    dismiss: dismissInFront,
    showAgain: showAgainInFront,
  } = useDismissals(inFront ?? "");
  const dormantInFront = workspacesInFront?.dormant;
  const forgetInFront = workspacesInFront?.forget;
  const standingInFront = useMemo<StandingNotice[]>(
    () =>
      (dormantInFront ?? [])
        .filter((name) => !dismissedInFront.has(dormantCause(name)))
        .map((name) => ({
          workspace: name,
          cause: dormantCause(name),
          says: dormantSays(name),
          fixes: [{ label: "Forget", onPress: () => forgetInFront?.(name) }],
          dismiss: () => dismissInFront(dormantCause(name)),
        })),
    [dormantInFront, dismissedInFront, dismissInFront, forgetInFront],
  );
  /** **Every ask the registry derives, in every project** (#1690): what the hand's number
   *  counts, and where it opens the Inbox. */
  const registry = useAsks(planes);
  /** **Brings chat `session` of the project in front forward** (#3652): a ✋ dot's press, whose
   *  chat answers its question on its own band. The same `showChat` a Go of the Inbox runs. */
  const bringChat = useCallback((session: number) => {
    const plane = inFrontNow.current;
    if (plane === undefined) return;
    setShowing({ at: "plane", plane });
    void reportsNow.current[plane]?.run({
      id: `needs.show:${session}`,
      title: "Show the chat that asks",
      available: true,
      reason: "",
      does: { verb: "showChat", session },
    });
    // The asking chat types next even where it was in front already: `showChat` hands the
    // keyboard over only when its pane changes, and the press was on the ✋ (#3652's review).
    giveKeyboardTo(plane, session);
  }, []);
  /** The notices hung from those chips: a view onto the kept results, so a dismissal in either
   *  place is one (#3648); and the ✋ of a chat asking to remove a chip's workspace (#3652). */
  const chipNotices = useChipNotices({
    asks: inFront === undefined ? undefined : registry.held[inFront],
    bring: bringChat,
    said,
    kept: keptResults,
    plane: inFront,
    chips: chipsInFront,
    drop: dropResult,
    onSettings: followSaid,
    standing: standingInFront,
    // A dormant chip's press asks for its line again, dismissed or not (#3651).
    bringBack: (workspace) => {
      if (dormantInFront?.includes(workspace) === true) showAgainInFront(dormantCause(workspace));
    },
  });
  /** **The status bar's card** (#3653): the project's and the app's results, its own list alone,
   *  which keeps while the chips change, so the project's view handed it is drawn again only when
   *  what the status bar shows does. It stays drawn under the palette, as a chip's card does: the
   *  palette is modal and gives the keyboard back where it was as it closes (#3653's review). */
  const statusList = chipNotices.on.get(AT_STATUS);
  const statusUp = chipNotices.open === AT_STATUS;
  const { show: showNotices, hide: hideNotices, dismiss: dismissNotice } = chipNotices;
  const statusNotices = useMemo<ChipNotices>(
    () => ({
      on: new Map(statusList === undefined ? [] : [[AT_STATUS, statusList]]),
      open: statusUp ? AT_STATUS : undefined,
      show: showNotices,
      hide: hideNotices,
      dismiss: dismissNotice,
      onSettings: followSaid,
    }),
    [statusList, statusUp, showNotices, hideNotices, dismissNotice, followSaid],
  );
  const windowLines = useMemo(
    () => ({
      at: setInboxLinesAt,
      count: windowLinesListed,
      resultsAt: setResultsAt,
      results: resultsInFront.length,
    }),
    [windowLinesListed, resultsInFront.length],
  );

  // What this window holds and what it has in front, told to the core. It buys two things: a
  // notification about a chat in a project the operator is NOT looking at is sent rather than
  // suppressed, and the arrangement is written into this machine's store so the next cold
  // launch puts it back.
  //
  // **Not while the restore is still running.** The store is where the arrangement comes
  // FROM, so a window that reported "I hold nothing" on its first render would wipe the very
  // record it is about to read.
  useEffect(() => {
    if (restoring) return;
    const at = inFront === undefined ? -1 : planes.indexOf(inFront);
    void commands.windowHoldsPlanes({ planes, active: at < 0 ? null : at }).catch(() => undefined);
  }, [inFront, planes, restoring]);

  // A second launch handed its directory over (ADR 0033). It goes through the same opener
  // every other path uses, so the trust ask is the same ask — and it opens as another project
  // tab rather than being said on screen, which is what tabs were the missing half of.
  useEffect(() => {
    const listening = listen<string>("open-plane", (event) => {
      void restoreOver.done.then(() => openInto(event.payload, true));
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [openInto, restoreOver]);

  // Projects moved into this window from another, or handed back by a split window that was
  // closed (charter#126). They are already open in the core; this window only draws them.
  useEffect(() => {
    const listening = listen<{ planes: PlaneId[]; front: PlaneId | null }>(
      "projects-arrived",
      (event) => {
        const { planes: arrived, front } = event.payload;
        setPlanes((was) => [...was, ...arrived.filter((plane) => !was.includes(plane))]);
        if (front !== null) setShowing({ at: "plane", plane: front });
      },
    ).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, []);

  // The core let go of a project (#1242): by this window's own `×`, which has already taken
  // it out, or behind this window's back — the UI RPC, another window. Either way it is not
  // drawn a moment longer: its tab, its chats and its views go, and what comes next comes to
  // the front, as a close from here would.
  useEffect(() => {
    const listening = listen<{ plane: PlaneId }>("plane-closed", (event) =>
      takeOut(event.payload.plane),
    ).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [takeOut]);

  // Cold start ends when a person can see the window, which is the frame after the one this
  // paints in. The core answers with why that took as long as it did, when it took longer
  // than the limit, and with nothing at all otherwise — so on an ordinary launch this is one
  // IPC call that changes nothing. Nothing about the window depends on the marker arriving:
  // a window that cannot send it is still a window.
  useEffect(() => {
    let gone = false;
    const frame = requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (gone) return;
        void commands
          .firstFrame()
          .then((why) => {
            if (!gone && why) setSlowStart(why);
          })
          .catch(() => undefined);
        // The first frame is on screen: the theme's faces are asked for now (#3034).
        facesAfterTheFirstFrame();
      }),
    );
    return () => {
      gone = true;
      cancelAnimationFrame(frame);
    };
  }, []);

  // What a quit would end, across every project this window holds. A warning that counted
  // only the project on screen would understate what it is about to end by however many
  // projects the operator had merged into the window.
  const ending = useMemo<Ending[]>(
    () => planes.flatMap((plane) => reports[plane]?.ending ?? []),
    [planes, reports],
  );
  /** Whether the core has answered, for every project, what it already had open. Before that
   *  "no tabs" is "not yet", and quitting on it would end chats the window had not drawn. */
  const settled =
    launch !== undefined && !restoring && planes.every((plane) => reports[plane]?.settled);

  // Read at the moment the event arrives rather than closed over, so the one listener below
  // is registered once and never torn down and rebuilt mid-quit. Every window's, not only this
  // one's (charter#126): it is set below, once the other windows' share is known.
  const atQuit = useRef({ settled, ending });

  // Something asked the app to quit: the menu, the tray, or Cmd-Q. The answer is the
  // operator's, and it is given here because this is where what would be ended is known.
  useEffect(() => {
    // The catch is attached here and not in the cleanup: a window that cannot listen is
    // still a window, and a rejection nothing is holding yet is an unhandled one.
    const askToQuit = () => {
      // Nothing to end is nothing to warn about — but only once every project has said what
      // it has open. Before that, no tabs means "not yet", and quitting on it would end every
      // chat the window had not drawn.
      if (atQuit.current.settled && atQuit.current.ending.length === 0)
        void commands.quit().catch(() => undefined);
      else setAsking(true);
    };
    const listening = listen("quit-asked", () => {
      // **The unsaved work of the window's views is asked about first** (#3929): Keep editing
      // is a quit not taken, which the core is told as the warning's Not now tells it.
      const work = unsavedNow();
      if (work.length === 0) return askToQuit();
      askUnsaved({
        work,
        then: askToQuit,
        kept: () => void commands.quitCancelled().catch(() => undefined),
      });
    }).catch(() => undefined);
    // Unlistening can fail too — the window may be going away under it — and a cleanup
    // that throws into nothing is an unhandled rejection, not a diagnosis.
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [askUnsaved]);

  // The app menu's Settings… (`⌘,`), which is the core's (`lifecycle.rs`) and says so with an
  // event, as its Quit does. The same verb the palette row runs.
  useEffect(() => {
    const listening = listen("settings-asked", () => windowDoes.openSettingsTab()).catch(
      () => undefined,
    );
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [windowDoes]);

  const quit = useCallback(() => {
    setAsking(false);
    void commands.quit().catch(() => undefined);
  }, []);

  /** Not now: the core is told, so the next ask warns again instead of quitting outright. */
  const dontQuit = useCallback(() => {
    setAsking(false);
    void commands.quitCancelled().catch(() => undefined);
  }, []);

  /**
   * What the window itself can do, for the surfaces that are drawn with no project in front.
   *
   * **The refusals are the point** (charter-app#111): outside a project the app stays useful
   * and refuses to start a chat, in charter's own words, rather than opening one somewhere it
   * guessed. Every other verb here belongs to a row the catalogue already marks unavailable
   * with no tabs and no plane, so it can only be reached by a surface that ignored
   * `available` — which `perform` checks again anyway.
   */
  const nowhere = (): Ran => ({
    ok: false,
    refused: "purlis has no plane open, so there is nowhere to start a chat.",
  });
  const windowDoing = useMemo<Doing>(
    () => ({
      // A refusal, not a silence: `perform` hands it back, `run` keeps it, and the palette
      // draws it beside the row that was pressed. Nothing is written to a second piece of
      // state that would then have to be cleared when a project arrives.
      newChat: () => undefined,
      newShell: () => undefined,
      newBrowser: () => undefined,
      split: () => undefined,
      closePane: () => undefined,
      closeTab: () => undefined,
      selectTab: () => undefined,
      renameTab: () => undefined,
      linkWorkItem: () => undefined,
      startFresh: () => undefined,
      restartChat: () => undefined,
      moveChat: () => undefined,
      restartListed: () => undefined,
      askPersona: () => undefined,
      useSkill: () => undefined,
      unlinkWorkItem: async () => nowhere(),
      focusWorkspace: () => undefined,
      pickClone: () => undefined,
      focusBranch: () => undefined,
      focusRepo: () => undefined,
      newBranch: () => undefined,
      cloneMissing: async () => nowhere(),
      askDropMembership: () => undefined,
      newChatIn: () => undefined,
      // Both are rows the catalogue marks unavailable with no plane — there is nowhere to make
      // a workspace and no workspace to delete — so `perform` refuses them before either of
      // these is reached. They exist because `Doing` is one shape for every surface.
      createWorkspace: () => undefined,
      removeWorkspace: () => undefined,
      showChat: () => undefined,
      showTabTasks: () => undefined,
      ownTab: () => undefined,
      beside: () => undefined,
      sendBack: () => undefined,
      showBrief: () => undefined,
      answerQuestion: () => undefined,
      // The queue is a project's, and there is no project here to have one.
      ignoreNeedsYou: async () => nowhere(),
      cancelSmartClose: async () => nowhere(),
      dismissStopped: () => undefined,
      stopChat: () => undefined,
      stopAllTasks: () => undefined,
      endTask: () => undefined,
      // A view is shown in a project's tab, and there is no project here. The rows that open one
      // do not exist without a plane, for the same reason the workspace rows above do not.
      openView: () => undefined,
      // A session record is resumed into a project, and there is no project here: its rows are
      // a project's catalogue's, and this one lists none.
      resumeSession: async () => nowhere(),
      // A memory is a project's, and its tab is shown in one: there is no project here, and no
      // memory rows in this catalogue.
      openMemory: () => undefined,
      editMemory: () => undefined,
      archiveMemory: async () => nowhere(),
      moveMemory: async () => nowhere(),
      newMemory: () => undefined,
      keepTab: () => undefined,
      // An extension's action runs in a project, and there is no project here: its rows are
      // a project's catalogue's, and this one lists none.
      runAction: async () => nowhere(),
      // No plane, no vaults: both rows are unavailable without one.
      pickVault: () => undefined,
      createVault: () => undefined,
      removeVault: () => undefined,
      // No plane, no personas and no workspace: every row these answer is unavailable.
      createPersona: () => undefined,
      removePersona: () => undefined,
      setPersonaProfile: () => undefined,
      editPersona: async () => nowhere(),
      closeTodo: async () => nowhere(),
      forgetTodo: async () => nowhere(),
      reopenTodo: async () => nowhere(),
      forgetDoneTodo: async () => nowhere(),
      // No plane, no browsers of its chats (#2022).
      pinBrowser: async () => nowhere(),
      handBrowser: async () => nowhere(),
      stopBrowser: async () => nowhere(),
      openBrowserTimeline: () => undefined,
      pinTab: async () => nowhere(),
      pinWorkspace: async () => nowhere(),
      pinProject: windowDoes.pinProject,
      removeWorktree: async () => nowhere(),
      mergeWorktree: async () => nowhere(),
      declareWorktreeDone: async () => nowhere(),
      sendKey: async () => nowhere(),
      openProject: windowDoes.openProject,
      createProject: windowDoes.createProject,
      showExtensions: windowDoes.showExtensions,
      // A side's views are a project's: with none open there is no side to show (#1673).
      showSideView: () => undefined,
      toggleRegion: () => undefined,
      installCli: windowDoes.installCli,
      selectProject: windowDoes.selectProject,
      switchProject: windowDoes.switchProject,
      closeProject: windowDoes.closeProject,
      moveProject: windowDoes.moveProject,
      openSettings: windowDoes.openSettings,
      openSaving: windowDoes.openSaving,
      // A workspace is a project's, and there is no project here to have one.
      openWorkspaceSettings: () => undefined,
      switchLive: () => undefined,
      renameWorkspace: () => undefined,
      openSettingsTab: windowDoes.openSettingsTab,
      openYourSettings: windowDoes.openYourSettings,
      // No workspace is focused with no project, so there is nothing to read again.
      readAgain: () => undefined,
      // Nor a finished task, which is a project's.
      taskBranch: () => undefined,
      // A curation chat is opened in a project, and there is no project here.
      curate: async () => nowhere(),
      quit: windowDoes.quit,
      // A file row is a project's: there is no branch here to have one.
      copyPath: async () => nowhere(),
      revealPath: async () => nowhere(),
      openInEditor: async () => nowhere(),
      shellInFolder: () => undefined,
      startChatHere: async () => nowhere(),
      addToChat: () => undefined,
    }),
    [windowDoes],
  );

  /**
   * The projects in the order the strip draws them: **pinned first** (ADR 0039).
   *
   * The same rule the chat strip follows (`tabs.tabsIn`) and for the same reason: nothing
   * moves that the operator did not move, and within each group the order is untouched.
   */
  const drawn = useMemo(
    () => [
      ...projects.filter((one) => pinnedProjects.includes(one.plane)),
      ...projects.filter((one) => !pinnedProjects.includes(one.plane)),
    ],
    [pinnedProjects, projects],
  );

  /**
   * The projects in the order the operator was last in them, the one in front first (FR-27).
   *
   * **The switcher's order, and nothing else's.** The strip's order never moves under the
   * operator's hand (ADR 0039); a switcher is a list read once, aimed at the last project, so
   * that the key and Enter is a switch back. Held for this window and never written down: it
   * is what this sitting did, and a cold launch starts it again from the project in front.
   */
  const [recent, setRecent] = useState<PlaneId[]>([]);
  useEffect(() => {
    if (inFront === undefined) return;
    setRecent((was) => (was[0] === inFront ? was : [inFront, ...was.filter((p) => p !== inFront)]));
  }, [inFront]);
  /**
   * **The projects that stay drawn while another is in front** (`projectShown.ts`): the ones the
   * person was in last, so going back to one puts its view in the window as it was left and
   * builds nothing. Bounded, so a window holding many projects keeps the views of a few.
   */
  const keptDrawn = useMemo(() => keptOf(recent, planes), [recent, planes]);
  /** Where the project in front's view is put: an element of the window's, between its notices
   *  and whatever is drawn after the projects. */
  const [projectSlot, setProjectSlot] = useState<HTMLElement | null>(null);

  /** The rows the project strip draws. The same rows `catalogue` splices into the palette —
   *  one place the words and the availability are written down (`actions.projectRows`). */
  const strip = useMemo(
    () => projectRows(drawn, inFront, pinnedProjects, split),
    [drawn, inFront, pinnedProjects, split],
  );

  /**
   * **The switcher's rows** (FR-27): the strip's own `Switch to project …` rows — so the words,
   * the reason the one in front cannot run and what a row does are written once — in the order
   * the operator was last in each, with the path as the note, because two projects can share a
   * directory name and the name is all the row shows.
   */
  /**
   * ⌘P's files (FM-7): the scope ladder from the window's focus — the branch the project in
   * front has picked, that project, every open project — and a found file opened in its own
   * project, brought to the front. None while no project is open.
   */
  const branchInFront = saying?.branch;
  const files = useMemo(
    () =>
      planes.length === 0
        ? undefined
        : {
            ladder: scopeLadder(inFront, branchInFront, planes.length),
            nameOf: calledOn,
            onOpen: (file: FoundFile) => {
              setShowing({ at: "plane", plane: file.plane });
              setFileAsk((was) => ({
                plane: file.plane,
                place: placeOf(file),
                path: file.path,
                at: (was?.at ?? 0) + 1,
              }));
            },
          },
    [branchInFront, inFront, planes.length],
  );

  const switcherRows = useMemo(() => {
    const held = drawn.map((project) => project.plane);
    const order = [
      ...recent.filter((plane) => held.includes(plane)),
      ...held.filter((plane) => !recent.includes(plane)),
    ];
    return order.map((plane) => ({ ...strip.switchTo[held.indexOf(plane)], note: plane }));
  }, [drawn, recent, strip]);

  /**
   * The project strip's rows as one list, for the context menu on a project tab.
   *
   * **The strip's own rows and not a second reading of anything.** `strip` is
   * `actions.projectRows`, which is what the tabs, the `×` and the palette all draw; the menu
   * is `menuRows` filtering this same list to the project it was opened on. So a project in
   * front has "It is already in front." on its greyed row here for the same reason its tab
   * does not react, and neither fact is written twice.
   */
  const stripOffers = useMemo(
    () => [
      strip.open,
      strip.create,
      ...strip.switchTo,
      ...strip.pin,
      ...strip.settings,
      ...strip.saving,
      ...strip.window,
      ...strip.back,
      ...strip.close,
    ],
    [strip],
  );

  /** The same rows by id, which is what a project tab's menu looks one up in. Built once for
   *  the strip rather than scanned per tab per render — `actions.catalogued` has the numbers,
   *  measured on the chat strip where fifty of them are drawn at ADR 0026's limits. */
  const stripFound = useMemo(() => catalogued(stripOffers), [stripOffers]);

  /**
   * A project tab dragged onto another (SI-6): the window's projects put in the new order, and
   * the project pinned or unpinned when it was carried across the boundary (`reorder.ts`).
   *
   * **The order is `planes`, and nothing new holds it.** The strip draws the window's projects
   * pinned first and otherwise in the order the window holds them, and that order is already
   * this machine's record of the window (`window_holds_planes`, ADR 0033) — so a dragged strip
   * is put back at the next cold launch by the record that already puts it back. A split
   * window's strip is its own window's, and is remembered as its own.
   */
  const dragProject = useCallback(
    (moved: string, onto: string) => {
      const made = afterDrop({
        whole: drawn.map((one) => one.plane),
        drawn: drawn.map((one) => one.plane),
        moved,
        onto,
        isPinned: (plane) => pinnedProjects.includes(plane),
      });
      if (made === undefined) return;
      setPlanes(made.order as PlaneId[]);
      if (made.pinned === undefined) return;
      void pinProject(moved, made.pinned, { early: true }).then((ran) => {
        if (!ran.ok) setReport({ from: "project.drag", refused: true, words: ran.refused });
      });
    },
    [drawn, pinProject, pinnedProjects],
  );
  /** How many chats in `project` need you: its tab's count, and its share of the count on the
   *  show-more button when the strip is not drawing it (ADR 0054). Its own report's queue for
   *  both, so the button goes down exactly when the tab would. */
  const askingIn = (project: Project) => reports[project.plane]?.asking.length ?? 0;

  /** The rail always shown or hiding itself (#2915, R82, R83), as the window was left. */
  const [railMode, setRailMode] = useRailMode();
  /** Below the title bar: the rail and the work, whose left edge peeks a hidden rail (R86). */
  const below = useRef<HTMLDivElement>(null);
  /** The title bar's show (R91): the rail slides back, and the keyboard goes to its project in
   *  front, the button that was pressed having gone. */
  const showRail = useCallback(() => {
    setRailMode("shown");
    requestAnimationFrame(() =>
      below.current
        ?.querySelector<HTMLElement>('[data-scope="rail"] [aria-selected="true"]')
        ?.focus({ preventScroll: true }),
    );
  }, [setRailMode]);

  /** Every action a window with no project in front can do. The project's own catalogue is
   *  `PlaneView`'s; this is the one for the opener, and its refusals are #111's. */
  const screenReader = useScreenReaderMode();
  const openerOffers = useMemo(
    () =>
      catalogue({
        tabs: noTabs(),
        workspaces: [],
        projects: drawn,
        needsYou: [],
        pinned: { chats: [], workspaces: [], projects: pinnedProjects },
        nameOf: String,
        split,
        screenReader,
      }),
    [drawn, pinnedProjects, split, screenReader],
  );

  const run = useCallback(
    async (offer: Offer): Promise<Ran> => {
      const answer = await perform(offer, windowDoing);
      setReport(saidOf(offer.id, answer));
      return answer;
    },
    [windowDoing],
  );

  const press = useCallback(
    (offer: Offer) => {
      void run(offer);
    },
    [run],
  );

  /**
   * The updater, **once for the whole window** (`Updates.tsx`, `TitleBar.tsx`).
   *
   * It used to be called inside every `PlaneView`, which made a window holding eight projects
   * hold eight updater clients for one app-wide fact — eight `update_channel` calls at open
   * and eight listeners for each `update://checked`. An offer is about the app, so it is asked
   * once, here, where the things that belong to the WINDOW live.
   */
  const updates = useUpdates();
  const titleBarRoom = useTitleBarRoom();

  /**
   * **Every project's chats asking, for the title bar's list** (charter-app#249), in the order
   * the project strip draws the projects and each project's queue in its own order.
   *
   * Out of the reports the window already holds, so the list costs no command of its own —
   * and a project behind the one on screen, which draws nothing, still reports its queue.
   */
  const needing = useMemo<Needing[]>(
    () =>
      planes.flatMap((plane) =>
        (reports[plane]?.asking ?? []).map((one) => ({ ...one, plane, project: calledOn(plane) })),
      ),
    [planes, reports],
  );

  /**
   * **The ✋ opens the Inbox** (#1692, I-2): the project in front's, where something waits there,
   * and otherwise the first project along the strip with something waiting, brought to the
   * front. With nothing waiting anywhere, the Inbox in front, which says so.
   */
  const openInbox = useCallback(() => {
    const front = inFrontNow.current;
    const waits = (plane: PlaneId) => (registry.held[plane]?.length ?? 0) > 0;
    const plane =
      front !== undefined && waits(front) ? front : (planesNow.current.find(waits) ?? front);
    if (plane === undefined) return;
    setShowing({ at: "plane", plane });
    setInboxAsk((was) => ({ plane, at: (was?.at ?? 0) + 1 }));
  }, [registry.held]);
  /**
   * **A clicked notification lands on the Inbox at its chat's group** (#1694, I-7): its project
   * brought to the front, at the first chat it counted that still has something waiting there
   * (#1989). One whose chats were all answered elsewhere since moves nothing.
   */
  const landOnNotified = useCallback(
    (to: Landing) => {
      const { plane } = to;
      if (!planesNow.current.includes(plane)) return;
      const asks = registry.held[plane] ?? [];
      const session = landsOn(to, (chat) => asks.some((ask) => ask.session === chat));
      if (session === undefined) return;
      setShowing({ at: "plane", plane });
      setInboxAsk((was) => ({ plane, at: (was?.at ?? 0) + 1, session }));
    },
    [registry.held],
  );
  useNotificationLanding(landOnNotified);
  /** The projects whose view has a close or a "Who draws it?" question up (#2761). */
  const closeQuestionsIn = useRef(new Set<PlaneId>());
  const onAsking = useCallback((plane: PlaneId, up: boolean) => {
    if (up) closeQuestionsIn.current.add(plane);
    else closeQuestionsIn.current.delete(plane);
  }, []);
  /** A link's close of chats with theme drafts (#2761): asked in that project, brought to the
   *  front, unless a question is up anywhere in this window. */
  const closeAsk = useCloseHanded(
    (plane) => planesNow.current.includes(plane),
    () => aQuestionIsUp(closeQuestionsIn.current),
    (plane) => setShowing({ at: "plane", plane }),
  );

  /**
   * **Every project's dispatches refused while nobody was there** (#1507): updates in each
   * project's Inbox (#1693), attached to no chat. Allow from now on grants the one pair for the
   * person on this machine, Never for this pair is their never, and the core's sentence is said
   * for each; Dismiss says nothing.
   */
  const awayRefusals = useAwayRefusals(planes);
  const { allow: allowAway, dismiss: dismissAway, never: neverAway } = awayRefusals;
  /** One answer for every project's view, so none is drawn again for a new one. */
  const answerAway = useCallback(
    (plane: PlaneId, item: AwayRefusal, how: "allow" | "dismiss" | "never") => {
      const answering = { allow: allowAway, dismiss: dismissAway, never: neverAway }[how];
      void answering(plane, item).then((answer) => {
        if (answer !== undefined) setReport({ from: `needs.away.${how}`, ...answer });
      });
    },
    [allowAway, dismissAway, neverAway],
  );

  /** And the chats that can be waiting without saying so, for the faint hand (charter-app#52). */
  const quiet = useMemo<Quiet[]>(
    () =>
      planes.flatMap((plane) =>
        (reports[plane]?.quiet ?? []).map(({ session, name }) => ({
          name,
          project: calledOn(plane),
          plane,
          session,
        })),
      ),
    [planes, reports],
  );

  /**
   * **What the other windows have asking and open** (charter#126). The ✋ list is every chat,
   * in every project, asking for the operator (ADR 0054) — and a project split into a window of
   * its own reports to that window, not to this one. So each window tells the others its share,
   * and draws theirs beside its own. A row a window has not caught up on — a project that has
   * just moved in here — is drawn once, from this window's own report.
   */
  const others = useOtherWindows({ needing, quiet, ending, settled });
  const everyNeeding = useMemo(
    () => [...needing, ...others.needing.filter((one) => !planes.includes(one.plane))],
    [needing, others.needing, planes],
  );
  const everyQuiet = useMemo(() => [...quiet, ...others.quiet], [quiet, others.quiet]);
  /**
   * **The hand's number: the asks, one per thing that waits** (#1690). A project the core has
   * not said its asks for yet counts its rows, as the hand always did, and so does a project in
   * another window, whose asks that window reads.
   */
  const askedCount = useMemo(() => {
    if (!planes.some((plane) => plane in registry.held)) return undefined;
    const here = planes.reduce(
      (sum, plane) =>
        sum + (registry.held[plane]?.length ?? needing.filter((one) => one.plane === plane).length),
      0,
    );
    return here + others.needing.filter((one) => !planes.includes(one.plane)).length;
  }, [planes, registry.held, needing, others.needing]);
  /** What a quit would end in every window: the main window is the one asked (`lifecycle.rs`),
   *  and a warning that left out a split window's chats would end them unannounced. */
  const everyEnding = useMemo(() => [...ending, ...others.ending], [ending, others.ending]);
  // In the same flush as the report that changed it, for the reason `PlaneView` reports in
  // one: a quit is not something the window gets to be a frame behind on. Settled only once
  // every other window has said it is: one that has said nothing yet is "not yet".
  useLayoutEffect(() => {
    atQuit.current = { settled: settled && others.settled, ending: everyEnding };
  });

  /**
   * A row off that list, carried out by the project it is about — through that project's own
   * `run`, so a Go and an Ignore are exactly the palette's rows.
   *
   * **A row that shows a chat shows its project first.** Go is "that chat, in front", and the
   * chat is only in front when its project is: the project's own `showChat` brings the tab and
   * its workspace forward, and this brings the project.
   */
  /**
   * **The project in front's save standing, for the title bar** (charter-app#294). The bar's
   * save button saves with the generated message; a refusal opens the project's Saving tab,
   * whose journal holds the refusal's own words.
   */
  // Read by the project itself and reported (charter-app#302), so the strip and the bar are
  // one reading of each project, not two.
  const saving = inFront === undefined ? undefined : reports[inFront]?.saving;
  /** The workspace in front's repos, which the indicator counts in (charter-app#299). Its save
   *  never saves them: a repo is a developer's, and pushing one is the Saving tab's Save all,
   *  asked first (ADR 0051, amended 2026-09-25). */
  const repoWorkspace = saying?.workspace;
  const repos = useRepoSaving(inFront, repoWorkspace);
  /** The project a save from the bar is running in: busy is that project's, not the window's. */
  const [savingIn, setSavingIn] = useState<PlaneId>();
  const saveInFront = useCallback(() => {
    const plane = inFrontNow.current;
    if (plane === undefined) return;
    setSavingIn(plane);
    void commands
      .savePlane(plane, null)
      .then((answer) => {
        if (answer.status === "error") windowDoes.openSaving(plane);
      })
      .finally(() => {
        setSavingIn(undefined);
        tellSaved();
      });
  }, [windowDoes]);

  const pressHere = useCallback((plane: string, offer: Offer) => {
    if (offer.does.verb === "showChat") setShowing({ at: "plane", plane });
    void reportsNow.current[plane]?.run(offer);
  }, []);
  /** A row for a chat in ANOTHER window is carried out there, and that window comes to the
   *  front (charter#126): the chat is in front only in the window holding its project. */
  const pressNeeding = useCallback(
    (plane: string, offer: Offer) => {
      if (planesNow.current.includes(plane)) pressHere(plane, offer);
      else void runElsewhere(plane, offer);
    },
    [pressHere],
  );
  useRunHere(pressHere);
  /**
   * **A press of the hand** (#1692, #1695): the Inbox of a project this window holds where
   * something waits there. Where what waits is only in another window's projects, the first of
   * those chats is brought to the front in its own window, where its Inbox lists it.
   */
  const pressHand = useCallback(() => {
    const waitsHere = planesNow.current.some(
      (plane) =>
        (registry.held[plane]?.length ?? 0) > 0 || needing.some((one) => one.plane === plane),
    );
    const elsewhere = everyNeeding.find(
      (one) => !planesNow.current.includes(one.plane) && one.go?.available,
    );
    // The Inbox in front lists a chat in another window too (`InboxElsewhere`), so the press
    // goes there unless this window has no project to open one in.
    if (!waitsHere && inFrontNow.current === undefined && elsewhere?.go)
      pressNeeding(elsewhere.plane, elsewhere.go);
    else openInbox();
  }, [everyNeeding, needing, openInbox, pressNeeding, registry.held]);
  /**
   * **What each Inbox lists that its registry cannot see** (#1695): the chats waiting in other
   * windows' projects and every chat that cannot say it waits, as the ✋'s list named them.
   */
  // **By value** (#1034's reason): every project's view is handed it, and a fresh object for
  // each report the window hears would draw every view again for a chat that only moved.
  // This window's projects too, each with how many asks wait there (#1700 item 5): each Inbox
  // names the others that wait, since the ✋ opens only one of them.
  const elsewhereSaid = JSON.stringify({
    asking: everyNeeding.filter((one) => !planes.includes(one.plane)),
    quiet: everyQuiet,
    waitingHere: planes.map((plane) => ({
      plane,
      project: calledOn(plane),
      count: registry.held[plane]?.length ?? 0,
      // What each says, cut to what is read there (qw359): the Inbox's All projects lists them.
      asks: (registry.held[plane] ?? []).map(waitingAsk),
    })),
  });
  /** Go to project (#1700 item 5): that project in front, with its Inbox open, as the ✋ does. */
  const openInboxOf = useCallback((plane: string) => {
    if (!planesNow.current.includes(plane)) return;
    setShowing({ at: "plane", plane });
    setInboxAsk((was) => ({ plane, at: (was?.at ?? 0) + 1 }));
  }, []);
  const elsewhere = useMemo<Elsewhere>(
    () => ({
      ...(JSON.parse(elsewhereSaid) as Pick<Elsewhere, "asking" | "quiet" | "waitingHere">),
      onPress: pressNeeding,
      onOpen: openInboxOf,
    }),
    [elsewhereSaid, pressNeeding, openInboxOf],
  );

  /** Every project as the title bar's switchers list it (#2916): its name, what waits in it
   *  and its workspaces, as its view reports them. */
  const switchProjects = useMemo<SwitchProject[]>(
    () =>
      drawn.map((project) => ({
        plane: project.plane,
        name: calledOn(project.plane),
        waiting: registry.held[project.plane]?.length ?? askingIn(project),
        workspaces: reports[project.plane]?.workspaces,
        cannotCreate: (() => {
          const create = reports[project.plane]?.offers.find(
            (one) => one.id === "workspace.create",
          );
          return create === undefined || create.available ? undefined : create.reason;
        })(),
      })),
    // `calledOn` and `askingIn` read `drawn`, `reports` and the registry, named here.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [drawn, registry.held, reports],
  );
  /** What the title bar's switchers do (#2916): each is a row of the project's own catalogue,
   *  run by the project, so the words, the refusals and the pins are the catalogue's. */
  const switchDoes = useMemo<SwitchDoes>(() => {
    const runOf = (
      plane: string,
      id: string,
      fits?: (offer: Offer) => boolean,
      first?: () => void,
    ) => {
      const report = reports[plane];
      const offer = report?.offers.find((one) => one.id === id);
      if (report === undefined || offer === undefined || !offer.available) return;
      if (fits !== undefined && !fits(offer)) return;
      // What goes with the row, only once it is sure to run (a step's slide).
      first?.();
      void report.run(offer);
    };
    return {
      go: (plane, workspace, step) => {
        if (plane === inFront)
          return runOf(plane, `workspace.focus:${workspace}`, undefined, () => {
            if (step !== undefined) slideTowards(step);
          });
        // Another project: brought to the front first, and its workspace focused once it is
        // there and has put back what it had open, which would otherwise take the front back.
        windowDoes.selectProject(plane);
        setGoingTo({ plane, workspace, from: inFront });
      },
      choose: (plane) => windowDoes.selectProject(plane),
      pin: (plane, workspace, pinned) => {
        if (plane === inFront)
          return runOf(
            plane,
            `workspace.pin:${workspace}`,
            (offer) => offer.does.verb === "pinWorkspace" && offer.does.pinned === pinned,
          );
        // A project behind the one in front builds no catalogue: its pin is written to the
        // store, and its view reads its pins again (`workspacePins.ts`).
        void commands
          .pinWorkspace(plane, workspace, pinned)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
          .then((said) => {
            if (said.status === "error")
              setReport({ from: "workspace.pin", refused: true, words: said.error });
            pinsChanged(plane);
            machineChanged();
          });
      },
      create: (plane) => {
        if (plane !== inFront) windowDoes.selectProject(plane);
        runOf(plane, "workspace.create");
      },
    };
  }, [inFront, reports, windowDoes]);
  /** A workspace of another project the switcher went to, focused once that project is in
   *  front and settled (`switchDoes.go`). */
  const [goingTo, setGoingTo] = useState<{
    plane: string;
    workspace: string;
    /** The project in front when it was asked: the one the window is leaving. */
    from: string | undefined;
  }>();
  // Forgotten once the person goes somewhere else before it is reached.
  if (goingTo !== undefined && inFront !== goingTo.plane && inFront !== goingTo.from)
    setGoingTo(undefined);
  useEffect(() => {
    if (goingTo === undefined || goingTo.plane !== inFront) return;
    const report = reports[goingTo.plane];
    // Its catalogue is built once it is in front: until its focus row is there, it waits.
    const offer = report?.offers.find((one) => one.id === `workspace.focus:${goingTo.workspace}`);
    if (report?.settled !== true || offer === undefined) return;
    setGoingTo(undefined);
    if (offer.available) void report.run(offer);
  }, [goingTo, inFront, reports]);
  /** The project in front's rows by id, for a chip's context menu: the workspace's own rows. */
  const inFrontFound = useMemo(
    () => catalogued(inFront === undefined ? [] : (reports[inFront]?.offers ?? [])),
    [inFront, reports],
  );
  const workspaceMenu = useCallback(
    (workspace: string, chip: ReactNode) =>
      inFront === undefined ? (
        chip
      ) : (
        <Menued
          // The plane root's chip takes the root's menu (#3654), which has no pin to let go.
          on={workspace === OUTSIDE ? { on: "root" } : { on: "workspace", workspace }}
          offers={inFrontFound}
          onPress={(offer) => void reports[inFront]?.run(offer)}
        >
          {chip}
        </Menued>
      ),
    [inFront, inFrontFound, reports],
  );

  return (
    <main
      className="window"
      {...{
        [FOCUS_MODE]: focusMode.on ? "" : undefined,
        [FOCUS_BACK]: focusMode.back ? "" : undefined,
      }}
    >
      {/* The window's own title bar (ADR 0054): on macOS it IS the title bar — the system's
          traffic lights float over it — and on every other platform it is the window's first
          row under the system's own bar. `TitleBar.tsx` argues the shape and the drag region.
          The projects are the rail at the window's left now (#2915); while the rail hides
          itself, the bar's first control shows it again (R91). */}
      <TitleBar
        leading={
          planes.length > 0 ? (
            <>
              {railMode === "auto" && (
                <IconButton
                  variant="ghost"
                  size="md"
                  aria-label="Show the projects sidebar"
                  icon={<SidebarIcon />}
                  data-rail-show=""
                  onPress={showRail}
                />
              )}
              {/* The project in front and its workspaces (#2916): its name, a slash and its pins
                  as chips; with the rail hidden, the name is the switcher of every project. */}
              {inFront !== undefined && (
                <TitleBarSwitch
                  projects={switchProjects}
                  inFront={inFront}
                  railShown={railMode !== "auto"}
                  held={paletteOpen}
                  does={switchDoes}
                  menuOf={workspaceMenu}
                  notices={chipNotices}
                />
              )}
            </>
          ) : undefined
        }
        updates={updates}
        room={titleBarRoom}
        // The level of the chat in front of the project in front (#2071).
        sandbox={inFront !== undefined ? reports[inFront]?.sandbox : undefined}
        chats={ending}
        needing={{
          count: askedCount ?? everyNeeding.length,
          chats: askedCount === undefined,
          quiet: everyQuiet,
          onInbox: pressHand,
        }}
        save={
          inFront !== undefined && saving !== undefined
            ? {
                saving,
                repos,
                busy: savingIn === inFront,
                onOpen: () => windowDoes.openSaving(inFront),
                onSave: saveInFront,
                // The incoming count's press: the Saving tab opens and runs it (qw348).
                onUpdate: () => {
                  askUpdate(inFront);
                  windowDoes.openSaving(inFront);
                },
                // A read-only project's save is refused by the core: off, with why (#826).
                off: reports[inFront]?.readOnly,
              }
            : undefined
        }
      />
      {/* **Below the title bar**: the project rail at the far left (#2915, R76), and the work
          beside it, which glides as the rail hides and shows (R84). Its left edge is where the
          pointer peeks a hidden rail (R86). */}
      <div className="window-below" ref={below}>
        {planes.length > 0 && (
          <ProjectRail
            projects={drawn}
            inFront={inFront}
            rows={strip}
            offers={stripFound}
            onPress={press}
            waitingIn={(project) => registry.held[project.plane]?.length ?? askingIn(project)}
            unsavedIn={(project) => unsavedOf(reports[project.plane]?.saving)}
            onDrop={dragProject}
            lettingGo={lettingGo}
            mode={railMode}
            onMode={setRailMode}
            edge={below}
            nameOf={calledOn}
          />
        )}
        <div className="window-work">
          {/* **No result is in the page's flow while a project is in front** (#3648, #3653): one
          about a workspace hangs from its chip, and a project's or the app's pops from the status
          bar's Notices item; each is listed in the Notices centre too. With no project in front
          the window has no status bar and no Notices centre, so its result stands here, at the
          top of the opener's page, as the window's own lines do (D-LB-1). */}
          {said && said !== closedSaid && !paletteOpen && inFront === undefined && (
            <ActionResultNotice
              cause={`result:${said.from}`}
              said={said}
              onClose={() => {
                setClosedSaid(said);
                // Closed here, it does not pop on the status bar of the project that comes next
                // (#3653's review).
                hideNotices();
              }}
              onSettings={followSaid}
            />
          )}
          {resultsAt !== null && inFront !== undefined && (
            <ResultsGroup
              kept={resultsInFront}
              into={resultsAt}
              onDrop={dropResult}
              onSettings={followSaid}
              // **No chip, so listed here alone, with a way there** (#3651, D-3648-16): a
              // result about a workspace the title bar draws no chip for offers "Open X",
              // while the project has that workspace.
              openOf={(one) => {
                const place = placeOfSaid(one, chipsInFront);
                if (place.at !== "centre") return undefined;
                // Only to a workspace the project still has: one removed or renamed since has
                // no way there, and a way that does nothing is no way (#3651's review).
                if (workspacesInFront?.names.includes(place.open) !== true) return undefined;
                return {
                  label: `Open ${place.open}`,
                  onPress: () => switchDoes.go(inFront, place.open),
                };
              }}
            />
          )}

          {/* **The window's own lines, one list** (D-LB-1): what the window says about itself
          rather than about a project (a slow start, the session bus, vaults left waiting, a
          project gone, an entry the store would not take back). With no project in front they
          are listed at the top of the opener's page, with the opener's own lines, the most
          important first; a window with no project has no Inbox to list them. While a project
          is in front they are listed at the top of that project's Inbox's Notices, and counted
          in its status line (#1695); only until its Inbox is first drawn do they stand under
          the title bar. Written once here, wherever they are drawn, so a
          dismissal or the session bus's answer is never lost when a project comes or goes. */}
          <NoticeList
            into={
              openerUp && !settingsAlone
                ? windowLinesAt
                : inFront !== undefined
                  ? (inboxLinesAt ?? undefined)
                  : undefined
            }
            onCount={onWindowLines}
            at={
              openerUp && !settingsAlone
                ? "page"
                : inFront !== undefined && inboxLinesAt != null
                  ? "lines"
                  : "band"
            }
          >
            {/* A launch nobody could see. The core only answers here when the start passed the
            spec's limit, so on an ordinary launch there is nothing to draw and nothing to
            dismiss. It is the one place an operator who clicked an icon can be told — the
            line purlis writes while it waits goes to standard error, which they do not have.
            Dismissible, because the launch is over and the news does not improve. Where the
            core knows the relaunch that avoids the wait, it is offered as Copy command (NO-4):
            the window has no fix for a launch already made, so this is V91q's last resort and
            `Notice.guard.test.ts` lists it as debt. */}
            {slowStart && (
              <Notice
                cause="slow-start"
                tone="trouble"
                copy={slowStart.relaunch ?? undefined}
                onDismiss={() => setSlowStart(undefined)}
              >
                {slowStart.said}
              </Notice>
            )}

            {/* A launch without the session bus, and what that run has not got (charter#746). */}
            <SessionBusNotice chats={ending} />
            {/* Browsers that cannot start for the engine's sandbox, and the ways out (#2835). */}
            <EngineDownloadNotice />
            <EngineSandboxNotice />

            {/* The session host stopped unexpectedly three times in five minutes, and the window's
            supervisor stopped starting it again: Try again (#2007). */}
            <HostFailingNotice />

            {/* Vaults the launch left under the old name because macOS would have asked (#1306). */}
            <VaultsWaitingNotice />

            {/* A project the last quit had open that has moved or gone, with Locate… and Forget
            (NO-5). A line, never an error dialog: the record is a convenience and the project
            is the truth (ADR 0033). Said up here, because the window may well have come back on
            another project; when the opener is drawn and lists the same project, the opener's
            line is the one, and settling it there settles it here. */}
            {goneAtLaunch
              .filter((gone) => !goneDismissed.has(gone.path))
              .filter(
                (gone) =>
                  !(openerUp && !settingsAlone) ||
                  (openerGone !== "unread" && !openerGone.includes(gone.path)),
              )
              .map((gone) => (
                <GoneProjectNotice
                  key={gone.path}
                  gone={gone}
                  cause={goneCause(gone.path)}
                  onLocated={(found) => {
                    goneSettled(gone.path);
                    letGoneGo(gone.path);
                    setGoneChanged((n) => n + 1);
                    void openInto(found, true);
                  }}
                  onForgotten={() => {
                    goneSettled(gone.path);
                    letGoneGo(gone.path);
                    setGoneChanged((n) => n + 1);
                  }}
                  onDismiss={() => {
                    dismissGone(gone.path);
                    keepGoneDismissed(gone.path);
                    goneSettled(gone.path);
                  }}
                />
              ))}
            {/* A browser sign-in a forge ended, with Sign in again (#729). */}
            <ForgeSignInEnded />
            {/* Add a pool… (#2679): a dialog, opened from the palette or the Pools group. */}
            <AddDockerPoolHost plane={inFront} />
            {/* An entry the store itself would not take back, said the same way. */}
            {notRestored.map((line) => (
              <Notice
                key={line}
                cause={`not-restored:${line}`}
                onDismiss={() => setNotRestored((was) => was.filter((one) => one !== line))}
              >
                {line}
              </Notice>
            ))}

            {/* No project in front: the welcome, and nothing else (#1812). "No sessions" would be
            true and useless — there is nowhere to open one, and the thing the operator needs is
            the way to give the window a project.
            **Not before the core has answered, and not while the restore is still opening
            projects.** Both are about to decide whether this window has one, and a welcome that
            flashed up in between would be purlis telling a newcomer there is nothing here half
            a second before eight projects arrive.
            Inside the window's list, so the welcome's own lines are listed with the window's. */}
            {openerUp && !settingsAlone && (
              <div className="body">
                <div className="panes">
                  <div className="window-notices-at" ref={setWindowLinesAt} />
                  <Onboarding
                    steps={steps}
                    gate={gitGate}
                    holdsMade={steps.holding !== undefined && planes.includes(steps.holding)}
                    reopen={(path) => void openInto(path, true)}
                    here={!heldSomething && (launch?.here ?? false)}
                    reason={launch?.reason ?? ""}
                    adding={planes.length > 0}
                    onOpen={(path) => void openInto(path, true)}
                    onGoneListed={setOpenerGone}
                    onGoneSettled={goneSettled}
                    goneChanged={goneChanged}
                    trouble={openTrouble}
                    // Create a project's act, on every window: a repo into this machine's project
                    // (FR-4), laid out from the template picked beside it (FR-17).
                    onOpenRepo={firstRunRepo}
                    onNewProject={windowDoes.createProject}
                    onSignInToForge={(row) => {
                      setRepoTrouble(undefined);
                      void signInToForge(row).then(setRepoTrouble);
                    }}
                    openingRepo={openingRepo}
                    repoTrouble={repoTrouble}
                    repoForgeAsk={repoForgeAsk}
                  />
                </div>
              </div>
            )}
          </NoticeList>

          {/* Every project this window holds. Only the one in front is in the window; the ones the
          person was in last stay drawn off it (`projectShown.ts`), and the rest keep their tabs,
          their splits and their chat states and draw nothing at all. */}
          <div className="project-views" ref={setProjectSlot} />
          {planes.map((plane) => (
            <PlaneView
              key={plane}
              plane={plane}
              inFront={plane === inFront}
              kept={keptDrawn.has(plane)}
              slot={projectSlot}
              projects={drawn}
              pinnedProjects={pinnedProjects}
              window={windowDoes}
              onReport={onReport}
              onSessionRows={onSessionRows}
              alerts={alerts}
              statusNotices={plane === inFront ? statusNotices : undefined}
              contributed={contributedPanels}
              views={extensionViews}
              commands={extensionCommands}
              settingsAsked={settingsAsk?.plane === plane ? settingsAsk.at : undefined}
              savingAsked={savingAsk?.plane === plane ? savingAsk.at : undefined}
              licencesAsked={licencesAsk?.plane === plane ? licencesAsk.at : undefined}
              focusMode={focusMode.on}
              settingsTabAsked={settingsTabAsk?.plane === plane ? settingsTabAsk.at : undefined}
              settingsLinkAsked={settingsLinkAsk?.plane === plane ? settingsLinkAsk : undefined}
              yourSettingsAsked={yourSettingsAsk?.plane === plane ? yourSettingsAsk.at : undefined}
              firstChatAsked={firstChat?.plane === plane ? firstChat : undefined}
              shellAsked={shellAsk?.plane === plane ? shellAsk : undefined}
              fileAsked={fileAsk?.plane === plane ? fileAsk : undefined}
              closeAsked={closeAsk?.plane === plane ? closeAsk : undefined}
              onAsking={onAsking}
              awayRefused={awayRefusals.held[plane]}
              onAway={answerAway}
              waiting={registry.held[plane]}
              inboxAsked={inboxAsk?.plane === plane ? inboxAsk.at : undefined}
              inboxGroup={inboxAsk?.plane === plane ? inboxAsk.session : undefined}
              elsewhere={elsewhere}
              windowLines={plane === inFront ? windowLines : undefined}
              paletteOpen={paletteOpen}
            />
          ))}

          {/* Settings drawn where the opener is, with no project open (#1206). */}
          {openerUp && settingsAlone && (
            <div className="body">
              <div className="panes">
                <section className="view-pane" aria-label={SETTINGS_TAB_TITLE}>
                  <header className="view-head">
                    <h2>{SETTINGS_TAB_TITLE}</h2>
                    <Button onPress={() => setSettingsAlone(false)}>Done</Button>
                  </header>
                  <div className="view-body">
                    <SettingsTab />
                  </div>
                </section>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* What opening a project puts in force, and the question about it (ADR 0035).
          Nothing has been attached and nothing has been started while this is up: cancelling
          leaves the window exactly as it was. One at a time, oldest first. */}
      {approving[0] && (
        <ApprovePlane
          ask={approving[0]}
          onApprove={(ask) => void approveProject(ask)}
          onCancel={() =>
            setApproving((queue) => queue.filter((q) => q.path !== approving[0].path))
          }
        />
      )}

      {/* Making a project: a plane charter scaffolds, opened through the gate like any other
          (ADR 0035, spec decision 27). The window's, like the opener — what it ends in
          is a project this window holds — and mounted only while it is up. */}
      {creating && (
        <NewProject
          trouble={createTrouble}
          forgeAsk={createForgeAsk}
          onEdit={() => setCreateForgeAsk(undefined)}
          making={makingProject}
          onCreate={(path, planeIsThisRepo, adopt) =>
            void makeProject(path, planeIsThisRepo, adopt)
          }
          onOpenRepo={(path) => dialogRepo(path)}
          opening={openingRepo}
          onCancel={() => {
            setCreating(false);
            setCreateTrouble(undefined);
            setCreateForgeAsk(undefined);
          }}
        />
      )}

      {/* The primary input (spec decision 1), and there is exactly one of it.
          **Always mounted**, because what opens it is a keystroke it listens for itself, on
          the window, capture-phase — a palette the window had to decide to render would be
          one the operator could not reach from inside a pane's terminal, and one that waited
          for the core to answer would swallow the first `F2` of every launch.
          **Once**, because that listener claims `F2` from the whole window: a second palette
          mounted behind a project tab would open two on one keypress.
          What it lists is the project in front's own catalogue, which travels up with the
          rest of that project's report, and the window's own when there is none — whose
          refusals are #111's. */}
      <Palette
        offers={paletteOffers ?? openerOffers}
        projects={{ rows: switcherRows, asked: switcherAsk }}
        files={files}
        said={said}
        onSettings={followSaid}
        onRun={saying?.run ?? run}
        onOpened={setPaletteOpen}
      />

      {/* What has contributed what to this window (ADR 0041 item 5). Mounted only
          while it is asked for: it reads every installed extension's files to re-take its
          fingerprint, and a launch does not pay for that unless somebody looked. */}
      {extensions && <Extensions onClose={() => setExtensions(false)} />}

      {asking && <QuitWarning chats={everyEnding} onQuit={quit} onCancel={dontQuit} />}

      {/* The one question a close asks about a view's unsaved work (#3929): a project's, a
          quit's. */}
      {unsavedAsk && <UnsavedWorkQuestion asked={unsavedAsk} onAnswered={unsavedAnswered} />}

      {closing !== undefined && (
        <ClosingProject
          name={calledOn(closing)}
          chats={reports[closing]?.ending ?? []}
          heard={reports[closing]?.settled ?? false}
          onClose={() => {
            const plane = closing;
            setClosing(undefined);
            void letGoOf(plane).then((answer) =>
              setReport(
                answer.ok
                  ? answer.said
                    ? { from: `project.close:${plane}`, refused: false, words: answer.said }
                    : undefined
                  : { from: `project.close:${plane}`, refused: true, words: answer.refused },
              ),
            );
          }}
          onCancel={() => setClosing(undefined)}
        />
      )}

      {relaunchAsking && (
        <RelaunchAsk
          question={relaunchAsking.question}
          nameOf={calledOn}
          onAnswer={relaunchAsking.answer}
        />
      )}
    </main>
  );
}

/**
 * What a project is called on its tab: the directory's own name.
 *
 * Two projects can share one, so the tab carries the whole path as its tooltip and the strip is
 * never the only way to tell them apart. Both separators, because a `PlaneId` is a root as the
 * operating system spells it and Windows spells it with backslashes.
 */
function calledOn(plane: string): string {
  const parts = plane.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? plane;
}

/**
 * Settings drawn where the opener is, with no project open: a way into it, so the keyboard
 * goes there too (#1206, `settings/entering.ts`).
 */
function showSettingsAlone(show: (alone: boolean) => void): void {
  show(true);
  enterSettings(settingsPlace("you"));
}

export default App;
