# The window's UI primitives

**Component families on React Aria, and Radix UI until each family moves. New UI is built from
them, not from hand-rolled markup.**

Since #2904 (ADR 0104), every control of a moved family is drawn from its **component family** in
`app/src/ui/<family>`, which wraps its React Aria primitive once; a screen never imports the
primitives library, and lint holds it. The button family is the first: [The button
family](#the-button-family). The families that have not moved still take their Radix primitive as
the rest of this file says.

This is a rule and not a preference, so the rest of this file is the reasons for it and the way
to follow it.

**Its sibling is [`design-system.md`](design-system.md), which says what the window is drawn
_in_: a theme is a data file, every colour is a semantic token, and a literal anywhere outside
`app/src/theme/` fails the build. Read that one before writing a rule with a colour in it, and
for what a copied-in shadcn/ui component has to satisfy.**

**Where the rule is decided.** purlis **ADR 0037**, _charter takes the behaviour and keeps the
look_, its amendment of 2026-09-22, which settled the conflict this file used to flag, and
**ADR 0104**, which puts the families on React Aria behind one wrapper each. That
record is authoritative; this file is its code-side expression, and where the two disagree this
file is the defect.

## The rule

- A dialog, a radio group, a checkbox, a menu, a popover, a tooltip, a collapsible section, a
  tab list: take the Radix primitive. Install the one package you need
  (`@radix-ui/react-<thing>`) — the primitives tree-shake per package, so the window pays for
  what it uses and nothing else.
- **No library between you and the primitive, and no indirection you cannot read.** No `<Modal>`,
  no `<Field>`, no house component library. The primitive is the component; purlis's look is CSS
  on it. A purlis API in front of Radix is the custom tooling this repo's first rule exists to
  prevent, and it is how a primitives migration turns back into hand-rolled markup with extra
  steps. The test is at the **call site**: can the next person see which primitive this is and
  reach its props?
- **A component's source copied into this repo is neither of those, and is allowed.** Until
  2026-09-22 the rule above read _"do not write a wrapper layer around them"_, which forbade a
  shadcn/ui component — a shadcn component is literally a thin wrapper around a Radix primitive.
  The operator settled it: that rule was written against **a dependency that owns your markup**,
  and a file in `app/src/components/ui/` is ours, editable line by line, and visible in the diff
  that adds it. `docs/design-system.md` has what a copy must satisfy.
- **One house set is allowed, and only this one: the settings set** (ADR 0037, amended
  2026-10-04, rulings V89f and V89j; SE-16 #1166, DS-3e #1177). `app/src/settings/components.tsx`
  holds six pieces — **SettingsLayout**, **SettingGroup**, **SettingRow**, **Field**, **Choice**
  and **SettingActions** — and nothing else may join it without a new amendment to that record. See
  [The settings set](#the-settings-set-is-the-one-house-set) below for why.
- purlis's own look, always. Radix ships **no CSS at all** — every primitive is an unstyled
  element with `data-state` attributes to hang rules off. A control takes its look from **its
  family's stylesheet in `app/src/ui/`** ([Every control is a family's](#every-control-is-a-familys)
  below), and `App.css` draws the rest of the window — but **no colour is written in either**: every one is
  `var(--<token>)` and the values live in `app/src/theme/`. See `design-system.md`.
- Native HTML that already does the job is not hand-rolled markup and does not need replacing:
  `<details>/<summary>`, `<label for>`, `<button>`. Reach for a primitive when the browser has
  no element for what you mean — a modal that traps focus, a listbox, a menu.
- `react-resizable-panels` stays what draws resizable panes. Radix has no panel primitive and
  there is nothing to move.

## The settings set is the one house set

The rule above says no `<Field>` and no house component library, and it was right for every
surface it was written against. Settings is where it stopped paying, for two reasons, and the
operator amended it there on 2026-10-04 (V89f; the spec on #558):

- **Two hand-built form styles already existed, and they had drifted.** Project settings,
  Saving, Memory and workspace repos draw a label, a control and a hint with `settings-field`
  and `settings-hint`; the dialogs (New project, New vault, Start chat, First run) draw a
  choice with `choices` and `choice`. The same row — a label, a control, a line of help — was
  written twice with different spacing, different focus rules and different names for the
  same thing, and every new screen picked one by copying it. A rule that forbids the
  component does not stop the drift; it moves it into CSS.
- **It was DS-3's _expand_ step** (#626, "One component set"). The set was added beside the old
  classes, each screen moved onto it in its own ticket (Saving, Memory, workspace repos, New
  project, New vault, Rename workspace, Start chat, First run, Updates), and DS-3e (#1177)
  moved the rest (New workspace, New branch, New persona, Link to work item, a vault's secret
  dialogs and Delete vault, the forge question, First task, the repo picker, an extension's
  on/off, the repo's instruction files, Land's squash box) and then **deleted** the old
  `settings-*`, `asks`, `choices`, `choice`, `who` and `picking` classes — an expand–contract
  change, never a big-bang one. `app/src/settings/oldFormClasses.test.ts` fails on any rule or
  `className` that names one again.

What keeps it from becoming the library this file forbids:

- **Six pieces, named here, and no more.** `SettingsLayout` (the level switcher, the nav of
  groups with its filter box above it, and the chosen group, in two columns), `SettingGroup`
  (a group's heading, its one help line ending on where its values are kept, and rows),
  `SettingRow` (label, control and reset on one line at the control height, help under them, and
  since SE-17 a refused write's reason and the last change's Undo; since SE-18 the value's
  origin, drawn quietly since DS-8: a badge only where the value is kept elsewhere than its
  group's, a mark on a value set at this level, and "Move to…" at the end of the line), `Field` (`text`, `list` and `range`), `Choice` (`radio`, since #2907 `cards`,
  `select`, `toggle`, since #1860 `switch`, since DS-3e `checks`: any of a few, each ticked on its own, and since #626
  `chips`: any of a few too, drawn small and side by side over the list they narrow, the Chats
  list's state chips) and `SettingActions` (a form's row of buttons, V89j). A seventh piece is
  an amendment to ADR 0037, not a commit.
- **`chips` is one Tab stop.** The arrow keys move between the chips (the checkbox family's
  roving `CheckboxGroup`, #2907) and Space ticks the one the keyboard is on: each chip is a native
  checkbox in its chip's `<label>`, so a screen reader hears a checkbox named by the chip. A list's
  filter costs the keyboard one stop however many chips it has. A chip says it is on by its tick
  and its ground, never by colour alone.
- **`cards`** (#2907) is `radio` drawn as option cards: one of a few compared before picking,
  each card as tall as the tallest, its mark and name on the first line and its line under them,
  and no radio circle (Start a chat's harnesses, R24).
- **A control with no row** (#626) is still the set's `Field` or `Choice`: a box in a table's
  cell, a pick inside a sentence, the chips over a list. Its caller hands it `RowIds` with no
  `labelledBy`, and with `label`, said as its `aria-label`, since no row's `<label>` names it;
  and `title`, drawn as its shared tooltip ([the tooltip family](#the-tooltip-family)), where a
  row's help line would have said something. A box or a
  select with no row is never wider than its cell or line (`min-w-0 max-w-full`), and a box in a
  table's cell takes its own `size` and `inputMode`.
- **SettingActions is a row, and the buttons are the caller's.** Native `<button>`s with their
  own `type`, `disabled` and `onClick` (and `tabIndex={0}`, for #190), drawn alike; a button
  whose act cannot be undone says so with `ends-it` on a page (NotCloned, a workspace's Repos).
  It ends a _form_ that makes or changes something.
- **`ends-it` means "cannot be undone from purlis", not only "destroys"** (D-1719-3). An act
  is marked when purlis has no Undo for it, whether it deletes (Delete, Discard), publishes
  (Push, Make live, Saving's Save all, whose save commits and goes as far as each repo's mode
  says), interrupts (Restart now, Close now) or throws a record away (Start fresh). An act
  purlis can take back, or that only opens or closes something, is not marked, even when it
  changes a lot. The mark is a warning about the answer, so it is never the button Return
  finds (`docs/design-system.md`, the dialog's footer). A dialog's answers are its footer's
  ([the dialog family](#the-dialog-family), #2910): a question's way out and its act or two, and a
  form's. A confirm whose only field is the typed name of what it ends is a question too, and its
  delete is the footer's main action, last (D-1210-8, Delete vault), so no dialog puts a `danger`
  act in `SettingActions`; `dialogAnswers.guard.test.ts` holds that. Its keyboard lands in the
  name box, not on the delete (D-1210-9).
- **A box the set's `Field` cannot hold goes in the row's control slot as a native element.**
  A `Field` is controlled; a secret's value is never held in React state (a vault's value box),
  so that box is the field family's `TextField` with `type="password"`, which holds its own words
  and says only whether it is filled, in a `SettingRow`'s control slot.
  The slot is how a row composes what the set does not draw, as `Browse…` beside a path is.
- **Each piece is a thin layer over a primitive already in the window, and says which.**
  The level switcher is the segmented control; the nav is buttons under Radix roving focus
  (`roving.ts`, as the explorer's rows); `Choice` is the radio group, the option cards, the
  select family's `Select` (`ui/select/Select.tsx`), the switch, a checkbox, or a group of
  checkboxes or chips, each from [the choice families](#the-choice-families); `Field` is the field
  family's box, or the slider.
  The props are the primitive's own words — `value`, `onValueChange`, `checked`,
  `onCheckedChange` — so the call site still answers "which primitive is this?". The one word
  of the set's own is `Field`'s `onCommit` (SE-17): a typed value is written when the field is
  left or, on one line, at Enter, and no single native event names that.
- **A switch is a setting; a checkbox is an option of a form** (#1860). `Choice kind="switch"`
  is an on/off setting that is in force the moment it is turned (the You level's Chats list
  settings, an extension on this machine). `kind="toggle"` is a box in a form whose button does
  the work (Start a chat's options, New project's Live), and `checks` are a few boxes ticked on
  their own. A key of the project's files that is on, off **or not set** (Sign commits,
  Auto-save, an extension's on/off and its own on/off settings, Certificate checks) is a switch
  too (#1865), at the value **in force**: the file's own where it sets the key, marked and with
  Reset beside it; where it does not, the value beneath it (another file's, or the default),
  with no mark and no Reset. Turning it writes the key at this level, and Reset takes it back to
  not set. A harness plugin's on/off stays a three-way `select` of on, off and not set: not set
  leaves it to the harness, whose answer purlis does not know, so a switch would show a value
  nobody said. So does a key holding something that is neither on nor off, shown as held.
  While the core has not yet said what is in force (Saving's keys, on load), a key the file
  does not set holds a quiet "Reading…" in its control's place, and one the file sets is its
  switch at once (#1875): no row draws a select that turns into a switch. Where what is in force
  could not be read, the key stays a select.
- **Every select is the select family's** (#1860, #2906): `ui/select/Select.tsx`, React Aria's
  select ([The select and combobox families](#the-select-and-combobox-families)). An option's
  empty value ("not set", "Every workspace") is an option like any other; a value none of the
  options has shows the placeholder. A pick that sends the keyboard on (a New… that opens its
  form) keeps it there; otherwise the keyboard comes back to the select when its list closes. A
  row's select is named by the row's label, which React Aria says after what it holds. No view
  draws a native `<select>` (`purlis/select-family`, `designSet.guard.test.ts`).
- **A list is a box of lines; a whole file is an editor** (DS-10, #1842). `Field kind="list"`,
  one entry per line, is drawn under its name at the row's width, and grows to show every line
  as it is typed (`grownTo`): it is never dragged and never scrolls inside. A whole file's text
  — Settings' Edit as TOML and Edit as JSON — is not a `Field` but the light editor's CodeMirror
  made editable (`editor/TextEditor.tsx`): numbered lines, the file's colours and find on ⌘F or
  Ctrl+F, with Tab left to the form. It is a chunk of its own, never in the main bundle. When
  the file changes on disk under an edit, the same editor shows the file beside the edit
  (`ConflictMerge`, the merge view) until the person keeps the edit or discards it. The arrow
  the gutter draws on each difference is the pointer's way to take the file's lines; the
  keyboard's is a row of buttons under the view, after the edit in the Tab order, one per
  difference and named by the lines it takes, every difference and not only those scrolled into
  sight (#1862). A memory's
  body is a page too, so it is the same editor with the Markdown grammar (#1862); when the memory
  changed on disk under an edit, the body as it now is stands beside the edit in the same merge
  view, and the choice stays the memory's own Reload or Overwrite (ADR 0065 Q10).
- **No look of its own.** The set is drawn in `App.css` under `ui-*` classes, every colour a
  design-system token, so a theme reaches it exactly as it reaches everything else.
- **Groups are data, not markup.** A settings screen declares its groups — a stable id, a
  label, a line of help and its settings (`app/src/settings/groups.ts`) — and the layout draws
  them. A new setting is a few lines of data, and it cannot look different from its neighbours.

### A collection is data too, and adds no piece (ST-3)

Settings › Forges is the first **collection**: a list of entries in a settings file that a group
adds to and takes from (V91e–g, the spec on #1221). It is drawn from the set's pieces and adds
none: `CollectionView` (`app/src/settings/Collection.tsx`) lays out, as `FileRow` does, an entry's
heading and Remove over the entry's own `SettingRow`s, and an Add form whose fields are
`SettingRow`s around a `Field` or a `Choice`, ending in `SettingActions`' Add and Cancel. ST-4 (harness profiles) and every later
collection follow the same shape, in three layers:

- **The core: one module per collection, `listed`, `add` and `remove`.**
  `purlis_core::settings::forges` is the model (`crates/purlis-core/src/settings/collection.rs`
  says the contract). `listed(text)` gives each entry an **opaque identity** (a forge's is
  `forge:<place>:<fingerprint of the block>`, so it changes once the block moves or changes; a
  profile's fingerprints its name and its table), its label, where its keys are, and the Add
  form's values that would write it again. `add(root, base, &Entry)` checks the whole entry,
  writes it to the collection's home file and answers the new identity; `remove(root, base, id)`
  takes the entry out unless something uses it, and answers what it took. A refusal
  (`collection::Refusal`) has three parts: `fields` (by the entry's own key, said under that field of the Add form),
  `referrers` (who uses the entry, each with the Settings group it is changed in, followed as an
  SE-22 deep link), and `file` (the whole write).
- **The drawn base.** Every add and remove is sent against **the text the entries were drawn
  from**, taken when the button is pressed (`Collection.base`, carried in `EntryOp`), never the
  text as it stands when the write's turn in the queue comes. The core checks it, as every
  settings write does, so a Remove queued behind an Undo is refused rather than made to the
  entry now in its place.
- **Undo** (D-ST3-i, as amended): the Undo of an add is the inverse operation through the
  core, a remove of the identity the add answered, with its reference check, so it is refused for
  exactly what that remove would be refused for (a repo catalogued on the host since, say). The
  Undo of a remove writes the file's earlier text back exactly, through the level's whole-text
  write, against the text the remove left: same place, same spelling, same comment. Order is
  meaning here (a plane's first `[[forge]]` block is its primary group), so an add at the end
  would not be an undo. Putting an entry back only declares again, so it asks no reference
  check; it is refused if the file moved since. A collection with no text to restore (the
  machine store) undoes a remove with an add.
- **The wire: the file lists its entries** (`SettingsFile.entries`), and one command per write,
  `add_project_forge` and `remove_project_forge`, answering `EntryWritten`: the file as it now
  stands with what was added or removed, or the three-part refusal.
- **The window: the group declares a `collection`** (`groups.ts`'s `Collection`: its name, the
  noun Add and Remove say, the drawn base, its entries as the core listed them with the ids of
  their settings, and the Add form's fields), the level maps its name to the commands and
  answers each write's inverse (`project.ts`'s `entry`), and the driver writes it
  (`driver.ts`'s `entry`), in the same queue as every key. A refused Remove is said under that
  entry, by its identity, and a refused Undo at the head of the collection, until the next write.
  The window computes no label, host or rule of its own (V91l). A collection group is offered in
  the nav while it has no entry, since that is where one is added.

**An entry with a page of its own, and Rename (ST-4).** Harness profiles are the second
collection (`purlis_core::settings::harness_profiles`, home file the local settings file). A
profile has many fields, so each has **a page of its own** (V91e): a group with `sub` set, drawn
indented under its collection's group in the nav. The collection's group lists each entry as a
heading with Open and Remove, and holds Add; the page draws that entry alone (`adds: false`) with
its rows, Rename and Remove. The page keeps its writes under its collection's group
(`Collection.home`), so a Remove there lands the person on that group, where its Undo is. The
core adds `rename(root, base, id, to)`, allowed only while nothing uses the entry (V91k); it is
an `EntryOp` like the others, and its Undo is the rename back. A refused rename names each user
and whether it **follows** a rename everywhere (`Referrer::follows`, #1380): when every user
does, the refusal is drawn as what a rename would change ("Renaming this profile also changes 1
setting that uses it"), before anything is written, and the form offers **Rename everywhere**
(`rename_everywhere`, the op's `everywhere`), which renames the entry and those users in one
write of one file, so a failure leaves nothing half-renamed; its Undo is the rename back
everywhere. A profile's users that follow are the local file's own `[harness] default`; the
project's default is every teammate's and never follows, so while it names the profile the
rename stays refused, and so does a persona whose definition names it with `profile:` or a
`[dispatch.profiles]` list holding it (#1380). A user changed at another level than the entry's
says which (`Referrer::elsewhere`, on the wire `level` and `target`, #1241), and its link goes
there: a persona's to its tab (*Show <persona>*), another level's group through the window's
`landing()`. A user with neither draws its group's link at this level, as before. A picker's
New… for the collection opens its Add form and picks what was added.

## The Notice is a house piece too, by its own amendment

ADR 0037's amendment of 2026-10-05 (V91d, NO-1 #1223) adds **Notice** (`app/src/Notice.tsx`)
beside the settings set: the one way to draw a standing line in a project's window. It wraps no
primitive, only a native element and native buttons (`tabIndex={0}` on each, for #186), so the
argument above about props being the primitive's own does not apply; its props are the line's
own words instead. What it adds is a type that refuses a line with no way out: `fixes`, `link`,
`copy` or `onDismiss`, at least one, and always a `cause`. `at="pane"` draws it in its pane's
band of warnings, inside the pane above its terminal and never over it (#1647, #3142); the default, `at="inbox"`, lists it
in the project's Inbox (#1695).
`Notice.guard.test.ts` fails on a hand-built one. A fix whose press is still on its way
(`busy`, an Undo in flight, #1190) is `aria-disabled` and `aria-busy`, and ignores a press, so a
second press sends nothing; it stays focusable, so the keyboard is not thrown off it, and is
pressable again the moment the answer comes, a refusal included. A fix that is off (`off`, a read-only
project's sentence for an answer that writes the project's files, #826) is drawn the same way,
`aria-disabled`, with the sentence as its tooltip.

In the Inbox, Notices are listed by `NoticeList` (V91i, NO-2 #1229; #1695): every one, the most
important first — trouble before news, then the family order `IMPORTANCE` exports. They stood
under the tab strip two at a time, the rest behind "+N more", until the band folded into the
Inbox. A Notice already in its place is never moved, so the focus stays on its buttons as others
come and go. A Notice that answers something the person just did (`ANSWERS`: a refusal, an Undo,
a save's record, the away summary) brings the Inbox on screen as it arrives. The alerts the
drawer listed are Notices here too (`InboxAlerts.tsx`), each with the way out the core gives it.
Outside a list, as the window's own lines under the title bar are, a Notice is drawn where it is
written. A Dismiss of a
cause the core answers for lasts until the cause changes, across relaunches
(`app/src/dismissals.ts`). A Dismiss of a line about something that still waits on the person (a
chat stopped on a prompt, qw386) hides it for this run and that wait only (`hideForNow`), and says
so as its tooltip (`dismissSays`): the needs-you list and the tab's hand still show the wait.

In a pane (`at="pane"`), a Notice is one box: its line, and under it what a way out opened. A
pane's Notices stand in **a band of their own at the top of the pane, inside it**
(`NoticePaneRow`, #1647, #3142), and the terminal has the height the band and the pane's header
leave, so no Notice is ever drawn over what a chat wrote. They were in the pane's corner, over
the terminal, until the operator's screenshot of three of them hiding the conversation; then in a
row above the pane, between the chosen tab and its pane, until the operator's screenshot of
2026-10-24 showed it breaking the tab's join. **A pane is three rows** (`paneFrame.css`, the
operator's variant A): the band, on the terminal's own fill with an 8px runway at its top so the
chosen tab joins the terminal; the header (`PaneHead`), the chat at a glance at its start and the
pane's actions at its end (`PaneActions`, the one cluster of split side by side, split stacked and
close, which a view's pane draws too, #3147); then the terminal, pinned to its bottom so a warning
coming or going moves its top and never its last line. **A view's pane carries no title of its
own** (#3147, the operator's ruling of 2026-10-24): its tab is its name and the content panel is
that tab's panel, so the view starts on the fill its chosen tab joins, with no band; the same
`PaneHead` and `PaneActions` sit at its top right, over its first row, which holds what can be
done to what the view is about (or Settings' levels) and keeps the actions' room at its end. In
the band each Notice is a washed card
(`wash.warning`, `danger.wash` for trouble, no edge), its words above its ways out, a consent's
Allow set apart from Keep blocked and the only answer with an edge. The band is a `region`, "Warnings
for this pane"; a card never shrinks in it, so past its room the cards scroll inside it whole. Esc
on a warning with a Dismiss puts it away, and the keyboard goes on to the next warning or to the
terminal, never onto an Allow: a consent is landed on by its line. A consent has no Dismiss and
Esc leaves it. The press-settle guard (#1695) reads where each line is drawn against the band
itself, scrolling aside, never its `offsetTop`, which a card positioned for its mark would pin. In the row they are stacked in `.pane-notices`, in the
order the pane writes them: the shown chat's own, the one that waits for an answer before
anything starts first, then each hidden chat's. **At most two stand, and the rest are behind
"+N more"**, on a row of its own under them, as wide as the longer of "+N more" and the Show
fewer it says while open (both are drawn in one cell, the one not said hidden); it opens the rest in the band, closing on Escape (back on "+N more") or a press outside it.
A Notice past two is `hidden` where it is drawn, never moved, so it keeps its state, and one the
keyboard is on stays until the focus leaves it. The band and the header are never more than
about half the pane, and inside it the stack is never wider or taller than the pane: the sentence has the row,
the ways out go under it when they do not fit beside it, and the stack scrolls (#1481). Every
change of the row's height refits the terminal, which resizes the chat's pty and redraws its
harness; with two at most, a third Notice and every one after it change only the count. The
chat at a glance (the gauge, the harness, the breadcrumb) is the header's line, never over the
terminal. `Notice.pane.test.tsx` and `PaneBand.test.tsx` hold the shape and the rules, and
`pane-notices.e2e.ts` and `pane-warnings.e2e.ts` measure them, with no Notice over the terminal in a narrow pane and a
wide one. Everything the window draws is ordered inside `#root`, which is one stacking
context, so no `z-index` in the window is over a dialog or a menu: those are portaled to the
body and need no number.

## Tables, list rows and tree rows share one style each (DS-3)

Before #1835 the window drew **4 tables and 8 kinds of list row**, each with its own padding,
alignment and selection. The Chats list alone put a name about 53px past its own filter, partly
because every leaf drew an empty twist so that names would line up. ADR 0094 §4 gives each of
these one class, built only from the core geometry and a theme's colours, and
`app/src/primitives/collections.css` holds all three. That file is in the `components` layer,
above `App.css`. A view that takes a class gets its look, and migrating a view means adding the
class and deleting the view's old rules (DS-9 settings, DS-11 chats). There is no React
component: each view keeps its own markup and keys, as ADR 0037 asks, and the class draws it.
`collections.test.ts` checks each rule against the markup below with `cascade.testkit.ts`, so
specificity and `:hover` are taken into account.

**The sizes are component tokens**, each an alias of a core token and declared in the class it
sizes: `--ui-row-height` is `--size-chrome`, `--ui-row-pad` is `--space-4`, `--ui-row-gap` is
`--space-3`, `--ui-tree-step` is `--space-5`, and `--ui-cell-pad-inline` and `-block` are
`--space-4` and `--space-2`. To change a part, change its alias. A theme never sees these.

### Table

```html
<table class="ui-table ui-table-fixed">
  <thead><tr>
    <th scope="col">Level</th>
    <th scope="col" title="How many tasks may run at once" aria-label="How many tasks may run at once">Tasks</th>
    <th scope="col" class="ui-cell-actions" aria-label="Actions"></th>
  </tr></thead>
  <tbody>
    <tr><th scope="row">Project</th><td class="ui-num">3</td><td class="ui-cell-actions">…</td></tr>
  </tbody>
</table>
```

- **`.ui-table`** lays out automatically: a column is as wide as its content needs.
  **`.ui-table-fixed`** is for a view that sizes its own columns. It gives the columns their
  widths (in `App.css`, on `<col>`s or the headings) and the table keeps them whatever the cells
  hold. Pick fixed when the cells are controls whose width should not depend on the values in
  them, as in the grants table.
- **A heading is one line.** `thead th` never wraps. In a fixed table a heading that does not fit
  is cut with an ellipsis. In an auto table its column grows to fit, so a long heading should be
  shortened. **A heading that is shortened, or that can be cut, carries its full words in a
  tooltip.** In Settings that is `ColumnHeading` (`settings/ColumnHeading.tsx`, DS-9 #1841): the
  short words sit in a `.ui-table-tip` behind the shared tooltip (#2908), which makes them a Tab
  stop and shows the whole words and what the column holds; the whole words are the heading's
  `aria-label`, so a screen reader announces them with each cell. `.ui-table-tip` gets a dotted
  underline, so the tooltip can be found. Headings are `--font-size-sm` in `text.secondary` above a
  `border.strong` hairline. A `caption` sits above the table, at the start, in `text.secondary`.
- **Cells are aligned.** Every cell and heading starts at `--ui-cell-pad-inline` and sits on the
  middle of its row, so text lines up with the 28px control in the next cell, not with that
  control's baseline. A row's own heading (`<th scope="row">`) is drawn as a cell.
  **`.ui-num`** puts a number at the cell's end, in tabular figures. **`.ui-cell-actions`** makes
  a column of buttons as narrow as they are, at the row's end.
- Rows are separated by a `border.subtle` hairline. A row that is `aria-selected` or
  `aria-current` is filled like a selected list row.
- **A table of controls stacks into a form where its room is narrow** (#1859).
  **`.ui-table-stacks`** on the table, drawn in a **`.ui-table-room`** (the holder whose width it
  answers to, a size container): under 29rem the column headings are put away, each row's own
  heading takes a line, and each cell sits under its column's whole words, which the cell
  carries as `data-heading`, as many cells side by side as fit. The row's actions take a line of
  their own, a cell across the columns (a question about the row above) takes the whole width,
  and a cell with nothing in it is not drawn. Nothing is cut and nothing scrolls sideways. Each
  control still names its column and row in full, so a screen reader hears the same either way.
  The Dispatch limits table and the dispatch grants table (#1865) wear it. 29rem lies between
  Settings' body at a 1280×800 window (the default one until 2026-10-24, when it grew to
  1440×900), where the table stays a table, and at the narrowest
  window with both sidebars open, where it stacks; CI's runs of `settings-tables.e2e.ts` and
  `settings-limits.e2e.ts` hold both ends in the real WebView (#1865). It is the one length the
  stylesheet writes, because a container query's condition cannot read a token.
- **Short headings live in the window** (#1859, after D-1841-1). A column's short words are the
  table's own (`SHORT` in `settings/dispatch.tsx`), and the core keeps one name per limit, its
  `label`. The command line draws no table of limits, so a short form in the core would have one
  reader. Stacked, a cell shows the whole label, never the short words.

### List row

**A list's row is the list family's** since #2911: [The list family](#the-list-family) below
draws it, from the same geometry, with its slots (`RowLead`, `RowLabel`, `RowDescription`,
`RowTrail`) where `.ui-row-lead`, `.ui-row-label` and `.ui-row-trail` were. `.ui-row` is left only
under a tree's row (`.ui-row.ui-tree-row`), which the tree family takes next.

### Tree row

```html
<ul class="ui-tree" role="tree">
  <li class="ui-row ui-tree-row" role="treeitem" aria-level="1" aria-expanded="true">
    <span class="ui-tree-twist" aria-hidden="true"><ChevronRight /></span>
    <span class="ui-row-label">A parent</span>
  </li>
  <li class="ui-row ui-tree-row" role="treeitem" aria-level="2">
    <span class="ui-row-label">A leaf: no twist, and its name lines up</span>
  </li>
</ul>
```

- **`.ui-tree`** is the container. A tree row is a list row with **`.ui-tree-row`** as well, so
  it has the same height, slots, selection and focus ring.
- **Indentation is in token steps.** The level is `aria-level` (or `data-level` in a flat tree
  that is not an ARIA tree), either on the row or on the treeitem that holds it, so a nested tree
  (`role="group"` inside the treeitem) and a flat one both work. Level 1 is depth 0, and each
  level is one `--ui-tree-step` (12px) further in. Levels deeper than 5 are drawn at 5: a sidebar
  has no room for more, and `aria-level` still announces the real level. Each level above a row
  draws a `tree.guide` hairline, as `.tree` does.
- **A narrow view may draw fewer levels.** `--ui-tree-depth-cap`, set by the view on an element
  around the tree and never on a row, is the deepest depth drawn: the Chats view sets it to 1
  under 16rem, so every row under another stands one step in (#1682). The row, its guides and
  its twist are drawn at `--ui-tree-drawn`, the lesser of the depth and the cap. The cap is read
  with a fallback and set nowhere in `collections.css`, so it reaches the rows by inheritance and
  no rule in the components layer contends with it.
- **The twist is drawn only on a row with children.** A row with children has `aria-expanded`,
  on itself or on the treeitem that holds it. **`.ui-tree-twist`** is hidden on any other row, so
  a leaf that renders one by mistake shows nothing. The twist hangs in the step before the name
  (each row stands one step further in than its depth) and takes no room in the row. A parent
  and a leaf therefore start their names at the same place, and no row draws an empty twist to
  line them up. Use one chevron pointing at the name: it turns down when `aria-expanded="true"`,
  and is mirrored in right-to-left text.
- **The twist is a box, in the middle of the row.** It is one step wide and one row high
  (`--ui-tree-step` by `--ui-row-height`), held between the row's block edges by its auto
  margins, with the chevron, one step square, in its middle. Write it as the `<span>` above with
  the icon inside it, and never give the class to the icon itself. Nothing else places it: no
  baseline, no offset. A row taller than one row keeps its chevron on its middle line.

**The Chats list is drawn with these classes** (DS-11, #1843). Each chat is a `<button
class="chat ui-row ui-tree-row" role="treeitem">` in a flat `.ui-tree`. Its name is the label and
comes first in the markup, so the row is heard by its name; the state's mark is the lead, drawn
first on the row; the persona's mark (or a shell's), a folded row's count and "Stopping…" are the
trail. The twist is a `<span>` inside the row, since a button cannot hold a button, and its press
folds the row without opening the chat. A top-level chat's name starts one tree step, one mark
and one gap in (`--chats-name-at`), and the filter's text starts at the same place, behind a
glass in the marks' column. `chats-list.e2e.ts` measures the two against each other. What the
filter hides and why a key did nothing are said beside the view's title, so the list has no
empty line under its filter.

**Every tree and list in the main views is on these classes** (DS-12, #1844): the explorer,
a file tab's tree and the Changes view are `.ui-tree` with `.ui-tree-row`s (the `.tree` class is
deleted); a finished task's name and its "Finished (n)" line are tree rows at their chat's level;
the panel lists, the memory archive, a vault's secret names, Search's hits and the opener's
recent projects are `.ui-row`s; the vault's, Dispatches' and Saving's tables are `.ui-table`.
A tree whose levels are nested groups gives each row its `aria-level`, and puts the children's
level (`data-level`) on the element that holds what a row says under it (a folder's "Nothing
here", a clone's "Asking git…"), so a sentence stands where the children's names start. A row
that must grow to its name rather than cut it (the explorer's, which scroll sideways) keeps its
own `min-width: max-content` beside the class.

## The button family

**Every button, icon button and button drawn as a link is `Button`, `IconButton` or
`ToggleButton`** (`app/src/ui/button/Button.tsx`, #2904, ADR 0104), on React Aria's `Button` and
`ToggleButton`. No screen draws a `<button>` of its own for one, and none wears the old
`.ui-button*` classes: `eslint.config.js` refuses both (`purlis/button-family`, an error). A raw
`<button>` left is another family's control, said by its role or its family's class (the rule's
`otherFamilies`).

| Prop | Says |
| --- | --- |
| `variant` | `secondary` (the default, the plain button), `primary` (the answer purlis suggests), `ghost` (no box until the pointer reaches it), `danger` (what cannot be undone from purlis, D-1719-3), `grants` (an answer that grants, an Allow or an Accept), `soft-accent` (a suggestion that is not the answer), `warning-soft` (care, without ending anything), `link` (a button that reads as a link, as tall as its words so it can stand in a sentence), `quiet` (a ghost whose words are in the quiet tier until the pointer reaches it: a strip's end mark, a pane's split and close, a line's own marks, #3214) |
| `size` | `sm` 24, `md` 28 (the default), `lg` 32, at the default text size and density; `lg` only beside a list's main filter field |
| `icon` | the icon before the words, drawn as the `icon` part; `IconButton` takes only an icon and an `aria-label`, and is square |
| `onPress`, `isDisabled`, `isSelected` | React Aria's own words; `onClick` still reaches a Radix trigger the button is the `asChild` of |
| `tooltip` | its hint, the shared tooltip ([the tooltip family](#the-tooltip-family)), never a `title`; on an `IconButton`, a second line under its name |
| `className` | the button's place in its view (margin, width, flex, order), never a look |

- **A row of buttons takes one size** (`ButtonRow`): every button in it is the row's size,
  whatever it asks for, so a mixed row cannot be built. A row wraps by default (#1855): the
  buttons go onto as many lines as they need, and one wider than the row breaks its words. A
  Notice's line is a small `ButtonRow`, and a dialog's answers (its footer, drawn by the dialog
  family) and a form's (`SettingActions`) are buttons of the family.
- **An answer that grants is `variant="grants"`** (ADR 0097 §6): as plain as Keep blocked beside
  it, on the control's own ground, edged in the accent and led by an open lock the family draws
  itself, hidden from a screen reader so the button's name stays its words. Allow never takes the
  primary look, even where the source suggests it (the Inbox draws an Allow it would have
  suggested as a grant).
- **Drawn by its parts** (ADR 0103): the root carries `data-scope="button"`, `data-part="root"`,
  `data-variant` and `data-size`; its words are `data-part="label"`, its icon
  `data-part="icon"`. `app/src/ui/button/button.css` draws the family default from those and
  React Aria's state attributes (`data-hovered`, `data-pressed`, `data-selected`,
  `data-focus-visible`, `data-disabled`), and a theme's parts reach the same attributes. A state
  changes a colour, a ground or a ring, never a size.
- **The keyboard is React Aria's**: Tab reaches every button (the wrapper writes the `tabIndex`
  WebKit needs, #190), Enter and Space press it, the ring shows only from the keyboard, 2px in
  `focus.ring` at 2px outside it (R162), keyed on React Aria's `data-focus-visible` alone. A
  disabled button is out of the tab sequence; one that is unavailable but keeps the focus
  (`aria-disabled`, #1700) or busy (`aria-busy`) stays reachable. Both draw their words in the
  faint tier, on their own ground and edge, and never as under the pointer or pressed: the
  wrapper marks them `data-disabled`.
- **Stories hold it** (`Button.stories.tsx`, `docs/component-stories.md`): each variant, size and
  state, Allow beside Keep blocked, a row of one size, the icon buttons and a toggle, each walked
  by the keyboard and checked by axe in Chromium and WebKit.

## The field family

**Every box a person types in is `TextField` or `TextArea`** (`app/src/ui/field/Field.tsx`,
#2905, ADR 0104), on React Aria's `TextField`, `Input`, `TextArea`, `Group`, `Label` and `Text`,
and the form it is sent from is the family's `Form`. No screen draws an `<input>` or a
`<textarea>` of its own for one, and none wears the old `.ui-input`, `.ui-input-lead` or
`.ui-list-field` classes: `eslint.config.js` refuses both (`purlis/field-family`, an error). A
raw `<input>` left is another family's: a checkbox, a radio, a slider or a colour well is the
[choice families'](#the-choice-families) (`purlis/choice-family`, an error), and a combobox's is
the select family's (`purlis/select-family`, an error: [The select and combobox families](#the-select-and-combobox-families)).

| Prop | Says |
| --- | --- |
| `label`, `help` | its name over the box and one line of help under it; with no visible label, `aria-label` or `aria-labelledby` names it (a filter, a box in a table's cell, a settings row's box, whose row draws the label) |
| `validate` | what is wrong with a value, or nothing: said under the box when it is left with words the person changed (a box only tabbed through says nothing), then following the typing until it is right (N26) |
| `error` | a refusal from elsewhere, in its own words (the core's), said at once; `isInvalid` marks a box refused whose words are drawn elsewhere (a tab's rename card) |
| `size` | a `TextField`'s `sm` 24, `md` 28 (the default), `lg` 32, at the default text size and density; `lg` only for a list's main filter field; one size for a row of controls |
| `icon` | the mark at a `TextField`'s start: a filter's or a search's glass, hidden from a screen reader, taking no press |
| `keepsFocus` | a name unique to the box in the window: the box drawn in its place keeps the keyboard and the caret (R168) |
| `value`, `onChange` | the words, as React Aria's words: `onChange` is handed the string. A secret's box (`type="password"`) is never given its words, a type error if it is: it holds them itself, out of React's state and the page's markup, and its `onChange` says only whether it is filled (`secretBoxes.guard.test.ts` names each one) |
| `onKeyDown`, `onBlur`, … | the element's own native handlers, unstopped: a dialog's Escape and a list's arrows still hear a key the box does not take |
| `className` | the field's place in its view (margin, width, flex, order), never a look |

- **A field is its label, its control, its help and its error, as one.** React Aria ties the
  label to the box and the help and the error to its description. The error is an alert, as every
  refusal in the window is, and the box's edge takes the danger colour beside it: the words are
  the cue that is not colour. **A field that can be refused keeps its error's line** whether it
  says anything or not, one line tall, empty and unheard until it does, so a refusal coming or
  going moves nothing; a long one is cut short on screen and read whole as the description.
- **⌘⏎ sends a form's primary action** (Ctrl+Enter off macOS; N26), its first submit button, from
  anywhere in the form, a textarea included, where Enter starts a new line. A `Form` checks its
  fields first: one that is not right says why and the focus goes to the first such. **A chord
  sends only a plain or a suggested answer** (`secondary`, `primary`): a grant, what cannot be
  undone and any look added later are only ever pressed. Start a chat's ⌘⏎ starts a chat, but
  never approves a command and never starts one without the sandbox. A key from what a portal
  draws outside the form (a dialog, an open list) is never the form's. A surface
  whose primary action is not a form's submit asks `submits` (`ui/field/submitKey.ts`) itself.
- **Drawn by its parts** (ADR 0103): the field's root is `data-scope="field"` with its `label`,
  `help` and `error` parts; its control is `data-scope="text-input"` (with `data-size`, and the
  `icon` part) or `data-scope="textarea"`, the box React Aria's `Group` draws around the element
  typed in. `app/src/ui/field/field.css` draws the family default from those and React Aria's
  state attributes (`data-hovered`, `data-focus-within`, `data-invalid`, `data-disabled`). A field
  shows its ring whenever it holds the caret, from the keyboard or a press, as a text box's
  `:focus-visible` has it: 2px in `focus.ring` at 2px outside the box (R162). A state changes a
  colour or a ring, never a size.
- **A box with a mark at its start** lines its words up past the mark by the box's inset, the
  mark's size and a gap, all core tokens. A view whose words must line up with something else
  hands the box three sizes, and only these: `--text-input-start`, `--text-input-icon` and
  `--text-input-gap` (the Chats filter, whose text starts where the chats' names do). Every
  search box in the window has its glass (`fieldIcon.test.tsx`).
- **A textarea** is three controls tall at least, in the mono face, and as tall as a person
  drags it; with `grows` (a settings list field, DS-10) it is as tall as its lines and never
  dragged.
- **Stories hold it** (`Field.stories.tsx`, `docs/component-stories.md`): a labelled field,
  inline validation on blur, a refusal, the three sizes with their glass, an unavailable box, a
  textarea, a form sent by ⌘⏎, and a filter that keeps its caret while its list redraws, each
  walked by the keyboard and checked by axe in Chromium and WebKit.

## The select and combobox families

**Every one-of-many pick is `Select`, and every box with suggestions is `ComboBox`**
(`app/src/ui/select/Select.tsx`, `app/src/ui/select/ComboBox.tsx`, #2906, ADR 0104), on React
Aria's `Select`, `Button`, `SelectValue`, `ComboBox`, `Input`, `ListBox`, `ListBoxItem` and
`Autocomplete`. No screen draws a `<select>` or a `<datalist>`, writes a `role="combobox"` or an
`aria-haspopup="listbox"` itself, or wears the old `.ui-select` classes: `eslint.config.js`
refuses all four (`purlis/select-family`, an error).

| Member | Is |
| --- | --- |
| `Select` | a button that shows what is picked and opens the list to pick from; `value`, `options` (`value`, `label`, `isDisabled`, `tooltip`), `onValueChange`, `placeholder`, `isDisabled`, `isInvalid`, `tooltip`, `size` |
| `ComboBox` | a box to type in with the suggestions that match in a list under it; what is typed is the value, and a suggestion picked writes its words (Add a skill) |
| `ListComboBox` | the box over a list drawn in place under it, always shown, whose rows and aim are its owner's (the command palette): the field family's box with the combobox's ARIA, which only this family writes |

- **Drawn by its parts** (ADR 0103): a select's button is `data-scope="select" data-part="root"`
  (with `data-size`, and `data-open` while its list is up), drawn as the text input's box; in it
  what is picked (`value`), what it shows while nothing is (`placeholder`, in the quiet tier) and
  the drawn `chevron` at its end, in the quiet tier (R64). A combobox's box is
  `data-scope="combobox" data-part="root"` with its `chevron`. **Their lists are no family of
  their own**: each stands on the popover family's `menu` surface (`data-of="select"`, at least
  as wide as its box), and each option is the menu family's row, its tick the row's `indicator`,
  so a theme that sets a menu's row sets an option's too. `ui/select/select.css` draws the
  family default; a state changes a colour or a ring, never a size.
- **Sizes**: `sm` 24, `md` 28 (the default), `lg` 32, as the text input's; one size for a row of
  controls (the browser bar's page size is `sm`).
- **Keyboard first**: Tab reaches a select (`tabIndex` 0 for WebKit, #190); Enter, Space, ↑ or ↓
  open it on what it holds; ↑ and ↓ move and stop at the ends, passing an option out of reach
  with no reason; **an option out of reach that says why** (`isDisabled` with `tooltip`) is the
  menu's row that cannot run: it stays in the arrows' way in the faint tier, with why on the line
  under its words as its description, and a press or Enter on it picks nothing; Enter picks;
  Escape closes the list and leaves the value as it was, and on a closed select goes on to what
  it stands in (a form that closes). A letter typed goes to the first option it starts. **A long
  list is filtered**: from `FILTERED_FROM` (10) options a select's list opens with the popover's
  filter row holding the keyboard, and what is typed narrows it while ↑ and ↓ still move in it;
  the filter never names an option the typing has filtered out (D-2906-14).
- **Named by what it holds, then its own name**, as React Aria names it: a settings row's select
  by its row's label (`aria-labelledby`), one with no row by its `aria-label`. An unavailable
  select with a reason (`tooltip`) stays in reach of the keyboard so its reason is heard, and
  opens nothing.
- **Refused** (`isInvalid`, #3110): a select whose value the form will not take, or a combobox
  whose typed value is not allowed, draws the danger colour on its edge, inside the box as the
  text input's, and is marked `aria-invalid` and `data-invalid`; the words that say why are drawn
  by its row or its form and named by `aria-describedby`, so they are heard with it. A colour
  only, so nothing moves; a theme reaches it as the `invalid` state of either family's `root`.
- **The button carries what it holds as `data-value`**, which the e2e specs read
  (`e2e/acting.ts`, `choose`), and a test picks through `ui/select/select.testkit.ts`
  (`selectNamed`, `pickIn`, `offeredIn`).
- **Stories hold it** (`Select.stories.tsx`): a select holding a value, its placeholder, its list
  from the keyboard, a pick, a long list filtered, each size, an unavailable select, a refused
  one, a settings row, and a combobox's suggestions, unavailable and refused, in Chromium and
  WebKit, with axe.

## The choice families

**Every checkbox, radio, switch, segmented control, slider and option card is its family**
(`app/src/ui/choice/`, #2907, ADR 0104), on React Aria's `Checkbox`, `RadioGroup` and `Radio`,
`Switch` and `Slider`. No screen draws an `<input type="checkbox">`, `radio`, `range`, `color` or
`file` of its own, writes `role="checkbox"`, `radio`, `radiogroup`, `switch` or `slider` by hand,
imports Radix's checkbox, radio group, switch, slider or toggles, or wears the old `.ui-checkbox`,
`.ui-radio`, `.ui-switch`, `.ui-segmented-*`, `.ui-levels` or `.ui-choice-*` classes:
`eslint.config.js` refuses each (`purlis/choice-family`, an error).

| Member | Is |
| --- | --- |
| `Checkbox` | a native box in its own label: a press of its words ticks it as a press of its box does, Space from the keyboard; `isIndeterminate` draws a bar. `variant="chip"` is one of a few small chips over the list they narrow, its tick in a slot it always keeps |
| `CheckboxGroup` | a named group of boxes, each holding its own value (two may stand for one); `roving` makes the row one Tab stop with the arrows between its `RovingCheckbox`es (the Chats list's state chips, #1687) |
| `RadioGroup`, `Radio` | one of a few, a column of options, each its mark, its words (and a `lead` before them: a persona's mark) and a `description` under them |
| `SegmentedControl`, `Segment` | a few views of one thing side by side (a level of Settings, a side view's scope), `sm` (the chrome's height, never under 24) or `md` (the control's, as a button's); a segment's `count` is beside its words and in its name. The chosen segment stands on the window's ground with a 1 px edge under it, held to 3:1; its weight never changes |
| `OptionCards`, `OptionCard` | one of a few compared before picking (Start a chat's harnesses, R24): cards of one height, each its `icon` and `title`, its `detail` under them, and **no radio circle**; the chosen card takes the accent's wash, its edge and its check. `variant="tile"` is a square tile that is only its icon, named by `aria-label` (a persona's mark and colour) |
| `Switch` | a setting in force as it is turned (R63): a 34 × 20 pill at the person's text size whatever a theme's control height, its 14 px thumb 3 px in and sliding on the knob's token (`quick`, 150 ms in Purlis), on in the accent's fill with the thumb in the window's colour; never sliding under reduced motion |
| `Slider` | a number along a range: the arrows, Page Up and Down, Home and End move it; its value is drawn beside it as `shown` says and heard as `spoken` says, once, as it moves; it is named by its row's words (`label`, React Aria's `Label`); its id is on its root and hands the focus to the thumb, so its row's name and Reset find it |
| `ColorWell` | the platform's colour well and picker, handing back the colour it settles on once |

- **All of one group is one Tab stop**, the arrow keys moving the pick along (the WAI-ARIA radio
  pattern, React Aria's own): a radio group, a segmented control and option cards alike. Each
  option is a native radio, named by its words; its description or detail describes it and is
  not part of its name.
- **A choice never moves anything** (#2800, R153). Ticked, picked, on, under the pointer, focused
  or unavailable, a choice changes a colour, a ground, an edge's colour or a ring, never a size;
  a tick, a dot and a card's check keep their room whether they show or not.
- **Chosen is never colour alone** (WCAG 1.4.1): a tick, a dot, the thumb's place, a chip's
  tick, a segment's edge under it, a card's edge and check.
- **Drawn by its parts** (ADR 0103): each root carries `data-scope` (`checkbox`, `radio`,
  `switch`, `segmented-control` and `segment`, `slider`, `option-card`) and `data-part="root"`,
  with its marks and words as parts inside it (`box`, `mark`, `track`, `thumb`, `fill`, `label`,
  `description`, `count`, `output`, `icon`, `title`, `detail`, `check`).
  `app/src/ui/choice/choice.css` draws the family default from them and React Aria's state
  attributes (`data-selected`, `data-hovered`, `data-focus-visible`, `data-disabled`). The ring
  is 2px in `focus.ring` at 2px outside the mark, from the keyboard only (R162).
- **Its hint is the shared tooltip** (`tooltip`, #2908): why an option is out of reach, or what
  it does that its words do not say.
- **The settings set draws from them**: `Choice` is the radio group (`radio`), the option cards
  (`cards`), the switch (`switch`), a checkbox (`toggle`), a group of boxes (`checks`) or chips
  (`chips`); `Field kind="range"` is the slider; the level switcher and a side view's
  `ScopeSwitch` are the segmented control.
- **Stories hold it** (`Choice.stories.tsx`, `docs/component-stories.md`): boxes ticked and not,
  a bar, unavailable; chips over a list; a radio group with an unavailable option; switches on,
  off and unavailable; the segmented control at both sizes; a slider; option cards and tiles,
  each walked by the keyboard and checked by axe in Chromium and WebKit, with a tick, a turn and
  a pick held to move nothing.

## The window's own families: panels, sections, empty states and status items

**Every region panel, panel section, empty state and status item is drawn by its family** (#2913,
spec #2897, ADR 0104), each one wrapper under `app/src/ui/` and its own lint rule at error
(`purlis/panel-family`, `purlis/panel-section-family`, `purlis/empty-state-family`,
`purlis/status-item-family`): a screen wears none of their old classes and never writes their
`data-scope` itself.

| Family | Wrapper | Parts | Variants |
| --- | --- | --- | --- |
| Panel | `Panel`, or `panelRoot(variant)` on a library's resizable panel (`ui/panel/Panel.tsx`) | `root` | `side`, `content`, `bottom` |
| Panel section | `PanelSection`, `PanelSectionHead`, `PanelSectionTitle` (`ui/panel/Panel.tsx`) | `root`, `head`, `title` | `default` |
| Empty state | `EmptyState` (`ui/empty-state/EmptyState.tsx`) | `root`, `icon`, `headline`, `body`, `action` | `page`, `panel` |
| Status item | `StatusItem` (pressed, runs a command), `StatusCell` (says, runs nothing), `StatusVerdict` (`ui/status/StatusItem.tsx`) | `root`, `icon`, `label`, `count` | `bar`, `pill`, `waiting`, `accent` |

- **The window's geometry is the theme's** (#2805): the gap between the regions is
  `window.gutter`, a panel's corners `panel.corner-radius`, and the hairline between two flush
  regions `panel.border-width`. Purlis Classic's are 0, 0 and 1, so it draws flush as it always
  did; Purlis floats its panels 8 apart with 12px corners and no hairline, one step lighter than
  the frame (its theme's `panel.root.fill: surface.raised`). The regions draw the
  gutter (`App.css`, `.regions`), the panel family the corners and the ground.
- **A section's title** is the label's capitals in the secondary tier, its mark in the title's
  colour. Where it sits (its margin) is its view's; the line between two sections is a separator.
- **The status bar follows its contribution model** (N28, `ui/status/StatusBar.tsx`): every item,
  purlis's own and an extension's, says its `id`, `side` (left or right) and `priority` (highest
  first from the side's left), and the person may hide any item from the bar's menu (a right
  click, or the menu key or Shift F10 on an item) and show it again there; what is hidden is kept
  in the layout file under `status`. One item the keyboard reaches cannot be hidden (Notices), so
  the menu always opens from one, and when it closes the keyboard is back on the item it was
  opened from. A press runs the item's command: purlis's own items are `StatusItem`s whose press
  is the command, and an item given as data names a command the bar runs through `onCommand`.
  An item that comes and goes sits on its end's inner side, so its arrival moves no neighbour;
  a long project name gives way first (`shrinks`), and a line after an item is the bar's
  separator part (`separated`).
- **Status pills** at the title bar's right (R115) are `StatusItem`s in the `pill`, `waiting` or
  `accent` variant: at least 24px tall and wide in CSS px at every text size and density, full
  corners, 12px words. `StatusItem` shares the button family's press (`ui/pressable.tsx`). The `waiting` pill
  stands on the amber wash, `wash.warning`, with its hand and count in the needs-you colours. A `tone` (`waiting`,
  `failed`, `success`, `quiet`, `accent`, `danger`) colours an item's icon beside its words, never
  instead of them.
- **Stories hold them** (`Panel.stories.tsx`, `EmptyState.stories.tsx`, `StatusItem.stories.tsx`):
  panels flush and floating, a section with its action, each empty state, the pills and the bar
  with its menu, each walked by the keyboard and checked by axe in Chromium and WebKit.
## The tooltip family

**Every hint in the window is the shared tooltip** (`app/src/ui/tooltip/Tooltip.tsx`, #2908,
ADR 0104), on React Aria's `TooltipTrigger` and `Tooltip`. No element carries a native `title`
for one: `eslint.config.js` refuses it on every element and every member of the button and status
item families (`purlis/tooltip-family`, an error). A frame's `title` is its name and stays.

| Where the hint is | How it is written |
| --- | --- |
| On words, a mark, a row, a tab, a field | `<Tooltip tip={…}>` around the one element, which hands its props and its ref to the DOM; `shortcut` draws the key that does the same after the words (the `shortcut` part, #2921) |
| On a `Button`, a `ToggleButton`, a `StatusItem`, a `StatusCell` or a menu's `MenuItem` | its `tooltip` |
| On an `IconButton` | its `aria-label`, which its tooltip always shows; its `tooltip` is a second line under it (the `detail` part) |
| Beside a menu's row, opened by the row's rest | `AnchoredTooltip` (`ui/menu/MenuTip.tsx`) |

- **One placement and one delay, which no call site changes**: beside its target at the target's
  end, 10 px from it, flipped to its start where the window has no room (R166); open after the
  pointer rests 500 ms, at once on the keyboard's focus, and at once for the next target while
  one was just open. A row's tooltip stands by the pointer or the row's words, not the whole row,
  and an anchored tooltip (a row's, a menu row's) keeps its width: it takes the side with room for
  all of it, and stands under its anchor where neither side has (#3328, `ui/tooltip/side.ts`).
- **WCAG 1.4.13**: Esc takes it down and leaves the focus where it was (dismissible). **The first
  Esc is the tooltip's alone**: heard first on the window's capture phase while it is up, it reaches no
  dialog, menu, palette or terminal, and the next Esc is theirs. The pointer can move onto a tooltip it
  opened and it stays (hoverable), a menu row's note included; it stays until the pointer or the
  focus leaves, or Esc (persistent). `Tooltip.test.tsx` and the stories hold all three.
- **A tooltip the keyboard opened takes no pointer** (`data-pointer` is the pointer's), so it never
  stands between the pointer and what is under it. One the pointer opened takes it, in the title
  bar too, where it is a drag region of its own: a press on it moves the window as the bar does.
- **Reachable by the keyboard.** A control takes the keyboard already. Words with a hint become a
  Tab stop and draw the focus ring, as a settings row's badge always was; a mark inside a control
  (a row's change mark, a tab's persona), one hidden from a screen reader, a label, and a mark
  whose hint is its own name take none, because the control or the name says what it means. A
  hint that only repeats an element's words (a name cut short) is the pointer's: no focus opens
  it. While it is shown, a hint is its target's description.
- **An icon button's tooltip and its name agree by construction**: the tooltip shows the
  `aria-label`, its hint a second line under it, and is not heard a second time. A raw icon-only
  `<button>` around which `<Tooltip>` stands is drawn the same way.
- **A disabled button or status item with a hint keeps the focus**, says it is unavailable (`aria-disabled`) and
  does nothing (`app/src/ui/unavailable.ts`, an allowlist: only the focus's and the pointer's
  handlers stay, whatever its parents hand it; it is a plain `type="button"`, and a click, Enter
  and Space stop at it). Its reason is its description for good, so a screen reader hears why
  whether or not the tooltip is up. A disabled text field or area leaves the Tab order, as a
  native one does, and is described by its hint for good the same way.
- **A read-only project's group of controls** (`ReadOnlyFieldset`) keeps its fields natively
  disabled. Its legend, "Read-only", is the group's name and a Tab stop whose tooltip is the
  project's sentence; the group and each settings row's control in it are described by it.
- **Only while its project is in front** (`projectShown.tsx`): a project kept behind another
  opens no tooltip over the window.
- **Drawn by its parts** (ADR 0103): the surface is `data-scope="tooltip"`, `data-part="root"`,
  an icon button's second line `data-part="detail"`; it has one look, so a key names the part
  alone (`tooltip.root.corner-radius`).
  `app/src/ui/tooltip/tooltip.css` draws the family default on [the overlay
  surface](#the-overlay-surface), and a theme's parts reach its fill, words, edge, corners and
  shadow; its size is its words'. Purlis draws it with corners of 8 and no edge (R166).
- **Tests read a hint as the keyboard does**: `tipOf(element)` (`ui/tooltip/tooltip.testkit.ts`)
  focuses the element as the keyboard would and reads the tooltip it opens.
- **Stories hold it** (`Tooltip.stories.tsx`): words, an icon button's name, its second line and
  a disabled button's reason, each walked by the keyboard (Tab opens, Esc dismisses, Tab leaves)
  and checked by axe in Chromium and WebKit.

## The menu and popover families

**Every menu, context menu and popover is the menu or the popover family**
(`app/src/ui/menu/Menu.tsx`, `app/src/ui/menu/ContextMenu.tsx`, `app/src/ui/popover/Popover.tsx`,
#2909, ADR 0104), on React Aria's `MenuTrigger`, `Menu`, `MenuItem`, `MenuSection`,
`SubmenuTrigger`, `Popover`, `Dialog` and `DialogTrigger`. No screen says a menu's role or wears
the old `.ui-menu*` and `.ui-popover` classes, and nothing imports Radix's menus or popover:
`eslint.config.js` refuses all three (`purlis/menu-family`, an error). A select's list stands on
the same surface, its options the menu's rows ([The select and combobox
families](#the-select-and-combobox-families)).

| Member | What it is |
| --- | --- |
| `MenuTrigger` | A menu opened by a press on its trigger: a button of the family, or any element (a row, a strip's mark) made pressable. Enter, Space or Down opens it on its first row, Up on its last |
| `AnchoredMenu` | A menu its owner holds open under an element that keeps its own keys and presses (a roving row, a strip's mark a click toggles); the owner says `aria-haspopup` and `aria-expanded` |
| `ContextMenu` | A menu opened on a thing by a right-click, at the pointer, or by Shift+F10 or the menu key on the thing itself. The innermost thing's opens; the ones around it stand back |
| `Menu`, `Submenu`, `InlineMenu` | The menu itself, a row that opens one, and a short list of rows drawn in place (a chat picker in a dialog) |
| `MenuItem`, `MenuWords`, `MenuSection`, `MenuSeparator`, `MenuTip` | A row; its words and the one line under them; a group, named by `title` (`named` for a name from data); the rule between groups; a row's tooltip |
| `PopoverTrigger` with `PopoverDialog` | A popover that asks, opened by a press on its trigger: the keyboard in it, handed back as it closes |
| `HeldPopover` | The same, held open by its owner beside a roving row |
| `AnchoredPopover` | A popover that says something beside an element (a card beside a row, a refusal under a field): it takes no keyboard, and its owner says when it is up |
| `PopoverFilter`, `PopoverRule`, `PopoverList`, `PopoverFooter` | A popover laid out as a list (R165, `layout="list"`): the filter row with its rule under it, the list that scrolls while they stay in view, and the footer that says its keys, "↑↓ move · ⏎ go · esc close", each key in a mono cap |

- **Placed under its trigger, lined up with its start or its end** (`align`), and never nearer
  the window's edge than 8 px; it turns to the other side where there is no room. A row's ⋯
  menu, as a setting's row has it (R59), lines up with the end of its button.
- **Closed by a click outside, by Escape and by choosing a row**, and the keyboard goes back to
  where it was, or where the owner's `onClosed` puts it (a row that went into Settings sent it
  there). A press outside still reaches what it was on: **a menu is not modal**, so the rest of
  the window is not hidden from a screen reader and a menu on a strip is no question. A preview
  the pointer's rest opened (a tab's task chip) takes no keyboard (`takesNoKeyboard`), and
  Escape and a press outside close it from wherever the keyboard is.
- **Drawn by its parts** (ADR 0103). The surface is `data-scope="popover" data-part="root"`, its
  `data-variant` `popover` or `menu`; inside it the `filter`, `rule`, `list`, `header`,
  `footer` and `key` parts. A menu's row is `data-scope="menu" data-part="root"`, its
  `data-variant` `default` or `danger` (D-1719-3), with its `label`, `description`, `shortcut`
  and `indicator` parts (a toggle's tick, a submenu's arrow). `ui/popover/popover.css` and
  `ui/menu/menu.css` draw the family default; a theme's parts reach the same attributes. A
  state changes a colour, a ground or a ring, never a size; the row the keyboard is on is
  highlighted and ringed inside (`--focus-ring-inset`), since a list's rows sit edge to edge (the
  operator's ruling of 2026-10-22, [ADR 0104's amendment](adr/0104/2026-10-22-a-row-draws-its-focus-ring-inset.md)). A
  view places a menu with a class of its own and may hand it its least and most width
  (`--menu-min`, `--menu-max`) and a row's indent (`--menu-indent`).
- **A row says its shortcut** at its end: the key registry's for its act (`MenuShortcuts`,
  #2921), else the key its caller knows. It is drawn, not run onto the row's description.
- **Drawn only while its project is in front** (`projectShown.tsx`): every surface asks, and
  `projectShown.guard.test.tsx` draws each member under a project behind.
- **Stories hold it** (`Menu.stories.tsx`, `Popover.stories.tsx`, `docs/component-stories.md`).

## The list family

**Every list is the list family** (`app/src/ui/list/List.tsx`, #2911, ADR 0104), on React
Aria's `ListBox` and `GridList`. No screen says a list's role (`listbox`, `option`) or wears the
old `.ui-list` class: `eslint.config.js` refuses both (`purlis/list-family`, an error).

| Member | What it is |
| --- | --- |
| `ListBox`, `ListOption`, `ListSection` | **A list to choose from** (the WAI-ARIA listbox): a persona to start a chat with, a theme, a search hit, a skill. A choice of one follows the arrows; `onAction` runs the one picked; `layout="grid"` lays its options out as cards the arrows walk in two directions; an option `unavailable` says so (`aria-disabled`) and keeps the keyboard, so why is read |
| `List`, `ListRow`, `ListRowSection` | **A list of rows that act** (a grid of one column): a panel's todos, a vault's rows. A row runs something (`onAction`), may be chosen, and may hold its own buttons, which ← and → reach |
| `RowLead`, `RowLabel`, `RowDescription`, `RowTrail`, `RowBelow` (`Row.tsx`) | A row's slots, the same in a list, a tree and a table: its marks, its words (one line, cut with an ellipsis), the one line about it in the quiet tier, its end (a count, its own actions), and a line under its words inside it (what a search matched in its body, why the core refused it). **In an option, its words are its name and its one line its description** (React Aria's `label` and `description` slots), so a reader hears the name alone and then what it is, never the row's content run together |
| `AimedList`, `AimedOption` (`AimedList.tsx`) | **A list a field aims at**: a combobox's list drawn in place, the palette's actions, projects and files. The field keeps the keyboard and moves the aim (`aria-activedescendant`); the aimed row wears the keyboard's ring inside it and the aim's fill (`control.aimed`); a press runs a row |
| `ListFilter`, `useListFilter`, `noMatchSaid` (`ListFilter.tsx`) | **A list over about seven items gets a filter** (R164): from eight, a field at the large size over the list, the total in its placeholder ("Filter 28 personas"), a case-insensitive part of each name, and "No persona is called “x”." where nothing matches. What is typed is the caller's, so drawing the list again keeps the field, its focus and its caret (R168) |
| `useRowDrag` (`drag.ts`) | What a dragged row carries, by type, to a drop target outside the window's collections (a file's reference onto a chat) |

- **Sizes**: `sm` (24 px, a side view's dense list), `md` (28, the default) and `lg` (32, a
  dialog's list beside its details). A list takes one size. Its rows keep `space.1` apart, so one
  row's pill never touches its neighbour's wash.
- **Where a list gets a filter** (R164): a list in a dialog takes `ListFilter` from eight items.
  A panel's list (`PanelList`) keeps its own search from a page and one row, which reads a row's
  body too and pages the rest (ADR 0065 Q11): a side panel is no dialog, and it searches from
  where it starts to page (`PAGE`).
- **Nothing moves when a state changes** (#2800). Under the pointer a row takes its pill
  (`surface.hover`); chosen, the wash (`list.selected`) with its words in the primary tier and an
  edge down its start (`list.selected-edge`), a shadow, so it is told by a shape as well as a
  colour (R21); the keyboard's ring is drawn inside the row (`--focus-ring-inset`, [ADR 0104's
  amendment](adr/0104/2026-10-22-a-row-draws-its-focus-ring-inset.md)). A row at rest is in the
  secondary tier (R19); one that cannot be used in the faint tier. **The rows stand in from the
  list's edges** (`space.2`), so a pill never touches the panel the list is joined to.
- **The keyboard is React Aria's**: one Tab stop, the row last focused; ↑ and ↓ move and stop at
  the ends; Home and End; typing finds a row by its `textValue`; Enter and Space choose or run.
- **A row's menu and its tooltip are the row's own** (`menu` and `tip` on a row): a collection
  draws its rows itself, so no `ContextMenu` or `Tooltip` can be put around one. The menu opens
  on a right-click, or by Shift+F10 or the menu key while the row has the keyboard, on its first
  row (`useContextMenu`, `askForAMenu` in `ui/menu/ContextMenu.tsx`); the tooltip shows under the
  pointer after the window's delay, at once while the keyboard is on the row, is the row's
  description while it is shown, and Esc takes it down (`RowTip.tsx`).
- **A drag is never the only way**: whatever dropping a row does, its menu or a key does too
  (Shift+Enter on a search hit hands it to a chat).
- **Drawn by its parts** (ADR 0103): the list is `data-scope="list" data-part="body"`, a row the
  family's root (`data-part="root"`, its `data-size`) with React Aria's state attributes, its
  slots and a group's `header` (`data-named` for a name from data: a day, a file). `ui/list/list.css`
  draws the family default; a theme's parts reach the same attributes.
- **Stories hold it** (`List.stories.tsx`): a list to choose from under its filter, a list a field aims at, rows that act
  with a row's own button and its menu, a grid of cards, and an empty one, in
  Chromium and WebKit, with axe.

## The dialog family

**Every dialog and alert dialog is `Dialog` or `AlertDialog`** (`app/src/ui/dialog/Dialog.tsx`,
#2910, ADR 0104), on React Aria's `ModalOverlay`, `Modal` and `Dialog`. No screen writes
`role="dialog"` or `role="alertdialog"` itself: `eslint.config.js` refuses it
(`purlis/dialog-family`, an error). A surface with an anatomy of its own, the palette, is
`DialogSurface`: the family's modal, fixed top, width and height, and its own rows inside.

**The dialog rules** (#2798, #2789, the operator's ruling in #2910), each held by the family:

| Rule | How |
| --- | --- |
| **Header, body, footer** | Three rows: the title and the line under it, the body, which is the only part that scrolls, and the footer with the answers, always in view. A control the keyboard is on is never under the footer (WCAG 2.4.11): the two never overlap. |
| **Width by size, never by content** | `size="standard"`, 560 px, or `size="wide"`, 780 px, only for a list beside its details; each at most the window's width less 16 px a side. |
| **Height** | At most the window less 48 px; `height` gives a dialog one of its own, and it takes the lesser of the two whatever it holds. |
| **A fixed top** | 24 px under the window's top edge, so a change in its content moves only its bottom edge. |
| **Text that changes with a choice keeps its room** | `Reserved` (`ui/dialog/Reserved.tsx`) lays every text a place can show in one cell, and an answer's `reserve` lists every word it can say (`Create`, `Creating…`), so nothing moves or resizes when a choice or a state changes. |
| **A list over about seven items gets a filter** | `useListFilter` and `ListFilter` (the list family's, `ui/list/ListFilter.tsx`): from eight items, a filter field at the large size over the list, its total in its placeholder ("Filter 28 personas"), a case-insensitive substring of each item's name, and "No persona is called “x”." where nothing matches. |
| **At most three answers, each in its place** | `cancel` leads, `main` trails, and one `alternative` may stand between them. They are props, not children, so a fourth, or a main action anywhere but last, cannot be written. The main action is in the same place in every dialog. |
| **⏎ and Esc** | The body and the footer are one form, so Return in a field is the main action and a button with the keyboard is pressed by its own Return. An answer that grants (`grants`) is pressed, never found by Return, from a field or a checkbox, and `byReturn` cannot say otherwise. An act that cannot be taken back (`danger`), or one that says so (`byReturn: false`, Start a chat), is pressed, never found by Return from a field. Esc closes, as `onClose`. A click outside answers nothing. |
| **No dialog opens another** | One opened while another is up says so (`console.error`). A question about a dialog's row is asked in its place (Extensions' Review), and the dialog comes back once it is answered. |
| **It starts on the right thing** | A dialog on its first field, else on the first thing it holds the keyboard can reach; an alert dialog on its safe answer, Cancel (#2980), or on the dialog itself while Cancel cannot be pressed, never on its main action. A grant never takes the keyboard as a dialog opens, even named by `initialFocus` (`grantFocus.guard.test.ts` holds every grant call site to it). `initialFocus` names another: an answer, a field, or the dialog itself while what it holds is read. Two alert dialogs start on another answer by ruling: Close a chat (ADR 0064) and Stop a task (V100-18). Closed, the keyboard goes back to what had it as it opened; `afterClose` may send it on. |

| Prop | Says |
| --- | --- |
| `title`, `subtitle` | the header: the title names the dialog, the line under it (where it is, what it is about) describes it |
| `description` | what the question is, said first in the body; it describes the dialog too |
| `cancel`, `main`, `alternative` | the answers: a button of the family each, its words as `children`, `variant` (`main` is `primary` unless it says `danger` or `grants`), `isDisabled`, `onPress`, `reserve`, and `reserveGrant` for an answer that is a grant for some choices only, which holds the lock's room in every look |
| `summary` | what the footer says before its answers, in the quiet tier |
| `size`, `height` | the width's size, and a height of its own |
| `className` | a hook for what is inside it and for the tests, never its size or place |

- **Answers a part of the body owns** (a guided sign-in's Test and Create, `VaultSignIn`) are
  drawn by it with `DialogAnswers`, in the footer, in their places, instead of the dialog's own.
- **Over the window, and only its project's** (`projectShown.tsx`): a dialog is drawn only while
  the project it is under is in front, and React Aria makes the rest of the window unreachable
  while it is up. A menu, a popover or a select's list opened in a dialog is React Aria's too, and
  stands over it. Esc closes the layer the keyboard is in, one at a time.
- **Drawn by its parts** (ADR 0103): the scrim, the root (`data-variant` `dialog` or `alert`,
  `data-size`), the header, title, subtitle, description, body, separator, footer, summary and
  actions, each `data-scope="dialog"` with its `data-part`. `ui/dialog/dialog.css` draws the
  family default, on the overlay surface's ground, border, corner and lift, and a theme's parts
  reach the root, the title, the subtitle, the description, the separator and the summary.
- **Drawn in place for a picture**: `DialogInPlace`, the same rows with no modal, is the theme
  specimen's dialog and the family's stories'.
- **Stories hold it** (`Dialog.stories.tsx`): a form, Esc, a question, a dialog longer than the
  window with every field walked and never under the footer, and a list beside its details
  whose choice moves nothing, in Chromium and WebKit, with axe.
## The session tabs are drawn by the tabs family's parts (#2914)

The chat strip's tabs are drawn by `app/src/ui/tabs`: `TabCell` is one tab's cell (its
`data-variant`, `data-selected`, `data-hovered` for a mouse and `data-first`), `tabsPart` marks the
column (`layout`), the strip and the panel under it (`body`), `tabPart` the label, the `×` and a
count, and `editorToolbar` the bar over a file. `tabs.css` draws Purlis's docked tabs from those
attributes; a theme's `tabs.*` parts reach the same ones (`crates/purlis-core/docs/themes.md`,
Session tabs). Choosing a tab changes its colour and fill only (`tabs.guard.test.ts`; the Docked
story measures every tab before and after each choice).

**The strip is on React Aria's tabs** (#3042, `app/src/ui/tabs/TabStrip.tsx`): `useTabStrip`
holds the strip's tabs as React Aria does (`useTabListState`), `TabStripRow` is the row that holds
the cells and its tablist (`useTabList`), and `StripTab` is one tab (`useTab`). They follow the
WAI-ARIA "Tabs" pattern:

- **The keyboard along it**: Left and Right move to the tab beside, wrapping past either end, Home
  and End to the first and the last, the direction following the locale. Only these keys are
  React Aria's, and only pressed on a tab: a `×` or a chip beside a tab keeps its own, Tab goes
  on to the next stop in the document (the `+`), and a key the tab has taken is its own (a lifted
  tab's arrows are the drag's).
- **Manual activation**: moving is not choosing; Enter or Space chooses, through the strip's own
  press (`tab.select:<id>`), which can do what React Aria's selection cannot: a press on the tab in
  front showing a task goes back to its chat, and a drag's release is no press.
- **One Tab stop**: the tab the keyboard is on while it is inside, the chosen tab once it leaves
  (or the first drawn when the chosen one is collapsed), and the chosen tab again when the tab
  with the keyboard goes.
- **Still the strip's own**: the tablist that owns the tabs by id and nothing else (below), the
  drag along it (`dnd-kit`, `sortable.tsx`; React Aria's drag and drop is for list boxes, grids,
  tables and trees) and the collapse into show-more (`fits.ts`, which counts the theme's gap
  between tabs).

`purlis/tabs-family` (an error) refuses a `tab`, `tablist` or `tabpanel` role written by hand,
the old hand-made tablist (`StripTablist`) and `data-scope="tabs"` outside `src/ui`. The tablists
still drawn by hand (the workspace strip, the activity bars, the project rail's) are turned off by
file in `eslint.config.js` until their own tickets move them.


## The activity family: the four corners

**A view's icon on a side's bar is an `ActivityItem`** (#2917, spec #2897 R116–R120, N17,
`ui/activity/ActivityItem.tsx`), with the lint rule `purlis/activity-family` at error. Its parts
are `item` (the root), `icon`, `count` and `indicator`; both corners draw the `bar` variant
(#3144, the operator's ruling of 2026-10-24, ADR 0038 as amended that day).

- **The left bar** (#3144): a 44px column of 36px icons, the frame's first column, beside the
  project rail, a gutter and a half from it (12px in Purlis, flush in Classic, D-3144-13), as
  the switcher prototype's variant D draws it
  with "Bar: column". The side panel's views at the top (Chats, Explorer, Search), its half of
  the bottom panel's (Repos, Changes) at its foot. A press switches the view; a press of the
  open one does nothing (R121), and ⌘B is the way to put the side away. The column stands
  whether or not the side is out, so nothing moves when it goes or comes back; while the side is
  away none of its icons is marked, and a press brings it back on that view. A hidden rail
  leaves the column at the window's edge, and the rail's peek slides in over it.
- **The right bar** (R117): the same column at the window's edge, its own views at the top (a
  press toggles the 300px right panel, whose header has a × that closes it) and its half of the
  bottom panel's at its foot.
- **The `tab` variant** (R116): 32×30 tabs across the top of a side panel, over a rule. No bar
  of the window draws it since #3144; it stays in the manifest, which never takes a variant
  away, for a theme that names it.
- **Classic's accent edge** is drawn along the left column's left edge, as the switcher
  prototype draws it, and along the right bar's right edge, as #2917 drew it.
- **Its states are colours** (R118): muted at rest, the card's fill under the pointer, the
  accent's wash while its view is open. The `indicator` is empty by default; Purlis Classic paints
  it as its accent edge through parts.
- **A count** (`ActivityCount`, R119) sits at the item's top right, ringed in its bar's ground;
  what waits on the person is drawn hot, in the needs-you colours (R120), which a theme's count
  colours never reach.
- **Only the part that appears moves** (R122, R124, R127, R128): the right panel comes 24px in
  over `duration.open`, a bottom half 16px up over `duration.reveal`, a side view's list 4px up
  over `duration.swap`. A view switch inside an open panel replays nothing, and closing has no
  motion.

## Every control is a family's

**Each kind of control is a component family, drawn only from the tokens** (ADR 0104, #3294). A
button is [the button family](#the-button-family), a box a person types in [the field
family](#the-field-family), a select [the select family](#the-select-and-combobox-families), a
checkbox, a radio, a switch, a segmented control, a slider or an option card [the choice
families](#the-choice-families), and a menu and its rows [the menu and popover
families](#the-menu-and-popover-families). The one stylesheet of control classes they replaced
(ADR 0094 §4, #1834) drew nothing once the last of them moved, and is gone. What still wears a
`ui-` class natively is a list's row, a tree's row and a table ([Tables, list rows and tree
rows](#tables-list-rows-and-tree-rows-share-one-style-each-ds-3)).

An answer that trusts text from outside is plain (#2591): the repository-instructions tab's Add
to memory saves a repository's own words into memory, so it is a secondary `Button` beside Not
now, with the same weight. It grants nothing, so it is not `variant="grants"`.

What every control holds to:

- **One height.** Each family's sizes are its own.
- **One focus ring.** A control's ring is `--focus-ring-width` in the theme's `focus.ring`,
  **outside** it at `--focus-ring-offset`.
- **A toggle button is `ToggleButton`** (DS-12): pressed, it says `aria-pressed` and stands on
  `accent.surface` behind an `accent.base` edge, as a chosen segment does, and stays so under the
  pointer (*Changed only*, *Show ignored files*, Search's switches, a file's *Source*).
- **Every colour is one token per declaration**, so `contrast.test.ts` reads each pair a family
  draws. No colour hides behind an alias of the family's own.
- **The families are in the `components` layer, above `App.css`** (`styles.css`). A view rule
  that reaches a control cannot redraw it, and the window's zero-specificity focus floor cannot
  pull its ring inside. A view still places a control (margin, width, flex, order), which no
  family sets.

`controls.guard.test.ts` holds this: a native button, field, select, table or menu that is not
its family's and does not wear its class fails, unless its line is in
`controls.guard.allowance.ts`, the list of the controls the window had before the classes. A
native `<button>` may wear a list's row or a tree's row and a `<table>` `ui-table`; no class makes
a native field, select, checkbox, radio or menu right. Moving a control onto its family deletes
its line there; nothing adds one (`docs/design-system.md`, "The guards, and the lists that only
shrink").

## Menus

**Every menu in the window is drawn by the same seven rules**, a context menu and a dropdown
alike. They are what VS Code's, JetBrains' and the platform's own menus do, and the next menu
follows them without deciding anything. For a context menu they are applied in one place,
`app/src/menuModel.ts`, which says how each row `actions.menuRows` lists is drawn; a row's
words are written once, in `app/src/menuWords.ts`; and every menu is drawn by the menu family
([The menu and popover families](#the-menu-and-popover-families)), a dropdown too.

1. **A row is a short verb, and does not say the name of the thing the menu is about.** A menu
   is opened ON a thing, which is in front of the person: *Rename…*, *Pinned*, *End*, never
   *Rename chat console-developer 16…*. The thing is said once, as the menu's own accessible
   name (`aria-label` on the content: "Tab console-developer 16"). **A row keeps a name only
   when it is another thing's**: *Go to steward 4*, *Open beside steward 4*, *Merge into svc*.
   The catalogue's title still names the thing, for the palette, where a row stands among every
   other row of the window (`docs/ui-copy.md`, "Buttons and menu items").
2. **A row's note is its tooltip and its description, not a line under it.** The tooltip is the
   window's shared one (`AnchoredTooltip`, #2908), shown beside the menu once the keyboard or the
   pointer has rested on the row for `TIP_AFTER_MS`; the description is the row's
   `aria-describedby`. **Two things stay under a row's words**: what a row that cannot be
   undone costs (*Ends the program it runs. There is no undo.*), and why a row cannot run. Each
   changes a decision the person is about to make. **Neither is ever cut**: it wraps inside the
   menu's width. A warning that lost "There is no undo." to an ellipsis would say less than
   nothing, and a row that cannot run takes no keyboard, so nothing would open the rest of its
   reason. The warning is a short sentence written for the menu (`menuWords.ts`, `WARNS`) that
   says no more than the catalogue's note for its row; the whole note is the tooltip.
3. **Rows of one kind are one row that opens a submenu.** *Open ▸* holds the views that open in
   a tab of their own (Activity, Network); *Ask ▸* a row per persona, named by the persona
   alone; *Stop ▸* the ways to stop a chat; *Curate ▸* and *Move to ▸* the rows the catalogue
   decides the length of. What every row of a submenu does is said once, as the tooltip of the
   row that opens it. A menu's top level is **at most ten rows in at most four groups**: the
   thing itself, what it opens or starts, how it runs, and last what ends or deletes it. A
   submenu whose rows end something (*Stop ▸*) is in that last group too, above the row that
   ends the thing itself, so no row that cannot be undone is anywhere but last. **A group's
   name in purlis's own words wears the label role; a group named by data** (a persona's group
   under *Curate ▸*) **is written as the name is**, in the secondary role and with no capitals.
4. **The danger colour is for what cannot be undone**: a program ended (End, the ways to stop,
   Close a project) or a thing deleted (a workspace, a persona, a vault, a forgotten todo). A
   row that loses nothing wears none, wherever it stands: Restart keeps the conversation,
   Remove folder keeps the branch, a memory's Delete is an archive with an Undo.
5. **An ellipsis means "asks something first", and nothing else** (`docs/ui-copy.md`). A row
   has one exactly when the catalogue's title has. **A row that could only say what a press
   would do is left out**: there is no greyed *Switch to* on the tab in front; on any other tab
   it is the first row.
6. **A shortcut is said at the row's end**, in the meta role, where the same act has a key on
   the thing itself: *Rename… F2*, *End ⌫*, *New shell ⌘⇧T*. A row says the key the window's key
   registry holds for its act (`MenuShortcuts`, #2921) or, until the registry has one, the key
   its caller knows (`keys`). **A toggle is one row with a tick**, not two rows that take turns:
   *Pinned*, a `menuitemcheckbox` of a group that chooses many.
7. **Type and geometry are the tokens'.** A row's words are the body role, the line under them
   and a tooltip the secondary, a shortcut the meta, a group's name the label
   ([Text roles](#text-roles)); a row is `--size-control` tall; a menu's width is the one menu
   style's ([the menu and popover families](#the-menu-and-popover-families)).

**A context menu's rows are still the catalogue's, and only drawn here.** `menuModel.ts` takes
every row `actions.menuRows` lists and decides none of them in or out, so an action cannot be
lost by folding it into a submenu: `menuModel.test.ts` holds that the rows drawn are the rows
listed, over every kind of menu at once, and that none says its thing's name, with the rows
that name another thing listed. A row the model has no words for is drawn by its catalogue
title, where the catalogue put it.

**The keyboard is React Aria's**: the arrows move and stop at the ends, Right opens a submenu,
Left and Escape close it, and typing finds a row by its words (`textValue`), never by its note. A
row's tooltip takes the first Escape (WCAG 1.4.13) and leaves the menu open on its row; the next
Escape closes the menu.

**Not a filter field in a long submenu.** A text field inside a menu fights its type-ahead and
its roving focus, and typing already finds a row. A submenu with more rows than the window has
room for scrolls. A list that needs a filter is a popover laid out as one (R165, below).

**A menu whose rows are not the catalogue's declares them the same way.** A context menu on
something `actions.menuRows` does not know (a terminal's own menu) builds its `GroupDrawn[]`
itself: each row a `RowDrawn` around an `Offer` made for that row, as `actions.fileRows` makes
a file's, with its `said`, `danger`, `warns`, `checked` and `keys`. It draws them with
`Menus.tsx`'s exported `Groups`, inside its own `Menu` named by `aria-label`, so its rows,
tooltips, submenus and keyboard are every other menu's. A dropdown, which is not a context
menu, puts `MenuWords` and `MenuTip` inside its own `MenuItem`s, as a secret's menu and a
setting's *Move to…* do. **The show-more and the away menus are the exception to the width**:
their rows are tabs and sentences with no part that gives way, so `.more-menu` hands
`--menu-max: none`.

## The overlay surface

**What the window draws over itself is one surface** (#1845). A dialog, a popover, a menu and a
tooltip share one ground (`surface.overlay`), one border, one corner (`--radius-lg`) and one lift
(`--elevation-overlay`), so they read as related; a dialog and a popover share one padding
(`--space-6`). The classes are in `app/src/primitives/overlays.css`, after the control families in
the same `components` layer, and the popover and menu families' surface is this one, a select's
list's included.

| Overlay | Class                                         | On                                                         |
| ------- | --------------------------------------------- | ---------------------------------------------------------- |
| Scrim   | `.ui-scrim`                                   | Radix Dialog's or AlertDialog's `Overlay`                  |
| Dialog  | `.ui-dialog`                                  | Radix Dialog's or AlertDialog's `Content`                  |
| Popover | the popover family's surface ([above](#the-menu-and-popover-families)) | `AnchoredPopover`, `HeldPopover`, `PopoverDialog` |

(`--elevation-overlay`), so they read as related. A dialog is [the dialog
family's](#the-dialog-family), drawn on that surface by `app/src/ui/dialog/dialog.css`; the other
classes are in `app/src/primitives/overlays.css`, after the control families in the same
`components` layer, and the menu's surface is this one.

| Overlay | Class                                         | On                                                         |
| ------- | --------------------------------------------- | ---------------------------------------------------------- |
| Dialog  | the dialog family's parts (`data-scope="dialog"`) | `Dialog`, `AlertDialog`, `DialogSurface`               |
| Popover | `.ui-popover`; its arrow `.ui-overlay-arrow`  | Radix Popover's `Content` and `Arrow`                      |
| Tooltip | the tooltip family's `root` part ([the tooltip family](#the-tooltip-family)) | React Aria's `Tooltip`, wrapped in `app/src/ui/tooltip` |
| Menu    | the popover family's surface, its `menu` variant | `Menu`, a context menu's too                      |

- **The look is the class's, the place is the view's.** A view places and sizes a popover or a
  menu with a class of its own: `.row-card` caps a popover, `.more-menu` sets a menu's width. None
  of them draws a ground, a border, a corner, a shadow or a padding. A dialog's place and size are
  the dialog family's alone.
- **Inside, the shared controls.** Every button in a dialog or a popover is a `Button`: a
  dialog's main action is `primary` unless it ends something (D-1719-3, `danger`) or grants
  (`grants`), and the rest are secondary. A field is the field family's, in a dialog's settings
  row too, so a dialog's fields never differ from one another; the dialog's body and footer are
  the field family's `Form`, so its fields are checked before it is sent and ⌘⏎ sends its main
  action.
- **A Notice is not an overlay.** It stands in its pane's row or in the Inbox, never over
  anything (#1647), so it has no lift and no overlay ground. It is the **inline surface** instead
  (#1855): `.ui-inline-surface` draws the overlay's border and the control's corner on the page's
  raised ground (`surface.raised`), with no shadow. `.ui-inline-surface-waiting` edges it in the
  waiting colour (a Notice's box in a pane); `.ui-inline-surface-trouble` is the danger ground (a
  Notice standing alone that reports something wrong). A Notice in the Inbox's list is the list's
  row, not a box of its own. What a way out opened under a Notice (a block's report, a
  dispatch's brief) is the inline surface too.

`app/src/primitives/overlays.test.tsx` holds the surface through the window's cascade (the same
ground, border, corner and lift on each kind, the same padding on a dialog and a popover), that a
dialog is still named by its title, keeps the keyboard and leaves on Escape, and that a popover
opens from the keyboard. It reads every component's source too: each Radix overlay part wears its
class, every button inside an overlay's content is the button family and every field its
field class, and `App.css` draws no overlay surface and lifts nothing by a shadow of its own.
The dialog family's own tests hold a dialog's rows (`ui/dialog/Dialog.test.tsx`).

## Text roles

**A word's rank is a class, as a control's look is** (#1920). The window's words have seven
roles, each a fixed size, weight and colour from the tokens, in
`app/src/primitives/text.css`, the last sheet of the `components` layer. A view gives a word
its role and writes no `font-size`, `font-weight` or `color` for it.

| Role      | Class               | For                                                     | Size                | Weight        | Colour           |
| --------- | ------------------- | ------------------------------------------------------- | ------------------- | ------------- | ---------------- |
| Title     | `ui-text-title`     | a view's or a dialog's title                            | 16px                | strong (600)  | `text.primary`   |
| Heading   | `ui-text-heading`   | a section inside a view; a card's heading               | 14px                | strong (600)  | `text.primary`   |
| Body      | `ui-text-body`      | what the surface is about: a row's name, an ask's words | 13px                | regular (400) | `text.primary`   |
| Secondary | `ui-text-secondary` | where, who, a help line under a field                   | 12px                | regular (400) | `text.secondary` |
| Meta      | `ui-text-meta`      | age, kind, counts, shortcuts: read last                 | 11px                | regular (400) | `text.quiet`     |
| Label     | `ui-text-label`     | a section label over a group; capitals, spaced `0.05em` | 11px                | strong (600)  | `text.secondary` |
| Code      | `ui-text-code`      | a command, a path, a host                               | the role it sits in | regular (400) | inherited        |

The px are at the default 14px text; each size is a core token and scales with the text-size
setting. `ui-text-strong` beside a role makes it the strong weight: **the row's one strong
line is `ui-text-body ui-text-strong`**, not a second body class. `ui-text-code` is the
monospace family alone, so a path under a name is `ui-text-secondary ui-text-code`.

- **Colour follows the role.** Do not pick a colour to make a word quieter; give it the role it
  has. **Muted is the meta's only**: a help line or a where line is secondary.
- **Nothing is under 11px.**
- **A word that says something is wrong is `danger.text`**, and that is the one colour a view
  still writes for words; `state.failed` is for a state's mark.
- **The role is the look, the view places it.** A view's own class still gives a word its grid
  area, its margin, and whether it is cut or wraps (`.inbox-primary`, `.inbox-clamped`). It sets
  no size, weight or colour, and could not: the roles are in the layer above `App.css`, so a
  view rule for the same word loses, at any specificity.
- **A state that changes a word's rank changes its class**, in the component. A rule in
  `App.css` cannot step a role back. Where the stepped-back look is no role at all (a read
  update in the Inbox), the word wears none and one rule built from tokens draws it.
- **A role sets no line height**, so a word keeps the line of what it sits in.

The Inbox's rows are the first surface on the roles: the ask's words are the body, strong; where
it is, the secondary; its kind and its age, the meta (`InboxRow.tsx`).

The Explorer and the Panel's halves are on them too (`Explorer.tsx`, `Panels.tsx`,
`PanelList.tsx`; `Explorer.roles.test.tsx` and `textRoles.testkit.ts` hold each word to its
role). **A name that a state tints takes its role from the root**: a changed file's name is
drawn in its change's colour, which a role on the name would overrule, so the Explorer and a
panel wear `ui-text-body` on their root and every other word wears its own class. A section's
title is `ui-text-label` on the element that holds its words (a folding section's button), a
count is `ui-text-meta ui-text-strong`, and a sentence (`.none`, `.pending`, `.empty`) is
`ui-text-secondary` with no dimming on top.

The Chats list is on them too (`ChatsSection.tsx`, `FinishedTasks.tsx`;
`ChatsSection.roles.test.tsx` holds each word to its role). **A surface whose words are mostly
one role wears it on its root**: the section is `ui-text-body` and a row's card
`ui-text-secondary`, so a mark sized in `em` and a word nothing ranks take that size, and each
ranked word wears its own: a name the body, a line under a row or a finished task the secondary,
a count the meta, a heading over a group the label.

`app/src/primitives/text.test.ts` holds each class to this table through the stylesheet's own
cascade, and `InboxScope.window.test.tsx` that the Inbox's rows are drawn with what they had
before. `app/src/type.guard.test.ts` refuses a size, weight, line height, family or text colour
written any other way, with no allowance (#2319); `design-system.md`
(["Text roles"](design-system.md#text-roles)) has the rules, what a theme may change, where
today's window still differs, and the order the surfaces move in.

## Why Radix, and why not the other two

**Not Material (MUI).** Material is the wrong visual language for a dense terminal-adjacent
window — the reference for this app's design is Zed. Its runtime CSS-in-JS is also the one
thing that would put the render path back on the table: the workspace strip's counts measure
0.022 ms at fifty chats (#133), and Emotion serialises styles per render.

**Not Base UI** (`@base-ui/react`, MUI's unstyled layer), although it is a real candidate:
stable at 1.8.0, actively released, ships no CSS either, and has every primitive the next two
tickets need. It costs more for the same components. Measured on this repo, as the added weight
of the production bundle over `main`:

| primitives imported                                   | Radix     | Base UI   |
| ----------------------------------------------------- | --------- | --------- |
| dialog, radio group, checkbox, collapsible            | +60.6 kB  | +84.2 kB  |
| ...plus menu and popover (the strips and layout work) | +101.6 kB | +165.9 kB |

Gzipped, the six-primitive set is +32.6 kB against +54.7 kB. Both tree-shake — the four-way set
costs less than the six-way one in each — and both add zero CSS. Radix is also 2.3 MB installed
against 20 MB, which is the first priority in `AGENTS.md` rather than a rounding error.

What actually shipped is narrower than either row: dialog, radio group and checkbox, with
`<details>` left as the collapsible because the browser already has one. The window's bundle
goes from 641.27 kB to 700.62 kB — **+59.35 kB, +18.63 kB gzipped** — and `App.css` from
13.28 kB to 15.33 kB, which is the rules that were missing rather than anything the library
brought.

Cold start is already a missed limit: ADR 0026 gives 2 s, and Linux takes 25 s behind a dead
portal (#24, accepted). Neither library is what makes that true, but the smaller one is the one
to add to it.

## What is already converted

_History: the dialogs below moved from Radix's `Dialog` and `AlertDialog` onto [the dialog
family](#the-dialog-family) in #2910, and the menus from Radix's `DropdownMenu` and
`ContextMenu` onto [the menu family](#the-menu-and-popover-families) in #2909; those packages
are no longer in the window (#2906). What follows records why each was drawn as it was; the
rules it states hold, kept by the families._

`StartChat`, `QuitWarning`, `ApprovePlane` and `Palette` — every modal surface the window has.
The terminal panes are xterm.js and are not a candidate.

And the chat strip's **show-more menu** (`@radix-ui/react-dropdown-menu`, ADR 0039),
which is the first surface built under this rule rather than converted to it. Two decisions it
does NOT share with the four dialogs, both because it is a menu and not a question:

- **It is not modal** (`modal={false}`). A modal Radix surface marks everything outside itself
  `aria-hidden`, which is right for a dialog that must be answered and wrong for a menu on a
  strip — the rest of the window stays reachable, to a screen reader and to a scenario spec.
- **A click outside closes it.** The dialogs prevent `onInteractOutside` because a click outside
  would answer a question by accident. A menu has no answer to lose, and every menu on every
  platform closes this way.

It is on all three strips — projects, workspaces and chats — because the operator's ruling of
2026-09-22 was about all of them: a strip that has more than it can draw collapses the rest into
this menu rather than scrolling. What decides how many fit lives in `app/src/fits.ts` and is
arithmetic over one measured width per strip, not a measurement of fifty tabs: every tab takes an
equal share of its strip and is never drawn narrower than a floor, exactly as a browser sizes its
own tabs, so `n` tabs fit in `width` when `width / n >= least`. It replaced an
`IntersectionObserver` over every tab, which is the right question about a strip that scrolls and
a loop on a strip that collapses.

And the **context menus** (`app/src/Menus.tsx`): `@radix-ui/react-context-menu`, on the project
tabs, the workspace tabs, the chat tabs, the panes, the explorer's worktree and clone rows, the
persona rows, the Changes view's repo rows and the activity bars' view icons (#1740: "Move to
panel" or "Move to sidebar", built for the icon as a file row's rows are). Four things about them are decisions and not
details:

- **A menu is a third reader of `app/src/actions.ts`**, after the palette and the bar.
  `actions.menuRows` filters the one catalogue to the item the menu was opened on, and a row the
  catalogue does not have is not in the menu — which is how the strip of chats outside every
  workspace gets a menu with no pin and no delete in it without anything in `Menus.tsx` knowing
  that strip exists. A menu with a list of its own is the defect the tmux frame shipped:
  `frame/tabmenu.py` and its palette grew two answers to "what can I do to this".
- **Not modal, and a click outside closes it**, exactly as the show-more menu above. It is a
  menu, not a question.
- **A row's accessible name is its words alone** (`aria-label`), and its note — what the row
  costs — is its `aria-describedby`. Left to the content, every row would announce as a
  paragraph: _"End Ends the program it runs. There is no undo."_ The thing the menu is about is
  the menu's own name, said once ([Menus](#menus)).
- **A group the catalogue decides the length of is a submenu**, Radix's own `ContextMenu.Sub`:
  "Curate ▸" on a workspace, a persona and the plane root's tab (ADR 0061). `actions.curateRows` picks that subject's
  rows out of the catalogue by id — purlis's own, then one named `ContextMenu.Label` group per
  declaring persona, then each action the core left out as a disabled row with the core's
  sentence under it — and it runs inside the menu's content, which Radix mounts only while the
  menu is open, so a strip's fifty tabs do not pay the scan per render. "Ask ▸" on a chat's tab
  is the same shape: `actions.askRows`, a row per persona.
  "Move to ▸" on a memory's row is the same shape (#1190): `actions.moveRows` picks that
  memory's `memory.move:<key>:<store>` rows, one per store but its own, each named as the
  memory tab's Move names the store, with the tab's audience sentence as the note of a row into
  a store the project publishes.
- **Shift+F10 and the menu key open it on the element that has the keyboard** (purlis#174).
  macOS has no keyboard convention for a context menu and its WebView raises no `contextmenu`
  for either key, so `Menued` dispatches the one a right-click would — and only when the
  trigger itself has focus, never a terminal inside the panes' trigger, whose program may want
  the key. A terminal answers the two keys itself, with its own menu (below).
- **A terminal's menu leads with the terminal's own rows** (`app/src/TerminalMenu.tsx`,
  `app/src/terminalRows.ts`): Copy, Paste and Select all, then under a line the rows the panes'
  menu has there — a new tab, the splits, ending the chat — so a right-click on a pane strands
  none of them. It is a menu of its own inside the panes' (Radix opens the innermost trigger's,
  as on a pane's breadcrumb); the panes' menu is still what a pane's top line and a chat still
  starting open. Its three rows are not catalogue rows: they are about what is selected in one
  terminal when the pointer came down, which the palette cannot name. They are declared as
  [Menus](#menus) says a menu outside the catalogue declares its rows (`terminalRows.terminalGroup`,
  a `GroupDrawn` of `RowDrawn`s), and drawn by `Menus.tsx`'s `Groups` with the pane's own groups
  from `menuOf` after them, so each is a short label, with why it cannot run under it and the
  key that does the same at its end (`MenuKeys`) where the window has one — a Mac's `⌘C`, `⌘V`
  and `⌘A`; off a Mac the window claims no key for them and draws none. The menu is named
  *Terminal* (`aria-label`).
  - **Copy** is off with nothing selected. It puts on the clipboard what the keyboard's copy
    does, xterm's own selection text, and keeps the selection. The selection is read at the
    press: if output cleared it while the menu was up, nothing is written and the pane says so. On a Mac a right-click on a word
    selects it first, as Terminal and iTerm2 do (xterm's `rightClickSelectsWord`).
  - **Paste is the person typing, by the path their keyboard's paste takes.** A menu row gets
    no `paste` event and the page is given no way to read the clipboard, so the core reads it
    on the press (`clipboard_text`, the window's alone, which no link is served; ADR 0052,
    amended 2026-10-14) and the text goes to xterm's own `paste()`, once: bracketed when the
    program asked, so several lines cannot run one by one, and handed on as everything typed
    is. Nothing else is sent. The read takes no argument, happens on the press and at no other
    time, and keeps and writes down nothing. A clipboard holding no text pastes nothing, and
    the pane says so; so it says the core's sentence where the clipboard holds more than
    1 MiB of text, or the read failed. **A menu opened from the keyboard never rests on
    Paste**: the keyboard is on the menu itself until an arrow moves it, so Enter after the
    menu key pastes nothing (with nothing selected Copy is off, and the first row that can
    run is Paste).
  - **No Clear, and no link rows.** xterm's `clear()` empties the screen as well as the
    history, and the history is the core's, sent whole to a pane drawn again. A pane registers
    no link provider, so there is no link the terminal knows it was clicked on.
  - **A click the program asked for is the program's.** While the program has mouse reporting
    on (`?1000h` and its kin) a plain right-click is reported to it, as before, and opens no
    menu, purlis's or the engine's. **Shift+right-click always opens the menu**: the press is
    stopped before xterm sees it, because on a Mac xterm reports a Shift+click too, and its
    default is prevented as xterm's own listener would have, so the keyboard is not taken from
    the terminal by a press xterm never saw. A Mac's Ctrl+click is the same gesture as the
    right button, here as everywhere.
  - **Closed on Escape or on one of its own three rows, the keyboard is the terminal's**, put
    there outright and not left to where the engine thinks it was. A pane's row decides for
    itself (a split's new pane takes it).

And with them, **the WebView's own menu is taken away from the whole window**
(`useNoBrowserMenu`), because a shipped app that answers a right-click with `Reload` and
`Inspect Element` is showing the operator the browser it is built on. Two properties of that one
listener are load-bearing:

- **It is on the BUBBLE phase.** Radix opens its menu from an `onContextMenu` composed with
  `composeEventHandlers`, which skips its own handler when the event is already
  `defaultPrevented`. React 19 attaches its delegated listeners to the root container, which is
  below `window`, so a capturing listener there would run first and silently stop every context
  menu in the app from ever opening.
- **It leaves an `input`, a `textarea` and a contenteditable alone.** That menu is the platform's
  Cut/Copy/Paste and it is the only pointer route to the clipboard a field has (a terminal has
  its own menu's Copy). The inspector is off in a release build anyway.

And the **delete-a-workspace dialog** (`app/src/DeleteWorkspace.tsx`):
`@radix-ui/react-alert-dialog`, the first use of that primitive here. It is `Dialog`'s sibling
for a question whose answer destroys something — `role="alertdialog"`, so what a screen reader
is handed first is the sentence about what is about to be lost; Cancel focused by the primitive;
Escape meaning Cancel. It keeps the four dialogs' rule that a click outside answers nothing.

The **alerts drawer** (M6.5) is gone (#1695): its rows are Notices in each project's Inbox, and
the status line's button that opened it opens the Inbox. A way into Settings from one of them
still takes the keyboard to the Settings tab's current group (`settings/entering.ts`, #1206).

And the **question a relaunch asks** (`app/src/RelaunchAsk.tsx`, purlis#250): an
`AlertDialog`, because it arrives without being asked for. **"Reopen all sessions" is the
primitive's `Cancel`**, first and focused, so Escape and a stray Return both keep the work.
**"Start fresh" is a plain button, not the primitive's `Action`**: an `Action` also closes the
dialog, and closing is this dialog's "Reopen all", so one press would send both answers.

And the **ask before Restart to update** (`app/src/Updates.tsx`, purlis#251): an
`AlertDialog` naming each chat that is mid-turn or reports no state, in the quit warning's rows
and words. **"Wait" is the primitive's `Cancel`**, first and focused; "Restart now" is a plain
button, for the relaunch question's reason.

And the **quit warning** (`app/src/QuitWarning.tsx`, #630): an `AlertDialog` since pass 2 of the
dialogs walk, because its answer ends every chat. Its title counts what it ends, "Quitting ends 3
chats", and its sentences say chats too. Cancel is focused by the dialog itself, and Escape
answers what Cancel answers.

And the **question before a chat ends** (`app/src/EndingChat.tsx`):
`@radix-ui/react-alert-dialog`, the operator's _"closing session should ask confirmation"_. It is
the first surface here that is **not** a `Dialog`, and the reason is the role: an
alert dialog is `role="alertdialog"`, announced as an interruption rather than as a surface, and
the primitive requires a `Cancel` that focus goes to. The four below are questions the operator
went looking for; this one arrives _because of_ something they did, which is the distinction the
role exists for. Two consequences worth knowing before the next one:

- **`AlertDialogContent` takes no `onInteractOutside`.** It refuses outside interaction itself,
  so the rule the four `Dialog`s write out by hand is the primitive here. A reviewer looking for
  the missing line should find this paragraph rather than a hole.
- **It is asked in one place** — `PlaneView`'s `run`, which carries out a catalogue row from
  whichever surface pressed it — so a tab's `×`, a pane's `×` and the palette's rows all ask.
  A confirmation on one surface and not another is the second answer the catalogue exists to
  not have.
- **Three answers since Smart close (ADR 0064): Cancel, Close, Smart close, in that order.**
  Cancel is the primitive's `Cancel`; Close and Smart close are both its `Action`, so either
  closes the dialog. **The focus goes to the default the operator ruled, not always to Cancel**:
  Smart close for a chat with turns behind it, Close for one with at most one, and Cancel
  wherever purlis cannot say. Smart close that is not offered is `disabled`, with the core's
  sentence beside it (`aria-describedby`); Radix's focus scope skips a disabled button, so
  Shift+Tab from Cancel reaches Close. Escape still answers Cancel.

And the **persona card** (`@radix-ui/react-popover`, `app/src/Panels.tsx`): what a row in the
right-hand region's persona list opens. It is the first popover in the window, and it was picked
over the other two surfaces Radix has for the same content:

- **Not a dialog**, because a dialog is modal and modal is wrong here twice. Radix marks
  everything outside an open dialog `aria-hidden`, which then
  included the needs-you queue two sections up (in the title bar since purlis#249) — the
  one surface ADR 0038 says this region must never compete with — and a modal is
  for a question that has to be answered before anything else happens. A persona's role is
  reading.
- **Not a sheet**, because the window had one and it was the window's: the alerts drawer was a
  sheet from the right over every open project (gone since #1695, its rows the Inbox's Notices).
  A second sheet, over one project's region, would have been two drawers with two different rules
  and two different scopes.
- **A popover is anchored to the row it is about**, which is what makes a card legible when five
  of them are listed one under the other.

It takes the show-more menu's two decisions rather than the four dialogs': **not modal**, so the
rest of the window stays reachable to a screen reader and to a scenario spec, and **a click
outside closes it**, because there is no answer to lose. `side="left"` is where it opens from in
the default arrangement and no more than that — a region MOVES (ADR 0038), and Radix flips to the
other side when there is no room, which is what makes naming a side safe at all.

**And then the persona card left the popover for a tab (2026-09-23).** It had grown a searchable
archive of everything a persona remembers and an extension's statistics, which a popover anchored
in a 260 px column is a thin surface for; a non-modal sheet over the centre region was built and
then replaced the same day, when the operator was asked where the card should open and chose
**"Its own tab"**: _"we dont have other tabs then sessions, and this can be good example for us -
that in tabs we can have what we want - not only harnesses"_.

So a persona is a **view**, and a view is what a tab's pane holds when it does not hold a chat
(ADR 0043, as amended; `app/src/tabs.ts`, `app/src/Views.tsx`). No primitive is involved
beyond the tablist the strip already is, and that is the point: the accessibility the sheet had to
argue for — not modal, not hiding the queue, dismissible, focus returned — is a tab's by
construction. A tab is not dismissed by focus leaving it, which is what closed the sheet when the
palette handed the keyboard back to a terminal (#212's adversarial review, finding 5). A memory
followed (SI-9b), and so did a todo (#1214): it opens as a view tab with its whole text, when it
was opened and its actions, and its row opens no card. The popover above is left to a row that
opens nothing else, such as an extension's declared row.

An extension's view — persona statistics — opens in a tab of its own the same way, from a button
on the personas panel's heading, a button on a persona's tab, or its palette row.

And a **Chats row's card** (`@radix-ui/react-popover`, `app/src/ChatsSection.tsx`, #1675):
what a chat's one-line row says when it is rested on, its state in words, what it is doing, its
persona, harness, workspace, time in its state, tasks, own branch, handoffs and a task's tokens.
It is the popover the window already has, used as a tooltip, and not a package of its own:

- **The row holds it open, and anchors it** (`Popover.Anchor`), and there is no `Trigger`. A
  trigger is a press that toggles the card and says `aria-expanded`, and a press on a row opens
  its chat while `aria-expanded` is the row's fold.
- **It comes on a rest, the pointer's or the keyboard's**, after `CARD_DELAY_MS`, so a pointer
  running down the list or the arrows going down the tree put up none. The keyboard's counts
  only where the keyboard was used last: a focus a press gave the row, or a closing menu gave
  back, is no rest. The pointer leaving, the keyboard moving on, a press and Escape take it
  down.
- **It is a tooltip to a screen reader**: `role="tooltip"`, named by the row's
  `aria-describedby` while it is up. It takes no focus as it opens or closes
  (`onOpenAutoFocus`/`onCloseAutoFocus` are prevented), so the keyboard stays on the row, and it
  holds nothing to press: every way to act on a chat is the row's own.

**It is drawn only while it is up**, so what it reads (the clock, the tasks, what the chat is
doing) costs a row nothing until then. Like the persona card, `side="right"` is the default
arrangement's side and no more: Radix flips it where there is no room (ADR 0038).

Two decisions those four share, taken once so they do not have to be taken again per dialog:

- **A click outside answers nothing.** `onInteractOutside` is prevented on all four, which is
  how every surface in this window has always behaved. It is written out rather than left to
  the default so it reads as a decision.
- **Escape answers, with the non-destructive answer.** The picker and the palette always did;
  the quit warning and the trust prompt did not, and a modal with no keyboard way out is the
  one thing a modal must not be. In both, Escape is exactly Cancel — nothing is ended, nothing
  is approved, and the core is told so the next ask is a first ask again.

And the **persona mark picker** (`app/src/PersonaMarkPicker.tsx`, #1449), in a persona's view:
two Radix radio groups, `Icon` and `Colour`, each a row of `RadioGroup.Item`s that wraps. Three
things about it are decisions:

- **The first item of each takes the key out.** `Initials` and `From its name` write no `icon:`
  or `color:`, so the persona goes back to what it inherits along `extends:` or to its default.
  A pick is written to the definition at once; there is no Save to forget.
- **A colour outside the palette is an item too.** A `#rrggbb` written by hand in the definition
  is drawn as one more item, checked, so the group never shows nothing picked for a colour the
  persona has. Any other hue is typed into one native text box under the group, as
  `#rrggbb`, and written at Enter; something else is marked `aria-invalid` and nothing is sent.
- **What the persona asked for and cannot have is a `Notice`** above the groups, one per
  reason, each dismissable: an icon purlis does not have, a colour it does not read, an image
  over its limits or one this window could not decode.

## Two things learned converting them, which will catch the next person

**A radio group's pick does not follow the arrow keys on its own.** Radix selects an item on
focus only while it believes an arrow key is down, and it learns that from a `keydown` listener
on `document`. React attaches its delegated listeners to the root container and to each portal
container, both below `document`, and the roving focus moves on a timer set when the key goes
down, so a single press moves the focus and picks nothing; only a held key picks. The repair
lives in the settings set's `Choice` (`app/src/settings/components.tsx`, DS-3d): it hears the
arrow in the capture phase on the group, picks the option the focus then lands on, and forgets
the arrow when the key comes up or the focus leaves the group. The repair is one hook,
`useArrowPick`, in the same file: every radio drawn with `Choice` has it, and so has
`SettingsLayout`'s level switcher (#626, D-626-4), whose level follows the arrow as a `Choice`
radio's pick does, and so has a side view's scope switch (`ScopeSwitch.tsx`: the Chats view's,
#1679, and the Inbox's), which imports it; a hand-built `RadioGroup` without it does not. Because `SettingsTab` draws each
level with a layout of its own, the switcher an arrow left is unmounted, and the layout drawn at
the level arrowed to takes the focus back (D-626-5). The guards are `components.test.tsx`'s
"picks the next option on a single arrow, once" and its level switcher's tests, and
`StartChat.test.tsx`'s "moves between harnesses with the arrow keys".

**A radio whose pick writes something with no Undo, or starts something, holds the pick and
writes on a button.** Because an arrow picks, and a held arrow repeats, a radio that wrote on
its pick would write every option it passed through on the way to the one meant. Such a radio
keeps the pick in the screen's own state and acts only on an explicit button: Saving's _Use
this_ (DS-3b), Updates' _Use this channel_ (DS-3d), and First task's profile, which starts a chat only
on _Start the first chat_ (DS-3e). A radio whose write can be undone, as
Settings' controls can (V89e), writes on its pick. Settings' move between files, which writes
both and has no Undo (SE-18), is no radio since DS-8: it is the row's _Move to…_ menu, and only
a row chosen in it moves the value. Each held pick is guarded by a test that
holds an arrow down for about 80 ms and finds nothing written.

**The arrow-key defect is the radio group's, not every primitive's — measured, not assumed.**
The menu added under ADR 0039 was expected to need the same repair and does not: its
roving focus is an `onKeyDown` on the content element rather than a `keydown` listener on
`document`, so React's delegated listeners being below `document` never comes into it. The guard
is `Strip.test.tsx`'s "moves between its rows with the arrow keys", which was written to fail and
passed first time. The rule the next primitive inherits is therefore **check, per primitive**:
the question is where Radix listens, and only a primitive that listens on `document` has this.

**A scenario run cannot right-click, on either engine — so a context menu is driven by the
event and not by the pointer.** `element.click({ button: "right" })` is a W3C pointer sequence;
`contextmenu` is a platform default action the engine raises from a native right-click, below
where a synthesised sequence lands. Measured in purlis's own window with a listener on the
element: **0 `contextmenu` events after a WebDriver right-click, 1 after a dispatched
`MouseEvent`** (webkit 605.1.15 on macOS, 2026-09-22), and run 35771806598 was red the same way
on WebKitGTK 605.1.15. `e2e/specs/workspace-lifecycle.e2e.ts` therefore dispatches the event,
and says so where the dispatch is.

**That is a driver limit and not a product one, and the discriminator is a jsdom test.**
`src/Menus.test.tsx`, _"a real contextmenu event, with the suppressor live"_, dispatches one
`MouseEvent` at a real workspace tab of the real `App` with `useNoBrowserMenu` mounted and no
WebDriver anywhere, and purlis's menu opens. Making that suppressor capture-phase — the one way
purlis could swallow the event — turns that test and only that test red. Put the question where
the driver is not, before changing a spec that cannot answer it.

**A modal dialog really is modal, and the tests notice.** Radix marks everything outside the
open dialog `aria-hidden`, so a `getByRole` for anything behind it finds nothing, and a scenario
spec cannot click a tab while a picker is up. That is the app behaving correctly. When a test
breaks on it, the test was reaching for something an operator could not have reached — fix the
test to take a route that exists.

**Tab inside a dialog is the PLATFORM's, except at the two edges — and that is the third time
"check, per primitive" has earned its place.** `FocusScope`'s `handleKeyDown`
(`@radix-ui/react-focus-scope`) intercepts Tab only when the focus is on the first or the last
tabbable of the scope: on the first it acts on **Shift+Tab** and moves the focus to the last
itself, on the last it acts on **Tab** and moves to the first. Anywhere in between it does
nothing at all and the browser's own tab sequence decides.

That matters because **WebKit does not put a `<button>` in the tab sequence at all** unless "tab
to all controls" is turned on, or the button's `tabindex` is written down — and **WebKit is the
engine on both platforms the scenarios run on**: a WKWebView on macOS and WebKitGTK on Linux.
purlis embeds the system WebView, so this is the window's own behaviour rather than one runner's
quirk. The second half of that sentence is the whole of the fix and is the section below; it was
not known when the rest of this was written.

It was measured rather than reasoned about, three times, and the third measurement **corrected
the first two**. `palette.e2e.ts`'s _"closes the chat it just opened, by the keyboard alone"_
pressed Tab to move from `Cancel` to the confirm; the question stayed on screen on `webkit macos`,
and after that was read as a macOS default it did the same on `WebKitGTK linux`. Then the spec was
made to write down every keydown the document sees, and it said (purlis#176):

```
the page saw: Shift on <button> "Cancel"; Tab on <button> "Cancel"; Enter on <button> "Cancel"
```

**Two separate findings live in that one line, and only the first is about the product.**

1. The tab sequence really does skip buttons, as above — and the trace adds that
   `browser.keys(["Shift", "Tab"])` **is not delivered as a chord**: the Tab keydown arrives with
   `event.shiftKey` unset, so Radix's edge handler never fires either.
2. **A focused button is not activated by a synthesised `Enter`.** The engine delivered the
   keydown _to_ `Cancel`, unprevented — the focus was genuine and the engine agreed — and nothing
   happened. WebDriver key actions carry no implicit activation.

The second is a fact about the **test rig**, not about purlis, and it is the one that matters
when writing a spec: **a scenario cannot press a button by keyboard at all, by any key.** That is
why Escape works in these specs where nothing aimed at a button does — Radix listens for Escape on
`document` — and why the palette's own Enter works, since the palette handles it in JavaScript.

**A third face of the rig's finding, and it is the same fact underneath — measured in
purlis#186 while trying to prove the fix below in a real window.** The two above read like
two unrelated quirks; they are one. **This driver dispatches a synthetic DOM keydown and performs
no default action whatsoever.** The decisive measurement is a control nobody can argue with: two
plain text `<input>`s, injected into an open dialog in the running app, with the focus on the
first.

```
PROBE start: input "probe-one"
PROBE after Tab from text box: input "probe-one"
PROBE keys seen: ["Tab shift=false on INPUT prevented=false"]
```

**Tab did not move the focus between two text boxes.** WebKit tabs between text fields with full
keyboard access off — that is ordinary Safari, and `TextFieldInputType::isKeyboardFocusable`
never consults the gate at all — so there is nothing left for this to be but the driver. The
keydown arrived, unprevented, and nothing followed it. That explains all three symptoms at once:
no activation from `Enter`, no focus navigation from `Tab`, and no `shiftKey` on a chord, because
a synthetic dispatch carries none of them.

So, for whoever writes the next spec: **the keyboard half of a scenario can only assert what the
app does in JavaScript.** Escape, `F2`, the palette's own Enter — all handled by a listener — are
fair game. Anything the _engine_ would have done in response to a key is not, and asking for it
produces a red that looks like an app defect and is not one. The reachability half of
purlis#186 therefore stayed in jsdom, where the engine's rule is written down and modelled
explicitly; `picker.e2e.ts` keeps the half a scenario really can prove, which is that the
attribute the rule needs survives the build and is on the element in the shipped app.

Three things follow, and the last is the one a reviewer should hold us to:

- **Shift+Tab from the first answer is Radix's own `focus()` call**, not the engine's tab
  sequence, so it reaches the last answer everywhere a real keyboard is driving. A _scenario_
  cannot use it, for finding 1 above; `App.test.tsx` is where that route is tested.
- **A claim of the form "this button can be pressed by keyboard" belongs in a unit test**, where
  jsdom implements activation. A scenario can prove that the keyboard reaches a surface and that
  keys the app handles in JavaScript do their work — `palette.e2e.ts` keeps exactly that half,
  raising the question by keyboard and answering it with Escape.
- **A control that is neither edge of a modal is reachable by neither mechanism**: the engine
  will not tab to a `<button>` and Radix only handles the edges. That was the state of this
  window until purlis#186, and the section below is what was measured and what was done
  about it.

## What Tab actually reached in each modal, and the one attribute that fixed it

purlis#186 asked the question above of **every** modal surface rather than of the one
dialog a scenario happened to break on, and the answer was worse than the ticket's guess.
`app/src/Modals.keyboard.test.tsx` is the measurement and now the guard; it walks each surface
with a Tab that is dispatched for real, so Radix's edge handling runs rather than being modelled,
and only the engine's half is written down. **This is a jsdom file on purpose**: a scenario
cannot answer the question at all, for finding 2 above.

What it found, before anything was changed:

| surface                            | tabbables | opened on       | Tab reached                  | Shift+Tab reached      | reachable by neither                      |
| ---------------------------------- | --------- | --------------- | ---------------------------- | ---------------------- | ----------------------------------------- |
| `StartChat` (the picker)           | 6         | `Cancel`        | **nothing — it never moved** | the form, then `Start` | the footer checkbox                       |
| `Updates` (the offer)              | 5         | channel         | nothing                      | `Close`                | `Install`, `Check now`                    |
| `NewProject`                       | 5         | the folder box  | nothing                      | nothing                | `Browse…`, the checkbox, `Create project` |
| `Extensions`                       | 4         | `Add…`          | nothing                      | `Done`                 | `Review`, `Remove`, per row               |
| `NewWorkspace`                     | 4         | the name box    | the vision box               | nothing                | `Create workspace`                        |
| `Doctor`                           | 3         | `<summary>`     | nothing                      | `Close`                | `Check again`                             |
| `QuitWarning`, `EndingChat`        | 2         | `Cancel`        | nothing                      | the other answer       | —                                         |
| `ApprovePlane`, `ApproveExtension` | 2         | `Cancel`        | the other answer             | nothing                | —                                         |
| `DeleteWorkspace`                  | 2         | `Cancel`        | nothing                      | the delete             | —                                         |
| `AlertsDrawer`, `PinItem`          | 1         | its one control | —                            | —                      | —                                         |
| `Palette`                          | 1         | its box         | —                            | —                      | —                                         |

The last three rows arrived from purlis#172 while this was being measured, each with the
defect on the day it was written — which is the argument for a file that walks every surface
rather than a fix per dialog. `NewProject` is the sharpest: only its folder box was in the
engine's sequence at all, and the checkbox in the middle is the one that decides whether purlis
writes into a repository the operator already has.

Read the `Tab reached` column first, because it is the one an operator lives in: **in eight of
the fourteen surfaces, pressing Tab moved the focus nowhere at all**, and in a ninth it moved
one step between two text boxes and then stopped. Three of the remaining five have a single
control and nowhere to go by construction. That leaves two — `ApprovePlane` and
`ApproveExtension` — where Tab did what Tab does, and it worked there for no better reason than
that `Cancel` happens to be written second in their JSX rather than first.

**The ticket's headline is half refuted, and the half that survives is the worse half.** It said
the picker's `Start` could not be reached; `Start` could be reached, by walking the dialog
backwards through every form control and out the far side on Radix's first edge — and never by
Tab. What was reachable by neither key was the footer checkbox, ADR 0029's one choice.
ADR 0022 makes this dialog the only way a chat ever starts, so "you may start a chat, but only
by pressing Shift+Tab five times" was the keyboard-only path to starting one.

**Also refuted: "radio groups and checkboxes are in WebKit's restricted tab sequence."** They
are when they are `<input>`s. Radix's are not — `RadioGroup.Item` and `Checkbox.Root` both
render `Primitive.button`, and the hidden `<input>` each keeps for form submission is
`tabIndex={-1}`. The radio rows were reachable for an unrelated reason: roving focus writes a
`tabindex` on them. The checkbox has no roving focus and was not.

**The fix is one attribute, and it is the engine's own escape hatch rather than a handler of
ours.** WebKit's rule, in full:

```cpp
bool HTMLFormControlElement::isKeyboardFocusable(const FocusEventData& focusEventData) const
{
    if (!!tabIndexSetExplicitly())
        return Element::isKeyboardFocusable(focusEventData);
    return isFocusable() && document().frame()
        && document().frame()->eventHandler().tabsToAllFormControls(focusEventData);
}
```

**A `tabindex` that is written down is never weighed against full keyboard access at all.** The
two lines that say so are WebKit's own, added in `[popover] Improve focus handling`
(r263447, 2023-04-29) and shipped in Safari 17 and WebKitGTK 2.42 — both older than anything
purlis runs on. So every `<button>` inside a modal surface in this window now says
`tabIndex={0}`, and Tab moves through these dialogs the way it moves through every other window
on the machine.

Three things about that choice, because each was a fork:

- **It is not a hand-written Tab handler, which ADR 0037 is against and which this
  would have been the fourth of.** Nothing wraps a primitive, nothing intercepts a key, and
  Radix's own edge behaviour is untouched and still does the wrapping at the ends.
- **It is not a WebView setting, and that route does not exist.** Turning "tab to all controls"
  on for purlis's own window would be the tidier answer and there is no public API for it:
  macOS exposes full keyboard access as a system preference and `WKPreferences` has only
  private SPI for the web half of it. WebKitGTK's `enable-tabs-to-links` is about links.
- **It goes on the two-answer dialogs as well**, which did not need it. "The keyboard works
  here" was a property of _how many buttons there are_, and a dialog that grows a third control
  should not silently lose it — which is the failure this whole section is the record of.

One consequence worth knowing before the next one: the same WebKit change makes an explicit
`tabindex` mouse-focusable too, so a click now leaves the keyboard on the button it pressed,
as it does on Windows and Linux and unlike the macOS default.

**Outside the modals, this is still true and is not fixed.** Every `<button>` in the window —
the tabs, the pane controls, the status line, the explorer rows — is equally absent from
WebKit's tab sequence, and nothing there has a focus scope to wrap at the edges, so there is no
"reachable backwards" to fall back on. That is a bigger change than a dialog's answer row and a
different question (where should Tab go between four regions?), so #186 stopped at the modals,
where a focus scope makes the boundary obvious and the surfaces are countable. purlis#189
carries the rest.

## Where Tab goes in the rest of the window (purlis#189)

The attribute above fixed the dialogs. Outside them it is the floor and not the answer: a
`tabIndex={0}` on every button would put fifty chat tabs between the explorer and the terminal.
So the window takes the WAI-ARIA Authoring Practices' shape, with the primitive this repo already
had in its tree under the radio groups and the menus, **`@radix-ui/react-roving-focus`**. It is
the package Radix's own `Tabs`, `RadioGroup`, `Menu` and `Toolbar` are built on rather than one of
the primitives it documents, and it is taken directly because `Tabs` would bring a `Tabs.Content`
model and a select-on-`mousedown` this window's strips do not have:

- **Each strip is ONE Tab stop** — projects, workspaces, chats. The selected tab says
  `tabindex="0"` and every other `-1`; Left, Right, Home and End move along it; Enter or Space
  selects, which is the button's own click. Moving is not selecting ("Tabs", manual activation):
  arrowing past a chat must not swap the panes under the operator. A strip's own `+` sits
  inside it, right after the last tab (DS-7, #1839), and is a Tab stop of its own after the
  tabs': the arrows pass over it.
- **Each list is ONE Tab stop** — the explorer, every panel's rows (`PanelList`, so a
  contributed panel gets it for nothing). Up, Down, Home and End move.
- **A list behind a button is a menu** — the title bar's needs-you list (purlis#249), the
  strips' show-more menus. The button is the Tab stop; Enter opens the menu on its first item,
  the arrows move, Escape closes it and puts the keyboard back on the button. Radix's
  `DropdownMenu` does all of that, so nothing here is hand-written.
- **The explorer is a tree** (purlis#238), the whole "Tree View" pattern on top of the
  same roving focus: `role="tree"`, each row a `treeitem` with its level and its place among
  its siblings, `aria-expanded` on every parent (a clone says whether it is open, a parent that
  cannot fold says `true`), Right to open or go in,
  Left to close or climb, and a typed letter to the next row it starts. `Explorer.tsx` says
  why the levels are written down and not left to the DOM. A tree's rows are `treeitem`s and
  no longer `button`s to a role query, so a test reaches them by that role.
- **The Changes view is a tree too** (#1701), an editor's source-control view: each repo a
  heading row (the repo and its branch) and, one level in, its changes, its branches (each
  branch one level further), its pipeline and a row per extension column. One Tab stop, the
  same keys, and nothing folds, since a fold would be a press in a view that is only read. Its
  rows are `treeitem`s that take the keyboard and run nothing; the repo's context menu opens
  from a focused row on Shift+F10, as on any menued row.
- **On the plane root's tab, Files is the whole project's** (issue 1916): the same `.ui-tree`
  and rows as a branch's files, read through the window's own `plane_tree`. A workspace's repo
  is one `.ui-tree-row` with the workspace's mark (`Folders`), its folder's name and
  _workspace <name>_ in `ui-text-secondary`, and pressing it selects that workspace's tab; it has no
  twist, because it never opens there. These rows have no menu and are not dragged.
- **Explorer is two sections, and each tree is its own Tab stop** (#1677): _Repos and
  branches_ and _Files_, in that order, each under a heading that is a disclosure button
  (`aria-expanded`, `aria-controls`) and folds it — a button and not a `<details>`, since a
  `<summary>` cannot hold the `h2` the section's title is. Tab goes heading, tree, heading, tree.
  A folded section is `hidden` and stays mounted, so its stop is taken out of the order and its
  folds come back with it. **It lists no workspaces**: the strip is the one place a workspace is
  selected (ADR 0036), with its live mark and its menu on its tab, and _Files_ is of the
  workspace selected there, named in its heading. On the plane root's tab the heading says
  _Plane root_ and the tree is the whole project's files (the point above). A view's key gives
  the keyboard to the first tree drawn in the view, past the headings.
- **Every other control says `tabIndex={0}`**, and a tab's `×` says `-1`: fifty closers would be
  fifty stops again. **Delete on a focused project or chat tab presses the row its `×` presses**
  (purlis#239, `closeOnDelete` in `app/src/tabKeys.ts`), and so does Backspace on a Mac,
  whose key marked "delete" sends it: ending a chat still asks first, a view tab still closes
  without asking, closing a project with chats open now asks first too (`ClosingProject`, on
  the row's verb, so the `×`, the menu and the palette ask it as well), and the keyboard lands
  back on the tab after a Cancel and on the strip's stop after a close. A new strip with a `×`
  (a vault tab's) calls it from its tab's `onKeyDown` with that `×`'s row. The palette and the
  tab's own menu reach the same row.
- **The order is the document's, which is the order the window is drawn in**: the title bar,
  the workspace strip, then the regions as the arrangement places them, with the chat strip at
  the top of the editor, before the panes (ADR 0036 as amended 2026-10-12) (ADR 0038 — a region moves,
  and its stops move with it; a fixed order would contradict the layout on screen), each
  resize handle between them (`react-resizable-panels` makes a separator a keyboard control),
  the focused pane's controls before its terminal, and the status line last.
  `Window.keyboard.test.tsx` pins the whole sequence.

`useTabStop` (`app/src/roving.ts`) is the one decision the primitive leaves open: which item is
the stop before anyone has touched the group. Left alone it is none of them and the group's
own element takes the stop. It is the selected item at rest, the item the keyboard is on while
it is inside, and the selected one again when the keyboard leaves. A strip's own controls
(`.strip-doing`, the `+`) are inside the group but are not its items, so reaching one is leaving.
**A new strip or list adopts it** by wrapping its container in `RovingFocusGroup.Root asChild` with the hook's props, and each
item in `RovingFocusGroup.Item asChild tabStopId={…} active={…}`. The primitive stays visible at
the call site, which is this file's rule.

**Two things about wiring it that will catch the next person.** `useTabStop` has to be told
every item that is DRAWN — a tab the strip collapsed into its menu, or a row inside a folded
clone, is never the stop, or the group has none. And a conditional that puts the `Root` around
an element in one branch and not the other makes React remount that element when the branch
changes: the explorer's empty `nav` is inside the same `Root` as its full one for that reason,
and `FourRegions.test.tsx` found it by holding a `nav` that had been replaced.

**A terminal keeps Tab.** xterm prevents Tab and Shift+Tab and sends them to the shell —
completion, and Claude Code's mode cycling — and nothing in the window takes either. So a
terminal is a stop you can reach and cannot Tab out of, which is why the pane's own controls are
written _before_ it in the document (they are absolutely positioned, so the look is unchanged).
**They are shown on hover and while the keyboard is on them, and at no other time** — not
because their pane is focused, which is the pane the operator is typing in and a corner he
asked to keep clear. A `visibility: hidden` control is out of the sequence, so the focused
pane's are hidden by `opacity` with `pointer-events: none` instead: Tab still lands on them, and
`.pane-doing:focus-within` draws the group the moment it does.

**The way out is Ctrl+Tab, and Ctrl+Shift+Tab backwards** (`SessionPane.leavesTheChat`). It is
the platforms' own key for leaving a control that keeps Tab — GTK's text view, Windows'
multi-line box, AppKit's field editor — and by the rule in the next section it takes nothing
from the chat: xterm.js 6.0.0 ignores Ctrl on key code 9, so it would have sent `\t` and
`ESC [ Z`, which are Tab's and Shift+Tab's own bytes. WebKit has no default action for
Ctrl+Tab, so the window walks the engine's sequence itself (`app/src/tabSequence.ts`, the rule
above written down once and shared with the tests). The palette (`F2`, `⌘K`) remains the other
way to act from inside a chat without leaving it.

**What only a person can confirm**, because a scenario cannot press Tab (above): that WebKit
walks this sequence in the real window with full keyboard access off; that `Ctrl+Tab` reaches
the page on macOS and on GTK rather than being taken by the window system first; and that the
focus ring is visible on each stop in both themes. `e2e/specs/keyboard-reach.e2e.ts` checks
what can be checked in the shipped bundle — the attributes, the order, the focused pane's
controls staying unseen until the keyboard is on them, and a dispatched Ctrl+Tab leaving xterm's real textarea.

## The three strips say their depth in shade, and that took no primitive either

ADR 0036 makes the window an axis — a project holds workspaces, a workspace holds chats —
and purlis#171 drew that with three signals so the nesting would be legible before a word
was read: **height** (a project's row is the tallest), **inset** (each row began under its
parent's first tab) and **surface** (deep, raised, then the bar). purlis#193 keeps
**height**, drops the **inset**, and turns **surface** into a quiet shade per strip in tokens of
its own — and drops the accent edge under the selected tab too, on the operator's reading of
the running app.

**The inset went because it could not be told apart from padding.** His words, unprompted:
_"workspaces tabs and sessions tabs have some padding from left, they should be like project
tabs without padding."_ That is the signal failing at the only test that matters — the person
it was drawn for read it as slop. It was not free either: 1.1rem off the workspace strip and
2.2rem off the chat strip is a tab's worth of room at ADR 0026's widths, taken from the two
strips that collapse first (`fits.ts`).

**What replaces it is a shade, and it is a theme token rather than a look.** The operator asked
for the layers to differ — _"lets make some different styles/collor for each layer"_ — and then
turned down coloured rules under each strip, the first answer to that, as not minimal enough:
_"i prefer to change little bit backgrounds of tabs and little lighter for selected tab, borders
are not feeling well."_ So each strip is one quiet step of neutral grey — `layer.project`,
`layer.workspace`, `layer.chat`, outermost deepest — and nothing is drawn under or between the
rows. Three properties of that answer are decisions rather than details:

- **It is a token, so a theme owns it.** `docs/design-system.md` has the group and the contrast
  floor each shade is held to in both built-in themes.
- **The selected tab is a step lighter, and that is the whole selection signal.** #171 also lit
  the selected tab's bottom edge in `accent.base`; it went with the coloured rules, on the same
  instruction. `layer.selected` is one token for all three strips, so the tab you are on reads
  the same way on every row.
- **No primitive, and no component.** This is four declarations in `App.css` and four values per
  theme file. A "strip" component parameterised by depth would be exactly the purlis API in
  front of nothing that the rule at the top of this file refuses.

**And a tab's label is centred in its cell** — _"also lets make tabs labels center aligned"_ —
which is one `justify-content` on the rule that already says what a tab is. A chat tab's `×` is
outside the button, so a chat's name is centred in the room the `×` leaves rather than in the
whole cell: the same rule, not an exception to it.

**Since 2026-10-12 the chat strip is the editor's tab bar** (ADR 0036 as amended, #1836): it is
drawn at the top of the centre column and begins at that column's edge rather than the window's.
It still adds no inset of its own, and its shade is still `layer.chat`.

`src/StripMarks.test.tsx` is the guard on both halves, and it is a stylesheet test on purpose —
jsdom computes no layout and no cascade, so what can be held there is the rule as written: each
strip names its own shade and draws no line, the three shades are three different tokens, no
strip's tab carries an edge, and there is no `--nested` left to indent anything with.

## A strip's tablist owns its tabs and nothing else (purlis#1204)

A `tablist` may own `tab` elements only, and what it owns is everything under it in the DOM plus
everything its `aria-owns` names: **`aria-owns` adds to a tablist's children and never takes them
away.** A strip's cells hold more than a tab (a project's `×` and gear, a workspace's gear, a
chat's `×`, fresh mark and task chip, every strip's own `+`), so the `tablist` role is not on the
element that holds them.

- **The strip element** (`.projects`, `.workspaces-strip`, `.tabs`) has no role. It is marked
  `data-strip="Projects" | "Workspaces" | "Tabs"`, and keeps its cells, its keyboard (React Aria's
  on the chat strip, `TabStripRow`; a `RovingFocusGroup.Root` on the others, whose arrows, Home and
  End read the order from the document, not from a role), its `DndContext` and the ref `useRoom`
  measures.
- **The tablist** is an empty `<span role="tablist" aria-label=…>` first inside it
  (`TabStripRow` on the chat strip, `StripTablist.tsx` on the others), owning the tabs drawn by
  id, in order. The stylesheet lays it over the strip with `pointer-events: none`, so it takes no
  room and no press.
- **Reaching a strip's tabs**: code, the stylesheet and the scenario specs go through
  `[data-strip="…"]`; a vitest finds the tablist by its role and name and takes the strip around
  it with `stripNamed` (`test-strips.ts`), which also holds the tablist to owning exactly the tabs
  the strip draws. `Strips.a11y.test.tsx` writes out axe's `aria-required-children` and holds all
  three strips to it.

How WebKit reads `aria-owns` under VoiceOver is checked on real hardware, not here.

## The four regions added no primitive, which is the rule working

ADR 0038 split the window into four regions, and the whole layout came out of what was
already here — a fact worth recording, because "a layout change" is the sort of ticket a
component library gets added on.

- **`react-resizable-panels` draws all four**, as this file already said it would. Every
  boundary is the same `Separator` the pane splits use, so a region's handle and a split's
  handle behave the same way and are styled once.
- **Putting a region away is the library's `collapse()`/`expand()`, not a conditional
  `<Panel>`.** Taking a panel out of a live group throws from a document listener where no
  `try` can reach it — _"Panel constraints not found for index 3"_ — because a separator
  recalculates its aria values against a constraint list the panel has just left.
  `RegionFrame`'s `Slot` holds the reasoning. What the constraints are is written once and
  never changed; only the collapse moves.
- **The layout is data, and the panels are slots** (`app/src/regions.ts`). The frame renders the
  same panels for the life of a window — a column of two, the row of left, centre and right
  above the **Panel** (#1737), which runs under all three; the old bottom slot, under the
  centre alone, went with the bottom region in #1676 — and an _arrangement_
  says which slot each region's content goes in, in what order, whether it is drawn and how big
  its slot starts. That is what makes the rule above survivable: a region can move side while
  the window is up, because moving it adds nothing to the group and takes nothing out. Adding a
  region is a line in the catalogue and a piece of content; no JSX moves. A slot's
  `minSize`/`maxSize` therefore belong to the **slot** and not to the regions in it — a bound
  derived from the current occupants would change the moment one moved, which is the second half
  of the same throw.
- **A slot that starts with nothing in it starts at `0%`, and that is how the flash was fixed.**
  purlis#141 sized a hidden region normally and collapsed it from a `useEffect`, which runs
  after the browser has painted, so every launch drew it for one frame. The obvious repair —
  `useLayoutEffect` — **throws**, _"Group &lt;id&gt; not found"_: the group registers itself in
  its own layout effect, and React runs a child's layout effects before its parent's, so there
  is no hook inside a group that runs after the group exists. The first layout is made right
  instead; the library snaps `0%` to `collapsedSize`, and the effect is left to handle only what
  changes while the window is up.
- **jsdom never lays a group out.** Every element measures zero, so the library defers its
  layout and no `defaultSize` is ever applied — which means a unit test cannot read a panel's
  width out of the DOM. `RegionFrame.sizes.test.tsx` mocks the library to assert what it was
  _told_; `RegionFrame.test.tsx` and `FourRegions.test.tsx` use the real one for everything that
  is about the tree. A size is only really checked by the scenario tests.
- **The explorer's repo groups are `<details>`**, per the rule above: the browser has a
  collapsible, and `@radix-ui/react-collapsible` is not installed because nothing needs it.
- **Picking a spot in the explorer is `aria-current`, not `aria-selected`.** `aria-selected`
  belongs to the three tablists that are the axis (ADR 0036), and to the one tree that really
  selects, a file tab's, whose selected file is what the tab previews; a spot is the current
  item of a list. Radix has no tree or listbox primitive, and native buttons are not hand-rolled markup.
- **Every tree is drawn by the shared tree row** (#1672, DS-12): `.ui-tree` on its
  `role="tree"` element and `.ui-row.ui-tree-row` on each row, which draw its levels, its
  straight guides, the selected row (`aria-current` or `aria-selected`, in `list.selected` with
  an edge) and the keyboard's ring. A class, not a component, so this rule needs no amendment; a
  new tree draws nothing of its own.
  `docs/design-system.md` has the tokens and why.
- **No region has a toggle button, on the status line or anywhere** (qw357, 2026-10-13). They
  were `aria-pressed` icons on the status line from purlis #193 until the operator called them
  redundant: _"bottom toggle icons are redundant now … they can be removed fully"_. A side is
  put away and brought back from its activity bar (below), by its key (⌘B, ⌥⌘B) and by the
  palette's row, and nothing else draws that state.
  - **A scenario puts a side away as a person does**: `app/e2e/sides.ts` presses the open view
    on the side's bar and reads "away" as a bar with no open view. `regions.e2e.ts`,
    `pane-fill.e2e.ts` and `status-line.e2e.ts` use it.
  - **The status line still cannot be put away.** `FourRegions.test.tsx`'s _"cannot be put
    away, because it is not a region"_ is the guard.
- **The status line is one bar of status items, and added no primitive** (qw357). The
  operator's _"notices and doctor buttons in bottom are not better UI integrated in bottom
  bar"_ was three hand-drawn boxed buttons (`.status-alerts`, `.status-doctor`, `.status-pin`).
  Each is the family's small ghost button now (`<Button variant="ghost" size="sm">`): flat,
  the chrome's height, the set's hover and focus ring, with an icon, a short word and a count.
  - **Context on the left, health on the right.** The project and the workspace with its
    counts; then Notices, the doctor and a drifting pin. The left end gives way in a narrow
    window (`.status-context`), the right never does (`.status-health`).
  - **The line's padding is the ring's room**: `--space-2`, which is `--focus-ring-width` and
    `--focus-ring-offset` together, so the set's ring is drawn whole at the window's bottom
    edge. The line is `--size-chrome` and that room tall.
  - **Severity is a colour and a word.** The doctor's count is in `state.failed` or
    `state.waiting` and reads _"2 blockers"_, _"10 warnings"_; a clean doctor draws no count.
    The Notices count is a number in the neutral count pill beside a bell in `state.waiting`.
    Each item's accessible name says its count (_"Notices: 6"_, _"Doctor: 10 warnings"_).
  - **The project is its name, and its directory is the window's tooltip** (the shared one,
    #2908): words that press nothing, so a focusable span and no button, as a table's
    shortened heading is. The tooltip opens under the pointer and on keyboard focus. A long
    name is cut with an ellipsis. The item carries the directory as `data-directory`, which is
    how a scenario learns which project is open.
- **A side with views has an activity bar** (ADR 0038 as amended 2026-10-10, #1673;
  `app/src/ActivityBar.tsx`). It is VS Code's strip of icons at the window's edge, and it is
  built from what this file already allows:
  - **A vertical tab list on the strips' own roving focus**: `role="tablist"` with
    `aria-orientation="vertical"`, a `button role="tab"` per view, `aria-selected` on the open
    one, `aria-controls` naming its `tabpanel`, and one Tab stop through `useTabStop`. The
    arrows, Home and End move along it; Enter, Space or a click presses. Activation is manual,
    so walking the bar opens nothing. **The tab list owns tabs and nothing else** (#1204): a
    view's badge is inside its own tab, and the tab is described by it.
  - **`aria-selected` is right here, as it is not in the explorer**: the bar is a tab list,
    and its tabs select what the side shows. The explorer keeps `aria-current`.
  - **Icon-only, with the words on `aria-label` and the press in its tooltip**: the tab is named for the view (`Chats`, `Explorer`) and its tooltip says what
    pressing does, with the key (_"Show the Explorer view (⌘⇧E)"_, or _"Put the Navigation
    region away"_ on the open one). Its mark is what the view is, never which edge it is on.
  - **A press of the open view puts the side away**; any other opens. The bar is outside the
    resizable group, so a side put away keeps its bar and its badges, and nothing is added to
    or taken from the group (the throw above).
  - **Putting a side away hides its views; it never unmounts them.** Each view is a `tabpanel`
    kept mounted with `hidden` when it is not open, which takes it out of the tab sequence and
    the accessibility tree as unmounting did, and keeps its folds, scroll and filter. A region
    without views is still unmounted when it is put away.
  - **A badge is a count drawn only when it is not zero**, in the needs-you colours for the
    chats that need you and in the plain count's for the open todos (`ActivityCount`).
  - **An extension's panel is a tab like purlis's own** (#1678): named and marked as its panel
    declares, after purlis's views on the right side's bar, sorted among themselves by `order`.

## A project behind the one in front stays drawn, off the window

Switching projects used to build the whole view of the project it showed: about 800 components
mounted and 15 more for each chat, some fifty commands sent to the core by what mounted, and the
chat's terminal closed and a new one started. A person felt it as a freeze. `projectShown.tsx`
keeps the view instead. Each project draws into an element of its own, and only the project in
front's is in the window; the others are in no document, so nothing lays them out, no query
finds a second tab strip or a second copy of an id, and no key or press reaches them. The four
projects the person was in last are kept (`KEPT_DRAWN`); one left longer ago draws nothing and
is built when they come back to it. `projectSwitch.cost.test.tsx` holds what a switch may cost,
in counts, and the doctor's dialog lists what each switch in the window took (`switchClock.ts`).

A part of a project's view is therefore mounted while nobody can see it. What that asks of it:

- **It is not drawn again while its project is behind** (`AsLastShown`), and takes its new props
  as the project comes back. Anything that must happen while a project is behind belongs in
  `PlaneView` or the window, as the chats' states and the needs-you counts already do.
- **A read made because time passed, the window was focused or the project changed on disk** is
  not made for a project behind, and is made once as it comes back, after it is painted:
  `useWhileShown`, `useWindowFocused` and `usePlaneChanged` do this themselves. What the view
  held is what it shows meanwhile. A part with another reason to read again, or that reads only
  as it mounts, uses `useOnShownAgain`, or it reads once for as long as its project is kept; one
  that reads on a timer of its own asks `useProjectShown()` before it does.
  **Never in the switch itself**: a part there is one of many (a pane has a gauge, a tab forty
  panes), and a read as its project comes back is that many commands ahead of the paint.
  `projectSwitch.cost.test.tsx` holds the count before the paint, and that it does not grow
  with the panes.
- **A measurement** taken while the element is in no document measures nothing: `fits.ts` and
  `wholeWays.ts` keep what they last measured, and measure again as the view comes back.
- **A split that resizes** uses `Separator` from `shownSeparator.tsx`, not the library's own:
  `react-resizable-panels` asks about every handle on every press on the document, and throws
  for one that is in no document. The handle is disabled while its project is behind, which the
  library changes in place; disabling the `Group` registers it again and lays it out again.
- **What is drawn over the whole window**, a dialog, a menu, a popover, a select's list or a
  tooltip, is drawn through a portal outside the project's element, so one of a project behind
  would stand over the project in front: a delete asked in one, answered over another. And it
  need not be open as the project leaves: a part still sets its own state while its project is
  behind, so the answer to something it asked the core can open its confirm then. So **every
  overlay is a family's**, on React Aria (#2906): the surface a popover, a menu or a select's list
  stands on, a dialog and a tooltip each ask `useProjectShown()` and draw nothing while their
  project is behind. Nothing of a project behind is ever mounted over the window, so nothing a
  dialog does to the document as it mounts happens either, and what opened it is there when
  the person is back. `projectShown.guard.test.tsx` fails for a Radix package that draws over the
  window and for a new `createPortal`. Behind that
  stands one backstop: a project that leaves the front with anything still over the window once
  the press is over (`drawnOverTheWindow`) is not kept.
- **A prop that says the project is in front** is the last one the part was drawn with while
  its project is behind. A part that acts on one outside its own drawing, such as a key held on
  the window, reads `useProjectShown()` for itself, as `RegionFrame` does for F6.
- **A terminal** keeps its view of the session open and goes on taking output, is fitted only
  while its pane is in the window, and takes the keyboard as its project comes back.
- **What was scrolled** comes back scrolled where it was: an element taken out of the document
  forgets, so `InTheWindow` writes each element's place down as it is scrolled and puts it back.

## The title bar added no primitive either, and inherited a rule from Tauri

The window's title bar (`app/src/TitleBar.tsx`) is a `<header>` holding the project strip — the
same Radix roving-focus tablist it was in its own row (ADR 0054) — and a `<span>` of buttons at
its right-hand end. The one thing about it that is not ordinary markup is the drag region, and that is a
**third-party rule the window now depends on** — the same class of thing as WebKit's tab
sequence above, so it is written down here for the same reason.

- **`titleBarStyle: "Overlay"` is macOS only.** Windows and WebKitGTK ignore the key in
  `tauri.conf.json` and keep drawing their own title bar above the webview. So purlis's bar is
  the title bar on one platform and the window's first row on the other two, and the only thing
  that differs in the markup is how much leading padding it reserves for the system's window
  controls. That number comes from `title_bar_room`, which is `cfg!(target_os = "macos")` in
  Rust — the target the binary was built for, rather than a `navigator.userAgent` string's guess
  at it, and it is a command because one frontend bundle is built per target and nothing in the
  bundle is told which target it landed in.
- **`data-tauri-drag-region="deep"`, never the bare attribute.** Tauri injects a `mousedown`
  listener (`tauri/src/window/scripts/drag.js`) that walks the composed path up from what was
  pressed. The bare attribute drags only on a **direct** press of the element carrying it —
  which, on a bar whose content is text in spans, means the bar drags everywhere except on its
  own words. `deep` drags anywhere in the subtree.
- **A control inside a drag region needs nothing to stay a control.** That same walk returns
  `false` at the first _clickable_ element it meets, and clickable there means a `<button>`, an
  `<a>`, an `<input>`, a `contenteditable`, an interactive `role` — **or anything carrying a
  `tabindex` other than `-1`**. **`BUTTON` is in that tag list, so the tag alone does it** and no
  attribute of purlis's is load-bearing for dragging. Worth writing down because it is easy to
  get backwards: the title bar's scenario was first written asserting `tabindex="0"` on every
  control and the real app refuted it — the update item's trigger carries none, for
  purlis#189's reason, and it presses perfectly well. Nothing in purlis's own code says
  "do not drag here".
- **`core:window:allow-start-dragging` is NOT in `core:default`.** The handler ends in
  `invoke('plugin:window|start_dragging')`, and without that permission named in
  `capabilities/default.json` the call is refused at runtime — a title bar that looks right and
  cannot be grabbed, with nothing on screen to say why. `internal_toggle_maximize`, the
  double-click, _is_ in the default set, so the double-click worked and the drag did not.
- **No test proves the window moves.** The rig performs no default action (above), and a
  synthetic `mousedown` that reached Tauri's listener would end in an IPC call rather than an
  observable drag. What is measured is the DOM the handler reads — the attribute on the bar, and
  every control in it being an element the walk stops at — in jsdom (`TitleBar.test.tsx`) and
  again in the shipped bundle (`e2e/specs/title-bar.e2e.ts`), which is `picker.e2e.ts`'s split
  applied to a second attribute. The permission is checked by nothing and is the sharpest thing
  to attack.

## Which keys belong to the chat, and which to the window

**Every key is declared in one registry** (`app/src/keys/registry.ts`, #2921, spec #2897 N24).
Each act has one entry: what it does, the group the cheat sheet lists it under, where it applies
(the window, the palette, ⌘P's files, a dialog, a menu, a focused tab or row, a chat's terminal,
an editor, a form), its chords on a Mac and everywhere else, and what it yields to:

- a **modal** dialog that is up (the dialog family's root or an alert dialog; the operator's
  ruling of 2026-10-24);
- any **dialog** that has the keyboard, a popover's included;
- the **palette** while it is up;
- a chat's **terminal**: always, only for its Ctrl chords, or only off a Mac;
- a **tab** that renames on `F2`, an **editor** (CodeMirror's `Mod-/`; and, for the palette,
  only the chords its keymap binds: ⌘⇧K and a Mac's Ctrl+K, `editorKeeps`), or a **field** the
  key would be typed into when its row cannot run.

A handler asks `pressed(event, ids)` which of its acts a keystroke is; the registry applies the
context rules, so one rule reads the same wherever its handler is. How a key is recognised stays
with its reasons, in `sideKeys.ts`, `taskKeys.ts`, `workspaceSwitch.ts` and the rest. Off a Mac
Ctrl takes ⌘'s place, except where a chat would lose the chord (Ctrl+Shift+P, Ctrl+Shift+F); a
test holds that. **`registry.test.ts` fails on a conflict**: two acts on one chord in places that
overlap, neither yielding to the other. That is checked on the declared chords and on each
act's own matcher fed every other act's chords. A new key goes into the registry first, and the
test says whether it is free.

The palette's rows, every menu's rows (`MenuShortcuts`, provided in `main.tsx`) and the activity
items' tooltips show their key from the registry, through the list, menu and tooltip families'
`shortcut` parts. The cheat sheet (`keys/ShortcutsView.tsx`, ⌘/ or the *Keyboard shortcuts* row)
is generated from it. Focus mode (⌘⇧↩, `keys/focusMode.ts`) is one of its keys.

Every pane in this window is a terminal running somebody's shell, and a shell's line editor has
bindings of its own. So "the window takes this key" is never free: it is taken out of the
program the operator is typing into. `F2` cost that once (purlis#47) and `Ctrl-K` cost it
again (purlis#106), which is enough to write the rule down rather than decide it a third
time per key.

> **A chord a terminal encodes belongs to the chat whenever a chat has the keyboard, unless the
> window claims it there deliberately — and a key claimed from under a focused chat must have a
> way to hand it back.**

Three keys, three answers, one rule. It is `Palette.opensIt` and `Palette.theChatKeepsIt` that
implement it, in that order: the first says whether this is the palette's chord at all, the
second whether the chat keeps it anyway.

- **`F2` is claimed from under the chat, and hands it back.** A second `F2` closes the palette
  and sends `ESC O Q` to the chat in front, through the catalogue's own `pane.sendkey` row.
  That is tmux's `send-prefix`, and it is the only reason claiming the key is defensible.
- **`⌘K` is claimed everywhere, and takes nothing.** Measured in the pinned `@xterm/xterm`
  6.0.0: `evaluateKeyboardEvent` answers a `⌘`-chord with `SELECT_ALL` for `⌘A` and with no
  bytes at all for anything else, so a pane that received `⌘K` would send nothing. There is
  nothing to hand back and nothing was taken.
- **`Ctrl-K` is the chat's while the chat has the keyboard.** xterm maps `Ctrl` with a key code
  in 65..90 to `String.fromCharCode(code - 64)`, so `Ctrl-K` is `\x0b` — readline's
  kill-to-end-of-line. From anywhere else in the window it still opens the palette, because on
  Linux there is no `⌘` and dropping the chord outright would take the desktop's own key away
  on the platform that has only it.

**How the window knows.** A capture listener on the window runs before the focus has had any
say, so the only thing it can ask is where the keystroke was _delivered_: `e.target`. A pane
marks itself with `actions.CHAT_KEYBOARD` (`data-chat-keyboard`) and xterm's textarea is a
descendant of it, so `target.closest()` is the whole test. An attribute and not the `.pane`
class, because the class is how a pane is drawn and this is what it means.

**A scenario cannot carry this claim.** WebDriver does not deliver a modifier chord here —
`browser.keys(["Shift", "Tab"])` arrives with `shiftKey` unset, measured in #176 and recorded
above — so a key claim is evaluated in jsdom, where the real event can be dispatched and what
happened to it asserted. The missing modifier is one symptom of a larger one, measured in #186
and written up above: **this driver dispatches a synthetic DOM keydown and performs no default
action at all**, which is also why no key it sends presses a button and why Tab does not move
the focus even between two text boxes. `Palette.test.tsx`'s _"a chord the chat's own terminal would encode"_
is where the rule is pinned, including the half that matters most: the keystroke reaches the
document **unprevented**, which is what reaching the shell actually means.

**What is still eaten, and where.** The palette's window listener is the only place the
front-end claims a key, and after #106 it claims no readline binding. The application menu is
the other claimant. On macOS its accelerators are `⌘` chords, which xterm encodes as nothing, so
it takes nothing. Off macOS muda 0.19.3 gives the predefined items fixed `Ctrl` chords, every one
a key a shell owns: `Ctrl-C` (copy, against SIGINT), `Ctrl-A` (select all, against
beginning-of-line), `Ctrl-Z` (undo, against SUSP), `Ctrl-Y` (redo, against `yank`), `Ctrl-V`
(paste, against `quoted-insert`), `Ctrl-X` (cut, against readline's `C-x` prefix) and `Ctrl-H`
(hide, against backward-delete-char). So since purlis#187 the menu off macOS is Quit alone,
on `Ctrl+Shift+Q`: a terminal app's own keys add `Shift` (GNOME Terminal, Konsole), and xterm
sends nothing for a `Ctrl+Shift` letter. `lifecycle::layout` is where that is decided, and its
tests hold every accelerator it produces off macOS to this rule.

**The text-size keys take nothing either** (purlis#283, `textSize.sizeKey`). `⌘` on a Mac
and `Ctrl` elsewhere, with `=` or `+`, makes the text in focus bigger, with `-` smaller, with `0`
its default: a terminal pane's size inside a pane, the window's anywhere else. Measured in
xterm.js 6.0.0, `Ctrl` is encoded with a letter, space, `3`–`8`, `[`, `\` and `]` — not `=`, `-`
or `0` — so none of these is a shell's key. One modifier per platform, not both as `⌘K` has, so
a Mac's `Ctrl` chords stay the terminal's whatever a later xterm.js sends for them. The one neighbour a shell does own is `Ctrl+Shift+-`, whose `key` is `_` and which xterm
sends as `^_`, readline's `undo`: it is never matched, and reaches the shell unprevented. (A
real xterm sends `^_` for a plain `Ctrl+-` too; xterm.js does not, and it is the terminal in
every pane.) They are a capture listener on the window, like the palette's, and not menu
accelerators: which size a key changes depends on where the keystroke landed, which only the
page knows. Settings… is on the menu, on `⌘,` and `Ctrl+,`; xterm sends nothing for either.

**The new-shell key takes nothing either** (SI-5, ADR 0062, `shellKey.opensAShell`): `⌘⇧T` on a
Mac and `Ctrl+Shift+T` elsewhere — "new tab" in GNOME Terminal, Konsole and Windows Terminal,
whose tabs are shells. xterm.js 6.0.0 encodes `Ctrl` with a letter only when Shift is not held,
so `Ctrl+Shift+T` is no byte, and plain `Ctrl+T` still reaches the shell as transpose-chars.
It is a capture listener on the window, held by the project in front, and it presses the
catalogue's own `shell.new` row.

**The project switcher's key takes nothing either** (FR-27, `switcherKey.opensTheSwitcher`): `⌘P`
on a Mac and `Ctrl+Shift+P` elsewhere. xterm.js 6.0.0 sends nothing for `⌘P`, and `Ctrl+Shift+P`
is no byte for the new-shell key's reason, so plain `Ctrl+P` still reaches the shell as
previous-history. It is the palette's own capture listener on the window, because the switcher
is the palette listing only the projects; a further press while it is up moves down one row.
It finds files too (FM-7): a _Files_ group after the projects, where Tab widens the scope rather
than moving the focus, which the dialog's trap would only send round to the box again. The
aimed file's path is copied with `⌥⌘C` on a Mac and `Shift+Alt+C` elsewhere, and the file is
revealed with `⌥⌘R` and `Shift+Alt+R` (#1143, `Palette.fileKeyOf`), VS Code's keys for the
same two rows. They are read only in the palette's box, so they take nothing from a chat or a
tab; the line under the files says them while a file is aimed. Keys and not buttons, because a
listbox option's contents are presentational and a button inside one is not reachable.

**The title bar's workspace key takes one chord from a chat, off a Mac, on purpose** (#2916,
`workspaceSwitch.opensTheWorkspaces`, the operator's rulings of 2026-10-24): `⌥⌘P` on a Mac and
`Ctrl+Alt+Shift+P` elsewhere open the workspace popover, read by the physical key (`KeyP`), since
`⌥` turns P into another letter on a Mac. xterm.js 6.0.0 sends nothing for a `⌘` chord, so on a
Mac nothing is taken. Off a Mac it would send `ESC ^P` (C-M-p) to a focused chat, which bash, zsh
and readline leave unbound and Emacs binds to backward-list: the window takes it, as VS Code takes
its own app keys, so the key works from a chat too. AltGr, which Windows reports as Ctrl+Alt, is
not the key: a letter typed with AltGr+Shift+P is left alone. `⌘1`–`⌘9`
go to the project's pins; off a Mac `Ctrl+1`–`9` is the chat's while it has the keyboard. None of
them acts while a dialog is up.

**Find in a pane is `⌘F` on a Mac and `Ctrl+Shift+F` elsewhere, and takes nothing either** (SI-4,
`SessionPane.opensFind`). xterm.js 6.0.0 sends nothing for `⌘F`. It would send `\x06` for
`Ctrl+F` — readline's forward-char — so off a Mac the chord adds `Shift`, as GNOME Terminal and
Konsole do for their own find, and a Mac's `Ctrl+F` stays the shell's as well. It is caught by the
pane's own terminal (`attachCustomKeyEventHandler`) rather than on the window, because find is
over the pane that has the keyboard and nowhere else. Neither the palette (`⌘K`, `F2`) nor the
native menu claims `F`: the macOS menu is purlis's, Edit's predefined items and nothing else.

**A right-click in a terminal opens its menu, and Shift+right-click always does** (`SessionPane`,
`terminalRows.ts`): Copy, Paste and Select all, then the pane's rows. A program that turned
mouse reporting on gets a plain right-click, as it gets every other click, and draws what it
likes; Shift held is the way past it, as in iTerm2, VS Code's terminal and Windows Terminal.
**The menu key opens the same menu under the cursor and takes nothing**: xterm.js 6.0.0 sends no
byte for it. **Shift+F10 opens it too, and is the one key here that is taken**: it is
`CSI 21;2~` to a program, so it is the menu's only while no program has the mouse — a shell, or
a harness at its prompt — and stays the program's in a full-screen program that asked for the
mouse (Midnight Commander has a use for it). That much is a partition and not a hand-back: it
says when the key is taken, not how the program gets it then. **The hand-back is a second
Shift+F10**: with the menu the first one opened still up, it closes the menu and sends the
program `CSI 21;2~`, through the terminal's own input, as a second `F2` sends `F2` from the
palette. A Mac's keyboard has no menu key, which is why Shift+F10 is claimed at all. Both are caught by the pane's own terminal (`attachCustomKeyEventHandler`),
as find is.

**Find in a file is `⌘F` on a Mac and `Ctrl+F` elsewhere** (#946, #1088,
`LightEditor.opensFindInAFile`). In a light editor tab, a file or what changed in it, the same
find bar opens over the file, and in the merge view over both sides, the side before first. Off
a Mac it is plain `Ctrl+F`, with no `Shift`: a file is read, not typed into, so no shell is owed
readline's forward-char, and `Ctrl+F` is the key every editor finds with. It is read in the
tab's own key handler, so it finds only in the tab that has the keyboard. Escape closes the bar
and gives the keyboard back to the file.

**Search in the files is `⌘⇧F` on a Mac and `Ctrl+Shift+F` elsewhere, and takes nothing either**
(FM-8, `searchKey.opensSearch`). xterm.js 6.0.0 sends nothing for a `⌘` chord, and `Ctrl+Shift+F`
is no byte for the new-shell key's reason. Off a Mac the chord is the pane's find first: while a
chat has the keyboard, `Ctrl+Shift+F` stays its find bar, and everywhere else in the window it
shows the left side's Search view (#1676; it opened a Search tab before), searching as narrow as
the focus. It is one of the left side's keys, below.

**In the Search view's hits, `Shift+Enter` hands the hit stepped to to a chat** (#1151): it opens
the preview's chat picker for that hit and its line, as *Add to a chat's context* does, and
Escape closes it. It is a key on the listbox, which is never a terminal, so it takes nothing from
a chat; and it is not a button inside an option, which a listbox cannot hold. Enter alone still
opens the hit. `Shift+Enter` is the one plain modifier on Enter the listbox had free: the arrows
step through the hits, so it walks nothing the way it does in a chat's find bar.

**A file or folder row's menu has *Add to a chat's context* too** (#1151), so the keyboard reaches
it with Shift+F10 or the menu key. The row's menu has closed by the time it runs, so the same
chat picker (`ChatsToPick`) is drawn in a dialog (`AddToAChat.tsx`), the keyboard on its first
chat, and Escape or Cancel closes it with nothing typed.

**Several file or folder rows go to a chat as one paste** (#1151). In *Files*, `⌘`-click (`Ctrl`
off a Mac) picks a row or lets it go, Shift-click picks every row from the last one clicked, and
Shift with Up or Down picks from the keyboard; picked rows are `aria-selected` in a tree that is
`aria-multiselectable`. A line under the tree says how many are picked (a screen reader hears the
count as it changes), with *Add to a chat's context*, the same dialog with how many in its title, and *Clear*; Esc in the tree lets them go
too. One paste is one branch's: a row of another branch starts the picks again, and a row purlis
will not hand over is never picked. A plain click still opens a file and folds a folder. On a
picked row, the row menu's *Add to a chat's context* is *Add N picked to a chat's context* and
hands every picked row, in the tree's order; on a row outside the picks it hands that row alone,
as a file manager's menu acts on the row it was opened on.

**The left side's keys are an editor's** (#1673, #1676, `sideKeys.sideKeyOf`): `⌘B` puts the
navigation region away or brings it back, `⌘⇧E` shows Explorer, `⌘⇧C` shows Chats and `⌘⇧F`
shows Search, each giving the view the keyboard (Search's box); off a Mac they are `Ctrl+B`,
`Ctrl+Shift+E`, `Ctrl+Shift+C` and `Ctrl+Shift+F`. **`⌃⇧G` shows Changes on every platform**, a
Mac's too, as VS Code has it: `Ctrl` with `Shift` and a letter is no byte, so it is the
window's even while a chat has the keyboard. Changes has nothing to press, so the keyboard stays
where it was. **A view's key opens it wherever it lives** (#1739, P-6): Changes is in the
Panel's left half, so `⌃⇧G` opens that half on it, even with the Panel hidden, and leaves the
sidebar as it was; a view in a sidebar opens there. The palette's row does the same.
xterm.js 6.0.0 sends nothing for a `⌘` chord, so on a Mac none was a byte. Off a Mac `Ctrl+B` is
one (readline's back-char, tmux's prefix) and `Ctrl+Shift+C` is a Linux terminal's copy, so while
a chat has the keyboard both stay the chat's, as Search's key stays the find bar's;
`Ctrl+Shift+E` is no byte and is the window's everywhere. They are a capture listener on the
window, held by the project in front, and never under a dialog. Every view is also a palette
row (_Show the Chats view_, _Show the Explorer view_, _Search in files_, _Show the Changes
view_), with the key said on the row.

**The Panel's key is `⌘J`** (#1737, P-6), VS Code's, and `Ctrl+J` off a Mac: it hides the whole
Panel, every half of it, and shows it again on the views its halves had. It hides only a Panel
that is drawn: a half holding only extensions' panels that no extension contributes now is not
drawn, so where that is all that is left, the key shows the Panel (#1752). `Ctrl+J` is a
terminal's newline, so a chat with the keyboard keeps it, as it keeps `Ctrl+B`; a `⌘` chord is no
byte. `⌘⇧J` is the tab's chats menu, so Shift held is never this key. Its palette row is _Hide
the Panel_ or _Show the panel_, with the key said on it. A press of a half's open **Lower icon**
puts that half away, as a press of a side's open view does; with both halves away the Panel is
not drawn and the editor has the height. The handle between the editor and the Panel resizes it
(30% of the regions' height by default, 120px to 70%), and a double-click puts it back at 30%.

**F6 moves the keyboard from region to region** (#1741, P-13, `regionCycle.ts`): the left
sidebar, the editor, the right sidebar, the Panel's left half, then its right half, and round;
Shift+F6 goes back. A region put away, or a half a narrow window does not draw, is skipped. It
lands on the region's Tab stop — a tree's current row, as Tab would come in — or on the open view
itself when it has nothing to press, and in the editor on the focused pane's own controls. **A
terminal sends F6** (`ESC [ 1 7 ~`, htop's sort, Midnight Commander's move), so inside a chat's
terminal it is the chat's, and Ctrl+Tab is the way out, as always; landing on the pane's
controls rather than in its terminal keeps F6 the window's all the way round. A capture
listener on the window, held by the project in front, and never under a dialog. Each half of the
Panel is a region named for its side (_Panel, left half_) and one Tab stop, its view's: the
chevron on its line is not a stop, since its open Lower icon and ⌘J put it away by keyboard.
Every stop is a named region (#1751): the sidebars by their region's name (_Navigation_,
_Attention_) while they are out, and the editor as _Editor_. Both keys are palette rows, _Focus
the next region_ and _Focus the previous region_, with the key said, and both are in
`sideKeys.ts`' table; the row steps once the palette has given the keyboard back.

**A view moves from the palette too** (#1752): each view but the Inbox has _Move Search to the
panel_ or _Move Search to the sidebar_, the move its icon's menu has, and the region rows' notes
name the views each region holds now, not where the catalogue starts them.

**The right side's key is `⌥⌘B`** (#1678), VS Code's for its secondary side bar, and `Ctrl+Alt+B`
off a Mac; it puts the attention region away and brings it back. On a Mac Option makes B type
`∫`, so a key that types no letter is read by its place, as the palette's `⌥⌘C` is. Off a Mac
Ctrl with Alt is AltGr on Windows, so only a key that still types `b` is read, and a terminal
sends the chord, so a chat with the keyboard keeps it. Each of the right side's views is a
palette row too (_Show the Memory view_, and an extension's panel by its own name), and so is the
region (_Put the Attention region away_).

**The Inbox's key is `⌘⇧I`** (#1692), `Ctrl+Shift+I` off a Mac: `Ctrl` with `Shift` and a letter
is no byte, so it is the window's while a chat has the keyboard, as `Ctrl+Shift+E` is. It shows
the Inbox, the right side's first view, and gives the keyboard to its first ask, never to one of
the ask's buttons, so the key and a stray Enter after it answer nothing. The title bar's ✋ does
the same. The Inbox is one Tab stop, on the window's own roving focus (`useTabStop`): ↑ and ↓
move from an ask to its buttons and on to the next ask, Home and End go to the ends, Enter
presses the focused button as it does anywhere, and Escape gives the keyboard back to the chat in
front. A dispatch's Notice drawn in it keeps its own buttons as Tab stops. There is no key of a
single letter (I-11).

**A switch at its top picks whose asks it lists**: All projects, This project (the default) or
This workspace, each with its count. It is the Chats view's own scope switch (`ScopeSwitch.tsx`,
one piece for both views), a Tab stop of its own before the list, and the keyboard still comes
in on the first ask. Only the scopes the window can list are offered, and none while nothing
waits anywhere. This workspace follows the strip, and at the plane root lists the asks of chats
filed in no workspace. All projects adds what waits in other windows' projects and, for each
other project of this window, its asks as text under its line, answered there: no answer is
carried from one project's Inbox to another's. The pick is kept for the project in the layout
file (`inboxScope.ts`, `projects[path].inbox.scope`). A scope with nothing in it says how many
wait in the nearest wider scope and widens in one press, and a notification about a chat the
scope leaves out widens it to the project's before it lands. Notices and updates are the
project's and are listed whatever the scope.

**An Inbox row reads in three ranks** (`InboxRow.tsx`), from the existing tokens only:

| Rank      | Says                                 | Class             | Size               | Weight | Colour             |
| --------- | ------------------------------------ | ----------------- | ------------------ | ------ | ------------------ |
| primary   | what needs you: the ask's words      | `inbox-primary`   | `--font-size-md`   | 600    | `--text-primary`   |
| secondary | where: project · workspace · persona | `inbox-secondary` | `--font-size-sm`   | 400    | `--text-secondary` |
| meta      | its kind, then its age, trailing     | `inbox-meta`      | `--font-size-xs`   | 400    | `--text-quiet`     |

The kind is a mark and its words together (`KindMark`, `data-kind`): a decision in
`--state-waiting`, an update that went wrong in `--state-failed`, anything else in the secondary
text, and the words in the meta beside it, so nothing is told by colour alone. The primary line
is cut to two lines (`inbox-clamped`) with the whole of it in its tooltip and in the page for a
screen reader. **The words of an ask whose answer here allows are never cut** (#1146): what an
Allow lets run is read whole first. A section of the Inbox itself (Notices, Updates) wears the
sidebar's title (`.sidebar-title`); a chat's chain is data and is drawn as written.

**Its updates follow the asks** (#1693), on the same roving focus: Mark all read and Dismiss all,
then each update and its buttons. With no ask, the keyboard comes in on the first update, never
on a button. An answer that grants something (a refused dispatch's Allow from now on) pressed
within `SETTLE_MS` of its update being drawn or moved by what came in above it is not sent, and
the update says why, as a dispatch's Notice does when its question changes.

**The keys for the chats inside a tab take nothing either** (#1487, `taskKeys.taskKeyOf`).

|                                                               | On a Mac      | Everywhere else                 |
| ------------------------------------------------------------- | ------------- | ------------------------------- |
| The tab's task menu                                           | `⌘⇧J`         | `Ctrl+Shift+J`                  |
| The next / previous chat in the tab                           | `⌘⌥↓` / `⌘⌥↑` | `Ctrl+Shift+]` / `Ctrl+Shift+[` |
| Back to the session's own chat                                | `⌘⇧H`         | `Ctrl+Shift+H`                  |
| **Reserved, not built**: the next / previous tab on the strip | `⌘⇧]` / `⌘⇧[` | `Ctrl+PageDown` / `Ctrl+PageUp` |

Read in xterm.js 6.0.0 (`common/input/Keyboard.ts`): a `⌘` chord with a letter sends nothing
but `⌘A`; each arrow's case breaks on `metaKey` before it builds anything, so `⌘⌥↓` and `⌘⌥↑`
send nothing, while `⌥↓` without `⌘` is `ESC [1;3B` and stays the terminal's; and `Ctrl` with a
letter, `[` or `]` is encoded only when Shift is not held (the branch is `ctrlKey && !shiftKey`,
and with Shift only `_` and `@` are turned into anything). So none of the chords was ever a
byte, and plain `Ctrl+J` (newline), `Ctrl+H` (backspace), `Ctrl+[` (Escape) and `Ctrl+]` still
reach the shell. `⌘H` alone stays the system's Hide: Shift is part of that chord.

**Where a task is drawn has keys too, and each is on the thing it moves** (#1489). On a task's
row in the Chats list, Space opens it beside the session that asked for it. On its line in its
tab's task menu, Enter goes to it and Enter with a modifier opens it elsewhere, as a link is
opened in a new tab: `⌘`/`Ctrl`+Enter gives it a tab of its own, and `Alt`+Enter opens it beside
its session. On a task's own tab, the key that closes a tab (Delete; Backspace on a Mac) sends
it back into its session's tab and ends nothing. None of these is a window-wide key: each is
handled by the row, the menu line or the tab that has the keyboard, so no chat loses a key.
The buttons at the end of a menu line are the pointer's way to the same, and are not in the
accessibility tree, as a list row's fold is not: the keyboard's way is the line's own keys and
the task's menu in the Chats list.

**Down and Up on a Mac, because a tab's chats hang under it**: its menu lists them top to
bottom and Down on a focused tab opens that menu. **`⌘⇧]` and `⌘⇧[` are not taken**: they are
next and previous tab in every tabbed program on a Mac, and they are kept for the strip. Off a
Mac the brackets have no such meaning (tabs there are `Ctrl+PageUp` and `Ctrl+PageDown`) and
`Ctrl+Alt` with an arrow is the desktop's.

**A key is matched by what it types, not by where it is**, so a layout that puts another
character on a key never fires two of the window's actions at once: on Dvorak the key in `]`'s
place types `=` and `+`, which with the same modifier is the text size's. Only where a layout
has no `[` or `]` without AltGr (German, Nordic, French) is the key in their place taken, and
then never when it types a letter or a digit or is another of the window's keys. The palette's
rows for the two say so.

They are one capture listener on the window, held by the project in front, and each presses
its catalogue row (`tasks.menu`, `tasks.next`, `tasks.previous`, `tasks.own`), which the
palette lists with its key. **Nothing is taken while a dialog has the keyboard.** Over a
terminal the chord is the window's whether or not its row can run, so the same chord never
means two things from one tab to the next; in a text field or an editor a chord whose row
cannot run is left to the field.

**A menu that opens under a resting pointer takes no keyboard** (#1487, `TabChip.tsx`). The
task menu on a tab's chip is Radix's dropdown menu, opened three ways: a press, the key above,
and the pointer resting on the chip. Radix moves the focus into a menu as it opens and onto
each row the pointer crosses, which is right for the first two and would take the keyboard out
of a terminal for the third. So a rest-opened menu refuses both (the menu's `onOpenAutoFocus`,
and each row's pointer handlers), the stylesheet's hover says which row the pointer is on, and
an Escape that closes it stops there instead of also reaching the chat. Only that menu closes
when the pointer leaves: a pressed or key-opened menu has the keyboard and stays until Escape,
a pick, a press outside or the same ask again. However it was opened, a menu closed without a
pick puts the keyboard back where it was before it opened.

**A rest is a move the person made, then stillness.** Both WebKits hit-test again after a
layout and say "the pointer came onto this" of a pointer that has not moved, so a chip that
appears, widens or slides under a parked pointer would open its menu by itself. Coming onto
the chip therefore arms nothing: the 350 ms start at the first pointer move to a new point,
with no button held.

**Shift+Enter in a harness's pane is the harness's newline** (SI-4, `Harness::newline`). A
terminal has no Shift+Enter: xterm.js 6.0.0 sends a bare CR for it, the byte Enter sends, so every
harness submitted on it. The pane sends the harness's own newline instead — ESC CR for all three,
measured — and a shell's pane, which runs no harness, keeps xterm's Enter.
