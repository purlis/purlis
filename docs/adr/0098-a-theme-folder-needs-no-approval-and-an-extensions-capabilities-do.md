# A theme folder needs no approval; an extension's capabilities do

**Accepted 2026-10-16**, from the operator's spec #2313 (M64, Themes), ticket #2314. It amends
the extension approval model of [ADR 0041](0041-a-plugin-is-a-subprocess-or-charter-has-no-plugins.md)
(decision 3, how an extension is installed and trusted) and
[ADR 0053](0053-an-extension-is-granted-capabilities-one-at-a-time.md) (capabilities, one at a
time). Each records the change as an amendment of its own, in [`0041/`](0041/) and
[`0053/`](0053/). It builds on [ADR 0097](0097-a-theme-is-bounded-data-and-may-set-every-token.md):
a theme is bounded data.

## Where purlis was

A theme could come only from the person's own file or from an extension. An extension is
approved by a fingerprint over its whole folder, and that fingerprint re-asks whenever a byte in
the folder changes. So an extension's theme asked for approval again on every save, which made
authoring one impractical, and nothing let a team commit a theme to its project or a workspace.

## The decision

### 1. A theme is data, not a capability

Reading a theme grants nothing and runs nothing: the engine parses it against a closed, bounded
grammar (ADR 0097) and the window draws the result. Approval exists for what purlis does for a
program (ADR 0041, ADR 0053), and a theme folder holds no program. So:

- **The person's own theme folders, a project's and a workspace's need no approval.** A project's
  and a workspace's are read as the rest of the plane is (ADR 0035), with no prompt of their own.
- **Editing a theme folder asks nothing**, and the window repaints on save.

### 2. An extension's capabilities still ask

An extension keeps its fingerprint approval (ADR 0041, decision 3, and its amendment of
2026-09-22 on the folder) for its capabilities: commands, panels, views and every word ADR 0053
lists. **An extension that runs a program is approved as a whole**: its fingerprint covers its
whole folder, as ADR 0041 rules, and the themes in it are part of what is approved.

### 3. A theme-only extension is a set of theme folders

**An extension whose manifest asks for no capability, names no program and contributes only
themes and icon themes is treated as theme folders.** It is installed by path as any extension
is, and turned on or off in a project as today. It is not fingerprinted, its approval prompt is
not shown, and editing its themes asks nothing. A manifest that adds a capability or a program
stops being theme-only, and is approved as any other extension from then on.

### 4. Theme folders are not proposed changes

A theme folder's review is the theme draft and its Keep step: a chat's writes to a theme go to a
draft drawn only in its workspace, and the person keeps or reverts it. **A theme pick in a
manifest** (the project's `[theme]`, a workspace's `settings.theme`) **stays a proposed change,
as today** (the proposed change rulings, #1975): what a project or a workspace draws by default
is a setting, reviewed like one.

### 5. What does not move

- **Hard floors hold whatever the theme's source**: a theme draft that breaks one is never drawn.
- **The consent surfaces and the state colours** keep the floor ADR 0041 gave them.
- **No theme reaches a secret, a permission, a hook or a harness setting.** ADR 0041's table
  stands.

## What was rejected

- **Leaving a theme's fingerprint out of an extension that runs a program.** Its program reads
  its own folder, so the folder stays one approved whole.
- **An approval for project and workspace themes.** A prompt on data that grants nothing teaches
  the person to click through prompts, which ADR 0041's 2026-09-22 amendment calls worse than no
  dialog at all.
- **Making a theme folder a proposed change.** The draft is already a review the person sees
  live; a second review of the same bytes would be one too many.

## Consequences

- Making a theme is a tight loop: save, and the window repaints.
- The extension registry tells a theme-only extension from one with capabilities, and the
  Extensions list says which it is.
- `themes/` becomes a reserved folder at the plane root and in every workspace
  (`docs/plane-format.md`), and `purlis doctor` names what sits there that is not a theme folder.
