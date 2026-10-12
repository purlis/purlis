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
  "ChatStrip.tsx": [
    "<button>",
    "<button>",
  ],
  "ChatsSection.tsx": [
    "<button className=\"chats-hidden-go\">",
    "<button className=\"at-limit-settings\">",
    "<button className=\"needs-you-mark rolled-up\">",
  ],
  "CreateOnForge.tsx": [
    "<button>",
    "<button>",
    "<button>",
    "<button>",
    "<button>",
    "<button>",
  ],
  "Doctor.tsx": [
    "<button className=\"doctor-fix\">",
    "<button className=\"doctor-settings\">",
    "<button className={`status-doctor doctor-tone-${tone}`}>",
  ],
  "DoorPage.tsx": [
    "<button className=\"welcome-link\">",
  ],
  "FindBar.tsx": [
    "<button>",
    "<button>",
    "<button>",
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
  "KillSwitch.tsx": [
    "<button className=\"title-kill-switch\">",
  ],
  "LayoutPanes.tsx": [
    "<button className={offer.id === \"pane.close\" && !closesOnly(offer.does) ? \"ends-a-chat\" : undefined}>",
  ],
  "LivesHere.tsx": [
    "<button>",
  ],
  "MakeItAProject.tsx": [
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
  "PanelSection.tsx": [
    "<button className=\"panel-view\">",
  ],
  "PlaneUpdated.tsx": [
    "<button className=\"plane-updated fresh-mark\">",
  ],
  "RegionFrame.tsx": [
    "<button className=\"panel-view half-away\">",
    "<button className=\"region-toggle\">",
  ],
  "RepoPicker.tsx": [
    "<button>",
    "<button>",
  ],
  "SaidLink.tsx": [
    "<button className=\"said-link\">",
  ],
  "SessionRecordTab.tsx": [
    "<button className=\"record-ref\">",
  ],
  "StatusLine.tsx": [
    "<button className=\"status-alerts\">",
    "<button className=\"status-alerts\">",
  ],
  "StripMarks.tsx": [
    "<button className={clsx( offer.id === \"pane.close\" && !closesOnly(offer.does) && \"ends-a-chat\", bare && \"bare\", )}>",
    "<button className={arrived ? \"show-more arrived\" : \"show-more\"}>",
    "<button className={clsx(\"closer\", closesOnly(offer.does) && \"keeps\", minimise && \"minimise\")}>",
    "<button className=\"gear\">",
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
    "<button>",
    "<button>",
    "<button className=\"panel-view\">",
    "<button>",
    "<button>",
    "<button>",
  ],
  "Views.tsx": [
    "<button className=\"panel-view\">",
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
  "WorkspaceStrip.tsx": [
    "<button className={root ? \"plane-root\" : undefined}>",
  ],
  "YourRepos.tsx": [
    "<button className=\"welcome-link\">",
    "<button className=\"welcome-link\">",
    "<button className=\"welcome-link\">",
  ],
  "cardLink.tsx": [
    "<button className=\"said-link\">",
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
  "settings/GrantedList.tsx": [
    "<button>",
    "<button>",
    "<button>",
    "<button className=\"ui-setting-reset\">",
  ],
  "settings/NetworkLists.tsx": [
    "<button>",
  ],
  "settings/RawToml.tsx": [
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
    "<button className={one.sub ? \"ui-settings-sub\" : undefined}>",
    "<input>",
    "<input type=\"checkbox\" className=\"m-0\">",
  ],
  "settings/discover.tsx": [
    "<button>",
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
