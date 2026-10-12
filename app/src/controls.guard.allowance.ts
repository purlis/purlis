// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The control guard's allowance** (`controls.guard.test.ts`, #1837): each button, field,
 * select, table or menu the window drew before it had a class per control, by file, one per
 * line as its tag is written (`<button className="tab-close">`).
 *
 * **This list only shrinks.** When a control takes its `ui-` class (`docs/ui-primitives.md`,
 * "The control classes"), delete its line: the guard fails on an entry that no longer names
 * anything, on an unclassed control that is not here, and on a file's list that grows past its
 * ceiling. Never add a line; give the control its class instead. A file whose list empties is
 * deleted from here.
 */
// prettier-ignore
export const CONTROLS_ALLOWANCE: Readonly<Record<string, readonly string[]>> = {
  "About.tsx": [
    "<button className=\"title-about\">",
  ],
  "ActivityBar.tsx": [
    "<button className=\"activity-tab\">",
  ],
  "ActivityTab.tsx": [
    "<button className=\"vault-secret\">",
    "<button className=\"activity-more\">",
    "<button className=\"panel-view activity-answer\">",
  ],
  "AnswerQuestion.tsx": [
    "<button className=\"panel-view\">",
    "<textarea className=\"ui-field answer-text\">",
    "<button className=\"panel-view\">",
    "<button className=\"panel-view\">",
  ],
  "App.tsx": [
    "<button>",
    "<button>",
  ],
  "AwaySummary.tsx": [
    "<button className=\"away-part\">",
    "<button className=\"away-part\">",
  ],
  "Brief.tsx": [
    "<button className=\"pane-brief\">",
  ],
  "ChangeActions.tsx": [
    "<button>",
    "<button className=\"ends-it\">",
  ],
  "ChatsSection.tsx": [
    "<input className=\"chats-filter-text\">",
    "<button className=\"chats-hidden-go\">",
    "<button className=\"at-limit-settings\">",
    "<button className=\"needs-you-mark rolled-up\">",
  ],
  "Cockpit.tsx": [
    "<button>",
    "<button>",
    "<button>",
    "<button className=\"cockpit-action\">",
  ],
  "CreateOnForge.tsx": [
    "<button>",
    "<button>",
    "<button>",
    "<button>",
    "<button>",
    "<button>",
  ],
  "DispatchesTab.tsx": [
    "<select className=\"search-scope\">",
    "<select className=\"search-scope\">",
    "<select className=\"search-scope\">",
    "<select className=\"search-scope\">",
    "<table className=\"vault-secrets dispatches\">",
    "<button className=\"dispatch-more\">",
    "<button className=\"dispatch-changes\">",
    "<button className=\"dispatch-discard\">",
    "<button className=\"dispatch-discard\">",
    "<button className=\"vault-secret\">",
    "<button className=\"vault-secret\">",
  ],
  "Doctor.tsx": [
    "<button className=\"doctor-fix\">",
    "<button className=\"doctor-settings\">",
    "<button className={`status-doctor doctor-tone-${tone}`}>",
  ],
  "DoorPage.tsx": [
    "<button className=\"welcome-link\">",
  ],
  "Explorer.tsx": [
    "<input>",
    "<button className=\"changed-toggle\">",
    "<button className=\"ignored-toggle\">",
    "<button>",
    "<button>",
    "<button className=\"spot\">",
    "<button className=\"spot spot-root\">",
    "<button className=\"spot\">",
    "<button>",
    "<button className=\"file-node\">",
    "<button className=\"file-node\">",
  ],
  "FindBar.tsx": [
    "<input>",
    "<button>",
    "<button>",
    "<button>",
  ],
  "FinishedTasks.tsx": [
    "<button className=\"finished-count\">",
    "<button className=\"finished-clear\">",
    "<button className=\"finished-name\">",
    "<button className=\"finished-brief\">",
    "<button className=\"finished-reopen\">",
    "<button className=\"finished-reopen\">",
    "<button className=\"finished-clear\">",
    "<button className=\"finished-clear\">",
    "<button className=\"finished-clear\">",
    "<button className=\"finished-reopen\">",
    "<button className=\"finished-changes\">",
    "<button className=\"finished-clear\">",
    "<button className=\"report-changed-link\">",
  ],
  "FirstRun.tsx": [
    "<button>",
    "<button>",
    "<button className=\"welcome-link\">",
  ],
  "FirstTaskTab.tsx": [
    "<button>",
    "<button>",
    "<button>",
  ],
  "ForgeQuestion.tsx": [
    "<button>",
    "<button>",
  ],
  "ForgeSignIn.tsx": [
    "<button>",
    "<button>",
  ],
  "ForgeTokenSignIn.tsx": [
    "<input className=\"ui-field\">",
  ],
  "GetProject.tsx": [
    "<button>",
    "<button>",
    "<button>",
    "<button>",
    "<button className=\"welcome-link\">",
  ],
  "GitIdentityForm.tsx": [
    "<button>",
    "<button>",
  ],
  "GitMissing.tsx": [
    "<button>",
    "<button>",
    "<button>",
  ],
  "HarnessCard.tsx": [
    "<button className=\"pane-harness\">",
  ],
  "HarnessSetupTab.tsx": [
    "<button>",
    "<button>",
    "<button>",
  ],
  "ImportCliLogin.tsx": [
    "<button>",
    "<button>",
  ],
  "Inbox.tsx": [
    "<button className={option.allows ? \"inbox-answer allows\" : \"inbox-answer\"}>",
    "<button className=\"inbox-go\">",
    "<button className=\"inbox-go\">",
    "<input>",
    "<button>",
    "<button className=\"inbox-go\">",
    "<button className=\"inbox-go\">",
    "<button className={answer.allows ? \"inbox-answer allows\" : \"inbox-answer\"}>",
    "<button className=\"inbox-go\">",
    "<button className=\"inbox-go\">",
  ],
  "InboxElsewhere.tsx": [
    "<button className=\"inbox-go\">",
    "<button className=\"inbox-go\">",
    "<button className=\"inbox-go\">",
  ],
  "KillSwitch.tsx": [
    "<button className=\"title-kill-switch\">",
  ],
  "LivesHere.tsx": [
    "<button>",
  ],
  "MakeItAProject.tsx": [
    "<button>",
    "<button>",
  ],
  "MemoryArchiveTab.tsx": [
    "<input>",
    "<button className=\"row\">",
    "<button className=\"panel-more\">",
    "<button>",
    "<button>",
  ],
  "MemoryTab.tsx": [
    "<button>",
    "<button>",
    "<button>",
    "<button>",
    "<button>",
    "<button>",
  ],
  "NeedsYou.tsx": [
    "<button className={`needs-you-button${asking ? \"\" : \" muted\"}${arrived && asking ? \" arrived\" : \"\"}`}>",
  ],
  "NotCloned.tsx": [
    "<button>",
    "<button>",
    "<button className=\"ends-it\">",
  ],
  "Onboarding.tsx": [
    "<button>",
    "<button>",
    "<button>",
  ],
  "PaneCrumbs.tsx": [
    "<button className={at === 0 ? \"crumb own\" : \"crumb between\"}>",
  ],
  "PanelList.tsx": [
    "<button>",
    "<input>",
    "<button className=\"panel-more\">",
    "<button className=\"row\">",
    "<button className=\"row\">",
    "<button className=\"row-action\">",
  ],
  "PanelSection.tsx": [
    "<button className=\"panel-view\">",
  ],
  "Panels.tsx": [
    "<input>",
  ],
  "PersonaMarkPicker.tsx": [
    "<input className=\"ui-field\">",
  ],
  "PlaneUpdated.tsx": [
    "<button className=\"plane-updated fresh-mark\">",
  ],
  "PlaneView.tsx": [
    "<button className={root ? \"plane-root\" : undefined}>",
    "<button>",
    "<button>",
    "<button className={clsx( offer.id === \"pane.close\" && !closesOnly(offer.does) && \"ends-a-chat\", bare && \"bare\", )}>",
    "<button className={arrived ? \"show-more arrived\" : \"show-more\"}>",
    "<button className={clsx(\"closer\", closesOnly(offer.does) && \"keeps\", minimise && \"minimise\")}>",
    "<button className=\"gear\">",
    "<button className={offer.id === \"pane.close\" && !closesOnly(offer.does) ? \"ends-a-chat\" : undefined}>",
  ],
  "RegionFrame.tsx": [
    "<button className=\"panel-view half-away\">",
    "<button className=\"region-toggle\">",
  ],
  "RepoInstructionsTab.tsx": [
    "<button>",
    "<button>",
    "<button>",
  ],
  "RepoPicker.tsx": [
    "<button>",
    "<button>",
  ],
  "SaidLink.tsx": [
    "<button className=\"said-link\">",
  ],
  "SavingView.tsx": [
    "<button className=\"panel-view\">",
    "<button className=\"panel-view\">",
    "<button className=\"panel-view\">",
    "<button className=\"panel-view\">",
    "<table className=\"saving-repos\">",
    "<button className=\"panel-view\">",
    "<button>",
    "<button className=\"save-indicator-where\">",
    "<button className=\"save-indicator-save\">",
  ],
  "SearchTab.tsx": [
    "<input className=\"search-query\">",
    "<select className=\"search-scope\">",
    "<button className=\"search-switch\">",
    "<button className=\"search-more\">",
  ],
  "SessionRecordTab.tsx": [
    "<button className=\"record-ref\">",
  ],
  "StatusLine.tsx": [
    "<button className=\"status-alerts\">",
    "<button className=\"status-alerts\">",
  ],
  "TabChip.tsx": [
    "<button className=\"tab-tasks-hand needs-you-mark\">",
    "<button className=\"tab-tasks-counts\">",
    "<button className=\"tasks-menu-place\">",
  ],
  "TabRename.tsx": [
    "<input className=\"tab-rename\">",
  ],
  "TaskAway.tsx": [
    "<button>",
    "<button>",
  ],
  "TaskChangesTab.tsx": [
    "<button>",
    "<button>",
    "<button>",
    "<button className=\"vault-secret\">",
  ],
  "TaskEnd.tsx": [
    "<button>",
    "<button className=\"ends-it\">",
    "<button className=\"task-end\">",
  ],
  "Updates.tsx": [
    "<button className={`status-update update-${state.kind}`}>",
    "<button className=\"status-pin\">",
  ],
  "VaultSignIn.tsx": [
    "<input className=\"ui-field\">",
    "<button>",
    "<button>",
    "<button className=\"panel-view\">",
    "<button>",
    "<button>",
    "<button>",
  ],
  "VaultTab.tsx": [
    "<button className=\"panel-view\">",
    "<button className=\"panel-view\">",
    "<input>",
    "<button className=\"panel-view\">",
    "<button>",
    "<table className=\"vault-secrets\">",
    "<button className=\"panel-view\">",
    "<input>",
    "<button>",
    "<button className=\"panel-view\">",
    "<button className=\"panel-view\">",
    "<button>",
    "<button className=\"vault-reveal\">",
    "<button className=\"vault-secret\">",
    "<input className=\"ui-field\">",
  ],
  "Views.tsx": [
    "<button className=\"panel-view\">",
    "<button>",
    "<button className=\"panel-view\">",
    "<button className=\"panel-view view-refresh\">",
    "<button className=\"panel-view change-push\">",
  ],
  "Welcome.tsx": [
    "<button className=\"welcome-door\">",
    "<button className=\"welcome-link\">",
    "<button>",
    "<button>",
  ],
  "WorkspaceRepos.tsx": [
    "<button className=\"panel-view\">",
    "<button className=\"panel-view\">",
    "<button className=\"ends-it\">",
  ],
  "YourRepos.tsx": [
    "<button className=\"welcome-link\">",
    "<button className=\"welcome-link\">",
    "<button className=\"welcome-link\">",
  ],
  "cardLink.tsx": [
    "<button className=\"said-link\">",
  ],
  "editor/BranchTree.tsx": [
    "<button className=\"ignored-toggle\">",
  ],
  "editor/PieceDiff.tsx": [
    "<button>",
    "<button>",
  ],
  "editor/PieceFiles.tsx": [
    "<button>",
    "<button>",
    "<button className=\"piece-image-play\">",
    "<button>",
    "<button>",
    "<button>",
  ],
  "editor/ToYourEditor.tsx": [
    "<button>",
    "<button>",
  ],
  "references.tsx": [
    "<button>",
    "<button>",
    "<button>",
  ],
  "settings/Collection.tsx": [
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<button>",
    "<button>",
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<button>",
    "<button>",
    "<button>",
    "<button>",
    "<button>",
  ],
  "settings/DispatchGrants.tsx": [
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<table className=\"dispatch-table\">",
  ],
  "settings/GrantedList.tsx": [
    "<button>",
    "<button>",
    "<button>",
    "<button className=\"ui-setting-reset\">",
  ],
  "settings/NetworkLists.tsx": [
    "<button>",
  ],
  "settings/OutLink.tsx": [
    "<button className=\"ui-setting-reset\">",
  ],
  "settings/RawToml.tsx": [
    "<button>",
    "<textarea className=\"ui-field ui-raw-text\">",
    "<button>",
    "<button>",
  ],
  "settings/SettingsTab.tsx": [
    "<button className=\"ui-setting-reset\">",
    "<button className=\"ui-setting-reset\">",
    "<button>",
    "<button className=\"ui-setting-reset\">",
    "<input>",
  ],
  "settings/accounts.tsx": [
    "<button className=\"ui-setting-reset\">",
    "<button>",
  ],
  "settings/components.tsx": [
    "<input>",
    "<button className={one.sub ? \"ui-settings-sub\" : undefined}>",
    "<input>",
    "<textarea className=\"ui-field\">",
    "<input type=\"checkbox\" className=\"m-0\">",
  ],
  "settings/discover.tsx": [
    "<button>",
  ],
  "settings/dispatch.tsx": [
    "<table>",
    "<button>",
    "<button>",
    "<button className=\"ui-setting-reset\">",
  ],
  "settings/harnessDeclarations.tsx": [
    "<button>",
  ],
  "settings/sandbox.tsx": [
    "<button>",
  ],
  "settings/thisMachine.tsx": [
    "<button className=\"ui-setting-reset\">",
  ],
  "settings/workspace.tsx": [
    "<button className=\"panel-view\">",
  ],
};
