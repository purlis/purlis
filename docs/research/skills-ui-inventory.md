# Inventory of skill surfaces today

Wayfinder task [#3624](https://github.com/purlis/purlis/issues/3624), for the map [#3623](https://github.com/purlis/purlis/issues/3623). This file records what the window shows and offers about skills on `origin/main` at `8a1c1f0a2` (2026-10-28). It records facts only and makes no judgements.

**Sources.** The React code under `app/src/`, the panels the core answers (`crates/purlis-core/src/skills/studio.rs`, `app/src-tauri/src/personaskills.rs`), the committed story screenshots, and five screenshots the operator took on 2026-10-28. No window was run for this file. Each code link points at file and line on this commit. Each Tauri command is named as `bindings.ts` invokes it.

**Words used here.**

- A **surface** is a place a person reaches: a tab, a sidebar view, a dialog, a section of a page, a notice or a chip.
- An **action** is anything the person presses, picks, types into, switches or drags.
- **Flag** means the Skill studio preview (Settings › You › Previews › Skill studio). Unless a section says otherwise, every surface here is drawn only while the flag is on.

## Contents

1. [The flag: Settings › You › Previews](#1-the-flag-settings--you--previews)
2. [Palette and tab-menu rows](#2-palette-and-tab-menu-rows)
3. [Sidebar Skills view](#3-sidebar-skills-view)
4. [Skill studio tab](#4-skill-studio-tab)
   - [4.1 Toolbar and states](#41-toolbar-and-states)
   - [4.2 First open: Skills to move, and the three starts](#42-first-open-skills-to-move-and-the-three-starts)
   - [4.3 Change notice, pins-refused line, router line](#43-change-notice-pins-refused-line-router-line)
   - [4.4 Level chips (drop targets)](#44-level-chips-drop-targets)
   - [4.5 Skills table](#45-skills-table)
   - [4.6 Change panels (New skill, Edit, Rename, Move grant, Delete)](#46-change-panels)
   - [4.7 Draft skills](#47-draft-skills)
   - [4.8 Harness skills (each program's own)](#48-harness-skills)
   - [4.9 Skill sources and install](#49-skill-sources-and-install)
   - [4.10 Deleted skills](#410-deleted-skills)
5. [A skill's own tab](#5-a-skills-own-tab)
6. [Grade scorecard tab and grade chat tab](#6-grade-scorecard-tab-and-grade-chat-tab)
7. [Skills inside a chat](#7-skills-inside-a-chat)
   - [7.1 Skills chip on the pane](#71-skills-chip-on-the-pane)
   - [7.2 Use skill… dialog](#72-use-skill-dialog)
   - [7.3 Skill run chip on the tab](#73-skill-run-chip-on-the-tab)
   - [7.4 Chat notices: Skills missing, Draft ready, Skills changed](#74-chat-notices)
8. [Workspace settings: Skills summary row](#8-workspace-settings-skills-summary-row)
9. [Persona page: Skills summary row](#9-persona-page-skills-summary-row)
10. [Pictures](#10-pictures)
11. [Every action, by surface](#11-every-action-by-surface)

---

## 1. The flag: Settings › You › Previews

- **Reached from:** the Settings tab, at You › Previews.
- **Shows:** one row labelled "Skill studio". Its help text reads "Lists every skill your chats get, and gives each chat the project's own skills. Open it from the palette: Open Skill studio." The row says it writes "this machine".
- **Actions:**
  - **Skill studio switch** turns the preview on or off. It calls `set_skill_studio_on`, and a refusal shows as the row's error.
- **States:** the window reads the flag once through `skill_studio_on`. Until the core answers, it treats the flag as off ([skillStudio.ts:177-186](../../app/src/skillStudio.ts#L177-L186)).
- **Code:** [settings/previews.tsx:26-47](../../app/src/settings/previews.tsx#L26-L47).

When the flag is off, none of the surfaces below is drawn: the palette rows, the Skills view, the chip, the run chip and the notices are all gone. A Skill studio tab that is already open shows "The Skill studio is not on yet" (§4.1).

## 2. Palette and tab-menu rows

- **Reached from:** the command palette. The Use skill… rows also appear on each chat tab's menu.
- **Shows:** rows in the catalogue. Each row is offered only while the flag is on.

| Row title | Note it carries | Does | Code |
|---|---|---|---|
| Open Skill studio | "Every skill this project's chats get, and why any other is left out." | Opens the studio scoped to **a chat at the project's root**, whatever workspace the title bar shows | [actions.ts:2534-2541](../../app/src/actions.ts#L2534-L2541) |
| Open in Skill studio (one per workspace) | "`<ws>`: every skill its chats get, and why any other is left out." | Opens the studio scoped to that workspace | [actions.ts:2237-2250](../../app/src/actions.ts#L2237-L2250) |
| Open in Skill studio (one per persona) | "`<persona>`: every skill its chats get in any workspace…" | Opens the studio scoped to a chat as that persona | [actions.ts:2447-2460](../../app/src/actions.ts#L2447-L2460) |
| Show the Skills view | "The focused workspace's skills by level, and what needs you, on the left." | Shows the sidebar Skills view | [actions.ts:1743-1748](../../app/src/actions.ts#L1743-L1748), [actions.ts:1504](../../app/src/actions.ts#L1504) |
| Use skill… / Use skill… (`N of M skills`[, near the budget]) | "Chat `<name>`: one of its skills, typed into it for you to send." plus the budget line | Opens Use skill… (§7.2) for that chat. The palette lists the front chat's row only; every other chat's row appears on its tab menu only (`menuOnly`). Shell tabs get no row. | [actions.ts:1996-2019](../../app/src/actions.ts#L1996-L2019) |

The studio row ids are `skills.studio` and `skills.studio:<scope>` ([skillStudio.ts:119-131](../../app/src/skillStudio.ts#L119-L131), [app/src-tauri/src/skillstudio.rs:18-27](../../app/src-tauri/src/skillstudio.rs#L18-L27)). The notices, the settings link and the persona row in §7–§9 run these same ids.

## 3. Sidebar Skills view

- **Reached from:** the activity bar's Skills mark (the `Sparkles` icon, [ActivityBar.tsx:189-190](../../app/src/ActivityBar.tsx#L189-L190)), or the palette row "Show the Skills view".
- **Mounting:** the view is mounted at [PlaneView.tsx:8953-8963](../../app/src/PlaneView.tsx#L8953-L8963). It reads nothing until it is first shown.
- **Code:** data comes from [SkillsView.tsx](../../app/src/SkillsView.tsx) and drawing from [SkillsList.tsx](../../app/src/SkillsList.tsx). Rows, groups, marks and menu rows come from [skillsListing.ts](../../app/src/skillsListing.ts). Folds and "Only this chat's" are kept in the layout file ([skillsViewPrefs.ts](../../app/src/skillsViewPrefs.ts)).

**What it shows:**

- **Header:** the button "Open Skill studio" and the menu button "New skill…".
- **"Only this chat's":** a toggle button, drawn only while a chat tab is in front.
- **Notice:** an error from the last menu action.
- **Empty state:** "This project has no skills of its own yet". The body reads "Every chat still gets the skills listed here. Start one of the project's own from New skill…". This empty state has no buttons.
- **Filter:** shown once the list is long enough.
- **Tree of groups:** each group shows its title and a count. The groups come in this order: Needs you, Built-in, Project, Workspace, Personas, Repo, Local, then one group per program (for example "Claude Code"), then "Not in this chat". At the project's root, some groups are left out ([skillsListing.ts:199-238](../../app/src/skillsListing.ts#L199-L238)).
- **Rows:** each row shows the skill's name and at most one mark: "Needs review", "Off", "Left out", "Hidden by the router" or "Lint" ([skillsListing.ts:95-101](../../app/src/skillsListing.ts#L95-L101)). On hover or keyboard focus, a tip shows the description and the reason for the mark.

**What it reads:**

- `skill_studio(plane, place, persona)` and `skill_persona_skills(plane, place)` ([SkillsView.tsx:83-99](../../app/src/SkillsView.tsx#L83-L99)).
- `place` is the title bar's workspace. While "Only this chat's" is on and a chat is in front, `place` is the chat's own workspace and persona instead ([SkillsView.tsx:132-140](../../app/src/SkillsView.tsx#L132-L140)).
- While narrowed, the view also reads `chat_skills(plane, session)` and narrows the rows to that chat's real set ([SkillsView.tsx:163-189](../../app/src/SkillsView.tsx#L163-L189)).

**Actions:**

| Action | What it does | Command | Code |
|---|---|---|---|
| Open Skill studio | Opens the studio scoped to the title bar's workspace, or to the root when no workspace is picked | (opens a tab) | [SkillsList.tsx:106-108](../../app/src/SkillsList.tsx#L106-L108), [SkillsView.tsx:202-203](../../app/src/SkillsView.tsx#L202-L203) |
| New skill… › Ask a chat to draft a skill | Opens a new chat with a draft prompt typed in and not sent | `skill_draft_chat` (via the `purlis-skill-chat` event, [PlaneView.tsx:5116-5135](../../app/src/PlaneView.tsx#L5116-L5135)) | [SkillsList.tsx:109-126](../../app/src/SkillsList.tsx#L109-L126), [SkillsView.tsx:205-212](../../app/src/SkillsView.tsx#L205-L212) |
| New skill… › Browse sources | Opens the studio and scrolls to Skill sources, moving focus there | (opens a tab) | same |
| New skill… › New skill | Opens the studio with the New skill form open | (opens a tab) | same |
| Only this chat's (toggle) | Narrows the list to the front chat's set, read from that chat's workspace and persona. The setting is kept in the layout file. | `chat_skills` | [SkillsList.tsx:128-132](../../app/src/SkillsList.tsx#L128-L132) |
| Filter | Filters rows by name | — | [SkillsList.tsx:142-150](../../app/src/SkillsList.tsx#L142-L150) |
| Fold / unfold a group | Twist, ← →, Enter or a click. The fold is kept in the layout file. While a filter is typed, every group is open. | — | [SkillsList.tsx:153-164](../../app/src/SkillsList.tsx#L153-L164) |
| Row: Enter or click | Opens the skill's own tab (§5) | (opens a tab) | [SkillsList.tsx:195](../../app/src/SkillsList.tsx#L195), [SkillsView.tsx:253](../../app/src/SkillsView.tsx#L253) |
| Row menu › Use in this chat | Only while a chat is in front. Opens Use skill… on this skill (§7.2). It is disabled with "The chat in front does not get it." when the row is marked Off, Left out or Needs review. | `chat_skills`, then `use_skill` | [skillsListing.ts:539-547](../../app/src/skillsListing.ts#L539-L547) |
| Row menu › Turn on in `<ws>` / Turn off in `<ws>` | Turns a project skill on or off for one workspace. Otherwise it is disabled with a reason, such as built-in ("It ships with purlis, and cannot be turned off."), not a project skill, no workspace picked, or not granted. | `skill_turn` | [skillsListing.ts:382-394](../../app/src/skillsListing.ts#L382-L394), [SkillsView.tsx:224-239](../../app/src/SkillsView.tsx#L224-L239) |
| Row menu › Pin to this workspace / Unpin from this workspace | Pins the skill for the workspace's chats. When the pin is unused, "Pinned, not used: …" shows under the row. Otherwise it is disabled with a reason, such as built-in, a persona's own skill, a program's own skill, no workspace picked, or not in this workspace's chats. | `skill_pin` | [skillsListing.ts:434-474](../../app/src/skillsListing.ts#L434-L474) |
| Row menu › Show in the Explorer view | Opens Explorer on the skill's folder. Otherwise it is disabled with a reason, such as a folder outside the project or a clone that belongs to another workspace. | — | [skillsListing.ts:488-505](../../app/src/skillsListing.ts#L488-L505), [skillsListing.ts:551-556](../../app/src/skillsListing.ts#L551-L556) |
| Dismiss (error notice) | Clears the notice | — | [SkillsView.tsx:272-283](../../app/src/SkillsView.tsx#L272-L283) |

The row menu is opened by right-click, Shift+F10 or the menu key. While a change is in flight, Turn and Pin show "A change is on its way." ([SkillsList.tsx:232-276](../../app/src/SkillsList.tsx#L232-L276)).

**States:**

- **Loading:** "Reading the project's skills…" ([SkillStarts.tsx:122-129](../../app/src/SkillStarts.tsx#L122-L129)).
- **Error:** "purlis could not read this project's skills", followed by the core's sentence ([SkillStarts.tsx:111-119](../../app/src/SkillStarts.tsx#L111-L119)).
- **Empty:** the project has no skill of its own, and the starts empty state shows. While the list is narrowed to a chat, the starts are hidden ([SkillsView.tsx:249](../../app/src/SkillsView.tsx#L249)).
- **Nothing reaches a chat:** "No skill reaches a chat here."
- **Filter matches nothing:** the shared no-match sentence.
- **Flag off:** the view is not drawn, and its palette row is gone. A side that was left on Skills is held empty until the core says whether the flag is on ([PlaneView.tsx:2386-2408](../../app/src/PlaneView.tsx#L2386-L2408)).
- **Chat in front:** "Only this chat's" and "Use in this chat" appear.
- **No chat in front:** neither appears.

**Pictures:**

- Stories: [by-level](../../app/src/__screenshots__/SkillsList.stories.tsx/skillslist-by-level_purlis_dark-chromium-linux.png), [no-skills-of-its-own](../../app/src/__screenshots__/SkillsList.stories.tsx/skillslist-no-skills-of-its-own_purlis_dark-chromium-linux.png), [a-pin-no-chat-reads](../../app/src/__screenshots__/SkillsList.stories.tsx/skillslist-a-pin-no-chat-reads_purlis_dark-chromium-linux.png). Each also exists in light, high-contrast, classic and WebKit variants.
- Operator: the left column of [3](skills-ui-inventory/operator-2026-10-28-3.png) through [7](skills-ui-inventory/operator-2026-10-28-7.png).

## 4. Skill studio tab

- **Reached from:**
  - the palette rows in §2;
  - the Skills view's "Open Skill studio" and "New skill…";
  - a chat notice's "Open in Skill studio" (§7.4);
  - Use skill…'s "Open Skill studio · …" buttons (§7.2);
  - Settings › Workspace › Skills (§8);
  - the persona page row (§9);
  - its own "Whose chats" picker, which opens another studio tab.
- **Tab title:** "Skill studio" at the root, or "Skill studio · `<name>`" for a workspace or persona ([skillStudio.ts:42-55](../../app/src/skillStudio.ts#L42-L55)).
- **Code:** [SkillStudioTab.tsx](../../app/src/SkillStudioTab.tsx), mounted at [Views.tsx:677-686](../../app/src/Views.tsx#L677-L686).
- **Reads:**
  - `skill_studio(plane, workspace, persona)` each time it opens and after each change ([SkillStudioTab.tsx:170-182](../../app/src/SkillStudioTab.tsx#L170-L182));
  - `skill_deletes_cut_short` once it is on ([SkillStudioTab.tsx:185-210](../../app/src/SkillStudioTab.tsx#L185-L210));
  - `skills_deleted` ([SkillStudioTab.tsx:211-223](../../app/src/SkillStudioTab.tsx#L211-L223));
  - `skill_drafts`, through `useSkillDrafts`.

The whole tab is **one scrolling page**. From top to bottom ([SkillStudioTab.tsx:547-835](../../app/src/SkillStudioTab.tsx#L547-L835)):

1. Toolbar
2. A line saying whose chats the page covers
3. Change notice
4. Skills to move notice
5. Change panel
6. Draft skills
7. The three starts
8. Pins-refused line
9. Router line
10. Level chips
11. Skills table
12. Harness skills (one section per program)
13. Skill sources
14. Deleted skills

### 4.1 Toolbar and states

**Shows:**

- the "Whose chats" picker;
- for a persona scope only, the "Add a skill" combobox and its Add button;
- the buttons "New skill" and "Ask a chat to draft a skill";
- a line under the toolbar: "Every skill `<a chat at the project's root | a chat in ws | a chat as persona>` gets, and why any other is left out." For a persona it adds " A persona brings its skills into every workspace."

**Actions:**

| Action | What it does | Command | Code |
|---|---|---|---|
| Whose chats picker | Options: "A chat at the project's root", "A chat in `<ws>`" for each workspace, and the current persona. Picking one opens **another** studio tab for that scope. | (opens a tab) | [SkillStudioTab.tsx:370-383](../../app/src/SkillStudioTab.tsx#L370-L383) |
| Add a skill (combobox) + Add | Persona scope only. Adds a skill name to the persona's skill list. Its suggestions are the studio's skills that the persona does not name. | `persona_skill(…, "add")` | [SkillStudioTab.tsx:384-393](../../app/src/SkillStudioTab.tsx#L384-L393), [personaSkills.tsx:36-105](../../app/src/personaSkills.tsx#L36-L105) |
| New skill | Opens the New skill panel (§4.6). It is disabled while that panel is open. | — | [SkillStudioTab.tsx:394-400](../../app/src/SkillStudioTab.tsx#L394-L400) |
| Ask a chat to draft a skill | Opens a new chat with the draft prompt typed in and not sent. The draft lands at the scope's level. | `skill_draft_chat` | [SkillStudioTab.tsx:401-406](../../app/src/SkillStudioTab.tsx#L401-L406), [skillDraftAsks.ts:80-83](../../app/src/skillDraftAsks.ts#L80-L83) |

**States** ([SkillStudioTab.tsx:225-236](../../app/src/SkillStudioTab.tsx#L225-L236)):

- **Error:** "purlis could not read this project's skills", followed by the core's sentence.
- **Loading:** "Reading the project's skills…".
- **Flag off:** the empty state "The Skill studio is not on yet", with the body "Turn it on in Settings › You › Previews to try it while it is being built."
- **Chat in front:** this does not change the page. The studio's scope comes from the tab's view key, not from the front chat.

### 4.2 First open: Skills to move, and the three starts

**Skills to move:**

- **Shows:** a news Notice labelled "Skills to move", drawn while the core reports `first.unmoved`. Its text reads: "This project keeps N skill(s) where only some chat programs read them. Moving them gives every chat each one, whichever program it runs on, in one commit that can be undone. The commit reaches everyone who works in the project, and a teammate on an older purlis opens the project read-only until they update." ([SkillStudioTab.tsx:344-357](../../app/src/SkillStudioTab.tsx#L344-L357))
- **Actions:**
  - **Move them** runs the doctor's `project-skills` fix through `plane_doctor_fix`. Afterwards a notice says "Skills moved", "Some skills were not moved" or "The skills were not moved" ([SkillStudioTab.tsx:316-343](../../app/src/SkillStudioTab.tsx#L316-L343)).
  - **Dismiss** hides the notice for this tab only, held in component state.

**The three starts:**

- **Shows:** drawn while the project has no skill of its own (`first.starts`) and no change panel is open. The empty state reads "This project has no skills of its own yet", with the body "Every chat still gets the skills listed here. Start one of the project's own:" ([SkillStarts.tsx:65-107](../../app/src/SkillStarts.tsx#L65-L107)). The table of skills the chats still get is drawn under it.
- **Actions:** three buttons, each with a line under it ([SkillStarts.tsx:33-57](../../app/src/SkillStarts.tsx#L33-L57), [SkillStudioTab.tsx:359-367](../../app/src/SkillStudioTab.tsx#L359-L367)):
  - **Ask a chat to draft a skill** ("Say what you want in a chat. Its draft comes back here to try and accept.") calls `skill_draft_chat`.
  - **Browse sources** ("Install a reviewed copy of a skill from a git repository.") scrolls to Skill sources and moves focus there.
  - **New skill** ("Write one yourself, from a blank template.") opens the New skill panel.
- **Pictures:** operator [3](skills-ui-inventory/operator-2026-10-28-3.png).

### 4.3 Change notice, pins-refused line, router line

**Change notice** ([SkillStudioTab.tsx:413-427](../../app/src/SkillStudioTab.tsx#L413-L427)):

- **Shows:** the outcome of the last change. A refusal is a trouble notice whose label is the action's failure wording, for example "The skill was not moved". News is "Skill changed" with the core's notes, "Skill deleted", "Skill delete cut short" or "A skill delete could not be finished".
- **Actions:**
  - **Restore** appears only on "Skill deleted". It calls `skill_restore`.
  - **Dismiss** clears the notice.

**Pins-refused line** ([SkillStudioTab.tsx:559-571](../../app/src/SkillStudioTab.tsx#L559-L571), [SkillPins.tsx:67-97](../../app/src/SkillPins.tsx#L67-L97)):

- **Shows:** drawn only for a workspace or persona scope whose chats read none of its pins. It shows the reason, then "Unpin some, or edit `<file>`." or a Remove button.
- **Actions:**
  - **Remove it** removes the one bad pin through `skill_pin(…, false)`.

**Router line** ([SkillStudioTab.tsx:1285-1304](../../app/src/SkillStudioTab.tsx#L1285-L1304)):

- **Shows:** drawn only when an extension's router is chosen or the router choice has a problem. It shows the core's sentence and, for each open chat, a line "Chat N: …" with the router's timing or why it fell back.
- **Actions:** none. The router is not chosen anywhere in the window; this line only reports it.

### 4.4 Level chips (drop targets)

- **Shows:** one chip per level the scope shows, such as "project", "local project", "workspace `<ws>`" and "local workspace `<ws>`" ([SkillStudioTab.tsx:573-591](../../app/src/SkillStudioTab.tsx#L573-L591)). The group's accessible name is "Drop a skill on a level to move it there"; nothing visible on the page says so.
- **Actions:**
  - **Drag a table row onto a chip** moves the skill to that level. It first asks `skill_move_needs_grant`. A move into the project's level opens the Move grant panel (§4.6); any other move calls `skill_move` at once ([SkillStudioTab.tsx:284-293](../../app/src/SkillStudioTab.tsx#L284-L293), [SkillStudioTab.tsx:536-546](../../app/src/SkillStudioTab.tsx#L536-L546)).
- **Pictures:** operator [3](skills-ui-inventory/operator-2026-10-28-3.png) and [4](skills-ui-inventory/operator-2026-10-28-4.png) show the chip row.

### 4.5 Skills table

- **Code:** a `ControlTable` (a plain HTML table) with seven columns: **Skill · Level · Granted to · In a chat · What it does · Works on · Change** ([SkillStudioTab.tsx:592-824](../../app/src/SkillStudioTab.tsx#L592-L824)).
- **Rows:** one per copy of a skill. When one copy hides another, the hidden copy gets its own row after the copy a chat gets.
- **Drawn only when** the answer has at least one skill.

**What each cell holds:**

- **Skill:** a ghost button with the name in code font. Pressing it opens the skill's own tab.
- **Level:** the level's label, for example "built-in" or "project". A skill kept on this machine only also gets a **Share** button.
- **Granted to:** a grant picker for a project skill; empty otherwise.
- **In a chat:** up to seven things in one cell:
  1. the state text: "On", "On, pinned", "On, `<hidden>` (on …)", the unchecked text, or "Left out: `<why>`";
  2. the **Pin** switch;
  3. for a built-in, "It ships with purlis, and cannot be turned off.";
  4. for each clash, "Also: `<said>`." and a **Rename the project's copy** button;
  5. the **Accept its folder** button;
  6. the not-granted sentence;
  7. in a workspace scope, a **Turn on/off in `<ws>`** button.
- **What it does:** the skill's description.
- **Works on:** one collapsible badge per program, "`<Program>`: full | partial | widens the chat | handed to no chat". There may also be a "Widens the chat" line, a "`<Program>`: skills not supported" badge for each unsupported program, and lint lines ([SkillCompatBadges.tsx:29-88](../../app/src/SkillCompatBadges.tsx#L29-L88)).
- **Change:** **Edit**, **Rename**, a persona's **Add to `<persona>`** or **Remove**, the **Move to…** picker, and **Delete** ([SkillRowActions.tsx:27-55](../../app/src/SkillRowActions.tsx#L27-L55)). A built-in, harness or repo row has no level in this scope, so it gets none of Edit, Rename, Move to… or Delete.

**Actions:**

| Action | What it does | Command | Code |
|---|---|---|---|
| Open `<name>` (the name) | Opens the skill's own tab | (opens a tab) | [SkillStudioTab.tsx:622-635](../../app/src/SkillStudioTab.tsx#L622-L635) |
| Share | Tooltip "Give it to the team: it is committed with the project". Moves a machine-only copy into the shared level. | `skill_share` | [SkillStudioTab.tsx:636-649](../../app/src/SkillStudioTab.tsx#L636-L649) |
| Granted to picker | Options: "The project's root only", "Every workspace", "The root and `<ws…>`" | `skill_grant` | [SkillStudioTab.tsx:650-666](../../app/src/SkillStudioTab.tsx#L650-L666), [skillChanges.ts:110-131](../../app/src/skillChanges.ts#L110-L131) |
| Pin switch ("Pinned" / "Pinned, not used") | Workspace or persona scope only, on the first pinnable row of a name. Keeps the skill listed for those chats past any budget. | `skill_pin` | [SkillStudioTab.tsx:502-535](../../app/src/SkillStudioTab.tsx#L502-L535), [SkillPins.tsx:26-61](../../app/src/SkillPins.tsx#L26-L61) |
| Rename the project's copy | Shown on a clash with a program's own skill. Opens the Rename panel. | — | [SkillStudioTab.tsx:680-705](../../app/src/SkillStudioTab.tsx#L680-L705) |
| Accept its folder | A level folder that waits for the person's accept | `skill_level_accept` | [SkillStudioTab.tsx:706-723](../../app/src/SkillStudioTab.tsx#L706-L723) |
| Turn on in `<ws>` / Turn off in `<ws>` | Workspace scope only. Turns a project skill on or off for that workspace. | `skill_turn` | [SkillStudioTab.tsx:730-741](../../app/src/SkillStudioTab.tsx#L730-L741), [SkillStudioTab.tsx:1242-1263](../../app/src/SkillStudioTab.tsx#L1242-L1263) |
| Works on badge (expand) | A `<details>` disclosure that lists what the program says | — | [SkillCompatBadges.tsx:51-66](../../app/src/SkillCompatBadges.tsx#L51-L66) |
| Edit | Reads the text, then opens the editor panel | `skill_text` | [SkillStudioTab.tsx:758-764](../../app/src/SkillStudioTab.tsx#L758-L764), [SkillStudioTab.tsx:268-274](../../app/src/SkillStudioTab.tsx#L268-L274) |
| Rename | Opens the Rename panel | — | [SkillStudioTab.tsx:765-771](../../app/src/SkillStudioTab.tsx#L765-L771) |
| Add to `<persona>` | Persona scope only. Adds the skill to the persona's list. | `persona_skill(…, "add")` | [SkillStudioTab.tsx:786-792](../../app/src/SkillStudioTab.tsx#L786-L792) |
| Remove | Persona scope only. Takes the skill out of the persona's list. | `persona_skill(…, "remove")` | [SkillStudioTab.tsx:779-785](../../app/src/SkillStudioTab.tsx#L779-L785) |
| Move to… picker | The other levels. It works the same as dragging a row onto a chip. | `skill_move_needs_grant`, `skill_move` | [SkillStudioTab.tsx:807-816](../../app/src/SkillStudioTab.tsx#L807-L816) |
| Delete | Reads who uses the skill, then opens the Delete panel | `skill_users` | [SkillStudioTab.tsx:797-804](../../app/src/SkillStudioTab.tsx#L797-L804), [SkillStudioTab.tsx:275-281](../../app/src/SkillStudioTab.tsx#L275-L281) |
| Drag the row | See §4.4 | `skill_move_needs_grant`, `skill_move` | [SkillStudioTab.tsx:609-621](../../app/src/SkillStudioTab.tsx#L609-L621) |

**Pictures:**

- Operator [4](skills-ui-inventory/operator-2026-10-28-4.png): built-in rows. The "In a chat" text "On It ships with purlis, and cannot be turned off." and the "Works on" badges wrap into rows about eight lines tall.
- Operator [3](skills-ui-inventory/operator-2026-10-28-3.png): the header row under the starts.

### 4.6 Change panels

Every panel opens inline under the toolbar. Only one panel is open at a time ([SkillStudioTab.tsx:840-957](../../app/src/SkillStudioTab.tsx#L840-L957)). None of them is a page or a tab of its own.

| Panel | Opened by | Controls | Command(s) | Code |
|---|---|---|---|---|
| New skill | Toolbar New skill, start New skill, Skills view › New skill | **Name** (help "Lowercase letters, digits and hyphens.", placeholder `pr-review`); **What it does and when to use it**; **Starts from** template picker (purlis's templates and extensions' templates); "It lands in the `<level>` skills." with a **Lands in** picker; **Make it** (disabled until the name and description are filled); **Cancel** | `skill_templates`, `skill_new` | [SkillStudioTab.tsx:991-1076](../../app/src/SkillStudioTab.tsx#L991-L1076) |
| Edit (light editor) | Row Edit | A SKILL.md text editor; a lint line ("It is a whole skill." or findings, with warnings prefixed "Worth knowing:"); **Save** (disabled when unchanged or when a finding blocks); **Close** | `skill_lint` (200 ms after typing stops), `skill_save` | [SkillEditor.tsx:20-85](../../app/src/SkillEditor.tsx#L20-L85), [SkillStudioTab.tsx:877-900](../../app/src/SkillStudioTab.tsx#L877-L900) |
| Rename | Row Rename, clash's Rename the project's copy | **New name**; "Every persona and skill that names it is changed with it."; **Rename**; **Cancel** | `skill_rename` | [SkillStudioTab.tsx:1079-1117](../../app/src/SkillStudioTab.tsx#L1079-L1117) |
| Move grant | A drag or Move to… into the project's level | "Move `<name>` to the `<level>` skills. A project skill reaches only the chats its grant names. Where does it reach?"; for a persona's skill, "`<persona>` keeps it in every workspace…"; a **Where `<name>` will reach** picker; **Move**; **Cancel** | `skill_move` | [SkillStudioTab.tsx:1121-1173](../../app/src/SkillStudioTab.tsx#L1121-L1173) |
| Delete | Row Delete | "Delete `<name>`? It can be restored afterwards."; who uses it ("Used by chats at the project's root, chats in ws…", "Named by N personas: …", "Named by N skills: …", or "No chat gets it now."); **Delete**; **Cancel** | `skill_delete`, then `skills_deleted` for the Restore offer | [SkillStudioTab.tsx:931-982](../../app/src/SkillStudioTab.tsx#L931-L982) |

While the editor's chunk loads, the panel shows "Opening the editor…".

### 4.7 Draft skills

- **Shows:** drawn only when there are drafts. The section "Draft skills" opens with "A chat's skill is kept here, on this machine, until you accept it. Accept is one commit." Each draft is a line: "`<name>` · `<level>` · a new skill | a change to it · N files · from chat N". A stale draft adds "The project's copy changed since the chat drafted it, so it cannot be accepted: reject it, or ask the chat to draft it again." ([SkillDrafts.tsx:34-174](../../app/src/SkillDrafts.tsx#L34-L174)).
- **Actions:**

| Action | What it does | Command | Code |
|---|---|---|---|
| Changes | Shows or hides a merge view of each file | — | [SkillDrafts.tsx:96-103](../../app/src/SkillDrafts.tsx#L96-L103) |
| Try `<name>` on (picker) | "Where it was drafted" or one of the project's profiles | `start_options` | [SkillDrafts.tsx:104-110](../../app/src/SkillDrafts.tsx#L104-L110) |
| Try it | Opens a chat with the draft, its prompt typed in and not sent | `skill_draft_try` | [SkillDrafts.tsx:111-125](../../app/src/SkillDrafts.tsx#L111-L125) |
| Accept | Commits the draft; disabled when stale | `skill_draft_accept` | [SkillDrafts.tsx:126-134](../../app/src/SkillDrafts.tsx#L126-L134) |
| Reject | Deletes the draft | `skill_draft_reject` | [SkillDrafts.tsx:135-143](../../app/src/SkillDrafts.tsx#L135-L143) |

### 4.8 Harness skills

- **Shows:** one section per program the core knows, such as Claude Code, Codex and opencode ([SkillStudioHarnesses.tsx:42-182](../../app/src/SkillStudioHarnesses.tsx#L42-L182)). Each section has:
  - a heading with the program's name;
  - the sentence "The skills `<Program>` finds itself. purlis lists them and never changes them; a switch here reaches the chats started after it. A plugin's skills come with the plugin, which `<Program>` turns on or off itself.";
  - a **plain `Table`** with five columns: **Skill · From · In a chat · What it does · Works on**. "From" reads "You", "Repo `<name>`" or "Plugin `<name>`";
  - when the program has no skills of its own, "`<Program>` has no skills of its own here." instead of the table;
  - when a repo leaves skills out, a second table "What a repo leaves out of `<Program>` chats" with the columns Skill · Level · Why.
  After the programs, each unsupported program gets a heading and one line.
- **Actions:**
  - **Turn on / Turn off** shows only for a skill whose switch is `alone`. It calls `skill_studio_switch` ([SkillStudioHarnesses.tsx:57-75](../../app/src/SkillStudioHarnesses.tsx#L57-L75)). A skill with switch `not` shows its reason instead, for example a plugin skill: "it comes with the plugin …, which Claude Code turns on or off itself, so its skills are shown read-only".
  - **Dismiss** clears the "Nothing was changed" trouble notice.
- **Pictures:** operator [5](skills-ui-inventory/operator-2026-10-28-5.png) shows the Claude Code section with two plugin skills. Each "In a chat" cell wraps to about nine lines.

### 4.9 Skill sources and install

- **Code:** [SkillSources.tsx:53-363](../../app/src/SkillSources.tsx#L53-L363).
- **Reads:** `skill_sources`, `skill_sources_offered` and `skill_updates`.
- **Shows:**
  - The heading "Skill sources", which Browse sources focuses, then the sentence "A skill source is a git repository of skills. An install copies a skill into this project, pinned to the commit you reviewed; a newer commit is offered here, and changes nothing until you accept it."
  - **Updates:** a line per offer, "`<name>` · `<source>`: a newer commit, `<short>` (edited here since it was installed)".
  - **Unconfirmed sources:** "`<source>`, where … came from, is not checked on this machine: purlis asks it nothing until you confirm it."
  - **Unasked lines.**
  - **Error line** when the sources could not be read.
  - **Empty line:** "This project has no skill sources yet. Add source takes a repository's https:// URL, or a folder on this machine."
  - **"Sources to start from":** the offered sources, with a name, "· from purlis" for catalogue sources, and why.
  - **The project's sources:** label, "· on this machine", "· signs in with `<vault>`".
  - **"Add source"**, then a pending line ("Reading …", "Reviewing …") and the "The skill source could not be read" notice.

**Actions:**

| Action | What it does | Command | Code |
|---|---|---|---|
| Review update | Opens the update review panel | `skill_update_review` | [SkillSources.tsx:159-175](../../app/src/SkillSources.tsx#L159-L175) |
| Confirm (unconfirmed source) | Confirms the remote | `skill_source_confirm` | [SkillSources.tsx:176-197](../../app/src/SkillSources.tsx#L176-L197) |
| Add… (offered source) | Opens the Add source form filled from the offer, with "Add `<name>` to this project's skill sources? … Nothing is installed until you review a skill from it." | — | [SkillSources.tsx:214-237](../../app/src/SkillSources.tsx#L214-L237) |
| Browse (a source) | Opens the browse panel: "`<source>` at `<commit>`", each skill with **Review install**, the non-skill folders with why, and **Close** | `skill_source_browse` | [SkillSources.tsx:247-254](../../app/src/SkillSources.tsx#L247-L254), [SkillSources.tsx:300-334](../../app/src/SkillSources.tsx#L300-L334) |
| Remove (a source) | Removes the source | `skill_source_remove` | [SkillSources.tsx:255-266](../../app/src/SkillSources.tsx#L255-L266) |
| Add source | Opens the empty Add source form | — | [SkillSources.tsx:270-274](../../app/src/SkillSources.tsx#L270-L274) |
| Add source form | Fields: **Repository** (https:// or a folder path), **Branch**, **Folder**, **Signs in with vault**, **Entry**; a **Who browses it** picker ("The whole project" / "Only this machine"); **Add it**; **Cancel** | `skill_source_add` | [SkillSources.tsx:367-457](../../app/src/SkillSources.tsx#L367-L457) |
| Review install | Opens the install review | `skill_install_review` | [SkillSources.tsx:316-323](../../app/src/SkillSources.tsx#L316-L323) |
| Install review panel | Shows "N skills at `<commit>`, N risky items marked. …"; per skill, the risk summary, lint, and a **Show `<file>`** toggle per file, plus what is not installed; a **Lands in** picker; **Install**; **Cancel** | `skill_install` | [SkillSources.tsx:509-614](../../app/src/SkillSources.tsx#L509-L614) |
| Update review panel | Shows "`<name>`: `<pinned>` to `<newer>`."; New in the summary; Gone from the summary; the source's change as `<details>` per file; your edits since the install; the reviewed skill and the skills it brings; **Accept update**; **Not now** | `skill_update` | [SkillSources.tsx:634-683](../../app/src/SkillSources.tsx#L634-L683) |
| Dismiss (source notice) | Clears the notice | — | [SkillSources.tsx:280-289](../../app/src/SkillSources.tsx#L280-L289) |

**Pictures:** operator [6](skills-ui-inventory/operator-2026-10-28-6.png) shows the sources sentence, the empty line, three offered sources (Anthropic's skills, OpenAI's curated skills, Superpowers), each with **Add…**, the full-width **Add source** button, and Deleted skills.

### 4.10 Deleted skills

- **Shows:** drawn when `skills_deleted` returns any. The section "Deleted skills" lists one line per skill: "`<name>` · `<label>` · `<stays>`", for example "browser · project · kept in the project's history" ([SkillStudioTab.tsx:462-481](../../app/src/SkillStudioTab.tsx#L462-L481)).
- **Actions:**
  - **Restore** calls `skill_restore`.
- **Pictures:** operator [6](skills-ui-inventory/operator-2026-10-28-6.png) shows six deleted project skills (browser, handoff, persona, secrets, update, working-in-a-clone).

## 5. A skill's own tab

- **Reached from:**
  - a skill's name in the studio table (§4.5);
  - a Skills view row (§3);
  - the skill run chip on a chat tab (§7.3), which opens the tab on that run.
- **Tab title:** the skill's name. The page has no heading with the name.
- **Code:** [SkillTab.tsx:155-246](../../app/src/SkillTab.tsx#L155-L246), mounted at [Views.tsx:687-700](../../app/src/Views.tsx#L687-L700).
- **Reads:**
  - `skill_details` ([SkillDetails.tsx:102-114](../../app/src/SkillDetails.tsx#L102-L114));
  - `skill_stats` ([SkillTab.tsx:66-135](../../app/src/SkillTab.tsx#L66-L135));
  - `skill_panels(plane, name, level)` ([SkillTab.tsx:187-199](../../app/src/SkillTab.tsx#L187-L199)).

**What it shows, in order** (all in one scrolling page):

1. A change notice.
2. A **Copy** picker, when the skill has more than one copy.
3. **State:** "In use." / "Deprecated: …" / "Archived: … never loads until it is restored.". It can add the unread line and "No chat has run `<name>` for N days. Deprecate it, so chats are pointed to something else?". Then the **Owners** sub-heading.
4. **Grade:** see the grade states below.
5. **History:** a table with the columns Saved · By · Change · Runs on it · Grade · (Actions). It is empty when the skill has no history: "`<name>` has no history yet". A line about runs on uncommitted changes can follow.
6. **Uses and used by.**
7. **Pinned by.**
8. **Totals:** three tables, By what it ran on / By persona / By workspace, with the columns Ran on|Persona|Workspace · Runs · Tool calls · Errors · Refused · Asks · Tokens · Time. Drawn only when there are runs.
9. The panels the core answers:
   - **Grades:** a note, then a table with the columns Graded · Version · Ran on · Tests passed · With the skill · Without it · Difference · Chats · Ended, and a Scorecard column the window adds. Empty: "No grades of `<name>` yet" ([crates/purlis-core/src/skills/studio.rs:418-484](../../crates/purlis-core/src/skills/studio.rs#L418-L484)).
   - **Runs:** a note, then a table with the columns Started · Persona · Ran on · Started by · Ended · Tool calls · Errors · Refused · Asks · Tokens · Time · Nested. Empty: "No runs of `<name>` yet", with a New chat offer ([crates/purlis-core/src/skills/studio.rs:519-590](../../crates/purlis-core/src/skills/studio.rs#L519-L590)).
   - **Each extension's panel**, headed "`<title>` · `<extension>`".

**Grade states:**

- **Idle:** the sentence "A grade runs `<name>`'s trigger tests in chats with it and without it, and shows what it costs before anything runs." and **Grade…**.
- **Planning:** "Reading what a grade runs…".
- **Plan shown:**
  - the cost paragraph, the trigger-test list and any left-out tests;
  - the token cap field and **Set the cap**;
  - the profile picker, the model picker, **Grade** and **Cancel**.
  - With no tests: "No trigger tests for `<name>` yet".
- **Running:** "Grading `<name>`: N of M chats done, X of Y tokens." and **Stop**.
- **Ended:** a notice labelled "Graded", "Stopped" or "Not graded".
- **Error:** "purlis could not plan a grade of `<name>`".

Code: [SkillGrade.tsx:111-466](../../app/src/SkillGrade.tsx#L111-L466).

**Actions:**

| Action | What it does | Command | Code |
|---|---|---|---|
| Copy picker (one button per copy) | Shows that copy's state, grade and history. The Grades panel follows it. | `skill_panels` | [SkillDetails.tsx:166-185](../../app/src/SkillDetails.tsx#L166-L185) |
| Use instead (optional) + Deprecate | Deprecates the skill, optionally naming a replacement | `skill_lifecycle` | [SkillDetails.tsx:275-287](../../app/src/SkillDetails.tsx#L275-L287) |
| Archive | Archives the skill | `skill_lifecycle` | [SkillDetails.tsx:288-295](../../app/src/SkillDetails.tsx#L288-L295) |
| Restore / Undo deprecation | Makes it active again | `skill_lifecycle` | [SkillDetails.tsx:297-301](../../app/src/SkillDetails.tsx#L297-L301) |
| Owners + Save owners | Saves the comma-separated owners | `skill_owners` | [SkillDetails.tsx:302-326](../../app/src/SkillDetails.tsx#L302-L326) |
| Grade… | Opens the grade plan | `skill_grade_plan`, `start_options` | [SkillGrade.tsx:188-203](../../app/src/SkillGrade.tsx#L188-L203), [SkillGrade.tsx:259-268](../../app/src/SkillGrade.tsx#L259-L268) |
| Token cap + Set the cap | Sets this machine's token cap | `skill_grade_set_cap` | [SkillGrade.tsx:413-430](../../app/src/SkillGrade.tsx#L413-L430) |
| Grade `<name>` on (profile picker) | Reads the plan again for that profile | `skill_grade_plan` | [SkillGrade.tsx:434-440](../../app/src/SkillGrade.tsx#L434-L440) |
| Model picker | Reads the plan again for that model | `skill_grade_plan` | [SkillGrade.tsx:441-449](../../app/src/SkillGrade.tsx#L441-L449) |
| Grade | Starts the grade with the plan's consent | `skill_grade_start` | [SkillGrade.tsx:450-457](../../app/src/SkillGrade.tsx#L450-L457), [SkillGrade.tsx:205-225](../../app/src/SkillGrade.tsx#L205-L225) |
| Cancel (plan) | Closes the plan | — | [SkillGrade.tsx:460-462](../../app/src/SkillGrade.tsx#L460-L462) |
| Stop | Stops the running grade | `skill_grade_stop` | [SkillGrade.tsx:251-257](../../app/src/SkillGrade.tsx#L251-L257) |
| Show changes / Hide changes (version row) | Shows the version's diff | `skill_diff` | [SkillDetails.tsx:453-455](../../app/src/SkillDetails.tsx#L453-L455) |
| Revert to this (not on the newest row) | Opens an inline confirm row, which first reads whether the revert widens anything | `skill_revert_widens` | [SkillDetails.tsx:456-460](../../app/src/SkillDetails.tsx#L456-L460) |
| Revert / Cancel (confirm row) | Reverts the skill as a new version | `skill_revert` | [SkillDetails.tsx:463-496](../../app/src/SkillDetails.tsx#L463-L496) |
| Unpin (Pinned by) | Unpins the skill for that workspace or persona | `skill_pin(…, false)` | [SkillPins.tsx:100-154](../../app/src/SkillPins.tsx#L100-L154), [SkillDetails.tsx:226-237](../../app/src/SkillDetails.tsx#L226-L237) |
| Remove it (Pinned by, unused pin) | Removes the bad pin | `skill_pin(…, false)` | [SkillPins.tsx:137-146](../../app/src/SkillPins.tsx#L137-L146) |
| Open scorecard (Grades row) | Opens the Grade scorecard tab (§6) | `skill_grade_scorecard` | [SkillTab.tsx:232-240](../../app/src/SkillTab.tsx#L232-L240) |
| New chat (empty Runs panel offer) | Runs the catalogue's `chat.new` | — | [crates/purlis-core/src/skills/studio.rs:584-586](../../crates/purlis-core/src/skills/studio.rs#L584-L586) |
| Dismiss (change notice, grade notice) | Clears the notice | — | [SkillDetails.tsx:156-165](../../app/src/SkillDetails.tsx#L156-L165), [SkillGrade.tsx:230-241](../../app/src/SkillGrade.tsx#L230-L241) |

**States:**

- **History:** "Reading the skill's history…", or the error "purlis could not read the history of `<name>`".
- **Totals:** the error "purlis could not total the runs of `<name>`".
- **Panels:** "Reading the skill's panels…", or the error "purlis could not read the panels of `<name>`". A single panel that is refused shows "`<Panel>` could not be shown for `<name>`".

**Pictures:** stories [pinned-by-two](../../app/src/__screenshots__/SkillPins.stories.tsx/skillpins-pinned-by-two_purlis_dark-chromium-linux.png), [pinned-by-none](../../app/src/__screenshots__/SkillPins.stories.tsx/skillpins-pinned-by-none_purlis_dark-chromium-linux.png) and [pins-not-used](../../app/src/__screenshots__/SkillPins.stories.tsx/skillpins-pins-not-used_purlis_dark-chromium-linux.png). These draw the Pin switch together with the Pinned by section. There are no story screenshots of the skill tab, the grade or the history.

## 6. Grade scorecard tab and grade chat tab

**Grade scorecard tab:**

- **Reached from:** Open scorecard on a Grades panel row (§5).
- **Tab title:** "Grade · `<skill>`".
- **Shows:**
  - "Graded `<when>`, version `<v>`, on `<ran on>`. `<ended>`, the skill made a difference of ±N points.";
  - when chats asked something, a line explaining that they were ended and not counted;
  - per trigger test, its query and one line per arm (With the skill / Without it / Its previous version), "N of M chats started it (should start it | should leave it alone), passed | failed | not run".
- **Actions:**
  - **Open run N**, one button per kept chat, opens the Grade chat tab. It is disabled with the tooltip "Not kept on this machine: …" when the chat is not kept.
- **States:**
  - **Loading:** "Reading the grade's scorecard…".
  - **Error:** "purlis could not read this grade of `<skill>`".
- **Command:** `skill_grade_scorecard`.
- **Code:** [GradeScorecard.tsx:90-184](../../app/src/GradeScorecard.tsx#L90-L184).

**Grade chat tab:**

- **Tab title:** "Grade chat N · `<skill>`".
- **Shows:** "What this chat of `<skill>`'s grade showed when the grade ended it. It takes no input." and a read-only terminal, 120×40.
- **Actions:** none.
- **States:**
  - **Loading:** "Reading the grade's chat…".
  - **Error:** "This grade's chat is not kept here".
- **Command:** `skill_grade_chat`.
- **Code:** [GradeScorecard.tsx:191-263](../../app/src/GradeScorecard.tsx#L191-L263).

## 7. Skills inside a chat

### 7.1 Skills chip on the pane

- **Reached from:** the chat pane's chip line. It is drawn only when the pane has room and no breadcrumb is shown (`!crumbs && roomy`, [LayoutPanes.tsx:233-238](../../app/src/LayoutPanes.tsx#L233-L238)).
- **Shows:** "N skills", where N counts listed skills plus manual-only skills, with a Sparkles icon. When the budget warns, the chip is a soft warning button with a warning icon. Its tooltip and accessible name read "`<warning>` N of M skills · X of Y characters[ · K manual-only skills free][ · H skills in the index skill]. Use one of this chat's skills." ([UseSkill.tsx:464-492](../../app/src/UseSkill.tsx#L464-L492), [UseSkill.tsx:42-102](../../app/src/UseSkill.tsx#L42-L102)).
- **Actions:**
  - **Press** opens Use skill… for this chat.
- **States:** the chip is hidden when the flag is off or no budget has been read yet. The budget comes from `chat_skills`, read once per open chat ([UseSkill.tsx:419-452](../../app/src/UseSkill.tsx#L419-L452)).
- **Pictures:** operator [7](skills-ui-inventory/operator-2026-10-28-7.png) shows "11 skills" beside "Claude Code" on the pane of chat "steward 5".

### 7.2 Use skill… dialog

- **Reached from:**
  - the skills chip;
  - the palette or tab-menu row "Use skill…";
  - the Skills view's row menu "Use in this chat", which opens the dialog on one skill.
- **Code:** [UseSkill.tsx:146-384](../../app/src/UseSkill.tsx#L146-L384), mounted at [PlaneView.tsx:9520-9543](../../app/src/PlaneView.tsx#L9520-L9543). Sending is at [PlaneView.tsx:6203-6226](../../app/src/PlaneView.tsx#L6203-L6226).

**Shows:**

- **Title:** "Use skill", or "Use `<name>`" once a skill is picked.
- **Subtitle:** "In `<chat>`. It is typed for you to send."
- **Lines above the list:**
  - the budget line;
  - the routing line, with a warning when the router fell back;
  - the "pins together" warning, with one **Open `<Skill studio · …>`** button per pinner;
  - the budget-warning sentence.
- **The list:** a filter once the list is long enough, then a list box. Each row shows the name; a line with the level, "· manual only" and "· in the index skill"; and the description.
- **Picked skill that takes arguments:** its description and one text box per argument.
- **Errors:** a trouble line.

**Actions:**

| Action | What it does | Command | Code |
|---|---|---|---|
| Filter | Matches on name and description | — | [UseSkill.tsx:291-299](../../app/src/UseSkill.tsx#L291-L299) |
| Pick a row | A skill that takes no arguments is typed into the chat at once. One that takes arguments shows the argument boxes. | `use_skill` | [UseSkill.tsx:204-212](../../app/src/UseSkill.tsx#L204-L212), [UseSkill.tsx:314-345](../../app/src/UseSkill.tsx#L314-L345) |
| Argument boxes | Typed values | — | [UseSkill.tsx:352-373](../../app/src/UseSkill.tsx#L352-L373) |
| Type it / Typing… | Types the skill with its arguments into the chat, unsent | `use_skill` | [UseSkill.tsx:231-236](../../app/src/UseSkill.tsx#L231-L236) |
| Back | Returns to the list | — | [UseSkill.tsx:230](../../app/src/UseSkill.tsx#L230) |
| Open `<Skill studio · …>` | Closes the dialog and opens the studio on that pinner's scope | (opens a tab) | [UseSkill.tsx:264-279](../../app/src/UseSkill.tsx#L264-L279), [PlaneView.tsx:9537-9540](../../app/src/PlaneView.tsx#L9537-L9540) |
| Close (dialog) | Cancels | — | [UseSkill.tsx:218](../../app/src/UseSkill.tsx#L218) |

**States:**

- **Loading:** "Reading this chat's skills…".
- **No skills:** "This chat has no skills. Open Skill studio from the palette to see why any skill is left out."
- **Unknown tool:** "This chat runs no tool purlis knows, so it has no skills to use here." or the not-supported sentence.
- **Filter matches nothing:** the shared no-match sentence.
- **Refused:** the trouble line stays in the dialog.
- **Afterwards:** a report says what was typed (`handedSaid`).

### 7.3 Skill run chip on the tab

- **Reached from:** the chat tab on the strip, while a skill run is open in that chat ([ChatStrip.tsx:443-448](../../app/src/ChatStrip.tsx#L443-L448)).
- **Shows:** a pill with a Sparkles icon, the skill's name and its tool-call count. The tooltip reads "Using the skill `<name>`: N tool calls so far. Open its runs." ([SkillRunChip.tsx:20-53](../../app/src/SkillRunChip.tsx#L20-L53)).
- **Actions:**
  - **Press** opens the skill's own tab, with the run marked in Runs.
- **States:** the chip is hidden when the flag is off or no run is open.

### 7.4 Chat notices

All three are pane notices, mounted in [ChatNotices.tsx:388-389](../../app/src/ChatNotices.tsx#L388-L389), [ChatNotices.tsx:439-440](../../app/src/ChatNotices.tsx#L439-L440) and [ChatNotices.tsx:478-480](../../app/src/ChatNotices.tsx#L478-L480).

| Notice | When | Text | Actions → what they do | Command | Code |
|---|---|---|---|---|---|
| Skills (trouble) | The chat started without skills its persona names, broken skills, skills a repo left out, or beside a repo's copy of a built-in | "This chat started without N skill(s) `<persona>` names, not installed on this machine: …" (and the matching sentences for the other cases). It carries the persona's mark. | **Open in Skill studio** opens the studio as the persona, or at the root when there is none, through the catalogue row; **Dismiss** puts it away for that chat in the core | `chat_skills_missing`, `put_away_chat_skills_missing` | [SkillsMissingNotice.tsx:24-98](../../app/src/SkillsMissingNotice.tsx#L24-L98) |
| Draft ready in Skill studio | This chat wrote drafts | "This chat drafted N skill(s): …. Try, accept or reject each in the Skill studio." | **Open in Skill studio** opens the studio at the **root**; **Dismiss** hides it until the drafts change | `skill_drafts` | [DraftReadyNotice.tsx:20-44](../../app/src/DraftReadyNotice.tsx#L20-L44) |
| Skills changed | The project's skills changed since this chat started | "Its skills changed since it started. It keeps the skills it started with until it restarts on the same conversation." | **Restart** restarts the chat on the same conversation; **Dismiss** hides it until the next change | (reads the plane's change feed) | [SkillsChangedNotice.tsx:25-61](../../app/src/SkillsChangedNotice.tsx#L25-L61) |

**Pictures:** operator [7](skills-ui-inventory/operator-2026-10-28-7.png) shows the Skills notice above chat "steward 5": "This chat started without 1 skill steward names, not installed on this machine: mattpocock-skills:grilling.", with **Open in Skill studio** and **Dismiss**.

## 8. Workspace settings: Skills summary row

- **Reached from:** the Settings tab at a workspace's level, in the Skills group. The group is drawn only while the flag is on ([settings/workspace.tsx:212-213](../../app/src/settings/workspace.tsx#L212-L213)).
- **Shows:**
  - the group "Skills", with help text "The skills this workspace's chats get. They are chosen in the Skill studio.";
  - the row "In its chats", with help text "Counted from the project's, the workspace's and the local skills, as a chat starting here gets them." Its value reads "No skills reach this workspace's chats yet." / "Its chats get N skills." / "Its chats get N skills, and M more are left out, each with why in the studio." While counting it reads "Counting…". On error it shows the core's sentence.
- **Actions:**
  - **Open in Skill studio** (a group link) opens the studio on that workspace through the `skills.studio:<ws>` catalogue row.
- **Command:** `skill_studio(plane, workspace, null)`.
- **Code:** [settings/skillsSummary.tsx:18-73](../../app/src/settings/skillsSummary.tsx#L18-L73).

## 9. Persona page: Skills summary row

- **Reached from:** a persona's view tab. The core answers it in panel blocks, behind the flag ([app/src-tauri/src/panels.rs:1414-1418](../../app/src-tauri/src/panels.rs#L1414-L1418)).
- **Shows:** a one-row list, "Brings N skills into every workspace", with the note "Open in Skill studio".
- **Actions:**
  - **The row** runs `skills.studio:persona/<name>`, which opens the studio as that persona.
- **Code:** [app/src-tauri/src/personaskills.rs:15-37](../../app/src-tauri/src/personaskills.rs#L15-L37).

## 10. Pictures

### Operator screenshots, 2026-10-28

The window shows project `purlis-plane` at the plane root ("Plane root · ws 2"). The sidebar is on the Skills view and the bottom panel is Repos ("No workspace selected, so there are no repos to list.").

| File | What it shows |
|---|---|
| [operator-2026-10-28-3.png](skills-ui-inventory/operator-2026-10-28-3.png) | The Skill studio tab at the top of the page. **Toolbar:** "A chat at the project's root" picker, **New skill**, **Ask a chat to draft a skill**. **Skills to move notice** ("This project keeps 1 skill…") with **Move them** and **Dismiss**. The empty state "This project has no skills of its own yet" with its three starts. The level chips: project · local project · workspace git-ci-cd · local workspace git-ci-cd · workspace smart-ide · local workspace smart-ide. The table header Skill · Level · Granted to · In a chat · What it does · Works on · Change. **Sidebar:** Skills view with **Open Skill studio** (focused) and **New skill…**, and its empty state ("…from New skill…"). |
| [operator-2026-10-28-4.png](skills-ui-inventory/operator-2026-10-28-4.png) | The studio scrolled to the table. Built-in rows add-curation-action, browser and compact. Each "In a chat" cell reads "On It ships with purlis, and cannot be turned off." over five lines. Each "Works on" cell lists "▶ Claude Code: full / ▶ Codex: full / ▶ opencode: full" over six to seven lines. The Change column is empty for these rows. |
| [operator-2026-10-28-5.png](skills-ui-inventory/operator-2026-10-28-5.png) | The studio scrolled to the **Claude Code** harness section: its sentence, then the table Skill · From · In a chat · What it does · Works on. It holds frontend-design:frontend-design and skill-creator:skill-creator, both "From: Plugin …". Each "In a chat" cell reads "On it comes with the plugin …, which Claude Code turns on or off itself, so its skills are shown read-only." over about nine lines. |
| [operator-2026-10-28-6.png](skills-ui-inventory/operator-2026-10-28-6.png) | The studio scrolled to **Skill sources**: its sentence and the empty line, then "Sources to start from" with Anthropic's skills, OpenAI's curated skills and Superpowers, each with **Add…**, then a full-width **Add source**. Then **Deleted skills**: browser, handoff, persona, secrets, update and working-in-a-clone, each "· project · kept in the project's history" with **Restore**. |
| [operator-2026-10-28-7.png](skills-ui-inventory/operator-2026-10-28-7.png) | Chat tab "steward 5" in front, beside the Skill studio tab. The pane's **Skills** trouble notice ("This chat started without 1 skill steward names, not installed on this machine: mattpocock-skills:grilling.") with **Open in Skill studio** and **Dismiss**. The pane chip line shows "11 skills" and "Claude Code". **Sidebar:** the Skills view now also shows the **Only this chat's** toggle (not pressed), and below it the same empty state, "This project has no skills of its own yet". |

### Committed story screenshots

Each story is committed in ten variants: purlis / purlis-classic, light / dark / dark-high-contrast, chromium / webkit. The links below go to `purlis_dark-chromium`.

| Story | Surface | Link |
|---|---|---|
| SkillsList · By level | Skills view: groups Needs you, Built-in, Project, Workspace, Claude Code; marks Needs review, Off, Hidden by the router, Lint; Only this chat's | [skillslist-by-level](../../app/src/__screenshots__/SkillsList.stories.tsx/skillslist-by-level_purlis_dark-chromium-linux.png) |
| SkillsList · No skills of its own | Skills view empty state, and the New skill… menu | [skillslist-no-skills-of-its-own](../../app/src/__screenshots__/SkillsList.stories.tsx/skillslist-no-skills-of-its-own_purlis_dark-chromium-linux.png) |
| SkillsList · A pin no chat reads | Skills view row menu with the "Pinned, not used" note | [skillslist-a-pin-no-chat-reads](../../app/src/__screenshots__/SkillsList.stories.tsx/skillslist-a-pin-no-chat-reads_purlis_dark-chromium-linux.png) |
| SkillPins · Pinned by two | Pin switch + Pinned by with Unpin | [skillpins-pinned-by-two](../../app/src/__screenshots__/SkillPins.stories.tsx/skillpins-pinned-by-two_purlis_dark-chromium-linux.png) |
| SkillPins · Pinned by none | "No workspace or persona pins …" | [skillpins-pinned-by-none](../../app/src/__screenshots__/SkillPins.stories.tsx/skillpins-pinned-by-none_purlis_dark-chromium-linux.png) |
| SkillPins · Pins not used | "Pinned, not used" with Remove it | [skillpins-pins-not-used](../../app/src/__screenshots__/SkillPins.stories.tsx/skillpins-pins-not-used_purlis_dark-chromium-linux.png) |

These surfaces have **no story and no committed screenshot**: SkillStudioTab, SkillTab, SkillDetails, SkillGrade, GradeScorecard, SkillSources, SkillDrafts, SkillStudioHarnesses, SkillEditor, UseSkill, SkillsChip, SkillRunChip, SkillsMissingNotice, DraftReadyNotice, SkillsChangedNotice, the workspace summary row and the persona row.

## 11. Every action, by surface

Each row below is one action on one surface. **Also on** names the other surfaces that hold the same action, or one with the same label. Where the same label does something different, the row says so. The **Section** column is the section of this file.

| # | Action (as labelled) | Surface | Section | Does | Command | Also on |
|---|---|---|---|---|---|---|
| 1 | Skill studio (switch) | Settings › You › Previews | 1 | Flag on/off | `set_skill_studio_on` | — |
| 2 | Open Skill studio | Palette | 2 | Studio at the root | — | 7, 83, 85, 87, 91, 92 (open-the-studio) |
| 3 | Open in Skill studio (workspace) | Palette | 2 | Studio on a workspace | — | 91 |
| 4 | Open in Skill studio (persona) | Palette | 2 | Studio as a persona | — | 85, 92 |
| 5 | Show the Skills view | Palette | 2 | Sidebar Skills view | — | activity bar mark |
| 6 | Use skill… | Palette, chat tab menu | 2 | Opens Use skill… | — | 15, 81, 82 |
| 7 | Open Skill studio | Skills view header | 3 | Studio on the title-bar workspace | — | 2 |
| 8 | New skill… › Ask a chat to draft a skill | Skills view header menu | 3 | Draft chat | `skill_draft_chat` | 23, 27 |
| 9 | New skill… › Browse sources | Skills view header menu | 3 | Studio, scrolled to sources | — | 28 |
| 10 | New skill… › New skill | Skills view header menu | 3 | Studio with the New skill form | — | 22, 29 |
| 11 | Only this chat's | Skills view | 3 | Narrow to the front chat | `chat_skills` | — |
| 12 | Filter | Skills view | 3 | Filter rows | — | 80 (Use skill filter) |
| 13 | Fold / unfold group | Skills view | 3 | Fold a level | — | — |
| 14 | Row Enter / click | Skills view | 3 | Skill tab | — | 33 |
| 15 | Use in this chat | Skills view row menu | 3 | Use skill… on one skill | `use_skill` | 6, 81, 82 |
| 16 | Turn on / off in `<ws>` | Skills view row menu | 3 | Workspace turn-off of a project skill | `skill_turn` | 39 (same); 56 (same label, harness switch, other command) |
| 17 | Pin to / Unpin from this workspace | Skills view row menu | 3 | Pin | `skill_pin` | 36, 75 |
| 18 | Show in the Explorer view | Skills view row menu | 3 | Explorer on the folder | — | — |
| 19 | Dismiss | Skills view notice | 3 | — | — | every notice |
| 20 | Whose chats (picker) | Studio toolbar | 4.1 | Another studio tab | — | — |
| 21 | Add a skill + Add | Studio toolbar (persona) | 4.1 | Add to the persona's list | `persona_skill` | 42 |
| 22 | New skill | Studio toolbar | 4.1 | New skill panel | — | 10, 29 |
| 23 | Ask a chat to draft a skill | Studio toolbar | 4.1 | Draft chat | `skill_draft_chat` | 8, 27 |
| 24 | Move them | Studio · Skills to move | 4.2 | Doctor fix `project-skills` | `plane_doctor_fix` | — |
| 25 | Dismiss | Studio · Skills to move | 4.2 | Hide for this tab | — | — |
| 26 | Restore | Studio · change notice | 4.3 | Restore the just-deleted copy | `skill_restore` | 66 (same); 70 (same label, lifecycle) |
| 27 | Ask a chat to draft a skill | Studio · starts | 4.2 | Draft chat | `skill_draft_chat` | 8, 23 |
| 28 | Browse sources | Studio · starts | 4.2 | Scroll to sources | — | 9 |
| 29 | New skill | Studio · starts | 4.2 | New skill panel | — | 10, 22 |
| 30 | Remove it (bad pin) | Studio · pins-refused line | 4.3 | Remove the bad pin | `skill_pin` | 76 |
| 31 | Drag row onto a level chip | Studio · level chips | 4.4 | Move | `skill_move_needs_grant`, `skill_move` | 45 |
| 32 | Dismiss | Studio · change notice | 4.3 | — | — | — |
| 33 | Open `<name>` | Studio · table · Skill | 4.5 | Skill tab | — | 14 |
| 34 | Share | Studio · table · Level | 4.5 | Commit a local copy | `skill_share` | — |
| 35 | Granted to (picker) | Studio · table · Granted to | 4.5 | Grant | `skill_grant` | 49 (grant in Move panel) |
| 36 | Pinned (switch) | Studio · table · In a chat | 4.5 | Pin | `skill_pin` | 17, 75 |
| 37 | Rename the project's copy | Studio · table · In a chat | 4.5 | Rename panel | — | 41 |
| 38 | Accept its folder | Studio · table · In a chat | 4.5 | Accept a level folder | `skill_level_accept` | — |
| 39 | Turn on / off in `<ws>` | Studio · table · In a chat | 4.5 | Workspace turn-off | `skill_turn` | 16 |
| 40 | Works on badge (expand) | Studio · table · Works on | 4.5 | Disclosure | — | harness table Works on |
| 41 | Edit | Studio · table · Change | 4.5 | Editor panel | `skill_text` | — |
| 42 | Rename | Studio · table · Change | 4.5 | Rename panel | — | 37 |
| 43 | Add to `<persona>` | Studio · table · Change | 4.5 | Persona list add | `persona_skill` | 21 |
| 44 | Remove | Studio · table · Change | 4.5 | Persona list remove | `persona_skill` | (label also on 62) |
| 45 | Move to… (picker) | Studio · table · Change | 4.5 | Move | `skill_move_needs_grant`, `skill_move` | 31 |
| 46 | Delete | Studio · table · Change | 4.5 | Delete panel | `skill_users` | — |
| 47 | New skill form: Name, Description, Starts from, Lands in, Make it, Cancel | Studio · change panel | 4.6 | Make a skill | `skill_templates`, `skill_new` | — |
| 48 | Editor: Save, Close | Studio · change panel | 4.6 | Save SKILL.md | `skill_lint`, `skill_save` | — |
| 49 | Rename panel: New name, Rename, Cancel | Studio · change panel | 4.6 | Rename | `skill_rename` | — |
| 50 | Move grant panel: Where it will reach, Move, Cancel | Studio · change panel | 4.6 | Move into project | `skill_move` | 35 |
| 51 | Delete panel: Delete, Cancel | Studio · change panel | 4.6 | Delete | `skill_delete` | — |
| 52 | Changes | Studio · Draft skills | 4.7 | Merge view | — | 84 (Show changes, skill tab) |
| 53 | Try `<name>` on (picker) + Try it | Studio · Draft skills | 4.7 | Try chat | `start_options`, `skill_draft_try` | — |
| 54 | Accept | Studio · Draft skills | 4.7 | Commit draft | `skill_draft_accept` | — |
| 55 | Reject | Studio · Draft skills | 4.7 | Delete draft | `skill_draft_reject` | — |
| 56 | Turn on / Turn off | Studio · harness section | 4.8 | Harness skill switch | `skill_studio_switch` | 16, 39 (same label, other command) |
| 57 | Dismiss | Studio · harness notice | 4.8 | — | — | — |
| 58 | Review update | Studio · sources | 4.9 | Update review panel | `skill_update_review` | — |
| 59 | Confirm | Studio · sources | 4.9 | Confirm remote | `skill_source_confirm` | — |
| 60 | Add… | Studio · sources (offered) | 4.9 | Prefilled Add source form | — | 63 |
| 61 | Browse | Studio · sources | 4.9 | Browse panel | `skill_source_browse` | — |
| 62 | Remove | Studio · sources | 4.9 | Remove source | `skill_source_remove` | (label also on 44) |
| 63 | Add source + form (Repository, Branch, Folder, Signs in with vault, Entry, Who browses it, Add it, Cancel) | Studio · sources | 4.9 | Add source | `skill_source_add` | 60 |
| 64 | Review install, Close | Studio · sources · browse panel | 4.9 | Install review | `skill_install_review` | — |
| 65 | Install review: Show `<file>`, Lands in, Install, Cancel | Studio · sources · install panel | 4.9 | Install | `skill_install` | — |
| 66 | Restore | Studio · Deleted skills | 4.10 | Restore deleted skill | `skill_restore` | 26 |
| 67 | Update review: Accept update, Not now | Studio · sources · update panel | 4.9 | Update | `skill_update` | — |
| 68 | Copy (picker) | Skill tab | 5 | Pick a copy | `skill_panels` | — |
| 69 | Use instead + Deprecate / Archive | Skill tab · State | 5 | Lifecycle | `skill_lifecycle` | — |
| 70 | Restore / Undo deprecation | Skill tab · State | 5 | Lifecycle back to active | `skill_lifecycle` | 26, 66 (same label "Restore", other meaning) |
| 71 | Owners + Save owners | Skill tab · State | 5 | Owners | `skill_owners` | — |
| 72 | Grade…; Token cap + Set the cap; profile picker; model picker; Grade; Cancel | Skill tab · Grade | 5 | Plan and start a grade | `skill_grade_plan`, `start_options`, `skill_grade_set_cap`, `skill_grade_start` | — |
| 73 | Stop | Skill tab · Grade (running) | 5 | Stop grade | `skill_grade_stop` | — |
| 74 | Show / Hide changes; Revert to this; Revert, Cancel | Skill tab · History | 5 | Diff and revert | `skill_diff`, `skill_revert_widens`, `skill_revert` | 52 |
| 75 | Unpin | Skill tab · Pinned by | 5 | Unpin | `skill_pin` | 17, 36 |
| 76 | Remove it | Skill tab · Pinned by | 5 | Remove bad pin | `skill_pin` | 30 |
| 77 | Open scorecard | Skill tab · Grades panel row | 5 | Scorecard tab | `skill_grade_scorecard` | — |
| 78 | New chat | Skill tab · empty Runs panel | 5 | `chat.new` | — | — |
| 79 | Open run N | Grade scorecard tab | 6 | Grade chat tab | `skill_grade_chat` | — |
| 80 | Filter | Use skill… | 7.2 | Filter | — | 12 |
| 81 | Pick a row / argument boxes / Type it / Back / Close | Use skill… | 7.2 | Type a skill into the chat | `use_skill` | 6, 15, 82 |
| 82 | Press (N skills) | Skills chip on the pane | 7.1 | Use skill… | `chat_skills` | 6, 15 |
| 83 | Open `<Skill studio · …>` | Use skill… (pins together) | 7.2 | Studio on a pinner | — | 2 (open-the-studio) |
| 84 | Press (run chip) | Skill run chip on the tab | 7.3 | Skill tab on that run | — | 14, 33 |
| 85 | Open in Skill studio | Chat notice · Skills | 7.4 | Studio as persona (or root) | `chat_skills_missing` | 2, 4 |
| 86 | Dismiss | Chat notice · Skills | 7.4 | Put away | `put_away_chat_skills_missing` | — |
| 87 | Open in Skill studio | Chat notice · Draft ready | 7.4 | Studio at the root | — | 2 |
| 88 | Dismiss | Chat notice · Draft ready | 7.4 | Hide | — | — |
| 89 | Restart | Chat notice · Skills changed | 7.4 | Restart the chat | — | — |
| 90 | Dismiss | Chat notice · Skills changed | 7.4 | Hide | — | — |
| 91 | Open in Skill studio | Settings › Workspace › Skills | 8 | Studio on the workspace | — | 3 |
| 92 | Brings N skills… (row) | Persona page | 9 | Studio as the persona | — | 4 |

### Counts

**Surfaces.** This file names 29 surfaces:

1. Settings preview row
2. Palette / tab menu
3. Skills view
4. Skills view row menu
5. Studio toolbar
6. Skills to move
7. Starts
8. Change notice
9. Pins-refused line
10. Router line
11. Level chips
12. Skills table
13. Change panels (five kinds)
14. Draft skills
15. Harness sections
16. Skill sources
17. Sources' four inline panels (Add, Browse, Install review, Update review), counted as one
18. Deleted skills
19. Skill tab
20. Skill tab's Grades and Runs panels
21. Grade scorecard tab
22. Grade chat tab
23. Skills chip
24. Use skill…
25. Run chip
26. Skills notice
27. Draft ready notice
28. Skills changed notice
29. Workspace summary row and persona row

**Actions.** The table above has **92 rows**. Counted one control per row, with each form field, picker and button taken alone, there are **about 130 controls**.

**Where actions concentrate.** Rows 20–67, that is **48 of the 92 rows**, are on the one Skill studio tab. They span ten sections of one scrolling page. Rows 68–78, that is **11 rows**, are on the skill's own tab.

**One action in several places:**

- **Open the studio** has 11 entry points under three labels: "Open Skill studio", "Open in Skill studio" and "Open `<Skill studio · x>`". They are rows 2, 3, 4, 7, 83, 85, 87, 91 and 92, plus the Skills view's Browse sources and New skill, which also open it.
  - At the root: the palette's Open Skill studio and the Draft ready notice.
  - On the title-bar workspace: the Skills view's button.
- **Ask a chat to draft a skill:** 3 places (rows 8, 23 and 27).
- **New skill:** 3 places (rows 10, 22 and 29).
- **Use a skill in a chat:** 4 ways in (rows 6, 15, 81 and 82).
- **Pin or unpin:** 3 places, as a switch, a menu row and a button (rows 17, 36 and 75).
- **Turn on/off:** 3 places (rows 16, 39 and 56). Rows 16 and 39 are one command; row 56 is another command on harness skills.
- **Restore:** 3 places with two meanings (rows 26, 66 and 70).
- **Remove:** 2 labels on unrelated actions (rows 44 and 62).
- **Move a skill:** 2 ways (rows 31 and 45).
- **Add to a persona:** 2 ways (rows 21 and 43).
- **Remove a bad pin:** 2 places (rows 30 and 76).
