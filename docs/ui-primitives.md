# The window's UI primitives

**Component families on React Aria. New UI is built from them, not from hand-rolled markup.**

Since #2904 (ADR 0104), every control of a moved family is drawn from its **component family** in
`app/src/ui/<family>`, which wraps its React Aria primitive once; a screen never imports the
primitives library, and lint holds it. The button family is the first: [The button
family](#the-button-family). **Every family has moved and Radix is gone** (#2930): no file imports
a `@radix-ui/*` package and none is a dependency (`app/src/radixGone.guard.test.ts`). Where the
rest of this file still says Radix, it is the history of why a family behaves as it does.

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
  tab list: take its family from `app/src/ui`, which wraps React Aria's primitive once (ADR 0104).
- **One wrapper per primitive, and nothing between a screen and its family.** A family is the
  only place its primitive is imported (`no-restricted-imports` in `app/eslint.config.js`), and no
  second purlis layer stands in front of a family: a screen reaches the family's own props. The
  test is at the **call site**: can the next person see which family this is and reach its props?
  Until ADR 0104 the rule here was the opposite, no layer at all between a screen and a Radix
  primitive (ADR 0037); the families replaced it, and Radix is gone (#2930).
- **A component's source copied into this repo is allowed** (ADR 0037, amended 2026-09-22): a
  file in `app/src/components/ui/` is ours, editable line by line, and visible in the diff that
  adds it, which is not a dependency that owns your markup. It builds on the families like any
  screen, since nothing outside `app/src/ui` imports a primitives library and no `@radix-ui`
  package may come back (`radixGone.guard.test.ts`). `docs/design-system.md` has what a copy must
  satisfy.
- **One house set is allowed, and only this one: the settings set** (ADR 0037, amended
  2026-10-04, rulings V89f and V89j; SE-16 #1166, DS-3e #1177). `app/src/settings/components.tsx`
  holds six pieces — **SettingsLayout**, **SettingGroup**, **SettingRow**, **Field**, **Choice**
  and **SettingActions** — and nothing else may join it without a new amendment to that record. See
  [The settings set](#the-settings-set-is-the-one-house-set) below for why.
- purlis's own look, always. React Aria ships **no CSS at all**: every primitive is an unstyled
  element with `data-*` state attributes (`data-pressed`, `data-selected`, `data-focus-visible`)
  to hang rules off. A control takes its look from **its
  family's stylesheet in `app/src/ui/`** ([Every control is a family's](#every-control-is-a-familys)
  below), and `App.css` draws the rest of the window — but **no colour is written in either**: every one is
  `var(--<token>)` and the values live in `app/src/theme/`. See `design-system.md`.
- Native HTML that already does the job is not hand-rolled markup and does not need replacing:
  `<details>/<summary>`, `<label for>`, `<button>`. Reach for a primitive when the browser has
  no element for what you mean — a modal that traps focus, a listbox, a menu.
- `react-resizable-panels` stays what draws resizable panes. React Aria has no panel primitive
  and there is nothing to move.

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

- **Six pieces, named here, and no more.** `SettingsLayout` (since #2919 prototype A of
  `prototype/settings-page`: a side column with the level switcher, the search box with its Set
  here toggle (one search across every level, #2920, whose matches are the nav's first page) and
  the nav of groups, which folds a page's own pages, and no title of its own (#3147), and the
  chosen page beside it, its rows scrolling under its header), `SettingGroup` (on a
  settings page the page's header — its title, its blurb and "Kept in ‹file› · Edit as TOML" —
  and its rows, and at `level` 3 one of the page's own groups, a heading and its one line, a
  rarely used one (`rarely`) folded as "‹title› · ‹count›"; in a dialog a heading and one help
  line), `SettingRow` (on a settings page a row
  of the aligned form: the label with its help line under it, then the control, then one ⋯ menu
  holding Reset, the moves between files and Copy key, every control at one x; in a dialog the
  label and control on one line with the help under them; since SE-17 a refused write's reason
  and the last change's Undo; since SE-18 the value's origin, drawn quietly since DS-8: a badge
  only where the value is kept elsewhere than its page's, a mark on a value set at this level
  with the tooltip "Set at this level", and since #2919 an amber "deprecated" badge), `Field` (`text`, `list` and `range`), `Choice` (`radio`, since #2907 `cards`,
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
  The level switcher is the segmented control; the nav, and the Edit as TOML files at its foot,
  are the panel family's page navigation (`ViewNav`, #2930); `Choice` is the radio group, the option cards, the
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
operator's variant A, and #4754): the header (`PaneHead`), with a runway at its top so the chosen
tab joins the terminal's fill, the chat at a glance at its start and the pane's actions at its
end (`PaneActions`: the chat's state button, bell and ⋯, a thin divider, then split side by side,
split stacked and close, which are all a view's pane draws, #3147, #5203); then the band, under
the header so a Notice arriving never moves it; then the terminal, pinned to its bottom so a warning
coming or going moves its top and never its last line. **A view's pane carries no title of its
own** (#3147, the operator's ruling of 2026-10-24): its tab is its name and the content panel is
that tab's panel, so the view starts on the fill its chosen tab joins, with no band; the same
`PaneHead` and `PaneActions` sit at its top right, over its first row, which holds what can be
done to what the view is about (or Settings' levels) and keeps the actions' room at its end. In
the band each Notice is **one compact row** (#5215, the person's approved prototype,
`Notice.tsx`'s `BandRow`): its icon on its first line, what it says on one line with its sub-line
muted beneath (who asks or what it is, and its age where known), its own buttons, a toggle where
the line cuts it, and a × last, with a separator to the next row. A consent keeps its full
waiting-coloured edge (ADR 0097 §6), and its Allow stands apart from Keep blocked as the only
answer with an edge. **× never answers.** A note's is its Dismiss; an ask's closes it from this
pane alone, so it leaves the pane's bell and stays in the Inbox and on ✋ until it is answered.

The band is a `region` named with its count, "Pane notices: 3 waiting on you", as its bell is
"Pane notices: …" (apart from the status line's Notices and a chip's "Notices for …"). Esc on a row
folds a message opened whole, then puts away a row with a Dismiss, and the keyboard goes on to the
next row or to the terminal, never onto an Allow: a consent is landed on by its line. A consent has
no Dismiss and Esc leaves it. The press-settle guard (#1695) reads where each line is drawn against
the band itself, scrolling aside, never its `offsetTop`. In the row they are stacked (the notice
family's `stack`), in the order the pane writes them: the shown chat's own, the one that waits for
an answer before anything starts first, then each hidden chat's.

**There is no cap** (#4754): the band is **minimised or expanded**, one choice for every pane,
minimised by default and kept in the layout file (`paneNotices.ts`). Minimised, its notes fold into
**the bell** (`PaneBell`), in the pane's header in a fixed slot, and the band is one line, a button
that says how many wait and on what ("3 waiting · A task's question, Sandbox block · Show"); with
only notes, there is no line at all. Expanded, every row stands under a head that says how many
wait, with Dismiss all where it clears any, which skips the asks and counts what it clears, and the
fold. The bell's count is the pane's notes, never its asks, the same number in both states, in the
trouble colour while one note is trouble; a new Notice never opens the band or takes the focus, and
the bell says its count politely. A folded row is `hidden` where it is drawn, never moved, so it
keeps its state, and one the keyboard is on stays until the focus leaves it.

**An opened message shows whole** (`data-whole`): the band grows, the terminal gives way down to
three lines (`.pane-body`'s floor, with the foot in place), and only past that does the band scroll,
as one, with the message's buttons pinned at its foot and no inner scroll box. At rest the rows take
about seven controls' height. A task's question (`TaskQuiz`) is such a row, with Answer, which opens
it in the Inbox, and its box to answer in pinned at its foot when opened; a prompt in the chat's
own terminal (`TerminalPromptNotice`) is one with Go to prompt; the where-you-answer note is an
information line with Change. Every change of the row's height refits the terminal, which resizes the
chat's pty and redraws its harness; minimised, a note arriving changes only the bell's count. The
chat at a glance (its name, its state, the breadcrumb) is the pane line, and harness · workspace and
the gauge are the pane foot, neither over the terminal. `Notice.pane.test.tsx` and `PaneBand.test.tsx` hold the shape and the rules, and
`pane-notices.e2e.ts` and `pane-warnings.e2e.ts` measure them, with no Notice over the terminal in a narrow pane and a
wide one. Everything the window draws is ordered inside `#root`, which is one stacking
context, so no `z-index` in the window is over a dialog or a menu: those are portaled to the
body and need no number.

## Tables, list rows and tree rows share one style each (DS-3)

Before #1835 the window drew **4 tables and 8 kinds of list row**, each with its own padding,
alignment and selection. The Chats list alone put a name about 53px past its own filter, partly
because every leaf drew an empty twist so that names would line up. ADR 0094 §4 gives each of
these one class, built only from the core geometry and a theme's colours, in
`app/src/primitives/collections.css`. Each kind is its family's now, the table and the list row
since #2911 and the tree row since #3198, drawn from the same geometry and keyed on its parts'
attributes; the last `.ui-row` (the whole Explorer's folding section head, now the disclosure
family's) moved in #2930, and `collections.css`, its test and its `--ui-row-*` aliases are
deleted.

### Table

**A table is the table family's** since #2911: [The table family](#the-table-family) below
draws it from the same geometry, keyed on its parts' attributes where `.ui-table`,
`.ui-table-fixed`, `.ui-table-stacks`, `.ui-table-room`, `.ui-num`, `.ui-cell-actions` and
`.ui-table-tip` were. A column's short
words with their whole words in a tooltip, once `settings/ColumnHeading.tsx`, are the family's
`ControlColumn` (`whole`, `about`). **Short headings live in the window** (#1859, after
D-1841-1): a column's short words are the table's own (`SHORT` in `settings/dispatch.tsx`), and
the core keeps one name per limit, its `label`. Stacked, a cell shows the whole label, never the
short words.

### List row

**A list's row is the list family's** since #2911: [The list family](#the-list-family) below
draws it, from the same geometry, with its slots (`RowLead`, `RowLabel`, `RowDescription`,
`RowTrail`) where `.ui-row-lead`, `.ui-row-label` and `.ui-row-trail` were. Nothing wears
`.ui-row` any more (#2930), and the button family's lint no longer lets a raw `<button>` through
by it.

### Tree row

**A tree is the tree family's** since #2911: [The tree family](#the-tree-family) below draws the
Changes view, every file tree (a file tab's, the Explorer's *Files* and a branch's cockpit), the
Explorer's *Repos and branches* and, last, the Chats list (#3198). The shared tree row
(`.ui-tree`, `.ui-tree-row`, `.ui-tree-twist` and `--ui-tree-*`) is deleted.

**The Chats list on the tree family** (#3198). A chat is a `TreeRow` named by its name (what
typing finds), whose description says its persona and state, its marks' words and what its lines
under it say (`RowHeard`, `useRowDescription`, so a chat moving draws no row). Under it, as its
rows, are its browsers (#2024), the chats it asked for and its finished tasks (#1485), so it
folds them all; the done ones fold into a **Finished (n)** row of their own, with Clear finished
at its end and in its menu. The hand of a chat below that needs you, the ⋯, the at-limit line's
Dispatch settings and a task's second step of ending stand in the row, each reached by → from
it. Space (open a task beside), F2 (rename) and Delete (stop a task) are the tree's `onRowKey`,
heard before React Aria's own keys. Its one Tab stop is the chat in front (`stopAt`, said
`current`) until the keyboard has been in, then the row it was last on. A workspace's sessions,
where they are grouped, are a `TreeSection` under the frame's group head. A top-level chat's name
starts one tree step, one mark and one gap in (`--chats-name-at`), and the filter's text starts at
the same place. What the filter hides and why a key did nothing are said beside the view's title.

**Every tree and list in the main views was on these classes** (DS-12, #1844), and is its
family's now: the explorer, a file tab's tree, the Changes view and the Chats list are the tree
family's (the `.tree` class is deleted); the panel lists, the memory archive, Search's hits and the opener's recent projects were
`.ui-row`s and are the list family's now; the vault's, Dispatches' and Saving's tables are the
table family's.
A tree whose levels are nested groups gives each row its `aria-level`, and puts the children's
level (`data-level`) on the element that holds what a row says under it (a folder's "Nothing
here", a clone's "Asking git…"), so a sentence stands where the children's names start. A row
that must grow to its name rather than cut it (the explorer's, which scroll sideways) keeps its
own `min-width: max-content` beside the class.

## The button family

**Every button, icon button and button drawn as a link is `Button`, `IconButton` or
`ToggleButton`** (`app/src/ui/button/Button.tsx`, #2904, ADR 0104), on React Aria's `Button` and
`ToggleButton`. No screen draws a `<button>` of its own for one, and none wears the old
`.ui-button*` classes: `eslint.config.js` refuses both (`purlis/button-family`, an error). No raw
`<button>` is let through as another family's control any more (#2930): the last ones, a tab's
close, the settings nav and its Edit as TOML files, moved onto their families, and the rule takes
no list of roles or classes to let one through.

| Prop | Says |
| --- | --- |
| `variant` | `secondary` (the default, the plain button), `primary` (the answer purlis suggests), `ghost` (no box until the pointer reaches it), `danger` (what cannot be undone from purlis, D-1719-3), `grants` (an answer that grants, an Allow or an Accept), `soft-accent` (a suggestion that is not the answer), `warning-soft` (care, without ending anything), `link` (a button that reads as a link, as tall as its words so it can stand in a sentence), `quiet` (a ghost whose words are in the quiet tier until the pointer reaches it: a strip's end mark, a pane's split and close, a line's own marks, a breadcrumb's crumbs and a pane head's tasks button, #3214, #3264, #5341), `quiet-link` (a link in the quiet tier, underlined in the primary words under the pointer and the keyboard and never filled: a card's way on beside its one answer, Go to chat in an Inbox card's head, #5214) |
| `size` | `sm` 24, `md` 28 (the default), `lg` 32, at the default text size and density; `lg` only beside a list's main filter field |
| `inline` | words in a line of words, a breadcrumb's crumb (#3264): as tall as the line, no box padding or edge, cut to an ellipsis where the line squeezes it, underlined (no fill) under the pointer and the keyboard; words only |
| `icon` | the icon before the words, drawn as the `icon` part; `IconButton` takes only an icon and an `aria-label`, and is square |
| `onPress`, `isDisabled`, `isSelected` | React Aria's own words; a parent that clones the button with its own handlers (`ui/trigger.tsx`, `ui/menu/ContextMenu.tsx`) still reaches it |
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
  `data-part="icon"`, and the key that presses it, where it says one (`shortcut`, R33),
  `data-part="shortcut"`: a `kbd` after the words, small and mono, drawn from its `data-key` and
  hidden from a screen reader (`aria-keyshortcuts` says the key), its room kept and nothing drawn
  while the key does not press it (`shortcutOff`). `app/src/ui/button/button.css` draws the family default from those and
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
  label to the box and the help and the error to its description. The error line is a polite
  live region, never an alert (#3291), and the box's edge takes the danger colour beside it: the
  words are the cue that is not colour. **A refused send is heard once**, as its `Form`'s one alert
  summary ("Name: A name is letters…", or "2 fields to fix: Name, Port."), for a screen reader
  alone; the lines are quiet while it shows, and each stays its field's description. A send the
  core refuses is handed to the `Form`: `refused` by its owner, with `answer` counting the core's
  answers, or `useFormRefusal()` from inside it; a select's pick ends the summary as a change
  does (ADR 0104, amendment 2026-10-30). **A field that can be refused keeps its error's line** whether it
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
  dragged. With `strip` (`data-foot-strip`; an Inbox card's reply, #5214) it is a strip at a card's foot: no box
  but a divider above it, in the words' face, one line tall at rest (`data-rows`), two while it
  holds the caret and up to four as its lines need, moving at the theme's pace. Its `hint` part
  (in the quiet tier) shows at its foot only while it holds the caret, and is always the box's
  description; `end` places its send button inside it. A shape, like `grows`, and not a
  variant: the strip keeps the box's colours and refused edge.
- **Stories hold it** (`Field.stories.tsx`, `docs/component-stories.md`): a labelled field,
  inline validation on blur, a refusal, the three sizes with their glass, an unavailable box, a
  textarea and its strip, a form sent by ⌘⏎, and a filter that keeps its caret while its list redraws, each
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
| `Select` | a button that shows what is picked and opens the list to pick from; `value`, `options` (`value`, `label`, `isDisabled`, `tooltip`), `onValueChange`, `placeholder`, `isDisabled`, `error`, `isInvalid`, `tooltip`, `size` |
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
- **Refused** (`error`, #3110, #3334): a select whose value the form will not take, or a
  combobox whose typed value is not allowed, draws the danger colour on its edge, inside the box
  as the text input's, and is marked `aria-invalid` and `data-invalid`. `error` says why in the
  field's error line under the box (the `field.error` part), a polite live region kept from the
  start, so a refusal that lands while the person types is heard and nothing moves; a select
  given `error` stands in a field of its own, placed by its `className`: a column as wide as its
  place, its button as wide as the field (in a toolbar row, give it a width). `error=""` is no
  refusal. `isInvalid` marks one
  refused whose words are drawn elsewhere (a settings row's), named by `aria-describedby`. A
  colour only; a theme reaches it as the `invalid` state of either family's `root`.
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
or wears the old `.ui-checkbox`,
`.ui-radio`, `.ui-switch`, `.ui-segmented-*`, `.ui-levels` or `.ui-choice-*` classes:
`eslint.config.js` refuses each (`purlis/choice-family`, an error).

| Member | Is |
| --- | --- |
| `Checkbox` | a native box in its own label: a press of its words ticks it as a press of its box does, Space from the keyboard; `isIndeterminate` draws a bar. `variant="chip"` is one of a few small chips over the list they narrow, its tick in a slot it always keeps; a chip's `count` is after its words and describes it, never part of its name (#3484) |
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
| Panel | `Panel`, or `panelRoot(variant)` on a library's resizable panel; `PanelTitle`, the title of the view it shows (`ui/panel/Panel.tsx`) | `root`, `title` | `side`, `content`, `bottom` |
| Panel section | `PanelSection` (with `view` and `top`), `PanelSectionHead`, `PanelSectionTitle`, `InPanelTop` (`ui/panel/Panel.tsx`) | `root`, `head`, `title`, `top` | `default` |
| Panel separator | `Separator` (`ui/panel/Separator.tsx`), every resize handle between two panels | `root`, `grip` | none |
| View frame | `ViewFrame`, `ViewNav`, `ViewListHead`, `ViewPageHeader` (`ui/panel/ViewFrame.tsx`), a view tab laid out as pages; `ViewBar`, `ViewHistoryButtons`, `ViewCrumbs` (`ui/panel/ViewBar.tsx`), its top bar | `root`, `nav`, `list`, `item`, `head`, `header`, `name`, `lead`, `bar`, `history`, `path`, `crumb`, `crumb-separator` | none |
| View nav | a page in the view frame's navigation, drawn by `ViewNav`; a page that heads pages of its own (`expanded`) ends in its group's `toggle` (#2919): → reaches it, ← goes back, and `onExpand` opens or folds the group; `rowWhenNarrow` lays the pages out as one sideways-scrolling row where its container is under 728 px, and `edge={false}` draws no line after it where its holder draws that edge | `item`, `label`, `indicator`, `toggle` | none |
| Review page     | `ReviewPage` (`ui/review-page/ReviewPage.tsx`), a review too long for a dialog: its body over a fixed action bar                                                                                       | `root`, `body`, `bar`, `summary`, `actions`, `answers`                                                                | none                               |
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
- **A resize handle is a panel separator** (#3143): the room it takes and its line at rest are
  the regions' (`App.css`, `--separator-room`): the gutter at a region's edge, and between two
  panes in a split the gutter in the window's colour, or the hairline where the panels sit flush; its box is never thinner than the keyboard's ring
  and straddles that room with a negative margin, so Tab always lands on something drawn and
  nothing moves. Under the pointer, while dragged and with the keyboard it shows its grip, a
  short mark at its middle drawn over the panels either side. Purlis's grip is the faint tier
  under the pointer, the muted tier while dragged and `focus.ring` with the keyboard on it, in
  place of the window's ring; Purlis Classic keeps its accent line through its theme's parts.
  Forced colours draw the grip in `CanvasText` and `Highlight`, and the ring comes back.
- **A sidebar view is a section with a top** (`view`, #3340, the sidebar-views prototype's
  Q1 A): ONE scroll, its region's. Its `top` (the heading line with its actions, then the field,
  then tabs and chips) is held at the top on the panel's own fill, `--panel-fill`, which a
  theme's `panel.root.fill` writes beside the ground; each group's name is held under the top
  while its group is in sight, in 11px capitals in the quiet tier with its count at its end; its
  rows are 30px. Its title, whether the region's head or its own heading line, is the panel's
  `title` part (`PanelTitle`): 13px and strong in the primary tier, in its own case. A list puts its search field in the top with
  `InPanelTop`, and draws it where it stands outside a view. Memory, Todos and Sessions are
  drawn on it, in a half of the Panel or in either sidebar, and so are Personas, Vaults and every
  extension's panel (#3341), and the Chats view (#3342), whose rows lead with the
  persona's mark and the chat's state as a dot on its corner.
- **A view tab laid out as pages is a view frame** (#3666, the Skill studio frame's variant A,
  `purlis/view-frame-family`): the page navigation, the page's list (its heading and lead line,
  `ViewListHead`, then a list or table) and the item chosen in it beside it (its page header,
  `ViewPageHeader`: the name, a tag and one mark, the one-line description, up to three actions
  and a "…" menu; then its banner, a Notice `alone`, and its body). The navigation is one Tab
  stop, the current page `aria-current="page"` and marked at its start edge as well as by its
  colour; ↑ ↓ Home End walk it and Enter, Space or a press chooses. A page's count is the count
  family's, `waiting` for what waits for the person, and a page with nothing in it is left out.
  Each pane scrolls on its own and the page header stays in view; the panes' sizes are the
  frame's, so a new item or a changed count moves nothing, and the list and the item are resized
  through the panel separator. Under 720px wide (a narrow tab, a split pane) it is page-and-back:
  the navigation is a `Select`, the list comes first, an opened item is drawn alone with a way
  back, and the keyboard follows: onto the way back as the item opens, onto the list as the
  person comes back.
- **A view tab has its own back and forward** (#3667, `ui/panel/viewHistory.ts`): the tab's
  history is kept by its id in memory (`useViewHistory`), so another tab never shares it and a
  relaunch starts afresh, and a place reached from outside the tab (`openViewAt`: the run chip's
  Runs) is a step on a tab that has a history and its start on one that has none. `ViewBar` draws
  back and forward at the bar's start, each unavailable with its reason at its end, and the path
  (`ViewCrumbs`, a list named "Path": each name before the last on the quiet ghost inline, the
  last `aria-current="page"` and no button, long names cut first, the whole path its tooltip),
  then the view's own controls. ⌘[ and ⌘] on a Mac, Alt+← and Alt+→ elsewhere (`view.back`,
  `view.forward` in the key registry, in its new `view` place; Ctrl+[ is a terminal's Escape and
  an editor's indent, so Ctrl does not take ⌘'s place), and the mouse's own back and forward
  buttons walk it anywhere in a `ViewFrame` handed the `history`; an editor, a menu or a dialog
  keeps the keys.
- **A review too long for a dialog is a review page** (#3672, the skill sources and skill
  grading prototypes, `purlis/review-page-family`): the install review and the update review,
  file by file, and the grade plan, test by test. Its body is the only part that scrolls, and it
  ends where the bar starts, so the page scrolls under the bar's edge and a focused control is
  never under it (WCAG 2.4.11); a body that scrolls with nothing in it the keyboard can reach is
  a Tab stop of its own. The bar, on the panel's fill under the window's hairline, says at its
  start what pressing the main action does and what it spends (`summary`, which describes the
  main action), then holds at most three answers in a dialog's places, drawn by the dialog's own
  answer row: Cancel leads, the main action trails, one other may stand between. The main action
  may be unavailable with its reason and stays a Tab stop; Return in a field is the main action
  unless it grants or cannot be taken back. Its words keep their room (`reserve` on the answer,
  `summaryReserve` on the summary), so "Start grading · 12 chats" turning to "· 9 chats" moves
  nothing. Under 30rem the summary takes its own line over the answers. In a view frame's item
  pane it takes the room under the page header. The bar's edge is never a theme's: it is a
  consent surface where it holds a grant.
- **The status bar follows its contribution model** (N28, `ui/status/StatusBar.tsx`): every item,
  purlis's own and an extension's, says its `id`, `side` (left or right) and `priority` (highest
  first from the side's left), and the person may hide any item from the bar's menu (a right
  click, or the menu key or Shift F10 on an item) and show it again there; what is hidden is kept
  in the layout file under `status`. One item the keyboard reaches cannot be hidden (Notices), so
  the menu always opens from one, and when it closes the keyboard is back on the item it was
  opened from. The person may reorder the items on an end, never between ends (#3030): a drag
  along the end (`dnd-kit`, by the pointer only) or, in the same menu, Move left and Move right,
  whose rows list every item the bar draws, stay up for the next move and say each move to a
  reader. A cell of the bar is no Tab stop, so the rows start from the menu and not from an item.
  The order is kept under `status.order`; an item absent today keeps its place in it, and one it
  never named stays after the item it follows by priority. Once the bar draws its items otherwise
  than their priorities do, the menu's last row is Reset order (#4776), which forgets the order and
  closes the menu. A press runs the item's command:
  purlis's own items are `StatusItem`s whose press
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

## The card, notice, badge and count, and separator families

**Every card, Notice, badge, count and separator is drawn by its family** (#2912, spec #2897,
ADR 0104), each one wrapper under `app/src/ui/` and its own lint rule at error
(`purlis/card-family`, `purlis/notice-family`, `purlis/badge-family`,
`purlis/separator-family`): a screen wears none of their old classes (`ui-inline-surface`,
`notice-*`, `pane-notices`, `ui-setting-badge`, `count-pill`), writes none of their
`data-scope`, and draws no `<hr>` of its own.

| Family | Wrapper | Parts | Variants |
| --- | --- | --- | --- |
| Card | `Card` (as a `div`, `li`, `p`, `section` or `article`), `CardTitle` (`ui/card/Card.tsx`); `Facts`, `Fact` (`Facts.tsx`); `Totals`, `Tile` (`Totals.tsx`); `CardButton`, a card pressed whole (`CardButton.tsx`, #3655), its own family `card-button` (#3885) | `root`, `title`; `facts-label`, `facts-value`, `tile`, `tile-value`; a pressed card's `root`, `mark`, `press`, `title`, `more`, `line` | `raised`, `ask`; a pressed card's `door`, `pick` |
| Notice | `Notice`, `NoticeList`, `NoticePaneRow` (`src/Notice.tsx`), drawn through `ui/notice/Notice.tsx` | `root`, `message`, `whose`, `title`, `mark`, `sub` (a card's state and age), `head`, `heading` and `end` (a card's head, layout only), and `under` (what a way out opened); `notice-card`'s `said` holds a card's live region | `news`, `trouble`, `attention` |
| Badge | `Badge`, `HostStatus` (`ui/badge/Badge.tsx`) | `root` | `neutral`, `accent`, `waiting`, `ok` |
| Count | `Count`, `CountGroup` (`ui/badge/Badge.tsx`) | `root`, `group` | `neutral`, `accent`, `waiting`, `hot` |
| Separator | `Separator` (`ui/separator/Separator.tsx`, on React Aria's) | `root` | none |

- **A card is a box one step off its surface**: an ask in the Inbox, a task's question in a chat,
  a block's report under its Notice, a code to copy. Purlis draws it the Figma way, on the window's
  fill with corners 10 and no hairline (R25, its theme's `card.root.fill` and
  `card.root.border-width: 0`); Classic on its raised ground with the subtle hairline. **An ask
  keeps its waiting edge in every theme** (ADR 0097 §6): no theme can take it away, as no theme
  can take Allow's.
- **Facts, as label and value** (`Facts`, #3673): what a screen knows about one thing, read only,
  a fact a line (a skill's Overview, a source's header, what a grade may spend). A `dl`, each fact
  its label and value, so a reader hears them together; one label column in the secondary tier
  (`card.facts-label`) and the value in the primary (`card.facts-value`), which may hold a badge,
  a link or one button (a fix). Where its own room is under 24rem, each label stands over its
  value. No screen writes its own `dl` grid.
- **A row of totals tiles** (`Totals`, `Tile`, #3673): a tile's label in the label role, in
  capitals; its value a size larger in figures of one width (`card.tile-value`); an optional
  line under it in the quiet tier. The tiles share the row (each at least about 170px) and wrap
  to more rows, never sideways; a tile is edged as a card is, with no ground of its own
  (`card.tile`). A value that changes keeps its tile's size, and `live` totals say their
  changes politely (`role="status"`), at most once a second (`ui/said.ts`). Facts and tiles stand
  on a page as often as in a card, so a theme reaches them by their own attributes.
- **A Notice is drawn by the notice family and is a Notice by `Notice.tsx`.** Its box is the
  family's `root`, its tone the variant and where it stands its `data-placement`: `pane` (its
  pane's row; in a chat's pane the band draws it as one compact row, `layout="row"`, `paneFrame.css`,
  #5215), `inbox` (in a list: in the Notices centre and the window's own lines there, a card of its own,
  below; in the same layout, a row of the list's box under the title bar and on the page a window
  with no project shows) or `alone` (on a page, edged in the hairline, which takes the card's edge width: none in Purlis).
  Each way out is a button of the button family that says which it is (`data-way`: `go`, `fix`,
  `link`, `copy`, `dismiss`, a band row's `whole` and `close`, and its folded line's `show`). **A Notice holds Allow, so it is a consent
  surface**: its fill is one of the grounds an answer that grants is measured on, and its edge's
  width is no theme's. The Inbox's list (`data-scope="notice-list"`) and a pane's row
  (`data-scope="notice-row"`) are the family's layout.
- **A Notice in the Notices centre is a card in the Inbox card's style** (#5217). A one-line
  `head`: its mark, its `title` (what it names, whose it is, its label, or else its state), its
  state and age muted beneath (`sub`), led by the chat it came from where it names one (`from`,
  the notice family's `whose` part, #5694: a report draft a chat wrote), Go to chat a quiet link
  and its ×. Under it, what it says at
  the card's size, in purlis's plain words (no Notice in the centre carries a chat's own prose;
  `chatWords` went for want of a caller, #5363); and its ways. One primary, where it has an
  answer, every other way a quiet link. **A consent takes no primary** (ADR 0097 §6): Allow keeps
  `grants`, what stands beside it stays `secondary`, and its card keeps its full waiting edge
  (`data-consent`), where any other Notice's outline is softer, in its kind's colour. A sandbox
  block says it is one (`consent`), as an answer that `allows` says it of itself, and says
  "Waiting on you" only where it has something to answer. A pane's band and a Notice alone on a
  page do not take this layout.
- **Three tones** (#3678): `news` (nothing is wrong, the person is being told), `attention`
  (nothing is wrong, but the person must decide: a change a chat made to review, a grade plan
  that changed under them, a draft out of date) and `trouble` (something failed, and only then).
  Attention stands on its own ground in every theme and appearance (`attention.surface`, its words
  and mark `attention.text`), leads with a question in a circle (a card's mark, a line's first word, the band's
  mark in a chat's pane) and says "Needs a decision:" to a reader, so it is never told by colour
  alone; in forced colours it keeps a dotted outline, apart from trouble's dashes. Like every
  Notice it is a polite `status`, never an `alert`. The Inbox lists trouble, then attention, then
  news.
- **A badge is a word or two; a count is a number.** Both are pills in 11px words, a badge on the
  count's ground or a wash in the colour of what it says, a count at the strong weight. Your hosts
  say each host's status in a badge (R66): "Waiting for you" on the amber wash until it is
  confirmed, then "Confirmed" in the ok words. A count is heard as what it counts (`said`), and a
  tab's counts are one pill (`CountGroup`): its chats, and those that need the person in the
  needs-you colours (`hot`). A badge with a hint is the tooltip's target and a Tab stop. A number
  inside a sentence or a segmented option ("Chats 3") is words, not a count.
- **A separator is its own part, never a control's border** (#2804, #2877). Its colour is
  `separator.root.fill`; its width is the theme's `separator.width`, 0 for none. Both reach the
  menu's rule, the dialog's footer rule, the status bar's line, the rows of the Inbox's list and
  the hairlines between rows inside a surface (`--separator-fill`).
  One that only parts two marks on a strip is `decorative`, hidden from a screen reader.
- **Controls take no border in Purlis**: its theme draws the edge of the plain and the primary
  button, the text input and the textarea in no colour, at every size; Allow and danger, the
  consent variants, keep theirs, and a refused field keeps its danger edge, drawn as an outline over the box's own edge that no part reaches (the focus ring takes its place while the field holds the caret).
- **Stories hold them** (`Card.stories.tsx`, `Notice.stories.tsx`, `Badge.stories.tsx`,
  `Separator.stories.tsx`): an ask with Allow and Keep blocked, a pane's band minimised and expanded, the
  Inbox's list, a Notice alone, what needs a decision in all three places, every badge and count, the hosts' statuses, a badge's hint, and a
  line across a card and down a strip, each walked by the keyboard and checked by axe.
## The steps family

**A guide's list of steps is the steps family** (`app/src/ui/steps/`, #4480, on React Aria's
`Button`): `StepList`, how far along the person is, each step's state, and a way back to any step
reached. A step's variant is its state (`done`, `current`, `attention`, `skipped`, `waiting`,
`later`), told apart by its mark and its words as well as its colour. A `later` step can't be
pressed. Pool setup's Where and Pool steps are its first user (`PoolSetupTab.stories.tsx`).

## The checks family

**A list of things checked is the checks family** (`app/src/ui/checks/`, #4481): `CheckList` and
`CheckLine`. A line's variant is its verdict (`ok`, `missing`, `running`, `todo`, `warn`, `skip`),
told apart by its mark and its words as well as its colour. Nothing in it is pressed. Pool setup's
contract check and Try it use it.

## The code family

**Text to read and copy is the code family** (`app/src/ui/code/`, #4481): `CodeBlock`, a command or
YAML in a box with its Copy button, which says it copied or that it couldn't. The text can be
reached by the keyboard, so it can be scrolled there. See what this does in Pool setup uses it.

## The meter family

**A value drawn as a bar is the meter family** (`app/src/ui/meter/`, #3674, on React Aria's
`Meter` and `ProgressBar`, added to spec #2897's closed list by the operator on 2026-10-28). No
screen writes a `<meter>` or a `<progress>`, or either role by hand: `purlis/meter-family` is an
error everywhere, with no file off (Get project's clone progress moved in #3731).

| Wrapper | What it is |
| --- | --- |
| `Meter` (`Meter.tsx`) | **A labelled value on a known range**: a grade's score from 0 to 100. Its label at the start, its track, its value in figures of one width at the end (`valueLabel`, "86%"). Its `variant` says what it is in a comparison: `subject` (the accent), `baseline` (without it, the waiting colour) and `previous` (its previous version, the muted tier) |
| `MeterGroup` | **Meters that compare**: one scale (0 to 100 unless said) and one label column, so their tracks are one length and a longer bar is a larger value; named for a reader ("Latest grade") |
| `ProgressBar` | **How far something is**: done of N (`value`, `maxValue`, `valueLabel`), or going with no end known (`isIndeterminate`), its stripes moving, and standing still under reduced motion |

- **A reader hears a meter by its label, with its value** (`aria-valuetext`). A progress bar's
  value is said politely as it changes, at most once a second, in a status line beside it: its
  own children are presentational to a reader. Going with no end known, it says its label alone,
  so a new stage is heard, and the first value after it is a change that is said (#3731).
- **Told apart by more than colour** (WCAG 1.4.1): each bar's label and its value beside it.
  Each fill is held at 3:1 on its track on every ground (`meter.fill.*.fill` on
  `meter.track.fill`), by the theme engine: a theme that would lose a bar draws its default.
- **Forced colours** draw the fill in the system's highlight on a track edged in the words'
  colour and the previous version in the words' colour (`forced-colors.css`); the system's
  palette has no pattern, so there the baseline is told by its label and its value. The value is
  words, in the system's words' colour.
- **Drawn by its parts** (ADR 0103): the `root` (its `data-variant`), `label`, `track`, `fill`
  and `value`, in `meter.css`; a theme's parts reach the same attributes (`meter.*`, the next
  release's manifest version).
- **Stories hold it** (`Meter.stories.tsx`): the latest grade's three bars, a running grade's
  progress that moves, and one going with no end known.

## The pane preview family

**What a drag draws over the panes is the pane preview family** (`app/src/ui/pane-preview/`, #5291,
spec #5288, ADR 0113): `PanePreviewArriving`, a pane the drop would make or a side that would fold;
`PanePreviewLine`, on the strip, where a tab would go; `PanePreviewOpening`, the mark along a tab a
drag rests on until it comes forward (#5294); `PanePreviewSize`, the columns and rows a
terminal will take; `PanePreviewRefused`, the mark on the pane a refused place means; and
`PanePreviewHeld` and `PanePreviewWords`, what is held and what letting go would do, by the pointer.
Only the wrapper writes `data-scope="pane-preview"`: `purlis/pane-preview-family` is an error.

- **It draws, and decides nothing.** What a drop would do is the drop model's (`paneDrop.ts`), and
  where each part goes is the drag's (`paneDragLayer.tsx`).
- **None is read by a reader or takes the pointer.** A reader hears the drag from the window's status
  line, and the pointer has to reach what is under them.
- **It is placement mode's preview too** (#5293): the keys' target is drawn as the pointer's place.
- **Its parts** are `arriving`, `name`, `line`, `opening`, `size`, `refused`, `held` and `words`, in
  `pane-preview.css`; a theme's parts reach the same attributes (`pane-preview.*`, the next release's
  manifest version).

## The secret ask family

**A secret's one notice is drawn from the secret ask family** (`app/src/ui/secret-ask/`, #5080,
spec #5235, ADR 0112): `SecretAskBody`, `SecretAskFacts` and its `SecretAskLine`s, `SecretAskChat`
with `SecretAskCommand` and `SecretAskQuote`, `SecretAskField`, `SecretAskTarget`,
`SecretAskValue`, `SecretAskAllow` with `SecretAskScopeSaid` and `SecretAskToolLine`,
`SecretAskNeedRow` and `SecretAskFoot`. `SecretAskNotice.tsx` composes them for a Secret ask and
for a run's one notice, in the Inbox's card. Only the wrapper writes `data-scope="secret-ask"`:
`purlis/secret-ask-family` is an error.

- **Who says what is drawn apart.** The app's facts sit in the `facts` panel under *From purlis*,
  and nothing of the chat's is ever a child of it. The chat's command and purpose sit behind the
  `chat` rule, said to be the chat's, and every control or invisible character in them is drawn
  as its name (`Inert`), on one flow.
- **A value box has an edge on the card**: a box inside `SecretAskValue` takes the `value` part's
  edge (`border.strong` by default), except a refused one, which keeps its danger edge. The
  window's control edge may be clear on a raised card.
- **The Allow is one choice and one press.** The scopes the registry offers, narrowest first and
  chosen; what the choice does under it; the line Always allow writes before it is pressed. The
  press that grants is the button family's `grants` variant, never the primary look, and nothing
  in the notice takes the keyboard as it comes. The action is never disabled: a press with a field
  still empty says what is missing.
- **The action bar stays in reach**: `foot` is held at the body's foot as it scrolls, and the body
  scrolls a focused control clear of it.
- **Its parts** are `root`, `facts`, `facts-head`, `who`, `chat`, `command`, `control`, `value`,
  `scope-said`, `tool-line` and `foot`, in `secret-ask.css`; a theme's parts reach the same
  attributes (`secret-ask.*`, the next release's manifest version).

## The chat mark family

`ChatMark` (`app/src/ui/chat-mark`, ADR 0104 amended 2026-11-05, #5340) is a chat's mark on a tab that shows it, its row in the list, its pane's head, a folded pane's rail and the row of an ask that names it. It draws the chat's **Chat state** (ADR 0121 decision 17, #6012), which one function derives: `shownState`. A screen draws it through `ChatPersonaState`, which reads that function; `QuitWarning`'s list and Asks hand it the state that function gave.

- **Props:** `state`, one of the eight Chat states in the order that wins (`CHAT_STATES`): `needs` (Waiting on you), `stuck` (Stuck?), `failed`, `working`, `waiting` (Waiting on a chat), `idle`, `ended` (Done) and `cant-tell` (Can't tell), or nothing for `who` alone; `who`, `word`, `changed`. The three keys the mark had before #6012 keep their names, since a theme format only grows.
- **Parts:** `root` carries the state as its `data-variant`; `who`, `badge` and `word`. Each state has one glyph and one colour (`CHAT_STATE_SAID`) and no two share either, so a glyph tells each state as well as its colour.
- **The place sets** `--chat-mark-size` and `--chat-mark-ground`, nothing else.
- **`who` is its own part** so a control can stand in it (the grip, #5290), with the badge left beside it. `ChatPersonaState`'s `grip` puts one there.

## The grip family

`Grip` (`app/src/ui/grip`, ADR 0104 amended 2026-11-06, #5290, spec #5288) is what a pane or a chat is picked up by: the persona's mark in a chat's row and its pane's head, and a view's mark at the head of a view in a split. At rest it is the mark; under the pointer, or with the keyboard on it or on the row it stands in, six dots in three rows of two take the mark's place in the same box. Only the grip under the pointer changes, so a list keeps its marks.

- **Props:** `children` (the mark), `onPress`, and `label` ("Move" and what it moves), which makes it a control of its own, a React Aria `Button`: Tab reaches it, and a press, Enter or Space picks up (WCAG 2.5.7). With no `label` it stands inside a control that is pressable itself (a chat's row): it is no control and not heard, and its press picks up without the row's press. A drag from it is its row's or its head's drag either way.
- **Parts:** `root`, `mark`, `dots` and each `dot`. Only `dot` has a property (`fill`), in the states `hover` and `focused`, which are keyed on `:hover` and on the keyboard on it or what holds it.
- **No button ground**, in any state, in any stylesheet: `controls.guard.test.ts` holds it, and the root has no property a theme could set. The cursor is an open hand, closed while pressed and while a drag holds it.
- **The place sets** `--grip-mark` and `--grip-reach` (a pane's head: the mark's size and the 24px target, which reaches past the mark and moves nothing), or neither.
- **Where none is drawn:** a one-job program, an empty pane and a view alone in its tab, which moves by its tab. The row's menu, the tab's menu and the palette keep **Move…**, and ⌘⇧M picks up what has the keyboard.

## The message family

**A message of the Inbox's views is a row of its view, not a boxed card** (`app/src/ui/message/`,
#5649, spec #5638, ADR 0119 decision 6): `MessageRow`, holding `MessageTop` (its title, the chat
or the thing, cut with an ellipsis, and its time at the right), `MessageMeta` (what it is and
where, quiet), `MessageBody` (what it says), `MessageLinks` (its ways out as text links in the
shared small `ButtonRow`) and `MessageForm` (what answers it in the row: a reply box, an ask's
answers, what a way out opened). Asks, Mine, Project and Notices draw every line with it. No
screen writes `data-scope="message"`: `purlis/message-family` is an error.

- **Variant A of the variants prototype** (`prototype/inbox`, the person's pick): no box and no
  edge on the view's ground; a hairline between two rows, from past the mark; the hover fill under
  the pointer or the keyboard.
- **Its mark says its state** (`tone`, the variant): `waiting`, `done`, `failed`, `report` or
  `plain`, a round icon in that colour on a seventh of it. It is decoration (`aria-hidden`): the
  meta says the state in words (WCAG 1.4.1).
- **The × takes the time's place** (`dismiss`): hidden until the row is under the pointer or holds
  the keyboard, and the time hidden then; where nothing hovers, the × always shows. The keyboard
  reaches it after the row's links and form.
- **A consent keeps a waiting edge** (`consent`, ADR 0097 §6, the ruling on #5222): a bar in the
  waiting colour at the row's start, which no theme property reaches. Its answers stay buttons in
  `MessageForm`, Allow in the `grants` look.
- **A New row has an accent bar** (`isNew`, `data-new`; #6002, ADR 0121 decision 10): a bar in
  the accent colour at its start, just inside a consent's waiting edge where it has one, and
  "New:" said first to a reader. A shadow, so nothing moves as it goes. What is New is
  `seen.ts`'s: a row is seen once in sight for about a second (`useSeenRow`), or once its chat is
  on screen in a focused window.
- **One link leads** (`data-lead` on it): an ask's Answer, a Notice's one answer, in the accent's
  words. Every other way is a `quiet-link`.
- **A long ask shows three lines** and "Show all · N lines" (`ChatText`'s `foldAt`, `ASK_FOLD_LINES`).
- **Drawn by its parts** (ADR 0103): `root`, `mark`, `main`, `top`, `title`, `time`, `meta`,
  `body`, `links`, `form` and `dismiss`, in `message.css`; a theme's parts reach the same
  attributes (`message.*`, the next release's manifest version).
- **As a list's row** (`as="row"`, #3123): Asks and Updates draw each line as a row of the list
  family's list of rows that act (`ListRowDrawn`), whose element is the message's own root. The
  list gives it a row's role, its keyboard (↑ ↓ between rows, ← → through its own links, answers,
  boxes and ×) and React Aria's states; it is named by `aria-labelledby` where given, else by its
  `textValue`; `onDrawn` hears each drawing of it once it is in the document. React Aria holds its
  parts in a cell drawn `display: contents`, so every rule that reaches a part as the root's child
  reaches it through that cell too; the keyboard's ring on the row itself is drawn inside it, in
  the window's focus ring tokens, as a list's row draws its own, beside the hover fill.

## The pane foot family

**The line under a chat's terminal is the pane foot family** (`app/src/ui/pane-foot/`, #5202,
spec #5191, ADR 0111): `PaneFoot`, a group named for its chat, holding its `PaneFootStart`
(harness · workspace, where it came from, its skills warning and its sandbox) and its `PaneFootEnd`
(the gauge); `PaneFootSeparator`, the `·` between two parts; and `PaneFootGauge`, a
labelled number, `ctx 62%`, with its fill as a thin bar under it where it has one. No screen
writes `data-scope="pane-foot"`: `purlis/pane-foot-family` is an error.

- **Look A** (the person's pick on the prototype): parts joined by `·`, muted words, a gauge's
  value a step up in the secondary tier and in figures of one width, on the terminal's fill with
  no ground or line of its own.
- **The tab strip's height at the density in force** (a control's height and 3/14 rem, the sum a
  tab is drawn at), so a compact appearance makes it compact. It is drawn whether or not the
  gauge is known yet, so the terminal never moves when the first usage arrives.
- **The bar is a mark**: its number says the value, so the track is hidden from a reader and the
  value is never told by its length or colour alone (WCAG 1.4.1). Its fill is in the core's tone
  for the value (`ok`, `warn`, `bad`), as is a warning on a number with no bar (`cache`), which its
  tooltip says in words too. A chat that has ended keeps its last numbers in `ended`, the muted tier,
  beside a badge that says so. In forced colours the fill is the system's highlight on a track
  edged in the words' colour.
- **Drawn by its parts** (ADR 0103): `root`, `start`, `end`, `separator`, `gauge`, `label`,
  `value`, `track` and `fill`, in `pane-foot.css`; a theme's parts reach the same attributes
  (`pane-foot.*`, the next release's manifest version).
- **Every press is a row** (#5204): `paneHead.FOOT_ROWS` says which row of the chat's list each part
  runs, and `chatList.test.ts` holds each to the chat's ⋯ and the palette. A part whose row is
  missing or cannot run now is its words alone. Its parts are inline buttons, so they stay words on
  the line; the skills warning takes the button family's `warning-soft` drawn inline, with no ground.
- **A narrow pane drops its parts in one order** (#5206, `paneHead.FOOT_DROPS`, widths measured on the
  default theme): `cache`, the skills warning to its ⚠, `↳ from` with what the studio started the
  chat for and its work item, then the workspace. The harness's name, ⚠ and `ctx` never go, and a
  longer name is cut at its end. `paneLinesOf` answers the head's chips, its actions and the foot's
  parts, so Split down's drop comes from there too.
- **Its chips are held like the head's**: `PANE_FOOT_CHIPS` in `controls.guard.test.ts`, read off
  `PaneFootStart` and `PaneFootEnd`, each at the chrome height.

## The disclosure family

**A button that shows and hides the region under it is the disclosure family**
(`app/src/ui/disclosure/`, #3675, on React Aria's `Disclosure`, `DisclosureGroup` and
`DisclosurePanel`, added to spec #2897's closed list by the operator on 2026-10-28): Add
source's More, a file to read under its risk mark, a deleted skill's last text.

| Wrapper                         | What it is                                                                                                                                                                                       |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Disclosure` (`Disclosure.tsx`) | **One fold**: its trigger (`label`, a quiet ghost of the button family, leading with the twist) and any mark after it (`trail`), then its panel. A heading of a `level` where it heads a section |
| `DisclosureGroup`               | **Folds that open one at a time** (an accordion), or together (`allowsMultipleExpanded`); the owner may hold which are open by each fold's `id`                                                  |
| `Twist`                         | **The chevron alone**, for a fold that is not a disclosure: a tree's group of rows, a table's row, a menu's item. `open` turns it                                                                |

- **The trigger says `aria-expanded`** and controls its panel; the panel follows it in the
  reading order, named by it (`role="group"`, or `region` for a large panel a reader jumps to).
  Return and Space on the trigger open and fold it, and the keyboard stays there.
- **Opening moves only what is under it**: the trigger's row keeps its size, and the twist turns,
  which is a transform, and never swaps for another chevron. Folded, the panel takes no room.
- **It starts folded unless told** (`defaultExpanded`), and the owner may hold its state
  (`isExpanded`, `onExpandedChange`).
- **Its twist leads its words, or follows them** (`twistAt="end"`, #2919): a settings page's
  rarely used group ("Branches · 1") draws it after its title and count, as every disclosure
  chevron in Settings trails (#2799 rule 1).
- **A refused field inside a fold opens it** (`ui/reveal.ts`): the field family's form sends
  `purlis:reveal` at the first field it refused before the keyboard goes there, and every fold
  around it opens at once, in a dialog's body and on a page alike. The refusal is heard once, by
  the field's own error line (#3291); the fold says nothing of itself.
- **Drawn by its parts** (ADR 0103): the `root`, the `trigger` row, the `twist` and the `panel`,
  in `disclosure.css`; a theme's parts reach the same attributes (`disclosure.*`, the next
  release's manifest), and the trigger's look is the button family's.
- `purlis/disclosure-family` (an error) refuses a fold's chevron swapped by hand
  (`open ? <ChevronDown /> : <ChevronRight />`) and `data-scope="disclosure"` by hand. The folds
  #1916 named take the twist: the finished tasks under a chat, a dispatch's brief and report,
  and the tasks menu's Finished.
- **Stories hold it** (`Disclosure.stories.tsx`): Add source's More in its dialog, folded and
  open, a refused folder in it opening it, and a review's files under their risk marks, each
  walked by the keyboard and checked by axe in Chromium and WebKit.

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
  and every tooltip beside its target (a control's, a row's, a menu row's) keeps its width: it is
  measured as it opens, takes the side with room for all of it, and stands under its target where
  neither side has (#3328, #3364, `ui/tooltip/side.ts`).
- **A strip's tooltips stand below it** (#3361): a strip is a row of targets, and a tooltip at a
  target's end would sit on the row, over the next. A target inside an element marked
  `TIPS_BELOW` (`data-tips="below"`; the tab strip's row) has its tooltip below it, or above
  where the window has no room, at the same 10 px. The strip says so, never a call site, and it
  is read from where the target stands in the DOM (`placementFor`), so a menu or a popover opened
  from the strip, portalled out of it, keeps R166's placement.
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
- **A hint that comes keeps its target** (#4659, #4671). A target stands in its tooltip's trigger
  with or without a hint, a menu's `MenuItem` and the button family's `Tipped` included, so a hint that comes or goes draws nothing afresh
  and the focus stays. One that comes while the keyboard is on the target opens at once, because
  no later focus would open it: a menu's row or a button that turns unavailable says why where
  the keyboard is. The pointer's way stays coming onto the target: one already resting on it sees the hint when
  it next comes back.
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
the old `.ui-menu*` and `.ui-popover` classes: `eslint.config.js` refuses both
(`purlis/menu-family`, an error). A select's list stands on
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
| `HungPopover` | A card of notices hung from what they are about (a title-bar chip's, #3648): the `notice` surface centred under it with its `point` (a part of its own, never a held popover's `arrow`), shifted only at the window's edge, and following what it hangs from as that scrolls; it comes only on its dot's press, which hands it the keyboard, never by itself (ADR 0121 decision 1, #6004: what it holds waits behind the dot, with a "new" pip until seen); a press anywhere else hides it, and its owner's Esc dismisses what it says. No timer |
| `PopoverFilter`, `PopoverRule`, `PopoverList`, `PopoverFooter` | A popover laid out as a list (R165, `layout="list"`): the filter row with its rule under it, the list that scrolls while they stay in view, and the footer that says its keys, "↑↓ move · ⏎ go · esc close", each key in a mono cap |

- **Placed under its trigger, lined up with its start or its end** (`align`), and never nearer
  the window's edge than 8 px; it turns to the other side where there is no room. A row's ⋯
  menu, as a setting's row has it (R59), lines up with the end of its button.
- **Closed by a click outside, by Escape and by choosing a row**, and the keyboard goes back to
  where it was, or where the owner's `onClosed` puts it (a row that went into Settings sent it
  there). A press outside still reaches what it was on: **a menu is not modal**, so the rest of
  the window is not hidden from a screen reader and a menu on a strip is no question. A preview
  the pointer's rest opened (a pane head's tasks menu) takes no keyboard (`takesNoKeyboard`), and
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
- **A row is named by its words alone, and they are its only visible words a reader is given**
  (WCAG 2.5.3 Label in Name, #6043). Its key is drawn by `menu.css` from the `shortcut` part's
  `data-key`, hidden from a reader, and said by the row's `aria-keyshortcuts` (the registry's,
  `menuRowKey`, or the caller's), as a button's key is (R33). What `MenuWords` draws under the
  words is kept out of the name (`aria-hidden`) and handed to the row as its description (React
  Aria's `description` slot), so it is heard once, after the name. The row is named by its
  content; **a row with words under it is never named by `aria-label`**. Why not simply
  `aria-hidden` on a drawn `<kbd>`: axe's `label-content-name-mismatch` (ACT 2ee8b8) applies to
  a row named by `aria-label` or `aria-labelledby`, and takes every visible word inside it,
  `aria-hidden` or not, as its label (axe 4.14's `visibleVirtual` asks what is on screen, never
  what a reader is given), so a row whose name leaves out a word drawn in it fails however that
  word is hidden. Words drawn by the stylesheet are not in the row's text, and a row named by
  its content is not a case for the rule; either way its name is the label a person sees and
  says ("click Rename"). `Menu.stories.tsx`'s *Named by its words* runs the rule on an open menu
  in Chromium and WebKit.
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
| `List`, `ListRow`, `ListRowSection` | **A list of rows that act** (a grid of one column): a panel's todos, a vault's rows, the Inbox's asks and updates. A row runs something (`onAction`), may be chosen, and may hold its own buttons, which ← and → reach, and text boxes, which keep their own keys (below). **A long one draws only the rows in sight** (`drawnInPart`, #6013), as a long tree does: React Aria's own `Virtualizer` with its list layout, each row measured as it is drawn, so the keyboard still walks every row; the list is then its own scroll, and its owner gives it a height (All chats) |
| `ListRowDrawn` | **A row another family draws** (#3123): its element is that family's root, with that family's parts and look (a message of the Inbox's views, `MessageRow as="row"`), and the list gives it a row's role, its keyboard and React Aria's state attributes, writing no part of its own. It is named and described by its own parts (`aria-labelledby`, `aria-describedby`, which React Aria hands no row), and `onDrawn` hears each drawing of it once it is in the document: React Aria draws a list's rows a commit after the components that make them, so whatever reads a row as it is drawn (where its answers stand, #3167) is told there, never in an effect of its own |
| `RowLead`, `RowLabel`, `RowDescription`, `RowTrail`, `RowBelow` (`Row.tsx`) | A row's slots, the same in a list, a tree and a table: its marks, its words (one line, cut with an ellipsis), the one line about it in the quiet tier, its end (a count, its own actions), and a line under its words inside it (what a search matched in its body, why the core refused it). **In an option, its words are its name and its one line its description**, so a reader hears the name alone and then what it is, never the row's content run together. **An option is named by its content, and every part of it but its words is kept from a reader** (#6043): `RowLead`, `RowTrail`, `RowBelow` and the description are `aria-hidden` in an option, and anything else a caller draws in one (a gallery card's pictures) is `aria-hidden` too. **What else it draws that says something is heard, after its name, as its description** (#6075): its one line, then its `RowBelow` (read whole, whatever its clamp cuts), then a `RowLead` given words to say (`said`, a search hit's "Line 42"; a mark with none says nothing), then what React Aria adds (its hint for dragging). React Aria's `useOption` sets `aria-describedby` to its `description` slot alone and takes no other id, so the family fills no slot: each part hands its id to its option (`OptionHeard`, `Row.tsx`), and `ListOption` draws its element (React Aria's `render`) with those ids before React Aria's. A reference from `aria-describedby` reaches content kept from a reader, so each line is heard once, and the name stays the label alone. Named by `aria-labelledby`, its visible words would be more than its name, which axe's `label-content-name-mismatch` refuses whatever is `aria-hidden` (see [the menu family](#the-menu-and-popover-families)); the list's stories hold the rule in Chromium and WebKit with nothing turned off. **A `RowBelow` with `clamp` and `tipWhenCut` says its whole words in its row's tooltip while the clamp cuts them** (#4711), under the pointer and while the keyboard is on the row, and nothing while they fit, the tooltip being the row's description while it is shown (a list's row is named by its `textValue` alone). One to a row, and a list's row's alone (`ListRow`, `ListOption`) |
| `ListCardRow`, `RowMenuButton` (`CardRow.tsx`); `RowSummary`, `RowActions` (`Row.tsx`) | **A card row** (#3669, the sidebar Skills view's variant C): a row of a `List` drawn as a small card (`variant="card"`), as tall as its lines. Its first line is its name, its tag, its own actions (Use) and its **⋯**, which opens the row's own menu (`menu`), the one a right-click and Shift+F10 open, at the ⋯: pressed by the pointer the menu takes the keyboard with no item chosen, from the keys on its first item, and either way the ⋯ says `aria-expanded` while it is open and has the keyboard back as it closes (`askForAMenuFrom`, #3769); then **two lines about it** (`RowSummary`), its description cut cleanly after the second line, or a reason in its place in the waiting colour (it needs the person) or the quiet tier (it is left out); then its **action line** (`RowActions`), its one fix. The whole of its summary is its tooltip. ← and → reach Use, the ⋯ and the fix; a press on it runs it. A state changes a fill or a ring, never its height; it fits a 260 px side view |
| `ListFoldSection`, `ListRun` (`Fold.tsx`) | **What folds** (#3670, in a `List`): a group whose name folds its rows, and a run of rows folded to one quiet line. A group's name is a row the keyboard reaches, with the tree's twist (`TwistMark`), its count at its end and an optional `waiting` tone (Needs you); Enter, Space, → and ← fold and unfold it, `aria-expanded` on its cell says which (a grid's row takes none), and it may start folded, its owner holding what is folded (`folded`, `onFoldedChange`). A run (`ListRun`, folded by default) is one row in the quiet tier, a mark and "N …"; pressing it unfolds its rows in place, then its closing line ("They ship with purlis…", with what may be done). A row of the run that must show (`standsOut`: it needs the person, it clashes, it matches the filter) is drawn on its own, outside the fold. Folding moves only what is under it, and a fold's row runs nothing of the list's `onAction`. In a list that chooses, a fold's row and a run's closing line are never chosen and Select all leaves them out; they are still reached and still fold (#3769). Folding a section unmounts what is under it: a run inside it keeps its fold only where its owner holds it (`folded`), and the family keeps no state of its own (#3769) |
| `AimedList`, `AimedOption` (`AimedList.tsx`) | **A list a field aims at**: a combobox's list drawn in place, the palette's actions, projects and files. The field keeps the keyboard and moves the aim (`aria-activedescendant`); the aimed row wears the keyboard's ring inside it and the aim's fill (`control.aimed`); a press runs a row. Its owner gives it `count` and draws each option with `option(index)`: past 250 rows (`DRAWN_WHOLE`) it draws only the rows in sight and the aimed one, each with `aria-posinset` of `aria-setsize` (#5501) |
| `useDrawnRows`, `useRowPlace` (`virtualRows.tsx`) | **A long list draws only the rows in sight** (#5501): the one wrapper of `@tanstack/react-virtual`. A list past `DRAWN_WHOLE` rows keeps the whole list's height and places each drawn row in it; the aimed row is always drawn, so `aria-activedescendant` names an element that is there, and each row says its place in the whole. Its geometry (padding, gap, row height) is read from the list's CSS. `AimedList` and the title bar's `SwitcherList` stand on it |
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
  **Typing to find a row can be turned off** (`disallowTypeAhead`, the Inbox's asks and updates,
  #3123): a letter then moves nowhere, and a Space after it folds a fold's row at once.
  **A control in a row that takes the keyboard makes its row the list's own** (#3123), however it
  got there (a script moving the keyboard into a form just opened, a screen reader's cursor), so
  the list never pulls the keyboard back to another row a frame later; a key's own move and a Tab
  into the list stay React Aria's.
  **Tab from a row leaves the list at once, and Tab from a control in a row walks that row's
  controls, then leaves** (#6071, the WAI-ARIA APG grid, where Tab moves out of the grid and the
  arrows reach a cell's widgets, and Tab moves between the widgets of the cell being worked in).
  From a row, Tab and Shift+Tab each leave the list in one press, past the row's controls either
  way. From a control in a row, Tab goes on to the row's next control and Shift+Tab back to the
  one before; from its last control Tab leaves the list, and from its first Shift+Tab leaves it
  the other way, never landing on a row. So Tab and Shift+Tab are one rule turned round. React
  Aria's collection moves a Tab out of the list from wherever it is in it (its one Tab stop), so
  when the row has a control of its own to go to, the family's row keeps the Tab from the
  collection, and the browser moves there (`movesWithinItsRow` in `rowKeys.ts`, heard in `List.tsx`'s `rowElement`), for a text box as
  for a button. Seen in `List › One Tab Stop Coming Back` and
  `List › Tab Walks A Rows Controls Then Leaves`.
  **Shift+Tab back into a list lands on the row the keyboard left, and → re-enters its controls**
  (#6033, the WAI-ARIA APG grid's one Tab stop, which gives focus back to the last focused row,
  never to a control in it), whether or not that row holds controls. The browser's Shift+Tab
  lands on the list's last control, and React Aria moves the keyboard to its row only when that
  control is in another row; when it is in the row the list keeps (that row's controls are the
  list's last), the family gives the keyboard to the row itself (`backToTheRow` in `List.tsx`). A
  list the keyboard has not been in lands on its last row, as React Aria chooses. Only a Tab does
  this (a Tab another component handles itself, a dialog's focus wrap, counts as one): a control
  the pointer presses or a script focuses keeps the keyboard. Seen in
  `List › Shift Tab Back To A Row With Controls`.
- **A text box in a row keeps its own keys** (`rowKeys.ts`, #3123): → reaches it from its row as
  it reaches a button; in it ← and →, Home and End, PageUp and PageDown move its caret, a letter,
  Space and Enter are typed or are its own (its handler hears them, the row and the list never do,
  so Enter runs no row), and ⌘A selects its words; ↑ and ↓ move its caret between a box's lines,
  the lines it draws (a line its words wrap on to as much as one after a line break, measured
  unseen in a copy of the box laid out as it is, #5993; a caret at a soft wrap, which the box may
  draw at the end of one line or the start of the next and never says which, is taken to be on
  the line its key moves from, so it never leaves from a line between, at the cost of one more
  press from a wrap drawn on the first or last line, #6045), and go to the row above or below from its first or last
  line or from a box of one line; Tab moves on to the row's next control while it
  has one, as from any control in a row (#6071, above); Esc gives the keyboard back to its row,
  keeping what was typed, and Esc on the row is the list's (`onEscape`). **While it composes**
  (Chinese, Japanese, Korean input), every key is the input method's alone (#5993): composing, or
  key code 229 (WebKit's Esc once the composition ends), the list keeps it from the row and from
  everything above, so Esc cancels the composition and the keyboard stays. A chord with Enter (⌘⏎, a
  form's send) goes on up to whatever answers it. The arrows are kept by the list's own capture
  handler (`data-part="keys"`), the rest by the field family's boxes once their own handler has
  heard them, where React Aria's row hears them. **A radio group in a row keeps its arrows too**
  (a secret ask's How to fill): → and ↓ choose the next radio, ← and ↑ the one before, round at
  the ends, as the WAI-ARIA radio group does. **A select or a combobox in a row keeps ↑ and ↓**
  (#5993): React Aria's row takes them from whatever is in it (a menu's button, whose arrows go to
  the rows above and below), so the family's row lets them by for a button that opens a list
  (`aria-haspopup="listbox"`) or a combobox, whose own handler opens it (`List › A Select In A
  Row`).
  **A list that chooses several** (`selectionMode="multiple"`, a `Checkbox` in the row's lead with
  `slot="selection"`) takes a click or Space to tick a row, Shift-click and Shift+↑ ↓ for a range,
  and ⌘A (Ctrl+A off macOS) for every row, which a list that draws its rows a screen at a time
  answers with every item of its data, not only the rows drawn (`"all"`, the Library's Select
  mode, #5332). Esc clears what is chosen, or does what `onEscape` says: a list in a mode of its
  own leaves it.
- **A row's menu and its tooltip are the row's own** (`menu` and `tip` on a row): a collection
  draws its rows itself, so no `ContextMenu` or `Tooltip` can be put around one. The menu opens
  on a right-click, or by Shift+F10 or the menu key while the row has the keyboard, on its first
  row (`useContextMenu`, `askForAMenu` in `ui/menu/ContextMenu.tsx`); the tooltip shows under the
  pointer after the window's delay, at once while the keyboard is on the row, is the row's
  description while it is shown, and Esc takes it down (`RowTip.tsx`). As every tooltip, it meets
  WCAG 1.4.13 (#5038): the pointer can cross onto it and it stays, going a moment after the
  pointer has left both; Esc's dismissal lasts until the pointer rests on the row again or the
  keyboard comes to it again; and it stays where it opened while it is up, never drawn afresh
  (#3364).
- **A drag is never the only way**: whatever dropping a row does, its menu or a key does too
  (Shift+Enter on a search hit hands it to a chat).
- **Drawn by its parts** (ADR 0103): the list is `data-scope="list" data-part="body"`, a row the
  family's root (`data-part="root"`, its `data-size`) with React Aria's state attributes, its
  slots and a group's `header` (`data-named` for a name from data: a day, a file). `ui/list/list.css`
  draws the family default; a theme's parts reach the same attributes.
- **A card row is the row's `card` variant** (#3669): on the raised ground at rest (Purlis puts it
  on the window's, `list.root.card.fill`), the pill under the pointer, the wash ringed all round
  when chosen, and its own room whatever the list's size. A row of one line is the `row` variant;
  a theme's `list.root.*` without a variant reaches both. Its parts are `summary` and `actions`.
- **A fold is a row of its own** (#3670): a group's name that folds is the `header` part, a row
  with the pointer's pill and the keyboard's ring, its count the `count` part; a run's quiet line
  and its closing line are the `run` part. → and ← reach them through an element around the list
  that draws nothing (`data-part="keys"`, as a tree's), heard before React Aria's row.
- **Stories hold it** (`List.stories.tsx`, `CardRow.stories.tsx`, `Fold.stories.tsx`, and
  `Message.stories.tsx` for a row another family draws): a list to choose from under its filter, a
  list a field aims at, rows that act with a row's own button and its menu, a text box in a row
  with typing to find off, a grid of cards, the Skills view's card rows and its groups folded and
  unfolded, a filter finding a folded built-in, messages as a list's rows, and an empty one, in
  Chromium and WebKit, with axe.

## The table family

**Every table is the table family** (`app/src/ui/table/`, #2911, ADR 0104). No screen draws a
table's element (`<table>`, `<tr>`, `<td>`, …), says a table's or a grid's role, or wears the old
`.ui-table` classes: `eslint.config.js` refuses all three (`purlis/table-family`, an error).
Two members, by what a cell holds:

| Member                                                                                                                                                      | What it is                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Table`, `TableHeader`, `TableColumn`, `TableBody`, `TableRow`, `TableCell` (`Table.tsx`)                                                                   | **A table of data** (React Aria's `Table`, the WAI-ARIA grid): rows of words, a cell holding at most a button, a link or a switch. The Dispatches tab, a workspace's repos in Saving, a skill's runs, totals and history, the Skill studio's harness plugins and skills, a vault's secrets                                                                                                                                  |
| `TableBar` (`TableBar.tsx`)                                                                                                                                 | **The bar over a table that chooses** (the settings prototype's hosts table, R65): its filter (`ListFilter`, from eight rows, R164) at its start, and at its end what the table offers, or how many rows are chosen and what can be done to them all. The filter never moves when a row is chosen, and the count is said to a reader as it changes                                                                          |
| `ControlTable`, `ControlHead`, `ControlColumn`, `ControlBody`, `ControlRow`, `ControlRowHeading`, `ControlGroupHeading`, `ControlCell` (`ControlTable.tsx`) | **A table of controls**: a plain table whose cells hold fields, selects and numbers, or whose rows are dragged: the Dispatch limits, the dispatch grants, the Skill studio's skills. The grid would take ← and → from a field's caret and ↑ ↓ from a select, so each control keeps its own keys and is its own Tab stop. `stacks` makes it a form where its frame is narrow (below); `fixed` keeps the columns a view sizes |
| `cellsOf`, `tipInTable` (`table.testkit.ts`)                                                                                                                | A test's reading of a data table: a row's cells (its `rowheader` and `gridcell`s), and a cell's tooltip read as the keyboard reaches it along its row                                                                                                                                                                                                                                                                       |

- **The keyboard of a table of data is the APG grid's**: one Tab stop, the row last focused; ↑
  and ↓ move a row and stop at the ends, ← and → a cell and into the button a cell holds, Home and
  End to the first and last row from a row and to a row's ends from a cell, Ctrl+Home and Ctrl+End to the table's, Page Up and Page Down a page; typing
  finds a row by its `textValue`. A table that chooses (`selectionMode="multiple"`) draws the
  choice family's `Checkbox` at each row's start and one over them for every row, a bar when only
  some are chosen (React Aria's `selection` slot, "Select"
  and "Select All" to a reader); Space chooses the row the keyboard is on. A column with
  `allowsSorting` sorts by a press on its name (`sortDescriptor`, `onSortChange`, `aria-sort`),
  its arrow keeping its room while another column sorts, so sorting moves no name. `onRowAction`
  (or a row's `onAction`) runs a row on Enter or a press.
- **A view chooses for a column** (#3671, `CellWords.tsx`), on `TableColumn` and `ControlColumn`
  alike, so a column of short facts (a date, a level, "Works on", "3 of 3") keeps one line where a
  long column beside it takes the room, and no row grows tall word by word:
  - `cut`: its cells keep one line, cut with an ellipsis where the column is narrower, and the
    whole words are the cell's tooltip, only while they are cut;
  - `clamp={2}`: its cells wrap to two lines at most and are cut after them, whole in the tooltip
    (a description column, "What it does");
  - `size`: `fit`, as narrow as its widest cell on one line, or `fill`, the room the others leave;
  - `width`, `minWidth`, `maxWidth`: a length in px or CSS, the column's width and its words' least
    and most (a `cut` column with a `maxWidth` is cut there).

  A cut cell's tooltip opens under the pointer after the window's delay, and at once as the keyboard
  reaches it: the grid's arrows on a table of data's cell, and in a table of controls the cut words
  themselves, which are a Tab stop while they are cut, as a column's shortened name is. Esc takes it
  down. The words stay the cell's, whole, so a screen reader hears them all. React Aria's own
  `width` is read only by a column-resizing table, so the family's widths are its own. A column that
  chooses nothing draws exactly as before. A table of data where its frame is narrow keeps its
  one-line columns and scrolls sideways; it stacks only where the view asks (`stacks` on `Table`,
  with `heading` on each `TableCell`), as a table of controls does.
- **A row's menu** (`menu` on `TableRow`, `rowMenus` on the table): the row's actions under one ⋯
  in a column of its own at the row's end, named "More for" the row, aligned on every row (a row
  with none keeps its cell); a right-click on the row, Shift+F10 or the menu key open the same
  menu, and a row with a menu and no action of its own opens it when it is run (Enter, or a
  press), on its first row, as from the keys; Escape hands the keyboard back to the row. A row's
  `menu` needs `rowMenus` on its table: without the column the row takes no menu, and says so in
  a development build. In a table that chooses, a press on a row runs it while no row is chosen
  and chooses it once one is (React Aria's toggle with an action). This is the settings rows'
  one action column (R65 and the aligned form's ⋯ row menu, #2919).
- **Screen-reader names and headers**: a table is named by `aria-label`; the column with
  `isRowHeader` names each row, which a reader hears with every cell; a column of buttons is
  named for a reader alone (`hideName`, "Actions", "Reveal"). In a table of controls the row's
  heading is `ControlRowHeading` (`scope="row"`), a group's name `ControlGroupHeading`
  (`scope="rowgroup"`), and each control names its column and its row in full.
- **Empty**: `empty` on `TableBody` is what a table with no rows says, across it in the quiet
  tier; a view that has no rows at all draws its `EmptyState` instead of the table, and a filter
  that leaves none says so in a line (`noMatchSaid`).
- **Nothing moves when a state changes** (#2800). Under the pointer a row that opens or is chosen
  takes `surface.hover`; chosen, or the row the view is about (`current`, `aria-current`), the
  wash (`list.selected`), told by its check or its place as well as its colour; the keyboard's
  ring is drawn inside the row or the cell (`--focus-ring-inset`, [ADR 0104's
  amendment](adr/0104/2026-10-22-a-row-draws-its-focus-ring-inset.md)), so the frame that scrolls
  never clips it. A table's words are data, in the primary tier; a column's name is the label
  role (11/600, capitals, the muted tier, R74); a row that cannot be used is in the faint tier.
  The bands are square: a table's row is drawn across its cells, so its ground stays the row's
  part (`table.root.*.fill`) for a theme to reach.
- **Purlis draws first** (the settings prototype's hosts table): the table stands in a rounded
  card (`frame`, `surface.base`, `radius.lg`) that scrolls it sideways, a table of data no
  narrower than 560px; cells 8 by 12 apart (`space.4`, `space.5`); **no rule between rows**
  (#2804): the rows' room, the pointer's ground and the chosen wash carry the bands, and the
  head is set apart by its capitals and a little more room under it, never by a line or a zebra.
  The rule is its own part (`table.rule`, a line at each cell's top and under each column's
  name), which the family draws as none and Purlis Classic fills with `border.subtle`. A
  number's column is at the end in figures of one width (`align="end"`); a column of checks or
  of menus is as narrow as what it holds; the check's target is 24 px with its box in the middle of it.
- **A table of controls stacks into a form where its frame is narrow** (`stacks`, #1859): under
  32rem the columns' names are put away, each row's heading takes a line, and each cell sits under
  its column's whole words (`heading` on `ControlCell`, drawn from `data-heading`), as many side
  by side as fit; the row's actions and a cell across the columns (a question about the row
  above) take a line of their own, and a cell with nothing in it is not drawn. Nothing is cut and
  nothing scrolls sideways. 32rem sits between Settings' body at a 1280px window, where the table
  stays a table, and at the narrowest window with both sidebars open, where it stacks; CI's runs
  of `settings-tables.e2e.ts` and `settings-limits.e2e.ts` hold both ends in the real WebView
  (#1865). It is the one length the stylesheet writes, because a container query's condition
  cannot read a token.
- **Drawn by its parts** (ADR 0103): the `frame`, the table (`body`, `data-controls` on a table of
  controls), a column's name (`head`), a row (the family's root, with React Aria's state
  attributes), a `cell`, the check (`select`) and the `bar`; inside a cell the list's slots
  (`RowLabel`, `RowDescription`, …) draw in the table's scope. `ui/table/table.css` draws the
  family default and a theme's parts reach the same attributes (`table.*`, the next release's manifest version).
- **Stories hold it** (`Table.stories.tsx`): a table of hosts that chooses, sorts and opens a
  row's menu under its bar; a table of data with buttons in its cells; an empty one; and a table
  of controls, wide and stacked; in Chromium and WebKit, in Purlis and Classic, with axe.

## The tree family

**A tree is the tree family** (`app/src/ui/tree/Tree.tsx`, #2911, ADR 0104 and [its
amendment](adr/0104/2026-10-25-the-tree-family.md)), on React Aria's `Tree`. No screen says a
tree's role (`tree`, `treeitem`, `treegrid`), wears the shared tree's old classes (`.ui-tree`,
`.ui-tree-row`, `.ui-tree-twist`) or its `data-scope`: `eslint.config.js` refuses them
(`purlis/tree-family`, an error), with no file let off: the Chats list was the last tree drawn
by hand (#3198).

| Member | What it is |
| --- | --- |
| `Tree` | **A tree**: rows that may hold rows under them. `expandedKeys` and `onExpandedChange` say which are open (a folder is read when it opens); `selectionMode` lets rows be chosen where a press chooses (a file tab's file, `toggle`, so the arrows never open a file); `onAction` runs one; `twist` puts the twist before a row's words (`"lead"`, the default, R129) or at its end (`"trail"`, #2799 rule 1); `fixed` is a tree that never folds (the Changes view): every row open, no twist, nothing to press; `draggable` drags a row out with what it `carries`, by the pointer and by its drag button; `choosesMany` says its owner chooses several rows at once (#1151's picks); `onRowKey` takes a key before the tree does (Shift and the arrows picking); `stopAt` is the row the tree's one Tab stop is on until the keyboard has been in (the place the next chat starts, the file a tab shows) |
| `TreeRow` | **One row**, with the rows under it as its children: `content` is its slots (the list's `RowLead`, `RowLabel`, `RowDescription`, `RowTrail`, in the tree's scope), `textValue` what typing finds it by **and its name to a reader**, so the whole of what it says (a file's name, then its marks' words); `hasChildItems` for a folder not read yet (it holds a row that is never drawn until the owner's arrive); `menu` and `tip` as a list's row; `onHoverStart`, `onHoverEnd`, `onFocus` and `onBlur` for what a row raises itself beside the family's `tip` (a chat's card, #3198; the row hears its own focus, which React Aria hands it none of); `unavailable` for a row that keeps the keyboard to say why it cannot run (a file purlis will not open); `chosen` for a row its owner chooses while a press still runs it; `carries` what a drag of it carries; `onPressStart` to hear a press's modifier keys before its action runs; `current` for the current one of its kind (`aria-current`, drawn as chosen: where the next chat starts); `alwaysOpen` for a row whose rows never fold (a workspace's own) |
| `TreeLine` | **A line that is not a row to act on**, in its place among the rows: "Reading…", why a folder could not be read, "12 more not shown", a path the branch names and does not read. In the quiet tier; the arrows read it, and it holds nothing to press, drag or pick |
| `TreeSection` | A group of rows under its name (`header`) |

- **The keys are the WAI-ARIA tree's**: one Tab stop, the row last focused (the tree itself until
  a row has had the keyboard, which hands it to its first row or its chosen one); ↑ ↓, Home and
  End; → opens a closed row and moves into an open one's first child, or, on a row with
  controls of its own (a chat's hand and ⋯), into those, as the APG treegrid's → does, ↓ going on
  to the child (#3198); ← closes an open row and moves from a closed one to its parent; typing finds a row; Enter runs it; Shift+F10 or the menu
  key opens its menu. In a tree that never folds, ← on an open row moves to its parent.
- **A row is the list's row** (`ui/list/list.css` draws its slots in every collection's scope):
  the pill under the pointer, the wash and edge when chosen, the ring inside it, the secondary
  tier at rest. **Each level is a step in** (R129, `space.5` + `space.1`, 14 px at the default
  text size), drawn at most four deep; each level above draws a guide (`tree.guide`) that reaches
  into the hairline between rows, so it is one line. Where twists lead, every row stands one step
  further in, the step a parent's twist hangs in, so a parent and a leaf start their words at one
  place.
- **The twist** is a chevron in the quiet tier (R129, the Explorer's twisties, over R156's faint
  chevrons, which are a select's and a nav's), turned down while open (R43; a transform,
  no motion, so opening moves nothing). It is React Aria's toggle, "Expand ‹row›" or "Collapse
  ‹row›", never a Tab stop, and the arrows never land on it.
- **Sizes**: `sm` (24 px, a side view's), `md` (28, the default), `lg` (32). A tree takes one.
- **Where a drop lands is marked `data-drop-target`**, React Aria's own mark: on a row, the wash
  and an edge all round; on the tree itself, the edge alone, so its rows read as they were. Files
  dropped from the file manager on a workspace's own files mark either as the drag moves
  (`finderDrop.ts`, #5378), and Import files… in the row's menu does the same import.
- **Drawn by its parts** (ADR 0103, [the tree family joins the
  manifest](adr/0103/2026-10-25-the-tree-family-joins-the-manifest.md)): the tree is
  `data-scope="tree" data-part="body"` (its `data-twist`), a row the family's root with
  `data-size`, React Aria's state attributes, `data-level`, `data-expanded` and its depth
  (`--tree-depth`); its `twist`, its slots and a group's `header`. `ui/tree/tree.css` draws the
  family default; a theme's `tree.*` parts reach the same attributes, all but a row's padding,
  which is its indentation.
- **A long tree draws only the rows in sight** (`drawnInPart`, #2947): React Aria's own
  `Virtualizer` with its list layout, each row measured as it is drawn and the room between and
  around the rows read from `tree.css`; the tree is its own scroller, its height its owner's
  `className`. Settings › Theme's parts tree, the manifest's thousand entries and more, is the
  first. A tree whose entries the core caps (a folder's) draws whole.
- **Stories hold it** (`Tree.stories.tsx`): a branch's files with a folder being read, the Changes
  view's tree that never folds, twists that trail, an empty tree, and 1,260 rows drawn in part,
  in Chromium and WebKit, with axe.

## The dialog family

**Every dialog and alert dialog is `Dialog` or `AlertDialog`** (`app/src/ui/dialog/Dialog.tsx`,
#2910, ADR 0104), on React Aria's `ModalOverlay`, `Modal` and `Dialog`. No screen writes
`role="dialog"` or `role="alertdialog"` itself: `eslint.config.js` refuses it
(`purlis/dialog-family`, an error). A surface with an anatomy of its own, the palette, is
`DialogSurface`: the family's scrim, fixed top, width and height, and its own rows inside.
**`DialogSurface` is not modal**, as VS Code's palette is not (the operator's ruling of
2026-11-04, [ADR 0104's amendment](adr/0104/2026-11-04-the-palette-is-not-modal.md)): it holds
the keyboard in itself (the one `FocusScope` React Aria's `Overlay` draws round it, which
contains because the dialog in it asks it to:
[the amendment of 2026-11-10](adr/0104/2026-11-10-the-palette-keeps-one-focus-scope.md)) and
closes on Esc and on a press outside it, but the window behind it is never made inert or hidden.
Every other dialog stays modal, and a click outside one answers nothing.

**The dialog rules** (#2798, #2789, the operator's ruling in #2910), each held by the family:

| Rule | How |
| --- | --- |
| **Header, body, footer** | Three rows: the title and the line under it, the body, which is the only part that scrolls, and the footer with the answers, always in view. A control the keyboard is on is never under the footer (WCAG 2.4.11): the two never overlap. |
| **Width by size, never by content** | `size="standard"`, 600 px, or `size="wide"`, 800 px, only for a list beside its details; each at most the window's width less 16 px a side. |
| **Height** | At most the window less its top less 24 px; `height` gives a dialog one of its own, and it takes the lesser of the two whatever it holds. |
| **A fixed top** | `max(24px, 6vh)` under the window's top edge (R2, as the prototype places it), so a change in its content moves only its bottom edge. |
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
## A workspace's tabs are drawn by the tabs family's parts (#2914)

The tabs on a workspace's strip are drawn by `app/src/ui/tabs`: `TabCell` is one tab's cell (its
`data-variant`, `data-selected`, `data-hovered` for a mouse and `data-first`), `tabsPart` marks the
column (`layout`), the strip and the panel under it (`body`), `tabPart` the label and a count,
`TabCloser` the `×` (a task's own tab's `−`), on React Aria's `Button` through the shared press
(`ui/pressable.tsx`, #2930: a Tab stop on the front tab only, and a background tab's never takes
the keyboard), and `editorToolbar` the bar over a file. `tabs.css` draws Purlis's docked tabs from those
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
  on to the next stop in the document, the front tab's `×` and then the `+` (#5341), and a key the tab has taken is its own (a lifted
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

**A page's tabs are `PageTabs`** (#3668, `app/src/ui/tabs/PageTabs.tsx`, the skill page
prototype's variant A): React Aria's `Tabs`, `TabList`, `Tab` and `TabPanel`, with automatic
activation (one Tab stop on the row, ← and → choose the tab beside, Home and End the first and
the last), each panel named by its tab. A count after a tab's words is its description, never
part of its name, in room kept for three figures (past 99 it says 99+), so 9 to 10 widens
nothing. The chosen tab is told by the indicator under it as well as by its words' colour, and no
state changes a size. Narrow, the row scrolls along and never wraps. The chosen tab is the
owner's (`selected`, `onSelect`), so a view's history brings it back. Its parts are the
`page-tabs` family's: `tab`, `label`, `count`, `indicator`, `list` and `panel`. The segmented
control is a radio group, never a page's tabs.

`purlis/tabs-family` (an error) refuses a `tab`, `tablist` or `tabpanel` role written by hand,
a page's or a strip's alike, and `data-scope="tabs"` or `"page-tabs"` outside `src/ui`, and is
off in no file (#3134). **Where another family draws the tabs**, the project rail's column and
buttons and an activity bar's group and items, it wears the tabs family's keyboard and roles
rather than drawing its own: `useTabStrip` holds the tabs, `useTabStripRow` hands the container
the keyboard along them and the tablist its role, name and orientation, and `AsStripTab` hands
each family button its tab's role, `aria-selected`, Tab stop and focus. The keyboard is React
Aria's, as the session strip's: the arrows along it, Home and End, wrapping at its ends, one
Tab stop on the chosen tab, and manual activation. The title bar's workspaces are a toolbar of
chips, never a tablist (#2916).


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
  open one puts the side away, as on the right (R121 as amended 2026-10-26, #3329), and so does
  ⌘B. The column stands whether or not the side is out, so nothing moves when it goes or comes
  back; while the side is away none of its icons is marked, and a press brings it back on that
  view. A hidden rail
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
  what waits on the person is drawn hot, in the hot-count role `needs-you.count` and
  `needs-you.count-text` (R120, #3098), which a theme's count colours never reach.
- **A "new" pip** (`ActivityNew`, #6002, ADR 0121 decision 10) sits at the item's bottom right
  in the accent colour, ringed as a count is, while its view lists something the person has not
  seen; a count or a dot keeps its place above it. Its sentence ("2 new") is read after the
  count's, as the item's description. The window hands each view's number of New rows to
  `RegionFrame`'s `fresh` (`withNew`).
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
native `<button>` may wear a list's row or a tree's row; no class makes a native field, select,
checkbox, radio, table or menu right. Moving a control onto its family deletes
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
   A row keeps the KIND of thing it acts on where a pane and its tab are both in front of the
   person: *Rename chat…* on a chat's ⋯, *Rename tab…* on a split tab's menu (#5343).
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
settings row's ⋯ do. **The show-more and the away menus are the exception to the width**:
their rows are tabs and sentences with no part that gives way, so `.more-menu` hands
`--menu-max: none`.

## The overlay surface

**What the window draws over itself is one surface** (#1845). A dialog, a popover, a menu and a
tooltip share one ground (`surface.overlay`), one border, one corner (`--radius-lg`) and one lift
(`--elevation-overlay`), so they read as related; a dialog and a popover share one padding
(`--space-6`). A dialog is [the dialog family's](#the-dialog-family), drawn on that surface by
`app/src/ui/dialog/dialog.css`; a popover and a menu are the popover family's
(`app/src/ui/popover/popover.css`), a select's list's included, and a tooltip the tooltip
family's, each in its own sheet in the `components` layer. The old overlay and menu classes
(`.ui-menu`, `.ui-menu-item`, `.ui-popover`, `.ui-overlay-arrow`, `.ui-scrim`, `.ui-dialog`) and
`primitives/overlays.css`, which drew them, are retired (#3365).

| Overlay | Class                                         | On                                                         |
| ------- | --------------------------------------------- | ---------------------------------------------------------- |
| Dialog  | the dialog family's parts (`data-scope="dialog"`) | `Dialog`, `AlertDialog`, `DialogSurface`               |
| Popover | the popover family's surface ([above](#the-menu-and-popover-families)) | `AnchoredPopover`, `HeldPopover`, `PopoverDialog` |
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
  anything (#1647), so it has no lift and no overlay ground. It is [the notice
  family's](#the-card-notice-badge-and-count-and-separator-families) box instead, on the page's
  raised ground (`surface.raised`) with no shadow: a compact row with its mark in a chat pane's band (#5215), the
  danger ground for trouble, and in the Inbox's list the list's row, not a box of its own. What a
  way out opened under a Notice (a block's report, a dispatch's brief) is a card.

`app/src/primitives/overlays.test.tsx` holds the surface through the window's cascade (the same
ground, border, corner and lift on each kind, the same padding on a dialog and a popover), that a
dialog is still named by its title, keeps the keyboard and leaves on Escape, and that a popover
opens from the keyboard. It reads every component's source too: every button inside a popover or a
menu is the button family and every field the field family, and `App.css` draws no overlay surface
and lifts nothing by a shadow of its own. The dialog family's own tests hold a dialog's rows
(`ui/dialog/Dialog.test.tsx`).

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
  has. **Muted is the meta's only**: a help line or a where line is secondary. The one exception,
  until the view tabs wear the roles, is a view's sentence with no role (`.view-body .none`,
  `.pending`; `design-system.md`, #3938).
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

## Why Radix, and why not the other two (history)

_History: this is why the window took Radix first (ADR 0037). ADR 0104 moved every family
onto React Aria, which has the tree, table and combobox Radix has none of, and #2930 removed
Radix. The measurements below are of that first choice, not of today's bundle._

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
The terminal panes are xterm.js and are not a candidate. **A screen that is over is the terminal's read-only mode**
(`ReadOnlyTerminal.tsx`, #3676): xterm drawn from the theme's terminal tokens and font with no
cursor and no input, its grid hidden from a screen reader and its text beside it as a list of
its lines (a wrapped row joined to the row it continues), so a reader walks it line by line. It
is one Tab stop, a region named for what it is and described by the words that say it is
read-only, drawn outside the grid; ⌘F (Ctrl+F elsewhere) opens the find bar on it, ⌘A selects
the whole screen and ⌘C copies the selection. A saved grade chat and the theme specimen's
terminals are drawn by it, and no `role="img"` stands for a terminal's text.

And the tab strip's **show-more menu** (`@radix-ui/react-dropdown-menu`, ADR 0039),
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
tabs, the workspace tabs, the tabs on a workspace's strip, the panes, the explorer's worktree and clone rows, the
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
- **A row's accessible name is its words alone**, and its note — what the row costs — is its
  description. Its content names it, with the line under its words and its key kept out of the
  name (#6043, [the menu family](#the-menu-and-popover-families)), so no row announces as a
  paragraph: _"End Ends the program it runs. There is no undo."_ The thing the menu is about is
  the menu's own name, said once ([Menus](#menus)).
- **A group the catalogue decides the length of is a submenu**, Radix's own `ContextMenu.Sub`:
  "Curate ▸" on a workspace, a persona and the plane root's tab (ADR 0061). `actions.curateRows` picks that subject's
  rows out of the catalogue by id — purlis's own, then one named `ContextMenu.Label` group per
  declaring persona, then each action the core left out as a disabled row with the core's
  sentence under it — and it runs inside the menu's content, which Radix mounts only while the
  menu is open, so a strip's fifty tabs do not pay the scan per render. "Ask ▸" on a chat's menu
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
    menu key pastes nothing (with nothing selected Copy is off, and Paste is the first of
    these rows that can run).
  - **No Clear.** xterm's `clear()` empties the screen as well as the history, and the
    history is the core's, sent whole to a pane drawn again.
  - **Over a link, *Open link* and *Copy link* come first** (`terminalRows.linkRows`, #6219),
    above Copy, each with the link's target under its words. The link is the one the pane's
    link hook has under the pointer as the menu is asked for (`paneLinks.linkPane`'s
    `under()`), not one worked out from the pointer's place, so it is the link the person sees
    underlined. *Open link* goes the way ⌘-click does, through the core, asks included, and
    draws ⌘-click (Ctrl-click) at its end as a **gesture**, not a key: `RowDrawn.gesture` is
    drawn where a key would be and named to no `aria-keyshortcuts`. *Copy link* copies the
    target the core resolved, not the printed words. Text the core refuses offers *Copy link*
    alone. **A menu opened from the keyboard has no link rows** (D4), even with the pointer
    resting on a link, since a keyboard user cannot see which link that would be; *Links in
    this pane…* stands in their place, with its key at its end, and over a link it leads the
    pane's rows instead.
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
  whichever surface pressed it — so End… on a chat's ⋯, on its row's menu and in the palette
  all ask. A tab's `×` and a pane's `×` end nothing, so they ask nothing (#4756).
  A confirmation on one surface and not another is the second answer the catalogue exists to
  not have.
- **Three answers since Smart close (ADR 0064): Cancel, Close, Smart close, in that order.**
  Cancel is the primitive's `Cancel`; Close and Smart close are both its `Action`, so either
  closes the dialog. **The focus goes to the default the operator ruled, not always to Cancel**:
  Smart close for a chat with turns behind it, Close for one with at most one, and Cancel
  wherever purlis cannot say. Smart close that is not offered is `disabled`, with the core's
  sentence beside it (`aria-describedby`); Radix's focus scope skips a disabled button, so
  Shift+Tab from Cancel reaches Close. Escape still answers Cancel.

And the **persona card** (first drawn on Radix's popover, now the popover family's, `ui/popover`;
`app/src/Panels.tsx`): what a row in the right-hand region's persona list opens. It is the first
popover in the window, and it was picked over the other two surfaces Radix has for the same
content:

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

And a **Chats row's card** (the popover family's `AnchoredPopover`, `app/src/ChatsSection.tsx`,
#1675):
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

_History: learned on Radix, before the families. What a family does in its place is in its own
section above._

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
both and has no Undo (SE-18), is no radio since DS-8: it is rows of the row's ⋯ menu ("Move to
‹file›", #2919), and only a row chosen in it moves the value. Each held pick is guarded by a test that
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

_History: measured on Radix's dialogs. The dialog family now moves Tab itself, round from the last
stop to the first (`ui/dialog/Dialog.tsx`, `ui/dialog/tabOrder.ts`), and the button family writes
the `tabindex` WebKit needs on every button (#190)._

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

- **It was not a hand-written Tab handler, which ADR 0037 was against and which this
  would have been the fourth of.** Nothing wrapped a primitive, nothing intercepted a key, and
  Radix's own edge behaviour was untouched and did the wrapping at the ends. (ADR 0104 has
  since put one there on purpose: the dialog family moves Tab itself, `useTabbing`.)
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

_History in part: the shape below holds, but what draws it has moved. The strips are the tabs
family's (React Aria's tabs), the lists and the explorer the list and tree families', and a menu
the menu family's; `@radix-ui/react-roving-focus` left with the last of Radix (#2930)._

The attribute above fixed the dialogs. Outside them it is the floor and not the answer: a
`tabIndex={0}` on every button would put fifty tabs between the explorer and the terminal.
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
  the arrows move, Escape closes it and puts the keyboard back on the button. The menu
  family does all of that, so nothing here is hand-written.
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
- **On the plane root's tab, Files is the whole project's** (issue 1916): the same tree
  and rows as a branch's files, read through the window's own `plane_tree`. A workspace's repo
  is one row with the workspace's mark (`Folders`), its folder's name and
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
  fifty stops again. **Delete on a focused project's tab or a workspace strip's tab presses
  the row its `×` presses** (purlis#239, `closeOnDelete` in `app/src/tabKeys.ts`), and so does
  Backspace on a Mac, whose key marked "delete" sends it: closing a tab ends nothing and asks
  nothing, since its chats go on in the sidebar (#4756), a task's own tab sends the task back
  (#1489), closing a project with chats open now asks first too (`ClosingProject`, on
  the row's verb, so the `×`, the menu and the palette ask it as well), and the keyboard lands
  back on the tab after a Cancel and on the strip's stop after a close. A new strip with a `×`
  (a vault tab's) calls it from its tab's `onKeyDown` with that `×`'s row. The palette and the
  tab's own menu reach the same row.
- **The order is the document's, which is the order the window is drawn in**: the title bar,
  the workspace strip, then the regions as the arrangement places them, with the tab strip at
  the top of the editor, before the panes (ADR 0036 as amended 2026-10-12) (ADR 0038 — a region moves,
  and its stops move with it; a fixed order would contradict the layout on screen), each
  resize handle between them (`react-resizable-panels` makes a separator a keyboard control),
  the focused pane's controls before its terminal, and the status line last.
  `Window.keyboard.test.tsx` pins the whole sequence.

_History: `@radix-ui/react-roving-focus` and `useTabStop` (`app/src/roving.ts`) left with the last
lists on them, the Inbox (#5995) and Settings' nav (#2930); every list, tree, strip and page
navigation now takes its one Tab stop from its family. What follows is how it was wired._

`useTabStop` was the one decision the primitive leaves open: which item is
the stop before anyone has touched the group. Left alone it is none of them and the group's
own element takes the stop. It is the selected item at rest, the item the keyboard is on while
it is inside, and the selected one again when the keyboard leaves. A strip's own controls
(`.strip-doing`, the `+`) are inside the group but are not its items, so reaching one is leaving.
A new list takes its family's keyboard instead (the list, tree and tabs families, and the panel
family's `ViewNav`).

**Two things about wiring it that will catch the next person.** `useTabStop` has to be told
every item that is DRAWN — a tab the strip collapsed into its menu, or a row inside a folded
clone, is never the stop, or the group has none. And a conditional that puts the `Root` around
an element in one branch and not the other makes React remount that element when the branch
changes: the explorer's empty `nav` is inside the same `Root` as its full one for that reason,
and `FourRegions.test.tsx` found it by holding a `nav` that had been replaced.

**A terminal keeps Tab.** xterm prevents Tab and Shift+Tab and sends them to the shell —
completion, and Claude Code's mode cycling — and nothing in the window takes either. So a
terminal is a stop you can reach and cannot Tab out of, which is why the pane's own controls are
written _before_ it in the document, in the pane's head above it.
**They are always drawn** (#5318, ADR 0111), so every pane's are in the sequence, in the order
they are drawn. A tab's only pane has no Close, so no stop is left for it.

**The way out is Ctrl+Tab, and Ctrl+Shift+Tab backwards** (`SessionPane.leavesTheChat`). It is
the platforms' own key for leaving a control that keeps Tab — GTK's text view, Windows'
multi-line box, AppKit's field editor — and by the rule in the next section it takes nothing
from the chat: xterm.js 6.0.0 ignores Ctrl on key code 9, so it would have sent `\t` and
`ESC [ Z`, which are Tab's and Shift+Tab's own bytes. WebKit has no default action for
Ctrl+Tab, so the window walks the engine's sequence itself (`app/src/tabSequence.ts`, the rule
above written down once and shared with the tests). The palette (`F2`, `⌘K`) remains the other
way to act from inside a chat without leaving it.

**A browser the person drives, and a browser's DevTools, keep every key too** (#2014, #2028):
each key goes to the page or to DevTools. The same chord leaves them
(`leaveKeys.leavesTheSurface`), on the capture phase of the surface, so Ctrl+Tab and
Ctrl+Shift+Tab never reach the page or DevTools; neither has a use for it that the window
needs to pass on.

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
which is one `justify-content` on the rule that already says what a tab is. A tab's `×` is
outside the button, so a tab's name is centred in the room the `×` leaves rather than in the
whole cell: the same rule, not an exception to it.

**Since 2026-10-12 the tab strip (the chat strip, until a chat stopped being a tab in
#4750) is the editor's tab bar** (ADR 0036 as amended, #1836): it is
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
tab's `×`, fresh mark and hand, every strip's own `+`), so the `tablist` role is not on the
element that holds them.

- **The strip element** (the rail's column, `.tabs`) has no role. It is marked
  `data-strip="Projects" | "Tabs"`, and keeps its cells, its keyboard (React Aria's, through the
  tabs family: `TabStripRow` on the tab strip, `useTabStripRow` on the rail's column, #3134), its
  `DndContext` and the ref `useRoom` measures.
- **The tablist** is an empty `<span role="tablist" aria-label=…>` first inside it
  (`TabStripRow` on the tab strip; the rail's `tablist` part, given its attributes by
  `useTabStripRow`), owning the tabs drawn by id, in order. The stylesheet lays it over the strip with `pointer-events: none`, so it takes no
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
  collapsible, and no collapsible primitive is installed because nothing needs one.
- **Picking a spot in the explorer is `aria-current`, not `aria-selected`.** `aria-selected`
  belongs to the three tablists that are the axis (ADR 0036), and to the one tree that really
  selects, a file tab's, whose selected file is what the tab previews; a spot is the current
  item of a list, and native buttons are not hand-rolled markup.
- **Every tree is drawn by the tree family** (#1672, DS-12, #2911, #3198): its levels, its
  straight guides, the selected row (`aria-current` or `aria-selected`, in `list.selected` with
  an edge) and the keyboard's ring are `ui/tree`'s; a new tree draws nothing of its own.
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
  - **An item's ring is inside it** (#1922, [ADR 0104's amendment of
    2026-11-09](adr/0104/2026-11-09-a-status-bar-item-draws-its-focus-ring-inset.md)): the bar
    is a strip that clips, as a tab strip is, so a `bar` item rings inside its own edge
    (`--focus-ring-inset`) and the bar keeps no room above or below for a ring. The line is
    `--size-chrome` tall (24 CSS px at the least, the target's floor). The title bar's pills keep
    the outside ring.
  - **Severity is a colour and a word.** The doctor's count is in `state.failed` or
    `state.waiting` and reads _"2 blockers"_, _"10 warnings"_; a clean doctor draws no count.
    The Notices count is a number in the neutral count pill beside a bell in `state.waiting`.
    Each item's accessible name says its count (_"Notices: 6"_, _"Doctor: 10 warnings"_).
  - **The project is its name, and its directory is the window's tooltip** (the shared one,
    #2908). It is a status item that copies the directory when pressed (#1922), named
    _"Project <name>"_, and says _"Directory copied."_ in a polite status: a focusable span was
    read by no role. The tooltip opens under the pointer and on keyboard focus. A long
    name is cut with an ellipsis. The item carries the directory as `data-directory`, which is
    how a scenario learns which project is open.
- **A side with views has an activity bar** (ADR 0038 as amended 2026-10-10, #1673;
  `app/src/ActivityBar.tsx`). It is VS Code's strip of icons at the window's edge, and it is
  built from what this file already allows:
  - **A vertical tab list on the tabs family's keyboard** (#3134): `role="tablist"` with
    `aria-orientation="vertical"`, a `button role="tab"` per view, `aria-selected` on the open
    one, `aria-controls` naming its `tabpanel`, and one Tab stop, React Aria's (`useTabStripRow`,
    `AsStripTab`). The arrows, Home and End move along it, wrapping at its ends; Enter, Space or a
    click presses. Activation is manual,
    so walking the bar opens nothing. **The tab list owns tabs and nothing else** (#1204): a
    view's badge is inside its own tab, and **its count is in the tab's name** (ADR 0121
    decision 25): `aria-labelledby` names the tab itself and then its badge, so a reader hears
    "Asks 2 things wait on you", while `aria-label` stays the view's alone, which is what names
    its panel and what a scenario finds it by.
  - **`aria-selected` is right here, as it is not in the explorer**: the bar is a tab list,
    and its tabs select what the side shows. The explorer keeps `aria-current`.
  - **Icon-only, with the words on `aria-label` and the press in its tooltip**: the tab is named for the view (`Chats`, `Explorer`) and its tooltip says what
    pressing does, with the key (_"Show the Explorer view (⌘⇧E)"_, or _"Put the Navigation
    region away"_ on the open one). **An attention view's tooltip asks its question instead**,
    open or not (ADR 0121 decisions 1 and 24, `VIEW_QUESTIONS`): Asks _"Do I need to answer
    something?"_, Updates _"Did my work finish?"_, Notices _"Is something about my setup wrong
    or worth knowing?"_. Its mark is what the view is, never which edge it is on.
  - **The ✋ pulses once** (ADR 0121 decision 24): the first time on this machine a Blocking ask
    arrives while Asks is closed, its item is marked `data-pulse` for one `breathe`
    (`asksArrival.ts`, kept in web storage as `purlis.asks-pulsed`).
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
- **A split that resizes** uses `Separator` from `ui/panel/Separator.tsx`, not the library's own:
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
  the person is back. `projectShown.guard.test.tsx` fails for a new `createPortal`, and
  `radixGone.guard.test.ts` for a Radix package. Behind that
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

The window's title bar (`app/src/TitleBar.tsx`) is a `<header>` holding the project strip — a
tablist, since moved to the project rail (#2915) and onto the tabs family (#3134) — and a `<span>` of buttons at
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
is generated from it. **Focus this pane** (⌘⇧↩, Ctrl+Shift+Enter off a Mac, `keys/focusMode.ts`
and `tiling.ts`) is one of its keys (#4765). In a split tab it zooms the focused pane to fill the
tab; in a one-pane tab it is Focus mode, which hides the sides. The same key brings everything
back, and while a pane is zoomed the strip's tab wears a zoom mark.

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

**The Skills view's key is `⌘⇧X`** (#3266), VS Code's for its Extensions view, and
`Ctrl+Shift+X` off a Mac: `Ctrl` with `Shift` and a letter is no byte, so it is the window's while
a chat has the keyboard, as `Ctrl+Shift+E` is. The view sits under Search on the left and is
there only while the Skill studio's preview is on; while it is off the key is left alone, no
palette row names the view, and a side left on it opens on Chats until the preview is back
(`sideViews.PREVIEWED_VIEWS`, which `regions.openView` reads as it reads an extension's panel
that is not contributed now). Its list is the list family's: one Tab stop, ↑ and ↓ past the
groups' names, typing finds a skill, Enter opens its tab, and Shift+F10 or the menu key opens a
row's menu.

**Use skill…'s key is `⌘⇧S`** (#3954), and `Ctrl+Shift+S` off a Mac, `S` for skill: it opens Use
skill… on the chat in front (in a split tab, the pane with the keyboard), as its palette row does,
and is said on that row and on each chat
tab's menu row (`chat.useSkill`, `tab.useskill:<tab>`). `Ctrl` with `Shift` and a letter is no
byte, so it is the window's while a chat has the keyboard, as `Ctrl+Shift+T` is; an editor keeps
it, and it is never taken under a modal dialog. With no chat in front, a shell in front or the
Skill studio off there is no Use skill… to open, and the key goes on to whatever has the keyboard. The row says no budget: a chat's budget is the
Skills view's to say (S8).

**The Panel's key is `⌘J`** (#1737, P-6), VS Code's, and `Ctrl+J` off a Mac: it hides the whole
Panel, every half of it, and shows it again on the views its halves had. It hides only a Panel
that is drawn: a half holding only extensions' panels that no extension contributes now is not
drawn, so where that is all that is left, the key shows the Panel (#1752). `Ctrl+J` is a
terminal's newline, so a chat with the keyboard keeps it, as it keeps `Ctrl+B`; a `⌘` chord is no
byte. `⌘⇧J` is the focused chat's tasks, so Shift held is never this key. Its palette row is _Hide
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

**Asks' key is `⌘⇧I`** (#1692; the Inbox's until #5644), `Ctrl+Shift+I` off a Mac: `Ctrl` with
`Shift` and a letter is no byte, so it is the window's while a chat has the keyboard, as
`Ctrl+Shift+E` is. It shows Asks, the right side's first view, and gives the keyboard to its first ask, never to one of
the ask's buttons, so the key and a stray Enter after it answer nothing. The title bar's ✋ does
the same. Asks is one Tab stop, a list of rows that act of [the list family](#the-list-family)
(#3123, ruling B, the WAI-ARIA grid): ↑ and ↓ move between the asks and the rows of what waits
elsewhere, ← and → through an ask's own links, answers and boxes (a dispatch's Notice drawn in it
included), Home and End go to the first and last ask, Enter presses the focused button as it does
anywhere, a reply or a reason box keeps its own keys and Esc in it gives the keyboard back to its
ask, and Escape on an ask gives the keyboard back to the chat in front. Updates is a list of its
own, after its Mark all read and Dismiss all. Typing finds no row: there is no key of a single
letter (I-11).

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

**The Browsers grid** (#2027, `BrowsersGridTab.tsx`, `browsersGrid.ts`) is a view tab, one per
project, opened by *Open browsers* in the palette and on the Chats view's heading while the
project has a browser. It wears the Inbox's switch over the same three scopes, kept as
`projects[path].browsers.scope`; This workspace is the strip the tab is on, and All projects is
every project this window holds. Its tiles are one `ListBox` with `layout="grid"`: one Tab stop,
the arrows walk the grid, and a press or Enter opens the browser's view in its own project,
which comes to the front. A tile is the browser's thumbnail view (about a frame a second, drawn
only while the tab can be seen) over its name and, in the quiet tier, where it is: its project
among every project's, its workspace, its page and device as the chip says them, and who drives
it. An empty scope is the empty state family, saying how many run in the nearest wider scope and
widening in one press.

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

**The keys for panes, tabs and the focused chat take nothing either** (#4767, ruled on #4699;
`moveKeys.ts`, `taskKeys.ts`). A chat is not a tab, so the keys that stepped through a tab's
chats went with it.

|                                          | On a Mac      | Everywhere else                                  |
| ---------------------------------------- | ------------- | ------------------------------------------------ |
| The pane in that direction (no wrap)     | `⌘⌥←↑→↓`      | `Alt+←↑→↓`, left to a chat's terminal and a view |
| The next / previous tab (round the ends) | `⌘⇧]` / `⌘⇧[` | `Ctrl+Shift+]` / `Ctrl+Shift+[`                  |
| The focused chat's tasks                 | `⌘⇧J`         | `Ctrl+Shift+J`                                   |
| Back to the chat this pane showed before | `⌘⇧H`         | `Ctrl+Shift+H`                                   |

**Read from xterm.js itself, not from memory**: `app/src/keys/harnessBytes.test.ts` puts every
chord of these four through the pinned xterm.js's `evaluateKeyboardEvent` and fails if one the
window takes from a terminal is a byte, or if one it leaves to the terminal is not. A `⌘` chord
with a letter sends nothing but `⌘A`; each arrow's case breaks on `metaKey` before it builds
anything, so `⌘⌥←` sends nothing, while `⌥←` without `⌘` is `ESC [1;3D` (word movement); and
`Ctrl` with a letter, `[` or `]` is encoded only when Shift is not held. So plain `Ctrl+J`
(newline), `Ctrl+H` (backspace), `Ctrl+[` (Escape) and `Ctrl+]` still reach the shell, and off a
Mac `Alt` and an arrow is the terminal's while it has the keyboard: there `Ctrl+Tab` leaves the
terminal first. `⌘H` alone stays the system's Hide. **If the pinned xterm.js gains the kitty
keyboard protocol**, which Claude Code asks for, the off-a-Mac chords become bytes and that test
fails: they then need a `chatKeeps` review.

**Where a task is drawn has keys too, and each is on the thing it moves** (#1489). On a task's
row in the Chats list, Space opens it beside the chat that asked for it. On its line in the
task menu on that chat's pane head (#5341), Enter goes to it and Enter with a modifier opens it
elsewhere, as a link is opened in a new tab: `⌘`/`Ctrl`+Enter gives it a tab of its own, and
`Alt`+Enter opens it beside the chat that asked for it. On a task's own tab, the key that closes
a tab (Delete; Backspace on a Mac) sends it back under the chat that asked for it and ends
nothing. None of these is a window-wide key: each is
handled by the row, the menu line or the tab that has the keyboard, so no chat loses a key.
The buttons at the end of a menu line are the pointer's way to the same, and are not in the
accessibility tree, as a list row's fold is not: the keyboard's way is the line's own keys and
the task's menu in the Chats list.

**⌘⌥ and an arrow is iTerm2's pane key, and Alt and an arrow Windows Terminal's**; `⌘⇧]` and
`⌘⇧[` are next and previous tab in every tabbed program on a Mac, and were kept free for this.
Off a Mac `Ctrl+Alt` with an arrow is the desktop's, and Alt with ← and → is a view's Back and
Forward (`keys/viewKeys.ts`), which a view keeps.

**A key is matched by what it types, not by where it is**, so a layout that puts another
character on a key never fires two of the window's actions at once: on Dvorak the key in `]`'s
place types `=` and `+`, which with the same modifier is the text size's. Only where a layout
has no `[` or `]` without AltGr (German, Nordic, French) is the key in their place taken, and
then never when it types a letter or a digit or is another of the window's keys.

They are capture listeners on the window, held by the project in front. ⌘⇧J and ⌘⇧H press
their catalogue rows (`tasks.menu`, `pane.back`), which the palette lists with their keys. ⌘⇧J
opens the menu of the focused pane's chat and its tasks, from that pane head's state word, the
same menu a press of the word opens (`TabChip.ChatTasks`, #5058, #5341): the one picked is shown
in that pane as a press shows it, and one on screen already is jumped to. The pane and tab keys move the keyboard and have no rows. **Nothing is taken while a dialog has the keyboard.** Over a
terminal the chord is the window's whether or not its row can run, so the same chord never
means two things from one tab to the next; in a text field or an editor a chord whose row
cannot run is left to the field.

**A menu that opens under a resting pointer takes no keyboard** (#1487, `TabChip.tsx`). The tasks
menu on a pane head's state word (a tab's chip until #5341) is the menu family's, opened three ways:
a press, ⌘⇧J, and the pointer resting on the word. A menu moves the focus into itself as it opens
and onto each row the pointer crosses, which is right for the first two and would take the keyboard
out of a terminal for the third. So a rest-opened menu refuses both (`autoFocus` off, and the
family's `takesNoKeyboard`), the stylesheet's hover says which row the pointer is on, and an Escape
that closes it stops there instead of also reaching the chat. Only that menu closes when the pointer
leaves: a pressed or key-opened menu has the keyboard and stays until Escape, a pick, a press
outside or the same ask again. However it was opened, a menu closed without a pick puts the keyboard
back where it was before it opened.

**A rest is a move the person made, then stillness.** Both WebKits hit-test again after a
layout and say "the pointer came onto this" of a pointer that has not moved, so a chip that
appears, widens or slides under a parked pointer would open its menu by itself. Coming onto
the chip therefore arms nothing: the 350 ms start at the first pointer move to a new point,
with no button held.

**Shift+Enter in a harness's pane is the harness's newline** (SI-4, `Harness::newline`). A
terminal has no Shift+Enter: xterm.js 6.0.0 sends a bare CR for it, the byte Enter sends, so every
harness submitted on it. The pane sends the harness's own newline instead — ESC CR for all three,
measured — and a shell's pane, which runs no harness, keeps xterm's Enter.
