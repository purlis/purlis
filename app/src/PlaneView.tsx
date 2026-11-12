// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { SigningSetupAsked } from "./SigningRefusal";
import {
  memo,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ComponentProps,
  type CSSProperties,
  type ReactNode,
} from "react";
import { dequal } from "dequal/lite";
import { emit } from "@tauri-apps/api/event";
import { listen } from "./here";
import {
  CHAT_RESTORED,
  isRestoring,
  RESTORE_LOOKS_EVERY,
  restoredSince,
  seenFirst,
  type ChatRestored,
  type RestoreSaid,
  type RestoreSeen,
} from "./restoringChats";
import { QueueRead } from "./QueueRead";
import { QuestionsHere } from "./QuestionsHere";
import { QuizPlaces } from "./QuizBand";
import { standingOf, waitingOnYou, type QuestionsHeld } from "./taskQuestions";
import { machineChanged } from "./settings/thisMachine";
import { MAIN, thisWindow } from "./windows";
import { clearedHere, keptOutside, letGoHere } from "./notesClearedEverywhere";
import { frontTabIs } from "./slowLog";
import { createPortal } from "react-dom";
import {
  AsLastShown,
  InTheWindow,
  OnlyInFront,
  ProjectShownProvider,
  useProjectHost,
} from "./projectShown";
import {
  commands,
  PROJECT_READY,
  type AtRisk,
  type AwayRefusal,
  type ByHand,
  type ChatSkills,
  type HarnessKind,
  type ChatWorktree,
  type HarnessGlance,
  type HarnessSetupFound,
  type NeedsApproval,
  type NotStarted,
  type DoctorFix,
  type FinishedTask,
  type LaidOut,
  type OpenChat,
  type OwnWork,
  type StudioLink,
  type PlaneId,
  type LimitRefused,
  type PoolLimitOffer,
  type OpenRun,
  type Refused,
  type RunsOnPick,
  type Sidebar as SidebarModel,
  type StartOptions,
  type StartSecretAsk as StartSecretAskNamed,
  type ViewTab,
  type WindowDraft,
  type ArtifactFound,
} from "./bindings";
import {
  catalogue,
  catalogued,
  endId,
  freshId,
  ignoreId,
  stopId,
  taskStopId,
  needsYouRows,
  paneCloseOf,
  OUTSIDE,
  OUTSIDE_TITLE,
  pinMoved,
  perform,
  showId,
  stoppedRows,
  taskEndIds,
  stopAllId,
  withQueue,
  PASS_THROUGH_BYTES,
  PASS_THROUGH_KEY,
  FOCUS_MODE_ID,
  SHORTCUTS_ID,
  saidOf,
  type BranchPath,
  type FileRef,
  type Clone,
  type Cut,
  type Doing,
  type MovedFrom,
  type Offer,
  type Project,
  type Ran,
  type BrowserOffered,
  type Said,
  type TaskEndWay,
  type ViewPlaced,
} from "./actions";
import { CHOOSE_EDITOR, NO_EDITOR, yourEditor } from "./yourEditor";
import { LICENCES_TITLE } from "./LicencesView";
import {
  FIX_STARTED,
  OPEN_SAVING,
  usePlaneSaving,
  useRepoSavingKept,
  WAY_OUT,
  type FixStarted,
  type WayOut,
} from "./saving";
import { HARNESS_SETUP, type HarnessSetupAsk } from "./harnessSetup";
import { curationSubjects, useCurations } from "./curations";
import { LiveDialog } from "./LiveDialog";
import { slideIn, takeSlide, wayOf } from "./workspaceSlide";
import { onPinsChanged } from "./workspacePins";
import { RemoveFromWorkspace } from "./RemoveFromWorkspace";
import { DeleteWorkspace } from "./DeleteWorkspace";
import { Menued } from "./Menus";
import { afterDrop, reslotted } from "./reorder";
import { stripAccessibility, useStripSensors } from "./sortable";
import { NewWorkspace } from "./NewWorkspace";
import { FORGE_LOGIN, typableLogin, type LoginAsk } from "./RepoPicker";
import { NewBranch } from "./NewBranch";
import { LinkWorkItem } from "./LinkWorkItem";
import { RenameChat } from "./RenameChat";
import { RenameTab } from "./RenameTab";
import { chatRenamed, nameTab, renameOf } from "./tabRenaming";
import { useWorkspaceWrites } from "./WorkspaceWrites";
import { RenameWorkspace } from "./RenameWorkspace";
import { AddVision } from "./AddVision";
import { failedNotNamed } from "./NotCloned";
import {
  cloneRepos,
  forgetEndedClones,
  forgetFailedClone,
  forgetLandedClones,
  settleRepoClones,
  stopRepoClones,
  useRepoClones,
} from "./repoClones";
import {
  StartChat,
  pickerAnswerUnsent,
  refusedSaid,
  typedIn,
  type RunsOnInPicker,
} from "./StartChat";
import { withDetails } from "./startDetails";
import {
  handedSaid,
  handReference,
  ReferenceChats,
  type ChatsForReferences,
  type Referenced,
} from "./references";
import {
  ChatSkillSetsHere,
  UseSkill,
  UseSkillOpener,
  useChatSkillSets,
  type ChatSkillSets,
  type OpenUseSkill,
} from "./UseSkill";
import { Explorer, type SearchKeyHere, type Spot } from "./Explorer";
import { keptChanged } from "./branchFolders";
import {
  isPlaneRoot,
  outsideFileTitle,
  outsideFileView,
  pieceFileTitle,
  pieceFileView,
  pieceFilesTitle,
  pieceFilesView,
  workspaceFiles,
  type Place,
} from "./pieceViews";
import { useOutsideFileAsks } from "./paneLinkFiles";
import { searchFromFocus, searchOf, searchView } from "./contentSearch";
import { ChangesView, reposGap, uncommitted } from "./ChangesView";
import { SearchTab } from "./SearchTab";
import { readRefusedIn, type WorkspaceState } from "./workspaceState";
import {
  GIT_SIDE,
  sameFor,
  useWorkspacePanels,
  useWorkspacePanelsStore,
  WorkspaceRegion,
  type PanelsField,
  type WorkspacePanels,
} from "./workspacePanels";
import { useTaskBranchActs } from "./taskBranchActs";
import {
  heardFrom,
  lostOnResume,
  sessionTitle,
  sessionView,
  usePlaneRootPanels,
  type Resuming,
} from "./sessions";
import { useExtensionFacts } from "./extensionFacts";
import {
  CURATIONS,
  INSTRUCTIONS,
  ROOT_PANELS,
  SETTINGS,
  SIDEBAR,
  skillSetsOfChats,
  usePlaneChanged,
  useViewChanged,
} from "./planeChanged";
import { useRemoveWorkspaceAsked } from "./removeWorkspaceAsk";
import { useOpenProposalAsked } from "./proposedChangeOpen";
import { useOpenRunnerWriteAsked } from "./runnerWriteOpen";
import { useOpenRunnerDispatchAsked } from "./runnerDispatchOpen";
import { BROWSER_ASK_ENDED, useOpenBrowserAsked } from "./browserNeedsYou";
import { usePlaneUpdated } from "./PlaneUpdated";
import { useHarnessCards } from "./harnessCards";
import { useEditorTabs, type EditorTabOpened } from "./editorTab";
import { Notice, NoticeInCentre, NoticeOf, type NoticesDot } from "./Notice";
import { ReadOnlyLine, ReadOnlyMendable, ReadOnlySaid, useReadOnly } from "./ReadOnlyLine";
import { LivesHere } from "./LivesHere";
import { InboxAlerts, type WindowAlerts } from "./InboxAlerts";
import { handCount, handSaid, theirAsks, type Elsewhere } from "./InboxElsewhere";
import type { AllChatsFilter } from "./InboxAsks";
import { useAsksArrival } from "./asksArrival";
import { sayAboutThisMachine, useAboutThisMachine } from "./windowprefs";
import { drawWhatIsInForce } from "./Extensions";
import { useScreenReaderMode } from "./screenReader";
import { PersonaMarks, ReloadPersonaMarks } from "./PersonaMark";
import { usePersonaMarks } from "./usePersonaMarks";
import { taskChangesTitle, taskChangesView } from "./taskChanges";
import { heldFor as blockHeldFor, type HeldBlock, useSandboxBlocks } from "./sandboxBlocks";
import { taskBlockGroups, whoseOf, withoutGrouped, type TaskBlockGroup } from "./taskAsks";
import { TaskBlocksAnswered, TaskBlocksNotice } from "./TaskBlocksNotice";
import {
  chatArrived,
  clonesRefused,
  reposCloned,
  reposCloning,
  reposNotCloned,
  reposStopped,
  workspaceMade,
} from "./actionResult";
import {
  heldUpdate,
  Inbox,
  landOnGroupIn,
  NEEDS_A_DECISION_DOT,
  NEEDS_FIXING,
  type UpdateRow,
  type WindowLines,
} from "./Inbox";
import { useDismissHold } from "./dismissHold";
import { awayRow } from "./AwayRefusals";
import {
  LIVE,
  awayUpdateKey,
  awayUpdates,
  browserAskUpdates,
  seenAt,
  reasonUpdates,
  arrivedUpdates,
  reportSentKey,
  reportSentUpdates,
  resumeUpdates,
  sandboxUpdates,
  smartCloseKey,
  smartCloseUpdates,
  stuckUpdates,
  taskUpdates,
  useInboxUpdates,
  unreadMine,
} from "./inboxUpdates";
import { useInboxOpenTold } from "./askNotices";
import { ReportDraftNotices, undoAlwaysSend, type SentDraft } from "./ReportDraftNotices";
import { useAnsweredWhenGone } from "./inboxRules";
import { usePermissionAsks } from "./permissionAsks";
import { useDismissals } from "./dismissals";
import { dormantCause, dormantSays, type ChipNotices } from "./ChipNotices";
import {
  ATTENTION_VIEWS,
  openView as viewOpenIn,
  placesOf,
  useArrangement,
  type Toggled,
  type ViewId,
} from "./regions";
import { drawnOnlyPresent, previewsDrawn, type ViewPlace } from "./sideViews";
import { SkillsView, type FrontChat, type SkillsViewProps } from "./SkillsView";
import type { ExplorerPlace } from "./skillsListing";
import { SIDE_KEYS_SAID, sideKeyOf } from "./sideKeys";
import { beginTriage, firstLead, triaging } from "./asksTriage";
import { RegionFrame, stepRegions } from "./RegionFrame";
import { putAway } from "./regionsAway";
import { ActivityCount, ActivityDot, type PanelTab } from "./ActivityBar";
import { DOCTOR_LINE, DoctorNotices, useDoctor } from "./Doctor";
import { useFirstOpen } from "./firstOpen";
import { StaleCloneNotices, useStaleClones } from "./StaleCloneNotices";
import { CloneVouchNotices } from "./CloneVouchNotice";
import { BrowsersAtQuitNotice } from "./BrowsersAtQuitNotice";
import { usePin } from "./Updates";
import { projectName } from "./projectName";
import { StatusLine, runningIn, todoCount, type Alerts } from "./StatusLine";
import {
  closeChat,
  closeFocusedPane,
  closePane as closePaneOfTab,
  closeLayout,
  closeTab,
  otherTabsToClose,
  focusPane,
  chatNamed,
  dropEmptyTabsOfGoneWorkspaces,
  newChatNumber,
  noTabs,
  chatNameOf,
  chatInFrontOf,
  chatOf,
  contentsOf,
  hasOnePane,
  findView,
  openViewBesidePane,
  openChatsKey,
  openEmptyTab,
  openNew,
  opensAt,
  wouldLeaveStrip,
  openView,
  offerView,
  harnessSetupTitle,
  harnessSetupView,
  repoInstructionsTitle,
  repoInstructionsView,
  firstTaskTitle,
  firstTaskView,
  panesOf,
  putViewBack,
  replaceSession,
  setSplit,
  showInstead,
  splitOf,
  refileViews,
  followRename,
  SAVING_TITLE,
  POOL_SETUP_TITLE,
  POOL_SETUP_VIEW,
  SAVING_VIEW,
  LICENCES_VIEW,
  SETTINGS_TAB_TITLE,
  settingsView,
  viewNamedNow,
  workspaceSettingsTitle,
  workspaceSettingsView,
  viewKey,
  pinsOf,
  selectTab,
  setPinned,
  showWorkspace,
  splitFocusedPane,
  stopWaiting,
  tabsIn,
  workspaceOf,
  filedElsewhere,
  type Direction,
  type FiledIn,
  focusedChat,
  focusedContent,
  homeOf,
  layoutShown,
  type Content,
  moveToOwnTab,
  openBeside,
  panesMovedInto,
  sendBack,
  showOwn,
  showOwnIn,
  shownIn,
  shownLive,
  taskShownIn,
  back,
  noteUsed,
  press as pressChat,
  openOneJob,
  oneJobEnded,
  shellExited,
  shownBeside,
  type PaneAt,
  type Press,
  type PressedChat,
  type Pressing,
  type AskedBy,
  type Pinned,
  type Tabs,
  type TabName,
  type ViewRef,
  returnsTo,
  namedForFirstPanes,
  tabNameOf,
} from "./tabs";
import { ArtifactGroup } from "./ArtifactGroup";
import { PinPage } from "./PinToWorkspace";
import { ProposedPins } from "./ProposedPin";
import { artifactOf, artifactView } from "./artifacts";
import { tileAll, zoomedLayout, zoomFocusedPane } from "./tiling";
import { recorded } from "./layoutUndo";
import { heardOffers, offersInForce, viewOpened } from "./offerStore";
import { OfferNotices, type OfferOpening, OfferOpeningContext } from "./ViewOpen";
import { paneSize } from "./paneSize";
import { besideOf as viewBesideOf, besideRefusedSaid } from "./viewBeside";
import { useArtifactDiscuss } from "./artifactDiscuss";
import { layoutOf, pathTo, type Reopening, tabsOf, withShare } from "./laidOut";
import {
  chatsElsewhere,
  LAYOUT_MOVED,
  LAYOUT_REPORTED,
  type LayoutMoved,
  movedBetweenWindows,
  NOTE_CLEARED,
  type NoteCleared,
  NOTES_ASKED,
  NOTES_HANDED,
  type NotesAsked,
  type NotesHanded,
  withStartHanded,
  PRESS_HERE,
  type PressHere,
  pressInProjectWindow,
  type HandedChat,
  type StartedAs,
} from "./tabWindows";
import type { TabFacts } from "./ChatRows";
import { rowFactsOf } from "./shownState";
import {
  BROWSER_TITLE,
  browserView,
  goWhenShown,
  browsersLetGo,
  browsersShown,
  chatBrowserTitle,
  isBrowser,
  openBrowserBeside,
} from "./browsers";
import { sendToDriven } from "./browserDriving";
import { programsOf, useBrowsersTakenBack, useChatBrowsers } from "./chatBrowsers";
import { browserTimelineTitle, browserTimelineView } from "./browserTimelines";
import { onInspect, openInspector } from "./inspector";
import { PaneBrowsers, type BrowserShown } from "./ChatBrowsersShown";
import { SkillRunChip } from "./SkillRunChip";
import {
  forgetProjectsBrowsers,
  tellProjectBrowsers,
  tileTitle,
  type GridTile,
} from "./browsersGrid";

/** Which window action a trouble line is about (NO-4): the picker's options, a shell, Needs
 *  you, or a browser (#2012). */
type TroubleFrom = "start-options" | "shell" | "needs-you" | "browser";

/** **A chat ended on purpose in its pane** (#5019), as the window held it when it ended: its
 *  name, and the record a Smart close wrote, which is its way back. */
type EndedHere = EndedChat & { workspace: string };
import {
  askedByOf,
  chatsOfPane,
  chatsOfChat,
  chatsOfTabs,
  crumbsOf,
  hiddenNeeding,
  placedCrumbsOf,
  type Crumbs,
} from "./tabChats";
import { useChatsListPrefs } from "./chatsListPrefs";
import { placeRowId, type Place as TaskPlace } from "./TabChip";
import { ChatTasks } from "./TabChip";
import { runningSaid } from "./taskLimits";
import { hasTasks, type Ended, type Needing } from "./tabTasks";
import { readUsed, type UsedAsk } from "./tasksUsed";
import { finishedBucketOf, taskBucketOf } from "./taskBuckets";
import { stateClock } from "./stateClock";
import { useTabIds } from "./ui/tabs/TabStrip";
import { TASK_KEY_ROW, taskKeyOf } from "./taskKeys";
import { paneStepOf } from "./moveKeys";
import { paneToward } from "./panePlaces";
import { paneSaidOf } from "./paneSaid";
import { usePretendLimits, usePretendTasks, withPretendedLimits } from "./e2eTasks";
import { BriefOpener, BriefPanel, type BriefAsk, type OpenBrief } from "./Brief";
import { AnswerOpener, AnswerPanel, type AnswerAsk, type OpenAnswer } from "./AnswerPanel";
import { activityTitle, activityView } from "./activity";
import { type ChipToExplain } from "./ChipExplained";
import { AwaySummary, useAwaySummary } from "./AwaySummary";
import { AsksQuiet } from "./QuietSummary";
import {
  asksInAModal,
  inPlace,
  TaskEndAsk,
  TaskEndContext,
  type TaskEndAsked,
  type TaskEndFrom,
  type TaskEndHand,
  type TaskEndInline,
} from "./TaskEnd";
import { giveKeyboardTo } from "./paneKeyboard";
import { endedState } from "./EndedBand";
import { useHeldAmong } from "./dispatchesHeld";
import { useRefusedAmong } from "./vaultRefusals";
import { type Asking } from "./NeedsYou";
import { EndingChat, type SmartAsk } from "./EndingChat";
import { draftsOfClosing, settleClosingDrafts, type KeptOnClose } from "./closingDrafts";
import { useCloseQuestion } from "./closeQuestion";
import { KeepAsk, ThemeDraftStrip } from "./ThemeDraftStrip";
import { ChatAsk, focusAfterNoticeGone } from "./ChatAsk";
import { ApprovalSentence, ProfileMeta } from "./ProfileApproval";
import { DID_NOT_START, saidWhenItEnds, stoppedWhy, useSmartClosing } from "./smartClose";
import { attentionPanels, Panels } from "./Panels";
import { NewVault } from "./NewVault";
import { OpenVault, useVaults } from "./Vaults";
import { usePlaneEdits } from "./PlaneEdits";
import { MemoryStores, useMemoryEdits, useMemoryStores } from "./MemoryEdits";
import { PlacingHere, usePaneDrag } from "./paneDrag";
import { PANES_ROOM } from "./paneHeld";
import { firstPaneNameOf, type DropContext } from "./paneDrop";
import type { MemoryTargets } from "./memoryMoves";
import { AddToAChat } from "./AddToAChat";
import { handReferences } from "./referencesMany";
import { DRAFT, isMemory, memoryRefOf } from "./memories";
import type { FirstTaskDoes } from "./FirstTaskTab";
import { onAMac } from "./tabKeys";
import { pressed, SIDE_KEY_IDS } from "./keys/registry";
import { SandboxChangedNotice, useLevelWhys, useOlderSandbox } from "./SandboxChanged";
import { useSandboxCommands } from "./sandboxAsked";
import { useAskOffer } from "./askOffer";
import {
  AskPersona,
  AskPersonaOpener,
  type AskFrom,
  type AskPrefill,
  type OpenAskPersona,
} from "./AskPersona";
import { SandboxOffer } from "./SandboxOffer";
import { ProjectHostsNotice } from "./ProjectHostsNotice";
import { RunnerGoneNotice } from "./RunnerGoneNotice";
import { FreshRunnerKeptNotice } from "./FreshRunnerKeptNotice";
import { RunnerKeyChangedNotice } from "./RunnerKeyChangedNotice";
import { ProjectExpectsNotice } from "./ProjectExpectsNotice";
import { TeamSandboxOffNotice } from "./TeamSandboxOffNotice";
import { ProjectPresetsNotice } from "./ProjectPresetsNotice";
import { PersonaHostsNotice } from "./PersonaHostsNotice";
import { ProjectDispatchNotice } from "./ProjectDispatchNotice";
import type {
  ExtensionCommand,
  ExtensionHeard,
  ExtensionView,
  Offered,
  PanelView,
  BrowsersEnded,
  AskEnded,
  ProjectReady,
  Rescreened,
  RowAction,
  SavedRecord,
  Shown,
  SmartClosing,
  WithoutSandbox,
} from "./bindings";
import { AskFirst, runExtensionAction } from "./ExtensionAction";
import { extensionsChanged, useExtensionsOn } from "./extensionsOn";
import {
  SKILL_STUDIO_ASKED,
  SKILL_STUDIO_TITLE,
  SKILL_STUDIO_VIEW,
  AT_THE_ROOT,
  ChatToUseHere,
  asPersona,
  inWorkspace,
  isSkillStudio,
  skillRuns as skillRunsAt,
  skillRowNamed,
  skillStudioAt,
  studioOfSkillTab,
  type ChatToUse,
  type SkillStudioAsk,
  studioHistoryOf,
  studioOpening,
  studioTabsGone,
  useSkillStudioSaid,
} from "./skillStudio";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { readStudioAhead } from "./studioReadAhead";
import { forgetSkillEdit } from "./skillEdit";
import { discardGone, discardRemoved, unsavedClosedBy, unsavedIn } from "./unsavedWork";
import { UnsavedWorkQuestion, type UnsavedAsk } from "./UnsavedWorkQuestion";
import { SkillRunsHere, aPersonasOwn, useSkillRuns, type RunsOf } from "./skillRuns";
import { CHAT_SKILLS_ASKED, askChatSkills, type ChatSkillsAsk } from "./chatSkillsAsk";
import { projectThemeChanged, useProjectThemeKept } from "./projectTheme";
import { inForce, isWorkspaceColour, onDrawn, tabTint } from "./theme/theme";
import { handedFromNote } from "./handedFrom";
import { ChatsSection } from "./ChatsSection";
import { chatsListDrawn, type Reveal } from "./revealTask";
import { owedAfterEnded, restartsDue, withRestartOwed } from "./restartAtPause";
import { useFailuresSeen } from "./failuresSeen";
import { tasksBelowOf, type TasksBelow } from "./sessionTasks";
import { TasksBelowLent } from "./TasksBelow";
import { finishedOf, qualifierOf, shownOf, useFinishedTasks, useRowsUntilRead } from "./finished";
import { WaitingTaskWaysContext, type WaitingTaskWays } from "./waitingTasks";
import {
  below as chatsBelow,
  cantReportAsks,
  chatsTree,
  listedChat,
  type ListedChat,
} from "./chatsTree";
import {
  stopAllAnswer,
  stopAllSays,
  stopAllTitle,
  stopAnswer,
  stopSays,
  stopTitle,
  useStopping,
  type StopAllAsked,
  type StopAsked,
} from "./stopping";
import { DoingsHere, useDoings, type DoingsOf } from "./chatDoing";
import { AllChatsOf, type EveryChat } from "./AllChats";
import { fleetOf } from "./everyChat";
import type { ChatState } from "./shownState";
import {
  ChatsHere,
  backSaid,
  inItsTurn,
  isShell,
  ownWorkOf,
  quietChats,
  sameList,
  sameOwnWork,
  sameQuiet,
  stateOf,
  turnStateOf,
  useChats,
  useChatsHere,
  useChatsSelect,
  type FailedBelow,
  type State,
} from "./chatState";
import {
  askMove,
  chatMovesOf,
  forgetMove,
  heardMove,
  movedAfterSignIn,
  moveOffers,
  movesOnceAPoolIsSetUp,
  movesDue,
  movesWith,
  poolsToMoveTo,
  useChatMoves,
  useChatMovesHeard,
} from "./chatMoves";
import { anyPoolKept, readRunnerStage, refusedWith, saveLimits, usePools } from "./pools";
import { useSignOutTerminals } from "./settings/signOutTerminals";
import { LEAST, LEAST_CHIP, leastAt, pinnedFirst, useRoom } from "./fits";
import type { Naming } from "./tabFace";
import { tabPanelOf, tabsPart } from "./ui/tabs/Tabs";
import { panelRoot } from "./ui/panel/Panel";
import { oneChatInterrupted, type Ending } from "./QuitWarning";
import { answerTaken, asksMoved, secretAskEntries } from "./asks";
import type { Snoozing } from "./AsksSnooze";
import type { SecretAskGiven } from "./SecretAskNotice";
import type { SecretAskEnded } from "./StartChat";
import { useTextSizes } from "./textSize";
import { focusStands } from "./Cockpit";
import {
  landing,
  linkToGroup,
  PROFILE_SIGN_IN,
  PROFILE_SIGN_OUT,
  SETTINGS_ACTION,
  SETTINGS_LINK,
  THEME_CHAT,
  askThemeChat,
  type ProfileSignInAsk,
  type ProfileSignOutAsk,
  type SettingsActionAsk,
  type SettingsLink,
  type SettingsLinkAsk,
  type ThemeChatAsk,
} from "./settings/links";
import { enterSettings, placeOfView } from "./settings/entering";
import { onceTheKeyboardIsBack } from "./keyboardBack";
import { SessionOffersLent } from "./sessionOffers";
import { askSkillChat, SKILL_CHAT, useSkillDrafts, type SkillChatAsk } from "./skillDraftAsks";
import {
  StudioChatsHere,
  doneByDrafts,
  linkOfAsk,
  scopeOfLevel,
  studioPlaceOf,
  type StudioChatsLent,
} from "./studioChats";
import { skillChatRefused } from "./SkillChatApproval";
import {
  StartApproveAsk,
  startRefused,
  startRefusedRan,
  type StartApproving,
} from "./StartApproval";
import { ChatStrip, NO_ENDED, NO_ROWS } from "./ChatStrip";
import {
  type FirstChat,
  type PlaneReport,
  type ReportedWorkspaces,
  type WindowDoing,
} from "./planeReport";
import type { SandboxSaid } from "./SandboxMark";
import type { ChatHead } from "./paneHead";
import {
  type RestartAct,
  type RestartSaid,
  type Arrived,
  type ByHandNote,
  type StartNotes,
  withNote,
  ChatNotices,
} from "./ChatNotices";
import {
  type EndedChat,
  LayoutPanes,
  type PaneFace,
  type TasksOn,
  type UsingOn,
} from "./LayoutPanes";
import { drawnOf } from "./paneFold";
import { usePaneRoom } from "./usePaneRoom";
import { ChatPersonaState } from "./ChatPersonaState";
import { carryPaneBand, forgetPaneBand, forgetPaneBands, paneBand } from "./noticeSettle";
import { useWorkedHere } from "./workedHere";
import { newTabRows } from "./newTabList";
import {
  newAsks,
  newNotices,
  newUpdates,
  useChatOnScreenSeen,
  useSeen,
  useSeenElsewhere,
  type SeenRows,
} from "./seen";
import { SeenRowsOf } from "./seenRows";

/** Whether two lists of the right side's extension panels draw the same tabs. */
const samePanels = (a: readonly PanelTab[], b: readonly PanelTab[]) =>
  a.length === b.length &&
  a.every(
    (one, at) => one.view === b[at].view && one.name === b[at].name && one.mark === b[at].mark,
  );

/** No browsers: a chat that uses none. */
const NO_BROWSERS: readonly BrowserShown[] = [];

/** One empty list, so a prop left out is the same list at every render. */
const NONE: readonly never[] = [];

/** The keys that open something, each its catalogue row's (#5292): ⌘⇧T, ⌘T and ⌘N. ⌘⇧T first,
 *  so ⌘T's matcher never meets a Shift. */
const OPENING_KEYS = ["shell.new", "tab.new", "chat.new"] as const;

/** Where the picker's chat goes: a new tab — started in `in` when a row asked for one
 *  directory for that tab alone (charter-app#174), else where the explorer's pick says — or a
 *  split of the pane in front. `prefer` is the harness (a profile's `kind`) the picker starts
 *  on, where something already knows which one is wanted: a harness started by hand in a shell
 *  tab, opened as a chat instead (ADR 0062). */
type Where =
  | {
      tab: true;
      in?: string;
      prefer?: string;
      /** A pool chosen on the picker's Runs on row as it opens, in the workspace `workspace`
       *  (#4483): Pool setup's Open a chat. */
      pool?: { name: string; workspace: string };
      /** The empty pane New chat was pressed for (#5289): the chat takes it, while it stays
       *  empty, whichever tab is in front by the time the start answers. */
      pane?: PaneAt;
      /** Asked from the strip's `+` list (#5292): a tab of its own, even where an empty pane
       *  waits. */
      ownTab?: boolean;
    }
  | { split: Direction };

/** `set` without `one`, or `set` itself where it does not hold it. */
function withoutOne(set: ReadonlySet<number>, one: number): ReadonlySet<number> {
  if (!set.has(one)) return set;
  const left = new Set(set);
  left.delete(one);
  return left;
}

/** The words that name a forge login: `gh auth login`, `glab auth login`. */
const LOGIN_WORDS = 3;

/** **A forge login's one-job pane is named for the login** (#4761): its first words,
 *  `gh auth login`, never the host a held line goes on to name. */
function loginName(line: string): string {
  return line.trim().split(/\s+/).slice(0, LOGIN_WORDS).join(" ");
}

/** What a new shell has typed into it: a line spelled out (FR-4's `gh auth login`), or a
 *  harness whose compiled-in installer the core types itself (FR-29, V65). `held` is a line typed and left for the person to run: the repo picker's forge login,
 *  whose host is the project's, read before Return (NO-8). */
type Typed =
  | {
      line: string;
      held?: boolean;
      installer?: undefined;
      signIn?: undefined;
      signOut?: undefined;
    }
  | {
      installer: string;
      line?: undefined;
      signIn?: undefined;
      signOut?: undefined;
    }
  | {
      /** A sign-out that asks in its terminal, on this pool's runner (#4124). */
      signOut: string;
      pool: string;
      signIn?: undefined;
      line?: undefined;
      installer?: undefined;
    }
  | {
      signIn: string;
      provider?: string;
      /** The pool whose runner it is signed in on (#2049), where it is not this machine. */
      pool?: string;
      /** By the harness's device-code flow (#4044). */
      device?: boolean;
      signOut?: undefined;
      line?: undefined;
      installer?: undefined;
    };

/**
 * **Which clone of a new workspace's repos this is**, for its track (`cloneTrack`, #3649): each
 * run's track is its own and ends with it, so a workspace made again under the same name starts
 * afresh. Counted for the window, not for one project's view (#3898): the view is drawn anew
 * when its project is closed and opened, while the kept results and the tracks put away live on
 * in the window, so a count started again would say an old run's track once more.
 */
let cloneRuns = 0;

/** `was`, and which chat each task `sidebar` lists is a task of now: `was` itself if none moved. */
function withAskersOf(
  was: ReadonlyMap<number, number>,
  sidebar: Pick<SidebarModel, "unfiled" | "workspaces">,
): ReadonlyMap<number, number> {
  const tasks = [...sidebar.workspaces.flatMap((ws) => ws.chats), ...sidebar.unfiled].flatMap(
    (chat) =>
      chat.from?.task && was.get(chat.session) !== chat.from.chat
        ? [[chat.session, chat.from.chat] as const]
        : [],
  );
  return tasks.length === 0 ? was : new Map([...was, ...tasks]);
}

/**
 * One project, with everything that belongs to it.
 *
 * **This is the extraction ADR 0033 costed and #121 named as what it was leaving behind.** The
 * window used to BE the project: one `App` held the tabs, the sidebar, the panels, the chat
 * states, the picker and the palette, so a second project had nowhere to put any of them and
 * the app could draw exactly one. Here a project holds its own, and the window holds projects.
 *
 * **A project that is not in front keeps everything, and its view stays drawn off the window**
 * (`projectShown.tsx`). It stays mounted: its tabs, its splits, its focused workspace and its
 * picker are React state, and its `useChats` goes on listening, so the project tab can say a
 * chat over there needs you. That is the operator's own reason for wanting one window per
 * project in the first place — fifty chats in project A must not be torn down because he
 * glanced at project B.
 *
 * **What it drew is kept too, in an element that is in no document**, for the projects the
 * person was in last (`KEPT_DRAWN`). It used to draw nothing while another project was in
 * front, so every switch built the whole view again: about a thousand components mounted, more
 * with each chat the project holds, some fifty commands sent to the core by what mounts, and
 * the terminal of its chat disposed and a new one started and sent the screen. The operator
 * felt that as one to three seconds of nothing. Now a switch takes one element out of the
 * window and puts another in (`projectSwitch.cost.test.tsx` holds the counts).
 *
 * Off the window, and not hidden in it with CSS: a hidden `[data-strip="Tabs"]` is a second tab
 * strip, and a second copy of every id, for every query in this app and in the scenario tests
 * to trip over. An element in no document is found by none of them, is laid out and painted by
 * nothing, and takes no key and no press.
 *
 * **And not kept drawn under React's `<Activity mode="hidden">`** (FR-27, #620). That was
 * prototyped around this view's chrome only, on CI, and the switch it was meant to speed up
 * moved by less than CI's own spread from run to run. A hidden `Activity` keeps what was
 * drawn and takes its effects down, and the effects are most of what a switch costs here: the
 * reads each part makes as it mounts, and the terminal.
 *
 * **While it is off the window it is not drawn again** (`AsLastShown`): what it hears about its
 * chats is kept as state here and reported to the window, and its view takes it all at once as
 * it comes back. Its terminals are not React's to draw: they go on taking what their programs
 * write. What any part of it draws over the whole window, a dialog or a menu, is drawn only
 * while the project is in front, whenever it was opened (the families' surfaces); and should anything
 * still be over the window once the project has left, the view is not kept at all
 * (`drawnOverTheWindow`).
 *
 * A project the person has not been in lately draws nothing, as every project behind the front
 * one used to, and is drawn again when they come back to it. Only coming to the front draws a
 * view: a project the window opens behind another draws nothing until then.
 *
 * **The palette is the window's, not a project's**, for the same reason turned round. It has
 * to be mounted before the core has said which project this launch opened — `F2` is a
 * keystroke it listens for itself — and mounted once, because it claims that key on the
 * window with a capturing listener. So what it lists travels up from here with the rest of
 * this project's report, and the window draws the one palette.
 *
 * Its panes stay with it: only the tab in front has panes drawn (`tabs.ts`), so a project kept
 * off the window keeps those terminals and their views of the session open. A project that
 * draws nothing has none, which costs it nothing: the core has held every session's terminal
 * all along, and a view opened again is sent the screen as it already is. Apart from those, the
 * last three chats shown across the window keep theirs (`keptTerminals.ts`), so a tab switched
 * back to one of them starts nothing either.
 *
 * **Held (`memo`), so the window redrawing is not this view redrawing** (SC-3). Every chat move
 * reaches the window as this project's report — the quit warning's states, the project strip's
 * order — and the window redraws for it; a project view drawn again for that, with the same
 * props, would be every tab and pane redrawn for one chat's move.
 */
export const PlaneView = memo(function PlaneView({
  plane,
  inFront,
  kept,
  slot,
  projects,
  pinnedProjects,
  window: windowDoes,
  onReport,
  onSessionRows,
  alerts,
  statusNotices,
  contributed: surveyedPanels = NONE,
  views: surveyedViews = NONE,
  commands: surveyedCommands = NONE,
  settingsAsked,
  savingAsked,
  poolSetupAsked,
  licencesAsked,
  focusMode = false,
  tabWindow,
  settingsTabAsked,
  settingsLinkAsked,
  yourSettingsAsked,
  firstChatAsked,
  shellAsked,
  fileAsked,
  artifactAsked,
  closeAsked,
  onAsking,
  awayRefused,
  onAway,
  waiting,
  questions,
  inboxAsked,
  inboxGroup,
  inboxUpdate,
  elsewhere,
  everyChat,
  windowLines,
  profileNames,
  onSeen,
}: {
  plane: PlaneId;
  /** Whether this is the project the operator is looking at. */
  inFront: boolean;
  /** Whether it stays drawn while another project is in front: the person was in it lately. */
  kept: boolean;
  /** Where its view goes while it is in front: the window's element for the project it shows. */
  slot: HTMLElement | null;
  /** Every project this window holds, for the rows that switch between them. */
  projects: readonly Project[];
  /** Which of them this operator has pinned, by root. The window holds it, because the
   *  project strip is the window's; this project's catalogue lists the rows. */
  pinnedProjects: readonly string[];
  /** What the WINDOW does, which this project asks for rather than doing itself: opening
   *  another project, switching to one, letting one go, and quitting. */
  window: WindowDoing;
  /** What this project has open and whether it has found out yet, for the window's quit
   *  warning and for this project's own tab. */
  onReport: (plane: PlaneId, report: PlaneReport) => void;
  /** The session rows of the place in front, for the window's palette (#1160): built beside the
   *  panels (`sessionOffers.tsx`) and not in this view's catalogue, so a record written does not
   *  draw this view again. */
  onSessionRows?: (plane: PlaneId, rows: readonly Offer[]) => void;
  /** Every open project's alerts, as the window reads them (#1695): this project's Inbox lists
   *  its own and this machine's as Notices, and names each other project that has some. */
  alerts?: WindowAlerts;
  /** The notices that pop from the status line's Notices item (#3653): the window's, handed to
   *  the project in front alone. */
  statusNotices?: ChipNotices;
  /** What approved extensions contribute to the side region. The window's, for the same reason
   *  the alerts are: an extension is installed per machine and never travels in a plane
   *  (ADR 0041), so one survey serves every project this window holds — and this project keeps
   *  of it what it has on (ADR 0048). */
  contributed?: readonly PanelView[];
  /** The views approved extensions offer (ADR 0041 stage 2), the window's for the
   *  same reason: one survey per window, not one per project. */
  views?: readonly ExtensionView[];
  /** The palette commands approved extensions add (charter-app#341), the window's for the same
   *  reason, and filtered here by what this project has on, as the views are. */
  commands?: readonly ExtensionCommand[];
  /** A count that goes up each time the window is asked for THIS project's Settings tab at the
   *  Project level (`WindowDoing.openSettings`, SE-19); `undefined` until it is. */
  settingsAsked?: number;
  /** The same, for THIS project's Saving tab (`WindowDoing.openSaving`, charter-app#294). */
  savingAsked?: number;
  /** The same, for the Pool setup tab (spec #4469). */
  poolSetupAsked?: number;
  /** The same, for the Licences tab About opens in the project in front (#1719). */
  licencesAsked?: number;
  /** Focus mode is on (#2921): its palette row says it brings the window's parts back. */
  focusMode?: boolean;
  /**
   * **This is tab window `tabWindow` of the project** (#4768): a bare window holding the strip
   * and panes of its own tabs, and no sidebar. It draws only the tabs the core holds for it,
   * says only those, and opens no tab for a chat by itself: a chat arriving is the project
   * window's to announce. Absent in the project window.
   */
  tabWindow?: number;
  /** The same, for the Settings tab (`WindowDoing.openSettingsTab`, SE-16): a count that goes
   *  up each time the window asks for it on THIS project's strip. */
  settingsTabAsked?: number;
  /** A link into a Settings group the window followed (SE-22), once per `at`: Settings opens
   *  at the link's level and group on THIS project's strip. */
  settingsLinkAsked?: { link: SettingsLink; at: number };
  /** The same, for the Settings tab at the You level (`WindowDoing.openYourSettings`, SE-23). */
  yourSettingsAsked?: number;
  /** A file ⌘P found in THIS project (FM-7), opened in its file tab once per `at`; with a
   *  `line`, a jump to it (a search hit, FM-8), opened in the branch's file tab at that line. */
  fileAsked?: { place: Place; path: string; line?: number; at: number };
  /** An artifact the palette found in THIS project (#5968), opened in its own tab once per
   *  `at`, as pressing its icon opens it. */
  artifactAsked?: { found: ArtifactFound; at: number };
  /** A link's close of chats with theme drafts, handed to THIS project (#2761), asked here once
   *  per `at`: the chats it ends. */
  closeAsked?: { sessions: readonly number[]; at: number; dropped: boolean };
  /** Told whether THIS project has a close or a "Who draws it?" question up (#2761), so the
   *  window asks no link's close over any project's question. */
  onAsking?: (plane: PlaneId, up: boolean) => void;
  /** This project's dispatches refused while nobody was at their chat (#1507), which the
   *  window holds: updates in the Inbox (#1693), and a part of the away summary (#1551). */
  awayRefused?: readonly AwayRefusal[];
  /** Answers one of those in this project, as its update's buttons do: the window holds the
   *  answers. One callback for every project, so this view is not drawn again for it. */
  onAway?: (plane: PlaneId, item: AwayRefusal, how: "allow" | "dismiss" | "never") => void;
  /** This project's asks, as the window's registry derived them last (#1690): what the Inbox
   *  lists and its tab counts (#1692). Nothing before the first read. */
  waiting?: readonly Shown[];
  /** This project's task questions as the registry derived them last (#4564): what a task's
   *  row, its tab's hand and the rows above say of its question (`taskQuestions.ts`). */
  questions?: QuestionsHeld;
  /** A count that goes up each time the window asks for THIS project's Inbox: the title bar's
   *  ✋ (#1692, I-2). */
  inboxAsked?: number;
  /** The chat whose group that Inbox opens at, where one was asked for: the one a clicked
   *  notification was about (#1694, I-7). */
  inboxGroup?: number;
  /** The task's line in Updates that Inbox opens at instead, read in place, where a clicked
   *  notification was about a task that ended (#6005). */
  inboxUpdate?: string;
  /** What its Inbox lists that its asks registry cannot see (#1695): chats waiting in other
   *  windows, and chats that cannot say they wait. */
  elsewhere?: Elsewhere;
  /** **Every chat of every open project** (#6013), for the Chats sidebar's All chats: what the
   *  window gathers from every project's report, here and in the other windows. Without it, the
   *  sidebar offers no All chats. */
  everyChat?: EveryChat;
  /** **The window's own lines** (D-LB-1): listed at the top of this project's Inbox's Notices
   *  and counted in its status line while it is in front, so none stands under the title bar
   *  with a project open. The window draws them; this view gives them their place. */
  windowLines?: WindowLines;
  /** This project's harness profiles, by name, for the palette's rows to their pages (#1201):
   *  read by the window as the palette opens over this project, and the same list while the
   *  names are (#4351). None while none was read, or the project has none. */
  profileNames?: readonly string[];
  /** **Hears this project's seen record while it is in front** (#6004): the window's chips, its
   *  status item and the Results it lists read what is New from it, as this project's rows do. */
  onSeen?: (plane: PlaneId, rows: SeenRows) => void;
  /** The first chat a repository opened into this project asks for (FR-4): started in that
   *  repository's clone, on the workspace named after it. `at` counts the asks, so each is
   *  answered once. */
  firstChatAsked?: FirstChat;
  /** A one-job shell at the project root with a command typed into it — `gh auth login` or
   *  `glab auth login` from the first run (FR-4) — asked for once per `at`. */
  shellAsked?: { typed: string; at: number };
}) {
  const [tabs, setTabs] = useState<Tabs>(noTabs);
  // The tab in front of the project in front, for what the stall log writes (`slowLog.ts`).
  const inFrontTab = tabs.inFront === undefined ? undefined : tabs.byId[tabs.inFront];
  const frontTabName = inFrontTab === undefined ? undefined : tabNameOf(inFrontTab);
  useEffect(() => (inFront ? frontTabIs(frontTabName) : undefined), [inFront, frontTabName]);
  const signingOutIn = useSignOutTerminals(tabs, plane);
  // **A pane closed for good lets its band's memory go** (#3167): a pane's number is never
  // used again, so one gone from every tab is gone for good.
  const panesDrawn = useRef<{ plane: string; panes: ReadonlySet<number> }>(undefined);
  useEffect(() => {
    const panes = new Set(tabs.order.flatMap((id) => contentsOf(tabs, id).map((one) => one.pane)));
    const was = panesDrawn.current;
    if (was !== undefined) {
      for (const pane of was.panes) {
        if (was.plane !== plane || !panes.has(pane)) forgetPaneBand(paneBand(was.plane, pane));
      }
    }
    panesDrawn.current = { plane, panes };
  }, [plane, tabs]);
  // The project leaves the window: every one of its panes is gone for good.
  useEffect(() => () => forgetPaneBands(plane), [plane]);
  /**
   * **The breadcrumb each pane last drew for the task it shows**, by tab, pane and chat
   * (#1486). A pane never shows a task without its breadcrumb, and a task that has ended has
   * no path left to read off the list: this is what the pane goes on saying of it, as ended,
   * until the person leaves it. Forgotten with what it is about.
   */
  const [remembered, setRemembered] = useState<ReadonlyMap<string, Crumbs>>(() => new Map());
  /** What every chat is doing, in THIS project. Pushed from the core; nothing here polls.
   *  It keeps listening while the project is behind another one, which is what lets its tab
   *  say that something over there needs you. */
  const chats = useChats(plane);
  const doings = useDoings(plane);
  // The skill run open in each chat, for the chip on its tab (SK-3).
  const skillRuns = useSkillRuns(plane);
  // **Only the shares this view draws from, each redrawing it only when it changes** (SC-3).
  // A chat's own state is not one of them: its tab, its explorer row and its pane read that
  // themselves, so a chat that only went from running to waiting redraws those and not this.
  //
  // **Nor is the needs-you queue** (#1034). The counts, the chips' hands and the show-more
  // counts read it where they are drawn (`QueueRead`), and the catalogue's rows for it and the
  // title bar's list are put in as this view reports, off the store (`withQueue`, `queued`). A
  // chat that starts asking for you redraws those and not every pane.
  /**
   * **The one polite line about a pane the keyboard did not ask for** (#4767, ruled on #4699):
   * a press that lands on a chat already in another pane of the tab says where it went. Where
   * the keyboard follows a change into its own pane, the terminal's name says it, and nothing
   * here does. Counted, so the same line said twice is said twice.
   */
  const [paneSaid, setPaneSaid] = useState<{ said: string; count: number }>({
    said: "",
    count: 0,
  });
  const pretended = usePretendTasks();
  /** The limit numbers a scenario spec pretends a session has (`e2eTasks.ts`, #1545): none in a
   *  shipped build. Laid over each chat as it is listed, as the core would send them. */
  const pretendedLimits = usePretendLimits();
  const reports = useChatsSelect(chats, (states) => states.reports);
  const refusals = useChatsSelect(chats, (states) => states.refusals);
  const needs = useChatsSelect(chats, (states) => states.needs);
  const stoppedBelow = useChatsSelect(chats, (states) => states.stoppedBelow);
  const failedTasks = useChatsSelect(chats, (states) => states.failedTasks);
  /** What the core noticed of each chat (#6014): why it reads Stuck?, and work left behind. */
  const noticedChats = useChatsSelect(chats, (states) => states.noticed);
  /** The finished row the Chats list is asked to bring into view: a task that failed, when the
   *  person goes to its needs-you item (#1491). */
  const [revealed, setRevealed] = useState<Reveal>();
  /**
   * **What the last window action here refused, and which action** (NO-4): the picker's
   * options, or a shell. A Notice with Dismiss, and it clears itself when that same action next
   * succeeds — it used to stand until some other refusal replaced it.
   */
  const [trouble, setTrouble] = useState<{
    from: TroubleFrom;
    said: string;
  }>();
  const refusedBy = useCallback(
    (from: TroubleFrom, said: string) => setTrouble({ from, said }),
    [],
  );
  const succeeded = useCallback(
    (from: TroubleFrom) => setTrouble((was) => (was?.from === from ? undefined : was)),
    [],
  );
  const [sidebar, setSidebar] = useState<SidebarModel>();
  /** The plane's sidebar could not be read: the window still keeps a tab on its root strip
   *  (#4758), so it is never blank. */
  const [sidebarUnread, setSidebarUnread] = useState(false);
  /** The sidebar held, for its read's answer to compare with outside React's own update. */
  const sidebarHeld = useRef<SidebarModel>(undefined);
  useEffect(() => {
    sidebarHeld.current = sidebar;
  }, [sidebar]);
  /** Reads the finished tasks again, drawing nothing if they are as held (set further down). */
  const finishedQuietly = useRef<() => void>(() => undefined);
  /** How many chats a stop has ended: the sidebar is read again for each, since a task chat
   *  with no tab leaves the list without any tab changing (#1448). */
  const [stopsEnded, setStopsEnded] = useState(0);
  /** This project's save standing (charter-app#302): every project reads its own, so the project
   *  strip can mark the ones with unsaved work and the title bar can show the one in front.
   *  **On the 10 s beat only while in front** (felt speed, round 2): read here, outside this
   *  view's own `ProjectShownProvider`, every project ran `git status` on the same tick. One
   *  behind reads when its git standing moves, on focus, after a save and as it comes back. */
  const { saving, read: savingRead } = usePlaneSaving(plane, inFront);
  /** The workspace whose LIVE/LOCAL confirmation is open (charter-app#301). */
  const [liveAsk, setLiveAsk] = useState<string>();
  /** The repo the "Remove from workspace" question is about, while it is open (#1228). */
  const [membershipAsk, setMembershipAsk] = useState<{ workspace: string; repo: string }>();
  /**
   * The workspace the operator last PICKED, which is not always the one drawn.
   *
   * `focused` below is the one drawn, and it is derived from this and from the tab in front —
   * see it for why. This is only half the answer, so nothing outside those few lines reads
   * it: a workspace with no chats has nothing in front to be derived from, and this is what
   * keeps the window on it.
   */
  const [picked, setPicked] = useState<string>();
  /** The chats the core put back at this launch, so a pane can say which came back how. */
  const [reopened, setReopened] = useState<OpenChat[]>([]);
  /** The picker, when a new chat has been asked for, and what to do with the session it
   *  starts. A harness starts only once a row in it is picked: nothing is opened until then,
   *  so cancelling leaves nothing to tear down. Both a new tab and a split come through
   *  here, because both start a harness and ADR 0022 admits no path that does not pick. */
  const [picking, setPicking] = useState<{
    options: StartOptions;
    where: Where;
    /** Where a chat started here would run, and why (#4536): the picker's Runs on row. */
    runsOn?: RunsOnInPicker;
    /** Whether the rows' details (approval, sandbox, billing) are still to come (#4736): the
     *  picker opens on the first answer and draws those rows as pending. */
    pending?: boolean;
  }>();
  /** The picker the details being read are for (#4736): a details answer for a picker since
   *  closed, or opened again, fills in nothing. */
  const pickerAsked = useRef(0);
  /** Why the last start did not happen, shown in the picker rather than behind it. */
  const [pickerTrouble, setPickerTrouble] = useState<string>();
  /** The profile of this machine's a start was refused on because its harness says it is not
   *  signed in here (#2035), with that refusal: the picker offers Sign in beside it, and beside
   *  no other. */
  const [pickerSignIn, setPickerSignIn] = useState<{ profile: string; refused: string }>();
  /** **The Secret ask a start was refused on** (#5137), with that refusal: the picker draws the
   *  ask beside it, and beside no other. */
  const [pickerSecret, setPickerSecret] = useState<{
    asked: StartSecretAskNamed;
    refused: string;
    profile: string;
  }>();
  /** Whether the picker's Secret ask is being answered, and what its last press was told. */
  const [pickerSecretBusy, setPickerSecretBusy] = useState(false);
  const [pickerSecretSaid, setPickerSecretSaid] = useState<string>();
  /** The pool's limit a start was refused on (spec #4275), with that refusal: the picker offers
   *  to raise it beside it, and beside no other. */
  const [pickerLimit, setPickerLimit] = useState<{
    offer: PoolLimitOffer;
    refused: string;
    profile: string;
  }>();
  /** The pool a start was to run on and could not reach (#4536), with that refusal: the picker
   *  offers Try again, Pool setup and Run on this machine this once beside it, and beside no
   *  other. */
  const [pickerPoolGone, setPickerPoolGone] = useState<{ pool: string; refused: string }>();
  /** Whether the picker's fix (NO-8: its `local-ignore`) is running. */
  const [pickerFixing, setPickerFixing] = useState(false);
  /** **Where a new chat in `workspace` would run** (#4536), read as the picker opens so its Runs
   *  on row is drawn with it: `undefined` where the core could not say, and no row is drawn. */
  const readRunsOn = useCallback(
    async (workspace: string | null): Promise<RunsOnInPicker | undefined> => {
      const [said, standing] = await Promise.all([
        commands.startRunsOn(plane, workspace).catch(() => undefined),
        commands.poolSetupInThePicker(plane).catch(() => undefined),
      ]);
      if (said?.status !== "ok" || !Array.isArray(said.data?.pools)) return undefined;
      // How each pool stands (#4484): one where a chat can't start isn't offered, and one whose
      // setup isn't finished says so. Unread, every pool is offered as the levels list it.
      const pools = standing?.status === "ok" && Array.isArray(standing.data) ? standing.data : [];
      const notOffered = new Set(pools.filter((one) => !one.offered).map((one) => one.pool));
      const notFinished: Record<string, string> = {};
      const notOfferedSaid: Record<string, string> = {};
      for (const one of pools)
        if (one.said !== null) (one.offered ? notFinished : notOfferedSaid)[one.pool] = one.said;
      return {
        ...said.data,
        pools: said.data.pools.filter((pool) => !notOffered.has(pool)),
        notFinished,
        notOffered: notOfferedSaid,
      };
    },
    [plane],
  );
  /** The tab whose Rename is open, if one is (#5343), and how its answer is going. */
  const [renamingTab, setRenamingTab] = useState<{
    tab: number;
    trouble?: string;
    busy: boolean;
  }>();
  /** The work item each chat works on, by session, as the core answered (V60, ADR 0088). */
  const [workItems, setWorkItems] = useState<Record<number, string>>({});
  // **Pin to workspace** (#5953): the page it asks about, while its dialog is up.
  const [pinningPage, setPinningPage] = useState<BranchPath>();
  /** The chat Rename chat… is asking about, in a dialog. */
  const [renamingChat, setRenamingChat] = useState<{
    session: number;
    name: string;
    trouble?: string;
    busy: boolean;
  }>();
  /** The chat the Link to work item dialog is asking about, while it is open. */
  const [linkingWork, setLinkingWork] = useState<{
    session: number;
    trouble?: string;
    busy: boolean;
  }>();
  /** The chat the Ask {persona} dialog is asking from, and the persona, while it is open. */
  const [askingPersona, setAskingPersona] = useState<{
    session: number;
    persona: string;
    prefill?: AskPrefill;
    from?: AskFrom;
    trouble?: string;
    busy: boolean;
  }>();
  /** The chat Use skill… is open for (#2775), with its skills once they are read. */
  const [usingSkill, setUsingSkill] = useState<{
    session: number;
    /** The skill it opens on, where one was asked for: the Skills view's Use in this chat. */
    picks?: string;
    skills?: ChatSkills;
    trouble?: string;
    busy: boolean;
  }>();
  /** {@link usingSkill} as the last render drew it: what a restart, heard outside a render,
   *  moves to the chat's new session (`followRestart`). */
  const usingSkillNow = useRef(usingSkill);
  useLayoutEffect(() => {
    usingSkillNow.current = usingSkill;
  });
  /**
   * **The Notices dismissed here until their cause changes** (`dismissals.ts`, V91j): kept on
   * this machine across relaunches, and let go once the core answers without the cause.
   */
  const { dismissed, dismiss, settle: settleNotices, showAgain } = useDismissals(plane);
  /** **This project's stale repos** (#1985), read once: listed in the Inbox, and each hung from
   *  its workspace's chip in the title bar too (#3650), with one dismissal. */
  const staleClones = useStaleClones({
    plane,
    shown: inFront,
    dismissed,
    dismiss,
    settle: settleNotices,
  });
  /** Chats this launch could not start, by name and why. They are still recorded. */
  const [wouldNotStart, setWouldNotStart] = useState<NotStarted[]>([]);
  /**
   * **The chats the launch has still to start** (`restoring.rs`), by the number each tab is
   * drawn under. The core registers every chat of the record and starts them behind the
   * window, so this window can be used at once: each has its tab in its place from the first
   * frame, its pane says it is restoring, and `chat-restored` says when it is the chat.
   */
  const [restoringChats, setRestoringChats] = useState<ReadonlySet<number>>(() => new Set());
  const restoringNow = useRef<ReadonlySet<number>>(new Set());
  /** What the reads of the restoring chats have seen, for the next to go on from. */
  const restoreSeen = useRef<RestoreSeen>(seenFirst([]));
  /** Raised when the chats were adopted with some still restoring: they are read again once,
   *  for one that started between the read and the listener. */
  const [restoreLooks, setRestoreLooks] = useState(0);
  /** **Forget this chat…** (NO-3): the chat whose record is about to be dropped, while the
   *  question is up, and the core's refusal of the last answer. */
  const [forgetting, setForgetting] = useState<{
    id: string;
    name: string;
    busy: boolean;
    trouble?: string;
  }>();
  /**
   * **Review and approve…** (#1246, D-1246-5): the waiting chat whose profile's command is being
   * approved, with the approval as the core said it when the question opened. It is never
   * updated while the question is up: the line approved is the line on screen, and the core
   * checks it against the file (`approve_profile`).
   */
  const [approving, setApproving] = useState<{
    id: string;
    name: string;
    approval: NeedsApproval;
    busy: boolean;
    trouble?: string;
  }>();
  /** The stop the person is being asked about, before anything is stopped (#1448). */
  const [stopAsk, setStopAsk] = useState<StopAsking>();
  /** The Stop all tasks the person is being asked about, before anything is stopped (#1498). */
  const [stopAllAsk, setStopAllAsk] = useState<StopAllAsking>();
  /** The task the person is being asked about ending, before anything is ended (#1488). */
  const [taskEndAsk, setTaskEndAsk] = useState<TaskEndAsked>();
  /** The second step of ending an idle task, asked where the press was made (#1488). */
  const [taskEndInline, setTaskEndInline] = useState<TaskEndInline>();
  /**
   * **Start fresh** (NO-3, charter#369): the chat that is about to be started again on the
   * project's instructions as they are now, while the question is up. The tab mark and the
   * chat's rows both come here (`chat.fresh:<session>`).
   */
  const [freshening, setFreshening] = useState<{
    session: number;
    name: string;
    files: readonly string[];
    /** What the chat was doing when it was asked about, as the board said (#1246). */
    doing: State;
    /** What it waited on of its own then (#3878), which ending its program ends (#5782). */
    ownWork?: OwnWork;
    /** Whether a background turn ran in it then (#4562), which ending it interrupts (#5782). */
    backgroundTurn: boolean;
    busy: boolean;
    trouble?: string;
  }>();
  /** What the core last said about where each chat is working, by the directory it was asked
   *  about — so an answer about a chat that WAS there is never drawn under one that is now.
   *  `workspaceState` keys its answers the same way, for the same reason. Every chat's, not only
   *  the one in front's: each chat's list has its own Branch rows (#4851). */
  const [located, setLocated] = useState<ReadonlyMap<string, ChatWorktree | undefined>>(
    () => new Map(),
  );
  /** Bumped when something changed the answer, so it is asked again rather than guessed. */
  const [relocate, setRelocate] = useState(0);
  /**
   * Bumped when THIS window changed which workspaces the plane has.
   *
   * The sidebar is re-read whenever the chats change, because opening or ending a chat is what
   * this window could previously change about the answer. Making and deleting a workspace
   * changes it without touching a chat, so they say so — the plane is still the truth and it is
   * read again, rather than the window editing its own copy of what it thinks is there.
   */
  const [replan, setReplan] = useState(0);
  /**
   * Asked again when a worktree row this window ran changed what the focused workspace holds.
   *
   * `replan` re-reads the SIDEBAR, which is workspaces and chats; this re-reads the one
   * workspace record the three regions share, which is where the clones, their git state and
   * their pieces live. They are two asks and a worktree removal invalidates only the second —
   * the piece is off the tree and `git status` in that clone now says something else
   * (charter-app#174).
   */
  const [rereadWorkspace, setRereadWorkspace] = useState(0);
  /** Bumped when the core says the instructions a chat reads at its start changed on disk
   *  (charter-app#264, FD-10d): `CLAUDE.md`, the harness settings, a persona's charter —
   *  never a todo, a memory or a session record. Every reader of the plane hears only what
   *  the core says concerns it (FD-10). */
  const instructionsChanges = usePlaneChanged([plane], INSTRUCTIONS);
  /** The same for the curation actions (FD-10d): the personas' curation files, the project's
   *  launch profile, the workspaces a subject can be. */
  const curationsChanges = usePlaneChanged([plane], CURATIONS);
  /** The changes what this project has on and its theme are made of (FD-10). */
  const settingsChanges = usePlaneChanged([plane], SETTINGS);
  /** Each harness's card, for the palette's *What <product> can do here* rows (#1134). */
  const harnessCards = useHarnessCards(plane, inFront, settingsChanges);
  /** The same, counting only the changes the sidebar is made of (FD-10): a memory an agent
   *  saves does not make it list every workspace's todos again. */
  const sidebarChanges = usePlaneChanged([plane], SIDEBAR);
  /** Every persona's icon and colour (#1449), read with the sidebar's changes: a persona's
   *  definition or its folder moving is one of them. */
  const personaMarks = usePersonaMarks(plane, sidebarChanges);
  /** The chats running on instructions the plane has changed since they started (charter#369),
   *  each marked on its tab, and those whose skills changed since (#2770), for their Skills
   *  chip (#3953). */
  const { files: planeUpdates, skills: skillsChanged } = usePlaneUpdated(
    plane,
    instructionsChanges,
  );
  // The plane root is watched, so an edit to `charter.toml` or `charter.local.toml` — in an
  // editor, from a `git pull` — is one of these, and what this project has on may have moved
  // with it (charter-app#253). Not at the mount: `useExtensionsOn` asks then.
  useEffect(() => {
    if (settingsChanges === 0) return;
    extensionsChanged(plane);
    // And the theme it draws, which the same two files and each `workspace.json` pick
    // (charter-app#273, #281).
    projectThemeChanged(plane);
  }, [settingsChanges, plane]);
  /** Whether the new-workspace dialog is up, why the last attempt made nothing, and whether
   *  charter is making one right now. */
  const [makingWorkspace, setMakingWorkspace] = useState(false);
  const [workspaceTrouble, setWorkspaceTrouble] = useState<string>();
  const [busyMaking, setBusyMaking] = useState(false);
  /** The repo the New branch dialog is cutting in, and the workspace it is that repo of, while
   *  it is up — then `makingWorkspace`'s other two, for a branch (GL-1). */
  const [branching, setBranching] = useState<{ workspace: string; repo: string }>();
  const [branchTrouble, setBranchTrouble] = useState<string>();
  const [busyBranching, setBusyBranching] = useState(false);
  /** The plane's vaults (`vault_list`), read here because four surfaces answer from them: the
   *  Vaults panel, the palette's `vault.open:<name>` rows, the picker, and — by asking for it
   *  again after a write — a vault's own tab (charter-app#235). */
  const vaults = useVaults(plane);
  const vaultNames = useMemo(() => vaults.vaults?.map((one) => one.name), [vaults.vaults]);
  // Whether the Skill studio's flag is on (#2765): its palette row is offered only then. What
  // the core said of it, `undefined` until it has, holds a side left on Skills (#3308).
  const studioSaid = useSkillStudioSaid();
  const studioOn = studioSaid === true;
  /** Every chat open here, by session, in strip order: whose skill budgets the window holds. */
  const chatSessions = useMemo(
    () => [...new Set(tabs.order.flatMap((id) => panesOf(tabs, id).map((pane) => pane.session)))],
    [tabs],
  );
  /** Each open chat's skill set as a whole (#2775, #3953): the chip on its pane, read again
   *  when what the sets are made of moves on disk (#4094). */
  const skillSetsMoved = useViewChanged([plane], skillSetsOfChats);
  const [chatSkillSets, setChatSkills] = useChatSkillSets(
    plane,
    chatSessions,
    studioOn,
    skillsChanged,
    skillSetsMoved,
  );
  const reloadVaults = vaults.reload;
  /** Whether the vault picker is up. */
  const [pickingVault, setPickingVault] = useState(false);
  /** A file or folder row's *Add to a chat's context*, or the explorer's picked rows', picking
   *  their chat (#1151). */
  const [addingToChat, setAddingToChat] = useState<readonly Referenced[]>();
  /** Whether the new-vault dialog is up, why the last attempt made nothing, and whether
   *  charter is making one right now — `makingWorkspace`'s three, for a vault. */
  const [makingVault, setMakingVault] = useState(false);
  const [vaultTrouble, setVaultTrouble] = useState<string>();
  const [busyVault, setBusyVault] = useState(false);
  /** A palette command's action that asks first, waiting on the operator's answer
   *  (charter-app#341). */
  const [askingAction, setAskingAction] = useState<{ extension: string; action: RowAction }>();
  /**
   * The workspace the operator is being asked about deleting, if any.
   *
   * `atRisk` is `undefined` until the core has answered and is drawn as "still reading" — an
   * empty list and an unanswered question are the two states this must never merge, because
   * one of them says "nothing would be lost". `refusal` is what the core gave the last time
   * Delete was pressed, and its presence is the only thing that makes forcing reachable.
   *
   * **`refusal` is the whole `Refused` and not its sentence** (charter-app#182). It carries the
   * at-risk list the core read *inside* the delete, and that list — never `atRisk`, which is
   * the older reading taken when this dialog opened — is what the force button is drawn from
   * once there is one.
   */
  const [removing, setRemoving] = useState<{
    workspace: string;
    atRisk?: AtRisk[];
    unreadable?: string;
    refusal?: Refused;
    busy: boolean;
  }>();
  /** **A close waiting on the person's answer about the unsaved work it would lose** (#3929,
   *  `unsavedWork.ts`): a tab's, a pane's, or a workspace's removal. */
  const [unsavedAsk, setUnsavedAsk] = useState<UnsavedAsk>();
  const unsavedAskNow = useRef(unsavedAsk);
  /** Asks about `ask`'s work, unless a question about unsaved work is up already: that one is
   *  answered first, and this ask is one not taken (`kept`), as `App`'s is (#4029). */
  const askUnsaved = useCallback((ask: UnsavedAsk | undefined) => {
    if (ask !== undefined && unsavedAskNow.current !== undefined) return ask.kept?.();
    unsavedAskNow.current = ask;
    setUnsavedAsk(ask);
  }, []);
  /** The workspace being renamed, while its dialog is up: whether charter is renaming it now,
   *  and why the last attempt renamed nothing, in the core's words (charter#367). */
  /** The workspace whose vision is being added (#3344), whether it is being saved, and why the
   *  last Save recorded nothing. */
  const [addingVision, setAddingVision] = useState<{
    workspace: string;
    busy: boolean;
    trouble?: string;
  }>();
  const [renamingWs, setRenamingWs] = useState<{
    workspace: string;
    busy: boolean;
    trouble?: string;
    /** The core's sentence naming the chats that will start a fresh conversation after it,
     *  `null` for none, `undefined` until the core has answered. */
    startsFresh?: string | null;
  }>();
  /** The same, for the pins: a pin is written by the core, so the window asks what the core
   *  now says rather than assuming its own write landed as it expected. */
  const [pinning, setPinning] = useState(0);
  // Pinned from the title bar while this project is behind another (#2916): read them again.
  useEffect(
    () =>
      onPinsChanged((changed) => {
        if (changed === plane) setPinning((asked) => asked + 1);
      }),
    [plane],
  );
  /** What the last action answered: one line, or a refusal in the words it came in. The
   *  window draws it, beside the palette that shares it. */
  const [report, setReport] = useState<Said>();
  /** Whether the core has answered what this project already has open. Until it has, "no
   *  tabs" is "not yet", which is not the same thing as "nothing is running" — and a quit
   *  decides on it. */
  const [settled, setSettled] = useState(false);
  /**
   * Where charter has just put a chat, until the plane says the same thing.
   *
   * The plane is what files a chat under a workspace — the directory it works in — and the
   * sidebar is read fresh off the disk a tick after a chat starts. For that one tick charter
   * knows perfectly well where it put the chat, because it chose the directory. Without this
   * the tab would be missing from its own strip for a frame, on the strip the operator is
   * looking at, which is the tab going missing.
   *
   * Written in the same handler that starts the chat, so the render that first draws the tab
   * already knows where it is filed — React puts both in one flush.
   *
   * **Nothing prunes it, and that is safe because a session number is never reused** — which
   * #133 asked to be verified rather than assumed, since a reused number would hand a dead
   * chat's workspace to a live one. It was verified in the core, and it rests on three
   * things together, not on the counter alone:
   *
   * - `Sessions::open` takes the number from `opened.fetch_add(1)`, an `AtomicU32` that is
   *   only ever incremented and never stored to. There is no reset path.
   * - That counter belongs to the plane's `Held`, which `Planes::open` mints once per plane
   *   and `Planes::close` removes whole. Re-opening a plane makes a new one counting from 1.
   * - And this component is mounted `key={plane}`, so a plane that was closed and opened
   *   again is a new `PlaneView` with an empty map. The counter and the map restart together.
   *
   * So the map only grows with chats STARTED from this window in one project's lifetime — an
   * entry is a number and a workspace name — and no entry it holds can ever be asked about
   * by a different chat.
   */
  const [startedIn, setStartedIn] = useState<Record<number, string>>({});
  /** **Each chat this window started, as it was started** (#5308): what {@link nameOf} calls it
   *  until its row arrives, and is never read after. Not pruned, as {@link startedIn} is not. */
  const [startedAs, setStartedAs] = useState<ReadonlyMap<number, PressedChat>>(new Map());
  /** **Which chat each task the plane has listed was a task of**, kept once it lists it no more:
   *  the plane can be read again before a task's end arrives (#5106), and until then, and after,
   *  its tab stands where its asker is filed and its end keeps its pane. `askedByNow` is what
   *  the core lists now, and stays so. */
  const [askedOnce, setAskedOnce] = useState<ReadonlyMap<number, number>>(() => new Map());
  /**
   * Where each handed-off chat came from, by session, as its tab's tooltip and its pane say it:
   * `↳ from steward 3 · platform-next` (charter-app#258). By the parent's name, never its
   * number. A chat no handoff opened has none.
   */
  const [handedFrom, setHandedFrom] = useState<Record<number, string>>({});
  /**
   * **What the Skill studio started each chat for**, by session (#5185): read off each chat the
   * core puts back (`OpenChat.studio`, kept on its record) and set as the studio starts one. Lent
   * to the studio and the chats' panes for the chats still open (`studioChats`).
   */
  const [studioLinks, setStudioLinks] = useState<Readonly<Record<number, StudioLink>>>({});
  // A studio chat whose draft came is done for good, as the core marks its record (#5185): read
  // only while the studio has a chat open here.
  const studioDrafts = useSkillDrafts(plane, 0, Object.keys(studioLinks).length > 0);
  // Taken as the read changes, while drawing, as React adjusts state to a new prop: sticky, so a
  // draft accepted or discarded since leaves its chat done.
  const [draftsSeen, setDraftsSeen] = useState(studioDrafts);
  if (draftsSeen !== studioDrafts) {
    setDraftsSeen(studioDrafts);
    setStudioLinks((was) => doneByDrafts(was, studioDrafts));
  }
  /**
   * The chats that are shell tabs (SI-5): the operator's own shell, no harness, no profile. Its
   * tab wears a terminal's mark rather than nothing, so a shell is told from a harness before
   * its name is read. Filled when one is opened here and when one comes back from the record.
   */
  const [shells, setShells] = useState<ReadonlySet<number>>(() => new Set());
  /** **The one-job programs open** (#4761): a terminal editor or a forge login, drawn beside a
   *  pane, listed nowhere, and gone when it exits. */
  const [oneJobs, setOneJobs] = useState<ReadonlySet<number>>(() => new Set());
  /** The shells and one-job programs as of the last render, for what reads them outside one: a
   *  program's exit, End… and Close pane (#4761). */
  const shellsRef = useRef({ shells, oneJobs });
  useEffect(() => {
    shellsRef.current = { shells, oneJobs };
  }, [shells, oneJobs]);
  /**
   * A harness started by hand in a shell tab, by that tab's session, as its banner says it
   * (ADR 0062). The core says so over `harness-by-hand`; the banner stays until it is dismissed
   * or answered.
   */
  const [byHand, setByHand] = useState<Record<number, ByHandNote>>({});
  /** What each chat's start found to say, by session, until it is dismissed (ADR 0085). */
  const [startNotes, setStartNotes] = useState<Record<number, StartNotes>>({});
  /** What each chat's sandbox blocked, by session, for the Notice on its tab (#1338). */
  const {
    blocks: sandboxBlocks,
    dismiss: dropBlock,
    answered: blockAnswered,
    letGo: letBlockGo,
    take: takeBlocks,
  } = useSandboxBlocks(plane);
  /** **A note cleared here is cleared in every window of the project** (#5225): each hears what
   *  its chats say, so another may hold it for the chat coming back to it. */
  const noteCleared = useCallback(
    (session: number, note: NoteCleared["note"]) => clearedHere(plane, session, note),
    [plane],
  );
  /** A block put away. */
  const dismissBlock = useCallback(
    (session: number, block: HeldBlock) => {
      dropBlock(session, block);
      noteCleared(session, { kind: "block", block });
    },
    [dropBlock, noteCleared],
  );
  /** What one answer to several tasks' same block allowed, by the session they are tasks of,
   *  until it is put away (#1508). */
  const [taskBlocksAnswered, setTaskBlocksAnswered] = useState<
    readonly { at: number; session: number; target: string; said: string }[]
  >([]);
  /** The tab that was in front on each workspace's strip, so coming back to a workspace
   *  comes back to the chat that was on screen there rather than to its first. */
  const lastFront = useRef<Record<string, number>>({});
  /**
   * What this operator has pinned in this project (ADR 0039).
   *
   * **Two states and not one, because they are two stores** (ADR 0040). The workspaces come
   * from the machine store, which is where an arrangement of things the store already names
   * belongs; the chats come from the plane's own `.charter/app/reopen.json`, because ADR 0034
   * forbids a chat's name outside a plane. A design that held one "pins" object would be the
   * design ADR 0039 predicted would discover this in review.
   */
  const [storePins, setStorePins] = useState<string[]>([]);
  /**
   * **Made here, so pinned at once** (#1926, ADR 0054): a workspace this window has just made,
   * counted pinned from the moment `workspace_create` answers until the store's answer names
   * it. The window hears of a new workspace twice, the disk watcher's read of the plane and the
   * pin `makeWorkspace` asks for, and the read can land first; without this, the strip draws
   * the new workspace unpinned until the pin does. Let go when the store names it, when the
   * pin is refused, or when it is unpinned.
   */
  const [madePins, setMadePins] = useState<string[]>([]);
  /**
   * **The pins' order as the person just put them** (#3128), from a chip's drag or its Move left
   * or right, until the store's answer after the write says it: so a dropped chip stays where it
   * was dropped for the round trip instead of going back and coming again. Let go when the write
   * is refused, or when the store answers with none of this window's writes still on the way.
   */
  const [arranged, setArranged] = useState<readonly string[]>();
  /** How many of this window's arrangements are still being written (`arrangePins`), and how
   *  many have been: a read of the store asked before the last write landed is not its word. */
  const arranging = useRef({ writing: 0, written: 0 });
  const pinnedWorkspaces = useMemo(() => {
    const held = [...storePins, ...madePins.filter((name) => !storePins.includes(name))];
    if (arranged === undefined) return held;
    return [
      ...arranged.filter((name) => held.includes(name)),
      ...held.filter((name) => !arranged.includes(name)),
    ];
  }, [arranged, madePins, storePins]);
  /**
   * **Dormant pins** (V91c as amended, NO-1): pins whose workspace is gone. They stay in the
   * machine store, in their place in the order, and are drawn as dormant chips that open nothing
   * (#3651); when the workspace comes back (a pull, another branch, a relaunch) the store's
   * answer draws them as pins again, in place.
   */
  const [dormantPins, setDormantPins] = useState<string[]>([]);
  /** Every pin, dormant ones included, in the store's order (`plane_pins`' `order`). */
  const [pinOrder, setPinOrder] = useState<string[]>([]);
  /**
   * **Dormant pins the operator forgot this run**, each with the pins that came before it in the
   * order, nearest first: its Notice's Undo puts it back after the nearest of them still pinned,
   * so several Undos in any order, or an unpin made in between, give back the order they had.
   */
  const [forgottenPins, setForgottenPins] = useState<{ name: string; after: string[] }[]>([]);
  /**
   * Whether the core has said which view tabs the record put back. Until it has, the window
   * says nothing about its own: an empty list sent first would be written over the record
   * before it was read.
   */
  const [viewsHeard, setViewsHeard] = useState(false);
  /**
   * The spot the explorer has picked, and the workspace it was picked in.
   *
   * Both together, so that moving to another workspace goes back to that workspace's own
   * directory rather than leaving the next chat pointed at a piece of the workspace the
   * operator has just left. Derived below rather than cleared in an effect: a `setState`
   * from inside an effect is a second render, and the answer is already here.
   */
  const [pickedSpot, setPickedSpot] = useState<{ workspace: string; spot: Spot }>();
  /**
   * The branch the explorer is focused on — its cockpit (FM-5, #1108) — with the workspace it
   * is in. Kept when the window moves to another workspace, and drawn again on coming back,
   * as the pick is; remembered with the window's views (`window_focus`), so one window can
   * stay on one branch across a relaunch.
   */
  const [focusedBranch, setFocusedBranch] = useState<Place>();
  /**
   * The panel row whose card is open on the right-hand region, if any, as
   * `<panel key>/<row key>` — `charter/personas/steward`, `charter/todos/<slug>`,
   * `ext/acme/reviews/<key>`.
   *
   * **Held here rather than in the row that draws it**, for charter-app#174's reason: a persona's
   * card was a catalogue row the palette ran. A persona opens its own TAB now (`showView`,
   * the operator's ruling of 2026-09-23), so what this still holds is the popover card of a row
   * that has one — a todo's, a contributed row's.
   *
   * **It was `shownPersona` and is a row key now**, because the panels are contributions and
   * every contributed panel's rows open the same way. A second piece of window state per panel
   * would be the special case the contract exists to remove — and it could not exist at all for
   * a panel nobody has written yet.
   */
  const [shownRow, setShownRow] = useState<string>();
  /** How the window is laid out — which regions are drawn, on which side, in what order and
   *  how big (ADR 0038). Data rather than the shape of the JSX below; `regions.ts` says why. */
  const {
    arrangement,
    toggle,
    resized,
    reset: resetSize,
    split: splitHalves,
    pick,
    show: showSide,
    moveView,
  } = useArrangement(plane);
  /**
   * **What the Search view asks** (#1676): the Search tab's own question, held here because the
   * view is in the side and not in a tab. Nothing until the view is first shown, so a window
   * that never searches never draws it.
   */
  const [sideSearch, setSideSearch] = useState<ViewRef>();
  /** Where the window is focused, for the Search view to search as narrow as it (below). */
  const searchFocus = useRef<{ branch?: Place; workspace?: string }>({});
  /** A view shown by its key or its palette row, and the keyboard given to it once it is drawn:
   *  the stop of its first tree drawn, as an editor's ⌘⇧E lands in the explorer (#1673) — past
   *  the headings of Explorer's sections (#1677) — or its first stop, in a view with no tree;
   *  Search's box.
   *
   *  **Search searches where the window is** (#1676), as ⌘⇧F's Search tab did: the branch
   *  nearest the person, else the workspace in front, else the project. What was typed and the
   *  three switches are kept, as an editor's search view keeps them. */
  const searchHere = useCallback(
    () =>
      setSideSearch((was) => {
        const kept = was === undefined ? undefined : searchOf(was);
        const here = searchFromFocus(searchFocus.current.branch, searchFocus.current.workspace);
        return searchView({
          ...here,
          query: kept?.query ?? "",
          matching: kept?.matching ?? here.matching,
        });
      }),
    [],
  );
  /** What ⌘⇧F runs while the keyboard is in Explorer's files (#3343), kept by Explorer. */
  const explorerSearchKey = useRef<SearchKeyHere | undefined>(undefined);
  const keepExplorerSearchKey = useCallback((here: SearchKeyHere | undefined) => {
    explorerSearchKey.current = here;
  }, []);
  /** **Search contents, from Explorer's filter** (#3343): the Search view on the scope the filter
   *  found in, Branch or Workspace, with what it held; the switches as they were left. */
  const searchContents = useCallback(
    (kind: "branch" | "workspace", at: Place, query: string) => {
      setSideSearch((was) => {
        const kept = was === undefined ? undefined : searchOf(was);
        const here = searchFromFocus(at, at.workspace);
        return searchView({
          ...here,
          kind,
          query: query !== "" ? query : (kept?.query ?? ""),
          matching: kept?.matching ?? here.matching,
        });
      });
      showSide("search");
      requestAnimationFrame(() => giveViewTheKeyboard("search"));
    },
    [showSide],
  );
  const showSideView = useCallback(
    (view: ViewId) => {
      if (view === "search") searchHere();
      showSide(view);
      requestAnimationFrame(() => giveViewTheKeyboard(view));
    },
    [searchHere, showSide],
  );
  /** **Show in Explorer's ask** (#3266, #3308, the Skills view): the folder, in the project's own
   *  files or a clone's, with a count that moves on each ask, so asking for the same folder
   *  again shows it again. */
  const [explorerReveal, setExplorerReveal] = useState<ExplorerPlace & { asked: number }>();
  const revealInExplorer = useCallback(
    (place: ExplorerPlace) => {
      setExplorerReveal((was) => ({ ...place, asked: (was?.asked ?? 0) + 1 }));
      showSideView("explorer");
    },
    [showSideView],
  );
  /** "Move to panel" or "Move to sidebar" (#1740): the view moved, open where it went, and the
   *  keyboard in it, as its key leaves it — the icon the menu was opened on is gone from its
   *  bar. */
  const moveSideView = useCallback(
    (view: ViewId, to: ViewPlace) => {
      moveView(view, to);
      requestAnimationFrame(() => giveViewTheKeyboard(view));
    },
    [moveView],
  );

  /** Every chat the core lists, by session, as of the last read: what {@link showChat} opens a
   *  tab from for a chat that has none. A ref, so a read of the list does not make a new
   *  `showChat` for every row and catalogue entry holding one. Kept up to date below, where
   *  the list is read. */
  const listedNow = useRef<ReadonlyMap<number, OpenChat>>(new Map());
  /** Which chat a task is a task of, as the core lists it now: what says which tab a task is
   *  shown in. Task links only (V100-69). */
  const askedByNow = useCallback<AskedBy>((session) => {
    const from = listedNow.current.get(session)?.from;
    return from?.task ? from.chat : undefined;
  }, []);
  // The arrangement as it is right now, so that what a button does is decided here and not
  // inside a state update. React may run an update again, and a session must not be opened or
  // ended twice because it did.
  const now = useRef(tabs);
  /**
   * **⌘⇧J: the focused chat's own tasks** (#5058, ruled on #4699, #5341): whose tasks the
   * keyboard asked for, the pane they are asked in, the focused one, and a count of the asks.
   * The state button on that pane's head opens its menu on a rise of the count (`ChatTasks`), so
   * being drawn is never being asked, and ⌘⇧J and the head open one menu. Asked again while it
   * is open, it closes.
   */
  const [chatTasksAsked, setChatTasksAsked] = useState<{
    session: number;
    tab: number;
    pane: number;
    count: number;
  }>();
  const showChatTasks = useCallback((session: number) => {
    const front = now.current.inFront;
    if (front === undefined) return;
    const pane = now.current.byId[front].focused;
    setChatTasksAsked((was) => ({ session, tab: front, pane, count: (was?.count ?? 0) + 1 }));
  }, []);
  /** The chats a scenario spec pretends (`e2eTasks.ts`, none in a shipped build): the core has
   *  none of them, and is never told one is in front. */
  const pretendedNow = useRef<ReadonlySet<number>>(new Set());
  useEffect(() => {
    pretendedNow.current = new Set(pretended.map((one) => one.chat.session));
  }, [pretended]);
  /** Tells the core the window's tabs as they are in `tabs` (`window_layout`, #4751), when
   *  they differ from what was last said. Set below, once what it reads is known. */
  const sayLayout = useRef<(tabs: Tabs) => void>(() => undefined);
  /**
   * **Whether a drag holds a tab forward** (#5294): resting on a tab, or pressing one in placement
   * mode, draws it in front as a preview. Until the drag is over the core is told nothing of it:
   * not the layout, not the view tabs, and no strip remembers it as its front tab. The ref is what
   * `change` reads in the same call; the state reruns the effects that tell, so what they held
   * back is told as it ends.
   */
  const heldForward = useRef(false);
  const [holdingForward, setHoldingForward] = useState(false);
  /** What was last said of the tabs, as JSON: a change to a terminal or a notice is not one. */
  const layoutSaid = useRef<string | undefined>(undefined);
  /** The layout the core last refused, as JSON: never said again until it has taken another.
   *  A refusal is the core's verdict on that layout (a chat in two panes, a chat not open), so
   *  saying it again unchanged would only be refused again. */
  const layoutRefused = useRef<string | undefined>(undefined);
  /** Whether the record's tabs have been heard: tabs said before would be the empty window's,
   *  written over the record it is about to read. */
  const layoutHeard = useRef(false);
  /** **Whether the project has a tab window** (#4768), as the record and the core's last move
   *  said: a press asks the core where the chat is only then. */
  const otherWindows = useRef(false);
  /** **Every window of the project** (#4768): its project window (`0`) and each tab window, as
   *  the record and the core's last move said. What Move to window ▸ lists, less this one. */
  const [windowsHere, setWindowsHere] = useState<readonly number[]>([0]);
  /** **The chats another window of the project shows** (#5225), as the record, the core's last
   *  move and a tab window's last report said: what they have to say is theirs to say. */
  const [shownElsewhere, setShownElsewhere] = useState<ReadonlySet<number>>(() => new Set());
  /** **Whether the project has this one window** (#4768, #5108): neither a tab window nor a
   *  project window with tab windows. Its press goes to its focused pane, asking nothing. */
  const oneWindow = useCallback(
    () => tabWindow === undefined && !otherWindows.current,
    [tabWindow],
  );
  const windowsOf = useCallback(
    (laid: LaidOut) => {
      const each = [...new Set([0, ...laid.tabs.map((one) => one.window)])].sort((a, b) => a - b);
      otherWindows.current = each.length > 1;
      setWindowsHere((was) => (was.join() === each.join() ? was : each));
      const elsewhere = chatsElsewhere(laid, tabWindow ?? 0);
      setShownElsewhere((was) =>
        was.size === elsewhere.size && [...was].every((one) => elsewhere.has(one))
          ? was
          : elsewhere,
      );
    },
    [tabWindow],
  );
  /** What a tab is called when a content is its first pane (`paneDrop.firstPaneNameOf`), as the
   *  drop context below names it, once it is made: nothing renames a tab before then. */
  const firstPaneNameNow = useRef<(content: Content, before: Tabs) => TabName>(undefined);
  const change = useCallback(
    (how: (tabs: Tabs) => Tabs): Tabs => {
      // **A tab whose first pane is another is named for it** (#5342, #5308): its first chat
      // closed or moved out, so it takes the next one's name. One rule, for every change.
      const changed = how(now.current);
      const nameOf = firstPaneNameNow.current;
      const done =
        nameOf === undefined ? changed : namedForFirstPanes(now.current, changed, nameOf);
      // **The pane in front was used** (#4757): what a press with a view focused goes by.
      const next = done === now.current ? done : noteUsed(done);
      // **A chat given a new pane keeps what its band remembered** (#3271): its Notices were
      // drawn in the band of the pane it was at home in, and are drawn in the new pane's now.
      // Carried before anything is drawn, so the new band's first read reads on from there.
      for (const { from, to } of panesMovedInto(now.current, next, askedByNow)) {
        carryPaneBand(paneBand(plane, from), paneBand(plane, to));
      }
      now.current = next;
      setTabs(next);
      // **The layout is told in the same call** (#4751): a press that brings a chat on screen
      // says so before it returns, so a task the person pressed onto is held from that moment.
      // Only what differs from what was last said is sent (`sayLayout`).
      sayLayout.current(next);
      return next;
    },
    [askedByNow, plane],
  );

  /**
   * **Takes the number the chat being started is named with** (#4758), at once, so two starts
   * that overlap are never given the same one. A count and nothing on screen: no layout is told.
   */
  const takeChatNumber = useCallback((): number => {
    const number = newChatNumber(now.current);
    now.current = chatNamed(now.current, number);
    setTabs(now.current);
    return number;
  }, []);

  // What the core already has open in this project, which at a launch is the record put back
  // before there was a window. The window draws them; it never ends them — a reload during
  // development, or a crash in the window, would otherwise take the day's sessions with it.
  //
  // Once per project, which a ref is what makes true: React runs an effect twice in
  // development, and a second pass would draw every chat again. The ref is this component's,
  // so a project that is closed and opened again is a fresh one and asks again — which it
  // must, because the core still holds whatever it put back.
  const adopted = useRef(false);
  /** Settles the notes a relaunch says about its chats (`resumed`, `guessed`, `fresh`,
   *  `untaken`) against the chats the core put back. */
  const settleChatNotes = useCallback(
    (open: readonly OpenChat[]) => {
      for (const family of CHAT_NOTES) {
        settleNotices(
          family,
          open.flatMap((one) => chatNote(family, one) ?? []),
        );
      }
    },
    [settleNotices],
  );
  /** **What came back, as a layout's tabs are drawn from it** (#4751, #4768): each open chat by
   *  its names, and each view as it is named now (a studio's as the one studio). */
  const reopeningOf = useCallback(
    (named: ReadonlyMap<number, OpenChat>): Reopening => ({
      chat: (session) => {
        const chat = named.get(session);
        return chat === undefined
          ? undefined
          : { name: chat.name, who: whoOf(chat.persona, chat.harness), label: chat.label };
      },
      view: (view, title) => {
        const now = viewNamedNow(view, title);
        const studio = isSkillStudio(now.view) || studioOfSkillTab(now.view) !== undefined;
        return {
          view: studioPutBack(plane, now.view),
          title: studio ? SKILL_STUDIO_TITLE : now.title,
        };
      },
    }),
    [plane],
  );
  useEffect(() => {
    if (adopted.current) return;
    adopted.current = true;
    void Promise.all([
      commands.openedChats(plane).catch(() => undefined),
      // The view tabs the record put back, beside the chats. **Asked for, never started**: a
      // view has no program of charter's, and an extension's view is not asked anything until
      // the operator presses for it (`Views.tsx`), so this is a list of tabs and nothing else.
      commands.reopenedViews(plane).catch(() => undefined),
      // The branch the window's explorer was focused on (FM-5), kept with the views.
      commands.reopenedFocus(plane).catch(() => undefined),
      // Every tab as the record had it (#4751), or nothing for a record written before tabs
      // were recorded, which is put back the old way below.
      commands.reopenedLayout(plane).catch(() => undefined),
    ])
      .then(([answer, viewAnswer, focusAnswer, layoutAnswer]) => {
        setSettled(true);
        void commands
          // A window that cannot ask, or is answered with nothing, simply says nothing.
          .chatsThatWouldNotStart(plane)
          .then((trouble) => setWouldNotStart(trouble.status === "ok" ? (trouble.data ?? []) : []))
          .catch(() => undefined);
        const open = answer?.status === "ok" ? (answer.data ?? []) : [];
        // The ones the launch has still to start (`restoring.rs`): drawn with the rest, each
        // in its place, and filed where the core says its folder is until the sidebar lists it.
        const coming = open.filter(isRestoring);
        if (coming.length > 0) {
          restoringNow.current = new Set(coming.map((chat) => chat.session));
          restoreSeen.current = seenFirst(open);
          setRestoringChats(restoringNow.current);
          setStartedIn((was) => ({
            ...was,
            ...Object.fromEntries(
              coming.map((chat) => [chat.session, chat.restoring?.workspace ?? OUTSIDE]),
            ),
          }));
          setRestoreLooks((was) => was + 1);
        }
        // What the relaunch did to each chat is the core's answer: a dismissed note about a chat
        // that came back some other way, or not at all, is let go (`dismissals.ts`). Once every
        // chat has started: one still restoring has not said how it came back.
        if (answer?.status === "ok" && coming.length === 0) settleChatNotes(open);
        // Each as it is named now: a record an older charter wrote can hold a view since renamed.
        const back = (viewAnswer?.status === "ok" ? (viewAnswer.data ?? []) : []).map((tab) => {
          const now = viewNamedNow(refOf(tab), tab.title);
          return { ...tab, ...now.view, title: now.title };
        });
        if (open.length > 0) {
          setReopened(open);
          setHandedFrom((was) => ({
            ...was,
            ...Object.fromEntries(
              open.flatMap((chat) => {
                const note = handedFromNote(chat.from);
                return note ? [[chat.session, note]] : [];
              }),
            ),
          }));
          setStudioLinks((was) => ({
            ...was,
            ...Object.fromEntries(
              open.flatMap((chat) => (chat.studio ? [[chat.session, chat.studio]] : [])),
            ),
          }));
          // A shell comes back a shell: on no profile, running no harness. One still restoring
          // says once it has started (`readRestored`): its harness is its start's to say.
          setShells(
            new Set(
              open
                .filter((chat) => !isRestoring(chat) && isShell(chat))
                .map((chat) => chat.session),
            ),
          );
          // A one-job program still running at a reload of the window: no launch puts one back,
          // since the record never holds one (#4761).
          setOneJobs(new Set(open.filter((chat) => chat.one_job).map((chat) => chat.session)));
        }
        // **What a record from before tabs were recorded says is pinned** (#4762, #4935): each
        // chat's pin and each view tab's, read once, at this upgrade, onto the tab that opens on
        // it. A record with tabs says the pin on the tab, and the chat's own has retired. A
        // studio tab an older launch kept per scope is pinned as the one studio tab (#3681).
        const laid = layoutAnswer?.status === "ok" ? layoutAnswer.data : null;
        const pinnedBefore: PinnedBefore = {
          chats: new Set(
            laid == null ? open.filter((chat) => chat.pinned).map((chat) => chat.session) : [],
          ),
          views: new Set(
            back
              .filter((view) => view.pinned)
              .map((view) => {
                // Only a studio's own tab (or a skill's old one) is the one studio tab.
                const kept = studioOfSkillTab(refOf(view)) ?? refOf(view);
                return viewKey(isSkillStudio(kept) ? studioOpening(kept, () => true).tab : kept);
              }),
          ),
        };
        const focusedBack = focusAnswer?.status === "ok" ? focusAnswer.data : null;
        if (focusedBack !== null && focusedBack !== undefined)
          setFocusedBranch({
            workspace: focusedBack.workspace,
            repo: focusedBack.repo,
            piece: focusedBack.piece,
          });
        // **The record's tabs, where it has them** (#4751): each split with its sizes, its views
        // and each strip's front tab, as the window left them. **A launch draws tabs from the
        // stored layout only** (#4758): a chat no pane holds runs on in the sidebar, and never
        // gets a tab it did not have. A record from before tabs were recorded is read into a
        // layout by the core, once (`reopen::upgrade`).
        if (laid !== null && laid !== undefined) {
          const named = new Map(open.map((chat) => [chat.session, chat]));
          const back = tabsOf(laid, noTabs(), reopeningOf(named), tabWindow ?? 0);
          // **A tab window draws its own tabs and nothing more** (#4768): every other chat is
          // the project window's.
          if (tabWindow !== undefined) {
            windowsOf(laid);
            layoutHeard.current = true;
            if (back.tabs.order.length > 0)
              change(() =>
                back.inFront === undefined ? back.tabs : selectTab(back.tabs, back.inFront),
              );
            setViewsHeard(true);
            return;
          }
          const all = back.tabs;
          lastFront.current = { ...lastFront.current, ...back.fronts };
          const inFront = back.inFront;
          layoutHeard.current = true;
          // **A one-job program a reload of the window found running** opens beside the focused
          // pane again (#4761): the layout the core holds never has its pane.
          const fronted = inFront === undefined ? all : selectTab(all, inFront);
          const withJobs = open
            .filter((chat) => chat.one_job)
            .reduce((tabs, chat) => openOneJob(tabs, chat.session, chat.name), fronted);
          if (withJobs.order.length > 0) change(() => withJobs);
          setViewsHeard(true);
          // **Each tab window the record names comes back** (#4768), in its place on screen.
          windowsOf(laid);
          if (otherWindows.current) void commands.tabWindowsBack(plane).catch(() => undefined);
          return;
        }
        // A tab window of a record with no layout has nothing of its own to draw.
        if (tabWindow !== undefined) {
          layoutHeard.current = true;
          setViewsHeard(true);
          return;
        }
        // **No layout: no chat gets a tab** (#4758). The core reads every record into a layout,
        // so this is a launch with nothing recorded, or an older core's answer: its chats run
        // in the sidebar, and only its view tabs come back.
        const chats = noTabs();
        // Each view at the place it had, in the order of those places, so a view recorded at 2
        // lands at 2 after the one at 1 is already in.
        const drawn = [...back]
          .sort((one, other) => one.at - other.at)
          .reduce(
            (tabs, view) =>
              putViewBack(
                tabs,
                studioPutBack(plane, refOf(view)),
                isSkillStudio(refOf(view)) || studioOfSkillTab(refOf(view)) !== undefined
                  ? SKILL_STUDIO_TITLE
                  : view.title,
                view.workspace ?? OUTSIDE,
                view.at,
                view.split ?? undefined,
              ),
            chats,
          );
        const front = open.find((chat) => chat.in_front);
        const frontView = back.find((view) => view.active);
        const tasks = new Map(
          open.map((chat) => [chat.session, chat.from?.task ? chat.from.chat : undefined]),
        );
        const askedBy: AskedBy = (session) => tasks.get(session);
        // A view tab's pin, from a record of view tabs alone.
        const shownBack = pinnedAsBefore(drawn, pinnedBefore);
        // **A view tab in front comes first**: the chat the core holds in front can be one
        // beside that view (`chatInFrontOf`), and that chat's own tab is not the one the person
        // left in front.
        const viewInFront =
          frontView === undefined
            ? undefined
            : drawn.order.find((id) => {
                const lead = contentsOf(drawn, id)[0]?.content;
                return lead?.kind === "view" && viewKey(lead.view) === viewKey(refOf(frontView));
              });
        const inFront =
          viewInFront ??
          (front !== undefined ? homeOf(shownBack, front.session, askedBy)?.tab : undefined);
        layoutHeard.current = true;
        if (shownBack.order.length > 0)
          change(() => (inFront === undefined ? shownBack : selectTab(shownBack, inFront)));
        // Only now may the window say what view tabs it has: saying it before this point would
        // write an empty list over the record it is about to read.
        setViewsHeard(true);
      })
      // Nothing open is the ordinary first launch, and a window that cannot ask is still
      // a window the operator can open a chat in.
      .catch(() => {
        setSettled(true);
        layoutHeard.current = true;
        setViewsHeard(true);
      });
  }, [change, plane, reopeningOf, settleChatNotes, tabWindow, windowsOf]);

  /**
   * The window's view tabs, told to the core whenever they change — so the record brings them
   * back at the next launch, as it brings back chats (ADR 0043, as amended).
   *
   * **The whole list, and only when it differs from what was last said.** The core writes the
   * record when what it holds changes and not otherwise (`Chats::hold_views`), and this keeps a
   * chat's keystrokes and splits from being a command each.
   */
  const lastViewsSaid = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (!viewsHeard || holdingForward) return;
    const said = viewTabsOf(tabs);
    const text = JSON.stringify(said);
    if (text === lastViewsSaid.current) return;
    lastViewsSaid.current = text;
    void commands.windowViews(plane, said).catch(() => undefined);
  }, [holdingForward, plane, tabs, viewsHeard]);

  /** The branch the explorer is focused on (FM-5), told to the core when it changes, after the
   *  record is heard for the views' reason: a focus said first would write over the record's. */
  const lastFocusSaid = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (!viewsHeard) return;
    const said = focusedBranch ?? null;
    const text = JSON.stringify(said);
    if (text === lastFocusSaid.current) return;
    lastFocusSaid.current = text;
    void commands.windowFocus(plane, said).catch(() => undefined);
  }, [focusedBranch, plane, viewsHeard]);

  /**
   * The order the chats are in across every strip, told to the core whenever it changes — so
   * the record lists them in it, and the next launch and a reloaded window put them back in it
   * (SI-6). A tab's chats in its panes' order, each once.
   *
   * **After the record is heard, and only when it differs**, for the view tabs' two reasons
   * above: an order said before the adoption would be the empty strip's, and most changes to
   * `tabs` are not to the order.
   */
  const lastOrderSaid = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (!viewsHeard) return;
    const sessions = [
      ...new Set(tabs.order.flatMap((id) => panesOf(tabs, id).map((pane) => pane.session))),
    ];
    const text = JSON.stringify(sessions);
    if (text === lastOrderSaid.current) return;
    lastOrderSaid.current = text;
    void commands.chatOrder(plane, sessions).catch(() => undefined);
  }, [plane, tabs, viewsHeard]);

  // A chat a handoff opened (charter-app#204): the core has started it, and this files it on
  // its workspace. **It starts in the sidebar only and takes no pane** (#4758; spec story 23):
  // a chat the person did not start never rearranges the screen, and the window is not raised.
  useEffect(() => {
    const listening = listen<Arrived>("handoff-arrived", (event) => {
      const arrived = event.payload;
      // A chat arriving is the project window's to put on its strip, never a tab window's.
      if (arrived.plane !== plane || tabWindow !== undefined) return;
      // Already drawn: the adoption above can race the event and draw it first.
      if (alreadyShows(now.current, arrived.session)) return;
      // A task dispatched by a chat at the project's root is in no workspace, as that chat is.
      const filed = arrived.workspace ?? OUTSIDE;
      setStartedIn((was) => ({ ...was, [arrived.session]: filed }));
      const note = handedFromNote(arrived.from);
      if (note) setHandedFrom((was) => ({ ...was, [arrived.session]: note }));
      // **Announced, never put on screen** (#4759): a handoff hangs a card on its workspace's
      // chip, or is listed in the Notices centre where there is none, and so does a task that
      // can't report asks, which can't say when it needs the person. Any other task is its
      // asking chat's to read, and a chat the person started is theirs: the core
      // says which (`announced`), as its record's "new" mark does. The card takes no keyboard.
      if (arrived.announced === true) {
        const who = whoOf(arrived.persona, arrived.harness);
        setReport(
          chatArrived(
            {
              chat: arrived.label ?? (who === null ? arrived.name : `${who} ${arrived.name}`),
              from: arrived.from?.name ?? "the person",
              workspace: filed,
              session: arrived.session,
            },
            {
              cantReport: cantReportAsks(arrived.from, arrived.cant_report_asks),
              // Said to the window as an event of its own, which the press below hears: the
              // result is state, and holds no function of the window's beyond this one.
              onGo: () => goToArrived(plane, arrived.session),
            },
          ),
        );
      }
      // Filing it above is what reads the list again, where its row is.
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane, tabWindow]);

  /**
   * The persona chats that still owe a report. A task's report is followed by the end of its
   * turn, and a task that dies ends: either moves `tasksMoved`, and the sidebar is read again,
   * which is where a task's reported mark comes from. **A turn beginning moves nothing**
   * (#1468): no report comes with it. Empty while no chat has a task in flight, so nothing is
   * read again for it then.
   */
  const unreported = useMemo(
    () =>
      sidebar === undefined
        ? []
        : [...sidebar.unfiled, ...sidebar.workspaces.flatMap((ws) => ws.chats)]
            .filter(
              (chat) =>
                chat.from?.task === true &&
                chat.from.reported !== true &&
                chat.from.unreported !== true,
            )
            .map((chat) => chat.session),
    [sidebar],
  );
  const tasksMoved = useTurnsEnded(chats, unreported);
  // A sandbox command of the window's returned, such as a browser site allowed or removed
  // (#2364): each chat's tab says how many sites only its browser reaches, so it is read again.
  const sandboxMoved = useSandboxCommands();

  // The sidebar is read from the plane, and re-read whenever the chats change: the plane is a
  // directory the operator also edits by hand and another charter process writes, so there is
  // nothing to invalidate a cache of it. `openHere` is the dependency because opening or ending
  // a chat is what this window can change about the answer, and `sidebarChanges` because the
  // core says when something else changed it (charter-app#264) — a change of a kind the sidebar
  // reads (FD-10). **Not `tabs`** (#4729): a switch between chats already open changes the tabs
  // and not the answer, and each switch read the plane again and drew the window again.
  //
  // **An answer equal to the one held is not drawn** (#4729): the state keeps the object it
  // holds, so nothing that reads the sidebar is drawn again for a read that found no change.
  // Equal is structural (`dequal`): it walks the two answers once and does not care in which
  // order a key was written, where comparing them as JSON serialised both whole each time.
  //
  // **Only the newest read is drawn.** The core reads the sidebar off the window's thread
  // (SC-2), so two reads can answer in either order, and one asked before a workspace was
  // renamed must not put it back after the read that saw the rename. `gone` is set when a
  // newer read is asked, which is when this effect runs again.
  /**
   * **The chats ended on purpose in a pane, kept there as ended** (#5019, spec #4750 story 62):
   * End…, a Smart close once its record is written, and a task's End… leave the pane on the
   * ended band, as a program that exits does (#4756). The core no longer lists such a chat, so
   * what its band says is held here as it ends: its name, and the record a Smart close wrote,
   * which is its Resume. A ref beside the state, since the close that reads it is made in the
   * same handler as the end.
   */
  const [endedHere, setEndedHere] = useState<ReadonlyMap<number, EndedHere>>(() => new Map());
  const endedHereNow = useRef<Map<number, EndedHere>>(new Map());
  /**
   * **The tasks that ended in a pane, kept there as ended** (#4757): a press put each in its
   * pane, and its end does not take the pane away (`closeChat`'s `keeps`). The core is told
   * the pane is empty (`sayLayout`'s `told`), since it has no such chat open to hold. Each with
   * the workspace it was filed in as it ended (#5106): the plane lists it no more, and its tab
   * stays on that workspace's strip.
   */
  const endedInPanes = useRef<Map<number, string>>(new Map());
  /** The same workspaces, as the window draws from them: `filedIn` reads these. */
  const [endedFiled, setEndedFiled] = useState<ReadonlyMap<number, string>>(() => new Map());
  // **A chat ended on purpose counts as gone** (#5019): its pane keeps it, and the core no longer
  // lists it, so its ending is a change to what is open here.
  const openHere = useMemo(
    () => `${openChatsKey(tabs)}|${[...endedHere.keys()].sort((a, b) => a - b).join(",")}`,
    [endedHere, tabs],
  );
  useEffect(() => {
    let gone = false;
    void commands
      .planeSidebar(plane)
      .then((answer) => {
        if (gone) return;
        // Only an `ok` answer WITH a body is used. Both halves are load-bearing: the
        // state updater below runs on the NEXT render, outside this promise, so nothing
        // here catches a throw from it — and an answer whose `data` is absent reads as
        // `ok` all the same. The window must not go blank because one command answered
        // oddly; it has its panes to draw.
        const next = answer.status === "ok" ? answer.data : undefined;
        if (!next?.workspaces) {
          setSidebar(undefined);
          setSidebarUnread(true);
          return;
        }
        setSidebarUnread(false);
        // An equal answer moves nothing the finished tasks are read on, so they are read
        // again here: a finished row can change while the sidebar does not (Clear finished).
        const held = sidebarHeld.current;
        if (held !== undefined && dequal(held, next)) finishedQuietly.current();
        setSidebar((held) => (held !== undefined && dequal(held, next) ? held : next));
        setAskedOnce((was) => withAskersOf(was, next));
        // Focus follows the plane rather than being guessed: whichever workspace was picked,
        // while it still exists — and otherwise **the workspace of the chat in front**,
        // because the strip below shows that workspace's chats and a strip that opened on
        // another one would be hiding the chat the operator is looking at (ADR 0036). Only
        // then the first workspace, which is what a launch with nothing open lands on.
        const here = (session: number) =>
          next.workspaces.find((ws) => ws.chats.some((chat) => chat.session === session))?.name ??
          (next.unfiled.some((chat) => chat.session === session)
            ? OUTSIDE
            : (startedIn[session] ?? OUTSIDE));
        const front = now.current.inFront;
        const lead = front === undefined ? undefined : contentsOf(now.current, front)[0]?.content;
        const ofFront =
          lead === undefined
            ? undefined
            : lead.kind === "session"
              ? here(lead.session)
              : lead.workspace;
        // The plane root always stands: it is the plane's own directory (SI-1).
        const stands = (name: string) =>
          name === OUTSIDE || next.workspaces.some((ws) => ws.name === name);
        // Never a workspace that has gone: the front tab can be an empty tab of the one that
        // did (#4758), and it goes below.
        setPicked((current) =>
          current !== undefined && stands(current)
            ? current
            : ofFront !== undefined && stands(ofFront)
              ? ofFront
              : (next.workspaces[0]?.name ?? OUTSIDE),
        );
        // A view tab is on the strip it was opened from, which it carries; when the plane stops
        // having that workspace nothing else would move it, and it would be on a strip that is
        // never drawn. It goes where a chat working in no workspace goes.
        // An empty tab holds nothing but its strip, so it goes with it when the workspace goes
        // (#4758), and the window lands where the rule above puts it. **Only a workspace this
        // read no longer lists, that the last one did**, or any the first read does not list:
        // a strip the window was already on unlisted is not a workspace that has just gone,
        // and an empty tab there may be the one New chat is about to fill.
        const wasListed = (name: string) =>
          held === undefined || held.workspaces.some((ws) => ws.name === name);
        change((tabs) =>
          dropEmptyTabsOfGoneWorkspaces(
            refileViews(tabs, (name) => next.workspaces.some((ws) => ws.name === name), OUTSIDE),
            (name) => stands(name) || !wasListed(name),
          ),
        );
      })
      // A window with no readable plane still runs its panes; the header already says so.
      .catch(() => {
        if (gone) return;
        setSidebar(undefined);
        setSidebarUnread(true);
      });
    return () => {
      gone = true;
    };
  }, [
    change,
    sidebarChanges,
    openHere,
    plane,
    replan,
    sandboxMoved,
    startedIn,
    stopsEnded,
    tasksMoved,
  ]);

  /**
   * What the machine store says this operator has pinned here, and what it says is gone.
   *
   * Asked again whenever the plane is read again, for the same reason the sidebar is: which
   * workspaces exist is the plane's answer, and a pin that no longer names one has to stop
   * being drawn the moment the plane stops having it. `pinning` is bumped by a pin, so the
   * answer is the store's rather than this window's guess about the store.
   */
  useEffect(() => {
    let gone = false;
    /** The arrangements written when this read was asked: an answer to an older one may hold an
     *  order from before the last of them (#3128). */
    const asked = arranging.current.written;
    void commands
      .planePins(plane)
      .then((answer) => {
        if (gone || answer.status !== "ok") return;
        // **Held to the shape, not merely to `ok`** — the same rule the sidebar's read
        // states. An `ok` answer with no body, or with a body of another shape, would put
        // `undefined` where a list belongs and take the strip down on the next render. A
        // window must not go blank because one command answered oddly.
        const said = answer.data as Partial<typeof answer.data> | null | undefined;
        const pins = Array.isArray(said?.workspaces) ? said.workspaces : [];
        setStorePins(pins);
        // The store's word on the order, once it was asked after this window's last arrangement
        // landed and nothing else is on its way.
        if (arranging.current.writing === 0 && asked === arranging.current.written)
          setArranged(undefined);
        setMadePins((was) => {
          const still = was.filter((name) => !pins.includes(name));
          return still.length === was.length ? was : still;
        });
        setPinOrder(Array.isArray(said?.order) ? said.order : []);
        // Named, drawn dormant, and never written away (V91c as amended, #3651): a dormant pin
        // is the operator's to forget.
        setDormantPins(Array.isArray(said?.missing) ? said.missing : []);
        // The store's answer is what says a gone pin came back, never a list not read yet
        // (`dismissals.ts`): a dismissal is let go only when the core answers without it.
        // **Only a certain answer**: with a rename between its steps, `workspaces/` not there or
        // a listing it could not read whole, the store names nothing gone and says it is not
        // sure, and a pin it leaves out has not come back (D-NO2-10).
        if (said?.certain === true && Array.isArray(said.missing))
          settleNotices("pin-dormant", said.missing.map(dormantCause));
      })
      // A window that cannot ask draws nothing pinned: the workspace strip holds the one you
      // are in, and the rest are behind its show-more — the arrangement an operator who has
      // pinned nothing already has (ADR 0054).
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [pinning, plane, settleNotices, sidebar]);

  /**
   * Which workspace each chat is filed under — the strip its tab appears on.
   *
   * **The plane's answer**, through the sidebar the core reads off it: what relates a chat to
   * a workspace is the directory it works in, and nothing on the plane records a chat. What
   * charter has just started is laid under that, for the one tick before the plane has been
   * read again.
   *
   * A chat working in no workspace is `OUTSIDE`, which is a strip of its own. The sidebar has
   * always shown those rather than dropping them, and a strip per workspace has to have
   * somewhere to put them or they become unreachable.
   */
  /** The persona chat `session` runs as, as the plane's chats say it, or `null`. */
  const personaOf = useCallback(
    (session: number | undefined): string | null => {
      if (session === undefined || sidebar === undefined) return null;
      for (const chats of [...sidebar.workspaces.map((ws) => ws.chats), sidebar.unfiled]) {
        const found = chats.find((chat) => chat.session === session);
        if (found) return found.persona ?? null;
      }
      return null;
    },
    [sidebar],
  );
  /** **The program chat `session` runs** (#3266), as the plane's chats say it: one the Skill
   *  studio knows the own skills of, or `undefined` (a shell, or another program). */
  const harnessOf = useCallback(
    (session: number): HarnessKind | undefined => {
      for (const chats of [
        ...(sidebar?.workspaces.map((ws) => ws.chats) ?? []),
        sidebar?.unfiled ?? [],
      ]) {
        const found = chats.find((chat) => chat.session === session);
        if (found) return HARNESS_KINDS.find((kind) => kind === found.harness);
      }
      return undefined;
    },
    [sidebar],
  );
  /** Chat `session` as the plane's chats list it, in a workspace or outside them all. */
  const listedChatOf = useCallback(
    (session: number | undefined) => {
      if (session === undefined || sidebar === undefined) return undefined;
      return [...sidebar.workspaces.flatMap((ws) => ws.chats), ...sidebar.unfiled].find(
        (chat) => chat.session === session,
      );
    },
    [sidebar],
  );
  /** How much chat `session`'s sandbox holds (#2071), as the plane's chats say it, or `null`. */
  const levelWhys = useLevelWhys(plane);
  const sandboxOf = useCallback(
    (session: number | undefined): SandboxSaid | null => {
      const found = listedChatOf(session);
      if (found?.sandbox == null) return null;
      // Why it runs at its level (#5207), where its start did not say already.
      const why = found.sandbox_why ?? levelWhys.get(found.session);
      return { level: found.sandbox, why, sites: found.browser_sites };
    },
    [levelWhys, listedChatOf],
  );
  /** What chat `session`'s row reads its state from beside the board (#4935), as the plane's
   *  chats say it: so its tab says its row's word. */
  const factsOf = useCallback(
    (session: number): TabFacts | undefined => {
      const found = listedChatOf(session);
      return found === undefined ? undefined : rowFactsOf(found);
    },
    [listedChatOf],
  );
  /** How many chats run in `workspace`, as the sidebar lists them: what an empty pane says
   *  (#4755). */
  const runningIn = useCallback(
    (workspace: string) =>
      workspace === OUTSIDE
        ? (sidebar?.unfiled.length ?? 0)
        : (sidebar?.workspaces.find((ws) => ws.name === workspace)?.chats.length ?? 0),
    [sidebar],
  );
  const filedIn = useCallback<FiledIn>(
    (session) => {
      const at = (chat: number) => {
        const workspace = sidebar?.workspaces.find((ws) =>
          ws.chats.some((one) => one.session === chat),
        );
        if (workspace) return workspace.name;
        if (sidebar?.unfiled.some((one) => one.session === chat)) return OUTSIDE;
        // A chat ended on purpose in its pane is listed no more, and stays where it was filed
        // as it ended (#5019), its tab on that workspace's strip; so does a task kept in its pane
        // as it ended (#5106).
        return startedIn[chat] ?? endedHere.get(chat)?.workspace ?? endedFiled.get(chat);
      };
      // A task the plane lists no more (#5106), or one a scenario spec pretends (`e2eTasks.ts`,
      // none in a shipped build), which is in no list of the core's: it is filed where the chat
      // that asked for it is.
      const asker =
        pretended.find((one) => one.chat.session === session)?.chat.from?.chat ??
        askedOnce.get(session);
      return at(session) ?? (asker === undefined ? undefined : at(asker)) ?? OUTSIDE;
    },
    [askedOnce, endedFiled, endedHere, pretended, sidebar, startedIn],
  );

  /** What a press reads that the tabs do not hold (#4757): where each chat is filed, and each
   *  strip's front tab as the window last left it. */
  const pressing = useMemo<Pressing>(
    () => ({ filedIn, frontOf: (workspace) => lastFront.current[workspace] }),
    [filedIn],
  );
  /** Chat `session` as a press shows it: its own name, and what a tab named after it says. */
  const pressedOf = useCallback((session: number): PressedChat => {
    const listed = listedNow.current.get(session);
    return {
      session,
      chat: listed?.name ?? String(session),
      who: listed === undefined ? null : whoOf(listed.persona, listed.harness),
      label: listed?.label ?? null,
    };
  }, []);
  /**
   * **A press lands** (#4757): the tabs change by `how`, a pinned tab whose first pane now
   * shows another chat keeps its pin, the strip it landed on is drawn, and the chat in the
   * focused pane has the keyboard where it is another one now.
   */
  const landOn = useCallback(
    (how: (tabs: Tabs) => Press) => {
      const before = now.current;
      let landed: string | undefined;
      const next = change((tabs) => {
        const pressed = how(tabs);
        landed = pressed.workspace;
        return pressed.tabs;
      });
      if (next === before) return;
      if (sidebar !== undefined && landed !== undefined) setPicked(landed);
      const typing = focusedChat(next);
      if (typing !== undefined && typing !== focusedChat(before)) giveKeyboardTo(plane, typing);
      const said =
        typing === undefined
          ? undefined
          : paneSaidOf(before, next, typing, (session) => {
              const listed = listedNow.current.get(session);
              return listed?.label ?? listed?.name ?? String(session);
            });
      if (said !== undefined) setPaneSaid((was) => ({ said, count: was.count + 1 }));
    },
    [change, plane, sidebar],
  );
  /**
   * **A chat the person started opens by the one rule** (`tabs.opensAt`, #5289): New chat, a
   * shell, a curation, theme or skill chat, a file's chat, a Fix. It replaces nothing: it fills
   * an empty pane of the front tab, or gets a tab of its own. It goes on the strip it was filed
   * under, which is told here because the list that files it has not been read yet.
   */
  const filedAs = useCallback(
    (session: number, workspace: string): Pressing => ({
      ...pressing,
      filedIn: (one) => (one === session ? workspace : pressing.filedIn(one)),
    }),
    [pressing],
  );
  /**
   * **A tab window keeps its strip on a chat started or resumed here** (#5533), as it does on a
   * press (#5330): a chat filed on another strip and in no pane here is opened by the project
   * window, which comes forward, and is told where the chat is filed, since no list has it yet.
   * Where no window holds the project, it lands here after all.
   */
  const landOrHandOn = useCallback(
    (handed: HandedChat, land: () => void) => {
      const { session } = handed.as;
      const elsewhere =
        tabWindow !== undefined &&
        wouldLeaveStrip(now.current, session, filedAs(session, handed.filed).filedIn);
      if (!elsewhere) {
        land();
        return;
      }
      void pressInProjectWindow(plane, session, handed).then((held) => {
        if (!held) land();
      });
    },
    [filedAs, plane, tabWindow],
  );
  /** **What a chat is known by is kept where it lands** (#5573): here, or by the window it is
   *  handed to, so a tab window keeps nothing of a chat the project window shows. */
  const keepStartedAs = useCallback((session: number, known: StartedAs) => {
    if (known.shell) setShells((was) => new Set(was).add(session));
    const { studio } = known;
    if (studio !== undefined) setStudioLinks((was) => ({ ...was, [session]: studio }));
  }, []);
  const showStarted = useCallback(
    (
      chat: PressedChat,
      workspace: string,
      named = false,
      asked?: PaneAt,
      ownTab = false,
      known: StartedAs = {},
    ) => {
      // What it is called until its row arrives (#5308), drawn with its pane.
      setStartedAs((was) => new Map(was).set(chat.session, chat));
      const land = () => {
        keepStartedAs(chat.session, known);
        landOn((tabs) =>
          openNew(tabs, chat, filedAs(chat.session, workspace), { named, asked, ownTab }),
        );
      };
      if (asked !== undefined) land();
      else landOrHandOn({ filed: workspace, started: true, as: chat, ...known }, land);
    },
    [filedAs, keepStartedAs, landOn, landOrHandOn],
  );
  /**
   * **A chat that came back is a press** (#5289; spec #4750): a resumed chat or a reopened task
   * was the person's to pick, so it lands as a pressed row does. Not listed yet, so it is told
   * where it is filed, as a start is.
   */
  const showResumed = useCallback(
    (chat: PressedChat, workspace: string) => {
      landOrHandOn({ filed: workspace, started: false, as: chat }, () =>
        landOn((tabs) => pressChat(tabs, chat, filedAs(chat.session, workspace))),
      );
    },
    [filedAs, landOn, landOrHandOn],
  );
  /**
   * **A chat the core started and placed** (#4758): a curation, a Fix, a theme, a skill, a
   * file's chat, a first task. Filed where the core put it, and shown on that place's strip,
   * since the start named its place.
   */
  const showPlaced = useCallback(
    (chat: Placed, known?: StartedAs) => {
      const workspace = chat.workspace ?? OUTSIDE;
      setStartedIn((was) => ({ ...was, [chat.session]: workspace }));
      showStarted(
        {
          session: chat.session,
          chat: chat.name,
          who: whoOf(chat.persona, chat.harness),
          label: chat.label,
        },
        workspace,
        true,
        undefined,
        false,
        known,
      );
    },
    [showStarted],
  );

  /** Whether a tab is pinned: the tab's own pin (#4935), whatever its panes hold. */
  const isPinned = useMemo<Pinned>(() => pinsOf(tabs), [tabs]);

  /**
   * **The chats whose Smart close the operator has just pressed**, until the core's first step
   * about them arrives (SI-8f). The press is what makes its head and row breathe, so they do at
   * once rather than a round trip later; a start the core refuses takes it back.
   */
  const [leaving, setLeaving] = useState<ReadonlySet<number>>(() => new Set());
  /** Why each chat's smart close stopped without its record, for the title bar's needs-you
   *  list (SI-8f) — until the operator goes to it or dismisses it, or the chat goes. */
  const [stopped, setStopped] = useState<Readonly<Record<number, string>>>({});
  /** The record the last smart close ended on, for the quiet "Session saved" notice. */
  const [savedNotice, setSavedNotice] = useState<{ record: SavedRecord | null }>();
  /** The chats that finished a Smart close with no pass, by session: their record is saved and
   *  their tab stayed open, so each has a Notice with Close tab (#1361, D-1361-7). */
  const [keptOpen, setKeptOpen] = useState<readonly { session: number; name: string }[]>([]);
  const settle = useCallback((session: number) => {
    setLeaving((was) => {
      if (!was.has(session)) return was;
      const now = new Set(was);
      now.delete(session);
      return now;
    });
  }, []);
  /** Chat `session` is being ended on purpose: where a pane shows it, that pane keeps it. */
  const leavesItsBand = useCallback(
    (session: number, record?: SavedRecord) => {
      const held = tabHolding(now.current, session);
      if (held === undefined) return;
      const listed = listedNow.current.get(session);
      // **A shell clears its pane** (spec #4750 story 64): it has no conversation to resume and
      // nothing to read back, so its pane goes as a shell's exit takes it.
      if (listed !== undefined && isShell(listed)) return;
      // Named as its row named it (`shownName`), and as its tab does where it was not listed.
      const name =
        listed !== undefined ? shownName(now.current, listed) : now.current.byId[held].name;
      // A Smart close's record brings it back; else its conversation.
      const resumes = record !== undefined || listed !== undefined;
      const one: EndedHere = { name, record, resumes, workspace: filedIn(session) };
      endedHereNow.current.set(session, one);
      setEndedHere((was) => new Map(was).set(session, one));
    },
    [filedIn],
  );
  /** A task's pane no longer shows it as ended: cleared, or resumed. */
  const forgetEndedFiled = useCallback((session: number) => {
    endedInPanes.current.delete(session);
    setEndedFiled((was) => {
      if (!was.has(session)) return was;
      const left = new Map(was);
      left.delete(session);
      return left;
    });
  }, []);
  const forgetEndedHere = useCallback(
    (session: number) => {
      forgetEndedFiled(session);
      if (!endedHereNow.current.delete(session)) return;
      setEndedHere((was) => {
        const left = new Map(was);
        left.delete(session);
        return left;
      });
    },
    [forgetEndedFiled],
  );
  /**
   * **A chat ended on purpose that no tab shows any more is forgotten** (#5019): its pane went
   * another way than Clear pane or Resume (Close pane, its tab's close, Close other tabs). Kept,
   * it would still count as gone from what is open here, and its number held as ended.
   */
  useEffect(() => {
    for (const session of [...endedHereNow.current.keys()])
      if (tabHolding(tabs, session) === undefined) forgetEndedHere(session);
  }, [forgetEndedHere, tabs]);
  /** Whether chat `session`, as it ends, stays in its pane: a task does, and so does a chat
   *  ended on purpose where a pane shows it (#5019). */
  const keepsItsPane = useCallback(
    (session: number) => {
      const keep =
        askedByNow(session) !== undefined ||
        askedOnce.has(session) ||
        endedHereNow.current.has(session);
      // Filed where it was as it ends (#5106), before the plane stops listing it; or, where the
      // plane was read again before the end arrived, where the chat that asked for it is.
      if (keep) {
        const workspace = filedIn(session);
        endedInPanes.current.set(session, workspace);
        setEndedFiled((was) =>
          was.get(session) === workspace ? was : new Map(was).set(session, workspace),
        );
      }
      return keep;
    },
    [askedByNow, askedOnce, filedIn],
  );

  const stoppedFor = useCallback((session: number, why: string | undefined) => {
    setStopped((was) => {
      if (why === undefined) {
        if (!(session in was)) return was;
        return Object.fromEntries(Object.entries(was).filter(([one]) => Number(one) !== session));
      }
      return { ...was, [session]: why };
    });
  }, []);

  /**
   * **A smart close that has ended** (ADR 0064). The core closed the chat when its record
   * landed, so the window never says `close_session`, which would end it a second time. It
   * ends like End… (#5019): the pane that showed it keeps it under the ended band, whose Resume
   * is the record, and the window says so quietly, with the record a click away (SI-8f).
   * Given up, ended on its own, or its queued prompt could not be sent: the tab comes back where
   * it was and the window says why, in one sentence and in the needs-you list. Cancelled: the tab coming back says it all.
   */
  const smartCloseEnded = useCallback(
    (step: SmartClosing) => {
      settle(step.session);
      if (step.phase === "closed") {
        // **It ends like End…** (#5019, spec #4750 story 62): its pane keeps it, under the
        // ended band, and its record is its Resume.
        leavesItsBand(step.session, step.record ?? undefined);
        change((tabs) => closeChat(tabs, step.session, filedIn, isPinned, keepsItsPane));
        stoppedFor(step.session, undefined);
        // A kept-open Notice of it has nothing left to end: the chat has ended.
        setKeptOpen((was) => was.filter((one) => one.session !== step.session));
        setSavedNotice({ record: step.record });
        return;
      }
      const tab = tabHolding(now.current, step.session);
      const listed = listedNow.current.get(step.session);
      const name =
        tab === undefined
          ? `chat ${step.session}`
          : listed !== undefined
            ? shownName(now.current, listed)
            : now.current.byId[tab].name;
      if (step.phase === "kept_open") {
        setKeptOpen((was) => [
          ...was.filter((one) => one.session !== step.session),
          { session: step.session, name },
        ]);
        return;
      }
      const said = saidWhenItEnds(step.phase, name);
      if (said !== undefined)
        setReport({ from: `smartclose:${step.session}`, refused: true, words: said });
      stoppedFor(step.session, stoppedWhy(step.phase));
    },
    [keepsItsPane, change, filedIn, isPinned, leavesItsBand, settle, stoppedFor],
  );
  /** The chats wrapping up, as the core tells it. */
  const told = useSmartClosing(plane, smartCloseEnded);
  /**
   * **A chat the person stopped has ended** (#1448). The core ended it, so the window takes its
   * pane away with `closeChat`, and never `close_session`, which would end it a second time.
   */
  const chatStopped = useCallback(
    (session: number) => {
      change((tabs) => closeChat(tabs, session, filedIn, isPinned, keepsItsPane));
      stoppedFor(session, undefined);
      setStopsEnded((count) => count + 1);
    },
    [keepsItsPane, change, filedIn, isPinned, stoppedFor],
  );
  /** The chats being stopped, as the core tells it. */
  const stopping = useStopping(plane, chatStopped);
  /** Every chat the Chats section lists, as of the last render: what a stop is asked about. A
   *  ref, so a read of the list does not make a new catalogue of the rows holding the ask. */
  const chatsListed = useRef<readonly ListedChat[]>([]);
  /** Each chat's finished rows as last read, for a press that asks for one (#1491). */
  const finishedNow = useRef<ReadonlyMap<number, readonly FinishedTask[]>>(new Map());
  /**
   * **Stop asks first** (#1448): a row of the catalogue opens the question, which says what
   * ends, and nothing is stopped until it is answered. A chat with no chat below it is asked
   * about alone, whichever row was pressed.
   */
  const askToStop = useCallback(
    (session: number, below: boolean) => {
      const chat = chatsListed.current.find((one) => one.session === session);
      if (chat === undefined) return;
      const under = chatsBelow(chatsListed.current, session);
      setStopAsk({
        session,
        name: chat.name,
        below: below && under.length > 0,
        under: under.length,
        dispatched: chat.parent !== null,
        already: stopping.has(session),
        waitingOn: under.filter((one) => stopping.has(one)).length,
        busy: false,
      });
    },
    [stopping],
  );
  /** The person answered: the core stops the chat, or says why not, in the question. */
  const stopAsked = useCallback(async () => {
    const asked = stopAsk;
    if (asked === undefined) return;
    setStopAsk({ ...asked, busy: true, trouble: undefined });
    const said = await commands
      .stopChat(plane, asked.session, asked.below)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setStopAsk(
      said.status === "error" ? { ...asked, busy: false, trouble: said.error } : undefined,
    );
  }, [plane, stopAsk]);
  /**
   * **Stop all tasks asks once, naming how many** (#1498, V100-53): the core says which tasks
   * below the session are at work, and the answer stops those and no more. A session with none
   * left is said so on the window's report line, and nothing is asked.
   */
  const askToStopAll = useCallback(
    async (session: number) => {
      const read = await commands
        .allTasksEnding(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (read.status === "error") {
        setReport({ from: `chat.stop.tasks:${session}`, refused: true, words: read.error });
        return;
      }
      const ending = read.data;
      // No answer is no leave to stop anything: nothing is guessed about a task.
      if (ending === null || typeof ending !== "object" || !Array.isArray(ending.tasks)) return;
      if (ending.tasks.length === 0) {
        setReport({
          from: `chat.stop.tasks:${session}`,
          refused: true,
          words: `No task below ${ending.name} is at work, so there is nothing to stop.`,
        });
        return;
      }
      setStopAllAsk({ session, name: ending.name, tasks: ending.tasks, busy: false });
    },
    [plane],
  );
  /** The person answered: the core stops the tasks the question named, or says why not. */
  const stopAllAsked = useCallback(async () => {
    const asked = stopAllAsk;
    if (asked === undefined) return;
    setStopAllAsk({ ...asked, busy: true, trouble: undefined });
    const said = await commands
      .stopAllTasks(plane, asked.session, [...asked.tasks])
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setStopAllAsk(
      said.status === "error" ? { ...asked, busy: false, trouble: said.error } : undefined,
    );
  }, [plane, stopAllAsk]);
  /**
   * **Ends a task the way the person chose**, by the core (`end_task`), which tells the chat
   * that asked which way it was. A refusal is said where the answer was given: in the
   * question while one is up, else on the window's report line.
   */
  const endTaskNow = useCallback(
    async (session: number, way: TaskEndWay, below: boolean, asked?: TaskEndAsked) => {
      if (asked !== undefined) setTaskEndAsk({ ...asked, busy: true, trouble: undefined });
      const said = await commands
        .endTask(plane, session, way, below)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status !== "error") {
        setTaskEndAsk(undefined);
        return undefined;
      }
      if (asked !== undefined) setTaskEndAsk({ ...asked, busy: false, trouble: said.error });
      // Where no question is up, whoever asked says the refusal where the answer was given.
      return said.error;
    },
    [plane],
  );
  /**
   * **Stop and get its report, or Close now** (#1488, V100-5, V100-18): every press to end a
   * task comes here, and **none ends anything by itself**. The core is asked what ending the
   * task would do, and then the person is asked:
   *
   * - a task mid-turn, or with tasks of its own at work below it, in the one modal question;
   * - every other task **in place**, where the press was made: on its row for its menu and
   *   Delete, on the breadcrumb for its two controls ("Stop it" or "Close it", and Keep, which
   *   has the keyboard). A press from anywhere else is asked on the breadcrumb where a pane
   *   shows the task, and in the modal question where none does, so it is never asked
   *   somewhere the person is not looking.
   *
   * Where purlis may not type into the task, either says why and offers Close alone. The
   * standard close dialog is never part of this.
   */
  /** Whether a pane of tab `tab` draws a breadcrumb ending in task `session`: the one place
   *  a second step is drawn in place (`TaskEndAsked`). Held here for what is declared before the
   *  list is read. */
  const drawsCrumbFor = useRef<(tab: number, session: number) => boolean>(() => false);
  const askToEndTask = useCallback(
    async (session: number, way: TaskEndWay, from: TaskEndFrom) => {
      const read = await commands
        .taskEnding(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (read.status === "error") {
        setReport({ from: `task.end:${session}`, refused: true, words: read.error });
        return;
      }
      const ending = read.data;
      // No answer is no leave to end anything: nothing is guessed about a task.
      if (ending === null || typeof ending !== "object") return;
      // Being stopped already: it has its one short turn, and is not asked a second time.
      if (ending.stopping && way === "report") return;
      // On screen: a pane of the tab in front draws a breadcrumb that ends in it, which is
      // where the second step is drawn. A tab behind draws none, and neither does a task's own
      // tab with no path to say (its asker is not listed), nor a task's pane switched to a
      // task of its own: a question set there would be asked where nothing draws it.
      const front = now.current.inFront;
      const shown = front !== undefined && drawsCrumbFor.current(front, session);
      const where = from === "elsewhere" ? (shown ? "crumb" : undefined) : from;
      if (asksInAModal(ending) || where === undefined) {
        setTaskEndInline(undefined);
        setTaskEndAsk({ ...ending, session, belowToo: true, busy: false });
        return;
      }
      setTaskEndInline({ session, where, busy: false, ...inPlace(ending, way) });
    },
    [plane],
  );
  /** Restarts a chat, as the Restart chat row does: held here for the second step's answer,
   *  which is declared before the restart is (`askRestart`). */
  const restartNow = useRef<(session: number) => void>(() => undefined);
  /** The second step was answered: the task is ended the way it said, or kept. */
  const answerTaskEnd = useCallback(
    async (yes: boolean) => {
      const asked = taskEndInline;
      if (asked === undefined) return;
      if (!yes) {
        setTaskEndInline(undefined);
        return;
      }
      // A task's Restart chat, asked the way an end is (#1489): the answer restarts it.
      if (asked.act === "restart") {
        setTaskEndInline(undefined);
        restartNow.current(asked.session);
        return;
      }
      setTaskEndInline({ ...asked, busy: true, trouble: undefined });
      const refused = await endTaskNow(asked.session, asked.way, false);
      // A refusal stays where the answer was given, in the core's words, and nothing ended.
      setTaskEndInline(
        refused === undefined ? undefined : { ...asked, busy: false, trouble: refused },
      );
    },
    [endTaskNow, taskEndInline],
  );
  /** A way to end a task was pressed on its row in the Chats list: asked about on that row.
   *  Held, so the list's rows are not drawn again for it. */
  const endTaskOnRow = useCallback(
    (session: number, way: TaskEndWay) => void askToEndTask(session, way, "row"),
    [askToEndTask],
  );
  const answerTaskEndOnRow = useCallback(
    (yes: boolean) => void answerTaskEnd(yes),
    [answerTaskEnd],
  );
  /** What every breadcrumb's two controls are handed (`TaskEndContext`). */
  const taskEndHand = useMemo<TaskEndHand>(
    () => ({
      confirming: taskEndInline,
      onEnd: (session, way) => void askToEndTask(session, way, "crumb"),
      onConfirm: (yes) => void answerTaskEnd(yes),
    }),
    [answerTaskEnd, askToEndTask, taskEndInline],
  );
  /**
   * **A kept-open chat whose tab closed another way is forgotten** (#1361): by a Smart close
   * that went through after all, the tab's own ×, or the chat ending. Its Notice went with the
   * tab; kept in the list, it came back on the next chat the core gave the same number. Set
   * while rendering, React's way of adjusting state to what a render found, so the Notice is
   * never drawn without its tab, not for a frame.
   */
  const keptOpenHere = keptOpen.filter(({ session }) =>
    tabs.order.some((id) => panesOf(tabs, id).some((pane) => pane.session === session)),
  );
  if (keptOpenHere.length !== keptOpen.length) setKeptOpen(keptOpenHere);
  /** A kept-open chat's Notice put away, by Dismiss or by its End chat. */
  const forgetKeptOpen = useCallback(
    (session: number) => setKeptOpen((was) => was.filter((one) => one.session !== session)),
    [],
  );
  /** **End chat** on a kept-open chat's Notice: the person's own press, ending the chat as
   *  End…'s Close does, and granting nothing. Named End, since a close ends nothing (#4756). */
  const closeKeptOpen = useCallback(
    (session: number) => {
      forgetKeptOpen(session);
      leavesItsBand(session);
      change((tabs) => closeChat(tabs, session, filedIn, isPinned, keepsItsPane));
      void commands.closeSession(plane, session);
    },
    [keepsItsPane, change, filedIn, forgetKeptOpen, isPinned, leavesItsBand, plane],
  );
  /**
   * **End…, a chat's own row, once it is answered** (#4753): the core ends its program, and the
   * pane that showed it keeps it, as ended, under the ended band with Clear pane (#5019); its
   * tasks stay wherever they are shown (#4757). Asked first (`EndingChat`), as every row that ends a chat is.
   */
  const endChat = useCallback(
    (session: number) => {
      // **Its pane keeps it, under the ended band** (#5019), with Clear pane to take it away.
      leavesItsBand(session);
      change((tabs) => closeChat(tabs, session, filedIn, isPinned, keepsItsPane));
      void commands.closeSession(plane, session);
    },
    [keepsItsPane, change, filedIn, isPinned, leavesItsBand, plane],
  );
  /** …and the ones just pressed: their tabs, their explorer rows and their menus say so. */
  const wrapping = useMemo<ReadonlySet<number>>(
    () => (leaving.size === 0 ? told : new Set([...told, ...leaving])),
    [leaving, told],
  );

  /**
   * The workspace the strip DRAWS, and the one axis rule: **the tab in front is on it.**
   *
   * Derived rather than maintained. Until #133 this held because `bringToFront`,
   * `focusWorkspace`, `closeTab`, `openTab` and the sidebar-read's focus rule each kept it —
   * five handlers agreeing, with no single place that re-establishes it, so a sixth that
   * forgot would break the axis silently and a reviewer of #131 said exactly that.
   *
   * It was already broken without a sixth handler. **Nothing on the plane records a chat**:
   * which workspace it is in is decided by the directory it works in, so a workspace added on
   * disk moves chats between strips with no handler involved at all. The window then drew the
   * strip the last handler had left it on, with the front chat's pane under a strip that did
   * not list it.
   *
   * So the front tab decides, and `picked` answers only when there is no front tab — which is
   * the workspace holding no chats, the one case the axis cannot be read off a tab.
   */
  const focused = useMemo(() => {
    if (sidebar === undefined || tabs.inFront === undefined) return picked;
    return workspaceOf(tabs, tabs.inFront, filedIn) ?? picked;
  }, [filedIn, picked, sidebar, tabs]);

  /** The workspace whose repos and worktrees the three regions read. The strip for chats
   *  outside every workspace is not a workspace on the plane, so there is no directory to
   *  read and every region says so rather than drawing another workspace's answer. */
  const ofWorkspace = focused === OUTSIDE ? undefined : focused;
  // **The artifact whose tab is in front, of this workspace** (#5952): its icon is drawn chosen.
  const artifactInFront = (() => {
    if (tabs.inFront === undefined || tabs.byId[tabs.inFront] === undefined) return undefined;
    const front = tabs.byId[tabs.inFront];
    const shown = contentsOf(tabs, front.id).find((one) => one.pane === front.focused)?.content;
    const artifact = shown?.kind === "view" ? artifactOf(shown.view) : undefined;
    return artifact !== undefined && artifact.workspace === ofWorkspace
      ? artifact.folder
      : undefined;
  })();
  /**
   * **What this project has on in the focused workspace** (charter-app#253, #280, ADR 0048): the
   * core's answer for this plane's two files and the workspace's `workspace.json`, over this
   * machine's approvals. The window's survey is filtered by it here, once, so the side region,
   * the view buttons and the palette's view rows all read one list. Charter's own panels (`from`
   * null) are not an extension's and are never filtered.
   */
  const on = useExtensionsOn(plane, ofWorkspace);
  const contributed = useMemo(
    () => surveyedPanels.filter((panel) => panel.from === null || (on?.has(panel.from) ?? false)),
    [on, surveyedPanels],
  );
  const views = useMemo(
    () => surveyedViews.filter((view) => on?.has(view.extension) ?? false),
    [on, surveyedViews],
  );
  const extensionCommands = useMemo(
    () => surveyedCommands.filter((command) => on?.has(command.extension) ?? false),
    [on, surveyedCommands],
  );
  // **This project's theme and its repos' save rows stay known while the window holds it**
  // (FR-27), in front or not. The window reads both for the project in front, and an answer
  // nothing holds is forgotten (`projectTheme.ts`, `saving.ts`): held here, a switch back draws
  // them at once, rather than asking the core and drawing the whole window again when it
  // replies.
  useProjectThemeKept(plane, ofWorkspace);
  useRepoSavingKept(plane, ofWorkspace);
  /**
   * **The focused workspace's panels, in a store of their own** (#1160): read again for the
   * changes they are made of (FD-10) — its own and the personas'; a todo closed in another
   * workspace does not read this one again — and drawn by the regions that read them, each
   * taking its own part (`WorkspaceRegion`). This view takes only what it reads itself
   * (`WINDOW_READS`), so a memory or a session record saved in the workspace in front draws
   * the panel that lists it and not this whole window.
   */
  const workspacePanels = useWorkspacePanelsStore(plane, ofWorkspace, rereadWorkspace);
  const workspaceState = useWorkspacePanels(
    workspacePanels,
    ofWorkspace,
    (state) => state,
    forTheWindow,
  );
  /** The extensions' panels as views of the right side (#1678): a tab of their own each. Held
   *  by what they draw and not by the state they were read from, which is a new object on many
   *  a redraw: the catalogue reads this list, and a catalogue that changes draws every chat row
   *  again (SC-3). Kept in state and replaced only when it draws something else, which is
   *  React's own way to hold a value from the last render. */
  const sidePanelsNow = attentionPanels(workspaceState, contributed);
  const [sidePanels, setSidePanels] = useState(sidePanelsNow);
  if (!samePanels(sidePanels, sidePanelsNow)) setSidePanels(sidePanelsNow);
  /** **The views behind a preview that is on** (#3266): Skills while the Skill studio's is. One
   *  left out is drawn nowhere, and its side opens on its first view until it is back. Until the
   *  core has said, a side left on Skills is held there, drawn empty (#3308). */
  const pickedViews = arrangement.map((one) => one.view).join(" ");
  const previews = useMemo(
    () => previewsDrawn(studioSaid, pickedViews.split(" ")),
    [studioSaid, pickedViews],
  );
  /** Every view drawn only while it is present: the extensions' panels contributed now, and the
   *  views behind a preview that is on. */
  const presentViews = useMemo(
    () => [...sidePanels.map((one) => one.view), ...previews],
    [sidePanels, previews],
  );
  /** **Whether the Skills view has been shown** (#3266): it reads nothing until it is first
   *  opened, as Search draws nothing until then, and is kept after. */
  const [skillsDrawn, setSkillsDrawn] = useState(false);
  if (
    !skillsDrawn &&
    studioOn &&
    arrangement.some((one) => !one.collapsed && viewOpenIn(one, presentViews) === "skills")
  )
    setSkillsDrawn(true);
  /** Whether `view` can be shown now: one drawn only while present is only then. */
  const drawable = useCallback(
    (view: ViewId) => !drawnOnlyPresent(view) || presentViews.includes(view),
    [presentViews],
  );
  /** The regions put away, and the Panel while no half of it is drawn now (#1737, #1752), for
   *  the palette's rows: a half holding only panels no extension contributes now is not drawn. */
  const awayNames = putAway(arrangement, presentViews).join(" ");
  const away = useMemo(
    () => (awayNames === "" ? [] : (awayNames.split(" ") as Toggled[])),
    [awayNames],
  );
  /** Put a region away or bring it back, or the whole Panel (its key, the palette):
   *  the Panel counts as drawn only where a half is drawn now, as the palette's title says
   *  (#1752), so a half holding only panels no extension contributes now hides nothing. */
  const toggleRegion = useCallback(
    (id: Toggled) => toggle(id, presentViews),
    [toggle, presentViews],
  );
  /**
   * **A region's key or row while focus mode is on** (#2921, D-2921-19): it leaves focus mode
   * first, so nothing changes out of sight. A key that puts a region away or brings it back
   * (⌘B, ⌥⌘B, ⌘J) only leaves it, and the regions come back as they were; a key that shows a
   * view (⌘⇧E…) leaves it and shows the view, which the person then sees.
   */
  const focusModeNow = useRef(focusMode);
  useLayoutEffect(() => {
    focusModeNow.current = focusMode;
  });
  const toggleFocusMode = windowDoes.toggleFocusMode;
  const toggleRegionSeen = useCallback(
    (id: Toggled) => {
      if (focusModeNow.current) toggleFocusMode();
      else toggleRegion(id);
    },
    [toggleFocusMode, toggleRegion],
  );
  const showSideViewSeen = useCallback(
    (view: ViewId) => {
      if (focusModeNow.current) toggleFocusMode();
      showSideView(view);
    },
    [toggleFocusMode, showSideView],
  );
  /** Where each side's view lives now (#1752): its move as a palette row, and the region rows'
   *  notes naming the views there. Held by what it says, so a resize or a view picked, which
   *  change the arrangement and not where a view lives, does not build the catalogue again. */
  const placesSaid = JSON.stringify(placesOf(arrangement, sidePanels, previews));
  const places = useMemo(() => JSON.parse(placesSaid) as ViewPlaced[], [placesSaid]);
  /** The focused workspace's open todos, for the Todos tab's count (#1678, B-8). */
  const openTodos = todoCount(workspaceState);
  /** Whether a read of that workspace stands refused, so the catalogue offers Read again
   *  where the explorer is not on screen to offer it (#1244). */
  const readRefused = readRefusedIn(workspaceState);
  /** The plane root's own panels — its session records (SI-8d) — while it is focused, and while
   *  no workspace is: its Personas panel is the project's, drawn then too (#1686).
   *
   *  **Followed only while they are read** (#1160): a persona's memory concerns the root's
   *  panels too, and counting it while a workspace is in front drew this whole view for
   *  nothing. Coming back to the root reads them anyway. */
  const rootChanges = usePlaneChanged([plane], ofWorkspace === undefined ? ROOT_PANELS : undefined);
  const rootPanels = usePlaneRootPanels(plane, ofWorkspace === undefined, rootChanges);
  /** The badges and repo columns the extensions on here show (charter-app#340). */
  const facts = useExtensionFacts(plane, ofWorkspace);
  /** What `charter doctor` says about this project, run inside the app: the preflight when
   *  the project opens, the full doctor when the operator opens it (`Doctor.tsx`). */
  const doctor = useDoctor(plane);
  /**
   * This plane's pin (`Updates.tsx`).
   *
   * **The updater used to be here beside it and is not any more.** An offer is a fact about
   * the app, and this component is one project of as many as the window holds — so
   * `useUpdates` here was one updater client, three event listeners and an `update_channel`
   * call PER OPEN PROJECT, all reporting the same thing. It is called once in `App` now and
   * drawn once, on the title bar, which is the window's own chrome.
   *
   * The pin stays, because it is the opposite kind of fact: `charter version`'s verdict about
   * THIS plane's `[charter] version` (ADR 0030). It belongs beside the project it is
   * about, and two open projects can honestly disagree about it.
   */
  const pin = usePin(plane);

  /**
   * The piece the explorer has picked, when it is still a piece of the workspace on screen.
   *
   * Two things can make a pick stop standing, and they are different. Moving to another
   * workspace only SETS IT ASIDE — coming back brings it with you, the way coming back to a
   * workspace comes back to the tab that was in front there. A piece that is gone from the
   * listing is another matter: the worktree was removed while it was picked, and pointing the
   * next chat at a directory that is not there would make the operator read a refusal charter
   * could see coming. A listing that has not arrived yet is not evidence of either, so the
   * pick stands until git has answered.
   */
  const cloneNames = workspaceState.panels?.repos;
  const spot = useMemo(() => {
    if (pickedSpot === undefined || pickedSpot.workspace !== ofWorkspace) return undefined;
    const { repo, piece } = pickedSpot.spot;
    // A picked CLONE (charter-app#174) stops standing the same way, one level up: when the
    // plane is read again and the clone is not in it.
    if (piece === undefined)
      return cloneNames !== undefined && !cloneNames.includes(repo) ? undefined : pickedSpot.spot;
    const listed = workspaceState.pieces[repo];
    if (listed !== undefined && !listed.some((one) => one.piece === piece)) return undefined;
    return pickedSpot.spot;
  }, [cloneNames, ofWorkspace, pickedSpot, workspaceState.pieces]);

  // Where a chat starts: **the spot the explorer picked**, and the focused workspace's own
  // directory when nothing is picked — so the sidebar can file it under that workspace.
  // Nothing on the plane records a chat, so where it works is the only thing relating the
  // two, and a piece of a workspace is still in that workspace. Null when there is neither,
  // and the core starts that chat in the plane's own directory.
  //
  // **The plane root starts its chats in the plane's own directory, said out loud** (SI-1),
  // rather than as the `null` that happens to mean the same: the core tells a chat started
  // there that it is in no workspace.
  const startIn =
    spot?.path ??
    (focused === OUTSIDE
      ? sidebar?.root
      : sidebar?.workspaces.find((ws) => ws.name === focused)?.path) ??
    null;

  /**
   * The branch the explorer is focused on in the workspace in front, while the workspace still
   * lists it (`Explorer`'s own rule), and else none: what ⌘P and ⌘⇧F look in first.
   */
  const cockpit = useMemo(() => {
    if (focusedBranch === undefined || focusedBranch.workspace !== ofWorkspace) return undefined;
    return focusStands(workspaceState, focusedBranch) === undefined ? undefined : focusedBranch;
  }, [focusedBranch, ofWorkspace, workspaceState]);
  // **A focus whose branch was removed is forgotten** (#1152), once the workspace's listing has
  // answered without it, so the record the window keeps does not put it back at the next launch.
  // Never while the listing is still being read, and never on a refusal, which says nothing of
  // the branch. Only for the workspace in front: another one's listing is not read here.
  //
  // Set while rendering, React's way of adjusting state to what a render found: an effect would
  // draw the gone focus once more first, and the answer is already here.
  if (
    focusedBranch !== undefined &&
    focusedBranch.workspace === ofWorkspace &&
    focusGone(workspaceState, focusedBranch)
  )
    setFocusedBranch(undefined);
  /** The nearest branch to the operator: the cockpit's, else the one the explorer picked. */
  const nearBranch = useMemo<Place | undefined>(
    () =>
      cockpit ??
      (ofWorkspace !== undefined && spot !== undefined
        ? { workspace: ofWorkspace, repo: spot.repo, piece: spot.piece ?? null }
        : undefined),
    [cockpit, ofWorkspace, spot],
  );
  useEffect(() => {
    searchFocus.current = { branch: nearBranch, workspace: ofWorkspace };
  }, [nearBranch, ofWorkspace]);
  /** The view open on the left, or nothing while the side is away. */
  const leftOpen = (() => {
    const navigation = arrangement.find((one) => one.id === "navigation");
    return navigation === undefined || navigation.collapsed
      ? undefined
      : viewOpenIn(navigation, presentViews);
  })();
  // The Search view's first question when it comes back open from the layout file: as narrow
  // as the focus. Every other way of opening it asks one (`searchHere`).
  if (leftOpen === "search" && sideSearch === undefined) {
    setSideSearch(searchView(searchFromFocus(nearBranch, ofWorkspace)));
  }
  /** A press of a view's tab (`regions.picked`). Opening Search searches where the window is,
   *  as its key does, and gives its box the keyboard, as an editor's does: a search view is
   *  opened to type into. */
  const pickSideView = useCallback(
    (view: ViewId) => {
      if (view === "search" && leftOpen !== "search") {
        searchHere();
        requestAnimationFrame(() => giveViewTheKeyboard(view));
      }
      // The extension panels present now: a panel's view is picked only while it is there (#1678),
      // and a view behind a preview only while the preview is on (#3266).
      pick(view, presentViews);
    },
    [leftOpen, pick, searchHere, presentViews],
  );
  /** The files git has uncommitted in the focused workspace's clones: the Changes tab's count. */
  const changedFiles = uncommitted(workspaceState);
  /** How far those clones are behind and ahead, once one is behind: the Repos icon's (#1739). */
  const gap = reposGap(workspaceState);

  /** What the explorer picks, remembered against the workspace it was picked in. */
  const pickSpot = useCallback(
    (next: Spot | undefined) => {
      if (ofWorkspace === undefined) return;
      setPickedSpot(next === undefined ? undefined : { workspace: ofWorkspace, spot: next });
    },
    [ofWorkspace],
  );

  /** Every workspace the window can bring forward: **the plane root first, always** (SI-1),
   *  then this project's workspaces — the pinned ones first, in the order they were pinned in,
   *  then the rest in the plane's own order, which is the sidebar's.
   *  The palette and the title bar's ⌄ list all of them; the title bar's chips are the pins. */
  const strips = useMemo(() => {
    if (sidebar === undefined) return [];
    const names = sidebar.workspaces.map((ws) => ws.name);
    // **Pinned first, in the order they were pinned in, then the rest in the plane's own
    // order** (ADR 0039, ADR 0054, charter#402). The store answers the pins in that order, and
    // only the ones the plane still has. The root is never a pin: it is drawn before them.
    return [
      OUTSIDE,
      ...pinnedWorkspaces.filter((name) => names.includes(name)),
      ...names.filter((name) => !pinnedWorkspaces.includes(name)),
    ];
  }, [pinnedWorkspaces, sidebar]);

  // The floors grow with the window's text (charter-app#283, `fits.leastAt`).
  const windowText = useTextSizes().window;
  const chatLeast = leastAt(LEAST.chat, windowText);
  // An icon-only pin is exactly this wide (#4762), and the chat strip fits by it: a pin holding
  // a tab's floor would hide a tab the strip has room for.
  const chipWidth = leastAt(LEAST_CHIP, windowText);

  /**
   * **Each workspace's colour** (charter-app#281), as the core read it out of its
   * `workspace.json` with the sidebar — so it is read again whenever the plane changes on disk,
   * a write in Settings at the Workspace level included. `null` for a workspace with none, for the
   * chats outside every workspace, which have no file to hold one, and for a grey `#rrggbb`,
   * which has no hue to tint with: no mark is drawn for a colour that tints nothing (the
   * Workspace level in Settings says why).
   */
  const colourOf = useCallback(
    (workspace: string | undefined): string | null =>
      colourWithHue(sidebar?.workspaces.find((ws) => ws.name === workspace)?.colour),
    [sidebar],
  );
  /** The theme the window draws, which a colour is a hue shift of: a tab's tint follows it. */
  const drawnTheme = useSyncExternalStore(followTheme, inForce);
  /** What a workspace's own tab and its chat strip put on themselves: its colour, on the tokens
   *  the theme's recipe tints there (`theme.tabTint`). Nothing for a workspace with no colour. */
  const tintOf = (workspace: string | undefined) =>
    tabTint(drawnTheme, colourOf(workspace)) as CSSProperties;

  /**
   * What a workspace is drawn as: its name, its live dot, its counts in one pill, and its pin.
   *
   * **One definition, used by the strip and by the menu of what the strip is not drawing.**
   * They are the same workspace and a second copy of the markup is a second answer — the
   * rule the catalogue already follows for words, applied to marks.
   *
   * Counted here in the render body, once per workspace, and DELIBERATELY not memoised. It
   * looks quadratic and it is — ten workspaces × fifty tabs × a scan of the sidebar — so
   * charter-app#133 measured it at the limits before touching it: **0.022 ms at ADR 0026's
   * ten workspaces and fifty chats**, against a 16.7 ms frame, and half a percent of the
   * re-render it sits in. A memo over `tabs` would save that 22 µs on the one event it was
   * proposed for — `chat-moved` changes neither `tabs` nor `sidebar`, so the memo would hit
   * every time — and be paid for on every event that does change them. The measurement is
   * kept as assertions in `tabs.test.ts`, "the workspace strip at fifty chats", where a
   * third nested scan fails a test instead of being a surprise.
   */
  /** Whether `workspace` is LIVE, as the plane was last read (charter-app#301). */
  const liveOf = useCallback(
    (workspace: string) =>
      sidebar?.workspaces.some((ws) => ws.name === workspace && ws.live) ?? false,
    [sidebar],
  );
  const liveNames = useMemo(
    () => (sidebar?.workspaces ?? []).filter((ws) => ws.live).map((ws) => ws.name),
    [sidebar],
  );

  /** Every chat the core lists, by number. */
  const chatsByNumber = useMemo(
    () =>
      new Map(
        [
          ...(sidebar?.workspaces.flatMap((ws) => ws.chats) ?? []),
          ...(sidebar?.unfiled ?? []),
          // Tasks a scenario spec pretends (`e2eTasks.ts`): none in a shipped build.
          ...pretended.map((one) => one.chat),
        ].map((chat) => [chat.session, chat]),
      ),
    [pretended, sidebar],
  );
  /**
   * **Where a chat came from**, as the core lists it; and **until that list is read, as the
   * launch put it back** (#1489). The tabs are drawn from what the launch said before the
   * list arrives, and a task's own tab must be a task's from its first frame: its `−`, never
   * a session's `×` for the moment in between.
   */
  const fromOf = useCallback(
    (session: number) =>
      chatsByNumber.get(session)?.from ??
      (sidebar === undefined ? reopened.find((chat) => chat.session === session)?.from : undefined),
    [chatsByNumber, reopened, sidebar],
  );
  /**
   * **Which chat a task is a task of**, as the core lists it (#1486). Task links only: a
   * handoff is a session of its own and is never shown inside another's tab (V100-69).
   */
  const askedBy = useCallback<AskedBy>(
    (session) => {
      const from = fromOf(session);
      return from?.task ? from.chat : undefined;
    },
    [fromOf],
  );
  /**
   * **What each pane of the tab in front shows, and whether it can be drawn** (`shownLive`,
   * #1486): a pane draws another chat's terminal only while that chat is open and at home in
   * that pane. Nothing is judged before the list of chats is read.
   */
  const frontShown = useMemo(
    () =>
      tabs.inFront === undefined
        ? []
        : shownLive(tabs, tabs.inFront, askedBy, (session) =>
            sidebar === undefined ? false : chatsByNumber.has(session),
          ),
    [askedBy, chatsByNumber, sidebar, tabs],
  );

  /** How many chats in `workspace` need you: its chip's count in the title bar, and its row's
   *  in the title bar's switchers (#2916). One reading of the queue for all of them. */
  //
  // **A chat is counted where its tab is** (#1486, V100-40): a task that works in another
  // workspace is at home in the tab it returns to (`returnsTo`), so that tab's workspace
  // wears it. A chat with no tab is counted where it works, as before.
  const waitingIn = useCallback(
    (queue: readonly number[], workspace: string) =>
      queue.filter((session) => {
        const home = homeOf(tabs, session, askedBy);
        return (
          (home === undefined ? filedIn(session) : workspaceOf(tabs, home.tab, filedIn)) ===
          workspace
        );
      }).length,
    [askedBy, filedIn, tabs],
  );

  /**
   * The chats the strip shows: the focused workspace's.
   *
   * **Every one of them while charter has not read the plane yet.** With no sidebar there is
   * nothing that knows which workspace a chat is in, and a strip that showed none of them
   * would be hiding chats that are running — which is worse than a strip that shows them all
   * for the moment before the answer arrives.
   */
  const onStrip = useMemo(
    () => (sidebar === undefined ? tabs.order : tabsIn(tabs, focused, filedIn, isPinned)),
    [filedIn, focused, isPinned, sidebar, tabs],
  );

  /**
   * How much room the chat strip has, and therefore which of its tabs it draws.
   *
   * **What does not fit is not drawn** — the operator's call, reversing what ADR 0039 left
   * open. `fits.ts` holds the whole of why the answer is arithmetic over one measured width
   * rather than an intersection measurement over fifty tabs, and what it costs.
   *
   * The `+` is the strip's last child, right after its last tab as Chrome draws it (DS-7,
   * #1839), so `controls` goes on it and its width comes off the room before the tabs share
   * what is left: it is never one of the things that collapse. The show-more button is a
   * sibling at the right end of the bar, and appears only once the strip is full.
   */
  const {
    strip: measured,
    controls: chatControls,
    width: room,
    gap: chatGap,
  } = useRoom(onStrip.length);
  /** The chat tabs' ids, which the strip's tablist owns them by (#1204). */
  const chatTabId = useTabIds();
  /** The chat strip itself. */
  const chatStrip = useRef<HTMLElement | null>(null);
  /** Unique to this view, so two projects' tabs never name each other's fresh marks. */
  const freshMarks = useId();
  const freshMarkOf = (tab: number) => `${freshMarks}-fresh-${tab}`;
  /** The id of the words a pane head's tasks button is described by: its counts (#5341). */
  const tasksIdOf = useCallback(
    (session: number, pane: number) => `${freshMarks}-tasks-${pane}-${session}`,
    [freshMarks],
  );
  const strip = useCallback(
    (element: HTMLElement | null) => {
      chatStrip.current = element;
      measured(element);
    },
    [measured],
  );
  const { shown, hidden, pinsIconOnly } = useMemo(
    // Each tab and chip costs the room between it and the next too (the theme's gap, #2914),
    // the last one's before the `+`. **A pinned tab never collapses** (#4762; ADR 0039, amended
    // 2026-11-02): only the others go behind 'N more', and the pins go to their icons only
    // when they alone would take more than half the strip.
    () =>
      pinnedFirst(onStrip, tabs.inFront, room, chatLeast + chatGap, {
        is: isPinned,
        iconWidth: chipWidth + chatGap,
      }),
    [onStrip, room, tabs.inFront, chatLeast, chatGap, isPinned, chipWidth],
  );
  /** Where the tab in front is drawn on the strip, or -1 where it is not. */
  const frontPlace = tabs.inFront === undefined ? -1 : shown.indexOf(tabs.inFront);
  /** The id of the tab in front, which names the panel under it. */
  const panelNamedBy = useMemo(
    () => (frontPlace >= 0 ? chatTabId(frontPlace) : undefined),
    [chatTabId, frontPlace],
  );

  /** How many of a tab's chats need you: every one of its panes' that is in the queue, split
   *  or not. Its share of the chat strip's show-more count when the strip is not drawing it. */
  const waitingOn = (queue: readonly number[], id: number) =>
    panesOf(tabs, id).filter((pane) => queue.includes(pane.session)).length +
    // And its tasks that need you and are not on screen (#1486): the tab wears them. Its own
    // chat is counted above, shown or not.
    hiddenNeeding(tabs, id, askedBy, queue).filter(
      (session) => !panesOf(tabs, id).some((pane) => pane.session === session),
    ).length;

  // The tab that was in front on this strip, remembered so that coming back to a workspace
  // comes back to the chat that was on screen there.
  useEffect(() => {
    const front = tabs.inFront;
    if (front === undefined || sidebar === undefined || holdingForward) return;
    const workspace = workspaceOf(tabs, front, filedIn);
    if (workspace !== undefined) lastFront.current[workspace] = front;
  }, [filedIn, holdingForward, sidebar, tabs]);

  /**
   * **Every tab, told to the core whenever it changes** (#4751): the record keeps it, so a
   * relaunch puts each split, its sizes, its views and each strip's front tab back, and the core
   * derives what is on screen from it (`Chats::looked_at`). A press says it in the same call
   * (`change`); this says what changes otherwise: a chat filed, a pin, the strip read.
   *
   * **After the record is heard**, and only what differs: a layout said before would be the
   * empty window's. A tab's strip is where its chat works, which the sidebar says, so the
   * layout is said again once it is read. **A layout the core refused is never said again
   * unchanged** (`layoutRefused`), however often the window draws: the next one that differs is
   * said, as the tabs change.
   */
  useEffect(() => {
    sayLayout.current = (next: Tabs) => {
      if (!layoutHeard.current || heldForward.current) return;
      const laid = layoutOf(
        next,
        {
          workspaceOf: (id) => workspaceOf(next, id, filedIn),
          frontOf: (workspace) => lastFront.current[workspace],
          told: (session) =>
            !pretendedNow.current.has(session) && !endedInPanes.current.has(session),
        },
        tabWindow ?? 0,
      );
      const text = JSON.stringify(laid);
      if (text === layoutSaid.current || text === layoutRefused.current) return;
      layoutSaid.current = text;
      const unsaid = () => {
        if (layoutSaid.current === text) layoutSaid.current = undefined;
      };
      void commands
        .windowLayout(plane, laid)
        .then((answer) => {
          // The core's refusal is its verdict on this layout: not said again unchanged.
          if (answer.status !== "ok") {
            unsaid();
            layoutRefused.current = text;
          }
          // A layout taken: one refused before may be taken now, once the person goes back to it.
          else if (layoutRefused.current !== text) layoutRefused.current = undefined;
        })
        // A call that never reached the core is no verdict: said again at the next draw.
        .catch(unsaid);
    };
    sayLayout.current(tabs);
  }, [filedIn, holdingForward, plane, tabs, tabWindow, viewsHeard]);

  /**
   * **Tabs moved between the project's windows** (#4768): Move to new window, Move to window ▸,
   * a tear-off or a tab window closing. The core moved them in one step and tells every window
   * of the project what it holds now; this window lets go of its tabs that are elsewhere and
   * draws the ones that came to it (`tabWindows.ts`). Nothing ends: a tab that left takes its
   * chats on screen with it.
   */
  // Heard once per project: what it reads is the window's now, so a workspace removed, which
  // files chats again, does not listen anew.
  const layoutMovedHere = useRef<(moved: LayoutMoved) => void>(() => undefined);
  useEffect(() => {
    layoutMovedHere.current = (moved) => {
      windowsOf(moved.layout);
      void commands
        .openedChats(plane)
        .then((answer) => {
          const open = answer.status === "ok" ? (answer.data ?? []) : [];
          const reopening = reopeningOf(new Map(open.map((chat) => [chat.session, chat])));
          change(
            (tabs) =>
              movedBetweenWindows(moved.layout, tabs, tabWindow ?? 0, reopening, filedIn).tabs,
          );
        })
        .catch(() => undefined);
    };
  }, [change, filedIn, plane, reopeningOf, tabWindow, windowsOf]);
  // **What a tab window's own report changed** (#5225): nothing moved, so only what each shows.
  useEffect(() => {
    const listening = listen<LayoutMoved>(LAYOUT_REPORTED, (event) => {
      if (event.payload.plane === plane) windowsOf(event.payload.layout);
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane, windowsOf]);
  useEffect(() => {
    const listening = listen<LayoutMoved>(LAYOUT_MOVED, (event) => {
      if (event.payload.plane === plane) layoutMovedHere.current(event.payload);
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane]);

  /** **A press in another of the project's windows landed on a chat this one has in a pane**
   *  (#4768): it is shown here, in its pane, and this window has come forward. */
  const showChatHere = useRef<(session: number, handed?: HandedChat) => void>(() => undefined);
  useEffect(() => {
    const listening = listen<PressHere>(PRESS_HERE, (event) => {
      if (event.payload.plane === plane)
        showChatHere.current(event.payload.chat, event.payload.handed);
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane]);

  /**
   * Asks which profile and which persona. It starts nothing by itself.
   *
   * **On a machine with no harness it offers the setup tab instead** (FR-29c, #963): the picker
   * would list harnesses none of which can start, so a new chat from the `+`, the palette or a
   * split brings forward the harness setup tab for the directory it would have started in, as
   * the first chat does (FR-29). A split splits nothing then (D-963-2). The machine is looked
   * at beside the options, so the picker waits for no extra round trip.
   *
   * **It opens on the cheap first answer** (#4736): the personas, the profiles and the default
   * persona. Each row's approval, sandbox and billing take a moment more (a sandbox compile per
   * harness, a check of each approved profile's program), so they are asked at the same time and
   * fill the rows in as they arrive. A start pressed before then waits for them (`StartChat`),
   * and the core's start asks every gate again whatever the picker drew.
   */
  const ask = useCallback(
    async (where: Where) => {
      setPickerTrouble(undefined);
      const cwd = ("in" in where ? where.in : undefined) ?? startIn;
      // The workspace the chat would be filed in, or none at the project's root.
      const filed = "pool" in where && where.pool ? where.pool.workspace : filedFor(cwd, focused);
      const asked = ++pickerAsked.current;
      const details = commands
        .startDetails(plane)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      const [options, found, runsOn] = await Promise.all([
        commands
          .startOptionsFirst(plane)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) })),
        commands.harnessSetupFound().catch(() => undefined),
        readRunsOn(filed === OUTSIDE ? null : filed),
      ]);
      if (options.status === "error") {
        refusedBy("start-options", options.error);
        return;
      }
      succeeded("start-options");
      if (cwd !== null && found?.status === "ok" && nothingCanStart(options.data, found.data)) {
        const title = harnessSetupTitle(filed === OUTSIDE ? OUTSIDE_TITLE : filed);
        change((tabs) => openView(tabs, harnessSetupView(cwd), title, filed));
        return;
      }
      setPicking({ options: options.data, where, runsOn, pending: true });
      const read = await details;
      if (pickerAsked.current !== asked) return;
      // Details that could not be read leave the rows as the first answer drew them, and say
      // why: a start then goes to the core, whose gate asks them again.
      if (read.status === "error") setPickerTrouble(read.error);
      setPicking((was) =>
        was === undefined || was.pending !== true
          ? was
          : {
              ...was,
              options: read.status === "ok" ? withDetails(was.options, read.data) : was.options,
              pending: false,
            },
      );
    },
    [change, focused, plane, readRunsOn, refusedBy, startIn, succeeded],
  );

  /**
   * **A new chat on pool `pool`** (#4483): Pool setup's Open a chat. The picker opens with the pool
   * chosen on its Runs on row, in workspace `workspace` where the tab names one and else the
   * project's first, as Try it started its chat: a chat at the project's root runs here. The
   * person still presses Start.
   */
  const newChatOn = useCallback(
    (pool: string, workspace?: string) => {
      const workspaces = sidebar?.workspaces ?? [];
      const ws =
        workspaces.find((one) => one.name === workspace) ??
        [...workspaces].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))[0];
      void ask(
        ws === undefined
          ? { tab: true }
          : { tab: true, in: ws.path, pool: { name: pool, workspace: ws.name } },
      );
    },
    [ask, sidebar],
  );

  /**
   * **A fix the picker's refusal offers** (NO-8, #1233): the doctor's fix by its id, through the
   * one entry point `charter doctor --fix` and the Doctor dialog use. Then the picker reads its
   * options again, so what it draws is what is true after the fix; a refusal is said in it.
   */
  const fixInPicker = useCallback(
    async (id: DoctorFix) => {
      setPickerFixing(true);
      setPickerTrouble(undefined);
      const fixed = await commands
        .planeDoctorFix(plane, id)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      const refused =
        fixed.status === "error"
          ? fixed.error
          : (fixed.data.refused ?? (fixed.data.complete ? undefined : fixed.data.said.join(" ")));
      const options = await commands
        .startOptions(plane)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      setPickerFixing(false);
      if (options.status === "ok") {
        // The whole answer, details and all: any details still being read are for what was
        // true before the fix.
        pickerAsked.current++;
        setPicking((was) =>
          was === undefined ? was : { ...was, options: options.data, pending: false },
        );
      }
      setPickerTrouble(refused ?? (options.status === "error" ? options.error : undefined));
    },
    [plane],
  );

  /** **New chat** (#5289): the picker, and the chat it starts opens by the one rule. An empty
   *  pane it would fill now is the pane it was pressed for, whichever tab is in front by the time
   *  the start answers. */
  const newChat = useCallback(
    ({ ownTab = false }: { ownTab?: boolean } = {}) => {
      // From the strip's `+` list (#5292): "a new tab with this", whatever waits empty.
      if (ownTab) return void ask({ tab: true, ownTab });
      const opens = opensAt(now.current, { kind: "new" }, pressing);
      void ask({ tab: true, pane: opens.how === "fills" ? opens.at : undefined });
    },
    [ask, pressing],
  );
  /** **The strip's `+`** (#4758; spec story 13): a tab with one empty pane on the strip in front,
   *  in front, which New chat then fills. */
  const newTab = useCallback(
    () => void change((tabs) => openEmptyTab(tabs, focused ?? OUTSIDE)),
    [change, focused],
  );
  /** A new tab whose chat starts in that directory — this one, and not the next. */
  const newChatIn = useCallback((path: string) => void ask({ tab: true, in: path }), [ask]);

  /**
   * **A shell** (SI-5): the operator's own `$SHELL` in `cwd`, filed on `filed`'s strip. `open_session`
   * with no program is the core's shell, and the core puts charter's shims first on its `PATH`
   * (ADR 0062). Nothing asks first: a shell starts no harness, so there is no profile for the
   * picker to ask about.
   *
   * **It follows a chat's rules** (#4761, ADR 0062 as amended 2026-11-02): it takes the focused
   * pane as a press does, with the keyboard, and what that pane showed keeps running in the
   * sidebar. **A one-job shell** (`oneJob`, a forge login typed into it) opens as a split
   * beside the focused pane instead, goes when it exits and is never recorded.
   *
   * Named `shell <N>` as the chat's own name, so the pane that comes back from the record reads
   * the same: a chat's name is what its pane is drawn from at a relaunch.
   */
  const startShell = useCallback(
    (
      start: (name: string) => ReturnType<typeof commands.openSession>,
      filed: string,
      typed?: Typed,
      named = false,
      oneJob = false,
      ownTab = false,
    ) => {
      const name =
        oneJob && typed?.line !== undefined ? loginName(typed.line) : `shell ${takeChatNumber()}`;
      void start(name)
        .then((opened) => {
          if (opened.status === "error") {
            refusedBy("shell", opened.error);
            return;
          }
          succeeded("shell");
          const session = opened.data;
          setStartedIn((was) => ({ ...was, [session]: filed }));
          if (oneJob) {
            setShells((was) => new Set(was).add(session));
            setOneJobs((was) => new Set(was).add(session));
            change((tabs) => openOneJob(tabs, session, name));
            giveKeyboardTo(plane, session);
          }
          // A shell follows a chat's rules (#4758; spec story 63): it opens by the one rule.
          else
            showStarted({ session, chat: name }, filed, named, undefined, ownTab, { shell: true });
          // A command typed in for the operator, run as if they had typed it: the shell reads
          // it once it is up, and the tab is theirs to leave. A harness's installer is named,
          // never spelled: the core types its own compiled-in line (FR-29, V65).
          if (typed?.line !== undefined)
            void commands.sendInput(plane, session, typed.held ? typed.line : `${typed.line}\n`);
          else if (typed?.installer !== undefined)
            void commands
              .typeInstaller(plane, session, typed.installer)
              .then((said) => {
                if (said.status === "error") refusedBy("shell", said.error);
              })
              .catch((err: unknown) => refusedBy("shell", String(err)));
          // A profile's sign-in (#2035): named, never spelled. The core types the harness's own
          // sign-in with the profile's folder set, and the person drives it in this tab.
          else if (typed?.signIn !== undefined)
            void commands
              .typeProfileSignIn(
                plane,
                session,
                typed.signIn,
                typed.provider ?? null,
                typed.pool ?? null,
                typed.device ?? false,
              )
              .then((said) => {
                if (said.status === "error") refusedBy("shell", said.error);
              })
              .catch((err: unknown) => refusedBy("shell", String(err)));
          // A sign-out that asks in its terminal (#4124): named, never spelled, as a sign-in.
          else if (typed?.signOut !== undefined) {
            signingOutIn(session, typed.pool);
            void commands
              .typeProfileSignOut(plane, session, typed.signOut, typed.pool)
              .then((said) => {
                if (said.status === "error") refusedBy("shell", said.error);
              })
              .catch((err: unknown) => refusedBy("shell", String(err)));
          }
        })
        .catch((err: unknown) => refusedBy("shell", String(err)));
    },
    [change, plane, refusedBy, showStarted, signingOutIn, succeeded, takeChatNumber],
  );
  const openShell = useCallback(
    (cwd: string | null, filed: string, typed?: Typed, named = false, ownTab = false) =>
      startShell(
        (name) =>
          commands.openSession(
            plane,
            null,
            [],
            cwd,
            name,
            STARTING_SIZE.columns,
            STARTING_SIZE.rows,
          ),
        filed,
        typed,
        named,
        false,
        ownTab,
      ),
    [plane, startShell],
  );
  /**
   * **A forge login, as a one-job program** (#4761; ADR 0062 as amended 2026-11-02): `line`
   * typed into the operator's shell in `cwd`, which opens as a split beside the focused pane and
   * goes when it exits, never recorded (`open_one_job_shell`). `held` leaves the line for the
   * person to read before Return (NO-8).
   */
  const openLogin = useCallback(
    (cwd: string, line: string, held = false) =>
      startShell(
        (name) =>
          commands.openOneJobShell(plane, cwd, name, STARTING_SIZE.columns, STARTING_SIZE.rows),
        OUTSIDE,
        { line, held },
        false,
        true,
      ),
    [plane, startShell],
  );

  /**
   * **A shell tab in a folder of a branch** (FM-10): `openShell`'s tab, started by the core in
   * the folder it resolved — inside the branch, through no link — so the window never names the
   * directory. Filed on the branch's workspace's strip.
   */
  const shellInFolder = useCallback(
    (at: BranchPath) =>
      startShell(
        (name) =>
          commands.openShellInBranch(
            plane,
            at.workspace,
            at.repo,
            at.piece,
            at.path,
            name,
            STARTING_SIZE.columns,
            STARTING_SIZE.rows,
          ),
        at.workspace,
        undefined,
        true,
      ),
    [plane, startShell],
  );

  /** **A terminal editor the core opened** (#1042), a one-job program (#4761, ADR 0081 as
   *  amended 2026-11-02): a split beside the focused pane, in the same tab, with the keyboard,
   *  filed on its workspace and marked as a shell. It goes when the editor exits. */
  const editorTab = useCallback(
    (opened: EditorTabOpened) => {
      setShells((was) => new Set(was).add(opened.session));
      setOneJobs((was) => new Set(was).add(opened.session));
      setStartedIn((was) => ({ ...was, [opened.session]: opened.workspace }));
      change((tabs) => openOneJob(tabs, opened.session, opened.name));
      giveKeyboardTo(plane, opened.session);
    },
    [change, plane],
  );
  useEditorTabs(plane, editorTab);

  /**
   * **A file or folder row's own rows** (FM-10): its path copied, revealed, or handed to your
   * editor. Each is the core's to place and carry out; a refusal comes back in its sentence.
   */
  const fileDoing = useMemo(() => {
    const answered = (asked: Promise<{ status: "ok" } | { status: "error"; error: string }>) =>
      asked
        .then((said): Ran =>
          said.status === "error" ? { ok: false, refused: said.error } : { ok: true },
        )
        .catch((err: unknown): Ran => ({ ok: false, refused: String(err) }));
    return {
      copyPath: async (at: BranchPath, absolute: boolean): Promise<Ran> => {
        const said = await answered(
          commands.copyBranchPath(plane, at.workspace, at.repo, at.piece, at.path, absolute),
        );
        return said.ok
          ? { ok: true, said: `Copied ${absolute ? "the absolute" : "the"} path of ${at.path}.` }
          : said;
      },
      revealPath: (at: BranchPath) =>
        answered(commands.revealBranchPath(plane, at.workspace, at.repo, at.piece, at.path)),
      // A LIVE workspace's own entry (#5377): its mark is read from the project's `.gitignore`,
      // which no folder watch sees, so the trees showing the workspace read it again.
      saveWithTheProject: async (at: BranchPath): Promise<Ran> => {
        const said = await answered(
          commands.workspaceSaveWithTheProject(plane, at.workspace, at.path),
        );
        if (!said.ok) return said;
        keptChanged(plane, at.workspace);
        return {
          ok: true,
          said: `${at.path} is saved with the project. Your next save shares it.`,
        };
      },
      // **Pin to workspace** (#5953): its dialog asks first, and pins on the person's answer.
      pinToWorkspace: async (at: BranchPath): Promise<Ran> => {
        setPinningPage(at);
        return { ok: true };
      },
      openInEditor: async (at: BranchPath, line: number): Promise<Ran> => {
        const editor = yourEditor();
        // The refusal names a setting, so it carries the way to it (#1201, #1244).
        if (editor === undefined)
          return {
            ok: false,
            refused: NO_EDITOR,
            settings: { label: "Choose your editor", link: CHOOSE_EDITOR },
          };
        return answered(
          commands.openInYourEditor(plane, at.workspace, at.repo, at.piece, at.path, line, editor),
        );
      },
    };
  }, [plane]);

  /**
   * A one-job shell at the project root with `shellAsked`'s command typed in (FR-4: `gh auth
   * login` or `glab auth login`, from the first run's "Sign in to GitHub" or "Sign in to
   * GitLab"). Once the plane is read, so the root is known, and once per ask.
   */
  const shellHandled = useRef<number | undefined>(undefined);
  const root = sidebar?.root;
  useEffect(() => {
    if (shellAsked === undefined || root === undefined) return;
    if (shellHandled.current === shellAsked.at) return;
    shellHandled.current = shellAsked.at;
    // A forge's own login (FR-4): a one-job program beside the focused pane (#4761).
    openLogin(root, shellAsked.typed);
  }, [openLogin, root, shellAsked]);

  /** A shell tab where a new chat would start — or in `workspace`'s own directory, filed under
   *  it, when a row names one. */
  const newShell = useCallback(
    (workspace?: string, { ownTab = false }: { ownTab?: boolean } = {}) => {
      if (workspace === undefined) {
        // From the strip's `+` list (#5292): a tab of its own, whatever waits empty.
        openShell(startIn, filedFor(startIn, focused), undefined, false, ownTab);
        return;
      }
      // The plane root's own row (SI-1): the plane's directory, filed on the root's strip.
      if (workspace === OUTSIDE) {
        if (sidebar !== undefined) openShell(sidebar.root, OUTSIDE);
        return;
      }
      const path = sidebar?.workspaces.find((ws) => ws.name === workspace)?.path;
      if (path !== undefined) openShell(path, workspace, undefined, true);
    },
    [focused, openShell, sidebar, startIn],
  );

  /**
   * **A forge login the repo picker asked for** (NO-8, #1233): the forge CLI's own login, typed
   * in a shell tab at the project root and left for the operator to run — its host is the
   * project's, so they read the line before Return.
   */
  useEffect(() => {
    const login = (event: Event) => {
      const asked = (event as CustomEvent<LoginAsk>).detail;
      if (asked.plane !== plane || sidebar === undefined) return;
      // "Typed, not run" holds here, not only in the core's host check: a line with a control
      // character in it (a newline is a Return), or any other the one rule hides, is refused
      // and typed nowhere (#3470).
      if (!typableLogin(asked.line)) {
        refusedBy(
          "shell",
          "purlis will not type a login line that holds a control or a hidden character. Type the login in a terminal yourself.",
        );
        return;
      }
      openLogin(sidebar.root, asked.line, true);
    };
    window.addEventListener(FORGE_LOGIN, login);
    return () => window.removeEventListener(FORGE_LOGIN, login);
  }, [openLogin, plane, refusedBy, sidebar]);

  /**
   * **A blocked save's two ways out** (charter-app#295), asked by the Saving tab: a chat started
   * in the plane — the picker, so the operator chooses who resolves it — or a plain terminal
   * there, a shell with no harness, for somebody who resolves a conflict with git by hand. The
   * terminal is a shell tab like any other, opened by the same function.
   */
  useEffect(() => {
    const out = (event: Event) => {
      const asked = (event as CustomEvent<WayOut>).detail;
      if (asked.plane !== plane || sidebar === undefined) return;
      if (asked.way === "chat") {
        newChatIn(sidebar.root);
        return;
      }
      openShell(sidebar.root, OUTSIDE);
    };
    window.addEventListener(WAY_OUT, out);
    return () => window.removeEventListener(WAY_OUT, out);
  }, [newChatIn, openShell, plane, sidebar]);

  /**
   * **The harness setup tab's two asks** (FR-29): a harness's installer, typed by the core
   * into a shell tab at the project's root, or the picker to start a chat in the directory the
   * tab is about, on the harness it names, once one is installed.
   */
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<HarnessSetupAsk>).detail;
      if (wanted.plane !== plane) return;
      if (wanted.way === "chat") {
        void ask({ tab: true, in: wanted.cwd, prefer: wanted.harness });
        return;
      }
      // At the project's root, as FR-4's sign-in is, and never in the fresh clone: its own
      // shell hooks (direnv, mise) have nothing to do with installing a harness.
      // Filed on the workspace's strip, beside the setup tab the operator comes back to.
      if (root !== undefined) openShell(root, wanted.workspace, { installer: wanted.harness });
    };
    window.addEventListener(HARNESS_SETUP, asked);
    return () => window.removeEventListener(HARNESS_SETUP, asked);
  }, [ask, openShell, plane, root]);

  /**
   * **Sign in a profile** (#2035, ADR 0095 §3), from Settings or the picker: a shell tab at the
   * project's root, outside every workspace, with the profile's own sign-in typed and run by the
   * core. The person drives the harness's flow there; purlis reads none of it.
   */
  const signIn = useCallback(
    (profile: string, provider?: string, pool?: string, device?: boolean) => {
      if (root !== undefined) openShell(root, OUTSIDE, { signIn: profile, provider, pool, device });
    },
    [openShell, root],
  );
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<ProfileSignInAsk>).detail;
      if (wanted.plane === plane)
        signIn(wanted.profile, wanted.provider, wanted.pool, wanted.device);
    };
    window.addEventListener(PROFILE_SIGN_IN, asked);
    return () => window.removeEventListener(PROFILE_SIGN_IN, asked);
  }, [plane, signIn]);
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<ProfileSignOutAsk>).detail;
      if (wanted.plane === plane && root !== undefined)
        openShell(root, OUTSIDE, { signOut: wanted.profile, pool: wanted.pool });
    };
    window.addEventListener(PROFILE_SIGN_OUT, asked);
    return () => window.removeEventListener(PROFILE_SIGN_OUT, asked);
  }, [plane, root, openShell]);

  /**
   * A harness started by hand in one of this project's shell tabs (ADR 0062): the core says so,
   * and the tab's pane draws a banner until it is answered. Filtered on the plane, as
   * `chat-moved` is. A window that cannot listen (a unit test with no events) never hears one.
   */
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ByHand>("harness-by-hand", (event) => {
          const told = event.payload;
          if (gone || told.plane !== plane) return;
          setByHand((was) => ({
            ...was,
            [told.session]: { harness: told.harness, cwd: told.cwd },
          }));
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);

  const dropStartNote = useCallback((session: number) => {
    setStartNotes((was) => withoutKey(was, session));
  }, []);
  /** A start notice put away: it is said once, and the operator has read it. */
  const dismissStartNote = useCallback(
    (session: number) => {
      dropStartNote(session);
      noteCleared(session, { kind: "start" });
    },
    [dropStartNote, noteCleared],
  );

  const dropByHand = useCallback((session: number) => {
    setByHand((was) => withoutKey(was, session));
  }, []);
  /** The banner's two answers: open that harness as a chat where the shell was standing — the
   *  picker, started on that harness — or put the banner away. Either way it is answered. */
  const answerByHand = useCallback(
    (session: number, open: boolean) => {
      const note = byHand[session];
      dropByHand(session);
      noteCleared(session, { kind: "by-hand" });
      if (!open || note === undefined) return;
      void ask({ tab: true, in: note.cwd ?? startIn ?? undefined, prefer: note.harness });
    },
    [ask, byHand, dropByHand, noteCleared, startIn],
  );

  /**
   * Starts a chat on `profile` at `where`: the picker's Start, and the first chat of a repo
   * opened into this project when there was nothing to pick (FR-4). Answers the core's refusal,
   * or `undefined` once the chat is on a tab. `filedIn` names the strip the tab goes on when the
   * caller has just focused one this render cannot see yet.
   */
  const startOn = useCallback(
    async (
      where: Where,
      kind: string | undefined,
      profile: string,
      persona: string | null,
      showFooter: boolean,
      label: string | null,
      newBranch: boolean,
      filedIn?: string,
      withoutSandbox: WithoutSandbox | null = null,
      runsOn: RunsOnPick | null = null,
    ): Promise<LimitRefused | undefined> => {
      // A tab asked for in one directory starts there; everything else starts where the
      // explorer's pick says (charter-app#174).
      const cwd = ("in" in where ? where.in : undefined) ?? startIn;
      const inFrontTab = now.current.inFront;
      // The tab's CHAT name, not the sentence the tab bar draws: the core is being told what
      // this chat is called, and a split's chat is called what the tab's chat is called. The
      // persona the tab also shows is the operator's, not part of the chat's name.
      //
      // A tab that opened on a view has no chat to share a name with, so a chat started beside
      // it is named as a new tab's would be.
      const name =
        ("split" in where && inFrontTab !== undefined
          ? chatNameOf(now.current, inFrontTab)
          : undefined) ?? String(takeChatNumber());
      const started = await commands
        .startChat(
          plane,
          profile,
          persona,
          cwd,
          name,
          label,
          // The core cuts the branch only when `cwd` is a repo's clone (GL-1).
          // The opt-out is the picker's alone (ADR 0067 §7): every other start sends none.
          // Runs on for this start alone (#4536), sent only where the person chose one: left out,
          // the workspace's and the project's own decide.
          {
            show_footer: showFooter,
            new_branch: newBranch,
            without_sandbox: withoutSandbox,
            ...(runsOn === null ? {} : { runs_on: runsOn }),
          },
          STARTING_SIZE.columns,
          STARTING_SIZE.rows,
        )
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (started.status === "error") return refusedWith(started.error);
      const session = started.data.session;
      // `?? []`, as the label's `?? null` below: an older core, or a stand-in, sends none.
      const notes = started.data.notices ?? [];
      const agentsMd = started.data.agents_md ?? [];
      if (notes.length > 0) setStartNotes((was) => ({ ...was, [session]: { notes, agentsMd } }));
      // Where charter put it, written down before the tab is drawn: the plane will say the
      // same thing a tick later, and until it does this is what keeps the tab on the strip
      // the operator is looking at.
      const filed = filedIn ?? filedFor(cwd, focused);
      setStartedIn((was) => ({ ...was, [session]: filed }));
      const pressed: PressedChat = {
        session,
        chat: name,
        who: whoOf(persona, kind),
        label: started.data.label ?? null,
      };
      if ("tab" in where) {
        // **It opens by the one rule** (#5289), and replaces nothing. A start that names its
        // place, a directory or a strip the caller chose, keeps that place's strip.
        const named = where.in !== undefined || filedIn !== undefined;
        showStarted(pressed, filed, named, where.pane, where.ownTab);
        return undefined;
      }
      // The tab that was to be split can have gone while the picker was open: the chat goes on
      // in the sidebar, as every chat in no pane does (#4756). A split is a layout change, so
      // Undo puts it back (#4763); its chat runs on in the sidebar. **Only the tab that was
      // split**: every strip keeps a tab (#4758), so another one, such as the empty tab left
      // where that one went, can be in front by now, and is not the tab the person split.
      // What it is called until its row arrives (#5308), drawn with its pane.
      setStartedAs((was) => new Map(was).set(session, pressed));
      change((tabs) =>
        inFrontTab === undefined || tabs.inFront !== inFrontTab
          ? tabs
          : recorded(tabs, splitFocusedPane(tabs, where.split, session, name), "Split the pane."),
      );
      return undefined;
    },
    [change, focused, plane, showStarted, startIn, takeChatNumber],
  );

  /** A row was picked: the chat starts on that profile, with that persona, either drawing
   *  charter's footer in its pane or leaving it blank (ADR 0029), and under the name typed in
   *  the picker, if one was (charter-app#254). */
  /** Whether the picker's start is in flight: the ref is the guard, the state is the button. */
  const startingNow = useRef(false);
  const [starting, setStarting] = useState(false);
  /** The workspace the picker's chat starts in: the one a pool preset names (#4483), else the
   *  one in front. */
  const pickingIn =
    picking !== undefined && "pool" in picking.where && picking.where.pool
      ? picking.where.pool.workspace
      : ofWorkspace;
  const readStage = useCallback(
    (runsOn: RunsOnPick | null) => readRunnerStage(plane, pickingIn, runsOn),
    [plane, pickingIn],
  );
  /** The person cancelled the start in flight: what it answers is said to nobody. */
  const startLeft = useRef(false);
  /** Which start is the dialog's: one the person cancelled may still be winding down. */
  const startRun = useRef(0);
  const startPicked = useCallback(
    async (
      profile: string,
      persona: string | null,
      showFooter: boolean,
      label: string | null,
      newBranch: boolean,
      withoutSandbox: WithoutSandbox | null,
      runsOn: RunsOnPick | null = null,
    ) => {
      if (picking === undefined || startingNow.current) return;
      const kind = picking.options.profiles.find((one) => one.name === profile)?.kind;
      // **One start per picker at a time** (GL-1). A start runs off the main thread for as
      // long as its branch takes to check out, and a second press of Start in that time was a
      // second chat. The ref answers at once, where state would answer on the next render.
      startingNow.current = true;
      startLeft.current = false;
      const thisStart = ++startRun.current;
      setStarting(true);
      const started = await startOn(
        picking.where,
        kind,
        profile,
        persona,
        showFooter,
        label,
        newBranch,
        undefined,
        withoutSandbox,
        runsOn,
      ).finally(() => {
        if (startRun.current !== thisStart) return;
        startingNow.current = false;
        setStarting(false);
      });
      if (startRun.current !== thisStart || startLeft.current) return;
      if (started !== undefined) {
        // In the picker, not behind it: the operator is still choosing, and a refusal they
        // cannot see beside the rows is one they cannot act on.
        // Over a pool's limit, what to raise it to came with the refusal, as data.
        const { words: refused, limit: offer } = started;
        setPickerTrouble(refused);
        setPickerSignIn(undefined);
        setPickerLimit(offer === null ? undefined : { offer, refused, profile });
        // Its profile's credential is a Secret ask (#5137), named by the core, never read from
        // the refusal's words: the picker fills it, and this start goes on.
        const asked = started.secret_ask ?? undefined;
        setPickerSecretSaid(undefined);
        setPickerSecret(asked === undefined ? undefined : { asked, refused, profile });
        // A pool that is gone, stopped or not reached, as the core found it (#4536).
        const gone = started.pool_unavailable ?? undefined;
        setPickerPoolGone(gone === undefined ? undefined : { pool: gone, refused });
        // The refusal left an Ask to sign the profile in (#2035): the Inbox reads it now.
        asksMoved(plane);
        // A profile of this machine's its harness says is not signed in here points to that
        // Ask (#2035): asked of the core, never read from the refusal's words.
        const row = picking.options.profiles.find((one) => one.name === profile);
        if (row?.source === "this machine") {
          const mine = await commands
            .machineProfiles()
            .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
          if (
            mine.status === "ok" &&
            mine.data.some((one) => one.name === profile && one.standing === "signed-out")
          )
            setPickerSignIn({ profile, refused });
        }
        return;
      }
      setPicking(undefined);
      setPickerTrouble(undefined);
      setPickerSignIn(undefined);
      setPickerPoolGone(undefined);
    },
    [picking, plane, startOn],
  );

  /** The approval IS this click. After it, the whole chain of checks runs again from the
   *  top before anything is exec'd, so a yes never walks past a refusal standing behind it. */
  const approveAndStart = useCallback(
    async (
      profile: string,
      persona: string | null,
      showFooter: boolean,
      shown: string,
      label: string | null,
      newBranch: boolean,
      withoutSandbox: WithoutSandbox | null,
      runsOn: RunsOnPick | null = null,
    ) => {
      // A start already running from this picker is the one start it gets (GL-1).
      if (startingNow.current) return;
      const said = await commands
        .approveProfile(plane, profile, shown)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") {
        setPickerTrouble(said.error);
        return;
      }
      // The persona the OPERATOR picked, carried up from the dialog with the profile. It
      // used to take the plane's default out of the options instead, which silently threw
      // away the choice on the one path where a profile is being used for the first time.
      // The footer choice rides the same path, and for the same reason: a first run of a
      // profile is exactly where a dropped choice would go unnoticed.
      await startPicked(profile, persona, showFooter, label, newBranch, withoutSandbox, runsOn);
    },
    [plane, startPicked],
  );

  /** **The picker's Secret ask** (#5137): the open ask, in this project's list, for the vault
   *  and key the core named beside the refusal shown. None where it is answered elsewhere or not
   *  listed yet: the refusal's words are said, as before. */
  const pickerAsk =
    pickerSecret !== undefined && pickerSecret.refused === pickerTrouble
      ? waiting?.find(
          (one) =>
            one.answer.via === "secret" &&
            one.secret?.vault === pickerSecret.asked.vault &&
            one.secret?.key === pickerSecret.asked.key,
        )
      : undefined;
  /** **Answers the picker's Secret ask** (#5137): through the one ask for that vault and key, as
   *  the Inbox answers it, so it closes there too, and every list of this project's asks is read
   *  again. Answers how it ended: the picker starts again with what it shows now, never what it
   *  showed at the refusal, and only while it is open. Not now says the core's words for a start
   *  whose credential was declined. A refusal of a value typed says to type it again, since its
   *  box gave it up as it was read (#5902). With no ask, or one being answered, nothing is sent
   *  and nothing said: `unsent`, so no older line reads as this answer's (#5902). */
  const answerPickerAsk = useCallback(
    async (option: Offered, given: () => SecretAskGiven): Promise<SecretAskEnded> => {
      const unsent = pickerAnswerUnsent(pickerAsk, pickerSecret, pickerSecretBusy);
      if (unsent !== undefined || pickerAsk === undefined || pickerSecret === undefined)
        return "unsent";
      const { asked } = pickerSecret;
      setPickerSecretBusy(true);
      setPickerSecretSaid(undefined);
      // The value is read from its box only now, and the box emptied.
      const got = given();
      try {
        const { refused } = await answerTaken(
          plane,
          pickerAsk,
          option.id,
          undefined,
          got.typed,
          got.picked,
          got.remember,
          undefined,
          got.named,
        );
        if (refused !== undefined) {
          setPickerSecretSaid(refusedSaid(refused, typedIn(got), option));
          return "refused";
        }
        setPickerSecret(undefined);
        if (option.id === "keep") {
          setPickerTrouble(asked.declined);
          return "declined";
        }
        setPickerTrouble(undefined);
        return "filled";
      } finally {
        setPickerSecretBusy(false);
        asksMoved(plane);
      }
    },
    [pickerAsk, pickerSecret, pickerSecretBusy, plane],
  );

  const split = useCallback((direction: Direction) => void ask({ split: direction }), [ask]);
  // Tile all, its Undo and Focus this pane (#4765): the tab's own panes rearranged, nothing
  // started or ended.
  const tileAllIn = useCallback((tab: number) => change((tabs) => tileAll(tabs, tab)), [change]);
  const zoomPane = useCallback(() => change(zoomFocusedPane), [change]);

  useEffect(() => {
    listedNow.current = chatsByNumber;
  }, [chatsByNumber]);

  /**
   * **The keyboard goes into the focused pane of `next`** (#4767): its chat's terminal, or, in a
   * pane with nothing or a view, its first control once it is drawn: New chat, or the view.
   */
  const keyboardInto = useCallback(
    (next: Tabs) => {
      const session = focusedChat(next);
      if (session !== undefined) {
        giveKeyboardTo(plane, session);
        return;
      }
      setTimeout(() => {
        const pane = document.querySelector<HTMLElement>(".pane.focused");
        const into =
          pane?.querySelector<HTMLElement>('[data-scope="view-frame"][data-part="root"]') ??
          pane?.querySelector<HTMLElement>("button");
        into?.focus();
      });
    },
    [plane],
  );

  /**
   * **Close pane: the focused pane is cleared, and its chat goes on in the sidebar** (#4756).
   * Nothing ends, and its tasks stay wherever they are shown (#4757).
   */
  const closePane = useCallback(() => {
    const tab =
      now.current.inFront === undefined ? undefined : now.current.byId[now.current.inFront];
    const going = tab && panesOf(now.current, tab.id).find((pane) => pane.pane === tab.focused);
    // A task's own pane is never closed (#1489): its control is the minimise, another verb.
    if (going !== undefined && askedByNow(going.session) !== undefined) return;
    // Nor a one-job program's (#4761): it goes when its program exits, and the row says so.
    if (going !== undefined && shellsRef.current.oneJobs.has(going.session)) return;
    // A close is a layout change, so Undo puts it back (#4763, #4583 rule 3): it ended nothing.
    keyboardInto(
      change((tabs) =>
        recorded(tabs, closeFocusedPane(tabs, filedIn, isPinned), "Closed the pane."),
      ),
    );
    // Nothing ends, and its tasks stay in whatever panes they are in: a chat leaving moves no
    // other pane (#4757).
  }, [askedByNow, change, filedIn, isPinned, keyboardInto]);

  /**
   * **Clear pane, on an ended chat's band** (#4756, #5019): pane `pane` of the tab in front
   * closes, as Close pane closes one: what shared its split takes its place, and the strip's
   * last tab keeps one empty pane (`closePane`). It ends nothing more: a chat whose program
   * exited is still in the sidebar, where End… takes it away.
   */
  const clearPane = useCallback(
    (pane: number) => {
      const id = now.current.inFront;
      if (id === undefined) return;
      const going = panesOf(now.current, id).find((one) => one.pane === pane);
      if (going === undefined) return;
      forgetEndedHere(going.session);
      change((tabs) => closePaneOfTab(tabs, id, pane, filedIn, isPinned));
    },
    [change, filedIn, forgetEndedHere, isPinned],
  );

  /**
   * **Close tab: the layout closes, and its chats go on in the sidebar** (#4756). The strip's
   * last tab keeps one empty pane (`closeLayout`). A task whose pane was in the tab goes back
   * to the Chats list, and the core is told it has no tab (`close_chat_tab`). A task below a
   * chat of the tab stays in whatever pane it is in: a chat leaving moves no other pane (#4757).
   */
  const close = useCallback(
    (id: number) => {
      const before = now.current;
      const chats = panesOf(before, id).map((one) => one.session);
      // Undo brings the tab back as it was (#4763): it ended nothing.
      change((tabs) =>
        recorded(
          tabs,
          closeLayout(tabs, id, filedIn, isPinned),
          `Closed ${tabs.byId[id]?.name ?? "the tab"}.`,
        ),
      );
      for (const session of chats)
        if (askedByNow(session) !== undefined)
          void commands.closeChatTab(plane, session).catch(() => undefined);
    },
    [askedByNow, change, filedIn, isPinned, plane],
  );

  /**
   * **The tab's own close, as a person presses it** (#5318): closing the tab in front puts the
   * keyboard in what is now in front, as Close pane does, since a tab's only pane has no Close. A
   * tab that has the keyboard itself (Delete on it) keeps the strip's, which stays where it is.
   */
  const closeFromItsControl = useCallback(
    (id: number) => {
      const wasInFront = now.current.inFront === id;
      const onItsTab = document.activeElement?.matches('[role="tab"]') === true;
      close(id);
      if (wasInFront && !onItsTab) keyboardInto(now.current);
    },
    [close, keyboardInto],
  );

  /**
   * Brings a tab to the front — **and the workspace it is on with it**.
   *
   * The strip shows one workspace's chats, so bringing a chat forward from somewhere else
   * has to move the operator to where that chat lives; otherwise the pane would show a chat
   * whose tab is on a strip that is not drawn. Everything that shows a chat comes through
   * here: the strip itself, the palette's `Switch to tab` rows and the needs-you queue.
   *
   * **The strip no longer depends on this line getting it right** — `focused` derives it from
   * the tab in front. What `picked` is for is the moment AFTER this chat's tab closes and
   * there is no front tab left to read the axis off: the window stays in the workspace the
   * operator was in rather than jumping back to wherever they last pressed a strip.
   */
  const bringToFront = useCallback(
    (id: number) => {
      change((tabs) => selectTab(tabs, id));
      const workspace = sidebar === undefined ? undefined : workspaceOf(now.current, id, filedIn);
      if (workspace !== undefined) setPicked(workspace);
    },
    [change, filedIn, sidebar],
  );

  /** A view alone in its tab, split from the tab's menu (#5318): what its splits ask of the pane
   *  in front, once that tab is. `change` has moved the front by the next line. */
  const splitTab = useCallback(
    (id: number, direction: Direction) => {
      bringToFront(id);
      split(direction);
    },
    [bringToFront, split],
  );

  /**
   * **Gives a task a tab of its own** (#1489, V100-38): on the strip of the workspace it works
   * in, in front, with the keyboard in it. Asked for by the person: its row's menu, its line
   * in its tab's menu, its pane's controls, the palette; or by pressing it at all, where the
   * person set tasks to open that way (`chatsListPrefs`, V100-74). Also what a pressed task
   * falls back to when the session that asked has no tab here. **Nothing ends**, and the
   * pane it returns to is left on what it showed. The core is told, so the tab comes back at the
   * next launch.
   */
  const ownTab = useCallback(
    (session: number) => {
      const listed = listedNow.current.get(session);
      if (listed === undefined) return;
      const opened = change((tabs) =>
        moveToOwnTab(
          tabs,
          session,
          listed.name,
          whoOf(listed.persona, listed.harness),
          listed.label,
          askedByNow,
        ),
      );
      void commands.openChatTab(plane, session).catch(() => undefined);
      const tab = tabHolding(opened, session);
      if (tab !== undefined) bringToFront(tab);
      giveKeyboardTo(plane, session);
      stoppedFor(session, undefined);
    },
    [askedByNow, bringToFront, change, plane, stoppedFor],
  );

  /**
   * **Opens a task beside its session, split from the pane it returns to** (#1489, V100-38): the
   * session's chat on one side and the task on the other, with the keyboard in the task. A
   * tab it had of its own goes. Nothing for a task whose session has no tab here: its row
   * says why (`actions.placeRows`).
   */
  const besideOf = useCallback(
    (session: number) => {
      const before = now.current;
      const next = change((tabs) =>
        openBeside(tabs, session, askedByNow, listedNow.current.get(session)?.name),
      );
      if (next === before) return;
      // It is a pane of the tab it returns to now, and no tab of its own.
      void commands.closeChatTab(plane, session).catch(() => undefined);
      const tab = tabHolding(next, session);
      const workspace =
        sidebar === undefined || tab === undefined ? undefined : workspaceOf(next, tab, filedIn);
      if (workspace !== undefined) setPicked(workspace);
      giveKeyboardTo(plane, session);
      stoppedFor(session, undefined);
    },
    [askedByNow, change, filedIn, plane, sidebar, stoppedFor],
  );

  /**
   * **Sends a task back out of its own tab or its pane** (#1489): the minimise. The tab leaves
   * the strip, or the pane leaves the split; **the task goes on**, at home in its session's
   * tab again, whose chip lists it. No command that ends a chat is sent, and nothing is asked.
   * The front goes where closing that tab would send it; a split's other pane takes the room
   * and the keyboard.
   */
  const sendBackOf = useCallback(
    (session: number) => {
      const before = now.current;
      const from = tabHolding(before, session);
      const next = change((tabs) => sendBack(tabs, session, filedIn, isPinned));
      if (next === before) return;
      void commands.closeChatTab(plane, session).catch(() => undefined);
      // What shared its split has the room now, and the keyboard with it.
      const beside = from !== undefined && next.inFront === from ? focusedChat(next) : undefined;
      if (beside !== undefined) giveKeyboardTo(plane, beside);
    },
    [change, filedIn, isPinned, plane],
  );

  /** The chats in the tab in front's other panes, which the sidebar marks faintly (#4757). */
  const besides = useMemo(() => new Set(shownBeside(tabs)), [tabs]);

  /**
   * **A press lands in this window** (#4757, #5289), by the rule. **A tab window keeps the strip
   * it shows** (#5330): a chat in no pane here, filed on another strip, is opened by the project
   * window instead, which comes forward, since this window holds only the tabs torn off into it.
   * Where no window holds the project, it lands here after all.
   */
  const pressHere = useCallback(
    (session: number, into?: PaneAt) => {
      const land = () => landOn((tabs) => pressChat(tabs, pressedOf(session), pressing, into));
      // **Until the list of chats is read, nothing says which strip a chat is on** (#5533): a
      // chat no tab here holds is the project window's, which holds every strip.
      const elsewhere =
        tabWindow !== undefined &&
        into === undefined &&
        (sidebar === undefined
          ? tabHolding(now.current, session) === undefined
          : wouldLeaveStrip(now.current, session, pressing.filedIn));
      if (!elsewhere) {
        land();
        return;
      }
      void pressInProjectWindow(plane, session).then((held) => {
        if (!held) land();
      });
    },
    [landOn, plane, pressedOf, pressing, sidebar, tabWindow],
  );
  /**
   * **Goes to a chat: the one way** (#1486, #4757). Every surface that shows a chat comes
   * through here: a row of the Chats list and of the explorer, the hand on a row above it, the
   * title bar's list, the palette, a needs-you Go, a name in a pane's breadcrumb.
   *
   * **A press opens by the one rule** (`tabs.opensAt`, #5289): a chat on screen in a pane is
   * jumped to, never shown twice; otherwise it fills an empty pane of its workspace's front tab,
   * shows in a one-pane tab, or gets a tab of its own, so a split survives. A task lands like
   * any chat. **A chat a pane showed before keeps running**, and the pane remembers it for ⌘⇧H.
   * `into` is the pane a breadcrumb was pressed in, which walks up its chain in that same pane.
   *
   * The new layout is told in the same call (`change`), so a task pressed onto is held from
   * that moment. A chat listed because its Smart close stopped (SI-8f) has been looked at, so
   * it leaves the list.
   */
  const showChat = useCallback(
    (session: number, into?: PaneAt) => {
      stoppedFor(session, undefined);
      const here = () => pressHere(session, into);
      // **A chat in a pane of another of the project's windows** (#4768), **or a chat in no
      // pane where another window's pane was used last** (#5108): that window comes forward
      // and shows it, and this window changes nothing. Asked only while the project has another
      // window, so a project of one window presses at once.
      if (into !== undefined || oneWindow()) {
        here();
        return;
      }
      void commands
        .pressElsewhere(plane, session)
        .then((answer) => {
          if (answer.status !== "ok" || answer.data === null) here();
        })
        .catch(here);
    },
    [oneWindow, plane, pressHere, stoppedFor],
  );
  /** **A pane of this window was used** (#5108): pressed in, or typed in. Said to the core while
   *  the project has another window, so a press on a chat in no pane, in whichever window, goes
   *  to the most recently used pane across them (`press_elsewhere`). A project of one window
   *  says nothing: its press opens by the one rule there. A pane's focus is no
   *  use of it: the window coming forward hands focus back to the pane that last had it, so a
   *  press on a sidebar row would always be this window's. **Said once until the window has been
   *  away**: no other window's pane can be used while this one keeps the focus. */
  const paneSaidUsed = useRef(false);
  useEffect(() => {
    const away = () => {
      paneSaidUsed.current = false;
    };
    window.addEventListener("blur", away);
    return () => window.removeEventListener("blur", away);
  }, []);
  const paneUsed = useCallback(() => {
    if (oneWindow() || paneSaidUsed.current) return;
    paneSaidUsed.current = true;
    void commands.paneUsed(plane).catch(() => {
      paneSaidUsed.current = false;
    });
  }, [oneWindow, plane]);
  // **The pane it lands in counts as used** (#5330): a press a tab window handed over says so
  // to the core, which counted the tab window's pane, so the next press comes straight here.
  useEffect(() => {
    showChatHere.current = (session, handed) => {
      // A chat a tab window started or resumed (#5533), filed where it said: no list has it yet.
      if (handed === undefined) pressHere(session);
      else {
        setStartedIn((was) => ({ ...was, [session]: handed.filed }));
        if (handed.started) showStarted(handed.as, handed.filed, true, undefined, false, handed);
        else showResumed(handed.as, handed.filed);
      }
      if (tabWindow === undefined) paneUsed();
    };
  }, [paneUsed, pressHere, showResumed, showStarted, tabWindow]);
  // **Go to it, on a chat's arrival card** (#4759): a press on a card said before it was drawn,
  // heard here, where the press as it stands now is. Every project this window holds hears it,
  // and a chat's number means something only in its own.
  useEffect(() => {
    const go = (event: Event) => {
      const { detail } = event as CustomEvent<{ plane: PlaneId; session: number }>;
      if (detail.plane === plane) showChat(detail.session);
    };
    window.addEventListener(GO_TO_ARRIVED, go);
    return () => window.removeEventListener(GO_TO_ARRIVED, go);
  }, [plane, showChat]);
  /**
   * **Move to new window, or Move to window ▸** (#4768): tab `tab`, or its focused pane, goes to
   * another window of the project in one step of the core's, which tells every window what it
   * holds now (`layout-moved`). This window changes nothing itself: it draws its share then.
   */
  const moveToWindow = useCallback(
    async (tab: number, pane: boolean, to: number | null): Promise<Ran> => {
      const tabs = now.current;
      const nth = tabs.order.indexOf(tab);
      const held = tabs.byId[tab];
      if (nth < 0 || held === undefined) return { ok: false, refused: "That tab is not open." };
      const path = pane ? (pathTo(held.layout, held.focused) ?? "") : "";
      const answer = await commands
        .moveToWindow(plane, nth, path === "" ? null : path, to)
        .catch((why: unknown) => ({ status: "error" as const, error: String(why) }));
      return answer.status === "ok" ? { ok: true } : { ok: false, refused: answer.error };
    },
    [plane],
  );
  /**
   * **⌘⇧H: the focused pane goes back to the chat it showed before** (#4757), and again to
   * come back; where that chat is on screen in another pane, that pane comes forward instead.
   * Nothing where the pane showed nothing else, or that chat has ended.
   */
  const goBack = useCallback(() => {
    landOn((tabs) =>
      back(tabs, pressing, (session) =>
        listedNow.current.has(session) ? pressedOf(session) : undefined,
      ),
    );
  }, [landOn, pressedOf, pressing]);
  /** A name pressed in a breadcrumb of the tab in front: walks up the chain in that pane. */
  const showChatFromPane = useCallback(
    (session: number, pane?: number) => {
      const tab = now.current.inFront;
      showChat(session, tab === undefined || pane === undefined ? undefined : { tab, pane });
    },
    [showChat],
  );

  /**
   * **Goes to a task that has ended**, for a needs-you item's Go and the away summary's link
   * (#1491, #1514): its chat, where that is still open (a task that reported blocked stays
   * one), and otherwise its finished row under the session `asker` that asked, which the Chats
   * list brings into view. Where there is nothing to show (no finished row, or the Chats list
   * is not on screen), this says so and shows the session. **A failure its session is flagged
   * for is looked at only once it was shown**: the core is told which, by its id.
   */
  const goToEnded = useCallback(
    (asker: number, ended: FailedBelow) => {
      const flagged = (failedTasks[asker] ?? []).some((one) => one.id === ended.id);
      const open =
        ended.chat === null
          ? undefined
          : chatsListed.current.find((chat) => chat.session === ended.chat);
      if (open !== undefined) {
        showChat(open.session);
        if (flagged) void commands.taskFailureSeen(plane, asker, ended.id).catch(() => undefined);
        return;
      }
      const row = (finishedNow.current.get(asker) ?? []).find((task) => task.id === ended.id);
      if (row === undefined || !chatsListDrawn()) {
        refusedBy(
          "needs-you",
          row === undefined
            ? `${ended.task} has no row left to show. Its chat is shown instead.`
            : `${ended.task} is in the Chats list, which is not on screen. Its chat is shown instead.`,
        );
        showChat(asker);
        return;
      }
      succeeded("needs-you");
      setRevealed((was) => ({
        asker,
        task: row.name,
        id: row.id,
        at: (was?.at ?? 0) + 1,
      }));
    },
    [failedTasks, plane, refusedBy, showChat, succeeded],
  );
  /**
   * **Go, on a needs-you item** (#1491, V100-15). An item that is there because a task failed,
   * ended without a report or did not start **goes to that task**: its chat, where that is
   * still open (a task that reported blocked stays one), and otherwise its finished row
   * under the session that asked, which the Chats list brings into view.
   *
   * **The failure is looked at only once it was shown**, and only that one: the core is told
   * which, by its id, when the chat was shown or the row was found (`failureShown`). Where
   * there is nothing to show (no finished row, or the Chats list is not on screen), this says
   * so, shows the session and leaves the hand where it is: one attempt, and no more.
   *
   * Every other item goes to its chat, as it always did.
   */
  const showNeeding = useCallback(
    (session: number) => {
      const failed = failedTasks[session] ?? [];
      if (failed.length === 0) {
        showChat(session);
        return;
      }
      // The latest, which is the one the item says.
      const latest = failed[failed.length - 1];
      goToEnded(session, latest);
    },
    [failedTasks, goToEnded, showChat],
  );
  /** **Goes to a finished task** from the away summary (#1514): as a needs-you item's Go. */
  const showFinished = useCallback(
    (task: FinishedTask) =>
      goToEnded(task.asker, { id: task.id, task: task.name, chat: task.chat }),
    [goToEnded],
  );
  /**
   * What became of a row the Chats list was asked to bring into view. A failed task's row
   * that was shown has been looked at, so its item goes; one that could not be shown (the
   * list said so) leaves its item, and the session is shown instead.
   */
  const revealedSettled = useCallback(
    (asked: Reveal, shown: boolean) => {
      if (asked.id === undefined) return;
      const id = asked.id;
      // Only a failure its session is flagged for has anything to be looked at.
      if (!shown) showChat(asked.asker);
      else if ((failedTasks[asked.asker] ?? []).some((one) => one.id === id))
        void commands.taskFailureSeen(plane, asked.asker, id).catch(() => undefined);
    },
    [failedTasks, plane, showChat],
  );
  /** The chat of the focused pane of the tab in front: the chat on screen. */
  const chatOnScreen =
    tabs.inFront === undefined
      ? undefined
      : shownIn(tabs, tabs.inFront).find(
          (one) => one.pane === tabs.byId[tabs.inFront ?? -1]?.focused,
        )?.session;
  /** **The chat on screen and focused has been seen** (#5340): what failed of it leaves its mark. */
  useFailuresSeen(plane, chatOnScreen, failedTasks, inFront);
  /** A finished row was opened to be read: where it is a failure its session is flagged for,
   *  it has been looked at (#1491). */
  const lookedAtFinished = useCallback(
    (task: FinishedTask) => {
      if ((failedTasks[task.asker] ?? []).some((failed) => failed.id === task.id))
        void commands.taskFailureSeen(plane, task.asker, task.id).catch(() => undefined);
    },
    [failedTasks, plane],
  );

  /**
   * Opens a view in a tab of its own, **on the strip in front** — or brings forward the tab
   * already showing it, wherever that is, and its strip with it.
   *
   * The strip in front is where the operator opened it from: a persona row on the panel beside
   * this workspace, a palette row pressed while looking at it. A view has no directory to be
   * filed by, so it is filed there, explicitly (`tabs.Content`).
   *
   * **Opening one runs nothing**, but for the Skill studio's read, asked as it opens and taken by
   * its tab (`studioReadAhead.ts`, #4351). An extension's view asks its program when its tab
   * draws it, which is a separate, visible step (`Views.tsx`); the persona view reads the plane.
   *
   * `workspace` files it on that strip instead: a workspace's own settings belong on its strip
   * whichever one is in front (charter-app#280).
   */
  const present = useCallback(
    (open: (tabs: Tabs, strip: string) => Tabs) => {
      const next = change((tabs) => open(tabs, focused ?? OUTSIDE));
      const workspace =
        next.inFront === undefined ? undefined : workspaceOf(next, next.inFront, filedIn);
      if (workspace !== undefined) setPicked(workspace);
    },
    [change, filedIn, focused],
  );
  const showView = useCallback(
    (asked: ViewRef, named: string, on?: string) => {
      // **The Skill studio, opened at a place** (#3681): its one tab per project, gone to that
      // place in its own history, so back returns to where the person was inside it.
      let view = asked;
      let title = named;
      if (isSkillStudio(asked)) {
        const opening = studioOpening(
          asked,
          (key) => findView(now.current, { ...SKILL_STUDIO_VIEW, key }) === undefined,
        );
        view = opening.tab;
        title = SKILL_STUDIO_TITLE;
        if (opening.place !== undefined) {
          // A tab drawn now asks for its skills now, read while it is drawn (#4351); one already
          // open has them.
          if (findView(now.current, view) === undefined)
            readStudioAhead(plane, studioHistoryOf(plane, view), opening.place.scope);
          openViewAt(studioHistoryOf(plane, view), opening.place);
        }
      }
      present((tabs, strip) => openView(tabs, view, title, on ?? strip));
      // Opening an extension's view by any route clears its offer (#4769).
      viewOpened(plane, view);
      // Opening Settings, or bringing it forward, is a way into it: the keyboard goes there too
      // (#1206, `settings/entering.ts`).
      const place = placeOfView(view, plane);
      if (place !== undefined) enterSettings(place);
    },
    [plane, present],
  );
  /** A write on the person's own workspace files (#5376), while it asks; a new file opens in
   *  its file tab once made. */
  const workspaceWrites = useWorkspaceWrites(plane, (does, path) => {
    if (does.verb !== "newWorkspaceEntry" || does.folder) return;
    const place = workspaceFiles(does.at.workspace);
    showView(pieceFileView(place, path), pieceFileTitle(place, path));
  });

  /** A browser in the session host, opened in a view tab of its own (#2012); a refusal (Stop
   *  all, no host) is said on the window's trouble line. */
  const newBrowser = useCallback(() => {
    void commands.openBrowser(plane).then((opened) => {
      if (opened.status === "error") {
        refusedBy("browser", opened.error);
        return;
      }
      succeeded("browser");
      showView(browserView(opened.data), BROWSER_TITLE);
    });
  }, [plane, refusedBy, showView, succeeded]);

  /** A browser of the person's own, opened at `address` (#2176): it goes there once its view
   *  is live, as if they typed it, so a site new to the project asks there. */
  const newBrowserAt = useCallback(
    (address: string) => {
      void commands.openBrowser(plane).then((opened) => {
        if (opened.status === "error") {
          refusedBy("browser", opened.error);
          return;
        }
        succeeded("browser");
        goWhenShown(opened.data, address);
        showView(browserView(opened.data), BROWSER_TITLE);
      });
    },
    [plane, refusedBy, showView, succeeded],
  );

  /** The browsers of this project's chats (#2024): rows under their chats, live on their tabs. */
  const {
    browsers: chatBrowsers,
    kept: keptBrowsers,
    read: chatBrowsersRead,
  } = useChatBrowsers(plane);
  /** The person's own browsers a crash of the window left running, each back in a view tab of
   *  its own (#2025). */
  const showTakenBack = useCallback(
    (program: string) => showView(browserView(program), BROWSER_TITLE),
    [showView],
  );
  // Once the record's tabs are adopted, which lays the strip out afresh.
  useBrowsersTakenBack(plane, settled ? showTakenBack : undefined);

  // A browser view's Inspect opens its DevTools beside it (#2028).
  useEffect(
    () =>
      onInspect(plane, (program) => present((tabs, strip) => openInspector(tabs, program, strip))),
    [plane, present],
  );
  /** Their programs: a tab showing one closes only its view, since it ends with its chat, and
   *  one pinned past its chat ends only when the person stops it (#2022). */
  const chatPrograms = useMemo(
    () => programsOf(chatBrowsers, keptBrowsers),
    [chatBrowsers, keptBrowsers],
  );
  const chatProgramsNow = useRef(chatPrograms);
  useEffect(() => {
    chatProgramsNow.current = chatPrograms;
  }, [chatPrograms]);

  /** **A browser lives as long as a tab shows it** (#2012): when the last one closes, it ends.
   *  Not a chat's (#2024), which ends with its chat. */
  const browsersBefore = useRef(tabs);
  useEffect(() => {
    for (const program of browsersLetGo(browsersBefore.current, tabs, chatProgramsNow.current))
      void commands.closeBrowser(program);
    // Which browsers a view tab of its own shows, for the core to note: a crash of the window
    // brings those tabs back (#2025).
    const was = browsersShown(browsersBefore.current);
    const now = browsersShown(tabs);
    for (const program of now)
      if (!was.has(program)) void commands.browserViewed(plane, program, true);
    for (const program of was)
      if (!now.has(program)) void commands.browserViewed(plane, program, false);
    browsersBefore.current = tabs;
  }, [tabs, plane]);

  /** **A closed studio tab's history goes with it** (#3681), whichever way it was closed. */
  const studiosBefore = useRef(tabs);
  useEffect(() => {
    for (const gone of studioTabsGone(studiosBefore.current, tabs)) {
      forgetViewHistory(studioHistoryOf(plane, gone));
      forgetSkillEdit(studioHistoryOf(plane, gone));
    }
    // **And any view's unsaved work goes with its tab** (#3929), once it is asked about.
    discardGone(plane, studiosBefore.current, tabs);
    studiosBefore.current = tabs;
  }, [tabs, plane]);

  /**
   * Closes the tab showing `view` — a vault or a persona that was just deleted — **when that tab
   * shows nothing else.** A tab holding a chat beside it is left as it is: closing it would end
   * the chat, and a deletion is never a reason to end one.
   */
  const closeView = useCallback(
    (view: ViewRef) => {
      const found = findView(now.current, view);
      if (found === undefined || panesOf(now.current, found.tab).length > 0) return;
      change((tabs) => closeTab(tabs, found.tab, filedIn, isPinned));
    },
    [change, filedIn, isPinned],
  );

  /** Making and deleting personas, vaults and todos (SI-3): the verbs, the dialogs, the box. */
  const rereadPanels = useCallback(() => setRereadWorkspace((asked) => asked + 1), []);
  const edits = usePlaneEdits({
    plane,
    showView,
    closeView,
    reread: rereadPanels,
    reloadVaults,
    askUnsaved,
  });
  /** A plane's memories (SI-9b): the preview tab, edits, Delete and its Undo. */
  const memoryEdits = useMemoryEdits({
    plane,
    present,
    update: change,
    closeView,
    reread: rereadPanels,
  });

  /**
   * Settings at the Project level (SE-19; the Project settings page before it, charter-app#252),
   * opened when the window asks for it — the project's menu and the palette. Asked
   * through the window even from this project's own palette, so there is one way in: the
   * window brings the project forward and this opens its tab. `handled` keeps a rebuilt
   * `showView` — it changes with the focused workspace — from opening it a second time for
   * the same ask.
   */
  const handled = useRef(settingsAsked);
  useEffect(() => {
    if (settingsAsked === undefined || handled.current === settingsAsked) return;
    handled.current = settingsAsked;
    showView(settingsView("project"), SETTINGS_TAB_TITLE);
  }, [settingsAsked, showView]);

  /** The title bar's ✋ asked for this project's Asks (#1692, #5644): shown, and given the
   *  keyboard. A clicked notification asks it at a chat (`inboxGroup`), which is brought
   *  forward too, beside its ask's row (#6003, ADR 0121 decision 12). */
  const inboxHandled = useRef(inboxAsked);
  /** `goToAsking`, which is declared further down. */
  const goingToAsking = useRef<(session: number) => void>(undefined);
  /** The line a clicked notification opens in Updates, and the ask it came with (#6005). */
  const openedUpdate = useMemo(
    () =>
      inboxUpdate === undefined || inboxAsked === undefined
        ? undefined
        : { key: inboxUpdate, at: inboxAsked },
    [inboxUpdate, inboxAsked],
  );
  useEffect(() => {
    if (inboxAsked === undefined || inboxHandled.current === inboxAsked) return;
    inboxHandled.current = inboxAsked;
    // A notification about a task that ended opens its line in Updates, read in place.
    if (inboxUpdate !== undefined) {
      showSideView("updates");
      return;
    }
    if (inboxGroup !== undefined) goingToAsking.current?.(inboxGroup);
    showSideView("asks");
    // After the view has given its first stop the keyboard: the group's first ask takes it.
    // Through a scope that leaves its chat out, which widens for it (qw359).
    if (inboxGroup !== undefined) requestAnimationFrame(() => landOnGroupIn(plane, inboxGroup));
  }, [inboxAsked, inboxGroup, inboxUpdate, plane, showSideView]);
  /** Whether this project's Asks is open on screen, told to the core: no notification is sent
   *  about what the person is reading (#1694, I-7; #5644). */
  const asksShown = arrangement.some(
    (one) => !one.collapsed && viewOpenIn(one, presentViews) === "asks",
  );
  useInboxOpenTold(plane, inFront && asksShown);
  /** Whether this project's Notices is open on screen: where the alerts are listed (#5645). */
  const noticesShown = arrangement.some(
    (one) => !one.collapsed && viewOpenIn(one, presentViews) === "notices",
  );
  /** **Opening the Notices reads the alerts again** (#5702; the Inbox's before #5645): what a
   *  chat's last turn found in its folders is listed when the person comes to look, not at the
   *  next minute's read. Only as it opens: a project brought to the front with its Notices open
   *  is no opening, and a switch sends nothing more for it (`projectSwitch.cost.test.tsx`). */
  const rereadAlerts = alerts?.does.reread;
  const noticesWereShown = useRef(noticesShown);
  useEffect(() => {
    const opened = noticesShown && !noticesWereShown.current;
    noticesWereShown.current = noticesShown;
    if (opened && inFront) rereadAlerts?.();
  }, [noticesShown, inFront, rereadAlerts]);

  /** A file outside the project a pane's link opened (#6216), in a read-only tab of its own. */
  const outsideFile = useCallback(
    ({ session, path }: { session: number | null; path: string }) =>
      showView(outsideFileView(session, path), outsideFileTitle(path)),
    [showView],
  );
  useOutsideFileAsks(plane, outsideFile);

  /** A file ⌘P found here (FM-7), opened in its file tab the way the explorer opens one. */
  const fileHandled = useRef(fileAsked?.at);
  useEffect(() => {
    if (fileAsked === undefined || fileHandled.current === fileAsked.at) return;
    fileHandled.current = fileAsked.at;
    // A jump to a line (a search hit) opens the branch's file tab, which picks the file and
    // lands on the line (`fileJump.ts`); a file ⌘P found opens in a tab of its own.
    const { place, path } = fileAsked;
    // The plane root has no branch's file tab to land in: its file opens in a tab of its own.
    const jump = fileAsked.line !== undefined && !isPlaneRoot(place);
    showView(
      jump ? pieceFilesView(place) : pieceFileView(place, path),
      jump ? pieceFilesTitle(place) : pieceFileTitle(place, path),
    );
  }, [fileAsked, showView]);

  /** An artifact the palette found here (#5968), opened in its tab as its icon opens it. */
  const artifactHandled = useRef(artifactAsked?.at);
  useEffect(() => {
    if (artifactAsked === undefined || artifactHandled.current === artifactAsked.at) return;
    artifactHandled.current = artifactAsked.at;
    const { workspace, folder, name } = artifactAsked.found;
    showView(artifactView(workspace, folder), name);
  }, [artifactAsked, showView]);

  /** The Saving tab (charter-app#294), opened the same way and for the same reason. */
  const savingHandled = useRef(savingAsked);
  useEffect(() => {
    if (savingAsked === undefined || savingHandled.current === savingAsked) return;
    savingHandled.current = savingAsked;
    showView(SAVING_VIEW, SAVING_TITLE);
  }, [savingAsked, showView]);

  /** The Pool setup tab (spec #4469), opened from the palette for now. */
  const poolSetupHandled = useRef(poolSetupAsked);
  useEffect(() => {
    if (poolSetupAsked === undefined || poolSetupHandled.current === poolSetupAsked) return;
    poolSetupHandled.current = poolSetupAsked;
    showView(POOL_SETUP_VIEW, POOL_SETUP_TITLE);
  }, [poolSetupAsked, showView]);

  /** The Licences tab, asked from About (#1719) while this project is in front, as Saving is. */
  const licencesHandled = useRef(licencesAsked);
  useEffect(() => {
    if (licencesAsked === undefined || licencesHandled.current === licencesAsked) return;
    licencesHandled.current = licencesAsked;
    showView(LICENCES_VIEW, LICENCES_TITLE);
  }, [licencesAsked, showView]);

  /** The same tab, asked by a dialog of this project (`saving.askSavingTab`, #1296). */
  useEffect(() => {
    const asked = (event: Event) => {
      if ((event as CustomEvent<{ plane: string }>).detail.plane !== plane) return;
      showView(SAVING_VIEW, SAVING_TITLE);
    };
    window.addEventListener(OPEN_SAVING, asked);
    return () => window.removeEventListener(OPEN_SAVING, asked);
  }, [plane, showView]);

  /**
   * **A Fix's chat** (the operator's ruling P2, 2026-10-13; `saving.tellFixStarted`): the core
   * started it on what failed, and its tab goes on the strip it is filed under, in front, as a
   * curation chat's does. A second press for the same request starts nothing: the chat already
   * open for it comes forward.
   */
  useEffect(() => {
    const started = (event: Event) => {
      const { plane: of, chat } = (event as CustomEvent<FixStarted>).detail;
      if (of !== plane) return;
      if (chat.existing) {
        showChat(chat.session);
        return;
      }
      showPlaced(chat);
    };
    window.addEventListener(FIX_STARTED, started);
    return () => window.removeEventListener(FIX_STARTED, started);
  }, [change, plane, showChat, showPlaced]);

  /** Settings at a workspace's level (SE-20; charter-app#280), on that workspace's strip: what
   *  its menu's and the palette's Workspace settings… open. */
  const openWorkspaceSettings = useCallback(
    (workspace: string) =>
      showView(workspaceSettingsView(workspace), workspaceSettingsTitle(workspace), workspace),
    [showView],
  );

  /**
   * **Follows a link into Settings** (SE-22, `settings/links.ts`): the group is shown at its
   * place, and the Settings tab for its level and target is opened — or brought forward, when
   * it is open. A link this window cannot follow opens nothing.
   */
  const openSettingsAt = useCallback(
    (link: SettingsLink) => {
      const to = landing(link, plane);
      if (to === undefined) return;
      linkToGroup(to.place, link.group, link.setting);
      showView(to.view, to.title, to.workspace);
    },
    [plane, showView],
  );

  /** A link a view of this project asked for — the Saving view's Notice (NO-7) — opened the
   *  same way. */
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<SettingsLinkAsk>).detail;
      if (wanted.plane === plane) openSettingsAt(wanted.link);
    };
    window.addEventListener(SETTINGS_LINK, asked);
    return () => window.removeEventListener(SETTINGS_LINK, asked);
  }, [plane, openSettingsAt]);

  /** A link the window followed, opened the same way and for the same reason (SE-22). */
  const settingsLinkHandled = useRef(settingsLinkAsked?.at);
  useEffect(() => {
    if (settingsLinkAsked === undefined || settingsLinkHandled.current === settingsLinkAsked.at)
      return;
    settingsLinkHandled.current = settingsLinkAsked.at;
    openSettingsAt(settingsLinkAsked.link);
  }, [settingsLinkAsked, openSettingsAt]);

  /** **Where a task's question is drawn** (#4565): a task's top is the chat it returns to,
   *  the nearest above it with a pane (`returnsTo`); Open task is the person's own press. */
  const quizPlaces = useMemo(
    () => ({
      topOf: (task: number) => returnsTo(tabs, task, askedBy)?.own,
      openTask: (task: number) => showChat(task),
      openSettings: () => openSettingsAt({ group: "you.chats", setting: "you.chats.answer" }),
      // A question's row's Answer (#5215): Asks, at the task's question.
      answerInInbox: (task: number) => {
        showSideView("asks");
        requestAnimationFrame(() => landOnGroupIn(plane, task, "task-question"));
      },
    }),
    [tabs, askedBy, showChat, openSettingsAt, showSideView, plane],
  );

  /**
   * The Settings tab, opened the same way and for the same reason (SE-16), **at the focused
   * level** (SE-23, V89g): the focused workspace's, else this project's. The app menu's
   * Settings… (`⌘,`, `Ctrl+,` off a Mac) and the palette's Settings… both land here; You is the
   * window's answer when no project is open (`App.tsx`), and one press of the level switcher
   * from here. The plane root is not a workspace, so it is the project's level.
   */
  const settingsTabHandled = useRef(settingsTabAsked);
  useEffect(() => {
    if (settingsTabAsked === undefined || settingsTabHandled.current === settingsTabAsked) return;
    settingsTabHandled.current = settingsTabAsked;
    const workspace = focused === OUTSIDE ? undefined : focused;
    const open =
      workspace === undefined
        ? () => showView(settingsView("project"), SETTINGS_TAB_TITLE)
        : () => openWorkspaceSettings(workspace);
    open();
  }, [focused, openWorkspaceSettings, settingsTabAsked, showView]);

  /** Settings at the You level (SE-23's Your settings…), opened the same way. */
  const yourSettingsHandled = useRef(yourSettingsAsked);
  useEffect(() => {
    if (yourSettingsAsked === undefined || yourSettingsHandled.current === yourSettingsAsked)
      return;
    yourSettingsHandled.current = yourSettingsAsked;
    showView(settingsView("you"), SETTINGS_TAB_TITLE);
  }, [yourSettingsAsked, showView]);

  /**
   * Focuses a workspace: the strip below it shows that workspace's chats, and one of them
   * comes to the front — the one that was in front there last, or its first.
   *
   * **A workspace with no chats puts nothing in front.** Leaving another workspace's chat on
   * screen under this workspace's empty strip would be the app showing a chat the strip says
   * is not there. Nothing is ended and nothing is torn down: every chat in every workspace
   * keeps running, exactly as a project behind another one does (#125).
   */
  const focusWorkspace = useCallback(
    (workspace: string) => {
      setPicked(workspace);
      change((tabs) =>
        showWorkspace(tabs, workspace, filedIn, lastFront.current[workspace], isPinned),
      );
      // Reported to the extensions that hear it (charter-app#343). Not awaited: the core tells
      // them on a thread of its own, and the strip has already moved.
      void commands.workspaceFocused(plane, workspace).catch(() => undefined);
    },
    [change, filedIn, isPinned, plane],
  );

  /**
   * The first chat in a repository just opened into this project (FR-4, #603): its workspace
   * is focused and the picker is asked for, to start in the repository's clone. Only once the
   * plane has been read and lists that workspace, since focusing a workspace the strip has not
   * heard of would focus nothing. The picker still asks which harness (ADR 0022).
   */
  const firstChatHandled = useRef<number | undefined>(undefined);
  useEffect(() => {
    if (firstChatAsked === undefined || firstChatHandled.current === firstChatAsked.at) return;
    if (!sidebar?.workspaces.some((ws) => ws.name === firstChatAsked.workspace)) return;
    firstChatHandled.current = firstChatAsked.at;
    const { workspace, cwd, harness, noneInstalled, instructions } = firstChatAsked;
    const where: Where = { tab: true, in: cwd };
    // **The repo's agent instructions, offered beside the chat and not in front of it**
    // (FR-18a): a tab that asks nothing until the operator goes to it, so W10's budget —
    // the forge question when the repo's remote does not say, the trust question and, only
    // when there is a choice, the picker (`interruptBudget.ts`) — is not spent on it.
    // Nothing is written until its own press.
    // **The first task, offered the same way** (FR-28): a tab beside the first chat, so a
    // partner session has the script one press away and nobody is asked anything for it.
    const offerInstructions = () => {
      if (instructions > 0)
        change((tabs) =>
          offerView(
            tabs,
            repoInstructionsView(workspace),
            repoInstructionsTitle(workspace),
            workspace,
          ),
        );
    };
    // The first task (FR-28) is offered only beside a chat that can start: with no harness on
    // the machine (FR-29) its runs could not, so the setup tab offers the instructions alone.
    const offerBeside = () => {
      offerInstructions();
      if (cwd !== "")
        change((tabs) => offerView(tabs, firstTaskView(cwd), firstTaskTitle(workspace), workspace));
    };
    // `ask`'s steps, written out so the state is set in the command's callback: the
    // `react-hooks/set-state-in-effect` rule reads a call in an effect body as synchronous
    // however far the setState is from it (`Extensions.tsx` says the same).
    void commands
      .startOptions(plane)
      .then(async (options) => {
        if (options.status === "error") {
          refusedBy("start-options", options.error);
          return;
        }
        succeeded("start-options");
        focusWorkspace(workspace);
        // **No harness on this machine** (FR-29): the picker would list harnesses none of
        // which can start, so the first chat is the setup tab — each one's official installer,
        // run in a shell tab on a press — in front, and the picker comes from its Start a chat.
        if (noneInstalled) {
          change((tabs) =>
            openView(tabs, harnessSetupView(cwd), harnessSetupTitle(workspace), workspace),
          );
          offerInstructions();
          return;
        }
        // **Nothing to pick, so nothing is asked** (FR-4, W10's interrupt budget): one
        // harness signed in on this machine, and a profile of it that needs no approval. The
        // picker still comes up whenever there is a choice or a command to approve (ADR 0022).
        const ofIt = options.data.profiles.filter((row) => row.kind === harness);
        const only = ofIt.find((row) => row.is_default) ?? ofIt[0];
        if (harness !== null && only !== undefined && only.approval === null) {
          const persona =
            options.data.persona !== null && options.data.personas.includes(options.data.persona)
              ? options.data.persona
              : null;
          const started = await startOn(
            where,
            only.kind,
            only.name,
            persona,
            false,
            null,
            // Nothing was asked, so the default stands: a first chat in a repo starts on a
            // branch of its own like any other (GL-1).
            true,
            workspace,
          );
          if (started === undefined) {
            offerBeside();
            return;
          }
          // Over a pool's limit, the picker that comes up offers to raise it (spec #4275).
          const { words: refused, limit: offer } = started;
          setPickerTrouble(refused);
          setPickerLimit(offer === null ? undefined : { offer, refused, profile: only.name });
          // A pool that could not be reached is offered its three answers there (#4536).
          const gone = started.pool_unavailable ?? undefined;
          setPickerPoolGone(gone === undefined ? undefined : { pool: gone, refused });
        } else setPickerTrouble(undefined);
        offerBeside();
        setPicking({ options: options.data, where, runsOn: await readRunsOn(workspace) });
      })
      .catch((err: unknown) => refusedBy("start-options", String(err)));
  }, [
    change,
    firstChatAsked,
    focusWorkspace,
    plane,
    readRunsOn,
    refusedBy,
    sidebar,
    startOn,
    succeeded,
  ]);

  /** What each chat the project lists is called (`shownName`, #1484), by its number: the one
   *  rule, asked once per chat, for every surface that names a chat by number. */
  const chatNames = useMemo(
    () =>
      new Map(
        [...(sidebar?.workspaces.flatMap((ws) => ws.chats) ?? []), ...(sidebar?.unfiled ?? [])].map(
          (chat) => [chat.session, shownName(tabs, chat)],
        ),
      ),
    [sidebar, tabs],
  );
  /**
   * **The studio's chats still open** (#5185), lent to the studio and the chats' panes: each with
   * its name here, Open chat, the way to its skill's page and to its draft, and its Stop, which
   * asks first as the chat's own does (#5248).
   */
  const studioChats = useMemo<StudioChatsLent>(
    () => ({
      chats: Object.entries(studioLinks).flatMap(([at, link]) => {
        const session = Number(at);
        const listed = chatNames.get(session);
        const held = tabHolding(tabs, session);
        const name = listed ?? (held === undefined ? undefined : tabs.byId[held].name);
        return name === undefined ? [] : [{ session, name, link }];
      }),
      show: (session) => showChat(session),
      openSkill: (link) => {
        const place = studioPlaceOf(link);
        showView(
          place.page === "drafts"
            ? skillStudioAt(scopeOfLevel(link.level), "drafts")
            : skillStudioAt(scopeOfLevel(link.level), "library", {
                skill: skillRowNamed(place.skill),
                tab: place.tab,
              }),
          SKILL_STUDIO_TITLE,
        );
      },
      openDraft: (link, id) =>
        showView(
          skillStudioAt(scopeOfLevel(link.level), "drafts", { item: id }),
          SKILL_STUDIO_TITLE,
        ),
      stop: (session) => askToStop(session, false),
    }),
    [askToStop, chatNames, showChat, showView, studioLinks, tabs],
  );
  /** {@link nameOf} as it stands, for what is decided in a handler and not while drawing. */
  const nameOfNow = useRef<(session: number) => string>((session) => String(session));
  /** What a chat the project lists is called now, or nothing for one it does not list. */
  const nameOfListed = useCallback((session: number) => chatNames.get(session), [chatNames]);
  /**
   * What a chat is called here: as its row names it (`shownName`). **A chat the project does not
   * list yet** (#5308) is named as `shownName` will name it once its row arrives: the name of the
   * tab it is the own chat of; else, beside another chat, never that tab's name but what its row
   * will say, from what the core said of it as it came back or what this window started it as;
   * else its own name as its pane holds it; or its session number.
   */
  const nameOf = useCallback(
    (session: number) => {
      const listed = chatNames.get(session);
      if (listed !== undefined) return listed;
      const own = ownTabOf(tabs, session);
      if (own !== undefined) return tabs.byId[own].name;
      const back = reopened.find((chat) => chat.session === session);
      if (back !== undefined) return untabbedName(back);
      const started = startedAs.get(session);
      if (started !== undefined) return pressedName(started);
      // Any other: its own name, as its pane holds it, never the tab's.
      return paneChatName(tabs, session) ?? String(session);
    },
    [chatNames, reopened, startedAs, tabs],
  );

  useEffect(() => {
    nameOfNow.current = nameOf;
  }, [nameOf]);

  /** What a pane drop is judged against (#4763): where chats are filed, and what each is called. */
  const dropContext = useMemo<DropContext>(
    () => ({
      filedIn,
      pinned: isPinned,
      name: (content) =>
        content.kind === "session"
          ? nameOf(content.session)
          : content.kind === "view"
            ? (tabs.byId[findView(tabs, content.view)?.tab ?? -1]?.name ?? "this view")
            : "the empty pane",
      // A chat with no row yet has no name to give a tab: what its pane holds is no name to show.
      tabName: ({ session, chat }) => {
        if (!listedNow.current.has(session)) return undefined;
        const { who, label } = pressedOf(session);
        const defaultName = who ? `${who} ${chat}` : chat;
        return { name: label ?? defaultName, defaultName };
      },
    }),
    [filedIn, isPinned, nameOf, pressedOf, tabs],
  );
  useEffect(() => {
    firstPaneNameNow.current = (content, before) => firstPaneNameOf(content, before, dropContext);
  }, [dropContext]);
  /**
   * A tab a drag brings forward, drawn and not told (#5294), until {@link holdOver}; and the one
   * that was in front, drawn again as it is put back. Not a `change`: no pane is noted as used
   * (`noteUsed`), so a cancel leaves the tabs exactly as they were.
   */
  const holdTab = useCallback((id: number) => {
    heldForward.current = true;
    setHoldingForward(true);
    now.current = selectTab(now.current, id);
    setTabs(now.current);
  }, []);
  /** The drag is over: what the window draws now is told, if it differs from what was. */
  const holdOver = useCallback(() => {
    heldForward.current = false;
    setHoldingForward(false);
    sayLayout.current(now.current);
  }, []);
  const ownNameOf = useCallback(
    (session: number) =>
      [...(sidebar?.workspaces.flatMap((ws) => ws.chats) ?? []), ...(sidebar?.unfiled ?? [])].find(
        (chat) => chat.session === session,
      )?.name ?? String(session),
    [sidebar],
  );
  /** **Dragging panes: split, swap and move** (#4763), its Undo line and what it draws. */
  const paneDrag = usePaneDrag({
    tabs,
    change,
    ctx: dropContext,
    onHoldTab: holdTab,
    onHoldOver: holdOver,
    chatNameOf: ownNameOf,
    active: inFront,
  });
  /** The window's one Undo (#4763, #4765): the palette's row runs the toast's and ⌘Z's. */
  const undoLayoutIn = paneDrag.undo;
  /** A Move… row's pick-up (#4764): the keyboard goes back, on Esc, to what had it. */
  const { pickUp } = paneDrag.placing;
  const moveFrom = useCallback(
    (from: MovedFrom) =>
      pickUp(from.kind === "chat" ? { ...from, chat: "" } : from, document.activeElement),
    [pickUp],
  );

  /**
   * **Open beside** (#4769): `view` in a split beside pane `at` (the view's own pane for a
   * button on a view, the note's pane for an offer), or beside the focused pane where none is
   * named (a panel row, the palette, the Notices centre). A view already open is brought
   * forward, never opened twice. A split that would leave a side under the minimum is refused
   * before anything moves, said with Open in a tab (`viewBeside.ts`). The split is a layout
   * change, so its Undo line follows (#4763), and its offer goes.
   */
  const showViewBeside = useCallback(
    (view: ViewRef, title: string, at?: number) => {
      const tabs = now.current;
      const front = tabs.inFront === undefined ? undefined : tabs.byId[tabs.inFront];
      const pane = at ?? front?.focused;
      if (findView(tabs, view) !== undefined || pane === undefined) {
        showView(view, title);
        return;
      }
      const beside = viewBesideOf(paneSize(at));
      if (!beside.ok) {
        setReport({
          from: "view.beside",
          refused: true,
          words: besideRefusedSaid(beside),
          ways: [{ label: "Open in a tab", onPress: () => showView(view, title) }],
        });
        return;
      }
      const there = tabs.order
        .flatMap((id) => contentsOf(tabs, id))
        .find((one) => one.pane === pane)?.content;
      const name = there === undefined ? "this pane" : dropContext.name(there);
      present((was, strip) =>
        recorded(
          was,
          openViewBesidePane(was, pane, view, title, strip, beside.direction),
          `Opened ${title} beside ${name}.`,
        ),
      );
      viewOpened(plane, view);
    },
    [dropContext, plane, present, showView],
  );

  const viewsNow = useRef(views);
  useEffect(() => {
    viewsNow.current = views;
    offersInForce(plane, views);
  }, [plane, views]);
  // **Offers heard** (#4769): what the extensions that heard an event in this project offered,
  // each judged against what is in force here, what the person stopped and what is open. A
  // removed extension's offers are withdrawn as what is in force changes.
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ExtensionHeard>("extension-heard", (event) => {
          if (gone || event.payload.plane !== plane) return;
          heardOffers(
            plane,
            event.payload.offers.map((offer) => ({
              extension: offer.extension,
              view: offer.view,
              key: offer.key,
              ...(offer.chat === null ? {} : { chat: offer.chat }),
            })),
            {
              declared: viewsNow.current,
              open: (view) => findView(now.current, view) !== undefined,
            },
          );
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);
  const offerOpening = useMemo<OfferOpening>(
    () => ({
      plane,
      declared: views,
      onOpen: (view, name, how, pane) =>
        how === "beside" ? showViewBeside(view, name, pane) : showView(view, name),
    }),
    [plane, showView, showViewBeside, views],
  );

  /**
   * **What a tab's panes are called on the strip** (#4762): a chat by its own name, never the
   * name of the tab whose pane it is in, which is the first pane's; a chat with a tab of its
   * own by that tab's name, which a rename sets. A view is called by its own title (#5577), the
   * one it was opened with (a file's name, a settings page's), which its pane keeps
   * (`tabFace.faceOf`); one opened with none is called what its tab is.
   */
  const naming = useMemo<Naming>(() => {
    const known = new Map(
      [
        ...reopened,
        ...(sidebar?.workspaces.flatMap((ws) => ws.chats) ?? []),
        ...(sidebar?.unfiled ?? []),
      ].map((chat) => [chat.session, chat]),
    );
    return {
      chat: (session) => {
        const own = ownTabOf(tabs, session);
        if (own !== undefined) return tabs.byId[own].name;
        const chat = known.get(session);
        return chat === undefined ? nameOf(session) : untabbedName(chat);
      },
      // A view's own title is its pane's (`tabFace`, #5577): nothing to say over it here.
      view: () => undefined,
    };
  }, [nameOf, reopened, sidebar, tabs]);

  /** Each chat's browsers as they are drawn (#2024), by the name the chat has here. */
  const chatBrowsersShown = useMemo(
    () =>
      new Map(
        [...chatBrowsers].map(([session, listed]) => [
          session,
          listed.map((browser): BrowserShown => ({ browser, chat: nameOf(session) })),
        ]),
      ),
    [chatBrowsers, nameOf],
  );
  /** The pinned browsers whose chat closed (#2022), by what their chat was called. */
  const keptShown = useMemo(
    () => keptBrowsers.map((browser): BrowserShown => ({ browser, chat: browser.was ?? "" })),
    [keptBrowsers],
  );
  /** Every chat's browser as the catalogue offers rows for it (#2022). */
  const browsersOffered = useMemo(
    (): BrowserOffered[] => [
      ...[...chatBrowsersShown.values()].flatMap((list) =>
        list
          .filter(({ browser }) => !browser.unended)
          .map(({ browser, chat }) => ({
            program: browser.program,
            name: browser.name,
            session: browser.session,
            chat,
            // What its chat's browser timeline is kept by (#2017).
            chatId: reopened.find((open) => open.session === browser.session)?.id ?? null,
            pinned: browser.pinned,
            kept: false,
            login: browser.login,
          })),
      ),
      ...keptShown.map(({ browser, chat }) => ({
        program: browser.program,
        name: browser.name,
        session: browser.session,
        chat,
        chatId: null,
        pinned: true,
        kept: true,
        login: browser.login,
      })),
    ],
    [chatBrowsersShown, keptShown, reopened],
  );
  /** A chat's browser's view, in a tab of its own on its chat's strip (#2024). */
  const openChatBrowser = useCallback(
    ({ browser, chat }: BrowserShown) =>
      showView(
        browserView(browser.program),
        chatBrowserTitle(chat, browser.name),
        filedIn(browser.session),
      ),
    [showView, filedIn],
  );
  /**
   * **This project's browsers, as the Browsers grid draws them** (#2027): its chats' (one not
   * ended yet is no chat's, as on the tab), the pinned ones whose chat closed, and the person's
   * own that its tabs show, each filed where its chat or its tab is. The grid of any project
   * this window holds draws them; a press on one brings this project forward and opens its view
   * here.
   */
  const gridTiles = useMemo((): GridTile[] => {
    const ofChats: GridTile[] = [...chatBrowsersShown.values(), keptShown]
      .flat()
      .filter(({ browser }) => !browser.unended)
      .map(({ browser, chat }) => ({
        plane,
        program: browser.program,
        own: false,
        name: browser.name,
        // A pinned one whose chat's name was not kept is named by its own name alone.
        chat: chat === "" ? null : chat,
        workspace: filedIn(browser.session),
        driver: browser.driver,
        pool: browser.pool,
      }));
    const theirs = new Set(ofChats.map((one) => one.program));
    const own = new Map<string, GridTile>();
    for (const id of tabs.order)
      for (const { content } of contentsOf(tabs, id))
        if (
          content.kind === "view" &&
          isBrowser(content.view) &&
          !theirs.has(content.view.key) &&
          !chatPrograms.has(content.view.key) &&
          !own.has(content.view.key)
        )
          own.set(content.view.key, {
            plane,
            program: content.view.key,
            own: true,
            name: null,
            chat: null,
            workspace: content.workspace,
            driver: null,
            pool: null,
          });
    return [...ofChats, ...own.values()];
  }, [chatBrowsersShown, keptShown, tabs, chatPrograms, filedIn, plane]);
  const openGridTile = useCallback(
    (tile: GridTile) => {
      windowDoes.selectProject(plane);
      showView(browserView(tile.program), tileTitle(tile), tile.workspace);
    },
    [windowDoes, plane, showView],
  );
  useEffect(() => {
    tellProjectBrowsers({
      plane,
      project: projectName(plane),
      // **None until its tabs are back** (`settled`): putting them back lays the strip out
      // afresh, which would lose a tab a press on a tile opened before.
      tiles: settled ? gridTiles : [],
      open: openGridTile,
    });
  }, [plane, gridTiles, openGridTile, settled]);
  useEffect(() => () => forgetProjectsBrowsers(plane), [plane]);
  /** **A chat's new browser opens beside it**, where the person's setting says so (#2024): only
   *  one that started after the window first read them, so a reload opens nothing. */
  const { beside: browsersBeside } = useChatsListPrefs();
  const browsersKnown = useRef<Set<string> | undefined>(undefined);
  useEffect(() => {
    if (!chatBrowsersRead) return;
    const known = browsersKnown.current;
    browsersKnown.current = programsOf(chatBrowsers);
    if (known === undefined || !browsersBeside) return;
    for (const [session, listed] of chatBrowsers)
      for (const { program, unended } of listed)
        if (!unended && !known.has(program))
          change((was) => openBrowserBeside(was, session, program, filedIn(session)));
  }, [chatBrowsers, chatBrowsersRead, browsersBeside, change, filedIn]);

  const frontTab = tabs.inFront === undefined ? undefined : tabs.byId[tabs.inFront];
  /** **The room the front tab's panes have** (#4766), measured: a split that cannot give each
   *  side its minimum folds a side to a rail, and a split row greys out where a half would be
   *  under it. Nothing is measured before the window is laid out, and then nothing folds. */
  const [measurePanes, paneRoom] = usePaneRoom();
  const panesAt = useCallback(
    (element: HTMLDivElement | null) => {
      measurePanes(element);
    },
    [measurePanes],
  );
  const focusedRoom = useMemo(
    () =>
      frontTab === undefined || paneRoom === undefined
        ? undefined
        : drawnOf(
            zoomedLayout(tabs, frontTab.id, layoutShown(tabs, frontTab.id) ?? frontTab.layout),
            frontTab.focused,
            paneRoom,
          ).sizes[frontTab.focused],
    [tabs, frontTab, paneRoom],
  );
  /** What a folded pane's rail shows (#4766): a chat by its row's name and its persona and
   *  state icon, as its tab wears them; a view and an empty pane as the panes name them. */
  const paneFace = useCallback(
    (content: Content): PaneFace | undefined =>
      content.kind !== "session"
        ? undefined
        : {
            name: naming.chat(content.session),
            mark: (
              <ChatPersonaState
                persona={personaOf(content.session)}
                session={content.session}
                shell={shells.has(content.session)}
                harness={factsOf(content.session)?.harness ?? null}
                report={factsOf(content.session)?.report ?? null}
                outcome={factsOf(content.session)?.outcome ?? null}
              />
            ),
            // Its mark wears the chat's state, ✋ among them: the rail draws no second (#5447).
            stated: true,
          },
    [naming, personaOf, shells, factsOf],
  );
  /**
   * **Every workspace and the plane root always keep one tab** (#4758; spec story 9): the strip
   * the window draws, once the record is heard, gets a tab with one empty pane where it has
   * none, so `+`, New chat and a sidebar press always have a pane to act on. That one empty pane
   * is the window's only empty state.
   */
  useEffect(() => {
    if (!viewsHeard || (sidebar === undefined && !sidebarUnread) || tabs.inFront !== undefined)
      return;
    const strip = focused ?? OUTSIDE;
    if (tabsIn(tabs, strip, filedIn).length > 0) return;
    change((was) => openEmptyTab(was, strip));
  }, [change, filedIn, focused, sidebar, sidebarUnread, tabs, viewsHeard]);
  // The session the next worktree question is about: the chat in the pane that has the
  // keyboard, which is the one "this chat's worktree" means.
  // The chat SHOWN there (#1486): while the tab shows a task, the task is what is typed into.
  // And nothing at all while that pane shows a task it cannot draw: there is no terminal there.
  const frontSession = frontShown.some((one) => one.pane === frontTab?.focused && !one.live)
    ? undefined
    : focusedChat(tabs);
  /** **The chat a skill's page uses a skill in** (#3682): the chat last in front, kept while the
   *  studio is looked at beside or instead of it, until its tab closes. Never a shell's. */
  const [lastChat, setLastChat] = useState<number>();
  const chatNow =
    frontSession !== undefined && !shells.has(frontSession) ? frontSession : undefined;
  // Kept as it is drawn (React's state adjusted while rendering, not in an effect).
  if (chatNow !== undefined && chatNow !== lastChat) setLastChat(chatNow);
  const chatKept =
    lastChat !== undefined && tabHolding(tabs, lastChat) !== undefined ? lastChat : undefined;
  const chatToUse = useMemo<ChatToUse | undefined>(
    () => (chatKept === undefined ? undefined : { session: chatKept, name: nameOf(chatKept) }),
    [chatKept, nameOf],
  );
  /** Escape in the Inbox (#1692, I-11): the keyboard goes back to the chat in front. */
  const leaveInbox = useCallback(() => {
    if (frontSession !== undefined) giveKeyboardTo(plane, frontSession);
    // No chat in front to go back to: the keyboard leaves the list all the same.
    else if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  }, [frontSession, plane]);
  /** Whether chat `session` itself waits on the person's reply (#1700): such a chat gets the
   *  Inbox's reply box beside any reason it is in the queue for. */
  const chatAsked = useCallback(
    (session: number) => chats.store.statesFor(chats.plane).bySession[session] === "waiting",
    [chats],
  );
  /**
   * **What the app found each chat needs the person for** (#1448): its needs-you item's
   * sentences, without the tasks of its that came to nothing. The core lists what it found
   * before the failures (`hooks::seen_by`), so those are the ones at the front. Each is an
   * update in the Inbox (#1694), as a refused commit is.
   */
  const foundFor = useMemo(
    () =>
      Object.fromEntries(
        Object.entries(needs).map(([session, said]) => [
          session,
          said.slice(0, Math.max(0, said.length - (failedTasks[Number(session)] ?? []).length)),
        ]),
      ) as Readonly<Record<number, readonly string[]>>,
    [failedTasks, needs],
  );
  /** Why a queued chat waits, where it is not that it asked (#1448): what reported back to it
   *  or was stopped below it, said in place of a reply box (#1692). A report with nowhere to go
   *  and a refused commit are updates of their own (#1694), as a failed task is (#1693). */
  const inboxWhy = useCallback(
    (session: number) => backSaid(reports[session] ?? [], stoppedBelow[session] ?? []),
    [reports, stoppedBelow],
  );
  // Where each chat is working. The sidebar's chats carry it, and so does the record the
  // core put back at this launch; a chat the operator just opened is in the first. Spelled as
  // one line per chat first, so a re-read of the sidebar that moved no chat makes nothing new:
  // not the map, not `worktreeOf`, not the catalogue that reads it.
  const chatCwds = useMemo(() => {
    const seen = new Set<number>();
    const lines: string[] = [];
    for (const chat of [
      ...(sidebar?.workspaces.flatMap((ws) => ws.chats) ?? []),
      ...(sidebar?.unfiled ?? []),
      ...reopened,
    ])
      if (chat.cwd !== null && !seen.has(chat.session)) {
        seen.add(chat.session);
        lines.push(`${chat.session}\t${chat.cwd}`);
      }
    return lines.sort().join("\n");
  }, [sidebar, reopened]);
  const cwdOf = useMemo(
    () =>
      new Map(
        chatCwds === ""
          ? []
          : chatCwds.split("\n").map((line) => {
              const tab = line.indexOf("\t");
              return [Number(line.slice(0, tab)), line.slice(tab + 1)] as const;
            }),
      ),
    [chatCwds],
  );
  /** The directories to ask about, one line each in a fixed order: the effect's key. */
  const cwdsAsked = useMemo(() => [...new Set(cwdOf.values())].sort().join("\n"), [cwdOf]);
  /** The directories already asked about, and the plane and `relocate` they were asked at:
   *  only a new directory is asked about, until a removal says every answer may have moved.
   *  The effect below is all that reads or writes it. */
  const locatedAt = useRef({ plane, relocate, asked: new Set<string>() });

  // Which piece each chat sits in (#4851). `worktree_of_chat` is path arithmetic plus one git
  // listing for a directory in a piece, asked once per directory — never per keystroke, and
  // never for a palette that is not open — and again for every one after a removal.
  //
  // The plane travels with the directory (charter-app#127): the answer is about a piece of
  // THIS project, and the core used to derive the plane by walking up from the cwd alone.
  useEffect(() => {
    if (cwdsAsked === "") return;
    const was = locatedAt.current;
    if (was.plane !== plane || was.relocate !== relocate)
      locatedAt.current = { plane, relocate, asked: new Set() };
    const { asked } = locatedAt.current;
    const cwds = cwdsAsked.split("\n").filter((cwd) => !asked.has(cwd));
    for (const cwd of cwds) asked.add(cwd);
    // **An answer lands even after the directories changed**: it is keyed by the directory it
    // is about, so it is still true of that directory. Only one about another plane is dropped.
    for (const cwd of cwds)
      void commands
        .worktreeOfChat(plane, cwd)
        .then((answer) => (answer.status === "ok" ? (answer.data ?? undefined) : undefined))
        // A window that cannot ask simply offers no Branch rows for that chat.
        .catch(() => undefined)
        .then((piece) => {
          if (locatedAt.current.plane === plane) setLocated((was) => new Map(was).set(cwd, piece));
        });
  }, [cwdsAsked, plane, relocate]);

  // Only an answer about the directory a chat is in now. Nothing is cleared when a chat moves —
  // clearing state from inside an effect is a render the window does not need, and a stale
  // answer is simply not this chat's.
  const worktreeOf = useCallback(
    (session: number) => {
      const cwd = cwdOf.get(session);
      return cwd === undefined ? undefined : located.get(cwd);
    },
    [cwdOf, located],
  );

  /**
   * Removes the piece THE ROW NAMED. The core's refusal travels back whole.
   *
   * **It reads nothing off the window to work out which worktree was meant**
   * (charter-app#174). It used to take only `force` and act on whatever the chat in front was
   * working in, which is exactly why the explorer's rows had nothing to offer: a piece nobody
   * is running in is not in front of anything. The piece arrives on the row, so the front
   * chat's row and an explorer row are the same code with a different `cut` in them.
   */
  const removeWorktree = useCallback(
    async (cut: Cut, force: boolean): Promise<Ran> => {
      const answer = await commands
        .worktreeRemove(plane, cut.workspace, cut.repo, cut.piece, force)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      // Verbatim. The sentence names the repair, and an operator shown a reworded version of
      // it can neither follow that repair nor search for it.
      if (answer.status === "error") return { ok: false, refused: answer.error };
      // The core is asked again rather than the window assuming what it now says — both about
      // where every chat is working, and about the workspace, because the explorer and
      // the bottom bar are still drawing the row that has just gone.
      setRelocate((asked) => asked + 1);
      setRereadWorkspace((asked) => asked + 1);
      // The branch is not named any more: a row about a piece nothing is running in carries
      // no branch, and "its branch stays" is true of every worktree charter cuts.
      return { ok: true, said: `The folder of ${cut.piece} is gone. Its branch stays.` };
    },
    [plane],
  );

  /** Asks for a new branch in one of the focused workspace's repos. It cuts nothing: the
   *  dialog is what asks, and `worktree_add` is what cuts (GL-1). */
  const newBranch = useCallback(
    (repo: string) => {
      if (ofWorkspace === undefined) return;
      setBranchTrouble(undefined);
      setBranching({ workspace: ofWorkspace, repo });
    },
    [ofWorkspace],
  );

  /**
   * Cuts the branch the dialog named — `null` for charter's own `chat-<n>` — and makes it
   * where the next chat starts, the way picking its row would. A refusal stays in the dialog,
   * verbatim; what the cut found to say (a dirty clone, a layer that did not land) is reported.
   */
  const cutBranch = useCallback(
    async (branch: string | null) => {
      if (branching === undefined) return;
      const { workspace, repo } = branching;
      setBusyBranching(true);
      const answer = await commands
        .worktreeAdd(plane, workspace, repo, branch)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      setBusyBranching(false);
      if (answer.status === "error") {
        setBranchTrouble(answer.error);
        return;
      }
      setBranching(undefined);
      setBranchTrouble(undefined);
      setPickedSpot({
        workspace,
        spot: { repo, piece: answer.data.piece, path: answer.data.path },
      });
      setRereadWorkspace((asked) => asked + 1);
      setReport({
        from: "worktree.add",
        refused: false,
        words: [
          `New branch ${answer.data.branch} in ${repo}. New chats start on it.`,
          ...answer.data.warnings,
        ].join(" "),
      });
    },
    [branching, plane],
  );

  /** Asks for a new workspace. It makes nothing: the dialog is what asks, and
   *  `workspace_create` is what makes one. */
  const createWorkspace = useCallback(() => {
    setWorkspaceTrouble(undefined);
    setMakingWorkspace(true);
  }, []);

  /** Pins or unpins one workspace. It goes in the machine store, so what the store now says
   *  is asked again rather than assumed — `pinning` is what asks. */
  const pinWorkspace = useCallback(
    async (workspace: string, pinned: boolean): Promise<Ran> => {
      const said = await commands
        .pinWorkspace(plane, workspace, pinned)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      // A pin refused, or let go, is no longer one this window holds for the store (#1926).
      if (said.status === "error" || !pinned)
        setMadePins((was) =>
          was.includes(workspace) ? was.filter((one) => one !== workspace) : was,
        );
      if (said.status === "error") return { ok: false, refused: said.error };
      setPinning((asked) => asked + 1);
      // Settings › You › This machine lists the pins, and may be on screen (#1240).
      machineChanged();
      return { ok: true };
    },
    [plane],
  );

  /**
   * **Puts the project's pins in `order`** (#3128): what a chip's drag and its Move left and right
   * write, through the machine store's `arrange_workspace_pins`, which moves only the pins it
   * names, so a dormant pin keeps its place. Drawn in that order at once, and the store asked
   * again once it is written; a refusal is said, and the chips go back to the store's order.
   */
  const arrangePins = useCallback(
    async (order: readonly string[]): Promise<Ran> => {
      setArranged(order);
      arranging.current.writing += 1;
      const said = await commands
        .arrangeWorkspacePins(plane, [...order])
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      arranging.current.writing -= 1;
      arranging.current.written += 1;
      if (said.status === "error") {
        if (arranging.current.writing === 0) setArranged(undefined);
        setReport({ from: "workspace.pin", refused: true, words: said.error });
        return { ok: false, refused: said.error };
      }
      setPinning((asked) => asked + 1);
      // Settings › You › This machine lists the pins in their order (#1240).
      machineChanged();
      return { ok: true };
    },
    [plane],
  );

  /** **Moves a pin one place earlier or later** among the pins the plane still has, as the
   *  chips draw them: a chip's Move left and Move right (#3128). */
  const moveWorkspacePin = useCallback(
    (workspace: string, by: -1 | 1): Promise<Ran> => {
      const order = pinMoved(
        strips.filter((name) => pinnedWorkspaces.includes(name)),
        workspace,
        by,
      );
      return order === undefined ? Promise.resolve({ ok: true }) : arrangePins(order);
    },
    [arrangePins, pinnedWorkspaces, strips],
  );

  /**
   * **Forgets a dormant pin**, for good: the existing unpin (`Store::pin_workspace`). The pins
   * before it are kept, nearest first, so this run's Undo can put it back in its place.
   */
  const forgetDormantPin = useCallback(
    async (name: string) => {
      const after = pinOrder.slice(0, Math.max(pinOrder.indexOf(name), 0)).reverse();
      const unpinned = await pinWorkspace(name, false);
      if (!unpinned.ok) {
        setReport({ from: "workspace.unpin", refused: true, words: unpinned.refused });
        return;
      }
      setForgottenPins((was) => [...was.filter((one) => one.name !== name), { name, after }]);
    },
    [pinOrder, pinWorkspace],
  );

  /**
   * **Undoes a Forget**: pins the workspace again and puts it back after the nearest pin that
   * came before it and is still pinned, or first when none is. Pinning appends, so the order is
   * written after it; arranging moves only the pins it names (`Store::arrange_workspaces`).
   */
  const undoForget = useCallback(
    async (name: string, after: string[]) => {
      /** Puts the Notice back, with what refused, so a refused Undo can be pressed again. */
      const refused = (words: string) => {
        setForgottenPins((was) => [...was.filter((one) => one.name !== name), { name, after }]);
        setReport({ from: "workspace.pin", refused: true, words });
      };
      setForgottenPins((was) => was.filter((one) => one.name !== name));
      const pinned = await pinWorkspace(name, true);
      if (!pinned.ok) return refused(pinned.refused);
      // The store's order now, read rather than remembered: another Undo may have just put a
      // neighbour back.
      const now = await commands
        .planePins(plane)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (now.status === "error") return refused(now.error);
      const said = now.data as Partial<typeof now.data> | null | undefined;
      const rest = (Array.isArray(said?.order) ? said.order : []).filter((one) => one !== name);
      const anchor = after.find((one) => rest.includes(one));
      const place = anchor === undefined ? 0 : rest.indexOf(anchor) + 1;
      const order = [...rest.slice(0, place), name, ...rest.slice(place)];
      const arranged = await commands
        .arrangeWorkspacePins(plane, order)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (arranged.status === "error")
        setReport({ from: "workspace.pin", refused: true, words: arranged.error });
      setPinning((asked) => asked + 1);
    },
    [pinWorkspace, plane],
  );

  /**
   * **A new workspace's repos, cloned on its chip** (#3649, spec #3647): "Cloning N repos…" on
   * the workspace's chip's card, counting each repo as it lands, then in its place "Cloned …",
   * or "Could not clone …" with Retry, which clones the ones that failed again on the same
   * notice, and the way to the workspace's Repos settings. Never closed on a timer: it stays
   * until it is dismissed.
   */
  const cloneNewRepos = useCallback(
    (name: string, picked: string[], after?: Said): Promise<void> => {
      cloneRuns += 1;
      const track = cloneRuns;
      const run = async (repos: string[], before?: Said) => {
        let done = 0;
        const onCancel = () => stopRepoClones(plane, name, repos);
        setReport({ ...reposCloning(name, repos, done, track, { onCancel }), after: before });
        const failed = await cloneRepos(
          plane,
          name,
          repos,
          () => {
            done += 1;
            if (done < repos.length)
              setReport(reposCloning(name, repos, done, track, { onCancel }));
          },
          (_, moved) => setReport(reposCloning(name, repos, done, track, { moved, onCancel })),
        );
        // Only the ones that did not land, on the same notice. Those the person stopped go again
        // with the refused ones that are left (#4394).
        const refused = failed.filter((one) => one.stopped !== true);
        const stopped = failed.filter((one) => one.stopped === true).map((one) => one.repo);
        const retry = (still: string[]) => void run([...still, ...stopped]);
        setReport(
          failed.length === 0
            ? reposCloned(name, repos, track)
            : refused.length === 0
              ? reposStopped(name, stopped, () => retry([]), track)
              : reposNotCloned(name, refused, retry, track),
        );
        setReplan((asked) => asked + 1);
      };
      return run(picked, after);
    },
    [plane],
  );

  /**
   * Makes it, through `charter workspace create`.
   *
   * **The name is not checked here.** `workspace_create` runs `wscmd::create`, which runs
   * `wscmd::ensure`, which is where `contain::workspace_name_ok` lives — so the window refuses
   * exactly the names a terminal refuses, in the same sentence. A refusal stays in the dialog,
   * where the operator is still standing.
   */
  const makeWorkspace = useCallback(
    async (name: string, vision: string, live: boolean, repos: string[]) => {
      setBusyMaking(true);
      // **Made here, so pinned** from before it is made (#1926): its folder is on disk before
      // `workspace_create` answers, and the watcher's read of the plane may land first. A
      // refused make lets it go again.
      setMadePins((was) => (was.includes(name) ? was : [...was, name]));
      const answer = await commands
        .workspaceCreate(plane, name, vision.trim() === "" ? null : vision, live)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      setBusyMaking(false);
      if (answer.status === "error") {
        setMadePins((was) => was.filter((one) => one !== name));
        setWorkspaceTrouble(answer.error);
        return;
      }
      setMakingWorkspace(false);
      setWorkspaceTrouble(undefined);
      forgetEndedClones(plane, name);
      // **Made here, so pinned** (ADR 0054): the operator made it in order to work in it, and
      // the workspace strip draws what is pinned, and has counted it pinned since the make was
      // asked for (above). A pin the store refused leaves the workspace made and says why,
      // beside what charter said about making it.
      const pinned = await pinWorkspace(name, true);
      // Said in the app's own words, hung from the new workspace's chip (#3648); the core's
      // lines, where it landed and whether it is LOCAL or LIVE, are kept behind Details, less
      // the lines written for an agent (#3344, `actionResult.ts`).
      const ready = workspaceMade(name, answer.data, {
        onAddVision: () => setAddingVision({ workspace: name, busy: false }),
        pinRefused: pinned.ok ? undefined : pinned.refused,
      });
      setReport(ready);
      // **Made here, so opened** (#3648): the same focus a press on its chip gives, so the
      // window lands in it even with a chat in front, which the new workspace's empty strip
      // leaves nothing in front of. The plane is read again rather than this window writing
      // the workspace into its own copy of the sidebar.
      focusWorkspace(name);
      setReplan((asked) => asked + 1);
      if (repos.length === 0) return;
      // The repos land after the workspace, one at a time and each on its own (ADR 0055):
      // the dialog is closed and the operator can start a chat while they clone.
      // Said in the same breath as the ready notice, which the window would never draw: the
      // clone's first line brings it, so both are kept (#3649).
      await cloneNewRepos(name, repos, ready);
    },
    [cloneNewRepos, focusWorkspace, pinWorkspace, plane],
  );

  /**
   * **Add a vision** (#3344): `purlis workspace vision`'s write, through the core's own broker.
   * A refusal stays in the dialog, in the core's words; once recorded, the window says so.
   */
  const saveVision = useCallback(
    async (workspace: string, vision: string) => {
      setAddingVision({ workspace, busy: true });
      const answer = await commands
        .workspaceVision(plane, workspace, vision)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (answer.status === "error") {
        setAddingVision({ workspace, busy: false, trouble: answer.error });
        return;
      }
      setAddingVision(undefined);
      setReport({
        from: "workspace.vision",
        refused: false,
        title: answer.data,
        words: "Its chats read it in its workspace.md.",
        ways: [
          { label: `Open ${workspace}`, onPress: () => focusWorkspace(workspace), primary: true },
        ],
      });
      setReplan((asked) => asked + 1);
    },
    [focusWorkspace, plane],
  );

  /** Asks for a new vault. It makes nothing: the dialog asks, and `vault_create` makes one. */
  const createVault = useCallback(() => {
    setVaultTrouble(undefined);
    setMakingVault(true);
  }, []);

  const pickVault = useCallback(() => setPickingVault(true), []);

  /**
   * Makes it, through `charter vault add`'s own path (`vault_create`), and opens its tab — a
   * vault is made to have secrets put in it, and the tab is where that is done. A refusal stays
   * in the dialog, in the core's words.
   */
  const makeVault = useCallback(
    async (name: string, provider: string, opVault: string | null) => {
      setBusyVault(true);
      const answer = await commands
        .vaultCreate(plane, name, provider, opVault)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      setBusyVault(false);
      if (answer.status === "error") {
        setVaultTrouble(answer.error);
        return;
      }
      setMakingVault(false);
      setVaultTrouble(undefined);
      reloadVaults();
      showView({ from: null, view: "vault", key: answer.data.name }, answer.data.name);
    },
    [plane, reloadVaults, showView],
  );

  /**
   * Asks about deleting one, and reads the core's guard so the dialog can show it first.
   *
   * The reading is for DRAWING. `workspace_remove` asks `work_at_risk` again, inside the core,
   * against the disk at the moment of the delete — this is what the operator sees before they
   * press, not what decides.
   */
  const askToRemove = useCallback(
    (workspace: string) => {
      setRemoving({ workspace, busy: false });
      void commands
        .workspaceAtRisk(plane, workspace)
        .then((answer) =>
          setRemoving((now) =>
            now?.workspace !== workspace
              ? now
              : answer.status === "ok"
                ? { ...now, atRisk: answer.data }
                : { ...now, unreadable: answer.error },
          ),
        )
        // A preview charter could not take is said as one. It is never drawn as an empty list:
        // "nothing would be lost" is a claim, and this is the absence of one.
        .catch((err: unknown) =>
          setRemoving((now) =>
            now?.workspace === workspace ? { ...now, unreadable: String(err) } : now,
          ),
        );
    },
    [plane],
  );
  /** **The unsaved work a save writes into the workspace is asked about first** (#3929): it goes
   *  with the workspace. Then the removal's own question. */
  const removeWorkspace = useCallback(
    (workspace: string) => {
      const work = unsavedIn(plane, { workspace });
      if (work.length === 0) return askToRemove(workspace);
      askUnsaved({ work, then: () => askToRemove(workspace) });
    },
    [askToRemove, askUnsaved, plane],
  );

  // Remove… on a chat's question about a workspace opens this same dialog (qw356): the chat
  // asked, and what removes anything is the person's press in it.
  useRemoveWorkspaceAsked(plane, removeWorkspace);

  // Open on a proposed change too long for a row opens its own tab (#1975, PC-7).
  useOpenProposalAsked(plane, showView);
  // Open on a runner chat's change its row cannot show whole opens its own tab (#2046).
  useOpenRunnerWriteAsked(plane, showView);
  useOpenRunnerDispatchAsked(plane, showView);
  /** A browser whose chat asked the person to take it over, taken over from its Ask (#2015):
   *  its view opens, where the bar says they drive and Done hands it back. */
  const showBrowserTakenOver = useCallback(
    (program: string) => showView(browserView(program), BROWSER_TITLE),
    [showView],
  );
  useOpenBrowserAsked(plane, showBrowserTakenOver);

  /** Asks for a workspace's new name. Nothing is renamed until the dialog is answered. */
  const renameWorkspace = useCallback(
    (workspace: string) => {
      setRenamingWs({ workspace, busy: false });
      // Which chats will start fresh (charter#367, D10): asked of the core, which knows which
      // harness finds a conversation by its folder. A question that fails names nobody.
      void commands
        .workspaceStartsFresh(plane, workspace)
        .then((answer) => {
          const startsFresh = answer.status === "ok" ? answer.data : null;
          setRenamingWs((now) => (now?.workspace === workspace ? { ...now, startsFresh } : now));
        })
        // A question that fails names nobody, and does not hold the answer back.
        .catch(() =>
          setRenamingWs((now) =>
            now?.workspace === workspace ? { ...now, startsFresh: null } : now,
          ),
        );
    },
    [plane],
  );

  /**
   * Renames it, through `workspace_rename` — which is `charter workspace rename` — and nothing
   * else (charter#367).
   *
   * **The core decides and moves everything on disk**: the refusals (a taken or invalid name, a
   * chat running in it), the folder, the worktrees, every record, the pins and the save. What
   * the window follows is its own: the view tabs on the workspace's strip, every view keyed by
   * its name and those views' pins (#1248), and the workspace it has picked. Then it reads the
   * plane again, as after any change to what workspaces there are.
   */
  const doRename = useCallback(
    async (workspace: string, name: string) => {
      setRenamingWs((now) => (now?.workspace === workspace ? { ...now, busy: true } : now));
      const answer = await commands
        .workspaceRename(plane, workspace, name)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (answer.status === "error") {
        setRenamingWs((now) =>
          now?.workspace === workspace ? { ...now, busy: false, trouble: answer.error } : now,
        );
        return;
      }
      setRenamingWs(undefined);
      // Through `change`, as every other write to the tabs is: a write past it is undone by the
      // next one, which starts from `now` (#1248).
      change((was) => followRename(was, workspace, name, root));
      setPicked((was) => (was === workspace ? name : was));
      setPickedSpot((was) => (was?.workspace === workspace ? undefined : was));
      setReport({
        from: `workspace.rename:${workspace}`,
        refused: false,
        words: answer.data.join(" "),
      });
      setPinning((asked) => asked + 1);
      setReplan((asked) => asked + 1);
    },
    [change, plane, root],
  );

  /**
   * Deletes it — **through `workspace_remove` and through nothing else**.
   *
   * There is one path from this window to a deleted workspace and the core's guard is inside
   * it (`wscmd::remove`, which runs `wscmd::work_at_risk` before `remove_dir_all`). `force` is
   * never passed on the operator's behalf: it arrives here only from the second button, which
   * does not exist until a refusal does and which names what it will discard.
   *
   * **And what it names comes back with the refusal** (charter-app#182). `workspace_remove`
   * answers with the at-risk list the core refused on, so the button and the sentence above it
   * are two readings of one moment rather than one reading and a memory.
   */
  const deleteWorkspace = useCallback(
    async (workspace: string, force: boolean) => {
      setRemoving((now) => (now?.workspace === workspace ? { ...now, busy: true } : now));
      const answer = await commands
        .workspaceRemove(plane, workspace, force)
        // A command that never reached the core said nothing about what is at risk, and an
        // empty list is the truthful shape for that: there is no refusal here to force past.
        .catch((err: unknown) => ({
          status: "error" as const,
          error: { said: String(err), at_risk: [] } satisfies Refused,
        }));
      if (answer.status === "error") {
        // Verbatim: the sentence names the repair — push or commit first — and the force
        // button is drawn beside it rather than instead of it.
        setRemoving((now) =>
          now?.workspace === workspace ? { ...now, busy: false, refusal: answer.error } : now,
        );
        return;
      }
      setRemoving(undefined);
      // The unsaved work a save would have written into it goes with it (#3929).
      discardRemoved(plane, { workspace });
      setReport({
        from: `workspace.remove:${workspace}`,
        refused: false,
        words: answer.data.join(" "),
      });
      // Nothing is picked any more: the workspace that was picked may be the one that has just
      // gone, and the sidebar's own focus rule decides what the window lands on.
      setPicked(undefined);
      setReplan((asked) => asked + 1);
    },
    [plane],
  );

  /** Lands the piece the row named in its clone, fast-forward only. The core never pushes. */
  const mergeWorktree = useCallback(
    async (cut: Cut): Promise<Ran> => {
      const answer = await commands
        .worktreeMerge(plane, cut.workspace, cut.repo, cut.piece)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (answer.status === "error") return { ok: false, refused: answer.error };
      // The clone has moved, so what the bottom bar says about it is a commit behind.
      setRereadWorkspace((asked) => asked + 1);
      return {
        ok: true,
        said: `${answer.data.branch} landed: ${answer.data.was} → ${answer.data.now}`,
      };
    },
    [plane],
  );

  /** Records the piece the row named as `done` in its log (charter#368). The tree stays. */
  const declareWorktreeDone = useCallback(
    async (cut: Cut): Promise<Ran> => {
      const answer = await commands
        .worktreeDone(plane, cut.workspace, cut.repo, cut.piece)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (answer.status === "error") return { ok: false, refused: answer.error };
      // The explorer's row reads what the piece said, so it is read again.
      setRereadWorkspace((asked) => asked + 1);
      return { ok: true, said: `${cut.repo} · ${cut.piece} — done` };
    },
    [plane],
  );

  /**
   * Hands a key the palette claimed to the chat in front.
   *
   * **It writes the bytes the pane's own terminal would have written.** The palette takes
   * `F2` on the window, capture-phase, so xterm never gets the keystroke to translate — and
   * re-dispatching the event is not a way out of that, because the focus is in the palette's
   * box by then. So what the terminal would have sent is sent, through the one path a pane's
   * input already takes (`send_input`). Nothing is read back: this is input, not a reading of
   * anything the harness said.
   */
  const sendKey = useCallback(
    async (key: string): Promise<Ran> => {
      // A browser the person drives, in the pane in focus, takes it as a chat does (#2014).
      const focused = focusedContent(tabs);
      if (
        key === PASS_THROUGH_KEY &&
        focused?.kind === "view" &&
        isBrowser(focused.view) &&
        sendToDriven(focused.view.key, key)
      )
        return { ok: true };
      if (frontSession === undefined)
        return { ok: false, refused: "No chat is in front, so there is nowhere to send it." };
      if (key !== PASS_THROUGH_KEY) return { ok: false, refused: `purlis cannot send ${key}.` };
      const sent = await commands
        .sendInput(plane, frontSession, PASS_THROUGH_BYTES)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      // Verbatim: a session that has stopped reading its input says so in the core's words.
      if (sent.status === "error") return { ok: false, refused: sent.error };
      // Nothing is said. The chat's own answer to the key is the report, and a banner after
      // every press of a key an operator means to press repeatedly is noise.
      return { ok: true };
    },
    [frontSession, plane, tabs],
  );

  /**
   * Pins or unpins one tab (#4762, ADR 0039 amended 2026-11-02): the one pin, the tab's.
   *
   * **The layout the core is told is what keeps it** (`window_layout`): the record holds it as
   * the tab's `pinned`, in the same write as the tab, so the mark is drawn at once and there is
   * no answer to wait for.
   */
  const pinTab = useCallback(
    async (id: number, pinned: boolean): Promise<Ran> => {
      // Its own, whatever its panes hold, so a tab whose only pane is empty is pinned too
      // (#4935). Nothing is said: the mark appearing on the tab is the answer.
      change((tabs) => setPinned(tabs, id, pinned));
      return { ok: true };
    },
    [change],
  );

  /**
   * A chat or view tab dragged onto another on the chat strip (SI-6): moved to where it was put
   * down, and pinned or unpinned when it crossed from one group to the other (`reorder.ts`).
   *
   * **The order and the pin are the window's at once** (SI-6b): both ride the next layout the
   * core is told. A pin refused is said, as the tab's menu would say it, and the tab is drawn
   * where its pin says it goes.
   */
  const dragTab = useCallback(
    (moved: number, onto: number) => {
      const made = afterDrop({
        whole: onStrip,
        drawn: shown,
        moved,
        onto,
        isPinned,
      });
      if (made === undefined) return;
      change((tabs) => ({ ...tabs, order: reslotted(tabs.order, made.order) }));
      if (made.pinned === undefined) return;
      void pinTab(moved, made.pinned).then((ran) => {
        if (!ran.ok) setReport({ from: "tab.drag", refused: true, words: ran.refused });
      });
    },
    [change, isPinned, onStrip, pinTab, shown],
  );

  /** What a screen reader hears while a tab is dragged on the chat strip. */
  const chatDragWords = useMemo(
    () => stripAccessibility("tab", (id) => tabs.byId[Number(id)]?.name ?? String(id)),
    [tabs.byId],
  );
  const dragSensors = useStripSensors();

  /**
   * Ignores a queued chat's request until it asks again (charter-app#248).
   *
   * **Nothing is changed here.** The ignore is the core's, and the core answers it with a
   * `chat-moved` carrying the queue without this chat, which lowers the project's and the
   * workspace's red counts in the same render that drops the item — they are all read from
   * that one queue.
   */
  const ignoreNeedsYou = useCallback(
    async (session: number): Promise<Ran> => {
      stoppedFor(session, undefined);
      const said = await commands
        .ignoreNeedsYou(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      return said.status === "error" ? { ok: false, refused: said.error } : { ok: true };
    },
    [plane, stoppedFor],
  );

  /** Cancels a chat's smart close: its tab's menu row, or the palette's. */
  const cancelSmartClose = useCallback(
    async (session: number): Promise<Ran> => {
      const said = await commands
        .cancelSmartClose(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      return said.status === "error" ? { ok: false, refused: said.error } : { ok: true };
    },
    [plane],
  );

  /**
   * **Opens Rename chat** (charter-app#254, #4753, #5343): what its rows, the palette's row and
   * F2 on its row run, through the one catalogue row. Always the dialog, which says
   * it renames the chat everywhere; a tab's own Rename is {@link beginTabRename}.
   */
  const beginRename = useCallback((session: number) => {
    setRenamingChat({ session, name: nameOfNow.current(session), busy: false });
  }, []);
  /**
   * **Asks the core to hold `typed` as chat `session`'s name**, and names every tab it leads
   * with the core's answer, not what was typed: it trims it, a blank is the default back, and a
   * name it will not draw is refused in its own words, which this answers. Charter's label only:
   * the harness keeps its own.
   */
  const renameChatTo = useCallback(
    async (session: number, typed: string): Promise<{ refused: string } | undefined> => {
      const said = await commands
        .renameChat(plane, session, typed)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") return { refused: said.error };
      change((tabs) => chatRenamed(tabs, session, said.data));
      // Its row is named by the core's list, which is read again for the new name.
      setReplan((was) => was + 1);
      return undefined;
    },
    [change, plane],
  );
  /** The name the Rename chat dialog was answered with. A refusal stays in the dialog, in the
   *  core's words. */
  const saveChatName = useCallback(
    async (typed: string) => {
      const asked = renamingChat;
      if (asked === undefined || asked.busy) return;
      setRenamingChat({ ...asked, busy: true, trouble: undefined });
      const refused = await renameChatTo(asked.session, typed);
      setRenamingChat(refused ? { ...asked, busy: false, trouble: refused.refused } : undefined);
    },
    [renameChatTo, renamingChat],
  );

  /**
   * **Opens a tab's Rename** (#5343): its menu's Rename…, a double-click on it and F2 on it. Its
   * dialog says what it renames, from what the tab holds (`tabRenaming.renameOf`); a view's tab
   * has no row, so it never opens on one.
   */
  const beginTabRename = useCallback((tab: number) => {
    if (renameOf(now.current, tab) === undefined) return;
    setRenamingTab({ tab, busy: false });
  }, []);
  /**
   * The tab's Rename answered: **its own name** where only the tab is named (a split, or "Only
   * this tab"), window state the core is not told; **else the chat's**, through the core, and
   * a name the tab had of its own goes, so the tab follows its chat again.
   */
  const saveTabName = useCallback(
    async ({ typed, only }: { typed: string; only: boolean }) => {
      const asked = renamingTab;
      if (asked === undefined || asked.busy) return;
      const renames = renameOf(now.current, asked.tab);
      if (renames === undefined) return setRenamingTab(undefined);
      if (only || renames.kind === "tab") {
        change((tabs) => nameTab(tabs, asked.tab, typed));
        return setRenamingTab(undefined);
      }
      setRenamingTab({ ...asked, busy: true, trouble: undefined });
      const refused = await renameChatTo(renames.session, typed);
      if (refused) return setRenamingTab({ ...asked, busy: false, trouble: refused.refused });
      change((tabs) => nameTab(tabs, asked.tab, undefined));
      setRenamingTab(undefined);
    },
    [change, renameChatTo, renamingTab],
  );
  /** What the open tab's Rename renames, as the tab is now. */
  const tabRenames = renamingTab === undefined ? undefined : renameOf(tabs, renamingTab.tab);
  /** "Use the chat name again": the tab's own name goes, and it is its first chat's again. */
  const clearTabName = useCallback(() => {
    const asked = renamingTab;
    if (asked === undefined) return;
    change((tabs) => nameTab(tabs, asked.tab, undefined));
    setRenamingTab(undefined);
  }, [change, renamingTab]);

  /**
   * Runs an extension's action from the palette (charter-app#341): on nothing in particular, in
   * the workspace in front. **One that asks first is asked about**, and run from the dialog;
   * the core refuses it without the yes whatever this does.
   */
  const runAction = useCallback(
    async (extension: string, action: RowAction, name: string): Promise<Ran> => {
      if (action.asks_first) {
        setAskingAction({ extension, action });
        return { ok: true };
      }
      const outcome = await runExtensionAction(
        plane,
        extension,
        action,
        { view: null, key: "", row: null },
        ofWorkspace,
        false,
      );
      if ("refused" in outcome) return { ok: false, refused: outcome.refused };
      return {
        ok: true,
        said: outcome.answer.overreach ?? `${name}: “${action.title}” ran.`,
      };
    },
    [ofWorkspace, plane],
  );

  /** A start refused for its profile's approval, while Approve and start… asks (#3942, #3968). */
  const [startApproving, setStartApproving] = useState<StartApproving>();
  /** What a start run again after Approve and start… answered, said in the line of `from` where
   *  it says anything: a quiet start leaves the notice as it is (#3968). */
  const sayAgain = useCallback(
    (from: string) => (ran: Ran) => {
      const said = saidOf(from, ran);
      if (said !== undefined) setReport(said);
    },
    [],
  );

  /**
   * A curation action chosen from a "Curate ▸" menu or the palette (ADR 0061): the core opens
   * the chat — the default profile, the action's runner, where it runs — and holds its prompt
   * until the harness reports its start; this puts its tab on the strip it is filed under, in
   * front, named for the action and its subject. A refusal is the core's sentence, before
   * anything started, with Approve and start… where its profile needs approving (#3968).
   */
  const curate = useCallback(
    (subject: string, action: string, label: string): Promise<Ran> => {
      const start = async (): Promise<Ran> => {
        const started = await commands
          .curate(plane, subject, action, STARTING_SIZE.columns, STARTING_SIZE.rows)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
        if (started.status === "error")
          return startRefusedRan(
            started.error,
            {
              named: `The curation action ${label}`,
              typed: "its prompt",
              // The same action again, said in the line of the row that asked.
              again: () => void start().then(sayAgain(`curate:${subject}/${action}`)),
            },
            setStartApproving,
          );
        const chat = started.data;
        showPlaced(chat);
        return { ok: true };
      };
      return start();
    },
    [plane, sayAgain, showPlaced],
  );

  /**
   * **Settings › Theme's New theme and Edit with a chat** (#2333): the core opens the chat at the
   * gallery's level with its prompt typed and never sent, as a curation chat is opened, and its
   * tab goes on the strip it is filed under, in front. A refusal is the core's sentence, with
   * Approve and start… where its profile needs approving (#3968).
   */
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<ThemeChatAsk>).detail;
      if (wanted.plane !== plane) return;
      void commands
        .themeChat(
          { plane, workspace: wanted.workspace },
          wanted.theme,
          wanted.from,
          STARTING_SIZE.columns,
          STARTING_SIZE.rows,
        )
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
        .then((started) => {
          if (started.status === "error") {
            const said = startRefused(
              started.error,
              {
                named: wanted.theme === null ? "New theme" : "Edit with a chat",
                typed: "its request",
                again: () => askThemeChat(wanted),
              },
              setStartApproving,
            );
            setReport({ from: "theme-chat", refused: true, ...said });
            return;
          }
          const chat = started.data;
          showPlaced(chat);
        });
    };
    window.addEventListener(THEME_CHAT, asked);
    return () => window.removeEventListener(THEME_CHAT, asked);
  }, [change, plane, showPlaced]);

  // **Discuss** in an artifact's bar (#5957, ADR 0122 §5): a chat about the page, its tab placed here.
  useArtifactDiscuss(plane, showPlaced, setReport, setStartApproving);

  /**
   * **The Skill studio's "Ask a chat to draft a skill", Try it, a draft's redo and "Ask a chat to
   * write them"** (SK-9, #2772; S9, #3690; #3925): the core opens
   * the chat with its prompt typed and never sent (Try it's in a throwaway folder, with the
   * built-in skills and the draft alone), and its tab goes on the strip it is filed under, in
   * front. A refusal is the core's sentence, with Approve and start… where its profile needs
   * approving (#3942).
   */
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<SkillChatAsk>).detail;
      if (wanted.plane !== plane) return;
      // "Ask the chat to redo it" (S9, #3690): typed into the chat that drafted it, where a tab
      // here holds it, and that tab brought to the front; else a chat opened afresh below.
      const redoIn =
        wanted.kind === "redo" && wanted.chat !== null
          ? { session: wanted.chat, tab: tabHolding(now.current, wanted.chat) }
          : undefined;
      if (wanted.kind === "redo" && redoIn?.tab !== undefined) {
        const { session, tab } = redoIn;
        void commands
          .skillDraftRedoIn(plane, wanted.id, session)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
          .then((said) => {
            if (said.status === "error") {
              setReport({ from: "skill-chat", refused: true, words: said.error });
              return;
            }
            bringToFront(tab);
            giveKeyboardTo(plane, session);
            setReport({
              from: "skill-chat",
              refused: false,
              words:
                said.data.kind === "typed"
                  ? `Typed the request to redo ${wanted.name} into ${nameOfNow.current(session)}. Nothing was sent.`
                  : handedSaid(said.data, nameOfNow.current(session)),
            });
          });
        return;
      }
      const starting =
        wanted.kind === "draft"
          ? commands.skillDraftChat(
              plane,
              wanted.level,
              wanted.what,
              STARTING_SIZE.columns,
              STARTING_SIZE.rows,
            )
          : wanted.kind === "triggers" ||
              wanted.kind === "evals" ||
              wanted.kind === "checks" ||
              wanted.kind === "improve"
            ? {
                triggers: commands.skillTriggerTestsChat,
                evals: commands.skillOutcomeEvalsChat,
                checks: commands.skillChecksChat,
                improve: commands.skillImproveChat,
              }[wanted.kind](
                plane,
                wanted.level,
                wanted.name,
                STARTING_SIZE.columns,
                STARTING_SIZE.rows,
              )
            : wanted.kind === "redo"
              ? commands.skillDraftRedo(plane, wanted.id, STARTING_SIZE.columns, STARTING_SIZE.rows)
              : commands.skillDraftTry(
                  plane,
                  wanted.id,
                  wanted.profile,
                  STARTING_SIZE.columns,
                  STARTING_SIZE.rows,
                );
      void starting
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
        .then((started) => {
          if (started.status === "error") {
            // With Approve and start… where its profile needs approving (#3942).
            setReport(skillChatRefused(wanted, started.error, setStartApproving, askSkillChat));
            return;
          }
          const chat = started.data;
          // What the core recorded on the chat, so its chip and the skill's page say it at once.
          const link = linkOfAsk(wanted);
          showPlaced(chat, link === undefined ? undefined : { studio: link });
        });
    };
    window.addEventListener(SKILL_CHAT, asked);
    return () => window.removeEventListener(SKILL_CHAT, asked);
  }, [bringToFront, change, plane, showPlaced]);

  /**
   * **"Start a chat here"** on a file or folder row (FM-9): a chat the core starts on the
   * project's default profile in the branch's folder, with a reference to the row typed as its
   * first prompt once its harness has started, and never sent. Its tab goes on the strip it is
   * filed under, as a curation chat's does. Where the harness cannot be typed into, the core
   * copies the reference, and the window says so. A refusal is the core's sentence, with Approve
   * and start… where its profile needs approving (#3968).
   */
  const startChatHere = useCallback(
    (at: BranchPath, lines?: { first: number; last: number }): Promise<Ran> => {
      const start = async (): Promise<Ran> => {
        const started = await commands
          .startChatHere(
            plane,
            at.workspace,
            at.repo,
            at.piece,
            at.path,
            // The lines picked in a file's preview (#1151): the core types them in the reference.
            lines ?? null,
            STARTING_SIZE.columns,
            STARTING_SIZE.rows,
          )
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
        if (started.status === "error")
          return startRefusedRan(
            started.error,
            {
              named: "Start a chat here",
              typed: "its reference",
              // The same start again, said as a file's start is.
              again: () => void start().then(sayAgain("file.chat")),
            },
            setStartApproving,
          );
        const chat = started.data;
        showPlaced({ ...chat, persona: null });
        if (chat.copied === null) return { ok: true };
        const why = chat.copied.charAt(0).toUpperCase() + chat.copied.slice(1);
        return { ok: true, said: `${why}: ${chat.text}` };
      };
      return start();
    },
    [plane, sayAgain, showPlaced],
  );

  /**
   * **What the first task's tab asks for** (FR-28): a run started by the core — on a branch of its
   * own, with the task typed and unsent — whose tab goes on the strip it is filed under, in
   * front; and a run's diff, in a shell tab in its branch's folder with the core's diff command
   * run there.
   */
  const [firstTaskRuns, setFirstTaskRuns] = useState<FirstTaskDoes["runs"]>({});
  const firstTaskDoes = useMemo<FirstTaskDoes>(
    () => ({
      runs: firstTaskRuns,
      start: async (clone, profile, persona, run) => {
        const started = await commands
          .firstTaskRun(
            plane,
            clone,
            profile,
            persona,
            run,
            STARTING_SIZE.columns,
            STARTING_SIZE.rows,
          )
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
        if (started.status === "error") return started.error;
        const chat = started.data;
        setFirstTaskRuns((was) => ({ ...was, [clone]: { ...was[clone], [run]: chat } }));
        showPlaced(chat);
        return chat;
      },
      showDiff: (run) => openShell(run.folder, run.workspace ?? OUTSIDE, { line: run.diff }),
    }),
    [firstTaskRuns, openShell, plane, showPlaced],
  );

  /**
   * The chats a Resume started (SI-8d), by session, until each has said whether its harness
   * brought the conversation back: a ref, because what it holds decides a start and never a
   * render.
   */
  const resuming = useRef(new Map<number, Resuming>());

  /**
   * **Resume a session from its record** (SI-8d): the core starts a NEW chat in the record's
   * place, on its harness, given its conversation where it can be, and told the record in its
   * briefing (`resume_session`). Its tab opens in front, and the note above the panes says which
   * happened — *was resumed*, or *came back as a new chat: <why>* — from the same `reopened`
   * list a relaunch's chats are said from.
   *
   * `insteadOf` is the second ask, for a chat whose harness could not bring the conversation
   * back ({@link lostOnResume}): the same record, started fresh, as the same chat (ADR 0066).
   */
  const resumeSession = useCallback(
    async (path: string, insteadOf?: number, into?: number): Promise<Ran> => {
      const name = String(takeChatNumber());
      const afterFailure = insteadOf !== undefined;
      const said = await commands
        .resumeSession(
          plane,
          path,
          name,
          insteadOf ?? null,
          STARTING_SIZE.columns,
          STARTING_SIZE.rows,
        )
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") return { ok: false, refused: said.error };
      const chat = said.data;
      // A new occurrence: whatever was dismissed about this chat was about the one before.
      for (const family of CHAT_NOTES) {
        const cause = chatNote(family, chat);
        if (cause !== undefined) showAgain(cause);
      }
      resuming.current.set(chat.session, { path, afterFailure, heard: false });
      setStartedIn((was) => ({ ...was, [chat.session]: filedFor(chat.cwd, focused) }));
      setReopened((was) => [...was.filter((one) => one.session !== chat.session), chat]);
      // **Resume on an ended chat's band** (#5019) starts it in the pane that showed it, and
      // that pane no longer shows it as ended. Any other resume lands as a press (#5289).
      if (into !== undefined) forgetEndedHere(into);
      if (into !== undefined && tabHolding(now.current, into) !== undefined)
        change((tabs) => replaceSession(tabs, into, chat.session));
      else
        showResumed(
          {
            session: chat.session,
            chat: name,
            who: whoOf(chat.persona, chat.harness),
            label: chat.label,
          },
          filedFor(chat.cwd, focused),
        );
      return { ok: true };
    },
    [change, focused, forgetEndedHere, plane, showAgain, showResumed, takeChatNumber],
  );

  /**
   * **A chat the window started again** — a Retry now, or a Start fresh (NO-3) — known the way a
   * chat a relaunch put back is: where it is filed, its notes on how it came back, and its
   * shell's mark. Its pin is its tab's (#4935): a restart keeps the tab, and a chat that opens
   * a tab of its own carries a pin only a record from before tabs were recorded says
   * (`pinnedAsBefore`).
   */
  const noteStarted = useCallback(
    (chat: OpenChat) => {
      setStartedIn((was) => ({ ...was, [chat.session]: filedFor(chat.cwd, focused) }));
      setReopened((was) => [...was.filter((one) => one.session !== chat.session), chat]);
      if (isShell(chat)) setShells((was) => new Set(was).add(chat.session));
    },
    [focused],
  );

  /**
   * **Each chat's finished tasks** (#1485), read again whenever the sidebar is: that is when a
   * chat started, ended or closed, which is when a task finishes or its asking chat goes.
   */
  const {
    finished: finishedTasks,
    read: rereadFinished,
    readQuietly: rereadFinishedQuietly,
    settled: finishedSettled,
  } = useFinishedTasks(plane, sidebar);
  useEffect(() => {
    finishedQuietly.current = rereadFinishedQuietly;
  }, [rereadFinishedQuietly]);
  useEffect(() => {
    finishedNow.current = finishedTasks;
  }, [finishedTasks]);
  /**
   * **A finished task's Merge… and Discard branch…, from its row's menu** (#1534): the Changes
   * tab's own asks (`taskBranchActs.tsx`), drawn here, with what a press came to said on the
   * window's line. The asks are held in a ref so the catalogue's doing is not rebuilt on every
   * draw.
   */
  const taskBranchActs = useTaskBranchActs({
    plane,
    told: (said) =>
      setReport({ from: "task.branch", refused: said.tone === "trouble", words: said.says }),
    changed: rereadFinished,
  });
  const branchActs = useRef(taskBranchActs);
  useEffect(() => {
    branchActs.current = taskBranchActs;
  });
  /** Clear finished: the rows go, and nothing else does. */
  const clearFinished = useCallback(
    (ids: string[]) => {
      void commands
        .clearFinishedTasks(plane, ids)
        .catch(() => undefined)
        .then(rereadFinished);
    },
    [plane, rereadFinished],
  );
  /**
   * **Reopen a finished task** (#1485): the core starts a new chat on the task's conversation,
   * and it takes the focused pane as any chat started does (#4758). It is an ordinary chat from then on. A
   * refusal is the row's to say.
   */
  const reopenById = useCallback(
    async (id: string): Promise<string | undefined> => {
      const said = await commands
        .reopenFinishedTask(plane, id, STARTING_SIZE.columns, STARTING_SIZE.rows)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") return said.error;
      const chat = said.data;
      noteStarted(chat);
      showResumed(
        {
          session: chat.session,
          chat: chat.name,
          who: whoOf(chat.persona, chat.harness),
          label: chat.label,
        },
        filedFor(chat.cwd, focused),
      );
      rereadFinished();
      return undefined;
    },
    [focused, noteStarted, plane, rereadFinished, showResumed],
  );
  /** The same, from a finished task's row. */
  const reopenFinished = useCallback(
    (task: FinishedTask): Promise<string | undefined> => reopenById(task.id),
    [reopenById],
  );
  /**
   * **Resume, on the band of a pane left on a task that ended** (#4757, #4756): its finished row
   * reopened, as Reopen on that row does, in the pane that showed it rather than a tab of its
   * own. A refusal is the row's to say, as Reopen's is.
   */
  const resumeTaskIn = useCallback(
    async (pane: number, task: FinishedTask) => {
      const id = now.current.inFront;
      const was =
        id === undefined
          ? undefined
          : shownIn(now.current, id).find((one) => one.pane === pane)?.session;
      const said = await commands
        .reopenFinishedTask(plane, task.id, STARTING_SIZE.columns, STARTING_SIZE.rows)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") return;
      const chat = said.data;
      noteStarted(chat);
      if (was === undefined)
        showResumed(
          {
            session: chat.session,
            chat: chat.name,
            who: whoOf(chat.persona, chat.harness),
            label: chat.label,
          },
          filedFor(chat.cwd, focused),
        );
      else {
        forgetEndedFiled(was);
        change((tabs) => replaceSession(tabs, was, chat.session));
      }
      rereadFinished();
    },
    [change, focused, forgetEndedFiled, noteStarted, plane, rereadFinished, showResumed],
  );

  /**
   * Reads chat `session`'s skills for the Use skill… open on it: an answer for a chat it is no
   * longer open on is let go.
   */
  const readUseSkillOf = useCallback(
    (session: number) => {
      void commands
        .chatSkills(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
        .then((said) => {
          if (said.status === "ok") setChatSkills(session, said.data);
          setUsingSkill((was) =>
            was?.session !== session
              ? was
              : said.status === "ok"
                ? { ...was, skills: said.data }
                : { ...was, trouble: said.error },
          );
        });
    },
    [plane, setChatSkills],
  );

  /**
   * **A chat a pane shows as a task was started again under a new number** (#1486): what the
   * pane remembers of it goes with it, so the pane keeps showing it whichever arrives first,
   * the new number in the tabs or in the core's list. Between the two it is drawn as it was
   * last known, never as the session's own chat.
   *
   * **And Use skill… open on it follows it** (#6193): the old session leaves the core's open
   * chats, so a pick aimed there was refused and the dialog stayed up. The new session's skills
   * are read, as the new run started with what the skill folders hold now.
   */
  const followRestart = useCallback(
    (was: number, to: number) => {
      setRemembered((all) => {
        let moved: Map<string, Crumbs> | undefined;
        for (const [key, crumbs] of all) {
          const [tab, pane, session] = key.split(":");
          if (Number(session) !== was) continue;
          moved ??= new Map(all);
          moved.delete(key);
          moved.set(`${tab}:${pane}:${to}`, {
            ...crumbs,
            path: crumbs.path.map((chat) =>
              chat.session === was ? { ...chat, session: to } : chat,
            ),
          });
        }
        return moved ?? all;
      });
      const using = usingSkillNow.current;
      if (using?.session !== was) return;
      setUsingSkill({ session: to, picks: using.picks, busy: false });
      readUseSkillOf(to);
    },
    [readUseSkillOf],
  );

  /**
   * **A chat restarted** (#1342, #1428): the same chat on its conversation, in a new session,
   * which takes the old one's place as Start fresh's does.
   */
  const chatRestarted = useCallback(
    (was: number, chat: OpenChat) => {
      setReopened((all) => all.filter((one) => one.session !== was));
      // Its tab keeps its pin: the tab is the same, its chat in a new session.
      noteStarted(chat);
      followRestart(was, chat.session);
      change((tabs) => replaceSession(tabs, was, chat.session));
    },
    [change, followRestart, noteStarted],
  );

  /**
   * **The chats owed a restart** (#1342, #1428) — to take a sandbox grant, or because the person
   * asked with Restart chat — and the driver that restarts each once its turn has ended: here
   * and not in a Notice, so dismissing the Notice or redrawing the pane loses none. The core
   * holds the same list (`owed_restarts`), which is read again when the view is drawn anew.
   *
   * **One restart, whoever asked** (`restart_chat`): a block's Allow, Restart now on a Notice,
   * a tab's Restart chat and the Notice after a sandbox setting changed all come through here.
   */
  const [owedRestarts, setOwedRestarts] = useState<readonly number[]>([]);
  /** Why a restart did not happen, by session, until it is retried or put away. */
  const [restartTrouble, setRestartTrouble] = useState<Readonly<Record<number, string>>>({});
  /** Why a restart has not happened yet, in the core's sentence, by session: the chat waits on
   *  a permission prompt, and restarts once that is answered. */
  const [restartNotYet, setRestartNotYet] = useState<Readonly<Record<number, string>>>({});
  const restartingNow = useRef(new Set<number>());
  /** The chats a restart is under way for, which each one's pane says while it is. */
  const [restartsRunning, setRestartsRunning] = useState<readonly number[]>([]);
  useEffect(() => {
    let gone = false;
    void commands
      .owedRestarts(plane)
      .then((owed) => {
        // A list or nothing, as the view's other reads take one (#1342).
        if (!gone && owed.status === "ok")
          setOwedRestarts(Array.isArray(owed.data) ? owed.data : []);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane]);
  const oweRestart = useCallback(
    (session: number) => setOwedRestarts((owed) => withRestartOwed(owed, session)),
    [],
  );
  /**
   * **An ask answered in the Inbox** (#1692): what its source's Notice does after the same
   * answer. A sandbox host answered there is put away on the chat's pane, only while the pane
   * still holds that very host, and an Allow owes the chat its restart, as the block Notice's
   * does, unless the chat's proxy took it live and the command carried on (#1666). A permission
   * and a dispatch need nothing here: their Notices read the same source.
   */
  const inboxAnswered = useCallback(
    (ask: Shown, option: Offered, live: boolean) => {
      if (ask.answer.via !== "sandbox-block") return;
      const block = blockHeldFor(sandboxBlocks, ask.session, ask.answer.shown);
      if (block !== undefined) blockAnswered(ask.session, block);
      if (option.allows && !live) oweRestart(ask.session);
    },
    [blockAnswered, oweRestart, sandboxBlocks],
  );
  /**
   * Restarts chat `session`, which is owed it: the driver's step. `{ now: true }` is the
   * person's Restart now (#5753): the core goes ahead while the chat waits on its own work, which a
   * restart at a pause waits for, and the chat is told what the restart ended.
   */
  const restartOwed = useCallback(
    (session: number, { now }: { now: boolean } = { now: false }) => {
      if (restartingNow.current.has(session)) return;
      restartingNow.current.add(session);
      setRestartsRunning((running) => [...running, session]);
      const failed = (why: string) => {
        setOwedRestarts((owed) => owed.filter((one) => one !== session));
        setRestartNotYet((was) => withoutKey(was, session));
        setRestartTrouble((was) => ({ ...was, [session]: why }));
      };
      void commands
        .restartChat(plane, session, STARTING_SIZE.columns, STARTING_SIZE.rows, now)
        .then((done) => {
          if (done.status === "error") failed(done.error);
          else if (done.data.chat !== null) {
            const chat = done.data.chat;
            setOwedRestarts((owed) => owed.filter((one) => one !== session));
            setRestartTrouble((was) => withoutKey(was, session));
            setRestartNotYet((was) => withoutKey(was, session));
            chatRestarted(session, chat);
            // What the new run's start found to say: that a chat which ran without the
            // sandbox runs in it again. `?? []`, as a start's notices are read.
            const notes = done.data.notices ?? [];
            if (notes.length > 0)
              setStartNotes((was) => ({ ...was, [chat.session]: { notes, agentsMd: [] } }));
          } else {
            // Not yet (a permission prompt is open): still owed, asked again at its next move.
            // Said on its pane in the core's sentence, so a press is never answered by nothing.
            const why = done.data.not_yet ?? NOT_YET;
            setRestartNotYet((was) => (was[session] === why ? was : { ...was, [session]: why }));
          }
        })
        .catch((err: unknown) => failed(`purlis could not restart the chat: ${String(err)}`))
        .finally(() => {
          restartingNow.current.delete(session);
          setRestartsRunning((running) => running.filter((one) => one !== session));
        });
    },
    [plane, chatRestarted],
  );
  /**
   * **A chat the core started again, or ended, because its project's Claude Code settings no
   * longer pass** (ADR 0067 as amended 2026-10-16): the core has done it already, whatever this
   * window was doing, and the window only redraws, as after its own restart. Filtered on the
   * plane, as `chat-moved` is.
   */
  const redrawRescreened = useRef<(told: Rescreened) => void>(() => undefined);
  useEffect(() => {
    redrawRescreened.current = (told: Rescreened) => {
      setOwedRestarts((owed) => owed.filter((one) => one !== told.session));
      setRestartNotYet((was) => withoutKey(was, told.session));
      if (told.chat !== null) {
        const chat = told.chat;
        setRestartTrouble((was) => withoutKey(was, told.session));
        chatRestarted(told.session, chat);
        if (told.notices.length > 0)
          setStartNotes((was) => ({
            ...was,
            [chat.session]: { notes: told.notices, agentsMd: [] },
          }));
      } else if (told.ended !== null) {
        const why = told.ended;
        setRestartTrouble((was) => ({ ...was, [told.session]: why }));
        // The core keeps it among the chats that would not start, with why and a Retry.
        void commands
          .chatsThatWouldNotStart(plane)
          .then((trouble) => {
            if (trouble.status === "ok") setWouldNotStart(trouble.data ?? []);
          })
          .catch(() => undefined);
      }
    };
  }, [chatRestarted, plane]);
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<Rescreened>("chat-project-settings-changed", (event) => {
          const told = event.payload;
          if (gone || told.plane !== plane) return;
          redrawRescreened.current(told);
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);
  /** **Asks about a chat's browser that ended with it** (#2015), as the core told them, for the
   *  Inbox's updates. */
  const [browserAsksEnded, setBrowserAsksEnded] = useState<readonly AskEnded[]>([]);
  // The report drafts the person filed in this window (#5694): each is an update of its own.
  const [sentDrafts, setSentDrafts] = useState<readonly SentDraft[]>([]);
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<AskEnded>(BROWSER_ASK_ENDED, (event) => {
          const told = event.payload;
          if (gone || told.plane !== plane) return;
          setBrowserAsksEnded((was) => [...was, told]);
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);
  // **A chat whose browsers ended because the person narrowed its reach** (#2013): its tab
  // says so, once. Filtered on the plane, as `chat-moved` is.
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<BrowsersEnded>("chat-browsers-ended", (event) => {
          const told = event.payload;
          if (gone || told.plane !== plane) return;
          setStartNotes((was) => withNote(was, told.session, told.note));
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);
  // **Only at the chat's next pause between turns** (#1342, #1986): mid-turn it would lose
  // what it was doing; a harness that reports no state is never restarted behind the person's
  // back, and its pane offers Restart now instead. A chat whose program ended first is
  // restarted by nothing, and is owed nothing ({@link restartsDue}).
  useEffect(() => {
    if (owedRestarts.length === 0) return;
    const drive = () => {
      const states = chats.store.statesFor(chats.plane);
      const due = restartsDue(owedRestarts, states);
      for (const session of due.restart) restartOwed(session);
      // Dropped only once the core, which hears the program itself end, owes it nothing: a
      // hook can say the session ended while its program runs on ({@link owedAfterEnded}).
      if (due.ended.length > 0)
        void commands
          .owedRestarts(plane)
          .then((read) => {
            if (read.status === "ok" && Array.isArray(read.data)) {
              const coreOwes = read.data;
              setOwedRestarts((owed) => {
                const kept = owedAfterEnded(owed, due.ended, coreOwes);
                return kept.length === owed.length ? owed : kept;
              });
            }
          })
          .catch(() => undefined);
    };
    drive();
    return chats.store.subscribe(drive);
  }, [owedRestarts, chats, plane, restartOwed]);
  /**
   * The state of each chat owed a restart, by session (#1888): the pane's words follow the
   * chat's state as well as the owed list, so a chat owed a restart before it said it was
   * mid-turn says that the restart waits for its turn. The same states answer the same record,
   * so a move of any other chat redraws nothing.
   */
  const owedStates = useChatsSelect(
    chats,
    (states): OwedStates =>
      Object.fromEntries(
        // Waiting on its own work is mid-turn to a restart (#3887): it waits, and says so.
        owedRestarts.map((session) => [session, turnStateOf(states, session)]),
      ),
    sameStates,
  );
  /**
   * What each chat's pane says of its restart ({@link RestartSaid}): under way, not yet, owed by
   * hand, or why it failed.
   */
  const restartsSaid = useMemo(() => {
    const out: Record<number, RestartSaid> = {};
    for (const session of owedRestarts) {
      const state = owedStates[session] ?? "unknown";
      // Mid-turn it waits for the turn to end, and its pane says so, with Restart now (qw351).
      out[session] = { owed: true, byHand: state === "unknown", midTurn: state === "running" };
    }
    for (const session of restartsRunning) out[session] = { ...out[session], running: true };
    for (const [session, notYet] of Object.entries(restartNotYet))
      if (owedRestarts.includes(Number(session)))
        out[Number(session)] = { ...out[Number(session)], notYet };
    for (const [session, trouble] of Object.entries(restartTrouble))
      out[Number(session)] = { trouble };
    return out;
  }, [owedRestarts, owedStates, restartsRunning, restartNotYet, restartTrouble]);
  /** What a row of the Chats list says of its chat's restart (#1462): why it was refused, or
   *  why it waits. */
  const restartsOnRows = useMemo(() => {
    const out: Record<number, string> = {};
    for (const [session, said] of Object.entries(restartsSaid)) {
      const words = said.trouble ?? said.notYet;
      if (words !== undefined) out[Number(session)] = words;
    }
    return out;
  }, [restartsSaid]);
  /**
   * **The person asks for chat `session` to restart** (#1428, and #1362's Restart now): the core
   * owes it the restart (`ask_chat_restart`), and it happens when the chat's turn has ended.
   * Mid-turn it waits, as a restart for a grant does. A chat whose harness reports no state
   * restarts now: the person pressed for it, so there is nobody else to wait for.
   */
  const askRestart = useCallback(
    async (session: number, { now }: { now: boolean } = { now: false }) => {
      const said = await commands
        .askChatRestart(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") {
        setRestartTrouble((was) => ({ ...was, [session]: said.error }));
        return;
      }
      setRestartTrouble((was) => withoutKey(was, session));
      oweRestart(session);
      // Mid-turn, or waiting on its own work (#3887), it waits for the turn to end, unless the
      // person pressed Restart now (#5753).
      if (now || !inItsTurn(chats.store.statesFor(chats.plane), session))
        restartOwed(session, { now });
    },
    [chats, oweRestart, plane, restartOwed],
  );
  /** **A note another window of the project cleared** (#5225): it is not said here again. */
  useEffect(() => {
    const listening = listen<NoteCleared>(NOTE_CLEARED, ({ payload: cleared }) => {
      // Every window hears it, this one too, which has cleared it already.
      if (cleared.plane !== plane || cleared.from === thisWindow()) return;
      const { session, note } = cleared;
      // One a store of this window's own keeps (#5363).
      if (keptOutside(note)) letGoHere(plane, session, note);
      if (note.kind === "start") dropStartNote(session);
      if (note.kind === "by-hand") dropByHand(session);
      if (note.kind === "block") letBlockGo(session, note.block);
      if (note.kind === "restart") {
        setRestartTrouble((was) => withoutKey(was, session));
        setRestartNotYet((was) => withoutKey(was, session));
      }
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [dropByHand, dropStartNote, letBlockGo, plane]);

  /**
   * **What this window holds of a chat another window shows, handed to it** (#5225): each window
   * hears what a chat says from when it is listening, so a note held from before a tab was torn
   * off is in no other. Said as it comes to be held, and again when a tab window first draws. It
   * is kept here too, for the chat coming back; clearing it clears both (`note-cleared`).
   */
  const notesHeldElsewhere = useRef(new Map<number, string>());
  const handNotes = useCallback(
    (session: number, held: Omit<NotesHanded, "plane" | "session" | "from">) =>
      void emit(NOTES_HANDED, {
        plane,
        session,
        ...held,
        from: thisWindow(),
      } satisfies NotesHanded).catch(() => undefined),
    [plane],
  );
  const handAgain = useRef<() => void>(() => undefined);
  useEffect(() => {
    // Its start's notes, its blocks, and its by-hand banner and restart line (#5363). A restart
    // not yet made is not handed: the window that owes it says why at the chat's next move.
    const heldOf = (session: number) => ({
      start: startNotes[session],
      blocks: sandboxBlocks[session] ?? [],
      byHand: byHand[session],
      restart: restartTrouble[session],
    });
    const holding = new Map<number, string>();
    for (const session of shownElsewhere) {
      const held = heldOf(session);
      const { start, blocks, byHand: banner, restart } = held;
      const none = start === undefined && blocks.length === 0 && banner === undefined;
      if (none && restart === undefined) continue;
      const said = JSON.stringify(held);
      holding.set(session, said);
      if (notesHeldElsewhere.current.get(session) !== said) handNotes(session, held);
    }
    notesHeldElsewhere.current = holding;
    handAgain.current = () => {
      for (const session of holding.keys()) handNotes(session, heldOf(session));
    };
  }, [byHand, handNotes, restartTrouble, sandboxBlocks, shownElsewhere, startNotes]);
  const askedForNotes = useRef(false);
  useEffect(() => {
    const listening = Promise.all([
      listen<NotesHanded>(NOTES_HANDED, ({ payload: handed }) => {
        if (handed.plane !== plane || handed.from === thisWindow()) return;
        const { session, start, blocks, byHand: banner, restart } = handed;
        if (start !== undefined)
          setStartNotes((was) => ({ ...was, [session]: withStartHanded(was[session], start) }));
        takeBlocks(session, blocks);
        // What this window heard itself stands: it is the same chat's, and newer or the same.
        if (banner !== undefined)
          setByHand((was) => (session in was ? was : { ...was, [session]: banner }));
        if (restart !== undefined)
          setRestartTrouble((was) => (session in was ? was : { ...was, [session]: restart }));
      }),
      listen<NotesAsked>(NOTES_ASKED, ({ payload: asked }) => {
        if (asked.plane === plane && asked.from !== thisWindow()) handAgain.current();
      }),
    ])
      .then((stops) => {
        if (tabWindow !== undefined && !askedForNotes.current) {
          askedForNotes.current = true;
          void emit(NOTES_ASKED, { plane, from: thisWindow() } satisfies NotesAsked).catch(
            () => undefined,
          );
        }
        return stops;
      })
      .catch(() => [] as (() => void)[]);
    return () =>
      void listening
        .then((stops) => Promise.all(stops.map((stop) => stop())))
        .catch(() => undefined);
  }, [plane, tabWindow, takeBlocks]);

  const answerRestart = useCallback(
    (session: number, act: RestartAct) => {
      setRestartTrouble((was) => withoutKey(was, session));
      setRestartNotYet((was) => withoutKey(was, session));
      noteCleared(session, { kind: "restart" });
      // Asked again, so a restart the core no longer owes is owed once more.
      if (act === "now") void askRestart(session, { now: true });
      // Owed already, and the person will not wait for the turn to end: the driver's step, now,
      // unless the chat's program has ended meanwhile (#1986).
      if (act === "mid-turn") {
        const states = chats.store.statesFor(chats.plane);
        const due = restartsDue([session], states, { now: session });
        if (due.restart.includes(session)) restartOwed(session, { now: true });
      }
    },
    [askRestart, chats, noteCleared, restartOwed],
  );
  /** Restart chat, from a tab's menu or the palette. */
  useEffect(() => {
    restartNow.current = (session) => void askRestart(session);
  }, [askRestart]);
  /** A task whose Restart chat is asked about in a dialog: its tab draws no breadcrumb. */
  const [restartingTask, setRestartingTask] = useState<{ session: number; name: string }>();
  /**
   * **Restart chat on a row of the Chats list**, for a chat with no tab (#1462). A task is
   * asked first, as its tab's row asks, on the row it was pressed on; the answer restarts it.
   * Any other chat restarts as a tab's row restarts it.
   */
  const restartListed = useCallback(
    (session: number) => {
      if (askedByNow(session) !== undefined) {
        const name =
          chatsListed.current.find((one) => one.session === session)?.name ?? `chat ${session}`;
        setTaskEndInline({ session, where: "row", ...restartAsked(name) });
        return;
      }
      void askRestart(session);
    },
    [askRestart, askedByNow],
  );
  /**
   * **Restart chat, by the chat's number** (#1462, #4753): from its pane, its row and the
   * palette alike. A task with a tab of its own is asked first on its tab; a task with none is
   * asked on its row, as the Chats list asked ({@link restartListed}).
   */
  const restartChat = useCallback(
    (session: number) => {
      const tab = ownTabOf(now.current, session);
      if (tab === undefined) {
        restartListed(session);
        return;
      }
      // **A task's restart is asked about first** (#1489): a row in its tab's menu that ends
      // its program takes a second step, as its Stop does, and the same one (#1488): asked
      // on the task's own breadcrumb line, with Keep. Its tab comes in front first, so the
      // question is never asked where the person is not looking.
      if (askedByNow(session) !== undefined) {
        const name = now.current.byId[tab].name;
        bringToFront(tab);
        // Where its tab draws no breadcrumb for it, there is no line to ask on: a dialog.
        if (!drawsCrumbFor.current(tab, session)) {
          setTaskEndInline(undefined);
          setRestartingTask({ session, name });
          return;
        }
        setTaskEndInline({
          session,
          where: "crumb",
          ...restartAsked(name),
        });
        return;
      }
      void askRestart(session);
    },
    [askRestart, askedByNow, bringToFront, restartListed, setRestartingTask],
  );
  /** Whether a chat has a Restart chat row: not a shell, which has no conversation. */
  const restartable = useCallback((session: number) => !shells.has(session), [shells]);
  /**
   * **Move to runner and Move here** (#2048): where each chat can move, by what the core said of
   * it as it started (its harness, the pool it runs on) and the pools this machine keeps. A
   * chat already moving is offered nothing until its move is over or put away.
   */
  useChatMovesHeard(plane);
  const chatMoves = useChatMoves(plane);
  const poolsHeld = usePools();
  const poolNames = useMemo(
    () => poolsToMoveTo(poolsHeld.read.state === "read" ? poolsHeld.read.shown : undefined),
    [poolsHeld],
  );
  const movesTo = useCallback(
    (session: number) => {
      const moving = chatMoves[session];
      if (moving !== undefined && moving.stage !== "failed") return [];
      return moveOffers(
        reopened.find((one) => one.session === session),
        poolNames,
      );
    },
    [chatMoves, poolNames, reopened],
  );
  /** Whether a chat here would be offered Pool setup to move it, with no pool kept (#4484). Only
   *  once the pools are read, so a slow read never offers setup to a machine that has pools. */
  const noPoolKept = poolsHeld.read.state === "read" && !anyPoolKept();
  const movesOnceSetUp = useCallback(
    (session: number) =>
      movesOnceAPoolIsSetUp(
        reopened.find((one) => one.session === session),
        !noPoolKept,
      ),
    [noPoolKept, reopened],
  );
  /** The other chats of a chat's workspace running where it runs, which move with it (#3453). */
  const movesWithChat = useCallback((session: number) => movesWith(session, reopened), [reopened]);
  /** Whether Move here brings back what the chat cloned on its runner (#3811). */
  const clonesComeBack = useCallback(
    (session: number) => reopened.find((one) => one.session === session)?.clones_come_back === true,
    [reopened],
  );
  const moveChat = useCallback(
    (session: number, to: string | null) => {
      const from = reopened.find((one) => one.session === session)?.pool ?? null;
      void askMove(plane, session, from, to);
    },
    [plane, reopened],
  );
  /**
   * **A prepared move switches at its chat's next pause** (#2048), as an owed restart does: the
   * chat then runs where it moved, in its place. The core asks the board again, so a turn is
   * never cut off; a switch that fails is said on the pane, and the chat runs where it was.
   */
  const movingNow = useRef(new Set<number>());
  useEffect(() => {
    const drive = () => {
      const states = chats.store.statesFor(chats.plane);
      for (const session of movesDue(chatMovesOf(plane), states)) {
        if (movingNow.current.has(session)) continue;
        movingNow.current.add(session);
        const moving = chatMovesOf(plane)[session];
        const failed = (why: string) => {
          const toldFailed = chatMovesOf(plane)[session];
          heardMove(plane, {
            session,
            from: moving?.from ?? null,
            to: moving?.to ?? null,
            stage: "failed",
            why,
            // The core tells this same failure too, in either order: its offer stands.
            limit:
              toldFailed?.stage === "failed" && toldFailed.why === why ? toldFailed.limit : null,
            with: moving?.with ?? [],
            sign_in: null,
          });
        };
        void commands
          .moveChatNow(plane, session, STARTING_SIZE.columns, STARTING_SIZE.rows)
          .then((done) => {
            if (done.status === "error") failed(done.error);
            else
              // Every chat of the move, each in its place (#3453).
              for (const one of done.data.moved) {
                movedAfterSignIn(plane, one.was, one.chat.session);
                forgetMove(plane, one.was);
                chatRestarted(one.was, one.chat);
                // What came back that a tool here runs, said on its tab (the person's ruling of
                // 2026-10-27). `?? []`, as a start's notices are read.
                const notes = one.notices ?? [];
                if (notes.length > 0)
                  setStartNotes((was) => ({
                    ...was,
                    [one.chat.session]: { notes, agentsMd: [] },
                  }));
              }
          })
          .catch((err: unknown) => failed(String(err)))
          .finally(() => movingNow.current.delete(session));
      }
    };
    drive();
    return chats.store.subscribe(drive);
  }, [chatMoves, chats, plane, chatRestarted]);
  /**
   * The chats open, as one word, for a read that follows it: those on the strip and those the
   * Chats list holds, so a task with no tab starting or ending moves it too (#1462), as it
   * moves the core's answer.
   */
  const chatsOpen = useMemo(() => {
    const open = new Set<number>();
    for (const tab of tabs.order) {
      const chat = chatOf(tabs, tab);
      if (chat !== undefined) open.add(chat);
    }
    for (const chat of sidebar?.unfiled ?? []) open.add(chat.session);
    for (const ws of sidebar?.workspaces ?? []) for (const chat of ws.chats) open.add(chat.session);
    return [...open].sort((a, b) => a - b).join(",");
  }, [sidebar, tabs]);
  /** Moves when one of the window's own sandbox commands returns: what those write is in the
   *  project's state folder, which the watcher of its root does not report (D-1428-10). */
  const sandboxCommands = useSandboxCommands();
  /**
   * The chats still running under an older sandbox than the project's settings decide now
   * (#1428), asked again when those settings change on disk, when the window changes them
   * itself, when its chats change, and when a chat comes to be owed a restart or stops being
   * (the core leaves out a chat that is owed one).
   */
  const olderSandbox = useOlderSandbox(
    plane,
    `${settingsChanges}:${sandboxCommands}:${chatsOpen}:${owedRestarts.join(",")}`,
    inFront,
  );
  const restartThem = useCallback(
    (sessions: readonly number[]) => {
      for (const session of sessions) void askRestart(session);
    },
    [askRestart],
  );

  /** A chat that started again on a press: listed, and its tab in front. */
  const showBack = useCallback(
    (chat: OpenChat) => {
      noteStarted(chat);
      showStarted(
        {
          session: chat.session,
          chat: chat.name,
          who: whoOf(chat.persona, chat.harness),
          label: chat.label,
        },
        filedFor(chat.cwd, focused),
      );
    },
    [focused, noteStarted, showStarted],
  );

  /** The waiting chats a Retry now is under way for, by id, so a second press starts nothing. */
  const retrying = useRef(new Set<string>());
  /**
   * **Retry now** on a chat this launch could not start (NO-3), by its id: the core starts it
   * again the way the launch tried to (`retry_chat_that_did_not_start`). Started, its tab opens
   * in front and its Notice goes; refused, the Notice stays and says the new reason.
   */
  const retryChat = useCallback(
    async (id: string) => {
      if (retrying.current.has(id)) return;
      retrying.current.add(id);
      const said = await commands
        .retryChatThatDidNotStart(plane, id, STARTING_SIZE.columns, STARTING_SIZE.rows)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      retrying.current.delete(id);
      if (said.status === "error") {
        // The refusal's words now, and what the core read with it: the approval its profile
        // needs, which the window takes from the record and never from the words (#1246).
        const now = await commands
          .chatsThatWouldNotStart(plane)
          .then((read) =>
            read.status === "ok" ? read.data.find((one) => one.id === id) : undefined,
          )
          .catch(() => undefined);
        setWouldNotStart((was) =>
          was.map((one) =>
            one.id === id ? { ...one, why: said.error, approval: now?.approval ?? null } : one,
          ),
        );
        return;
      }
      const chat = said.data;
      setWouldNotStart((was) => was.filter((one) => one.id !== id));
      showBack(chat);
    },
    [plane, showBack],
  );

  /**
   * **Reads what has become of the chats drawn as restoring** (`restoring.rs`): the ones that
   * have started are the chats now, with the notes on how each came back; one that did not
   * start loses its tab and is listed with the chats that did not, with Retry now.
   *
   * The whole list is read again each time, so an event this window missed is made up by the
   * next. `said` is what an event said of one chat: the number its tab was drawn under and the
   * session it started as.
   */
  const readRestored = useCallback(
    async (said: RestoreSaid | null) => {
      if (restoringNow.current.size === 0 && said === null) return;
      const answer = await commands.openedChats(plane).catch(() => undefined);
      if (answer?.status !== "ok") return;
      const open = answer.data ?? [];
      const { still, started, gone, arrived, renumbered, seen } = restoredSince(
        restoringNow.current,
        open,
        said,
        (session) => alreadyShows(now.current, session),
        restoreSeen.current,
      );
      restoreSeen.current = seen;
      if (
        started.length === 0 &&
        gone.length === 0 &&
        arrived.length === 0 &&
        renumbered.length === 0
      )
        return;
      restoringNow.current = still;
      setRestoringChats(still);
      for (const chat of started) {
        setReopened((was) => [...was.filter((one) => one.session !== chat.session), chat]);
        if (isShell(chat)) setShells((was) => new Set(was).add(chat.session));
      }
      // Filed where they were, and said again: the sidebar is read once more, now that the
      // core lists them among the open chats (the Chats section, the workspace's count).
      if (started.length > 0) setStartedIn((was) => ({ ...was }));
      // Started under another number than its tab was drawn under, which is still drawn, as an
      // event or the list says: the new number goes into that tab, as Restart's does, and the
      // tab keeps its pin, its place and its panes (#4962).
      for (const { from, chat } of renumbered) {
        noteStarted(chat);
        followRestart(from, chat.session);
        change((tabs) => replaceSession(tabs, from, chat.session));
      }
      // Started under another number, no pane showing its old one: it runs on in the sidebar,
      // and gets no tab it did not have (#4758).
      for (const chat of arrived) noteStarted(chat);
      if (gone.length > 0) {
        change((tabs) =>
          gone.reduce((left, one) => closeChat(left, one, filedIn, isPinned, keepsItsPane), tabs),
        );
        setReopened((was) => was.filter((one) => !gone.includes(one.session)));
        void commands
          .chatsThatWouldNotStart(plane)
          .then((trouble) => {
            if (trouble.status === "ok") setWouldNotStart(trouble.data ?? []);
          })
          .catch(() => undefined);
      }
      // Every chat has said how it came back: the dismissed notes are settled against them.
      if (still.size === 0) settleChatNotes(open);
    },
    [keepsItsPane, change, filedIn, followRestart, isPinned, noteStarted, plane, settleChatNotes],
  );
  const readRestoredNow = useRef(readRestored);
  useEffect(() => {
    readRestoredNow.current = readRestored;
  }, [readRestored]);
  // A project's Retry has come out (#5324): which chats still wait, and its rows, read again. Each
  // chat that came back is told on `chat-restored`.
  useEffect(() => {
    const listening = listen<ProjectReady>(PROJECT_READY, (event) => {
      if (event.payload.plane !== plane) return;
      void commands
        .chatsThatWouldNotStart(plane)
        .then((trouble) => {
          if (trouble.status === "ok") setWouldNotStart(trouble.data ?? []);
        })
        .catch(() => undefined);
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane]);

  // Each chat of the launch as it starts, or is found not to. Read once more as the listener
  // is heard, for a chat that started before it was.
  useEffect(() => {
    const listening = listen<ChatRestored>(CHAT_RESTORED, (event) => {
      if (event.payload.plane !== plane) return;
      void readRestoredNow.current(event.payload);
    })
      .then((stop) => {
        void readRestoredNow.current(null);
        return stop;
      })
      .catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane]);
  useEffect(() => {
    if (restoreLooks > 0) void readRestoredNow.current(null);
  }, [restoreLooks]);
  // And every few seconds while any is still restoring: a tab is never left saying so because
  // one event went to another window.
  const anyRestoring = restoringChats.size > 0;
  useEffect(() => {
    if (!anyRestoring) return;
    const looking = window.setInterval(
      () => void readRestoredNow.current(null),
      RESTORE_LOOKS_EVERY,
    );
    return () => window.clearInterval(looking);
  }, [anyRestoring]);

  /**
   * **Forget this chat…** asked from a waiting chat's Notice: the Notice and the Inbox's list it
   * stands in, found when the question opens (WebKit does not focus a pressed button, so the focus
   * cannot say where it was), and whether the answer was carried out (#1246).
   *
   * - **Forgotten:** the Notice has gone, so the keyboard goes on to the next Notice, or the strip.
   * - **Cancelled**, after a refusal too: back to the Notice's own Forget this chat….
   */
  const forgetAsked = useRef<{ notice?: Element; band: Element | null; forgot: boolean }>({
    band: null,
    forgot: false,
  });
  const askForget = useCallback((id: string, name: string) => {
    const notice = [...document.querySelectorAll("[data-cause]")].find(
      (one) => one.getAttribute("data-cause") === `chat-did-not-start:${id}`,
    );
    forgetAsked.current = {
      notice,
      band: notice?.closest('[data-scope="notice-list"]') ?? null,
      forgot: false,
    };
    setForgetting({ id, name, busy: false });
  }, []);
  const afterForgetAsk = useCallback(() => {
    const { notice, band, forgot } = forgetAsked.current;
    const back = [...(notice?.querySelectorAll<HTMLElement>("button") ?? [])].find(
      (button) => button.textContent === FORGET_THIS_CHAT,
    );
    if (!forgot && back?.isConnected) back.focus();
    else focusAfterNoticeGone(band, chatStrip.current);
  }, []);

  const forgetChat = useCallback(
    async (id: string, name: string) => {
      setForgetting({ id, name, busy: true });
      const said = await commands
        .forgetChatThatDidNotStart(plane, id)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") {
        setForgetting({ id, name, busy: false, trouble: said.error });
        return;
      }
      forgetAsked.current.forgot = true;
      setForgetting(undefined);
      setWouldNotStart((was) => was.filter((one) => one.id !== id));
    },
    [plane],
  );

  /**
   * **Review and approve…** on a waiting chat's Notice (#1246, D-1246-5): the question that
   * records the approval of its profile's command, with the picker's own sentence and line.
   *
   * - **Approved:** the approval is recorded and nothing starts. The Notice stays, without the
   *   offer, and the keyboard goes to its Retry now, which runs the whole start again.
   * - **Cancelled**, after a refusal too: nothing is recorded, and the keyboard goes back to
   *   Review and approve….
   */
  const approveAsked = useRef<{ id?: string; approved: boolean }>({ approved: false });
  const askApprove = useCallback((id: string, name: string, approval: NeedsApproval) => {
    approveAsked.current = { id, approved: false };
    setApproving({ id, name, approval, busy: false });
  }, []);
  const afterApproveAsk = useCallback(() => {
    const { id, approved } = approveAsked.current;
    const notice = [...document.querySelectorAll("[data-cause]")].find(
      (one) => one.getAttribute("data-cause") === `chat-did-not-start:${id}`,
    );
    const to = approved ? "Retry now" : REVIEW_AND_APPROVE;
    [...(notice?.querySelectorAll<HTMLElement>("button") ?? [])]
      .find((button) => button.textContent === to)
      ?.focus();
  }, []);
  const approveWaiting = useCallback(
    async (asked: { id: string; name: string; approval: NeedsApproval }) => {
      setApproving({ ...asked, busy: true });
      // The line on screen, which the core checks against the file again: a profile changed
      // since the question opened is refused there, and nothing is recorded.
      const said = await commands
        .approveProfile(plane, asked.approval.profile, asked.approval.shown)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") {
        setApproving({ ...asked, busy: false, trouble: said.error });
        return;
      }
      approveAsked.current.approved = true;
      setApproving(undefined);
      setWouldNotStart((was) =>
        was.map((one) => (one.id === asked.id ? { ...one, approval: null } : one)),
      );
      // A task drawn under the chat that asked (#1497): the core reads what its profile needs
      // again when it is next tried, so its row is read again and offers the try.
      rereadFinished();
    },
    [plane, rereadFinished],
  );
  /**
   * **What a task a launch could not start again offers on its row** (#1497): the same try and
   * the same approval the window's own "did not start" line has, and End task, which is the
   * one thing that ends it. The rows are the core's, so each is read again afterwards.
   */
  const waitingTaskWays = useMemo<WaitingTaskWays>(
    () => ({
      // **The one Retry** (#5324): the action the project's row on the rail runs. It brings
      // back every chat this project is short of, this one among them, whose tab then opens.
      retry: async (task) => {
        const retried = await windowDoes.retryProject(plane);
        rereadFinished();
        const session = retried?.started.find((one) => one.id === task.id)?.session;
        if (session === undefined) return;
        const open = await commands.openedChats(plane).catch(() => undefined);
        const chat =
          open?.status === "ok"
            ? (open.data ?? []).find((one) => one.session === session)
            : undefined;
        if (chat !== undefined) showBack(chat);
      },
      approve: (task, approval) => askApprove(task.id, task.name, approval),
      end: async (task) => {
        const said = await commands
          .endTaskThatDidNotStart(plane, task.id)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
        rereadFinished();
        return said.status === "error" ? said.error : undefined;
      },
    }),
    [askApprove, plane, rereadFinished, showBack, windowDoes],
  );

  const askStartFresh = useCallback(
    (session: number) => {
      setFreshening({
        session,
        name: nameOfNow.current(session),
        files: planeUpdates[session] ?? [],
        // Asked now, as a close asks `smart_close_offer`: the program ends either way, and
        // the question says so when that interrupts a turn (#1246).
        // `stateOf`, as the quit warning reads it: a shell nothing has reported on is `unknown`
        // too, since a harness started by hand in it could be mid-turn.
        doing: stateOf(chats.store.statesFor(chats.plane), session),
        // Asked by the quit warning's test, which counts its own work and a background turn
        // (#5782), not by its state alone.
        ownWork: ownWorkOf(chats.store.statesFor(chats.plane), session),
        backgroundTurn: chats.store.statesFor(chats.plane).backgroundTurns[session] === true,
        busy: false,
      });
    },
    [chats.plane, chats.store, planeUpdates],
  );
  /**
   * The core starts the same chat again, fresh (`start_chat_fresh`), and ends the old one itself
   * once the new one runs. **Only the pane that showed it changes** (D-NO3-8): the new session
   * takes that pane, so the tab keeps its place and a chat split beside it runs on, untouched.
   * Refused, nothing changes and the question says why.
   */
  const startFresh = useCallback(async () => {
    const asked = freshening;
    if (asked === undefined) return;
    setFreshening({ ...asked, busy: true, trouble: undefined });
    const said = await commands
      .startChatFresh(plane, asked.session, STARTING_SIZE.columns, STARTING_SIZE.rows)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    if (said.status === "error") {
      setFreshening({ ...asked, busy: false, trouble: said.error });
      return;
    }
    const chat = said.data;
    setFreshening(undefined);
    setReopened((was) => was.filter((one) => one.session !== asked.session));
    // Its tab keeps its pin: the tab is the same, its chat in a new session.
    noteStarted(chat);
    followRestart(asked.session, chat.session);
    change((tabs) => replaceSession(tabs, asked.session, chat.session));
  }, [change, followRestart, freshening, noteStarted, plane]);

  // A resumed chat whose harness ended without a word — it could not find the conversation — is
  // replaced by a fresh chat with the same record, once (SI-8d). Everything else a resumed chat
  // does only marks it heard, and a heard chat is the operator's from then on.
  //
  // Watched off the store rather than read in a render, so it is not a reason to draw this view.
  useEffect(() => {
    const watch = () => {
      const states = chats.store.statesFor(chats.plane);
      for (const [session, watched] of [...resuming.current]) {
        const state = stateOf(states, session);
        if (lostOnResume(watched, state)) {
          resuming.current.delete(session);
          const tab = now.current.order.find((id) =>
            panesOf(now.current, id).some((pane) => pane.session === session),
          );
          if (tab !== undefined) change((tabs) => closeTab(tabs, tab, filedIn, isPinned));
          void commands.closeSession(plane, session);
          setReopened((was) => was.filter((one) => one.session !== session));
          void resumeSession(watched.path, session);
        } else if (state === "done" || state === "failed") {
          resuming.current.delete(session);
        } else {
          resuming.current.set(session, heardFrom(watched, state));
        }
      }
    };
    watch();
    return chats.store.subscribe(watch);
  }, [change, chats, filedIn, isPinned, plane, resumeSession]);

  /**
   * **A shell or a one-job program whose program exited goes** (#4761; ADR 0062 and 0081 as
   * amended 2026-11-02). The core says it is `done` or `failed`, its program's exit, the one
   * cause read here. A shell's pane is left empty, the keyboard where it was; a one-job
   * program's split goes, the keyboard back to the pane it opened from. Then the core closes
   * it, so its row goes and the record no longer holds it. One already closed (End…) is not
   * listed, and nothing is done twice.
   *
   * Watched off the store rather than read in a render, as the resume watch above is.
   */
  useEffect(() => {
    const watch = () => {
      const states = chats.store.statesFor(chats.plane);
      const { shells: open, oneJobs: jobs } = shellsRef.current;
      for (const session of open) {
        const state = stateOf(states, session);
        if (state !== "done" && state !== "failed") continue;
        if (!listedNow.current.has(session) && !jobs.has(session)) continue;
        const job = jobs.has(session);
        const before = now.current;
        const next = change((tabs) =>
          job ? oneJobEnded(tabs, session, filedIn, isPinned) : shellExited(tabs, session, filedIn),
        );
        const typing = focusedChat(next);
        if (job && typing !== undefined && typing !== focusedChat(before))
          giveKeyboardTo(plane, typing);
        setShells((was) => withoutOne(was, session));
        if (job) setOneJobs((was) => withoutOne(was, session));
        void commands.closeSession(plane, session).catch(() => undefined);
      }
    };
    watch();
    return chats.store.subscribe(watch);
  }, [change, chats, filedIn, isPinned, plane]);

  /**
   * A chat's work link (V60, ADR 0088 §3). **What each chat works on is asked once per chat**,
   * as it appears, and then follows what this window links and unlinks: the link is the
   * project's, and the core's answer is what the tab and the pane draw.
   */
  const askedWorkItem = useRef(new Set<number>());
  useEffect(() => {
    for (const id of tabs.order) {
      const session = chatOf(tabs, id);
      if (session === undefined || askedWorkItem.current.has(session)) continue;
      askedWorkItem.current.add(session);
      void commands
        .chatWorkItem(plane, session)
        .then((said) => {
          const item = said.status === "ok" ? said.data : null;
          if (item !== null) setWorkItems((was) => ({ ...was, [session]: item }));
        })
        .catch(() => undefined);
    }
  }, [plane, tabs]);

  /** Opens the dialog that asks which work item a chat works on. */
  const linkWorkItem = useCallback((session: number) => {
    setLinkingWork({ session, busy: false });
  }, []);

  /** Links the dialog's chat to `key`, through `chat_work_link`; a refusal stays in the dialog. */
  const saveWorkLink = useCallback(
    async (session: number, key: string) => {
      setLinkingWork({ session, busy: true });
      const said = await commands
        .chatWorkLink(plane, session, key)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") {
        setLinkingWork({ session, busy: false, trouble: said.error });
        return;
      }
      setWorkItems((was) => ({ ...was, [session]: said.data }));
      setLinkingWork(undefined);
    },
    [plane],
  );

  /** The task whose brief is being read, while its panel is open (#1494). */
  const [briefOf, setBriefOf] = useState<BriefAsk>();
  /** Opens the Brief panel for a task: the one way in, for the catalogue's rows, a finished
   *  row and the breadcrumb's button ({@link OpenBrief}). It reads when it opens. */
  const openBrief: OpenBrief = setBriefOf;
  /** The task whose question is being answered, while its dialog is open (#1551). */
  const [answerOf, setAnswerOf] = useState<AnswerAsk>();
  /** Opens the Answer dialog for a task: for the catalogue's rows and a tab menu's line. */
  const openAnswer: OpenAnswer = setAnswerOf;
  /**
   * Opens **Ask {persona}** for chat `session`: the one way in, for the catalogue's rows and
   * for a Notice that names the persona to ask ({@link OpenAskPersona}). Nothing starts here.
   */
  const openAskPersona = useCallback<OpenAskPersona>((session, persona, prefill, from) => {
    setAskingPersona({ session, persona, prefill, from, busy: false });
  }, []);
  /**
   * Opens **Use skill…** for chat `session` (#2775): the one way in, for the catalogue's rows and
   * a pane's Skills chip ({@link OpenUseSkill}). It reads the chat's skills as it opens; nothing
   * is typed until one is picked.
   */
  const openUseSkill = useCallback<OpenUseSkill>(
    (session, picks) => {
      setUsingSkill({ session, picks, busy: false });
      readUseSkillOf(session);
    },
    [readUseSkillOf],
  );
  /**
   * **What the Skills view's cards act through, held still** (#4351): the view, kept once shown,
   * draws every skill's card, and draws them again only for something of its own. Opening a tab
   * moves `showView` and the restart's ask with the window around them; these call whichever is
   * current.
   */
  const skillsActs = useRef({ showView, openUseSkill, revealInExplorer, askRestart });
  useLayoutEffect(() => {
    skillsActs.current = { showView, openUseSkill, revealInExplorer, askRestart };
  });
  const skillsOpenView = useCallback<SkillsViewProps["onOpenView"]>(
    (view, title) => skillsActs.current.showView(view, title),
    [],
  );
  const skillsUse = useCallback<SkillsViewProps["onUseSkill"]>(
    (session, skill) => skillsActs.current.openUseSkill(session, skill),
    [],
  );
  const skillsShowInExplorer = useCallback<SkillsViewProps["onShowInExplorer"]>(
    (place) => skillsActs.current.revealInExplorer(place),
    [],
  );
  const skillsRestart = useCallback(
    (session: number) => skillsActs.current.askRestart(session),
    [],
  );
  /**
   * A skill picked in Use skill…: the core types it into the chat in its tool's own syntax and
   * sends nothing (`use_skill`), or copies it with why. Either way the dialog closes and the
   * window says what became of it; a refusal stays in the dialog.
   */
  const sendUseSkill = useCallback(
    async (skill: string, values: string[]) => {
      const using = usingSkill;
      if (using === undefined || using.busy) return;
      setUsingSkill({ ...using, trouble: undefined, busy: true });
      const said = await commands
        .useSkill(plane, using.session, skill, values)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      // Said only to the dialog still open on that session: a restart meanwhile moved it to the
      // chat's new one (`followRestart`, #6193), and the old run's answer is not its answer.
      const same = (was: typeof usingSkill) => was?.session === using.session;
      if (said.status === "error") {
        setUsingSkill((was) => (same(was) ? { ...using, trouble: said.error, busy: false } : was));
        return;
      }
      setUsingSkill((was) => (same(was) ? undefined : was));
      setReport({
        from: "use-skill",
        refused: false,
        words: handedSaid(said.data, nameOf(using.session)),
      });
    },
    [nameOf, plane, usingSkill],
  );
  /** Who already works where the dialog's task would, by the core's sentence (#1534). */
  const askingFrom = askingPersona?.session;
  const folderShared = useCallback(
    async (place: string | null): Promise<string | undefined> => {
      if (askingFrom === undefined) return undefined;
      const said = await commands.taskFolderShared(plane, askingFrom, place);
      return said.status === "ok" ? (said.data ?? undefined) : undefined;
    },
    [askingFrom, plane],
  );
  /**
   * The dialog's answer: the core starts a chat as that persona under the asking chat
   * (`ask_persona_chat`) and tells this window of it as it tells of any chat another chat
   * started, so its tab arrives behind the one being read. A refusal stays in the dialog.
   */
  const sendAsk = useCallback(
    async (name: string, ask: string, place: string | null) => {
      const asked = askingPersona;
      if (asked === undefined || asked.busy) return;
      setAskingPersona({ ...asked, trouble: undefined, busy: true });
      const said = await commands
        .askPersonaChat(
          plane,
          asked.session,
          asked.persona,
          name,
          ask,
          place,
          STARTING_SIZE.columns,
          STARTING_SIZE.rows,
        )
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") {
        // What was typed stays in the boxes: the dialog is still mounted, and holds it.
        setAskingPersona({ ...asked, trouble: said.error, busy: false });
        return;
      }
      setAskingPersona(undefined);
    },
    [askingPersona, plane],
  );

  /** Ends a chat's work link, through `chat_work_unlink`. */
  const unlinkWorkItem = useCallback(
    async (session: number): Promise<Ran> => {
      const said = await commands
        .chatWorkUnlink(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") return { ok: false, refused: said.error };
      setWorkItems((was) =>
        Object.fromEntries(Object.entries(was).filter(([one]) => Number(one) !== session)),
      );
      return { ok: true };
    },
    [plane],
  );

  /**
   * **A repo the workspace names, cloned from the window** (#1215): the explorer's row, its
   * Clone all, the bottom bar's menu and the palette all end here. The same `cloneRepos` a new
   * workspace's repos and Settings › Repos go through, so there is one clone path; each repo's
   * progress and failure is drawn on its own row from that store, and the panels are read
   * again so a repo that landed moves among the clones.
   */
  const cloneMissing = useCallback(
    async (workspace: string, repos: string[]): Promise<Ran> => {
      // Read again as each repo lands, so a repo that is cloned leaves "Not cloned here" while
      // the rest of a Clone all is still running. A clone the workspace does not then see is
      // a failure with a Retry — `cloneRepos` checks that for every caller (V91b).
      const ended = await cloneRepos(plane, workspace, repos, rereadPanels);
      // A clone the person stopped is what they asked for: no failure to report.
      const failed = ended.filter((one) => one.stopped !== true);
      if (failed.length > 0) return clonesRefused(workspace, failed);
      return ended.length === 0
        ? { ok: true, said: `Cloned ${repos.join(", ")} into ${workspace}.` }
        : { ok: true };
    },
    [plane, rereadPanels],
  );

  const doing = useMemo<Doing>(
    () => ({
      newChat,
      newTab,
      newShell,
      newBrowser,
      runAction,
      split,
      splitTab,
      closePane,
      tileAll: tileAllIn,
      moveToWindow,
      moveTo: moveFrom,
      zoomPane,
      undoLayout: undoLayoutIn,
      closeTab: closeFromItsControl,
      // Each as its own close (#4762): the chats asked about once, by the row, before this.
      closeOtherTabs: (closing) => {
        for (const tab of closing) close(tab);
      },
      selectTab: bringToFront,
      renameChat: beginRename,
      renameTab: beginTabRename,
      linkWorkItem,
      unlinkWorkItem,
      startFresh: askStartFresh,
      restartChat,
      moveChat,
      endChat,
      // A chat's Ask {persona}… and Use skill… open their dialogs: the one way in to each.
      askPersona: openAskPersona,
      useSkill: openUseSkill,
      showChatSkills: (session) => askChatSkills(plane, session),
      openStudioChat: (session) => {
        const link = studioLinks[session];
        if (link !== undefined) studioChats.openSkill(link);
      },
      pinTab,
      pinWorkspace,
      moveWorkspacePin,
      pinProject: windowDoes.pinProject,
      focusWorkspace,
      createWorkspace,
      removeWorkspace,
      // The verb is only ever a needs-you item's Go, or the palette's row for one.
      showChat: showNeeding,
      reopenTask: async (id) => {
        const why = await reopenById(id);
        return why === undefined ? { ok: true } : { ok: false, refused: why };
      },
      paneBack: goBack,
      ownTab,
      beside: besideOf,
      sendBack: sendBackOf,
      showChatTasks,
      showBrief: (session: number) =>
        openBrief({ chat: session, name: nameOfNow.current(session) }),
      answerQuestion: (session: number) =>
        openAnswer({ chat: session, name: nameOfNow.current(session) }),
      ignoreNeedsYou,
      cancelSmartClose,
      dismissStopped: (session: number) => stoppedFor(session, undefined),
      stopChat: askToStop,
      stopAllTasks: (session: number) => void askToStopAll(session),
      endTask: (session: number, way: TaskEndWay) => void askToEndTask(session, way, "elsewhere"),
      // The verb still names a persona — that is what the catalogue row is about — and the
      // window turns it into the row it opens. `charter/personas` is charter's own panel's
      // key (`purlis_core::panel::Panel::key`), and it is written here because the catalogue
      // is not a reader of the panel list.
      openView: showView,
      openViewBeside: showViewBeside,
      resumeSession: (path: string) => resumeSession(path),
      pickVault,
      createVault,
      ...edits.doing,
      ...memoryEdits.doing,
      // A chat's browser pinned or unpinned, handed to a chat, or stopped (#2022): the core
      // says where it cannot, and the listing it moves is read again on its event.
      pinBrowser: async (program: string, session: number, pinned: boolean): Promise<Ran> => {
        const said = await commands.pinChatBrowser(plane, session, program, pinned);
        return said.status === "error" ? { ok: false, refused: said.error } : { ok: true };
      },
      handBrowser: async (program: string, session: number): Promise<Ran> => {
        const said = await commands.handBrowserToChat(plane, program, session);
        return said.status === "error"
          ? { ok: false, refused: said.error }
          : {
              ok: true,
              said: `${nameOfNow.current(session)} uses it as browser ${said.data} from its next browser call.`,
            };
      },
      stopBrowser: async (program: string): Promise<Ran> => {
        const said = await commands.closeBrowser(program);
        return said.status === "error" ? { ok: false, refused: said.error } : { ok: true };
      },
      // A chat's browser timeline, in a tab of its own (#2017), kept by the chat's id.
      openBrowserTimeline: (chat: string, name: string) =>
        showView(browserTimelineView(chat), browserTimelineTitle(name)),
      removeWorktree,
      mergeWorktree,
      declareWorktreeDone,
      // A clone picked from its menu is the explorer's own pick one level up: the same state,
      // so the explorer marks it and `New tab` starts there (charter-app#174).
      pickClone: (repo, path) => pickSpot({ repo, path }),
      // The explorer becomes that branch's cockpit (FM-5).
      focusBranch: (cut) =>
        setFocusedBranch({ workspace: cut.workspace, repo: cut.repo, piece: cut.piece }),
      // A clone's own folder, the same cockpit with no Merge or Done (#1152).
      focusRepo: (repo: string) => {
        if (ofWorkspace !== undefined)
          setFocusedBranch({ workspace: ofWorkspace, repo, piece: null });
      },
      newBranch,
      cloneMissing,
      askDropMembership: (workspace: string, repo: string) => setMembershipAsk({ workspace, repo }),
      forgetFailedClone: (workspace: string, repo: string) =>
        forgetFailedClone(plane, workspace, repo),
      newChatIn,
      sendKey,
      openProject: windowDoes.openProject,
      createProject: windowDoes.createProject,
      showExtensions: windowDoes.showExtensions,
      toggleFocusMode: windowDoes.toggleFocusMode,
      // A region's row in focus mode leaves it first (D-2921-19).
      showSideView: showSideViewSeen,
      editVision: (workspace: string) => setAddingVision({ workspace, busy: false }),
      askWorkspaceWrite: workspaceWrites.ask,
      toggleRegion: toggleRegionSeen,
      moveView: moveSideView,
      // F6 and Shift+F6 from the palette (#1751): once the palette has closed and given the
      // keyboard back, so the step starts from where the person was.
      // From where the palette hands the keyboard back, not from the palette (#1769).
      focusRegion: (back: boolean) =>
        onceTheKeyboardIsBack(() => requestAnimationFrame(() => stepRegions(back))),
      installCli: windowDoes.installCli,
      selectProject: windowDoes.selectProject,
      switchProject: windowDoes.switchProject,
      closeProject: windowDoes.closeProject,
      moveProject: windowDoes.moveProject,
      openSettings: windowDoes.openSettings,
      openSaving: windowDoes.openSaving,
      openPoolSetup: windowDoes.openPoolSetup,
      openWorkspaceSettings,
      switchLive: (workspace: string) => setLiveAsk(workspace),
      renameWorkspace,
      openSettingsTab: windowDoes.openSettingsTab,
      openYourSettings: windowDoes.openYourSettings,
      readAgain: rereadPanels,
      taskBranch: (id: string, act: "merge" | "discard") =>
        void (act === "merge"
          ? branchActs.current.askToMerge(id)
          : branchActs.current.askToDiscard(id)),
      curate,
      quit: windowDoes.quit,
      ...fileDoing,
      shellInFolder,
      startChatHere,
      // The picker of this project's chats, for that row's path (#1151): the pick hands it on.
      addToChat: (rows: readonly FileRef[]) =>
        setAddingToChat(rows.map(({ at, folder }) => ({ plane, ...at, folder }))),
    }),
    [
      studioChats,
      workspaceWrites.ask,
      studioLinks,
      beginRename,
      beginTabRename,
      showSideViewSeen,
      toggleRegionSeen,
      moveSideView,
      restartChat,
      moveChat,
      endChat,
      linkWorkItem,
      unlinkWorkItem,
      askStartFresh,
      openAskPersona,
      openUseSkill,
      askToStop,
      askToStopAll,
      askToEndTask,
      bringToFront,
      cancelSmartClose,
      close,
      closeFromItsControl,
      splitTab,
      closePane,
      tileAllIn,
      moveToWindow,
      moveFrom,
      undoLayoutIn,
      zoomPane,
      curate,
      createVault,
      createWorkspace,
      edits.doing,
      memoryEdits.doing,
      fileDoing,
      shellInFolder,
      startChatHere,
      plane,
      focusWorkspace,
      ofWorkspace,
      ignoreNeedsYou,
      mergeWorktree,
      newBranch,
      cloneMissing,
      declareWorktreeDone,
      newChat,
      newTab,
      newShell,
      newBrowser,
      pickVault,
      newChatIn,
      openAnswer,
      openBrief,
      openWorkspaceSettings,
      pickSpot,
      pinTab,
      pinWorkspace,
      moveWorkspacePin,
      rereadPanels,
      removeWorkspace,
      removeWorktree,
      renameWorkspace,
      resumeSession,
      runAction,
      sendKey,
      showNeeding,
      reopenById,
      goBack,
      ownTab,
      besideOf,
      sendBackOf,
      showChatTasks,
      showView,
      showViewBeside,
      split,
      stoppedFor,
      windowDoes,
    ],
  );

  // The chats that can be waiting on the operator without saying so. Read from the sidebar,
  // which is the core's own list of what is open and what each chat runs. It is needed up
  // here as well as in the title bar's list: the palette's row for the queue must not claim
  // "Nothing needs you." over the top of a chat that cannot say it does (charter-app#52).
  //
  // Held, because it is one of the catalogue's inputs: a fresh array on every render would
  // rebuild all 117 rows of a fifty-chat catalogue for every keystroke in the palette.
  const quietHere = useChatsSelect(
    chats,
    (states) =>
      sidebar
        ? quietChats(
            [...sidebar.workspaces.flatMap((ws) => ws.chats), ...sidebar.unfiled],
            states,
            // Named as its row names it (`shownName`, #1484): a task with no tab is never
            // its number here either.
            (chat) => shownName(tabs, chat),
          )
        : [],
    sameQuiet,
  );
  /** The same, by name, for the catalogue's queue row. */
  const quiet = useMemo(() => quietHere.map((chat) => chat.name), [quietHere]);

  /** Each open chat's harness card, at a glance, by session (HP-19): what its header draws. The
   *  core's list of open chats carries it, so a chat put back at a launch has it from its
   *  first frame. */
  const glances = useMemo(() => {
    const out: Record<number, HarnessGlance> = {};
    const open = sidebar
      ? [...reopened, ...sidebar.workspaces.flatMap((ws) => ws.chats), ...sidebar.unfiled]
      : reopened;
    for (const chat of open) if (chat.card) out[chat.session] = chat.card;
    return out;
  }, [sidebar, reopened]);

  /** The chats working in the focused workspace, which is what the explorer files under the
   *  spots they are working at. The plane's own answer, like everything else about where a
   *  chat is: nothing on the plane records a chat, so the directory it works in is it. */
  const workspaceChats = useMemo(
    () =>
      (sidebar?.workspaces.find((ws) => ws.name === ofWorkspace)?.chats ?? []).map((chat) => ({
        // Named as the Chats list names it (`shownName`, #1484), never by the harness's own
        // name, which is a number (charter-app#254): a task with no tab too.
        ...chat,
        name: shownName(tabs, chat),
      })),
    [ofWorkspace, sidebar, tabs],
  );

  /**
   * Every running chat of the project, as the Chats section lists it (#1447): the core's own
   * list, each named as its tab is, or as its tab would be for a task chat that has none yet.
   * Whether it has a tab is this window's answer, since the tabs are this window's.
   */
  const listedChats = useMemo(() => {
    if (sidebar === undefined) return [];
    const one = (chat: OpenChat, workspace: string) =>
      listedChat(
        withPretendedLimits(chat, pretendedLimits),
        workspace,
        shownName(tabs, chat),
        tabHolding(tabs, chat.session) !== undefined,
        standingOf(questions, chat.session),
      );
    // By number, which is the order they were started in: the list arrives workspace by
    // workspace, and a chat's children are read in the order it asked for them.
    return (
      [
        ...sidebar.workspaces.flatMap((ws) => ws.chats.map((chat) => one(chat, ws.name))),
        ...sidebar.unfiled.map((chat) => one(chat, ROOT_WORD)),
        // Tasks a scenario spec pretends a session has (`e2eTasks.ts`): none in a shipped build.
        ...pretended.map(({ chat, workspace }) => one(chat, workspace)),
      ]
        // **A one-job program has no row** (#4761): it is drawn beside the pane it opened from,
        // and goes when it exits.
        .filter((chat) => !oneJobs.has(chat.session))
        .sort((a, b) => a.session - b.session)
    );
  }, [oneJobs, pretended, pretendedLimits, questions, sidebar, tabs]);
  /** The chats of each tab, as its chip counts them and its menu lists them (#1487). */
  const chatsByTab = useMemo(() => chatsOfTabs(tabs, listedChats), [listedChats, tabs]);
  /** **What each chat's pane head says of it** (#4760): the chat as the Chats list lists it,
   *  its sandbox, whether it wraps up, and, in the tab in front, whether it is filed in another
   *  workspace than that tab's (#4982), whose badge its head wears. */
  const paneHeads = useMemo(() => {
    const byNumber = new Map(listedChats.map((chat) => [chat.session, chat]));
    const front = tabs.inFront;
    return (session: number): ChatHead | undefined => {
      const chat = byNumber.get(session);
      if (chat === undefined) return undefined;
      const elsewhere =
        front === undefined ? undefined : filedElsewhere(tabs, front, session, filedIn);
      return {
        chat,
        sandbox: sandboxOf(session),
        wrapping: wrapping.has(session),
        // Named and coloured by the one answer, `filedIn`'s, so the two never disagree.
        ...(elsewhere === undefined
          ? {}
          : {
              elsewhere: {
                workspace: elsewhere === OUTSIDE ? ROOT_WORD : elsewhere,
                colour: colourOf(elsewhere),
              },
            }),
      };
    };
  }, [colourOf, filedIn, listedChats, sandboxOf, tabs, wrapping]);
  /** Each tab menu's footer: how many of its session's tasks run against its limit (#1498). */
  useEffect(() => {
    chatsListed.current = listedChats;
  }, [listedChats]);
  const chatRows = useMemo(() => chatsTree(listedChats), [listedChats]);
  /** The Chats list's rows: a task that ended keeps its row until its finished row is read,
   *  so the rows below it move once and not up and then down (`useRowsUntilRead`). */
  const listRows = useRowsUntilRead(chatRows, finishedSettled);
  /** Each persona's chats, by number (#3341): what a persona's row in the Personas view reads
   *  its state from. */
  const personaChats = useMemo(() => {
    const of = new Map<string, number[]>();
    for (const row of listRows)
      if (row.persona !== null) of.set(row.persona, [...(of.get(row.persona) ?? []), row.session]);
    return of;
  }, [listRows]);
  /** The personas each workspace's chats ran as (#3483), kept after a chat closes (#3566): the
   *  Personas view's In this workspace, with the ones the workspace's session records name. */
  const chatPersonasIn = useWorkedHere(plane, listRows);
  /**
   * **How long each chat has been in its state, as this window saw it** (`stateClock.ts`): one
   * clock for the project, held here so it runs whether or not the Chats list is drawn, and
   * read by that list's rows and by a tab's menu alike (#1487). The list says which rows are
   * on screen, which is what the clock calls a change somebody saw.
   */
  const chatClock = useMemo(() => {
    void plane;
    return stateClock();
  }, [plane]);
  const rowsOnScreen = useRef<ReadonlySet<number>>(NO_ROWS_DRAWN);
  const rowsDrawn = useCallback((drawn: ReadonlySet<number>) => {
    rowsOnScreen.current = drawn;
  }, []);
  const { store: chatStore, plane: chatsOf } = chats;
  useEffect(() => {
    const read = () =>
      chatClock.read(chatStore.statesFor(chatsOf), chatRows, Date.now(), rowsOnScreen.current);
    read();
    return chatStore.subscribe(read);
  }, [chatClock, chatRows, chatStore, chatsOf]);
  /**
   * **Each session's tasks** (#1491): the open tasks below it and the finished rows under it,
   * from the two lists the Chats section draws. Lent to everything this view draws
   * (`TasksBelowLent`), so a session's row, its state's word and its tab count the same rows.
   */
  const tasksBelow = useMemo(() => {
    const limits = new Map(listedChats.map((chat) => [chat.session, chat.tasksLimit ?? null]));
    return tasksBelowOf(listedChats, finishedTasks, (session) => limits.get(session) ?? null);
  }, [finishedTasks, listedChats]);
  /**
   * **The breadcrumb of every pane that shows a task and can be drawn**, by tab, pane and
   * chat (#1486): the path read off the core's list, for the panes `shownLive` says are live.
   */
  const liveCrumbs = useMemo(() => {
    const live = new Map<string, Crumbs>();
    if (sidebar === undefined) return live;
    const open = (session: number) => chatsByNumber.has(session);
    for (const id of tabs.order) {
      // **A task a press put in a pane, with its path** (#4757): remembered like a shown one,
      // so the pane can still say it once the task has ended.
      for (const [pane, said] of Object.entries(placedCrumbsOf(tabs, id, listedChats))) {
        const last = said.path[said.path.length - 1];
        if (last.mode === "task") live.set(`${id}:${pane}:${last.session}`, said);
      }
      if (tabs.byId[id].shows === undefined) continue;
      const crumbs = crumbsOf(tabs, id, listedChats);
      for (const one of shownLive(tabs, id, askedBy, open)) {
        const said = crumbs[one.pane];
        if (one.session !== one.own && one.live && said !== undefined)
          live.set(`${id}:${one.pane}:${one.session}`, said);
      }
    }
    return live;
  }, [askedBy, chatsByNumber, listedChats, sidebar, tabs]);
  // What each pane shows of a task is remembered while it is live, and forgotten with the
  // pane's showing of it: a pane that left a task remembers nothing of it. Adjusted while
  // rendering, as React has state follow what it is drawn from.
  const recalled = useMemo(
    () => recalledCrumbs(remembered, liveCrumbs, tabs),
    [liveCrumbs, remembered, tabs],
  );
  if (recalled !== remembered) setRemembered(recalled);
  // **A task the record put back that is not there to be shown**, and that no pane ever drew:
  // its tab shows the session's own chat (#1486). This is the one place a pane goes back by
  // itself, and nobody was looking at it: a launch, before the first frame of that task.
  useEffect(() => {
    if (sidebar === undefined) return;
    const open = (session: number) => chatsByNumber.has(session);
    for (const id of now.current.order) {
      for (const one of shownLive(now.current, id, askedBy, open)) {
        if (one.live || recalled.has(`${id}:${one.pane}:${one.session}`)) continue;
        if (liveCrumbs.has(`${id}:${one.pane}:${one.session}`)) continue;
        change((tabs) => showOwnIn(tabs, id, one.pane));
      }
    }
    // `tabs` is read through `now`, and is here so a change to it is judged.
  }, [askedBy, change, chatsByNumber, liveCrumbs, recalled, sidebar, tabs]);
  /**
   * **The breadcrumb of each pane of the tab in front that shows a task** (#1486): a live task
   * has its path off the list. **A task that has ended** keeps its pane, with the path it last
   * had (#4757), and its last screen stays, read-only, under the ended band (#4756). One the list
   * has not been read for yet is drawn as nothing.
   */
  const { frontCrumbs, frontEndedPanes, frontUnread } = useMemo(() => {
    const crumbs: Record<number, Crumbs> = {};
    const ended: Record<number, Crumbs> = {};
    const unread = new Set<number>();
    const id = tabs.inFront;
    if (id === undefined)
      return { frontCrumbs: crumbs, frontEndedPanes: ended, frontUnread: unread };
    for (const one of frontShown) {
      const key = `${id}:${one.pane}:${one.session}`;
      const last = recalled.get(key);
      const gone = sidebar !== undefined && !chatsByNumber.has(one.session);
      if (one.session === one.own) {
        // A task a press put in this pane, ended (#4757).
        if (gone && last !== undefined) ended[one.pane] = last;
        continue;
      }
      const live = one.live ? liveCrumbs.get(key) : undefined;
      if (live !== undefined) crumbs[one.pane] = live;
      else if (sidebar === undefined || last === undefined) unread.add(one.pane);
      else if (gone) ended[one.pane] = last;
      // Still running, and no longer below the session whose tab shows it (a record from
      // before layouts were kept): drawn as the chat it is, with the path it last had.
      else crumbs[one.pane] = last;
    }
    return { frontCrumbs: crumbs, frontEndedPanes: ended, frontUnread: unread };
  }, [chatsByNumber, frontShown, liveCrumbs, recalled, sidebar, tabs.inFront]);
  /** **The chat each ended task a pane shows was asked by** (#5366), by the task: the last but
   *  one of the path its pane last had. */
  const endedAskers = useMemo(() => {
    const askers = new Map<number, number>();
    // **In every tab, not the front one's alone** (#5543): the palette lists a task a pane
    // behind still shows, and the path each pane last had is remembered for every tab.
    // Until the list is read every chat would look gone, so none is taken for ended yet.
    if (sidebar === undefined) return askers;
    for (const said of recalled.values()) {
      const [asker, task] = said.path.slice(-2);
      if (task !== undefined && !chatsByNumber.has(task.session))
        askers.set(task.session, asker.session);
    }
    return askers;
  }, [chatsByNumber, recalled, sidebar]);
  /**
   * **The finished row of each task a pane was left on when it ended**, by pane (#1485 under
   * #1486's ended view): its report is drawn on the pane, and the pane says how it ended in
   * the row's words. Found by the number the task's chat had, and never by its name: a pane
   * with no row of its own draws no report.
   */
  const frontFinished = useMemo(() => {
    const rows: Record<number, FinishedTask> = {};
    for (const [pane, gone] of Object.entries(frontEndedPanes)) {
      const [asker, task] = gone.path.slice(-2);
      const row =
        task === undefined ? undefined : finishedOf(finishedTasks, asker.session, task.session);
      if (row !== undefined) rows[Number(pane)] = row;
    }
    return rows;
  }, [finishedTasks, frontEndedPanes]);
  /**
   * **Resume, on the band of a chat ended on purpose** (#5019, spec #4750 story 62), in the pane
   * that showed it: from its record where a Smart close wrote one (SI-8d), and else on its
   * conversation, the same chat in a new run (`resume_ended_chat`). A refusal is said where
   * every refused press is, and the band stays.
   */
  const resumeEndedIn = useCallback(
    async (session: number) => {
      const was = endedHereNow.current.get(session);
      if (was === undefined) return;
      if (was.record !== undefined) {
        const ran = await resumeSession(was.record.path, undefined, session);
        if (!ran.ok) setReport({ from: `resume:${session}`, refused: true, words: ran.refused });
        return;
      }
      const said = await commands
        .resumeEndedChat(plane, session, STARTING_SIZE.columns, STARTING_SIZE.rows)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (said.status === "error") {
        setReport({ from: `resume:${session}`, refused: true, words: said.error });
        return;
      }
      const chat = said.data;
      noteStarted(chat);
      forgetEndedHere(session);
      change((tabs) => replaceSession(tabs, session, chat.session));
    },
    [change, forgetEndedHere, noteStarted, plane, resumeSession],
  );
  /**
   * **The breadcrumb of each pane of the tab in front that shows its own chat and has a path
   * to say** (#1489): a task in a pane of its own, and the session it is beside.
   */
  const frontPlaced = useMemo(
    () => (tabs.inFront === undefined ? {} : placedCrumbsOf(tabs, tabs.inFront, listedChats)),
    [listedChats, tabs],
  );
  useEffect(() => {
    drawsCrumbFor.current = (tab, session) => {
      const endsInIt = (crumbs: Crumbs) => {
        const last = crumbs.path[crumbs.path.length - 1];
        return last.session === session && last.mode === "task";
      };
      const at = now.current;
      return (
        Object.values(crumbsOf(at, tab, listedChats)).some(endsInIt) ||
        Object.values(placedCrumbsOf(at, tab, listedChats)).some(endsInIt)
      );
    };
  }, [listedChats]);
  // A pane that could not draw its task and can again (a restart caught up) gives the
  // keyboard back to the task's terminal, where the pane has the keyboard.
  const wasAway = useRef<ReadonlySet<number>>(new Set());
  useEffect(() => {
    const front = tabs.inFront === undefined ? undefined : tabs.byId[tabs.inFront];
    const away = new Set([...frontUnread, ...Object.keys(frontEndedPanes).map(Number)]);
    for (const one of frontShown)
      if (
        wasAway.current.has(one.pane) &&
        !away.has(one.pane) &&
        one.session !== one.own &&
        front?.focused === one.pane
      )
        giveKeyboardTo(plane, one.session);
    wasAway.current = away;
  }, [frontEndedPanes, frontShown, frontUnread, plane, tabs]);

  /**
   * **The chats that live in each pane of the tab in front, other than the one it shows**
   * (#1486, the train-1 part of V100-56), by pane: the session's own chat while a task is
   * shown, and every task while it is not. Each says what it has to say on this pane, with
   * whose it is first, since nothing that waits for the person may be off screen.
   */
  const frontOthers = useMemo(() => {
    const others: Record<number, { session: number; whose: string; task: boolean }[]> = {};
    const id = tabs.inFront;
    if (id === undefined) return others;
    for (const one of frontShown) {
      const [own, ...tasks] = chatsOfPane(tabs, id, one.pane, listedChats);
      if (own === undefined) continue;
      others[one.pane] = [own, ...tasks]
        .filter((chat) => chat.session !== one.session || !one.live)
        .map((chat) => ({
          session: chat.session,
          // The whole path, from the core's record of who asked whom (#1508, V100-56).
          whose: whoseOf(chat, own, listedChats),
          task: chat.session !== own.session,
        }));
    }
    return others;
  }, [frontShown, listedChats, tabs]);

  /**
   * **The questions each pane of the tab in front asks for several of its tasks at once**
   * (#1508, V100-57), by pane: tasks of the pane's session blocked on the same host or folder,
   * asked about in one Notice. Their blocks are taken from the tasks' own Notices
   * (`frontBlocks`), so each is asked once.
   */
  const frontGroups = useMemo(() => {
    const groups: Record<number, TaskBlockGroup[]> = {};
    const id = tabs.inFront;
    if (id === undefined) return groups;
    for (const one of frontShown) {
      const [own, ...tasks] = chatsOfPane(tabs, id, one.pane, listedChats);
      if (own === undefined) continue;
      groups[one.pane] = taskBlockGroups(
        sandboxBlocks,
        own.session,
        tasks.map((chat) => ({ session: chat.session, whose: whoseOf(chat, own, listedChats) })),
      );
    }
    return groups;
  }, [frontShown, listedChats, sandboxBlocks, tabs]);
  const frontBlocks = useMemo(
    () => withoutGrouped(sandboxBlocks, Object.values(frontGroups).flat()),
    [frontGroups, sandboxBlocks],
  );
  /** What each pane of the tab in front asks for its tasks together, and what was answered. */
  const frontAsked = useMemo(() => {
    const asked: Record<number, ReactNode> = {};
    for (const one of frontShown) {
      const groups = frontGroups[one.pane] ?? [];
      const answered = taskBlocksAnswered.filter((said) => said.session === one.own);
      if (groups.length === 0 && answered.length === 0) continue;
      asked[one.pane] = (
        <>
          {groups.map((group) => (
            <TaskBlocksNotice
              key={group.key}
              plane={plane}
              group={group}
              asks={waiting}
              onAnswered={(members, said) => {
                for (const member of members) {
                  blockAnswered(member.session, member.block);
                  oweRestart(member.session);
                }
                setTaskBlocksAnswered((was) => [
                  ...was,
                  { at: Date.now(), session: group.session, target: group.target, said },
                ]);
              }}
              onKeepBlocked={(members) => {
                for (const member of members) blockAnswered(member.session, member.block);
              }}
            />
          ))}
          {answered.map((said) => (
            <TaskBlocksAnswered
              key={said.at}
              target={said.target}
              said={said.said}
              onDismiss={() =>
                setTaskBlocksAnswered((was) => was.filter((one) => one.at !== said.at))
              }
            />
          ))}
        </>
      );
    }
    return asked;
  }, [blockAnswered, frontGroups, frontShown, oweRestart, plane, taskBlocksAnswered, waiting]);

  /** Every chat that lives in a tab of this window: a session's own chat, and each task
   *  below one. What a tab can be wearing the hand for. */
  const living = useMemo(() => {
    const byTask = askedByOf(listedChats);
    return listedChats
      .filter((chat) => homeOf(tabs, chat.session, byTask) !== undefined)
      .map((chat) => chat.session);
  }, [listedChats, tabs]);
  const heldFor = useHeldAmong(plane, living);
  /**
   * **This project's permission prompts held on their chats' hooks** (HP-6): a chat whose
   * harness asked the person's permission waits on them from the moment it asks, before its
   * harness says anything else, so it wears the hand at once (reported 2026-10-09: a task
   * stopped on one in a tab nobody was at, and nothing said so).
   */
  const planeOnly = useMemo(() => [plane], [plane]);
  const asksHere = usePermissionAsks(planeOnly)[plane] ?? NO_ASKS;
  const refusedFor = useRefusedAmong(plane, living);
  /**
   * **The chats waiting for the person beside the queue**: every chat with a Notice that waits
   * for an answer (#1486), a dispatch held for a grant, a vault it was refused, a sandbox block,
   * a restart that is owed or did not happen. A held dispatch is not in the core's queue, so
   * without this a chat stuck on one would say nothing anywhere. The queue goes first, where it
   * is read (`hiddenIn`).
   */
  const othersWaiting = useMemo(() => {
    const restarts = Object.entries(restartsSaid)
      .filter(([, said]) => said.trouble !== undefined || said.notYet !== undefined || said.byHand)
      .map(([session]) => Number(session));
    const blocked = Object.entries(sandboxBlocks)
      .filter(([, blocks]) => blocks.length > 0)
      .map(([session]) => Number(session));
    const prompted = asksHere.map((ask) => ask.session);
    // A task question that has reached the person (#4564); one with its asking chat is not.
    const reached = waitingOnYou(questions);
    return [...prompted, ...reached, ...heldFor, ...refusedFor, ...blocked, ...restarts];
  }, [asksHere, heldFor, questions, refusedFor, restartsSaid, sandboxBlocks]);
  /**
   * **Tab `id`'s chats that are waiting for the person and are not on screen**, by name, the
   * longest waiting first (#1486, V100-37): what the tab wears the hand for. A chat is on
   * screen only in the tab in front. Nothing switches a tab to one of them; going to it does.
   * Asked with the queue where the chip is drawn (`QueueRead`, #1034).
   */
  const hiddenIn = useCallback(
    (queue: readonly number[], id: number): readonly Needing[] => {
      if (!(id in tabs.byId)) return NO_NEEDS;
      const waitingForYou = [...new Set([...queue, ...othersWaiting])];
      const hidden = hiddenNeeding(tabs, id, askedBy, waitingForYou).flatMap(
        (session): Needing[] => {
          const chat = chatsByNumber.get(session);
          return chat === undefined ? [] : [{ session, name: shownName(tabs, chat) }];
        },
      );
      return hidden.length === 0 ? NO_NEEDS : hidden;
    },
    [askedBy, chatsByNumber, othersWaiting, tabs],
  );
  /**
   * **Each tab's tasks that have ended and still have a line** (#1487): they are not in the
   * list of open chats, so they are handed to the tab's chip beside its rows.
   *
   * - **The finished rows of every chat of the tab**, at every level, since a task's own tasks
   *   finish under it (`finished.ts`, #1485): each as its row in the Chats list says it, and
   *   folding as the core says it folds, and counted by how the core says it ended.
   * - **The task the tab still shows**, which stays on screen until the person goes back
   *   (`shownLive`, the ended band): the same line as its finished row where it has one, found as
   *   the pane finds it (`finishedOf`), and a line of its own until that row is read.
   */
  const endedByTab = useMemo(() => {
    const ended = new Map<number, Ended[]>();
    if (sidebar === undefined) return ended;
    const open = (session: number) => chatsByNumber.has(session);
    for (const id of tabs.order) {
      const lines: Ended[] = [];
      let home: string | undefined;
      for (const row of chatsByTab.get(id) ?? NO_ROWS) {
        if (row.level === 1) home = row.workspace;
        // What finished under a task in a tab of its own is that tab's to list (#1489).
        if (row.placed === "tab") continue;
        for (const task of finishedTasks.get(row.session) ?? []) {
          const shown = shownOf(task);
          if (shown === undefined) continue;
          // The core's word for the root and this window's, as one.
          const place = task.place === CORE_ROOT_WORD ? ROOT_WORD : task.place;
          lines.push({
            key: `finished:${task.id}`,
            asker: task.asker,
            name: task.name,
            persona: task.persona,
            shown,
            qualifier: qualifierOf(task),
            folds: task.folds,
            bucket: finishedBucketOf(task.how),
            elsewhere: place === home ? null : place,
            report: task.report,
          });
        }
      }
      if (tabs.byId[id].shows !== undefined)
        for (const one of shownLive(tabs, id, askedBy, open)) {
          if (one.live || one.session === one.own || open(one.session)) continue;
          const last = recalled.get(`${id}:${one.pane}:${one.session}`);
          if (last === undefined) continue;
          const task = last.path[last.path.length - 1];
          const asker = last.path[last.path.length - 2]?.session ?? one.own;
          const row = finishedOf(finishedTasks, asker, one.session);
          const at =
            row === undefined ? -1 : lines.findIndex((line) => line.key === `finished:${row.id}`);
          if (at >= 0) {
            lines[at] = { ...lines[at], session: one.session };
            continue;
          }
          const shown = endedState(last);
          if (shown === undefined) continue;
          lines.push({
            key: `ended:${one.session}`,
            session: one.session,
            asker,
            name: task.name,
            persona: task.persona,
            shown,
            folds: shown.kind === "done" || shown.kind === "cancelled",
            bucket: taskBucketOf(shown.kind),
            elsewhere: last.elsewhere,
          });
        }
      if (lines.length > 0) ended.set(id, lines);
    }
    return ended;
  }, [askedBy, chatsByNumber, chatsByTab, finishedTasks, recalled, sidebar, tabs]);
  /** **How many of tab `id`'s tasks are open** (#5341): what its hover says, the count it no
   *  longer wears. */
  const tasksRunningOf = (id: number) =>
    (chatsByTab.get(id) ?? NO_ROWS).filter((row) => row.level > 1).length;
  /** **The tab in front, if its session has tasks to explain the chip by** (#1501): what the
   *  session's pane draws the first dispatch's Notice from (`ChipExplained`). Not a task's tab,
   *  since the session is what dispatched. */
  const frontTabId = frontTab?.id;
  const frontOwn = frontTabId === undefined ? undefined : chatOf(tabs, frontTabId);
  const frontRows = frontTabId === undefined ? undefined : chatsByTab.get(frontTabId);
  const frontEnded = frontTabId === undefined ? undefined : endedByTab.get(frontTabId);
  const frontIsTask = frontOwn !== undefined && chatsByNumber.get(frontOwn)?.from?.task === true;
  const chipToExplain = useMemo((): ChipToExplain | undefined => {
    if (frontTabId === undefined || frontOwn === undefined || frontIsTask) return undefined;
    const tasks =
      (frontRows ?? NO_ROWS).filter((row) => row.level > 1).length + (frontEnded?.length ?? 0);
    return tasks === 0
      ? undefined
      : { session: frontOwn, tasks, onShow: () => showChatTasks(frontOwn) };
  }, [frontEnded, frontIsTask, frontOwn, frontRows, frontTabId, showChatTasks]);
  /** Goes to a chat from a tab's chip or its menu (#1487): the tab is switched to it, and its
   *  terminal takes the keyboard whether or not the tab was already on it. */
  //
  // **The same function for the window's life**: `showChat` is made again whenever the list of
  // chats is read, and a chip handed a new function is a chip drawn again, on every tab, for
  // one task's move (SC-3). So the chips hold this, and this reads the one that is current.
  const showChatNow = useRef(showChat);
  useEffect(() => {
    showChatNow.current = showChat;
  }, [showChat]);
  const showFromChip = useCallback(
    (session: number) => {
      showChatNow.current(session);
      giveKeyboardTo(plane, session);
    },
    [plane],
  );
  /** **A task picked in a head's tasks menu is shown in that head's pane** (#5058, #5341), as a
   *  press shows it: one on screen already is jumped to (D1), in this window or, asked first,
   *  another of the project's (#4768). That pane while it is still there, whichever window's
   *  pane was used last, so the core is told it was used before it is asked (#5108: the chord
   *  itself never reaches the panes); else wherever a press puts it. */
  const showPickedTask = useCallback(
    (session: number, asked: PaneAt | undefined) => {
      const into =
        asked !== undefined &&
        contentsOf(now.current, asked.tab).some((one) => one.pane === asked.pane)
          ? asked
          : undefined;
      const here = () => {
        showChatNow.current(session, into);
        giveKeyboardTo(plane, session);
      };
      if (oneWindow() || tabHolding(now.current, session) !== undefined) {
        here();
        return;
      }
      void (into === undefined ? Promise.resolve() : commands.paneUsed(plane))
        .then(() => commands.pressElsewhere(plane, session))
        .then((answer) => {
          if (answer.status !== "ok" || answer.data === null) here();
        })
        .catch(here);
    },
    [oneWindow, plane],
  );

  /** The task tab `id` shows in place of its session's own chat, by name (#1486): what its
   *  label says after the session's name. Nothing while it shows its own chat. */
  const taskNameOf = (id: number) => {
    const task = taskShownIn(tabs, id);
    if (task === undefined) return undefined;
    const chat = chatsByNumber.get(task);
    if (chat !== undefined) return shownName(tabs, chat);
    // One that has ended is still what the tab shows, by the name it had.
    const lead = contentsOf(tabs, id)[0]?.pane;
    const last = recalled.get(`${id}:${lead}:${task}`);
    return last?.path[last.path.length - 1].name;
  };
  /**
   * **Whether tab `id` is a task's own** (#1489), and then the name of the chat that asked for
   * it, where that chat is still listed: what the tab says after its own name. Nothing for a
   * chat's tab and for a view's.
   */
  const taskTabOf = (id: number) => {
    const own = chatOf(tabs, id);
    const from = own === undefined ? undefined : fromOf(own);
    if (!from?.task) return undefined;
    const asker = chatsByNumber.get(from.chat);
    return { asker: asker === undefined ? undefined : shownName(tabs, asker) };
  };
  /** The Start fresh row a tab's mark presses, its own chat's: only one that can run. Every
   *  chat with a conversation has the row (#4753), greyed with its reason while nothing it read
   *  has changed, or for a task while a task cannot start fresh; neither draws a mark. */
  const freshOf = (id: number) => {
    const own = chatOf(tabs, id);
    const row = own === undefined ? undefined : by(freshId(own));
    return row?.available ? row : undefined;
  };

  /**
   * The focused workspace's worktrees, as the catalogue names them (charter-app#174).
   *
   * **The clones in the plane's own order, and the pieces in git's** — the order the explorer
   * draws them in, so a palette listing them and a tree showing them agree. Held on the clone
   * names and the pieces, both of which the store keeps while an answer says them again
   * (`workspacePanels.ts`), so this is rebuilt when a listing moves and on no other render.
   * That is what keeps ~100 more catalogue rows off the per-keystroke path.
   */
  const pieces = useMemo<Cut[]>(
    () =>
      ofWorkspace === undefined
        ? []
        : (cloneNames ?? []).flatMap((repo) =>
            (workspaceState.pieces[repo] ?? []).map((piece) => ({
              workspace: ofWorkspace,
              repo,
              piece: piece.piece,
              branch: piece.branch,
            })),
          ),
    [cloneNames, ofWorkspace, workspaceState.pieces],
  );

  /**
   * The focused workspace's clones with the paths the core spelled (charter-app#174), for the
   * two rows each clone's menu lists. Held on the names and the paths, which the store keeps
   * while an answer says them again, for the pieces' reason above.
   */
  const clonePaths = workspaceState.panels?.paths;
  const clones = useMemo<Clone[]>(() => {
    if (ofWorkspace === undefined || cloneNames === undefined || clonePaths === undefined)
      return [];
    return cloneNames.flatMap((repo) => {
      const path = clonePaths[repo];
      return path === undefined ? [] : [{ repo, path }];
    });
  }, [cloneNames, clonePaths, ofWorkspace]);

  /** What this window is cloning into the focused workspace (#1215), by repo — each row of
   *  "Not cloned here" draws its own progress and failure from it. */
  const cloning = useRepoClones(plane, ofWorkspace ?? OUTSIDE);
  const absentHere = workspaceState.panels?.absent;
  // A repo that has cloned stays busy until the panels list it, so its Clone cannot be pressed
  // again in between; once they do, the window forgets it (`settleRepoClones`). The panels can
  // list it before its clone answers, so an answer settles it as well as a read.
  const cloningNow = useMemo(
    () => [...cloning].filter(([, one]) => one.state !== "failed").map(([repo]) => repo),
    [cloning],
  );
  const failedHere = useMemo(
    () => failedNotNamed(absentHere, cloneNames, cloning),
    [absentHere, cloneNames, cloning],
  );
  useEffect(() => {
    if (ofWorkspace !== undefined && cloneNames !== undefined)
      settleRepoClones(plane, ofWorkspace, cloneNames);
  }, [plane, ofWorkspace, cloneNames, cloning]);
  // One that landed while another workspace was focused was never going to be settled: the panels
  // this focus asks for are later than it, and may never list it (#4466).
  useEffect(() => {
    if (ofWorkspace !== undefined) forgetLandedClones(plane, ofWorkspace);
  }, [plane, ofWorkspace]);

  /** The repo whose clone the chat being picked would start in, if it would: what the picker's
   *  branch box names (GL-1). The same directory `startOn` sends, compared with the path the
   *  core spelled for each clone. */
  const pickingInRepo = useMemo(() => {
    if (picking === undefined) return undefined;
    const cwd = ("in" in picking.where ? picking.where.in : undefined) ?? startIn;
    return clones.find((one) => one.path === cwd)?.repo;
  }, [clones, picking, startIn]);

  /** The plane's personas, straight off the store — the array, not a copy of it, so the
   *  catalogue is rebuilt when the personas move and not per render. **With no workspace
   *  focused, the sidebar's**: the strip outside every workspace reads no workspace's panels,
   *  and the Personas view still lists the plane's personas there (#1686), whose rows open
   *  nothing without their catalogue rows (#4758, where a strip's kept empty tab puts that
   *  strip in front far more often). */
  const personas = workspaceState.panels?.personas ?? sidebar?.personas;
  /** The project's memory stores, for a memory's Move rows (#1190): read again when a memory is
   *  written, or a workspace or a persona comes or goes. */
  const memoryStores = useMemoryStores(
    plane,
    [
      memoryEdits.changed,
      ...(sidebar?.workspaces ?? []).map((ws) => ws.name),
      "",
      ...(personas ?? []),
    ].join("\n"),
  );
  /** The stores and the LIVE names a memory list's Move rows and a memory tab's Move are worded
   *  from (#1190): a move into a LIVE workspace's journal is published with the project. */
  /** **Why this purlis opens the project read-only, or null when it may write it** (#826,
   *  FR-24): said once on the project's tab, and the reason every row that writes a committed
   *  project file is off. Asked again when the project's manifest changes on disk. */
  const readOnly = useReadOnly(plane);
  const readOnlySaid = readOnly?.sentence;
  const memoryTargets = useMemo<MemoryTargets>(
    () => ({ stores: memoryStores, live: liveNames, readOnly: readOnlySaid }),
    [memoryStores, liveNames, readOnlySaid],
  );
  /** What Ask {persona}… offers on this project's chats, as the core answers. */
  const askOffer = useAskOffer(
    plane,
    personas,
    settingsChanges + curationsChanges,
    tabs.order
      .map((tab) => chatOf(tabs, tab))
      .filter((chat) => chat !== undefined)
      .join(","),
  );
  /** The focused workspace's open todos, the same way: one close and one forget row each. */
  const todos = workspaceState.panels?.todos;
  /** And its done ones (ADR 0106): one Reopen and one Delete row each. */
  const doneTodos = workspaceState.panels?.done_todos;
  /** What the plane, its workspaces and its personas are offered to curate (ADR 0061), read
   *  again when those change and when the plane changes on disk. */
  const subjects = useMemo(
    () => curationSubjects(sidebar?.workspaces.map((ws) => ws.name) ?? [], personas ?? []),
    [personas, sidebar],
  );
  const curations = useCurations(plane, subjects, curationsChanges);

  // The catalogue's screen-reader row says what pressing it does now: it is built again when
  // the mode is turned on or off, not only when something else draws this view again.
  const screenReader = useScreenReaderMode();

  /**
   * Every action this project's window can do, in one list.
   *
   * **The bar's buttons are rows of THIS list, not a second one.** The tmux frame kept a
   * menu beside its palette once and the two drifted; here `New tab`, the splits, `Close
   * pane` and every tab's `×` are looked up by id out of the same catalogue the palette
   * draws, so a row that goes away takes its button with it. The project strip does the same
   * thing one scope up, through `actions.projectRows`.
   *
   * **Only for the project in front.** Nothing draws a catalogue for a project that is not on
   * screen — not its bar, and not the window's palette, which lists the front one's. At ADR
   * 0026's limits that is the difference between rebuilding 117 rows once per hook event and
   * rebuilding them once per hook event per project the window happens to hold.
   */
  const offers = useMemo(
    () =>
      !inFront
        ? []
        : catalogue({
            tabs,
            workspaces: strips,
            live: liveNames,
            focused,
            worktreeOf,
            pieces,
            clones,
            absent: absentHere,
            failedNotNamed: failedHere,
            cloning: cloningNow,
            startsIn: spot?.path,
            personas,
            vaults: vaultNames,
            todos,
            doneTodos,
            browsers: browsersOffered,
            plane,
            projects,
            // Which window this is, for the rows that move a project between windows (charter#126).
            split: thisWindow() !== MAIN,
            // WHICH row was refused and is still on screen. The catalogue matches the ids it
            // wrote itself, so the discard row appears beside the removal that was refused and
            // beside no other — with one removal per piece that is the difference between one
            // offer to throw work away and fifty.
            refused: report?.refused ? report.from : undefined,
            readRefused,
            // **With no queue, and no Smart closes that stopped** (#1034): their rows are put in
            // as this view reports (`queued`), so a chat that starts asking for you neither
            // rebuilds the catalogue nor redraws the panes.
            needsYou: NO_QUEUE,
            quiet,
            nameOf,
            reportsTo: (session) => reports[session] ?? [],
            refusedIn: (session) => refusals[session] ?? [],
            neededFor: (session) => needs[session] ?? [],
            stoppedBelow: (session) => stoppedBelow[session] ?? [],
            listed: listedChats,
            askedBy,
            finished: finishedTasks,
            stopping: [...stopping],
            // The projects' pins are the WINDOW's, and travel down with the projects: a
            // project that is not in front draws nothing, so its pin cannot be held here.
            pinned: {
              workspaces: pinnedWorkspaces,
              projects: pinnedProjects,
            },
            views,
            commands: extensionCommands,
            curations,
            wrappingUp: [...wrapping],
            restoring: [...restoringChats],
            endedHere: [...endedHere.keys()],
            // An ended task's `↳ from` (#5366): the chat that asked it, off the path its pane
            // last had.
            endedAskedBy: endedAskers,
            // Close other tabs skips the pinned ones (#4762).
            otherTabs: (tab) => otherTabsToClose(tabs, tab, filedIn, isPinned),
            // Move to window ▸ lists every other window of the project (#4768).
            otherWindows: windowsHere.filter((one) => one !== (tabWindow ?? 0)),
            workItems,
            // A chat filed in a workspace; one at the project root, or outside the project, is
            // offered no work link (ADR 0088 §4).
            linkable: (session) => filedIn(session) !== OUTSIDE,
            planeUpdated: planeUpdates,
            restartable,
            movesTo,
            movesOnceAPoolIsSetUp: movesOnceSetUp,
            movesWith: movesWithChat,
            clonesComeBack,
            ask: askOffer,
            // Not from a shell: it is on no harness profile to start the persona's chat on.
            askable: (session) => !shells.has(session),
            cardOf: (session) => glances[session],
            studioOf: (session) => studioLinks[session],
            branch: nearBranch,
            harnesses: harnessCards,
            memoryStores,
            away,
            focusMode,
            sidePanels,
            paneRoom: focusedRoom,
            profiles: profileNames,
            screenReader,
            skillStudio: studioOn,
            readOnly: readOnlySaid,
            places,
          }),
    [
      glances,
      studioLinks,
      readOnlySaid,
      places,
      askedBy,
      away,
      focusMode,
      sidePanels,
      focusedRoom,
      clones,
      cloningNow,
      absentHere,
      failedHere,
      curations,
      focused,
      inFront,
      nameOf,
      personas,
      pieces,
      pinnedProjects,
      pinnedWorkspaces,
      plane,
      projects,
      quiet,
      readRefused,
      report,
      spot?.path,
      refusals,
      reports,
      needs,
      stoppedBelow,
      listedChats,
      finishedTasks,
      stopping,
      strips,
      tabs,
      vaultNames,
      todos,
      doneTodos,
      browsersOffered,
      views,
      extensionCommands,
      worktreeOf,
      liveNames,
      wrapping,
      restoringChats,
      endedHere,
      endedAskers,
      isPinned,
      windowsHere,
      tabWindow,
      workItems,
      filedIn,
      planeUpdates,
      restartable,
      movesTo,
      movesOnceSetUp,
      movesWithChat,
      clonesComeBack,
      askOffer,
      shells,
      nearBranch,
      harnessCards,
      memoryStores,
      profileNames,
      studioOn,
      screenReader,
    ],
  );

  /**
   * The catalogue by id, built once per catalogue rather than scanned per lookup.
   *
   * Every surface in this window that draws ONE row asks here: the tab strip (two lookups per
   * tab), the workspace strip, the show-more menus, each pane's own controls — and, since
   * #172, a context menu on every one of them, which is `menuRows` scanning the whole list
   * three times per tab per render. One render of a fifty-tab strip was 0.047 ms of scanning
   * and is 0.017 ms through this; what the 31 µs buys is not a speed anybody feels but a cost
   * that stops tracking the catalogue's length, which #174 is the change that grew.
   * `actions.catalogued` has the table and the method.
   */
  const found = useMemo(() => catalogued(offers), [offers]);

  const by = useCallback((id: string) => found.get(id), [found]);
  /** The strip's `+` list (#5292): the catalogue's rows marked for it. */
  const plusRows = useMemo(() => newTabRows(offers), [offers]);

  /** Carries a row out and keeps what it answered. Everything below this line has already
   *  been asked about, where asking was owed. */
  const carryOut = useCallback(
    async (offer: Offer): Promise<Ran> => {
      const answer = await perform(offer, doing);
      setReport(saidOf(offer.id, answer));
      return answer;
    },
    [doing],
  );

  /**
   * The row waiting on an answer, when the operator has asked for something that ends a chat.
   *
   * Held here and not in the dialog, because the dialog is drawn only while there is one:
   * a component that is not mounted cannot be holding the question it is about to ask.
   */
  const [endingChat, setEndingChat] = useState<{
    offer: Offer;
    /** The chat End… ends — what Smart close would close. */
    session: number;
    /** Whether that chat is offered Smart close (ADR 0064), or none where charter cannot say. */
    smart?: SmartAsk;
    /** The persona chats that chat asked for that are still running, by name: what the dialog
     *  asks about once, keep them running or stop them. */
    running?: readonly string[];
    /** Its persona chats that have reported and close with it, by name. */
    closing?: readonly string[];
    /** How many tasks below the closing session have a tab of their own (#1489). */
    ownTabs?: number;
    /** The closing chat's theme drafts (#2332), which the close asks Keep or Revert about. */
    drafts?: readonly WindowDraft[];
    /** Whether a link asked for this close (#2761): the dialog says so. */
    fromALink?: boolean;
    /** Whether the chat is a shell (#4761): End… asks only Cancel or End. */
    shell?: boolean;
  }>();
  /** The themes kept as their chat closed (#2332), each asked in turn who draws it (#2708). */
  const [keptOnClose, setKeptOnClose] = useState<readonly KeptOnClose[]>([]);

  /**
   * What every surface does with a row: ask first where a chat is about to end, then carry
   * it out.
   *
   * One function for the bar, the panes, the tabs and the palette. It is called from an
   * event handler and never while rendering, which is what lets the verbs it dispatches to
   * reach the window's live arrangement rather than a copy taken when the row was built.
   *
   * **The confirmation is HERE rather than on each button** (the operator: *"closing session
   * should ask confirmation"*). There are four ways to end a chat — a tab's `×`, a pane's
   * `×`, the palette's row and the palette's pane row — and a guard on three of them is a
   * guard an operator learns to trust and then walks past on the fourth.
   *
   * **It answers `{ ok: true }` for a row it has only ASKED about**, which is true: nothing
   * was refused and nothing has happened yet. The palette reads this answer to say what a row
   * did, and "nothing to say" is the right thing to say about a question still on screen.
   */
  // The close asked again, counted, once the question about its unsaved work is answered with
  // a leave (#3929).
  const askAgain =
    useRef<(offer: Offer, fromALink: boolean, unsavedAnswered: boolean) => Promise<Ran>>(undefined);
  const askToEndNow = useCallback(
    async (offer: Offer, fromALink: boolean, unsavedAnswered = false): Promise<Ran> => {
      // **A view with unsaved work asks first** (#3683, #3929; `unsavedWork.ts`): its tab comes
      // to the front, the window asks Keep editing, Discard and leave, or Save and leave, and the
      // close is asked again once it is answered with a leave, and goes on to what it ends. The
      // work goes with its tab once the close is carried out (`discardGone`).
      const unsaved =
        offer.available && !unsavedAnswered
          ? unsavedClosedBy(plane, now.current, offer.does)
          : undefined;
      if (unsaved !== undefined) {
        if (now.current.inFront !== unsaved.tab) bringToFront(unsaved.tab);
        askUnsaved({
          work: unsaved.work,
          then: () => void askAgain.current?.(offer, fromALink, true),
        });
        return { ok: true };
      }
      // **Only End… ends a chat, and is asked about first** (#4753, #4756): a close ends nothing.
      if (offer.available && offer.does.verb === "endChat") {
        const session = offer.does.session;
        // **The theme drafts of the chat it ends** (#2332), which it asks Keep or Revert about;
        // on a link, nothing is asked (#2708), and an end with drafts waits in the window
        // (#2761).
        const found = await draftsOfClosing(commands, plane, [session]);
        if (!found.ok)
          return "handed" in found
            ? { ok: true, said: found.handed }
            : { ok: false, refused: found.refused };
        const drafts = found.drafts;
        // **End… on a shell asks only Cancel or End** (#4761, ADR 0062 as amended 2026-11-02):
        // a shell runs no harness, so it has no record to write and no chats it asked for.
        if (shellsRef.current.shells.has(session)) {
          setEndingChat({ offer, session, drafts, fromALink, shell: true });
          return { ok: true };
        }
        // The core's answer, asked now: whether the chat can write a record depends on what it
        // is doing this moment, and the command that starts a smart close asks the same thing.
        const asked = await commands
          .smartCloseOffer(plane, session)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
        const smart =
          asked.status === "ok" && asked.data !== null && typeof asked.data === "object"
            ? asked.data
            : undefined;
        // And the chats at work below it, which the end asks about once, and its reported
        // persona chats, which end with it. A core that answers nothing (an older one) has none
        // below it. Its tasks in panes stay where they are (#4757): a chat leaving moves no
        // other pane, so the dialog has nothing to say of them.
        const theirs = await commands
          .personaChatsOf(plane, session)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
        const said = theirs.status === "ok" ? theirs.data : null;
        const running = Array.isArray(said?.running) ? said.running : [];
        const closing = Array.isArray(said?.tasks)
          ? said.tasks.filter((one) => one.closes_with_its_asker).map((one) => one.name)
          : [];
        setEndingChat({ offer, session, smart, running, closing, drafts, fromALink });
        return { ok: true };
      }
      return carryOut(offer);
    },
    [askUnsaved, bringToFront, carryOut, plane, setEndingChat],
  );
  /**
   * **A close counts as a question up from its first step** (#2761), and the mark is cleared
   * however it settles (#2845): `closeQuestion.ts`. The question about a view's unsaved work is
   * one too (#3929).
   */
  const { up: questionUp, counted } = useCloseQuestion(
    plane,
    endingChat !== undefined || keptOnClose.length > 0 || unsavedAsk !== undefined,
    onAsking,
  );
  const askToEnd = useCallback(
    async (offer: Offer, fromALink: boolean, unsavedAnswered = false): Promise<Ran> => {
      const asks =
        offer.available &&
        (offer.does.verb === "endChat" ||
          (!unsavedAnswered && unsavedClosedBy(plane, now.current, offer.does) !== undefined));
      if (!asks) return askToEndNow(offer, fromALink, unsavedAnswered);
      return counted(() => askToEndNow(offer, fromALink, unsavedAnswered));
    },
    [askToEndNow, counted, plane],
  );
  useEffect(() => {
    askAgain.current = askToEnd;
  }, [askToEnd]);
  const run = useCallback((offer: Offer) => askToEnd(offer, false), [askToEnd]);

  /**
   * **A link's close of chats with theme drafts, handed to this window** (#2761): asked here as
   * the window's own close of the tab that holds them is, Keep or Revert first, and the dialog
   * says a link asked. Nothing is asked while a question is on screen anywhere in the window
   * (`closeHanded.ts`), so a link never changes what the person is answering; nor for chats no
   * tab here holds alone, which the person closes here themselves. Either is said here.
   */
  /** Says, under the strip, why a link's close was not asked: once the ask is handled, as a
   *  close's own answer is said once it is carried out. */
  const sayNotAsked = useCallback((words: string) => {
    void Promise.resolve().then(() => setReport({ from: "close-handed", refused: true, words }));
  }, []);
  const closeHandled = useRef(closeAsked?.at);
  useEffect(() => {
    if (closeAsked === undefined || closeHandled.current === closeAsked.at) return;
    closeHandled.current = closeAsked.at;
    if (closeAsked.dropped || questionUp) {
      sayNotAsked(CLOSE_HANDED_NOT_ASKED);
      return;
    }
    // **Asked as the chat's own End…** (#4756): a close ends nothing, so a link's end of a chat
    // is End… of that chat, and End… is one chat's.
    const [only, ...more] = closeAsked.sessions;
    const offer = only === undefined || more.length > 0 ? undefined : by(endId(only));
    if (offer === undefined || !offer.available) {
      sayNotAsked(CLOSE_HANDED_NOT_HERE);
      return;
    }
    void askToEnd(offer, true);
  }, [askToEnd, by, closeAsked, questionUp, sayNotAsked]);

  /**
   * **Keeps or reverts a closing chat's theme drafts** (#2332), as its close was answered, while
   * the chat is still open: the core finds a draft by its chat. Answers whether the close goes
   * on; a draft the core would not keep (it breaks a hard floor) holds the close, and says why.
   */
  const settleDrafts = useCallback(
    async (drafts: readonly WindowDraft[] | undefined, keep: boolean | undefined) => {
      if (drafts === undefined || drafts.length === 0 || keep === undefined) return true;
      const settled = await settleClosingDrafts(commands, plane, drafts, keep);
      // Who draws each theme kept, asked about every one in turn, even when a later one is
      // refused (#2708).
      if (settled.kept.length > 0) setKeptOnClose(settled.kept);
      void drawWhatIsInForce();
      if (settled.refused !== undefined) {
        sayAboutThisMachine("theme draft", {
          severity: "warn",
          detail: settled.refused,
          remedy: "the chat is still open: fix its draft, or close it and answer Revert",
        });
        return false;
      }
      sayAboutThisMachine("theme draft", undefined);
      return true;
    },
    [plane],
  );

  /**
   * Smart close pressed: the core sends the chat its prompt, now or when its turn ends. **The
   * chat stays where it is** (#4760, spec #4750): no tab goes into the background as a chip any
   * more; its pane head and its row breathe, and Cancel smart close is its head's state button
   * and first in its list. A start the core refuses says why, and lists it as needing the
   * operator.
   */
  const beginSmartClose = useCallback(
    async (session: number) => {
      stoppedFor(session, undefined);
      setLeaving((was) => new Set([...was, session]));
      const began = await commands
        .smartClose(plane, session)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (began.status === "error") {
        settle(session);
        stoppedFor(session, DID_NOT_START);
        setReport({ from: `smartclose:${session}`, refused: true, words: began.error });
      }
    },
    [plane, settle, stoppedFor],
  );

  /**
   * **Stop them**, the person's answer about the chats at work below a closing chat: one
   * command to the core (`close_chat_stopping`), which ends them, deepest first, and then the
   * chat itself in the same step, so it cannot start another in between. For a Smart close
   * it ends them now and remembers the answer for the close the chat's record brings. The
   * window takes away the tabs of what the core closed — `closeChat`, never `close_session`,
   * which would end them a second time.
   */
  const closeStopping = useCallback(
    async (session: number, then: "close" | "smart_close") => {
      const stopped = await commands
        .closeChatStopping(plane, session, then)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (stopped.status === "error") {
        setReport({ from: `close-stopping:${session}`, refused: true, words: stopped.error });
        return false;
      }
      const gone = Array.isArray(stopped.data) ? stopped.data : [];
      // The chat End… was answered for keeps its pane, as ended (#5019). What the core stopped
      // below it goes as before.
      if (then === "close") leavesItsBand(session);
      if (gone.length > 0) {
        change((tabs) =>
          gone.reduce((left, one) => closeChat(left, one, filedIn, isPinned, keepsItsPane), tabs),
        );
        // Whatever below them the core did not end (it ends what is at work) has no tab left.
      }
      return true;
    },
    [keepsItsPane, change, filedIn, isPinned, leavesItsBand, plane],
  );

  const press = useCallback(
    (offer: Offer) => {
      void run(offer);
    },
    [run],
  );
  /**
   * **The two rows that end a task, for a tab chip's menu** (#1487 mounting #1488's
   * `taskEndIds`): read from the catalogue as the menu is drawn. One function for the life of
   * the view, so a chip is not drawn again each time the catalogue is made anew.
   */
  const byNow = useRef(by);
  const pressNow = useRef(press);
  useEffect(() => {
    byNow.current = by;
    pressNow.current = press;
  }, [by, press]);
  const pressTaskEnd = useCallback((offer: Offer) => pressNow.current(offer), []);
  /** A link out of Settings (#1387, #1388): the catalogue's row it names, run as the palette
   *  runs it. One the catalogue does not hold now does nothing. */
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<SettingsActionAsk>).detail;
      if (wanted.plane !== plane) return;
      const offer = byNow.current(wanted.action);
      if (offer !== undefined) pressNow.current(offer);
    };
    window.addEventListener(SETTINGS_ACTION, asked);
    return () => window.removeEventListener(SETTINGS_ACTION, asked);
  }, [plane]);
  /** A chat's skills notice's "Open Skill studio" (#3681): the studio at its page and scope. */
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<SkillStudioAsk>).detail;
      if (wanted.plane !== plane) return;
      showView(skillStudioAt(wanted.scope, wanted.page, { item: wanted.item }), SKILL_STUDIO_TITLE);
    };
    window.addEventListener(SKILL_STUDIO_ASKED, asked);
    return () => window.removeEventListener(SKILL_STUDIO_ASKED, asked);
  }, [plane, showView]);
  /** **A chat's "Show this chat's skills"** (S8, #3694: its skills chip, and its skills notices'
   *  Show in Skills and What changed): that chat put in front, so the sidebar Skills view
   *  follows it, and the view shown. The chips' own `showChatNow`, which reads the one current. */
  useEffect(() => {
    const asked = (event: Event) => {
      const wanted = (event as CustomEvent<ChatSkillsAsk>).detail;
      if (wanted.plane !== plane) return;
      showChatNow.current(wanted.session);
      showSideView("skills");
    };
    window.addEventListener(CHAT_SKILLS_ASKED, asked);
    return () => window.removeEventListener(CHAT_SKILLS_ASKED, asked);
  }, [plane, showSideView]);
  /** Stop all tasks of a session, for its tab chip's menu (#1498): the catalogue's row, read
   *  as the menu is drawn, and one function for the life of the view. */
  const stopAllOf = useCallback((session: number) => byNow.current(stopAllId(session)), []);
  const taskEndsOf = useCallback(
    (session: number) =>
      taskEndIds(session).flatMap((id) => {
        const offer = byNow.current(id);
        return offer === undefined ? [] : [offer];
      }),
    [],
  );

  /** Moves a task from its line in a tab's menu (#1489): to a tab of its own, beside the
   *  chat that asked for it, or back. The catalogue's own rows, read as they stand when
   *  pressed, and held as one function for the window's life, as {@link showFromChip} is. */
  const placeNow = useRef<(session: number, where: TaskPlace) => void>(() => undefined);
  useEffect(() => {
    placeNow.current = (session, where) => {
      const offer = by(placeRowId(session, where));
      if (offer?.available) press(offer);
    };
  });
  const placeFromChip = useCallback(
    (session: number, where: TaskPlace) => placeNow.current(session, where),
    [],
  );
  /** The catalogue's row a task's line in a tab's menu presses to move it (#5679): read as the
   *  line is drawn, so its button says that row's title. */
  const placeRowFromChip = useCallback(
    (session: number, where: TaskPlace) => byNow.current(placeRowId(session, where)),
    [],
  );
  /** What a tab's chats used, for its menu (#1500): read only as the window asks. */
  const usedFromChip = useCallback((ask: UsedAsk) => readUsed(plane, "menu", ask), [plane]);
  /** Opens a session's Activity from its tab's menu (#1495): the view the catalogue's
   *  `tab.activity` row opens, named as the chat is now. One function for the window's life. */
  const activityNow = useRef<(session: number) => void>(() => undefined);
  useEffect(() => {
    activityNow.current = (session) =>
      showView(activityView(session), activityTitle(nameOfNow.current(session)));
  });
  const activityFromChip = useCallback((session: number) => activityNow.current(session), []);
  /** Opens the Skill studio at skill `skill`'s Runs with run `run` marked, from the chip of the
   *  run open in chat `session`'s tab (SK-3, #3682): the skill by its own name, as the core
   *  counts the run (#3385), on the chats of the place the chat is in; a persona's own skill on
   *  that persona's chats, as the core says its level (#3694), since a workspace's scope does not
   *  list it; and another workspace's copy its persona names on that workspace's chats, whose
   *  runs that copy counts wherever they ran (#6083). */
  const openSkillRun = useCallback(
    ({ name: skill, id: run, level, copy_workspace: copy }: OpenRun, session: number) => {
      const workspace = filedIn(session);
      const persona = personaOf(session);
      const scope =
        aPersonasOwn(level) && persona !== null
          ? asPersona(persona)
          : copy !== null
            ? inWorkspace(copy)
            : workspace !== undefined && workspace !== OUTSIDE
              ? inWorkspace(workspace)
              : persona !== null
                ? asPersona(persona)
                : AT_THE_ROOT;
      showView(skillRunsAt(scope, skill, run), SKILL_STUDIO_TITLE);
    },
    [filedIn, personaOf, showView],
  );
  /** **What a chat uses now, on its pane head** (#5524): its skill run open (SK-3) and its
   *  browsers, live (#2024), each pressed to open what it is. They left its tab (#5339). */
  const usingOn = useCallback<UsingOn>(
    (session) => (
      <>
        <SkillRunChip session={session} onOpen={openSkillRun} />
        <PaneBrowsers
          browsers={chatBrowsersShown.get(session) ?? NO_BROWSERS}
          onOpen={openChatBrowser}
        />
      </>
    ),
    [chatBrowsersShown, openChatBrowser, openSkillRun],
  );

  /**
   * **A tab on the strip was pressed.** One that is behind comes forward, on whatever it was
   * left showing. **One already in front goes back to its session's own chat** (#1486,
   * V100-36), which is the way out of a task that needs no reading: the tab is the session.
   *
   * **Renaming a tab and moving it do not change what it shows.** A double-click renames, and
   * its first click is a click; a drag ends in a click too. So a pointer's press goes back a
   * moment later, unless a second click or a drag says it was not a press to go back. A key's
   * press is a press and nothing else, and goes back at once.
   */
  const backSoon = useRef<number | undefined>(undefined);
  const draggedAt = useRef(0);
  const dragging = useRef(false);
  /** Whether a tab is being carried along the strip: no task menu opens under a drag (#1487). */
  const isDragging = useCallback(() => dragging.current, []);
  /**
   * **What a pane head's state word opens** (#5341): the menu the tab's chip opened, moved as it
   * is, for the chat the pane shows and its tasks. ⌘⇧J asks for the same one (`chatTasksAsked`),
   * and a pick shows the task in this pane. A chat with no tasks keeps its word as it was.
   */
  // A pick's way into each pane, the same function for the pane's life, so a head's button is
  // drawn again only when what it says changes (SC-3).
  const showPickedNow = useRef(showPickedTask);
  useEffect(() => {
    showPickedNow.current = showPickedTask;
  }, [showPickedTask]);
  const pickedInto = useRef(new Map<string, (session: number) => void>());
  const showPickedIn = useCallback((at: PaneAt) => {
    const key = `${at.tab}:${at.pane}`;
    let into = pickedInto.current.get(key);
    if (into === undefined) {
      into = (session: number) => showPickedNow.current(session, at);
      pickedInto.current.set(key, into);
    }
    return into;
  }, []);
  const tasksOn = useCallback<TasksOn>(
    (session, at, word, alone) => {
      if (at === undefined) return alone ? null : word;
      const rows = chatsOfChat(chatsByTab.get(at.tab) ?? NO_ROWS, session);
      const below = new Set(rows.map((row) => row.session));
      const ended = (endedByTab.get(at.tab) ?? NO_ENDED).filter((task) => below.has(task.asker));
      // A head that says no state of its own draws nothing where there is no menu to open.
      if (alone && !hasTasks(rows, ended)) return null;
      const asked =
        chatTasksAsked?.session === session &&
        chatTasksAsked.tab === at.tab &&
        chatTasksAsked.pane === at.pane
          ? chatTasksAsked.count
          : 0;
      return (
        <ChatTasks
          id={tasksIdOf(session, at.pane)}
          name={nameOf(session)}
          rows={rows}
          ended={ended}
          current={session}
          asked={asked}
          clock={chatClock}
          dragging={isDragging}
          onShow={showPickedIn(at)}
          ends={taskEndsOf}
          onPress={pressTaskEnd}
          onPlace={placeFromChip}
          placeOf={placeRowFromChip}
          used={usedFromChip}
          // The limits of this pane's own chat, the menu's first row (#1498).
          limits={runningSaid(rows[0])}
          stopAll={stopAllOf}
          onActivity={activityFromChip}
          alone={alone}
        >
          {word}
        </ChatTasks>
      );
    },
    [
      activityFromChip,
      chatClock,
      chatTasksAsked,
      chatsByTab,
      endedByTab,
      isDragging,
      nameOf,
      placeFromChip,
      placeRowFromChip,
      pressTaskEnd,
      showPickedIn,
      stopAllOf,
      taskEndsOf,
      tasksIdOf,
      usedFromChip,
    ],
  );
  const notGoingBack = useCallback(() => {
    window.clearTimeout(backSoon.current);
    backSoon.current = undefined;
  }, []);
  useEffect(() => notGoingBack, [notGoingBack]);
  const pressTab = useCallback(
    (id: number, select: Offer | undefined, byPointer: boolean) => {
      notGoingBack();
      if (now.current.inFront !== id) {
        if (select?.available) press(select);
        return;
      }
      const back = () => {
        const tab = now.current.byId[id];
        if (tab?.shows === undefined || now.current.inFront !== id) return;
        const own = panesOf(now.current, id).find((one) => one.pane === tab.focused)?.session;
        change((tabs) => showOwn(tabs, id));
        if (own !== undefined) giveKeyboardTo(plane, own);
      };
      if (now.current.byId[id]?.shows === undefined) return;
      // A tab being carried, or just put down, was not pressed: the keys and the click that
      // start and end a drag are the drag's.
      if (dragging.current || performance.now() - draggedAt.current < AFTER_A_DRAG) return;
      if (!byPointer) back();
      else backSoon.current = window.setTimeout(back, A_DOUBLE_CLICK);
    },
    [change, notGoingBack, plane, press],
  );

  /**
   * **The keys that open things** (`shellKey.ts`; #5292): ⌘⇧T the catalogue's `shell.new` row,
   * ⌘T its `tab.new` and ⌘N its `chat.new`, each pressed as the palette would press it. Claimed
   * on the window, capture-phase, for the reason the palette's key is — a pane's terminal would
   * otherwise have it first — and only by the project in front, so one keypress is one shell,
   * one tab or one picker.
   */
  useEffect(() => {
    if (!inFront) return;
    const key = (e: KeyboardEvent) => {
      // The key and its rule are the key registry's (#2921): never under a modal.
      const act = pressed(e, OPENING_KEYS);
      if (act === undefined) return;
      const offer = by(act.id);
      if (offer === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      // A held key is not a second press: one shell for one press.
      if (e.repeat) return;
      press(offer);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [by, inFront, press]);

  /**
   * **Use skill…'s key** (#3954, the key registry's `chat.useSkill`): Use skill… on the chat in
   * front, as the Use skill… row of its tab opens it. In a split tab that is the pane that has
   * the keyboard (`focusedChat`), else the tab's chat in front (`chatInFrontOf`, a view's tab
   * included). Claimed on the window, capture-phase, by the project in front, as the shell's key
   * is; never under a modal. With the Skill studio off, a shell or no chat in front, there is no
   * Use skill… to open, and the key goes on.
   */
  useEffect(() => {
    if (!inFront || !studioOn) return;
    const key = (e: KeyboardEvent) => {
      if (pressed(e, ["chat.useSkill"]) === undefined) return;
      const tabs = now.current;
      const session =
        focusedChat(tabs) ??
        (tabs.inFront === undefined ? undefined : chatInFrontOf(tabs, tabs.inFront));
      if (session === undefined || shells.has(session)) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat) return;
      openUseSkill(session);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [inFront, studioOn, shells, openUseSkill]);

  /**
   * **Focus mode's key and the keyboard shortcuts' key** (#2921, `keys/ownKeys.ts`): each presses
   * its catalogue row, as the palette would, so the key and the row are one act. Claimed on the
   * window, capture-phase, by the project in front, as the shell's key is; their context rules
   * (a modal keeps both, an editor keeps ⌘/) are the key registry's.
   */
  useEffect(() => {
    if (!inFront) return;
    const key = (e: KeyboardEvent) => {
      const hit = pressed(e, ["focus.toggle", "shortcuts.show"]);
      if (hit === undefined) return;
      const offer = by(hit.id === "focus.toggle" ? FOCUS_MODE_ID : SHORTCUTS_ID);
      if (offer === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat) return;
      press(offer);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [by, inFront, press]);

  /**
   * **The keys for the chats inside the tab in front** (`taskKeys.ts`, #1487): its task menu,
   * the next and the previous chat in it, and back to its session's own chat. Each presses the
   * catalogue's row, as the palette would. Claimed on the window, capture-phase, by the
   * project in front, as the shell's key is.
   *
   * **Not under a dialog**: a question the person is answering keeps every key. **Over a
   * terminal the chord is the window's whether or not its row can run**: none of the four is a
   * byte to a terminal, so nothing is kept from a chat, and a key that did something on one tab
   * and reached the program on the next would be worse. **In a text field or an editor a chord
   * whose row cannot run is left alone**: there it may be the field's own.
   */
  useEffect(() => {
    if (!inFront) return;
    const key = (e: KeyboardEvent) => {
      // The keys are the key registry's (#2921), with its rules: not under a dialog, and left to
      // a field when the row cannot run.
      const mac = onAMac();
      const which = taskKeyOf(e, mac);
      if (which === undefined) return;
      const offer = by(TASK_KEY_ROW[which]);
      const runnable = offer?.available === true;
      if (pressed(e, [TASK_KEY_ROW[which]], { mac, runnable }) === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat || offer?.available !== true) return;
      press(offer);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [by, inFront, press]);

  /**
   * **The keys that move between panes and tabs** (`moveKeys.ts`, #4767, ruled on #4699): ⌘⌥
   * and an arrow (Alt and an arrow off a Mac) gives the keyboard to the pane in that direction,
   * with no wrap; ⌘⇧] and ⌘⇧[ bring the next and previous tab on the strip forward, round its
   * ends, with the keyboard in its focused pane. Claimed on the window, capture-phase, by the
   * project in front, with the registry's rules: not under a dialog, and off a Mac Alt and an
   * arrow is left to a chat's terminal and to a view. Neither key is a byte to a terminal where
   * it is taken (`keys/harnessBytes.test.ts`). Nothing is announced: the keyboard follows.
   */
  useEffect(() => {
    if (!inFront) return;
    const key = (e: KeyboardEvent) => {
      const mac = onAMac();
      const hit = pressed(e, ["pane.focus", "tab.next", "tab.previous"], { mac });
      if (hit === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      const tabs = now.current;
      const front = tabs.inFront === undefined ? undefined : tabs.byId[tabs.inFront];
      if (front === undefined) return;
      if (hit.id === "pane.focus") {
        const toward = paneStepOf(e, mac);
        const to =
          toward === undefined ? undefined : paneToward(front.layout, front.focused, toward);
        if (to === undefined) return;
        keyboardInto(change((was) => focusPane(was, to)));
        return;
      }
      const strip = onStrip;
      const at = strip.indexOf(front.id);
      if (at < 0 || strip.length < 2) return;
      const step = hit.id === "tab.next" ? 1 : -1;
      const id = strip[(at + step + strip.length) % strip.length];
      keyboardInto(change((was) => selectTab(was, id)));
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [change, inFront, keyboardInto, onStrip]);

  /**
   * **The keys of the left side** (`sideKeys.ts`, #1673): ⌘B puts the navigation region away
   * or brings it back, ⌘J the whole Panel (#1737), ⌘⇧E, ⌘⇧C, ⌘⇧F and ⌃⇧G show Explorer, Chats, Search and Changes and give
   * the view the keyboard, as an editor does (⌘⇧F opened a Search tab before #1676). Claimed on the window, capture-phase, by the project in front, as the
   * shell's key is; not under a dialog; and off a Mac a chord a terminal has a use for is left
   * to a chat that has the keyboard.
   */
  useEffect(() => {
    if (!inFront) return;
    const key = (e: KeyboardEvent) => {
      // The keys and their rules are the key registry's (#2921): not under a dialog, and off a
      // Mac a chord a terminal has a use for is left to a chat that has the keyboard.
      const mac = onAMac();
      const which = sideKeyOf(e, mac);
      if (which === undefined) return;
      if (pressed(e, SIDE_KEY_IDS, { mac }) === undefined) return;
      // A view behind a preview that is off is nowhere to show (#3266): its key is not claimed.
      if ("show" in which && !drawable(which.show)) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat) return;
      // ⌘⇧F in Explorer's files does what its Search contents link says (#3343): Search on the
      // filter's scope, with its words.
      const here = explorerSearchKey.current;
      if (
        "show" in which &&
        which.show === "search" &&
        here !== undefined &&
        here.inside?.contains(document.activeElement) === true
      ) {
        here.run();
        return;
      }
      // **⌘⇧I begins keyboard triage** (ADR 0121 decision 9, #6016): Asks with the first ask's
      // lead focused, and where the keyboard was, to go back to.
      if ("show" in which && which.show === "asks") beginTriage(document.activeElement);
      if ("toggle" in which) toggleRegionSeen(which.toggle);
      else showSideViewSeen(which.show);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [drawable, inFront, showSideViewSeen, toggleRegionSeen]);

  /**
   * A pane's own button: that pane becomes the focused one, and then the row runs.
   *
   * **The row is the bar's row unchanged**, which is what keeps one list of actions. A split
   * and a close act on the focused pane (`tabs.ts`), and pressing a control ON a pane is the
   * operator saying "this one" — exactly what clicking anywhere in the pane already says. So
   * the target is not a new parameter on three catalogue rows; it is the focus, moved first.
   *
   * **The two are one update, not a race.** `change` is applied to a ref synchronously before
   * it reaches React state (see it above), so the row that runs on the next line reads the
   * pane this button belongs to. A `setState` here would leave the split acting on whatever
   * was focused before, which is the defect the operator is asking to be rid of.
   */
  /** The close of pane `pane` of the tab in front, decided for that pane (`paneCloseOf`). */
  const closeOfPane = useCallback(
    (pane: number) => paneCloseOf(tabs, pane, nameOf, askedBy, [...restoringChats]),
    [askedBy, nameOf, restoringChats, tabs],
  );
  const onPaneDoes = useCallback(
    (pane: number, offer: Offer | undefined) => {
      if (!offer?.available) return;
      change((tabs) => focusPane(tabs, pane));
      press(offer);
    },
    [change, press],
  );

  // **`scrollIntoView` on the selected tab is gone with the scroller.** It was how a chat
  // brought forward from somewhere that is not the strip — the palette, the needs-you queue,
  // a close taking the tab beside it — came back on screen. The strip does not scroll any
  // more, so there is nowhere to scroll it to; `fits.ts` draws the selected tab instead, and
  // that is the same promise kept by construction rather than by a side effect.

  // The chats still open, in the order the tab bar shows them: what a quit would end. What
  // each one is doing is resolved when the report is made, below, and has to be looked up in
  // this project, because a session number names a chat only inside its own project.
  const quitting = useMemo<Leaving[]>(
    () =>
      tabs.order.flatMap((id) => {
        const filed = workspaceOf(tabs, id, filedIn);
        return panesOf(tabs, id).map(({ session }) => {
          const known = reopened.find((chat) => chat.session === session);
          return {
            session,
            chat: {
              key: `${plane}#${session}`,
              project: plane,
              // As its row names it, and not as the harness does: this is a list the operator
              // reads (charter-app#254).
              name: nameOf(session),
              harness: known?.harness ?? null,
              cwd: known?.cwd ?? null,
              workspace: filed === OUTSIDE ? OUTSIDE_TITLE : filed,
            },
          };
        });
      }),
    [filedIn, nameOf, plane, reopened, tabs],
  );

  // **This project's chats asking, for the title bar's list** (charter-app#249), which is the
  // window's and holds every project's. Their rows are the catalogue's own (`needsYouRows`),
  // asked on their own because the catalogue is built only for the project in front.
  //
  // **A chat whose Smart close stopped without a record** (SI-8f) is no item here: it is an
  // update in the Inbox (#1693). The palette keeps its Show and Dismiss rows (`stoppedRows`).
  //
  // **Asked with the queue as this view reports, and not as it draws** (#1034), with the
  // catalogue's queue rows: the queue changes whenever a chat starts or stops asking for you,
  // and nothing this view draws but the counts reads it.
  const queued = useCallback(
    (queue: readonly number[]): Pick<PlaneReport, "asking" | "offers"> => {
      const reportsTo = (session: number) => reports[session] ?? [];
      const refusedIn = (session: number) => refusals[session] ?? [];
      const neededFor = (session: number) => needs[session] ?? [];
      const rows = catalogued([
        ...needsYouRows(
          queue,
          nameOf,
          tabs,
          reportsTo,
          refusedIn,
          neededFor,
          (session) => listedChats.some((chat) => chat.session === session),
          (session) => stoppedBelow[session] ?? [],
        ),
        ...stoppedRows(stopped, queue, nameOf, tabs),
      ]);
      const item = (session: number, ignore: string): Asking => {
        const filed = filedIn(session);
        const persona = personaOf(session);
        return {
          session,
          name: nameOf(session),
          persona,
          mark: persona === null ? null : (personaMarks.marks.get(persona) ?? null),
          reported: reportsTo(session),
          stoppedBelow: stoppedBelow[session] ?? [],
          // What the app found the chat needs you for is said first (#1448).
          needed: neededFor(session)[neededFor(session).length - 1],
          // A refused commit (SQ-16) says its latest.
          why: refusedIn(session)[refusedIn(session).length - 1],
          workspace: filed === OUTSIDE ? OUTSIDE_TITLE : filed,
          go: rows.get(showId(session)),
          ignore: rows.get(ignore),
        };
      };
      return {
        asking: queue.map((session) => item(session, ignoreId(session))),
        offers: withQueue(offers, queue, {
          tabs,
          nameOf,
          reportsTo,
          refusedIn,
          neededFor,
          listed: listedChats,
          stoppedBelow: (session) => stoppedBelow[session] ?? [],
          stopped,
        }),
      };
    },
    [
      filedIn,
      listedChats,
      nameOf,
      needs,
      offers,
      personaMarks.marks,
      personaOf,
      refusals,
      reports,
      stopped,
      stoppedBelow,
      tabs,
    ],
  );

  /**
   * **One queued chat's own rows** (`needs.show:<session>`, `needs.ignore:<session>`), by id:
   * the Inbox's Go to chat and Ignore (#1692). They are not in this view's catalogue, which
   * holds no queue rows (`queued` builds them for the window), so they are built here as
   * `queued` builds them.
   */
  const queueRow = useCallback(
    (id: string, session: number): Offer | undefined =>
      catalogued(
        needsYouRows(
          [session],
          nameOf,
          tabs,
          (one) => reports[one] ?? [],
          (one) => refusals[one] ?? [],
          (one) => needs[one] ?? [],
          (one) => listedChats.some((chat) => chat.session === one),
          (one) => stoppedBelow[one] ?? [],
        ),
      ).get(id),
    [listedChats, nameOf, needs, refusals, reports, stoppedBelow, tabs],
  );
  /** Go to chat from Asks or Updates: a chat in the queue goes as the hand's Go did, by the
   *  catalogue's own row, which knows a failed task's row and a report with nowhere to go
   *  (#1448, #1491). */
  const goToAsking = useCallback(
    (session: number) => {
      const go = queueRow(showId(session), session);
      if (go?.available) press(go);
      else showChat(session);
    },
    [queueRow, press, showChat],
  );
  useEffect(() => {
    goingToAsking.current = goToAsking;
  }, [goToAsking]);

  // What this project has open, told to the window: the quit warning lists every project's
  // chats, and this project's own tab says when one of them needs you.
  //
  // **Its catalogue travels with it, and that is what keeps the palette one palette.** The
  // palette is mounted on the WINDOW — always, so `F2` reaches it before the core has said
  // which project this launch opened, and once, so a project that is not in front is not a
  // second capturing listener for the same key. What it lists has to be the project in
  // front's, and this is how it gets there.
  //
  // **Without what its chats are doing**, which is added as it is reported, below: that changes
  // with every move, and a report rebuilt here for each one was the whole view redrawn for each.
  // **Nor the queue** (#1034), for the same reason: what reads it is added there too (`queued`).
  const mine = useMemo<
    Omit<PlaneReport, "ending" | "asking" | "offers" | "workspaces"> & {
      workspaces?: Omit<ReportedWorkspaces, "waiting">;
    }
  >(
    () => ({
      quiet: quietHere,
      settled,
      run,
      // Its stores, which All chats' rows in this window read a working chat's line from (#6013).
      lent: { chats, doings },
      said: report,
      // Whether the plane has been read: until it has, the window does not know which
      // workspace's theme to draw (`App.tsx`'s `settledInFront`).
      read: sidebar !== undefined,
      // Which workspace, by name, and its colour: the window draws that workspace's theme and
      // tints its accent with that colour while this project is in front (charter-app#281).
      workspace: ofWorkspace,
      colour: colourWithHue(sidebar?.workspaces.find((ws) => ws.name === ofWorkspace)?.colour),
      saving,
      savingRead,
      readOnly: readOnlySaid,
      // The level of the chat in front, for the title bar (#2071).
      sandbox: tabs.inFront === undefined ? null : sandboxOf(chatOf(tabs, tabs.inFront)),
      // Where ⌘P's files are found first (FM-7): the cockpit's branch, else the one the
      // explorer picked (FM-5).
      branch: nearBranch,
      // Its workspaces, for the title bar's chips and switchers (#2916): every one but the
      // root in the order the palette lists them, the pins first, and the one in front.
      workspaces:
        sidebar === undefined
          ? undefined
          : {
              names: strips.filter((name) => name !== OUTSIDE),
              pins: strips.filter((name) => pinnedWorkspaces.includes(name)),
              // The pins whose workspace is gone, which the title bar draws dormant (#3651).
              dormant: dormantPins,
              forget: (name: string) => void forgetDormantPin(name),
              // A chip's drag (#3128): the pins in the order they were dropped in.
              arrange: (order: readonly string[]) => void arrangePins(order),
              // Its stale repos, for their workspaces' chips (#3650).
              standing: staleClones.lines,
              focused,
            },
    }),
    [
      arrangePins,
      chats,
      doings,
      dormantPins,
      forgetDormantPin,
      staleClones.lines,
      focused,
      pinnedWorkspaces,
      strips,
      nearBranch,
      ofWorkspace,
      quietHere,
      readOnlySaid,
      report,
      run,
      sandboxOf,
      saving,
      savingRead,
      settled,
      sidebar,
      tabs,
    ],
  );
  // **Before the paint, not after it.** A quit — Cmd-Q, the tray, the menu — arrives whenever
  // it arrives, and the window decides on what every project has told it: a report that
  // landed a frame late would let a quit warn about nothing, or worse, end a chat it had not
  // heard about yet. `useLayoutEffect` puts this in the same flush as the render that
  // produced it, which is the nearest thing to the synchronous ref the single-project window
  // used before there was anything to report to.
  //
  // **And again whenever a chat moves, straight off the store** (SC-3): what each chat is doing
  // is the report's, so the quit warning stays current, and this view is not redrawn to say so.
  // A move that changes nothing the report holds is not reported at all.
  //
  // **When a chat moved is not the report's** (#4727): a busy chat moves many times a second,
  // and each report is the window's root drawn again. What ordered projects by it was the
  // project strip's show-more, which went with the strip (ADR 0054, amended 2026-10-23).
  /** **Every chat the plane lists, by the workspace it files it in** (`OUTSIDE` for the root):
   *  what a chip's working mark counts (#3127). */
  const filedChats = useMemo(
    () => [
      ...(sidebar?.workspaces ?? []).flatMap((ws) =>
        ws.chats.map((chat) => ({ session: chat.session, workspace: ws.name })),
      ),
      ...(sidebar?.unfiled ?? []).map((chat) => ({ session: chat.session, workspace: OUTSIDE })),
    ],
    [sidebar],
  );
  /** Its sessions, for the status line's count of the ones running. */
  const quitSessions = useMemo(() => quitting.map((chat) => chat.session), [quitting]);
  // **Who it was sent to, and for which project, are part of what was sent** (#1037): a new
  // callback, or this view told it is another project, has heard nothing yet, so it is told once
  // even when nothing else moved.
  const reported = useRef<
    | {
        to: typeof onReport;
        plane: PlaneId;
        mine: typeof mine;
        waitingIn: typeof waitingIn;
        quitting: Leaving[];
        doing: State[];
        working: Readonly<Record<string, number>>;
        owning: (OwnWork | undefined)[];
        turning: boolean[];
        queued: typeof queued;
        queue: readonly number[];
        fromQueue: ReturnType<typeof queued>;
        fleetSaid: string;
      }
    | undefined
  >(undefined);
  useLayoutEffect(() => {
    const report = () => {
      const states = chats.store.statesFor(chats.plane);
      const doing = quitting.map((chat) => stateOf(states, chat.session));
      // What each waits on of its own (#3878): the quit warning and Restart to update say it
      // ends (#5748).
      const owning = quitting.map((chat) => ownWorkOf(states, chat.session));
      // Which a background turn runs in (#4562): ending it interrupts that turn (#5782).
      const turning = quitting.map((chat) => states.backgroundTurns[chat.session] === true);
      const queue = states.needsYou;
      // **How many chats are working in each workspace** (#3127): its chip's mark. A chat in its
      // turn, its own work in flight included (`inItsTurn`), as a hook says it (ADR 0076). Every
      // chat the plane lists, where it files it: a chat need not have a tab (#4758).
      const working: Record<string, number> = {};
      for (const { session, workspace } of filedChats)
        if (inItsTurn(states, session)) working[workspace] = (working[workspace] ?? 0) + 1;
      // **Its chats, as All chats lists them** (#6013): read off the rows its Chats list draws
      // and its clock, told again only when what they say changed.
      // The clock read first, so a chat's time is the one this move gave it, in this same report.
      chatClock.read(states, chatRows, Date.now(), rowsOnScreen.current);
      const fleet = fleetOf({
        plane,
        project: projectName(plane),
        rows: chatRows,
        states,
        since: new Map(
          chatRows.flatMap((row) => {
            const at = chatClock.since(row.session);
            return at === null ? [] : [[row.session, at] as const];
          }),
        ),
        asks: waiting ?? [],
      });
      const fleetSaid = JSON.stringify(fleet);
      const was = reported.current;
      const sameQueue = was?.queued === queued && sameList(was.queue, queue);
      if (
        was?.to === onReport &&
        was.plane === plane &&
        was.mine === mine &&
        was.waitingIn === waitingIn &&
        was.quitting === quitting &&
        sameList(was.doing, doing) &&
        sameCounts(was.working, working) &&
        // The same length as `doing`'s, each built off `quitting`, which is the same.
        was.owning.every((work, at) => sameOwnWork(work, owning[at])) &&
        sameList(was.turning, turning) &&
        was.fleetSaid === fleetSaid &&
        sameQueue
      )
        return;
      // The queue's rows are built again only when the queue, or what words them, changed.
      const fromQueue = sameQueue ? was.fromQueue : queued(queue);
      reported.current = {
        to: onReport,
        plane,
        mine,
        waitingIn,
        quitting,
        doing,
        working,
        owning,
        turning,
        queued,
        queue,
        fromQueue,
        fleetSaid,
      };
      // Each workspace's count, the root's under its own name, read off the one queue.
      const workspaces =
        mine.workspaces === undefined
          ? undefined
          : {
              ...mine.workspaces,
              waiting: Object.fromEntries(
                [OUTSIDE, ...mine.workspaces.names].map((name) => [name, waitingIn(queue, name)]),
              ),
              working,
            };
      onReport(plane, {
        ...mine,
        workspaces,
        ...fromQueue,
        fleet,
        ending: quitting.map(({ chat }, at) => ({
          ...chat,
          state: doing[at],
          ...(owning[at] && { ownWork: owning[at] }),
          ...(turning[at] && { backgroundTurn: true }),
        })),
      });
    };
    report();
    return chats.store.subscribe(report);
  }, [
    chats,
    chatClock,
    chatRows,
    filedChats,
    quitting,
    mine,
    onReport,
    plane,
    queued,
    waiting,
    waitingIn,
  ]);

  const frontChat = frontTab && reopened.find((chat) => chat.session === chatOf(tabs, frontTab.id));

  /**
   * **Files into chats** (FM-9): every chat of this project, for the preview's *Ask a chat about
   * this* and *Add to a chat's context*, and the one way a reference reaches a chat — a drop on
   * its tab or pane included. The core types it, unsent, or copies it and says why; what it
   * answered is said where every row's answer is.
   */
  const referenceChats = useMemo<ChatsForReferences>(() => {
    // A task that ended in its pane stays there as ended (#4757), and is no open chat: the
    // sidebar, once read, is what says which are.
    const chatsOpen = tabs.order.flatMap((id) =>
      panesOf(tabs, id)
        .filter((pane) => sidebar === undefined || chatsByNumber.has(pane.session))
        .map((pane) => ({ session: pane.session, name: nameOf(pane.session) })),
    );
    return {
      plane,
      chats: chatsOpen,
      hand: (r, session, how) => {
        // Named as its row names it: a task shown in a tab is not one of the tabs' own chats.
        const name =
          chatsOpen.find((chat) => chat.session === session)?.name ??
          nameOfListed(session) ??
          `chat ${session}`;
        void handReference(plane, session, name, r).then((ran) => {
          setReport(
            ran.ok
              ? { from: "reference", refused: false, words: ran.said ?? "" }
              : { from: "reference", refused: true, words: ran.refused },
          );
          if (how === "ask" && ran.ok) showChat(session);
        });
      },
      // "Start a chat here" from a file's preview, with its lines (#1151): the row's own act.
      start: (r) => {
        void startChatHere(
          { workspace: r.workspace, repo: r.repo, piece: r.piece, path: r.path },
          r.lines,
        ).then((ran) => {
          // A chat that started says nothing more: its tab is in front. A refusal keeps its
          // Approve and start… (#3968).
          if (ran.ok && ran.said === undefined) return;
          setReport(saidOf("file.chat", ran));
        });
      },
    };
  }, [chatsByNumber, nameOf, nameOfListed, plane, showChat, sidebar, startChatHere, tabs]);

  /** **While you were away** (#1514): kept for this project whether or not it is in front,
   *  so a project behind another has its own summary when the person switches to it. */
  const { away: awayOn } = useChatsListPrefs();
  const awayNow = useAwaySummary({
    plane,
    chats,
    on: awayOn,
    finished: finishedTasks,
    nameOf,
    refusedAway: awayRefused,
  });

  /**
   * **The Inbox's updates** (#1693): derived from the sources that already say each, noted
   * and kept a day on this machine whether or not the project is in front, and drawn after the
   * asks with what each source offers on it.
   */
  const derivedUpdates = useMemo(() => {
    const at = seenAt();
    return [
      ...taskUpdates([...finishedTasks.values()].flat(), nameOf, at),
      ...resumeUpdates(reopened, at),
      ...sandboxUpdates(sandboxBlocks, nameOf, at),
      ...awayUpdates(awayRefused ?? []),
      ...smartCloseUpdates(stopped, nameOf, at),
      ...reasonUpdates(foundFor, refusals, nameOf, at),
      ...browserAskUpdates(browserAsksEnded, nameOf),
      ...reportSentUpdates(sentDrafts),
      // A chat that reads Stuck? (#6014): a Should-know line while it does.
      ...stuckUpdates(noticedChats, nameOf, at),
      // A chat another chat started, announced and not looked at yet (#4759).
      ...arrivedUpdates(
        sidebar === undefined
          ? []
          : [...sidebar.unfiled, ...sidebar.workspaces.flatMap((ws) => ws.chats)],
        nameOf,
        at,
      ),
    ];
  }, [
    sidebar,
    browserAsksEnded,
    sentDrafts,
    noticedChats,
    finishedTasks,
    nameOf,
    reopened,
    sandboxBlocks,
    awayRefused,
    stopped,
    foundFor,
    refusals,
  ]);
  const {
    updates: keptUpdates,
    settle: settleUpdates,
    setAside,
  } = useInboxUpdates(plane, derivedUpdates);
  /** **The first-open baseline** (#5648, ADR 0119): a fresh clone opens quiet. */
  const firstOpen = useFirstOpen({ plane, report: doctor.report, setAside, dismiss });
  /** **Updates' and the Notices' Dismiss, held for their Undo** (#5650): each view's own. */
  const updatesHold = useDismissHold();
  const noticesHold = useDismissHold();
  /** The bell's count (#5646): the unread lines From your chats, those just dismissed left out. */
  const bell = unreadMine(keptUpdates?.filter((one) => !updatesHold.held.has(heldUpdate(one.key))));
  /** **What is New** (#6002, ADR 0121 decisions 10 and 11): seen, not received; the chat on
   *  screen in a focused window has nothing New. */
  const { seen, see, rows: seenRows } = useSeen(plane, waiting);
  /** **The other projects' seen records** (#1921), for their asks listed in this one's Asks. */
  const seenOf = useSeenElsewhere(theirAsks(plane, elsewhere).map((one) => one.plane));
  useChatOnScreenSeen(see, chatOnScreen, inFront, waiting, keptUpdates);
  const [noticeCauses, setNoticeCauses] = useState<readonly string[]>([]);
  useEffect(() => {
    if (inFront) onSeen?.(plane, seenRows);
  }, [inFront, onSeen, plane, seenRows]);

  /**
   * **The chats in no pane** (#4759, spec #4750): listed, and in no tab of any window of the
   * project. What each has to say is said in the Notices centre (`NoticeInCentre`), and its row
   * is marked while it has something there.
   *
   * **The window showing a chat owns its notes** (#5225): a chat shown only in a tab window says
   * them on its pane's band there, where the person is reading it, and never here too. A tab
   * window has no Notices centre, so it lists none: the project window's is the one.
   */
  const paneless = useMemo(
    () =>
      tabWindow !== undefined
        ? []
        : [...chatNames.keys()].filter(
            (session) => tabHolding(tabs, session) === undefined && !shownElsewhere.has(session),
          ),
    [chatNames, shownElsewhere, tabWindow, tabs],
  );
  /** The causes of the Notices each chat in no pane has standing in the Notices centre. */
  const [centred, setCentred] = useState<ReadonlyMap<number, ReadonlySet<string>>>(() => new Map());
  /** Chat `session`'s Notice `cause` came to the Notices centre, or went (`NoticeInCentre`). */
  const centredHeld = useCallback(
    (session: number, cause: string, held: boolean) =>
      setCentred((was) => {
        const now = new Set(was.get(session) ?? []);
        if (held === now.has(cause)) return was;
        if (held) now.add(cause);
        else now.delete(cause);
        const next = new Map(was);
        if (now.size === 0) next.delete(session);
        else next.set(session, now);
        return next;
      }),
    [],
  );
  /** The rows marked for a note in the Notices centre (#4759): a chat in no pane with one. */
  const noted = useMemo(() => new Set(centred.keys()), [centred]);
  /** Whether the registry lists an ask of chat `chat` now (#1694): its question or the prompt
   *  in its terminal is in the same needs-you item as what the app found for it. */
  const asksOfChat = useCallback(
    (chat: number) => (waiting ?? []).some((ask) => ask.session === chat),
    [waiting],
  );
  /**
   * **An update of what the app found puts the chat's needs-you item away with it** (#1694),
   * as the queue's own Ignore does, **unless the chat also asks something**: that item is then
   * its ask too, and putting an update away never answers or silences an ask, alone or in
   * Dismiss all.
   */
  const ignoreForUpdate = useCallback(
    (chat: number) => {
      if (asksOfChat(chat)) return;
      const ignore = queueRow(ignoreId(chat), chat);
      if (ignore?.available) press(ignore);
    },
    [asksOfChat, press, queueRow],
  );
  const inboxUpdates = useMemo(() => {
    const tasks = new Map(
      [...finishedTasks.values()].flat().map((task) => [`task:${task.id}`, task]),
    );
    const away = new Map((awayRefused ?? []).map((one) => [awayUpdateKey(one), one]));
    const open = (session: number | null) =>
      session !== null && tabHolding(tabs, session) !== undefined ? session : undefined;
    const rows = keptUpdates?.map((update): UpdateRow => {
      const read = () => settleUpdates([update.key], "read");
      const put = () => settleUpdates([update.key], "dismissed");
      const task = tasks.get(update.key);
      // A chat that arrived is in no pane (#4759): Go to it shows it while it is listed.
      const session =
        update.kind === "arrived" && update.session !== null && chatNames.has(update.session)
          ? update.session
          : open(update.session);
      const go =
        task !== undefined
          ? () => (showFinished(task), read())
          : session !== undefined
            ? () => (showChat(session), read())
            : undefined;
      const refused = away.get(update.key);
      if (update.kind === "refused-away" && refused !== undefined)
        return { update, ...awayRow(refused, (one, how) => onAway?.(plane, one, how), put) };
      const holding =
        update.kind === "report-sent"
          ? sentDrafts.find((one) => reportSentKey(one.id) === update.key && one.holding)
          : undefined;
      if (holding !== undefined)
        return {
          update,
          go,
          read,
          dismiss: put,
          // Always send's hold (#5695): Undo sends nothing, and the draft comes back as a card.
          answers: [
            {
              label: "Undo",
              name: `Undo: don't send ${holding.title}`,
              title: "Don't send it. The draft comes back for you to read.",
              press: () =>
                void undoAlwaysSend(holding.id).then((refused) => {
                  if (refused === undefined) {
                    setSentDrafts((was) => was.filter((one) => one.id !== holding.id));
                    put();
                  }
                }),
            },
          ],
        };
      if (update.kind === "stuck" && update.session !== null) {
        // "It's fine" (#6014): the core clears it until the chat changes state, and the line
        // goes as its source stops listing it.
        const stuckChat = update.session;
        return {
          update,
          go,
          read,
          dismiss: put,
          answers: [
            {
              label: "It's fine",
              name: `It's fine: ${update.chain[0] ?? "this chat"}`,
              title: "It isn't stuck. Asked again only once the chat changes state.",
              press: () => void commands.stuckFine(plane, stuckChat).then(read),
            },
          ],
        };
      }
      if (update.kind === "smart-close" && update.session !== null) {
        const stoppedChat = update.session;
        return { update, go, dismiss: () => (stoppedFor(stoppedChat, undefined), put()) };
      }
      if (
        (update.kind === "report-undelivered" || update.kind === "commit-refused") &&
        update.session !== null
      ) {
        // What the app found is put away with the chat's needs-you item, as the queue's own
        // Ignore puts it away (#1694); Go goes as the queue's Go does (#1448).
        const chat = update.session;
        const shown = queueRow(showId(chat), chat);
        const asking = asksOfChat(chat);
        return {
          update,
          go: shown?.available ? () => (press(shown), read()) : go,
          dismiss: () => {
            ignoreForUpdate(chat);
            put();
          },
          dismissSays: asking
            ? "Put this update away. What the chat asks stays"
            : "Put this update away, and the chat's needs-you item with it",
        };
      }
      // A finished task opens in place to show its report, or why it failed (#5651).
      const report = task?.report.trim() ? task.report : undefined;
      return { update, go, report, read, dismiss: put };
    });
    return {
      rows,
      opened: openedUpdate,
      // The lines of the list shown (#5646): Mine's or Project's, never the other's.
      onMarkAllRead: (keys: readonly string[]) => settleUpdates(keys, "read"),
      onDismissAll: (keys: readonly string[]) => {
        const these = new Set(keys);
        // Each update a source lists is put away there too, as its own Dismiss would.
        for (const one of keptUpdates ?? []) {
          if (!these.has(one.key) || !LIVE.has(one.kind)) continue;
          const refused = away.get(one.key);
          if (refused !== undefined) onAway?.(plane, refused, "dismiss");
          if (
            one.kind === "smart-close" &&
            one.session !== null &&
            stopped[one.session] !== undefined &&
            smartCloseKey(one.session, stopped[one.session]) === one.key
          )
            stoppedFor(one.session, undefined);
          if (
            (one.kind === "report-undelivered" || one.kind === "commit-refused") &&
            one.session !== null
          )
            ignoreForUpdate(one.session);
        }
        settleUpdates(keys, "dismissed");
      },
    };
  }, [
    keptUpdates,
    settleUpdates,
    sentDrafts,
    finishedTasks,
    awayRefused,
    chatNames,
    tabs,
    showFinished,
    showChat,
    onAway,
    plane,
    stoppedFor,
    stopped,
    queueRow,
    press,
    asksOfChat,
    ignoreForUpdate,
    openedUpdate,
  ]);
  /** Asks, shown (#5644): where a vault's request and a profile's sign-in wait. */
  const showInbox = useCallback(() => showSideView("asks"), [showSideView]);
  /** Updates, shown (#5644): where the away summary's refused dispatches are answered (#1693). */
  const showUpdates = useCallback(() => showSideView("updates"), [showSideView]);
  /** Notices, shown (#5645): the project's Notices, in the Panel's right half, under Asks. */
  const showNotices = useCallback(() => showSideView("notices"), [showSideView]);
  /** A runner's branch ready to push, from the away summary (#2051): its workspace in front,
   *  and Changes, where it is taken here and pushed. */
  const showChangesOf = useCallback(
    (workspace: string) => {
      focusWorkspace(workspace);
      showSideView("changes");
    },
    [focusWorkspace, showSideView],
  );
  /**
   * **An answer given on a pane is listed in the Inbox once its ask goes** (#1700, 2026-10-10
   * 21:09 item 2), as one given in the Inbox is: a pane's Notice says the words pressed and the
   * ask they answered (`onWay`), and the store notes it once the registry no longer lists that
   * ask. A capturing wrapper, as the Inbox's, cannot be used on a pane: its Notice stack manages
   * its own direct children. Held here, by the project, and not by the pane: an Allow restarts
   * its chat in a new session, and the pane moves on to it before the ask is read as gone.
   */
  const touchOnPane = useAnsweredWhenGone(plane, waiting);
  /** The ✋'s count: every open project's asks, and how many are outside this one (#5999). */
  const hand = handCount(plane, waiting?.length ?? 0, elsewhere);
  /** **A Blocking ask arriving** (ADR 0121 decisions 24 and 25, #6020): said once, politely,
   *  and the first one on this machine that arrives while Asks is closed pulses the ✋. */
  const arrival = useAsksArrival(waiting, hand.elsewhere, inFront && asksShown);
  /**
   * **Snooze all on Asks** (#5647): the core keeps what each chat held, and lists a snoozed
   * chat's asks apart until it asks something new, or until the time it was snoozed for runs out
   * (#6018); every list of this project's asks is read
   * again after, so the ✋ and Asks drop them, or take them back, at once.
   */
  const snoozedAsks = questions?.snoozed;
  const snoozedUntil = questions?.snoozed_until ?? null;
  const snoozing = useMemo<Snoozing>(() => {
    const done = (said: { status: "ok" } | { status: "error"; error: string }) => {
      asksMoved(plane);
      return said.status === "error" ? said.error : undefined;
    };
    const unsaid = (err: unknown) => ({ status: "error" as const, error: String(err) });
    return {
      snoozed: snoozedAsks ?? [],
      snoozedUntil,
      snooze: async (chats, until) =>
        done(await commands.snoozeInboxAsks(plane, chats, until).catch(unsaid)),
      wake: async (sessions) =>
        done(await commands.wakeInboxAsks(plane, sessions ?? null).catch(unsaid)),
    };
  }, [plane, snoozedAsks, snoozedUntil]);
  /** **The asks a cool-off holds quietly** (#6010), as the core lists them apart. */
  const coolingAsks = questions?.cooling;
  const coolingUntil = questions?.cooling_until ?? null;
  const cooling = useMemo(
    () => ({ asks: coolingAsks ?? [], until: coolingUntil }),
    [coolingAsks, coolingUntil],
  );
  /**
   * **Stop on a row of Asks** (ADR 0121 decision 21, #6013): the chat's own Stop, as its row in
   * the Chats list and the palette press it: a task's lets it report first.
   */
  const stopFromARow = useCallback(
    (session: number) => {
      const stop = found.get(stopId(session)) ?? found.get(taskStopId(session));
      if (stop?.available) press(stop);
    },
    [found, press],
  );
  /**
   * **All chats** (#6013): the Chats sidebar's mode, and the one state it is filtered to.
   * {@link openAllChats} is the way in for anything that names a state (#6020's empty Asks:
   * "7 chats working" opens All chats on Working, from `everyChat.countsByState`).
   */
  const [allChats, setAllChats] = useState<{ on: boolean; filter?: ChatState }>({ on: false });
  const openAllChats = useCallback(
    (filter?: ChatState) => {
      setAllChats({ on: true, filter });
      showSide("chats");
    },
    [showSide],
  );
  /** Where a part of the empty Asks leads (ADR 0121 decision 23): All chats, filtered. */
  const openAllChatsFrom = useCallback(
    (filter: AllChatsFilter) => openAllChats(filter === "all" ? undefined : filter),
    [openAllChats],
  );
  const answeredOnPane = useCallback(
    (words: string, ask: Shown) => touchOnPane(ask, words),
    [touchOnPane],
  );
  /** Notices brought on screen with the keyboard left where it is: a Notice answering the
   *  person's own press arrived in it (D-1695-3), and a Result with it (#5645). */
  const inboxOnScreen = useCallback(() => showSide("notices"), [showSide]);

  /** This machine's alerts (`windowprefs.ts`), listed in every project's Inbox. */
  const aboutThisMachine = useAboutThisMachine();
  /** How many Notices this project's Inbox lists, for the status line's button. */
  const [noticesListed, setNoticesListed] = useState(0);
  /** The Notices icon's dot (#5645; ADR 0121 decision 3, #6004): red for something broken,
   *  amber for a decision that stops no chat, none otherwise. */
  const [noticesDot, setNoticesDot] = useState<NoticesDot>();
  /**
   * **The status line's Notices button** (#1695): what the Inbox lists that is no ask, opened by
   * a press. No number where purlis cannot stand behind one (`StatusLine`): until this
   * project's alerts are read, or where purlis stopped looking for them.
   */
  const noticesButton = useMemo<Alerts | undefined>(() => {
    if (alerts === undefined) return undefined;
    const reading = alerts.reading;
    const read =
      reading.at === "read" ? reading.planes.find((one) => one.plane === plane) : undefined;
    const known = reading.at === "failed" || (read !== undefined && read.stopped === null);
    // Opening it reads the alerts again, so what it lists is what is true when it is looked at.
    const { reread } = alerts.does;
    return {
      count: known ? noticesListed + (windowLines?.count ?? 0) : undefined,
      open: () => {
        reread();
        showNotices();
      },
    };
  }, [alerts, plane, noticesListed, showNotices, windowLines?.count]);

  /** The element this project draws into, in the window only while it is in front. */
  const host = useProjectHost();
  /**
   * **A workspace switch slides the workspace in** (#2916, R114): its side panel, its work and
   * docked tabs, its right panel and its bottom halves, which are the regions; the activity bars
   * beside them stay still, as do the rail and the title bar. The way is the step's that asked,
   * or else the project's order (`workspaceSlide.ts`). Only a switch in the project in front:
   * coming to the front is the project's, not a workspace switch.
   */
  const slidFrom = useRef<string | undefined>(undefined);
  useLayoutEffect(() => {
    const from = slidFrom.current;
    slidFrom.current = focused;
    if (!inFront || sidebar === undefined) return;
    if (from === undefined || focused === undefined || from === focused) return;
    slideIn(
      host.querySelector<HTMLElement>(".region-frame > .regions"),
      takeSlide() ?? wayOf(strips, from, focused),
    );
  }, [focused, host, inFront, sidebar, strips]);
  /**
   * Whether its view is drawn. **Only coming to the front draws it**; it stays drawn behind
   * another project while it may (`kept`), and is taken down once it may not, or as it leaves
   * the front with something drawn over the window (`drawnOverTheWindow`). So a project the
   * window opens behind the front one draws nothing until the person goes to it.
   */
  const [drawn, setDrawn] = useState(inFront);
  if (inFront && !drawn) setDrawn(true);
  if (!inFront && drawn && !kept) setDrawn(false);
  const leftCovered = useCallback(() => setDrawn(false), []);

  // A project the operator has not been in lately keeps every piece of state above and draws
  // none of it. One they were in lately stays drawn, off the window: see this module's own
  // docstring.
  if (!inFront && !(drawn && kept)) return null;

  /**
   * **The project's standing lines, in its Inbox** (#1695, I-4): what stood under the tab strip
   * (NO-1 to NO-6, V91a–d) is listed in the Inbox's Notices, the most important first, each with
   * its ways out. Every one is a `Notice`, and `Notice.guard.test.ts` fails on one drawn any
   * other way. A refused action's line has Dismiss, and goes by itself when that action next
   * succeeds (NO-4).
   */
  /** What may yet list a Notice in the Inbox, each with its own mark there (#5325). */
  const inboxReading = [
    ...(staleClones.reading ? ["Checking repos for a stale branch or address…"] : []),
    ...(alerts?.reading.at === "reading" ? ["Reading the alerts…"] : []),
  ];
  /**
   * **What teammates changed in the project** (#5646, ADR 0119): its hosts and browser sites,
   * its Internet access presets and its dispatch grants. Listed under Updates' Project, never
   * among the Notices and never counted: each is told once, and asks the person nothing that
   * blocks a chat of theirs. purlis has no Notice of the purlis version a project expects yet.
   */
  // Drawn once the first-open baseline is answered: at a fresh open the core writes their seen
  // records first, so a teammate's past change is never told as new (#5648).
  const fromTheProject = firstOpen.ready ? (
    <>
      {/* **The project's own hosts changed** (#1341): told once to each teammate. With them,
          the project's browser sites (#2443), each told before it reaches their browsers. */}
      <ProjectHostsNotice
        plane={plane}
        onReview={() => openSettingsAt({ group: "project.sandbox" })}
      />
      {/* **The project's Internet access presets changed** (#1385): told once to each
          teammate, so a preset that widens never widens unseen. */}
      <ProjectPresetsNotice
        plane={plane}
        onReview={() => openSettingsAt({ group: "project.sandbox" })}
      />
      {/* **A teammate's dispatch grant arrived** (#1506): said here, at the window's level,
          when the project's settings gain one this person has not answered. */}
      <ProjectDispatchNotice
        plane={plane}
        onReview={() => openSettingsAt({ group: "project.dispatch" })}
      />
    </>
  ) : undefined;
  const standing = (
    <>
      {trouble && (
        <Notice
          cause={`window-trouble:${trouble.from}`}
          tone="trouble"
          onDismiss={() => setTrouble(undefined)}
        >
          {trouble.said}
        </Notice>
      )}

      {/* What happened to the chat in front when it was put back. Only a chat that came from
          the record has either, so a chat the operator just opened says nothing. */}
      {/* Both notes name the chat by what its tab says — `frontTab.name`, the one field the
          strip prints — and not by its recorded number, which the operator never reads
          ("5 came back" beside a tab that says "steward 5"). */}
      {frontTab && frontChat?.resumed && !dismissed.has(resumedNote(frontChat)) && (
        <Notice cause={resumedNote(frontChat)} onDismiss={() => dismiss(resumedNote(frontChat))}>
          {/* No conversation id (#1646): it means nothing to the person, and the session
                record and Activity keep it. The cause still names it (D-NO2-9). */}
          <strong>{frontTab.name}</strong> was resumed where it left off
        </Notice>
      )}
      {/* A chat reattached in the session host whose state the host could not hand over
          (#2006): what the window shows of it is right again from its next report. Why is in
          the log, not here. */}
      {frontTab && frontChat?.untaken && !dismissed.has(`chat-untaken:${frontChat.session}`) && (
        <Notice
          cause={`chat-untaken:${frontChat.session}`}
          onDismiss={() => dismiss(`chat-untaken:${frontChat.session}`)}
        >
          <strong>{frontTab.name}</strong> is back, but purlis could not bring back what it was
          doing while the window was closed. Its state here is right again from its next report.
        </Notice>
      )}
      {/* What a Resume from a session record had to guess because the record could not say
          it — its profile, its directory (SI-8e) — said beside what happened, never instead. */}
      {frontTab && frontChat?.guessed && !dismissed.has(`chat-guessed:${frontChat.session}`) && (
        <Notice
          cause={`chat-guessed:${frontChat.session}`}
          onDismiss={() => dismiss(`chat-guessed:${frontChat.session}`)}
        >
          <strong>{frontTab.name}</strong>: {frontChat.guessed}.
        </Notice>
      )}
      {/* Only for a harness. Every chat is a shell until the harness picker lands, and a
          shell has no conversation to bring back — saying so on every relaunch, forever,
          is noise about the normal case. */}
      {frontTab &&
        frontChat?.fresh &&
        frontChat.harness &&
        !dismissed.has(`chat-fresh:${frontChat.session}`) && (
          <Notice
            cause={`chat-fresh:${frontChat.session}`}
            onDismiss={() => dismiss(`chat-fresh:${frontChat.session}`)}
          >
            <strong>{frontTab.name}</strong> came back as a new chat: {frontChat.fresh}
          </Notice>
        )}

      {/* **A smart close that ended on its record** (SI-8f): its tab has gone, so this is where
          the window says so — quietly, as news and not as a question, with the record one
          press away in its own view tab (SI-8d). */}
      {/* **The sandbox's one-time offer** to a project made before it (ADR 0067 §1, V21 1):
          a notice like the one below, answered once, never a dialog. */}
      <SandboxOffer plane={plane} />
      {/* **A sandbox setting changed under running chats** (#1428): how many keep the old
          sandbox, and the restart that gives them the new one. */}
      <SandboxChangedNotice
        older={olderSandbox}
        dismissed={dismissed}
        dismiss={dismiss}
        settle={settleNotices}
        onRestart={restartThem}
      />
      {/* **The runner this project's chats run on answered with another key** (#2752): said
          once for the project, where its pool is removed. */}
      <RunnerKeyChangedNotice
        plane={plane}
        onOpenPools={() => openSettingsAt({ group: "you.machine.pools" })}
      />
      {/* **The runner this project's chats run on was removed outside purlis** (#4149): said
          once for the project, with Make the runner again. */}
      <RunnerGoneNotice
        plane={plane}
        onOpenPools={() => openSettingsAt({ group: "you.machine.pools" })}
      />
      {/* **A task's runner kept, since its work is not all here** (#4569): never silent, with
          Fetch again and Discard. */}
      <FreshRunnerKeptNotice plane={plane} />
      {/* **The project expects something this machine has not allowed** (ADR 0093): asked
          once, up front, where the declaration is read, never first by a tool that failed. */}
      <ProjectExpectsNotice
        plane={plane}
        onReview={() => openSettingsAt({ group: "project.sandbox" })}
      />
      {/* **The project turned the sandbox off** (#2071, SO-8): reaches each person only on
          their one Accept. */}
      <TeamSandboxOffNotice plane={plane} />
      {/* **A persona's hosts wait for you** (#1362): asked on each machine, never told. */}
      <PersonaHostsNotice plane={plane} />
      {/* The doctor's findings that stand as Notices, each with its fix (#1250). */}
      <DoctorNotices
        doctor={doctor}
        dismissed={dismissed}
        dismiss={dismiss}
        settle={settleNotices}
      />
      {/* **Where this project's browsers were at the last quit** (#2022): offered once. */}
      <BrowsersAtQuitNotice plane={plane} onOpen={newBrowserAt} />
      {/* **A workspace's repo left off its default branch, or on a moved remote** (#1985): one
          Notice per repo, with the Fix that loses no work. */}
      <StaleCloneNotices clones={staleClones} />
      {/* **A clone made outside purlis**: one Notice per workspace clone purlis did not clone
          and the person has not trusted, naming its remote, with Trust this clone. */}
      <CloneVouchNotices
        plane={plane}
        dismissed={dismissed}
        dismiss={dismiss}
        settle={settleNotices}
      />
      {savedNotice && (
        <Notice
          cause="session-saved"
          link={
            savedNotice.record
              ? {
                  label: "Open record",
                  onPress: () => {
                    const record = savedNotice.record;
                    setSavedNotice(undefined);
                    if (record) showView(sessionView(record.path), sessionTitle(record.title));
                  },
                }
              : undefined
          }
          onDismiss={() => setSavedNotice(undefined)}
        >
          Session saved{savedNotice.record ? ` — ${savedNotice.record.title}` : "."}
        </Notice>
      )}
      {/* **A Smart close that saved its record with no pass** (#1361): the chat ran it, and
          purlis did not see /purlis-smart-close typed in its pane, so it was not ended. Never a
          silent miss: one press ends it, as End… would (#4756: a close ends nothing, so the press
          says End). Only while its tab is there. */}
      {keptOpen.map(({ session, name }) => (
        <Notice
          key={session}
          cause={`smart-close-kept-open:${session}`}
          fixes={[{ label: "End chat", onPress: () => closeKeptOpen(session) }]}
          onDismiss={() => forgetKeptOpen(session)}
        >
          {name} saved its session record. purlis did not see <code>/purlis-smart-close</code> typed
          in it, so it was not ended.
        </Notice>
      ))}

      {/* **A pin whose workspace is gone is kept dormant** (V91c as amended): drawn in the
          title bar as a dormant chip that offers no way in, since that would be offering a
          workspace the project does not have (ADR 0034's hazard, one scope down), and never
          written away. This line hangs from that chip too (#3651); both read one dismissal. It
          is a live pin again when the workspace comes back. Forget is the operator's, with an
          Undo for this run, which stays here in the Notices centre: its chip is gone. */}
      {dormantPins
        .filter((name) => !dismissed.has(dormantCause(name)))
        .map((name) => (
          <Notice
            key={name}
            cause={dormantCause(name)}
            fixes={[{ label: "Forget", onPress: () => void forgetDormantPin(name) }]}
            onDismiss={() => dismiss(dormantCause(name))}
          >
            {dormantSays(name)}
          </Notice>
        ))}
      {/* **Every offer an extension made of its view** (#4769): one quiet note each, here in the
          Notices centre, and on the band of the chat it is about while that chat is on screen. */}
      <OfferOpeningContext.Provider value={offerOpening}>
        <OfferNotices />
      </OfferOpeningContext.Provider>
      {/* **What a chat in no pane has to say** (#4759, spec #4750): its notes, here in the
          Notices centre, each saying whose it is with Go to it, and a mark on its row while
          any stands. Its asks are the Inbox's own, from the registry, so none is drawn here. */}
      {paneless.map((session) => (
        <CentredFor key={session} session={session} onHeld={centredHeld}>
          <NoticeOf.Provider value={{ whose: nameOf(session), onGo: () => showChat(session) }}>
            <ChatNotices
              plane={plane}
              session={session}
              byHand={byHand[session]}
              onByHand={(open) => answerByHand(session, open)}
              startNotes={startNotes[session]}
              onDismissStartNote={() => dismissStartNote(session)}
              onRestart={() => askRestart(session)}
              blocks={sandboxBlocks[session]}
              onDismissBlock={(block) => dismissBlock(session, block)}
              onRestarted={(chat) => chatRestarted(session, chat)}
              onAllowed={() => oweRestart(session)}
              restartSaid={restartsSaid[session]}
              onRestartAnswer={(act) => answerRestart(session, act)}
              asks={NO_ASKS_HERE}
              waiting={NO_ASKS_HERE}
              onShowInbox={showInbox}
            />
          </NoticeOf.Provider>
        </CentredFor>
      ))}
      {forgottenPins.map(({ name, after }) => (
        <Notice
          key={name}
          cause={`pin-forgotten:${name}`}
          fixes={[{ label: "Undo", onPress: () => void undoForget(name, after) }]}
          onDismiss={() => setForgottenPins((was) => was.filter((one) => one.name !== name))}
        >
          Forgot the pin to {name}.
        </Notice>
      ))}

      {/* A memory's Delete, which can be undone for a few seconds (SI-9b, ADR 0065 Q8). */}
      {memoryEdits.undo}
      {/* A pane dropped, which Undo or ⌘Z puts back (#4763). */}
      {paneDrag.undoLine}

      {/* By the chat's id, never its name: two waiting chats can share a name (a split's chat
            takes its tab's), and each Notice acts on its own chat alone. */}
      {wouldNotStart.map(({ id, name, why, approval }) => (
        <Notice
          key={id}
          cause={`chat-did-not-start:${id}`}
          tone="trouble"
          fixes={[
            // Never a one-press Approve here (D-1246-5): the press opens the question that
            // shows what would run, and the approval is that question's answer.
            approval
              ? { label: REVIEW_AND_APPROVE, onPress: () => askApprove(id, name, approval) }
              : { label: "Retry now", onPress: () => void retryChat(id) },
            ...(approval ? [{ label: "Retry now", onPress: () => void retryChat(id) }] : []),
            {
              label: FORGET_THIS_CHAT,
              onPress: () => askForget(id, name),
            },
          ]}
          onDismiss={() => setWouldNotStart((was) => was.filter((one) => one.id !== id))}
        >
          <strong>{name}</strong> did not start ({why}). It is still recorded, and will be tried
          again at the next launch.
        </Notice>
      ))}

      {/* **While you were away** (#1514): what the project's tasks did while the person was
            away from the window, one line with a link to each part. It answers nothing. */}
      <AwaySummary
        away={awayNow}
        onShowChat={showChat}
        onShowFinished={showFinished}
        onShowInbox={showUpdates}
        onShowChanges={showChangesOf}
      />
      {/* What the project's chats drafted with `purlis report`, for the person to read and file
          (#5694). */}
      <ReportDraftNotices
        plane={plane}
        onGo={(session) => showChat(session)}
        onSent={(sent) =>
          setSentDrafts((was) => [...was.filter((one) => one.id !== sent.id), sent])
        }
        onUnsent={(id) => {
          // Undone, or past the day's cap (#5695): it is a draft again, and was never sent.
          setSentDrafts((was) => was.filter((one) => one.id !== id));
          settleUpdates([reportSentKey(id)], "dismissed");
        }}
      />
      {alerts !== undefined && (
        <InboxAlerts
          plane={plane}
          reading={alerts.reading}
          machine={aboutThisMachine}
          elsewhere={alerts.planes
            .filter((one) => one !== plane)
            .map((one) => ({ plane: one, name: alerts.nameOf(one) }))}
          does={alerts.does}
        />
      )}
    </>
  );

  /** **The chat whose skills the Skills view shows** (#3266, #3693): the one shown in the pane
   *  with the keyboard, never a shell's, which runs no tool with skills. **While a tab that is not
   *  a chat is in front** (#4351; ADR 0102, amended 2026-11-02), such as the Skill studio's, the
   *  chat last in front (`chatKept`, until its tab closes): its set is not read again for a new
   *  chat, and its cards are not drawn again. It is described as it is now, wherever it moved. */
  const skillsSession = chatNow ?? (frontTab !== undefined ? chatKept : undefined);
  const skillsWorkspace = skillsSession === undefined ? undefined : filedIn(skillsSession);
  // The checkout it works in, as the core answered for its directory (#4851), in front or not.
  const skillsPiece = skillsSession === undefined ? undefined : worktreeOf(skillsSession);
  const skillsChat: FrontChat | undefined =
    skillsSession === undefined
      ? undefined
      : {
          session: skillsSession,
          name: nameOf(skillsSession),
          workspace: skillsWorkspace === OUTSIDE ? undefined : skillsWorkspace,
          persona: personaOf(skillsSession),
          harness: harnessOf(skillsSession),
          // Only where it is filed: the view names a checkout by the chat's workspace.
          ...(skillsPiece === undefined || skillsPiece.workspace !== skillsWorkspace
            ? {}
            : { branchFolder: { repo: skillsPiece.repo, piece: skillsPiece.piece } }),
        };

  const view = (
    <ChatToUseHere.Provider value={chatToUse}>
      {/* Placement mode's pick-up, for the panes' grips and the sidebar's rows (#4764). */}
      <PlacingHere placing={paneDrag.placing}>
        <QuestionsHere.Provider value={questions}>
          <Lent
            chats={chats}
            tasksBelow={tasksBelow}
            doings={doings}
            skillRuns={skillRuns}
            references={referenceChats}
            memoryTargets={memoryTargets}
            readOnly={readOnlySaid}
            mendable={readOnly?.mendable === true}
            personas={personaMarks}
            askPersona={openAskPersona}
            useSkill={openUseSkill}
            chatSkillSets={chatSkillSets}
            brief={openBrief}
            answer={openAnswer}
            studioChats={studioChats}
          >
            <SessionOffersLent
              plane={plane}
              store={workspacePanels}
              workspace={ofWorkspace}
              atRoot={focused === OUTSIDE}
              rootRecords={rootPanels?.sessions}
              onRows={onSessionRows}
            >
              {/* **A project this purlis opens read-only says why, once, on its tab** (#826, FR-24):
          which feature is missing and which version has it. Above its workspaces, so it is the
          project's line and no workspace's; each row it turns off repeats it as its reason. */}
              <ReadOnlyLine readOnly={readOnly} />
              {/* **A project that lives only on this machine says so, with Publish beside it** (#1820). */}
              <LivesHere plane={plane} />
              {/* **The workspaces of this project are in the title bar** (#2916): the project's pins as
          chips after its name, and every workspace behind its ⌄ (`TitleBarSwitch.tsx`). The
          strip that drew them here is gone, as the project strip went to the rail (#2915). */}
              {/* **A chat's theme draft is named on the workspace it is drawn in** (#2332): the
          strip under the workspaces, with Keep, Revert and Compare, and the revert key. */}
              <ThemeDraftStrip plane={plane} workspace={ofWorkspace ?? null} />
              {/* **One polite line about a pane the keyboard did not ask for** (#4767). */}
              <p role="status" className="sr-only" data-testid="pane-said">
                <span key={paneSaid.count}>{paneSaid.said}</span>
              </p>
              {/* **A new Blocking ask, said once and politely** (ADR 0121 decision 25): a line
          that changes only as an ask arrives, so drawing the window again says nothing. */}
              <p role="status" className="sr-only" data-testid="ask-arrived">
                <span key={arrival.said.count}>{arrival.said.said}</span>
              </p>

              {/* **The regions** (ADR 0038): by default the navigation region on the left, the
          panes in the middle, and what is asking for you on the right. What the repos are
          doing was along the bottom until #1676 made it the left's Changes view, so the panes
          are the window's whole height. Every region resizes, and each side can be put away —
          the centre cannot, because the terminal panes are the product.

          **By default, and no longer by shape.** Which side each region is on, what order it
          is in and how big it is are the arrangement (`regions.ts`); this is the content that
          goes in whichever slot the arrangement names.

          One workspace answer for every region and view (`workspacePanels.ts`), not one each:
          they draw the same workspace, and `workspace_repos` runs `git status` per clone. */}
              <RegionFrame
                arrangement={arrangement}
                onResized={resized}
                onReset={resetSize}
                onSplit={splitHalves}
                // F6 between the regions (P-13, #1741) is the project in front's, as its side keys are.
                keyboard={inFront}
                // **The workspace's pinned artifacts** (#5952, ADR 0122 §2): their own group on the
                // left bar, below purlis's icons; a press opens the artifact's tab or brings it
                // forward.
                artifacts={
                  <ArtifactGroup
                    plane={plane}
                    workspace={ofWorkspace}
                    current={artifactInFront}
                    onOpen={(pin) => {
                      if (ofWorkspace !== undefined)
                        showView(artifactView(ofWorkspace, pin.folder), pin.name);
                    }}
                  />
                }
                // Skills while the Skill studio's preview is on (#3266).
                previews={previews}
                /* **The navigation region's views** (ADR 0038 as amended 2026-10-10, #1673): Chats and
           Explorer, one at a time, switched by the activity bar at the side's edge. Both stay
           mounted when the other is open or the side is away, so neither loses its folds. */
                views={{
                  chats:
                    allChats.on && everyChat !== undefined ? (
                      <AllChatsOf
                        every={everyChat}
                        filter={allChats.filter}
                        onFilter={(filter) => setAllChats({ on: true, filter })}
                        onLeave={() => setAllChats({ on: false })}
                      />
                    ) : (
                      <WaitingTaskWaysContext.Provider value={waitingTaskWays}>
                        <ChatsSection
                          onAllChats={everyChat === undefined ? undefined : () => openAllChats()}
                          rows={listRows}
                          // The trees that started in the focused workspace, or at the root (#1655).
                          here={focused === OUTSIDE ? ROOT_WORD : focused}
                          // The chat that has the keyboard: a task, while its pane shows it (#1486).
                          front={focusedChat(tabs)}
                          besides={besides}
                          noted={noted}
                          onOpen={showChat}
                          offers={found}
                          onPress={press}
                          stopping={stopping}
                          wrapping={wrapping}
                          sandboxOf={sandboxOf}
                          finished={finishedTasks}
                          onClearFinished={clearFinished}
                          onReopen={reopenFinished}
                          clock={chatClock}
                          onDrawn={rowsDrawn}
                          reveal={revealed}
                          onRevealed={revealedSettled}
                          onLookFinished={lookedAtFinished}
                          restarts={restartsOnRows}
                          ending={taskEndInline}
                          onEndTask={endTaskOnRow}
                          onEndConfirm={answerTaskEndOnRow}
                          onChanges={(task) =>
                            showView(taskChangesView(task.id), taskChangesTitle(task.name))
                          }
                          browsers={chatBrowsersShown}
                          kept={keptShown}
                          // The Browsers grid's button, while the grid has a tile (#2027).
                          anyBrowser={gridTiles.length > 0}
                          onBrowser={openChatBrowser}
                          // The side panel's header (R123): the workspace, then the project, quiet.
                          heading={
                            <>
                              {ofWorkspace ?? OUTSIDE_TITLE}{" "}
                              <span className="side-head-project">· {projectName(plane)}</span>
                            </>
                          }
                        />
                      </WaitingTaskWaysContext.Provider>
                    ),
                  explorer: (
                    <WorkspaceRegion
                      store={workspacePanels}
                      workspace={ofWorkspace}
                      equal={FOR_THE_GIT_SIDE}
                      draw={(state) => (
                        <Explorer
                          // Files, of the workspace the strip is on: Repos and branches is the Panel's
                          // Repos view (#1737), and the strip is where a workspace is selected (qw358).
                          part="explorer"
                          plane={plane}
                          workspace={ofWorkspace}
                          live={ofWorkspace !== undefined && liveOf(ofWorkspace)}
                          state={state}
                          chats={workspaceChats}
                          spot={spot}
                          onPick={pickSpot}
                          offers={found}
                          onPress={press}
                          onOpenFile={(place, path) =>
                            showView(pieceFileView(place, path), pieceFileTitle(place, path))
                          }
                          focus={cockpit}
                          onFocus={setFocusedBranch}
                          cloning={cloning}
                          onReadAgain={rereadPanels}
                          onAddToChat={setAddingToChat}
                          // A workspace's repo pressed in the plane root's files (issue 1916).
                          onGoToWorkspace={focusWorkspace}
                          // The Skills view's Show in Explorer (#3266).
                          reveal={explorerReveal}
                          // Search contents (#3343): Search on the filter's scope, with its words.
                          onSearchContents={searchContents}
                          onSearchKey={keepExplorerSearchKey}
                          // Files dropped from the file manager on the workspace's own (#5378).
                          onDropFiles={workspaceWrites.drop}
                        />
                      )}
                    />
                  ),
                  // **Repos** (#1737, P-2, P-10): Explorer's *Repos and branches*, in the left half of
                  // the Panel. The same pick as ever: a repo or branch picked here is what Files lists.
                  repos: (
                    <WorkspaceRegion
                      store={workspacePanels}
                      workspace={ofWorkspace}
                      equal={FOR_THE_GIT_SIDE}
                      draw={(state) => (
                        <Explorer
                          part="repos"
                          plane={plane}
                          workspace={ofWorkspace}
                          live={ofWorkspace !== undefined && liveOf(ofWorkspace)}
                          state={state}
                          chats={workspaceChats}
                          spot={spot}
                          onPick={pickSpot}
                          offers={found}
                          onPress={press}
                          focus={cockpit}
                          onFocus={setFocusedBranch}
                          cloning={cloning}
                          onReadAgain={rereadPanels}
                        />
                      )}
                    />
                  ),
                  // **Search** (#1676): the Search tab's content, in the side. Drawn from the first time
                  // it is shown, then kept, so what was typed and found survives a switch away.
                  search: sideSearch !== undefined && (
                    <SearchTab
                      plane={plane}
                      view={sideSearch}
                      onAsk={(_, to) => setSideSearch(to)}
                      takesKeyboard={false}
                    />
                  ),
                  // **Skills** (#3266): this workspace's skills by level, from the Skill studio's own
                  // answer, drawn only while its preview is on; the side falls back to Chats meanwhile.
                  skills: studioOn && skillsDrawn && (
                    <SkillsView
                      plane={plane}
                      workspace={ofWorkspace}
                      chat={skillsChat}
                      onOpenView={skillsOpenView}
                      onUseSkill={skillsUse}
                      onShowInExplorer={skillsShowInExplorer}
                      onRestart={skillsRestart}
                    />
                  ),
                  // **Changes** (#1676, B-7): what the bottom region drew, the same content.
                  changes: (
                    <WorkspaceRegion
                      store={workspacePanels}
                      workspace={ofWorkspace}
                      equal={FOR_THE_GIT_SIDE}
                      draw={(state) => (
                        <ChangesView
                          workspace={ofWorkspace}
                          state={state}
                          offers={found}
                          onPress={press}
                          columns={facts.columns}
                          cloning={cloning}
                        />
                      )}
                    />
                  ),
                  // **Asks** (#1692, #5644): what waits on the person here, answered in place.
                  asks: (
                    <Inbox
                      part="asks"
                      plane={plane}
                      seen={seenRows}
                      seenOf={seenOf}
                      asks={waiting}
                      onGo={goToAsking}
                      elsewhere={elsewhere}
                      // The scope's workspace is the strip's (qw359): the plane root is a place too.
                      here={focused}
                      workspaceOf={filedIn}
                      workspaceNamed={workspaceNamed}
                      asked={chatAsked}
                      personaOf={personaOf}
                      onLeave={leaveInbox}
                      whyOf={inboxWhy}
                      onAnswered={inboxAnswered}
                      onIgnore={(session) => {
                        // The queue's own row, as the palette presses it.
                        const ignore = queueRow(ignoreId(session), session);
                        if (ignore?.available) press(ignore);
                      }}
                      snoozing={snoozing}
                      cooling={cooling}
                      // **Quiet** (#6017): "Quiet now", and what the Quiet that ended held.
                      quietTop={
                        <AsksQuiet
                          plane={plane}
                          asks={waiting}
                          updates={keptUpdates}
                          isNew={seenRows.isNew}
                          onGoToAsk={goToAsking}
                          onShowUpdates={showUpdates}
                        />
                      }
                      // **The empty Asks' parts** (ADR 0121 decision 23): All chats filtered.
                      onAllChats={openAllChatsFrom}
                      onStop={stopFromARow}
                    />
                  ),
                  // **Updates** (#5644): the project's updates, as the Inbox listed them.
                  updates: (
                    <Inbox
                      part="updates"
                      plane={plane}
                      seen={seenRows}
                      seenOf={seenOf}
                      asks={waiting}
                      onGo={goToAsking}
                      updates={inboxUpdates}
                      project={fromTheProject}
                      hold={updatesHold}
                      elsewhere={elsewhere}
                      onLeave={leaveInbox}
                      here={focused}
                      workspaceOf={filedIn}
                      workspaceNamed={workspaceNamed}
                    />
                  ),
                  // **Notices** (#5645): the project's Notices, the window's own lines and
                  // Results, in the Panel's right half; always this project's, so no scope.
                  notices: (
                    <Inbox
                      part="notices"
                      plane={plane}
                      seen={seenRows}
                      seenOf={seenOf}
                      onNoticeCauses={setNoticeCauses}
                      asks={waiting}
                      onGo={goToAsking}
                      notices={standing}
                      reading={inboxReading}
                      onNotices={setNoticesListed}
                      onDot={setNoticesDot}
                      onNoticeAnswer={inboxOnScreen}
                      windowLines={windowLines}
                      onLeave={leaveInbox}
                      hold={noticesHold}
                    />
                  ),
                  // **The attention region's views** (#1678): the "for you" side (ADR 0038 as amended
                  // 2026-10-10), one panel per view, and each approved extension's panel a view too.
                  ...Object.fromEntries(
                    [
                      ...ATTENTION_VIEWS.filter(
                        (view) => view !== "asks" && view !== "updates" && view !== "notices",
                      ),
                      ...sidePanels.map((one) => one.view),
                    ].map((view) => [
                      view,
                      // Every part of the panels: these are what a memory or a session record draws.
                      <WorkspaceRegion
                        key={view}
                        store={workspacePanels}
                        workspace={ofWorkspace}
                        draw={(state) => (
                          <Panels
                            view={view}
                            workspace={ofWorkspace}
                            state={state}
                            offers={found}
                            onPress={press}
                            contributed={contributed}
                            shownRow={shownRow}
                            onShowRow={setShownRow}
                            vaults={vaults}
                            onAddTodo={edits.addTodo}
                            atRoot={focused === OUTSIDE}
                            rootPanels={rootPanels?.contributed}
                            personaChats={personaChats}
                            chatPersonasHere={
                              ofWorkspace === undefined
                                ? undefined
                                : chatPersonasIn.get(ofWorkspace)
                            }
                          />
                        )}
                      />,
                    ]),
                  ),
                }}
                panels={sidePanels}
                // The Chats tab's count is the queue's, drawn where the queue is read (#1034): a chat
                // that starts asking redraws the badge and nothing around it. It is on the bar, so it
                // is there while the side is put away (B-8). Changes counts what git has uncommitted.
                // The Lower icons' are drawn the same way, so they are there while their half of the
                // Panel is away (P-9, #1739): Changes' count, and Repos' only once a clone is behind.
                // **What is New in each view** (#6002): a pip beside its badge, until seen.
                pulse={arrival.pulse ? "asks" : undefined}
                onPulsed={arrival.pulsed}
                fresh={{
                  asks: newAsks(waiting, seen),
                  updates: newUpdates(
                    keptUpdates?.filter((one) => !updatesHold.held.has(heldUpdate(one.key))),
                    seen,
                  ),
                  notices: newNotices(noticeCauses, seen),
                }}
                badges={{
                  repos: gap && (
                    <ActivityCount
                      count={gap.behind}
                      drawn={`${gap.behind}↓${gap.ahead > 0 ? `${gap.ahead}↑` : ""}`}
                      said={`${gap.behind === 1 ? "1 commit" : `${gap.behind} commits`} behind${gap.ahead > 0 ? `, ${gap.ahead} ahead` : ""}`}
                    />
                  ),
                  changes: (
                    <ActivityCount
                      count={changedFiles}
                      said={
                        changedFiles === 1
                          ? "1 uncommitted file"
                          : `${changedFiles} uncommitted files`
                      }
                    />
                  ),
                  chats: (
                    <QueueRead>
                      {(queue) => (
                        <ActivityCount
                          count={queue.length}
                          said={
                            queue.length === 1
                              ? "1 chat needs you"
                              : `${queue.length} chats need you`
                          }
                        />
                      )}
                    </QueueRead>
                  ),
                  // The ✋'s count (#1692, I-2; ADR 0121 decision 4): every open project's asks, in
                  // its tone, on the bar, so it is there while the side is put away. Those outside
                  // this project are said as "+N elsewhere"; Asks' scope never narrows it.
                  asks: <ActivityCount count={hand.total} said={handSaid(hand)} tone="needs-you" />,
                  // The unread lines From your chats (#5646, ADR 0119): the bell's count, in the plain
                  // colours, since a result is news of the person's own work and asks nothing.
                  // Project is never counted.
                  updates: (
                    <ActivityCount
                      count={bell}
                      said={
                        bell === 1
                          ? "1 unread update from your chats"
                          : `${bell} unread updates from your chats`
                      }
                    />
                  ),
                  // A dot, never a number (#5645): red while a Notice needs fixing, amber while one
                  // asks a decision that stops no chat (#6004), and nothing otherwise.
                  notices: noticesDot !== undefined && (
                    <ActivityDot
                      tone={noticesDot === "trouble" ? "danger" : "warning"}
                      said={noticesDot === "trouble" ? NEEDS_FIXING : NEEDS_A_DECISION_DOT}
                    />
                  ),
                  // The focused workspace's open todos (B-8), the status line's own count: plain, since
                  // a todo is yours to do and not a chat asking for you.
                  todos: (
                    <ActivityCount
                      count={openTodos ?? 0}
                      said={openTodos === 1 ? "1 open todo" : `${openTodos ?? 0} open todos`}
                    />
                  ),
                }}
                keys={SIDE_KEYS_SAID}
                onPick={pickSideView}
                // The side panel's header (R123): Chats draws its own; Explorer names the workspace.
                heads={{ chats: null, explorer: ofWorkspace ?? OUTSIDE_TITLE }}
                // Right-click on a view's icon (#1740): its move, the catalogue's fourth reader.
                menu={(view, to, tab) => (
                  <Menued on={{ on: "view", view, to }} offers={found} onPress={press}>
                    {tab}
                  </Menued>
                )}
                // Every region is views now (#1676, #1678): nothing is drawn outside a view.
                content={{}}
                centre={
                  /* **The editor: the chat strip, then the panes** (DS-6, #1836; ADR 0036 as amended
             2026-10-12). The session strip is the centre's own tab bar, directly above the chat
             it shows, as an editor's tabs are; the workspaces stay the full-width row above every
             region. Splits live inside a tab, so one centre has one strip. */
                  <div className="editor" {...tabsPart("layout")}>
                    {/* The chat strip is the focused workspace's, so it is drawn in that workspace's colour
              (charter-app#281): its shade, its selected tab and its accent.

              **Outside `Menued on=pane`**, so a right-click on the strip's gaps is not the pane
              menu, and drawn over all three of the centre's states — the panes and both empty
              ones — because it is where `+` is. The editor's chrome (`REGION_CHROME`): F6 into
              the editor passes over it to the pane, as VS Code's F6 lands on the editor and not
              on its tabs. Tab still stops on it, in document order. */}
                    <ChatStrip
                      by={by}
                      newTabRows={plusRows}
                      found={found}
                      press={press}
                      tabs={tabs}
                      ofWorkspace={ofWorkspace}
                      colourOf={colourOf}
                      tintOf={tintOf}
                      shown={shown}
                      hidden={hidden}
                      chats={chats}
                      chatTabId={chatTabId}
                      strip={strip}
                      chatControls={chatControls}
                      chatLeast={chatLeast}
                      chipWidth={chipWidth}
                      dragSensors={dragSensors}
                      chatDragWords={chatDragWords}
                      dragTab={dragTab}
                      tearOff={(tab) => void moveToWindow(tab, false, null)}
                      draggingRef={dragging}
                      draggedAtRef={draggedAt}
                      notGoingBack={notGoingBack}
                      isPinned={isPinned}
                      pressTab={pressTab}
                      referenceChats={referenceChats}
                      wrapping={wrapping}
                      stopping={stopping}
                      shells={shells}
                      personaOf={personaOf}
                      factsOf={factsOf}
                      taskNameOf={taskNameOf}
                      taskTabOf={taskTabOf}
                      naming={naming}
                      pinsIconOnly={pinsIconOnly}
                      hiddenIn={hiddenIn}
                      tasksRunningOf={tasksRunningOf}
                      waitingOn={waitingOn}
                      showFromChip={showFromChip}
                      freshOf={freshOf}
                      freshMarkOf={freshMarkOf}
                      planeUpdates={planeUpdates}
                      handedFrom={handedFrom}
                      workItems={workItems}
                      restoring={restoringChats}
                    />
                    {/* The centre is where a chat is, so its menu is the pane's rows: the two splits,
                the tab in front's Tile all, the key the palette claimed, and — under the line —
                Close pane, which ends nothing (#4756). `asChild` again: the panes' box is measured, and it must not gain a
                wrapper, so the editor's column is outside it and not between it and the panes. */}
                    <Menued on={{ on: "pane", tab: frontTabId }} offers={found} onPress={press}>
                      {/* The content panel, which the chosen tab joins (`ui/tabs`): the tab panel of
                  the chosen tab, named by it, while it is drawn on the strip. */}
                      <div
                        ref={panesAt}
                        className="panes"
                        {...{ [PANES_ROOM]: "" }}
                        {...panelRoot("content")}
                        {...tabPanelOf(panelNamedBy)}
                        onPointerDownCapture={paneUsed}
                        onKeyDownCapture={paneUsed}
                      >
                        {frontTab ? (
                          <TaskEndContext.Provider value={taskEndHand}>
                            <OfferOpeningContext.Provider value={offerOpening}>
                              <QuizPlaces.Provider value={quizPlaces}>
                                <LayoutPanes
                                  plane={plane}
                                  restoring={restoringChats}
                                  // Each pane holding the chat it SHOWS (#1486): a task, in the pane of the
                                  // session that asked, while the tab is switched to it.
                                  // A zoomed pane is drawn alone, the tree kept whole (#4765).
                                  layout={zoomedLayout(
                                    tabs,
                                    frontTab.id,
                                    layoutShown(tabs, frontTab.id) ?? frontTab.layout,
                                  )}
                                  room={paneRoom}
                                  faceOf={paneFace}
                                  crumbs={frontCrumbs}
                                  placed={frontPlaced}
                                  endedTasks={frontEndedPanes}
                                  endedChats={endedHere}
                                  unread={frontUnread}
                                  finished={frontFinished}
                                  explaining={chipToExplain}
                                  tasksOn={tasksOn}
                                  usingOn={usingOn}
                                  others={frontOthers}
                                  asked={frontAsked}
                                  asks={asksHere}
                                  waiting={waiting}
                                  onShowInbox={showInbox}
                                  onAnswer={answeredOnPane}
                                  closeOf={closeOfPane}
                                  alone={hasOnePane(tabs, frontTab.id)}
                                  onShowChat={showChatFromPane}
                                  focused={frontTab.focused}
                                  onFocus={(pane) => change((tabs) => focusPane(tabs, pane))}
                                  onOpenFolded={(pane) =>
                                    keyboardInto(change((tabs) => focusPane(tabs, pane)))
                                  }
                                  offerFor={by}
                                  offers={found}
                                  heads={paneHeads}
                                  shells={shells}
                                  onPaneDoes={onPaneDoes}
                                  name={frontTab.name}
                                  handedFrom={handedFrom}
                                  glances={glances}
                                  workItems={workItems}
                                  byHand={byHand}
                                  onByHand={answerByHand}
                                  startNotes={startNotes}
                                  onDismissStartNote={dismissStartNote}
                                  onRestartChat={askRestart}
                                  blocks={frontBlocks}
                                  onDismissBlock={dismissBlock}
                                  onRestarted={chatRestarted}
                                  onAllowed={oweRestart}
                                  restartsSaid={restartsSaid}
                                  onRestartAnswer={answerRestart}
                                  offered={views}
                                  onOpenView={showView}
                                  onOpenViewBeside={showViewBeside}
                                  onAsk={(pane) => change((tabs) => stopWaiting(tabs, pane))}
                                  onVaultChanged={reloadVaults}
                                  memory={{
                                    // The views follow the disk themselves (`ViewPane`), so a memory saved
                                    // redraws the views and not this whole window (FD-10).
                                    changed: memoryEdits.changed,
                                    onSaved: memoryEdits.onSaved,
                                    onClose: closeView,
                                    showInstead: (from, to, title) => {
                                      change((tabs) => showInstead(tabs, from, to, title));
                                      // The level switcher moving Settings is a way into it (#1206).
                                      const place = placeOfView(to, plane);
                                      if (place !== undefined) enterSettings(place);
                                    },
                                  }}
                                  firstTask={firstTaskDoes}
                                  onNewChatOn={newChatOn}
                                  split={{
                                    of: (view) => splitOf(tabs, view),
                                    moved: (view, split) =>
                                      change((tabs) => setSplit(tabs, view, split)),
                                    shared: (path, share) =>
                                      change((tabs) => withShare(tabs, frontTab.id, path, share)),
                                  }}
                                  runningIn={runningIn}
                                  noWorkspace={
                                    sidebar !== undefined && sidebar.workspaces.length === 0
                                  }
                                  tab={frontTab.id}
                                  chatName={nameOf}
                                  onClearPane={clearPane}
                                  onResumeTask={(pane, task) => void resumeTaskIn(pane, task)}
                                  onResumeEnded={(session) => void resumeEndedIn(session)}
                                />
                              </QuizPlaces.Provider>
                            </OfferOpeningContext.Provider>
                          </TaskEndContext.Provider>
                        ) : /* No tab in front yet: the strip is given one empty pane as soon as the
                         record is heard (#4758), and that pane is the window's empty state. */
                        null}
                      </div>
                    </Menued>
                  </div>
                }
              />

              {/* **charter's status line**, under everything, the terminals included. It is not
          in the arrangement and `StatusLine.tsx` argues why at length: a slot is sized as a
          percentage of its group and this is one line of text, a region can be put away and
          this must not be, and the window is chrome · regions · chrome — the project
          strip above is not a region either.

          It reads what is already known. `workspaceState` is the one ask the three regions
          share, and the sidebar has already been read for the strip, so the line costs no
          command of its own — which matters here more than anywhere, because it is the one
          surface that is drawn whatever else the window is doing. */}
              {/* Set up signing, opened from the one Ask about lanes waiting on signing (#5633). */}
              <SigningSetupAsked plane={plane} />
              <StatusLineHere
                sessions={quitSessions}
                settled={settled}
                plane={plane}
                read={sidebar !== undefined}
                where={focused === OUTSIDE ? OUTSIDE_TITLE : focused}
                workspaces={sidebar?.workspaces.length}
                workspacePanels={workspacePanels}
                workspace={ofWorkspace}
                doctor={doctor}
                // Notices lists the doctor's findings: its icon shows them, so the Doctor item
                // draws no count of its own (ADR 0121 decision 1, #6004).
                doctorListed={noticeCauses.includes(DOCTOR_LINE)}
                onOpenSettings={(group) => openSettingsAt({ group })}
                pin={pin}
                alerts={noticesButton}
                // Focus mode puts the status bar away, and its card with it: the notices wait in the
                // Notices centre (#3653).
                notices={focusMode ? undefined : statusNotices}
                badges={facts.badges}
                factNotes={facts.notes}
              />

              {/* Making a workspace, and deleting one. Mounted only while they are up, and drawn
          here rather than in the window: a workspace belongs to a project. */}
              {/* **What is asked over the window is asked only of the project in front**: a dialog is
          drawn over the whole window, wherever the project that opened it is. One left open in
          a project that is no longer in front is put away with it and is there again when the
          project is. */}
              <OnlyInFront>
                {makingWorkspace && (
                  <NewWorkspace
                    plane={plane}
                    trouble={workspaceTrouble}
                    making={busyMaking}
                    planeId={plane}
                    onCreate={(name, vision, live, repos) =>
                      void makeWorkspace(name, vision, live, repos)
                    }
                    onCancel={() => {
                      setMakingWorkspace(false);
                      setWorkspaceTrouble(undefined);
                    }}
                  />
                )}

                {branching && (
                  <NewBranch
                    repo={branching.repo}
                    trouble={branchTrouble}
                    making={busyBranching}
                    onCut={(branch) => void cutBranch(branch)}
                    onCancel={() => {
                      setBranching(undefined);
                      setBranchTrouble(undefined);
                    }}
                  />
                )}

                {makingVault && (
                  <NewVault
                    plane={plane}
                    trouble={vaultTrouble}
                    making={busyVault}
                    onCreate={(name, provider, opVault) => void makeVault(name, provider, opVault)}
                    onMade={(done) => {
                      // The guided set-up made it in the core (#1527): the same ending as `makeVault`.
                      setMakingVault(false);
                      setVaultTrouble(undefined);
                      reloadVaults();
                      showView(
                        { from: null, view: "vault", key: done.contents.name },
                        done.contents.name,
                      );
                    }}
                    onCancel={() => {
                      setMakingVault(false);
                      setVaultTrouble(undefined);
                    }}
                  />
                )}

                {edits.dialogs}

                {addingToChat && (
                  <AddToAChat
                    referenced={addingToChat}
                    chats={referenceChats.chats}
                    onPick={(chat) => {
                      const [one, ...more] = addingToChat;
                      if (more.length === 0) {
                        if (one !== undefined) referenceChats.hand(one, chat.session, "add");
                        return;
                      }
                      // Several rows picked in the explorer: one paste of them all (#1151).
                      void handReferences(plane, chat.session, chat.name, addingToChat).then(
                        (ran) =>
                          setReport(
                            ran.ok
                              ? { from: "reference", refused: false, words: ran.said ?? "" }
                              : { from: "reference", refused: true, words: ran.refused },
                          ),
                      );
                    }}
                    onCancel={() => setAddingToChat(undefined)}
                  />
                )}

                {pickingVault && (
                  <OpenVault
                    vaults={vaults.vaults ?? []}
                    offers={found}
                    onPress={press}
                    onCancel={() => setPickingVault(false)}
                  />
                )}

                {workspaceWrites.dialog}

                {renamingTab && tabRenames && (
                  <RenameTab
                    // A tab that splits or closes to one pane while it is open asks again.
                    key={`${renamingTab.tab}:${tabRenames.kind}`}
                    renames={tabRenames}
                    name={tabNameOf(tabs.byId[renamingTab.tab])}
                    chat={nameOf(chatOf(tabs, renamingTab.tab) ?? -1)}
                    own={tabs.byId[renamingTab.tab].own !== undefined}
                    trouble={renamingTab.trouble}
                    busy={renamingTab.busy}
                    onRename={(answer) => void saveTabName(answer)}
                    onClear={clearTabName}
                    onCancel={() => setRenamingTab(undefined)}
                  />
                )}

                {/* A chat's proposed pin, opened by Review… on its row (#5963). */}
                <ProposedPins plane={plane} />

                {pinningPage && (
                  <PinPage
                    plane={plane}
                    workspace={pinningPage.workspace}
                    page={pinningPage}
                    onClose={() => setPinningPage(undefined)}
                  />
                )}

                {renamingChat && (
                  <RenameChat
                    name={renamingChat.name}
                    trouble={renamingChat.trouble}
                    renaming={renamingChat.busy}
                    onRename={(typed) => void saveChatName(typed)}
                    onCancel={() => setRenamingChat(undefined)}
                  />
                )}

                {linkingWork && (
                  <LinkWorkItem
                    chat={nameOf(linkingWork.session)}
                    linked={workItems[linkingWork.session]}
                    trouble={linkingWork.trouble}
                    linking={linkingWork.busy}
                    onLink={(key) => void saveWorkLink(linkingWork.session, key)}
                    onCancel={() => setLinkingWork(undefined)}
                  />
                )}

                {briefOf && (
                  <BriefPanel
                    // Another task's brief is another panel: it reads again, from nothing.
                    key={
                      "chat" in briefOf ? `chat:${briefOf.chat}` : `dispatch:${briefOf.dispatch}`
                    }
                    plane={plane}
                    of={briefOf}
                    onClose={() => setBriefOf(undefined)}
                  />
                )}

                {answerOf && (
                  <AnswerPanel
                    // Another task's question is another dialog: it reads again, from nothing.
                    key={answerOf.chat}
                    plane={plane}
                    of={answerOf}
                    asking={standingOf(questions, answerOf.chat) !== undefined}
                    onClose={() => setAnswerOf(undefined)}
                  />
                )}

                {askingPersona && (
                  <AskPersona
                    // A new question is a new dialog: its boxes start from its own prefill.
                    key={`${askingPersona.session}:${askingPersona.persona}`}
                    persona={askingPersona.persona}
                    chat={nameOf(askingPersona.session)}
                    // Every workspace but the one that chat works in: there, "this chat's folder" is it.
                    workspaces={(sidebar?.workspaces ?? [])
                      .filter(
                        (ws) => !ws.chats.some((chat) => chat.session === askingPersona.session),
                      )
                      .map((ws) => ws.name)}
                    prefill={askingPersona.prefill}
                    from={askingPersona.from}
                    trouble={askingPersona.trouble}
                    asking={askingPersona.busy}
                    folderShared={folderShared}
                    onAsk={(name, ask, place) => void sendAsk(name, ask, place)}
                    onCancel={() => setAskingPersona(undefined)}
                  />
                )}

                {usingSkill && (
                  <UseSkill
                    // Another chat's Use skill… is a new dialog: its filter and boxes start empty.
                    // Opened on one skill, it is drawn again once the chat's skills are read: what it
                    // starts with is decided from them (`UseSkill`'s `picks`).
                    key={
                      usingSkill.picks === undefined
                        ? usingSkill.session
                        : `${usingSkill.session}\u0000${usingSkill.picks}\u0000${usingSkill.skills === undefined ? "reading" : "read"}`
                    }
                    chat={nameOf(usingSkill.session)}
                    picks={usingSkill.picks}
                    skills={usingSkill.skills}
                    trouble={usingSkill.trouble}
                    busy={usingSkill.busy}
                    onUse={(skill, values) => void sendUseSkill(skill, values)}
                    onCancel={() => setUsingSkill(undefined)}
                  />
                )}

                {askingAction && (
                  <AskFirst
                    extension={askingAction.extension}
                    action={askingAction.action}
                    onRun={async () => {
                      const outcome = await runExtensionAction(
                        plane,
                        askingAction.extension,
                        askingAction.action,
                        { view: null, key: "", row: null },
                        ofWorkspace,
                        true,
                      );
                      if ("refused" in outcome) return { refused: outcome.refused };
                      // From the palette there is no view to say it above, so the dialog says it.
                      if (outcome.answer.overreach !== null)
                        return { seen: outcome.answer.overreach };
                      setAskingAction(undefined);
                      return undefined;
                    }}
                    onCancel={() => setAskingAction(undefined)}
                  />
                )}

                {liveAsk !== undefined && (
                  <LiveDialog
                    plane={plane}
                    workspace={liveAsk}
                    onClose={() => setLiveAsk(undefined)}
                    onDone={(said) => {
                      setLiveAsk(undefined);
                      setReport({
                        from: `workspace.live:${liveAsk}`,
                        refused: false,
                        words: said.join(" "),
                      });
                      // Read the plane again: the marks come from what is on disk, not from this press.
                      setReplan((asked) => asked + 1);
                    }}
                  />
                )}

                {membershipAsk !== undefined && (
                  <RemoveFromWorkspace
                    plane={plane}
                    workspace={membershipAsk.workspace}
                    repo={membershipAsk.repo}
                    onClose={() => setMembershipAsk(undefined)}
                    onDone={(said) => {
                      setMembershipAsk(undefined);
                      setReport({
                        from: `absent.drop:${membershipAsk.repo}`,
                        refused: false,
                        words: said.join(" "),
                      });
                      // The list is what is on disk: read again, so the repo leaves "Not cloned here".
                      rereadPanels();
                    }}
                  />
                )}

                {addingVision && (
                  <AddVision
                    workspace={addingVision.workspace}
                    current={
                      sidebar?.workspaces.find((ws) => ws.name === addingVision.workspace)
                        ?.vision ?? ""
                    }
                    trouble={addingVision.trouble}
                    saving={addingVision.busy}
                    onSave={(vision) => void saveVision(addingVision.workspace, vision)}
                    onCancel={() => setAddingVision(undefined)}
                  />
                )}

                {renamingWs && (
                  <RenameWorkspace
                    workspace={renamingWs.workspace}
                    startsFresh={renamingWs.startsFresh}
                    trouble={renamingWs.trouble}
                    renaming={renamingWs.busy}
                    onRename={(name) => void doRename(renamingWs.workspace, name)}
                    onCancel={() => setRenamingWs(undefined)}
                  />
                )}

                {/* The one question a close asks about a view's unsaved work (#3929). */}
                {unsavedAsk && (
                  <UnsavedWorkQuestion
                    asked={unsavedAsk}
                    onAnswered={() => askUnsaved(undefined)}
                  />
                )}

                {removing && (
                  <DeleteWorkspace
                    workspace={removing.workspace}
                    atRisk={removing.atRisk}
                    unreadable={removing.unreadable}
                    refusal={removing.refusal}
                    deleting={removing.busy}
                    onDelete={(force) => void deleteWorkspace(removing.workspace, force)}
                    onCancel={() => setRemoving(undefined)}
                  />
                )}

                {/* The one question charter asks before it ends a chat, wherever the row was pressed
          (the operator: *"closing session should ask confirmation"*). */}
                {/* A finished task's Merge… or Discard branch…, asked from its row's menu (#1534). */}
                {taskBranchActs.asking}
                {/* NO-3's two questions: a chat's record dropped, and a chat started again. */}
                {forgetting && (
                  <ChatAsk
                    title={`Forget ${forgetting.name}?`}
                    says={`purlis stops keeping ${forgetting.name}, and no later launch tries to start it again. This cannot be undone.`}
                    answer="Forget chat"
                    trouble={forgetting.trouble}
                    busy={forgetting.busy}
                    onAnswer={() => void forgetChat(forgetting.id, forgetting.name)}
                    onCancel={() => setForgetting(undefined)}
                    afterClose={afterForgetAsk}
                  />
                )}
                {startApproving && (
                  <StartApproveAsk
                    plane={plane}
                    asked={startApproving}
                    onCancel={() => setStartApproving(undefined)}
                    onApproved={() => {
                      // The refusal it answered is put away; any other line stays.
                      const answered = startApproving.way;
                      setReport((was) => (was?.ways?.includes(answered) ? undefined : was));
                      setStartApproving(undefined);
                    }}
                  />
                )}
                {approving && (
                  <ChatAsk
                    title={`Approve ${approving.approval.profile}?`}
                    says={`${approving.name} starts on the profile ${approving.approval.profile}. Approving records that purlis may run its command on this machine, and starts nothing: Retry now starts the chat.`}
                    answer="Approve"
                    trouble={approving.trouble}
                    busy={approving.busy}
                    onAnswer={() => void approveWaiting(approving)}
                    onCancel={() => setApproving(undefined)}
                    afterClose={afterApproveAsk}
                  >
                    {/* The picker's own sentence, line and mark (`ProfileApproval.tsx`, ruling V69). */}
                    <ApprovalSentence row={approving.approval} />
                    <ProfileMeta row={approving.approval} />
                  </ChatAsk>
                )}
                {stopAsk && (
                  <ChatAsk
                    title={stopTitle(stopAsk)}
                    says={stopSays(stopAsk)}
                    answer={stopAnswer(stopAsk)}
                    trouble={stopAsk.trouble}
                    busy={stopAsk.busy}
                    onAnswer={() => void stopAsked()}
                    onCancel={() => setStopAsk(undefined)}
                  />
                )}
                {stopAllAsk && (
                  <ChatAsk
                    title={stopAllTitle(stopAllAsk)}
                    says={stopAllSays(stopAllAsk)}
                    answer={stopAllAnswer(stopAllAsk)}
                    trouble={stopAllAsk.trouble}
                    busy={stopAllAsk.busy}
                    onAnswer={() => void stopAllAsked()}
                    onCancel={() => setStopAllAsk(undefined)}
                  />
                )}
                {taskEndAsk && (
                  <TaskEndAsk
                    asked={taskEndAsk}
                    onBelow={(belowToo) => setTaskEndAsk({ ...taskEndAsk, belowToo })}
                    onAnswer={(way) =>
                      void endTaskNow(
                        taskEndAsk.session,
                        way,
                        taskEndAsk.belowToo && taskEndAsk.below.length > 0,
                        taskEndAsk,
                      )
                    }
                    onCancel={() => setTaskEndAsk(undefined)}
                  />
                )}
                {freshening && (
                  <ChatAsk
                    title={`Start ${freshening.name} fresh?`}
                    says={`The project's instructions changed since it started (${freshening.files.join(", ")}). Its program ends, and it starts again on what is there now, as a new conversation.`}
                    answer="Start fresh"
                    warns={oneChatInterrupted({
                      key: String(freshening.session),
                      name: freshening.name,
                      harness: null,
                      cwd: null,
                      state: freshening.doing,
                      ownWork: freshening.ownWork,
                      backgroundTurn: freshening.backgroundTurn,
                    })}
                    trouble={freshening.trouble}
                    busy={freshening.busy}
                    onAnswer={() => void startFresh()}
                    onCancel={() => setFreshening(undefined)}
                  />
                )}
                {restartingTask && (
                  <ChatAsk
                    title={`Restart ${restartingTask.name}?`}
                    says="It is a task. Its program ends and starts again on the same conversation, once its turn has ended. It stays a task, and still owes its report."
                    answer="Restart it"
                    busy={false}
                    onAnswer={() => {
                      const { session } = restartingTask;
                      setRestartingTask(undefined);
                      restartNow.current(session);
                    }}
                    onCancel={() => setRestartingTask(undefined)}
                  />
                )}
                {endingChat && (
                  <EndingChat
                    offer={endingChat.offer}
                    smart={endingChat.smart}
                    running={endingChat.running}
                    closing={endingChat.closing}
                    ownTabs={endingChat.ownTabs}
                    drafts={endingChat.drafts}
                    fromALink={endingChat.fromALink}
                    shell={endingChat.shell}
                    onEnd={(stop, keepDrafts) => {
                      const { offer: ending, session, drafts } = endingChat;
                      setEndingChat(undefined);
                      void (async () => {
                        // The drafts first: they are the chat's, and go with it.
                        if (!(await settleDrafts(drafts, keepDrafts))) return;
                        // Stop them: the core closes the chat too, in the one step. Keep them: the
                        // ordinary close.
                        if (stop && session !== undefined) void closeStopping(session, "close");
                        else void carryOut(ending);
                      })();
                    }}
                    onSmartClose={(stop, keepDrafts) => {
                      const { session, drafts } = endingChat;
                      setEndingChat(undefined);
                      if (session === undefined) return;
                      void (async () => {
                        if (!(await settleDrafts(drafts, keepDrafts))) return;
                        if (stop && !(await closeStopping(session, "smart_close"))) return;
                        await beginSmartClose(session);
                      })();
                    }}
                    onCancel={() => setEndingChat(undefined)}
                  />
                )}

                {keptOnClose.length > 0 && (
                  <KeepAsk
                    key={`${keptOnClose[0].folder} ${keptOnClose.length}`}
                    plane={plane}
                    folder={keptOnClose[0].folder}
                    workspace={keptOnClose[0].workspace}
                    keptTo={keptOnClose[0].keptTo}
                    onDone={() => setKeptOnClose((was) => was.slice(1))}
                  />
                )}

                {picking && (
                  <StartChat
                    options={picking.options}
                    pending={picking.pending}
                    repo={pickingInRepo}
                    // Where it starts, by name (R8): the workspace in front, else the project.
                    place={pickingIn ?? projectName(plane)}
                    starting={starting}
                    readStage={readStage}
                    prefer={"prefer" in picking.where ? picking.where.prefer : undefined}
                    pool={"pool" in picking.where ? picking.where.pool?.name : undefined}
                    trouble={pickerTrouble}
                    // Only beside the refusal it was asked for, never beside a later one.
                    signIn={
                      pickerSignIn !== undefined && pickerSignIn.refused === pickerTrouble
                        ? pickerSignIn.profile
                        : undefined
                    }
                    // The Ask to sign it in (#2035): the picker closes, and the Inbox, where it waits,
                    // comes to the front. The person starts the chat again once it is signed in.
                    onShowSignIn={() => {
                      setPicking(undefined);
                      setPickerTrouble(undefined);
                      setPickerSignIn(undefined);
                      showInbox();
                    }}
                    limitOffer={
                      pickerLimit !== undefined && pickerLimit.refused === pickerTrouble
                        ? pickerLimit.offer
                        : undefined
                    }
                    limitProfile={pickerLimit?.profile}
                    onRaiseLimit={async (values) => {
                      const pool = pickerLimit?.offer.pool;
                      if (pool === undefined) return false;
                      const refused = await saveLimits(pool, values);
                      if (refused === undefined) return true;
                      // The offer stays beside the words that replaced the refusal.
                      setPickerTrouble(refused);
                      setPickerLimit((was) => was && { ...was, refused });
                      return false;
                    }}
                    onStart={(profile, persona, footer, label, newBranch, withoutSandbox, runsOn) =>
                      void startPicked(
                        profile,
                        persona,
                        footer,
                        label,
                        newBranch,
                        withoutSandbox,
                        runsOn ?? null,
                      )
                    }
                    onApprove={(
                      profile,
                      persona,
                      footer,
                      shown,
                      label,
                      newBranch,
                      withoutSandbox,
                      runsOn,
                    ) =>
                      void approveAndStart(
                        profile,
                        persona,
                        footer,
                        shown,
                        label,
                        newBranch,
                        withoutSandbox,
                        runsOn ?? null,
                      )
                    }
                    runsOn={picking.runsOn}
                    // Only beside the refusal it came with, never beside a later one (#4536).
                    poolUnavailable={
                      pickerPoolGone !== undefined && pickerPoolGone.refused === pickerTrouble
                        ? pickerPoolGone.pool
                        : undefined
                    }
                    // Pool setup, where the pool is added or mended: the picker closes, as it does
                    // for Settings, since the chat it was for starts after the mend.
                    secretAsk={
                      pickerAsk === undefined || pickerSecret === undefined
                        ? undefined
                        : {
                            ask: pickerAsk,
                            profile: pickerSecret.profile,
                            variable: pickerSecret.asked.variable,
                            entries: () => secretAskEntries(plane, pickerAsk),
                            busy: pickerSecretBusy,
                            said: pickerSecretSaid,
                            onAnswer: answerPickerAsk,
                            onSaid: setPickerSecretSaid,
                          }
                    }
                    onPoolSetup={(pool) => {
                      setPicking(undefined);
                      setPickerTrouble(undefined);
                      setPickerPoolGone(undefined);
                      // On the pool the picker named (#4484): setup carries on with it.
                      void (pool === undefined ? Promise.resolve() : commands.poolSetupPool(pool))
                        .catch(() => undefined)
                        .then(() => showView(POOL_SETUP_VIEW, POOL_SETUP_TITLE));
                    }}
                    fixing={pickerFixing}
                    onFix={(id) => void fixInPicker(id)}
                    // A refused profile is mended on its page in Settings, its command focused (NO-8,
                    // #1296): the picker closes, since the chat it was for starts after the mend.
                    onOpenSettings={(group, setting) => {
                      setPicking(undefined);
                      setPickerTrouble(undefined);
                      openSettingsAt({ group, setting });
                    }}
                    onCancel={() => {
                      // A start the session host has not answered stops here (#4264).
                      if (startingNow.current) {
                        startLeft.current = true;
                        startRun.current += 1;
                        startingNow.current = false;
                        setStarting(false);
                        void commands.cancelChatStart(plane).catch(() => undefined);
                      }
                      setPicking(undefined);
                      setPickerTrouble(undefined);
                    }}
                  />
                )}
              </OnlyInFront>
              {/* While a pane is dragged or placed (#4763, #4764): where it would land and what
                it would do, drawn over everything at fixed places, placement mode's targets,
                and what a reader is told. At the window's root, never inside a section that
                hides itself, as the Notices section in the Inbox does with none to show. */}
              {paneDrag.layer}
            </SessionOffersLent>
          </Lent>
        </QuestionsHere.Provider>
      </PlacingHere>
    </ChatToUseHere.Provider>
  );
  return createPortal(
    <ProjectShownProvider value={inFront}>
      <InTheWindow host={host} slot={slot} shown={inFront} onLeftCovered={leftCovered} />
      {/* What is New (#6002, #6004): the status bar's card and every Notice this project draws
      read its seen record, as the Inbox's rows do. */}
      <SeenRowsOf.Provider value={seenRows}>
        <AsLastShown>{view}</AsLastShown>
      </SeenRowsOf.Provider>
    </ProjectShownProvider>,
    host,
  );
});

/**
 * What a project's window lends everything it draws: its chats' states (`ChatsHere`), the
 * chats a file can be handed to (`ReferenceChats`, FM-9), the way to open Ask {persona}
 * for one of its chats (`useAskPersona`), which a Notice on a pane calls, and the way to open
 * a task's brief (`useOpenBrief`, #1494).
 */
function Lent({
  chats,
  tasksBelow,
  doings,
  skillRuns,
  references,
  memoryTargets,
  readOnly,
  mendable,
  personas,
  askPersona,
  useSkill,
  chatSkillSets,
  brief,
  answer,
  studioChats,
  children,
}: {
  chats: ComponentProps<typeof ChatsHere.Provider>["value"];
  /** Each session's tasks, by its number (#1491). */
  tasksBelow: ReadonlyMap<number, TasksBelow>;
  /** What each working chat is doing, for the one line under its name (#1493). */
  doings: DoingsOf;
  /** The skill run open in each chat, for the chip on its tab (SK-3). */
  skillRuns: RunsOf;
  references: ChatsForReferences;
  /** The project's memory stores and LIVE workspaces, for the memory lists' Move (#1190). */
  memoryTargets: MemoryTargets;
  /** Why the project is read-only, or undefined (#826): what a write control off the catalogue
   *  is turned off with (`ReadOnlyLine.ReadOnlySaid`). */
  readOnly: string | undefined;
  /** Whether Edit as TOML may still mend the project's unreadable manifest (#1759,
   *  `ReadOnlyLine.ReadOnlyMendable`). */
  mendable: boolean;
  /** Every persona's mark here, and how to read them again (#1449). */
  personas: ReturnType<typeof usePersonaMarks>;
  askPersona: OpenAskPersona;
  /** Opens Use skill… for a chat (`useOpenUseSkill`, #2775). */
  useSkill: OpenUseSkill;
  /** Each open chat's skill set as a whole, for the chip on its pane (#2775, #3953). */
  chatSkillSets: ChatSkillSets;
  /** Opens the Brief panel for a task (`useOpenBrief`, #1494). */
  brief: OpenBrief;
  /** Opens the Answer dialog for a task's question (`useOpenAnswer`, #1551). */
  answer: OpenAnswer;
  /** The chats the Skill studio started, still open (#5185). */
  studioChats: StudioChatsLent;
  children: ReactNode;
}) {
  return (
    <ChatsHere.Provider value={chats}>
      <TasksBelowLent below={tasksBelow}>
        <DoingsHere.Provider value={doings}>
          <SkillRunsHere.Provider value={skillRuns}>
            <ReferenceChats.Provider value={references}>
              <MemoryStores.Provider value={memoryTargets}>
                <ReadOnlySaid.Provider value={readOnly}>
                  <ReadOnlyMendable.Provider value={mendable}>
                    <PersonaMarks.Provider value={personas.marks}>
                      <ReloadPersonaMarks.Provider value={personas.reload}>
                        <AskPersonaOpener value={askPersona}>
                          <UseSkillOpener value={useSkill}>
                            <ChatSkillSetsHere.Provider value={chatSkillSets}>
                              <BriefOpener value={brief}>
                                <AnswerOpener value={answer}>
                                  <StudioChatsHere.Provider value={studioChats}>
                                    {children}
                                  </StudioChatsHere.Provider>
                                </AnswerOpener>
                              </BriefOpener>
                            </ChatSkillSetsHere.Provider>
                          </UseSkillOpener>
                        </AskPersonaOpener>
                      </ReloadPersonaMarks.Provider>
                    </PersonaMarks.Provider>
                  </ReadOnlyMendable.Provider>
                </ReadOnlySaid.Provider>
              </MemoryStores.Provider>
            </ReferenceChats.Provider>
          </SkillRunsHere.Provider>
        </DoingsHere.Provider>
      </TasksBelowLent>
    </ChatsHere.Provider>
  );
}

/** Redraws a component when the theme in force changes: what a workspace's tint is taken from
 *  (charter-app#281). */
function followTheme(changed: () => void): () => void {
  return onDrawn(() => changed());
}

/** A workspace's colour when it has a hue to tint with, else `null` (charter-app#281). */
function colourWithHue(colour: string | null | undefined): string | null {
  return isWorkspaceColour(colour) ? (colour ?? null) : null;
}

/**
 * The status line, with how many of the project's chats are running read off the store here
 * (SC-3): the count changes with most moves, and the project view is not redrawn for it.
 */
function StatusLineHere({
  sessions,
  settled,
  workspacePanels,
  workspace,
  ...line
}: Omit<ComponentProps<typeof StatusLine>, "running" | "state"> & {
  /** The project's open chats, in the quit warning's order. */
  sessions: readonly number[];
  settled: boolean;
  /** The focused workspace's panels, read off their store for the same reason (#1160). */
  workspacePanels: WorkspacePanels;
  workspace: string | undefined;
}) {
  const running = useChatsSelect(useChatsHere(), (states) =>
    runningIn({
      ending: sessions.map((session) => ({ state: stateOf(states, session) })),
      settled,
    }),
  );
  const state = useWorkspacePanels(workspacePanels, workspace, (one) => one, FOR_THE_LINE);
  return <StatusLine {...line} state={state} running={running} />;
}

/**
 * **What each reader of the focused workspace's panels draws from them** (#1160), by field:
 * a reader is drawn again when a field it names moves, or anything of git's (`sameFor`). Each
 * list is the reader's whole read — a field it draws and does not name is drawn stale.
 *
 * - The explorer, the Repos view and the Changes view: the clones, and git (`GIT_SIDE`).
 * - The status line: those, and the todos it counts (`StatusLine.todoCount`).
 * - This view: those, and the personas the catalogue builds rows from, and of the contributed
 *   panels only what their tabs say (`attentionPanels`). Their rows — a memory saved, a record
 *   written — are the `Panels` regions' alone, and a record's palette rows are built beside
 *   them (`sessionOffers.tsx`, #1160).
 */
const FOR_THE_GIT_SIDE = sameFor(GIT_SIDE);
const LINE_READS: readonly PanelsField[] = [...GIT_SIDE, "todos", "todos_refused"];
const FOR_THE_LINE = sameFor(LINE_READS);
const WINDOW_READS: readonly PanelsField[] = [...LINE_READS, "done_todos", "personas", "persona"];
const forTheWindowsFields = sameFor(WINDOW_READS);
function forTheWindow(a: WorkspaceState, b: WorkspaceState): boolean {
  return (
    forTheWindowsFields(a, b) &&
    (a.panels?.contributed === b.panels?.contributed ||
      tabsSaid(a.panels?.contributed) === tabsSaid(b.panels?.contributed))
  );
}
/** The state of each chat owed a restart, by session (#1888). */
type OwedStates = Readonly<Record<number, State>>;
/** Whether two readings name the same chats in the same states. */
function sameStates(a: OwedStates, b: OwedStates): boolean {
  const sessions = Object.keys(a);
  return (
    sessions.length === Object.keys(b).length &&
    sessions.every((session) => a[Number(session)] === b[Number(session)])
  );
}
/** What the contributed panels' tabs say, which is all this view reads of them. */
function tabsSaid(panels: readonly PanelView[] | undefined): string {
  return JSON.stringify(
    (panels ?? []).map(({ key, title, mark, order }) => [key, title, mark, order]),
  );
}

/** Whether two counts by name say the same, so a report that changes neither is not sent. */
function sameCounts(
  one: Readonly<Record<string, number>>,
  other: Readonly<Record<string, number>>,
): boolean {
  const names = Object.keys(one);
  return (
    names.length === Object.keys(other).length && names.every((name) => one[name] === other[name])
  );
}

/** A chat a quit would end, before what it is doing is looked up (`PlaneReport.ending`). */
type Leaving = { session: number; chat: Omit<Ending, "state"> };

/** The size a session starts at. The pane it lands in tells it the real one at once. */
const STARTING_SIZE = { columns: 80, rows: 24 };

/** `record` without `key`. */
function withoutKey<T>(record: Readonly<Record<number, T>>, key: number): Record<number, T> {
  return Object.fromEntries(Object.entries(record).filter(([held]) => Number(held) !== key));
}

/** The core's sentence for a restart held back by a permission prompt, where it sent none. */
const NOT_YET = "It is waiting on a permission prompt, so it restarts once that is answered.";

/** The notes a relaunch or a Resume says about a chat, by the family of their cause. */
const CHAT_NOTES = ["chat-resumed", "chat-guessed", "chat-fresh", "chat-untaken"] as const;

/**
 * **The cause of one note about a chat**, or none when the chat has nothing to say of that kind.
 *
 * A resumed chat's names the conversation it was resumed by (D-NO2-9), so a dismissal of one
 * conversation's note never hides another's. A new chat's and a guessed one's name the chat
 * alone: the core says why a conversation was lost and what was guessed in words, and exposes no
 * id of the conversation that was lost to name the occurrence by.
 */
/** A resumed chat's note, for a chat that was resumed. */
const resumedNote = (chat: OpenChat) => chatNote("chat-resumed", chat) ?? "chat-resumed";

function chatNote(family: (typeof CHAT_NOTES)[number], chat: OpenChat): string | undefined {
  if (family === "chat-resumed")
    return chat.resumed ? `chat-resumed:${chat.session}:${chat.resumed}` : undefined;
  if (family === "chat-guessed") return chat.guessed ? `chat-guessed:${chat.session}` : undefined;
  if (family === "chat-untaken") return chat.untaken ? `chat-untaken:${chat.session}` : undefined;
  return chat.fresh ? `chat-fresh:${chat.session}` : undefined;
}

/**
 * The strip a chat started in `cwd` is filed on until the plane says: the focused workspace's,
 * or the one for chats outside every workspace when it starts in no directory at all. One
 * function for a chat the picker started and a shell tab, so the two are filed alike.
 */
function filedFor(cwd: string | null, focused: string | undefined): string {
  return cwd === null ? OUTSIDE : (focused ?? OUTSIDE);
}

/**
 * **Whether no row the picker would list can start** (FR-29c, D-963-1): no harness the machine
 * was looked at for is installed, and every profile is a built-in of one of them. Any harness
 * installed keeps the picker as it was (ADR 0022). So does a profile the project declares,
 * which names its own command the look does not check, and a kind the look does not know:
 * the picker says for itself whether a start works. An empty list keeps it too, for the
 * picker's own reason to give, and so does a look that came back with nothing.
 */
function nothingCanStart(options: StartOptions, found: HarnessSetupFound | null): boolean {
  const looked = new Set((found?.harnesses ?? []).map((one) => one.name));
  return (
    looked.size > 0 &&
    !(found?.harnesses ?? []).some((one) => one.installed) &&
    options.profiles.length > 0 &&
    options.profiles.every((row) => row.source === "built-in" && looked.has(row.kind))
  );
}

/**
 * What a chat's default name puts before its number (charter-app#254): the persona it adopted,
 * or — with none — the program it runs, by the word the plane calls its harness. `steward 3`,
 * `claude 4`. Not the profile's name, which is the operator's word for an account (`work 4`
 * would name the account, not the program). A chat on neither is a shell, and says its own
 * name alone, as it always has. Every path a tab opens by goes through here.
 */
/** What `showPlaced` needs of a chat the core started and placed. */
interface Placed {
  session: number;
  name: string;
  persona: string | null;
  harness: string | null | undefined;
  label: string | null;
  workspace?: string | null;
}

function whoOf(persona: string | null, harness: string | null | undefined): string | null {
  return persona ?? harness ?? null;
}

/** **Go to it on a chat's arrival card** (#4759), told to the project that drew it. */
const GO_TO_ARRIVED = "purlis:go-to-arrived";
const goToArrived = (plane: PlaneId, session: number) =>
  window.dispatchEvent(new CustomEvent(GO_TO_ARRIVED, { detail: { plane, session } }));

/**
 * **Chat `session`'s Notices, said in the Notices centre** (#4759): its pane's Notices drawn
 * there (`NoticeInCentre`), each telling `onHeld` as it comes and goes, by one function per chat,
 * so they are not drawn again for a new one each render.
 */
function CentredFor({
  session,
  onHeld,
  children,
}: {
  session: number;
  onHeld: (session: number, cause: string, held: boolean) => void;
  children: ReactNode;
}) {
  const told = useCallback(
    (cause: string, held: boolean) => onHeld(session, cause, held),
    [onHeld, session],
  );
  return <NoticeInCentre.Provider value={told}>{children}</NoticeInCentre.Provider>;
}

/** No asks: a chat in no pane has its asks in the Inbox's own list (#4759), not drawn twice. */
const NO_ASKS_HERE: readonly Shown[] = [];

/** Where a chat in no workspace works, in the words its handoff note uses (`plane root`). */
const ROOT_WORD = "plane root";

/** What the strip calls the workspace a chat is filed in, for the Inbox's rows (qw359). */
const workspaceNamed = (filed: string): string => (filed === OUTSIDE ? OUTSIDE_TITLE : filed);

/** The core's word for the same place, where a finished task worked there
 *  (`FinishedTask.place`). */
const CORE_ROOT_WORD = "project root";

/** No row of the Chats list is on screen: what the clock is read with until the list says. */
const NO_ROWS_DRAWN: ReadonlySet<number> = new Set();

const NO_NEEDS: readonly Needing[] = [];
/**
 * Whether the listing of `focus`'s workspace has answered without its branch (#1152): its repo's
 * branches were listed and it is not among them, or the workspace no longer holds the repo. A
 * listing still being read, or refused, has not said so. A focus on a repo's own folder (#1152)
 * is gone once the workspace no longer holds the repo.
 */
function focusGone(state: WorkspaceState, focus: Place): boolean {
  const repos = state.panels?.repos;
  if (repos !== undefined && !repos.includes(focus.repo)) return true;
  if (focus.piece === null) return false;
  if (state.piecesRefused[focus.repo] !== undefined) return false;
  const listed = state.pieces[focus.repo];
  return listed !== undefined && !listed.some((one) => one.piece === focus.piece);
}

/** The queue the catalogue is built with: none (#1034, `withQueue`). */
const NO_QUEUE: readonly number[] = [];
const NO_ASKS: readonly Shown[] = [];

/**
 * **What each pane remembers of the task it shows** (#1486), by tab, pane and chat: the path it
 * can read now where it can, and otherwise the one it last could. Only for what a pane shows
 * now. Answers `was` itself when nothing changed, so it can be held as state.
 */
function recalledCrumbs(
  was: ReadonlyMap<string, Crumbs>,
  live: ReadonlyMap<string, Crumbs>,
  tabs: Tabs,
): ReadonlyMap<string, Crumbs> {
  const next = new Map<string, Crumbs>();
  for (const id of tabs.order)
    for (const one of shownIn(tabs, id)) {
      const key = `${id}:${one.pane}:${one.session}`;
      const said = live.get(key) ?? was.get(key);
      if (said !== undefined) next.set(key, said);
    }
  const same =
    next.size === was.size &&
    [...next].every(([key, said]) => {
      const had = was.get(key);
      return (
        had !== undefined &&
        had.elsewhere === said.elsewhere &&
        had.path.length === said.path.length &&
        had.path.every((chat, at) => chat === said.path[at])
      );
    });
  return same ? was : next;
}

/** How long a pointer's press on the tab in front waits before it goes back to the main chat:
 *  long enough for a second click to make it a double-click, which renames. */
const A_DOUBLE_CLICK = 250;
/** How long after a drag a click on a tab is the drag's own, and not a press. */
const AFTER_A_DRAG = 400;

/** Whether any tab already shows `session`, in any of its panes. */
function alreadyShows(tabs: Tabs, session: number): boolean {
  return tabs.order.some((id) => panesOf(tabs, id).some((pane) => pane.session === session));
}

/** What the window says of a link's close that came while a question was on screen (#2761). */
const CLOSE_HANDED_NOT_ASKED =
  "A link asked to end chats here that have theme drafts while a question was on screen, so it was not asked: end each here with End…, and answer Keep or Revert.";

/** What the window says when a link's close names chats it cannot ask End… about as one
 *  (#2761, #4756): more than one, or one not open here. */
const CLOSE_HANDED_NOT_HERE =
  "A link asked to end chats that have theme drafts, and they cannot be asked about together here: end each here with End…, and answer Keep or Revert.";

/** The programs the Skill studio lists the own skills of, by the word the project calls each. */
const HARNESS_KINDS: readonly HarnessKind[] = ["claude", "opencode", "codex"];

/** A view tab the core put back, as the view it names. */
function refOf(view: ViewTab): ViewRef {
  return { from: view.from, view: view.view, key: view.key };
}

/**
 * Gives a side's view the keyboard, once it is drawn (#1673): the stop of its first tree, as an
 * editor's ⌘⇧E lands in the explorer, past the headings of Explorer's sections (#1677), or its
 * first stop in a view with no tree — or Search's box (#1676). A view with nothing to stop on, as
 * Changes has (it is read, never pressed), leaves the keyboard where it was.
 */
function giveViewTheKeyboard(view: ViewId): void {
  const panel = `[role="tabpanel"][data-view="${view}"]`;
  (view === "search"
    ? document.querySelector<HTMLElement>(`${panel} input[type="search"]`)
    : // A tree of the tree family is its own stop until a row has had the keyboard (#2911).
      (document.querySelector<HTMLElement>(
        `${panel} :is([role="treegrid"][tabindex="0"], [role="treegrid"] [tabindex="0"])`,
      ) ??
      // In triage (⌘⇧I, #6016), the first ask's lead: its Answer, or an answer that grants
      // nothing. Else the Inbox's first ask, past the scope switch above its list (qw359).
      (view === "asks" && triaging()
        ? firstLead(document.querySelector(`${panel} .inbox-list`))
        : null) ??
      document.querySelector<HTMLElement>(`${panel} .inbox-list [tabindex="0"]`) ??
      document.querySelector<HTMLElement>(`${panel} [tabindex="0"]`))
  )?.focus();
}

/** What a record from before tabs were recorded says is pinned (#4762, #4935): chats by
 *  session, view tabs by `tabs.viewKey`. */
type PinnedBefore = { chats: ReadonlySet<number>; views: ReadonlySet<string> };

/**
 * `tabs` with the pins `before` says carried onto the tabs that open on them (#4935): each tab
 * whose first pane is a chat or a view `before` names is pinned, read once, at the upgrade, and
 * the tab's own pin from then on.
 */
function pinnedAsBefore(tabs: Tabs, before: PinnedBefore): Tabs {
  if (before.chats.size === 0 && before.views.size === 0) return tabs;
  return tabs.order.reduce((next, id) => {
    const lead = contentsOf(next, id)[0]?.content;
    const was =
      (lead?.kind === "session" && before.chats.has(lead.session)) ||
      (lead?.kind === "view" && before.views.has(viewKey(lead.view)));
    return was ? setPinned(next, id, true) : next;
  }, tabs);
}

/**
 * The view tabs, as the record keeps them: every tab whose first pane is a view.
 *
 * A view on the far side of a split is not a tab of its own and is not recorded — a chat's
 * split is not recorded either, and both come back as what they are: the chat as its own tab,
 * the view as nothing, to be opened again.
 */
function viewTabsOf(tabs: Tabs): ViewTab[] {
  return tabs.order.flatMap((id, at) => {
    const lead = contentsOf(tabs, id)[0]?.content;
    if (lead?.kind !== "view") return [];
    // A new memory's tab holds nothing on disk yet, so there is nothing to bring back.
    if (isMemory(lead.view) && memoryRefOf(lead.view.key)?.slug === DRAFT) return [];
    return [
      {
        from: lead.view.from,
        view: lead.view.view,
        key: lead.view.key,
        title: tabs.byId[id].name,
        workspace: lead.workspace === OUTSIDE ? null : lead.workspace,
        at,
        active: tabs.inFront === id,
        pinned: tabs.byId[id].pinned === true,
        split: splitOf(tabs, lead.view) ?? null,
      },
    ];
  });
}

/** The stop the person is being asked about (#1448): what was asked, of which chat, and how
 *  the answer is going. */
type StopAsking = StopAsked & { session: number; busy: boolean; trouble?: string };
type StopAllAsking = StopAllAsked & { session: number; busy: boolean; trouble?: string };

/** What a chat with no tab is called: what its tab would say, were it opened (#1447). */
function untabbedName(chat: OpenChat): string {
  return pressedName({
    session: chat.session,
    chat: chat.name,
    who: whoOf(chat.persona, chat.harness),
    label: chat.label,
  });
}

/** **What a chat is called by its own name, who runs it and its label** (#1447, #5308): the
 *  label the person gave it, else its persona or program before its name. One answer for a
 *  listed chat ({@link untabbedName}) and one this window started that is not listed yet. */
function pressedName(chat: PressedChat): string {
  return chat.label ?? (chat.who ? `${chat.who} ${chat.chat}` : chat.chat);
}

/** Chat `session`'s own name, as a pane showing it records it ({@link Content}'s `chat`). */
function paneChatName(tabs: Tabs, session: number): string | undefined {
  for (const id of tabs.order)
    for (const { content } of contentsOf(tabs, id))
      if (content.kind === "session" && content.session === session) return content.chat;
  return undefined;
}

/** The tab chat `session` is the own chat of, where it has one: the tab it leads. */
function ownTabOf(tabs: Tabs, session: number): number | undefined {
  return tabs.order.find((id) => chatOf(tabs, id) === session);
}

/** The tab that holds chat `session`, in any of its panes, where one does. */
function tabHolding(tabs: Tabs, session: number): number | undefined {
  return tabs.order.find((id) => panesOf(tabs, id).some((pane) => pane.session === session));
}

/**
 * **What a chat is called in every list** (#1484): the name of the tab it is the own chat of,
 * and for any other chat, what its tab would say. The Chats list, the explorer and the needs-you
 * list all read this, so a task is one name everywhere and never its number.
 */
function shownName(tabs: Tabs, chat: OpenChat): string {
  // **A chat beside a tab's own chat is not called what the tab is** (#1489, #5283): the tab's
  // name is its first pane's. Every other chat says its own name wherever it is drawn.
  const own = ownTabOf(tabs, chat.session);
  return own !== undefined ? tabs.byId[own].name : untabbedName(chat);
}

/** The waiting chat's Notice's way out that asks first (NO-3), and where a Cancel goes back to. */
const FORGET_THIS_CHAT = "Forget this chat…";
/** The waiting chat's Notice's way to its profile's approval question (#1246), and where a
 *  Cancel goes back to. */
const REVIEW_AND_APPROVE = "Review and approve…";

/** A task's Restart chat, asked first where it was pressed (#1489, #1462): the question, and
 *  the answer that restarts it. */
function restartAsked(name: string) {
  return {
    way: "now" as const,
    says: `Restart ${name}? Its program ends and starts again on the same conversation, once its turn has ended. It stays a task, and still owes its report.`,
    answer: "Restart it",
    busy: false,
    act: "restart" as const,
  };
}

/**
 * **How many times one of `sessions` has moved to a state that is not mid-turn** (#1468): a
 * turn ending, a task ending or failing. A turn beginning is not counted, so a read that
 * follows this is not made again for what cannot have changed its answer.
 */
function useTurnsEnded(chats: ReturnType<typeof useChatsHere>, sessions: readonly number[]) {
  const [ended, setEnded] = useState(0);
  useEffect(() => {
    const was = new Map<number, string>();
    const read = () => {
      const states = chats.store.statesFor(chats.plane);
      let moved = false;
      for (const session of sessions) {
        const state = stateOf(states, session);
        const before = was.get(session);
        was.set(session, state);
        if (before !== undefined && before !== state && state !== "running") moved = true;
      }
      if (moved) setEnded((n) => n + 1);
    };
    read();
    return chats.store.subscribe(read);
  }, [chats, sessions]);
  return ended;
}

/**
 * **A studio tab put back from the record** (#3681): an older launch kept a studio tab per scope,
 * keyed by it; it comes back as the one studio tab, on that scope's Library.
 */
function studioPutBack(plane: string, kept: ViewRef): ViewRef {
  // A skill's own tab (SK-3) comes back as its page in the studio (#3682).
  const view = studioOfSkillTab(kept) ?? kept;
  if (!isSkillStudio(view)) return view;
  const opening = studioOpening(view, () => true);
  if (opening.place !== undefined) openViewAt(studioHistoryOf(plane, opening.tab), opening.place);
  return opening.tab;
}
