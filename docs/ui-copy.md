# How the window talks

**Plain, active and specific.** Every label, button, empty state and error in purlis says what
is true, in words the reader already has, and says what to do next when there is something to
do. This page writes down how the window already talks, so new copy matches it. Every example
below is a string the app ships today.

`docs/design-system.md` covers how the window looks. ADR 0072 covers which nouns it may use. This
page covers how it puts them into sentences. ST9's definition of done asks that every feature's
empty state and error copy follow it.

## The voice

- **purlis is lowercase**, at the start of a sentence too, and in the About dialog's title
  (V93a). It is the program's name, and it does the acting: _purlis could not read the alerts_,
  _purlis checks on its own every few hours_, _This is purlis {version}_, _About purlis_. A
  capital Charter or Purlis in the window's text is a fault the copy guard names.
- **The reader is "you".** _Open a project you already have._ _Nothing is written into your
  repo._
- **Active and present.** Say who does what: _It checks again when you press Delete_, not
  _a check will be performed_.
- **Say what is true, then stop.** One claim per sentence. Don't use "simply", "just", "please"
  or "successfully". A save that worked says _Session saved_, not _Session saved successfully!_
- **No exclamation marks**, and no emoji.
- **Plain words for the result, not the mechanism.** _Its value is gone from the vault for good._
  If the reader needs a mechanism to decide, name it in one clause: _It needs sudo, so purlis
  types it in a shell and leaves running it to you._ A shell is shown in a pane like a chat, so
  the window says _shell_, never _shell tab_ (#5637).

## Case

- **Sentence case everywhere:** buttons, menu items, headings, tab titles, dialog titles,
  placeholders and `aria-label`s. Write _Create workspace_, _Open project…_ and _Recent
  projects_, never _Open Project…_.
- **Names keep their own capitals:** GitHub, GitLab, Claude Code, Codex, opencode, Keychain,
  Touch ID.
- **A control named inside a sentence is written as its label reads**, with no quotes: _start
  one and press Check again_, _Make one with the + above, or New vault… in the palette_. The
  capital tells the reader it is something to press. An act named for its button is cased
  that way wherever it is the subject, too: _Smart close stopped — …_ (#1156).
  A path to a Settings group is its labels as they read, so it is no title case: _lift it in
  Settings › Project › Dispatch_.

## Buttons and menu items

- **A verb and what it acts on:** _Create branch_, _Delete vault_, _Restart now_, _Open in its
  pane_. A button that confirms names the act it confirms. It is never _OK_ or _Yes_.
- **Cancel is the way back** and changes nothing. A second way out says how it differs: _Wait_
  beside _Restart now_, _Keep it off_ beside _Turn the sandbox on_.
- **An ellipsis means "asks something first".** _New project…_, _Browse…_, _Add an extension…_
  open a dialog or a picker before anything happens. A button that acts at once has none.
  Always the one character `…`, never three dots.
- **The same act has the same words everywhere:** the palette row, the button and the empty
  state's way out. The window's catalogue (`actions.ts`) is where each act's label is written
  once. An ask's answers (_Allow for me on this machine_, _Keep blocked_) are written once in
  the asks registry (`asking.rs`), and every Notice and the Inbox draw the label its ask
  carries (#1700).
- **A context menu's row drops the name of the thing the menu was opened on**, and keeps the
  verb: the palette's _Rename workspace alpha…_ is the workspace's menu's _Rename…_, and
  _End chat console-developer 16_ is _End_. The thing is in front of the person and the menu
  says it once, as its own name. A row keeps a name when it is another thing's (_Merge into
  svc_). **A row keeps the kind of thing it acts on where two things are in front of the
  person**: a chat's ⋯ sits on a pane inside a tab, so its rename is _Rename chat…_, and a
  split tab's is _Rename tab…_, so neither reads as the other's (#5343). These shorter words
  are written once too, in `menuWords.ts`, and `docs/ui-primitives.md` ("Menus") has the rest
  of a menu's rules.

## Work in progress

- **A present participle and an ellipsis:** _Reading the workspace…_, _Asking git…_, _Copying
  your repo into its workspace…_. Say what is being read or asked, not just _Loading…_.
- Something that is not done yet is _Not read yet._ or _not written yet_, not blank.

## Empty states

`EmptyState.tsx` draws them, and its fields are the rule:

- **The headline is a claim about what is true, never an instruction.** _No workspaces yet_,
  _No chats in this workspace_, _Nothing needs you here._, _Nothing remembered yet_. Use "yet"
  when the thing is expected to arrive.
- **The body is the way out, named the way the window names it:** _Make one with the + above,
  or New persona… in the palette._, _Add one in the box above._, _purlis runs each chat in its own pane. Open
  the first one here._
- **Never the storage underneath.** Leave out file layouts, store names and ADR numbers. _Todos
  are files in this workspace's store_ told the reader nothing they could act on.
- **A panel's way out is a catalogue row.** A panel written in Rust names it in
  `panel::Empty::offer`, and the window draws it under the body as a button with the row's own
  title, only while the catalogue offers it. A command in the body goes in backticks, and the
  window draws it in code font.
- **An empty state with no way out is fine.** When there is nothing the reader can do, say so
  and stop. Don't invent an action.
- **Something that has gone says it has gone:** _This memory is not here any more_, _Nothing
  is at {path} now._

## Errors and refusals

- **Say what happened, from purlis's side, then why:** _purlis could not list the branches of
  {repo}: {reason}_. The reason after the colon is the underlying error, passed through as it
  was said.
- **Then what to do, when there is something to do:** _Could not clone {repos} into {workspace}
  — {reason} Retry from the workspace's settings._ A message with nothing to do ends after the
  reason.
- **Never a stock phrase.** _Something went wrong_, _An error occurred_, _Unknown error_,
  _Oops_ and a leading _Error:_ each say that nothing was found out. If purlis really does not
  know, say what it was doing: _purlis did not answer with a reading_.
- **A refusal says what purlis will not do, and why:** _purlis cannot send {key}._, _purlis
  will not read …_. If it does something else instead, say that too: _{name} did not start
  ({reason}). It is still recorded, and will be tried again at the next launch._
  (`PlaneView.tsx`, the notice for a chat that did not reopen).
- **Don't blame the reader.** The subject is what failed, not what the reader did wrong.
- **Say the cost of an act that cannot be undone, before it happens:** _There is no undo._,
  _Its 2 secrets are destroyed in your system keychain and cannot be recovered._
- **Uncertainty is stated, not hidden:** _{name} reports no state, so purlis cannot tell
  whether it is mid-turn._
- **A stop is Blocked, not Refused** (R-170): a call a guard, the sandbox, a policy or a
  harness's own check stopped is _blocked_, in a label and a sentence alike: _Commit blocked_,
  _Blocked by the sandbox_, _purlis blocked a force push_. _Refused_ names nothing the reader
  can tell from blocked, so the window has one word for it. A sentence may still use _refused_
  as English where it is no stop: _The pool refused the move: it is full._

### Why a chat is not hibernated

The core says why a chat may not be hibernated as one typed reason (`purlis_core::rested`,
SC-4c), for the person's Hibernate and, later, for the app's own; the window words it, in
`app/src/hibernationWords.ts`. Each says what happened, then what to do where there is
something to do:

| Reason                                    | Words                                                                                                    |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Not at a turn boundary                    | _Not hibernated: this chat is not at the end of a turn. Hibernate it once its turn ends._                |
| Asking                                    | _Not hibernated: this chat is asking you something. Answer it first._                                    |
| Paused                                    | _Not hibernated: this chat is paused. Let it go on first._                                               |
| Hibernated already                        | _Not hibernated: this chat is already hibernated. Resume it to go on._                                   |
| Ended                                     | _Not hibernated: this chat has ended._                                                                   |
| Cannot resume by itself                   | _Not hibernated: this chat's harness cannot resume a conversation by itself._                            |
| purlis cannot see its background work     | _Not hibernated: purlis cannot see the background work this chat's harness leaves running._              |
| Background work running                   | _Not hibernated: this chat has background work running. Wait for it to finish._                          |
| A dispatched task running or not reported | _Not hibernated: a task this chat dispatched is still running or has not reported. Wait for its report._ |
| Its parent waits on its report            | _Not hibernated: this chat is a task, and the chat that dispatched it is waiting on its report._         |
| A dispatched task                         | _Not hibernated: this chat is a dispatched task._                                                        |
| Observed                                  | _Not hibernated: this chat is only watched here, not run by purlis._                                     |
| A runner's chat                           | _Not hibernated: this chat runs on a runner, not on this machine._                                       |
| A sub-agent's run                         | _Not hibernated: this is a sub-agent of a chat, not a chat of its own._                                  |
| Looked at recently                        | _Not hibernated: someone looked at this chat or typed in it recently._                                   |

## Words

- **The nouns are ADR 0072's.** Five concepts (Project, Workspace, Chat, Persona, Memory), and
  on the first-hour surfaces only those plus _Save_ and _branch_. `CONTEXT.md` defines each
  word and lists the ones to avoid. A code repository is always a **repo**, on every forge,
  never a project.
- **No jargon in the window:** no ADR or ticket numbers, no internal type, module or store
  names, no protocol names where the effect can be said instead.
- **A command is written in code font** (`<code>`) where the command line is the way to do
  something: _You can add it later with <code>purlis workspace vision</code>._ Only name
  commands the `purlis` binary has. A panel's text from Rust carries a command in backticks,
  and the window draws it in code font (#1156).
- **Counts are digits, and plurals agree:** _1 secret_, _2 secrets_ (`counted` in
  `Vaults.tsx`; in Rust, `purlis_core::counts::counted`, and `plural` for the verb that
  follows). Never _2 secret(s)_: `crates/purlis-core/tests/it/no_count_says_thing_s.rs` fails on
  one in shipped Rust.
- **An `aria-label` says what the visible label says**, and an icon-only control's label is the
  verb it would have had as text: _Close find_, _Next match_.

## What the build checks

`app/src/copy.test.ts` reads every string in `app/src` (`uiStrings.ts` finds them with
TypeScript's parser) and fails on:

- **a stock phrase**, in any string: _something went wrong_, _an error occurred_, _unknown
  error_, _oops_, _please_, _successfully_, or a leading _error:_. A code span and an id or a
  path (`ask.please`, `hooks/please-hold.sh`) are not read as words;
- **an exclamation mark** ending any sentence of text the window shows;
- **a word in capitals** in text the window shows: four letters or more, all capitals, as in
  _NEVER close_. Acronyms and the words the window keeps in capitals (`ACRONYMS` in `copy.ts`:
  _JSON_, _README_, _PATH_, a workspace's _LIVE_ and _LOCAL_), names, key chords, code spans
  and file names such as `AGENTS.md` are taken out first;
- **title case** in text the window shows: two or more words of four letters or more, all
  capitalised. Names (`NAMES` in `copy.ts`) and key chords such as `Ctrl+Shift+F` are taken
  out first. A name the check does not know fails on its first label, and adding it to
  `NAMES` is the fix;
- **a capital "purlis"** in text the window shows, anywhere but the About dialog's title.

A rule of its own, `refusedWord` in `copy.ts`, refuses **Refused for a stop** in text the window
shows and in copy that reads as a sentence: _Refused_ with its capital anywhere, and a label
that ends on the word in any case (_Commit refused_). Lowercase _refused_ inside a sentence is
English and passes.

A rule of its own, `retiredTerms` in `copy.ts`, refuses **the retired terms** in text the window
shows (FR-3, ADR 0072): _plane_ (say project), _worktree_ and _piece_ (say branch, or its
folder), and _sync_ anywhere but `purlis sync` and the _Sync repos_ row. _A piece of_ is
English and passes. A code span, a path, an id and an assignment (`NAME=value`) are not read
as words, but a hyphenated word is: _plane-wide_ is refused. The code and the plane format
still say plane until the rename lands, so only shown text is read. Both guards run it, each
with a debt list of its own (`RETIRED_TERM_DEBT`), exact and with a reason per entry, so paying
one off or adding one is a visible change.

"Text the window shows" is:

- JSX text, and a JSX child in braces;
- the value of an attribute a person reads or hears: `aria-label`, `title`, `placeholder`,
  `alt`, `label`, `EmptyState`'s `headline` and `body`, and a setting row's `help` and `hint`;
- a `label:` property, and the others like it (`SHOWN_PROPERTIES` in `uiStrings.ts`): a
  settings group's `title` and `note`, a field's `help` and `hint`, a provider's `says`, the
  sandbox table's `what` and `why`, an empty state's `headline` and `body`;
- the catalogue's titles and reasons: the second argument of `can` and `cannot` in
  `actions.ts`, and the third of `cannot`; and the labels, hints and unset values of the
  settings' control builders (`textAt`, `listAt`, `pickAt`, `onOffAt`).

Through an expression, both branches of a conditional and the right-hand side of `&&`, `||` and
`??` count as shown; the condition does not.

Copy written in Rust and sent to the window is held to the same rules by `copy.rust.test.ts`,
which reads the string literals of named places in the Rust source and builds nothing:

- purlis's own panels' empty states (a headline and a body, not the offer's id);
- the native menu's and the tray's labels;
- the core's `in_window` sentences, and what an `in_window` body calls in its own file: a
  helper function, a `const`, and `self.to_string()`, read as the type's `Display` and its
  variants' `#[error(…)]` text. A call into another module is either read (below) or named in
  `NOT_FOLLOWED` with why it holds no copy, or the check fails;
- the errors of every `#[tauri::command]` in `app/src-tauri/src`: the literals inside its
  `Err(…)`, `map_err(…)`, `ok_or(…)` and `ok_or_else(…)`, in its body and in every function of
  its own file it calls, and those call in turn, since an error passed on with `?` is shown as
  well. A helper or a `const` of the same file that builds the error is read too, whether it is
  called (`Err(gone(id))`) or passed by name (`.map_err(not_kept)`). The window shows a
  command's error word for word. A file is found by its attribute, so a new command is read
  without a list.

A format string's `{name}` reads as `…`. A new place the window shows Rust copy from is added to
its `PLACES`. A fault found there that cannot be fixed at once goes in its `KEPT`, with why, and
on #1156's checklist.

A helper or a `const` of another module, in the app or the core, is read too when its words
are the sentence: a `const`, or a function that returns a `String` or a `&str`, found by the
call's path (`crate::dispatchaway::refused`, `purlis_core::dispatchplace::CHANGED_SINCE_ASKED`)
or the file's `use`s. A call the check cannot find is named in `NOT_FOLLOWED` with why it holds
no copy.

**An error the core writes and a command passes on as it was said**
(`map_err(|err| err.to_string())`) reaches the window word for word, so the check reads it
where it is written: it finds the call the error comes from, reads the error type its signature
names, and holds that type's `Display` and `#[error(…)]` text to the same rules, every
variant's. A `String` error is read as its own `Err(…)` literals. What the check cannot read,
such as an error the operating system words or a method whose receiver it cannot tell, is
named in `NOT_READ` with why. A core sentence that is right for the terminal but uses a word
the window retires ("worktree", "plane") is listed sentence by sentence in `TERMINAL_WORDS`,
with the ticket that gives the window its own words: an `in_window`, as `worktree::Refusal`
and `contain::Refused` have. The list is empty.

**A closure a caller passes for the sentence** is read where the caller writes it. Where an
error calls a parameter of its function (`not_drawn: impl FnOnce(String) -> String`), the check
finds every caller of that function and reads what it passes there: a closure, a function by its
name, or the caller's own parameter, through its callers in turn. Only a parameter that answers
a `String` or a `&str` is read so.

Outside the check: copy assembled from parts at run time.
DS-8's audit of every surface reads what the check cannot, and a review reads every new string
against this page.
