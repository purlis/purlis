# Wayfinder map: links in a pane (PROTOTYPE, throwaway)

Spec #6208 (M73 · Links in a pane) and #5820's card, ticket #6211, branch `prototype/pane-links`.
It lists every surface and action, and where each one is reached from. It is the input for the UI
tickets, not code. Story names are under `PROTOTYPE/Pane links` in Storybook.

## Surfaces

| #   | Surface                                   | What it shows                                                                                                                         | Family                                         | Story                                                                         |
| --- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ----------------------------------------------------------------------------- |
| S1  | A link at rest                            | Nothing: the program's text as it printed it (D1)                                                                                    | xterm text                                     | `AtRest`                                                                      |
| S2  | A link on hover                           | An underline, and a tooltip: the resolved target in full, then where it is and what ⌘-click does (D2)                                | xterm decoration + `ui/tooltip` (anchored)     | `HoverAChat`, `HoverAClosedChat`, `HoverAFileLine`, `HoverAUrl`, `HoverAnOsc8Link` |
| S3  | The terminal's menu, over a link          | *Open link* and *Copy link* first, each with its target under it; then Copy, Paste, Select all; then the pane's rows (D3)             | `ui/menu` `ContextMenu` (`TerminalMenu.tsx`)   | `RightClickOverALink`                                                         |
| S4  | The terminal's menu, from the keyboard    | *Links in this pane…* in the link rows' place: no pointer, no link under it (D4)                                                      | same                                           | `MenuFromTheKeyboard`                                                         |
| S5  | *Links in this pane…*                     | The links on screen, newest first, a filter box, each row: its text, its kind, its target, "asks first" where it does (D5)            | `ui/dialog` + `ui/list` (Use skill's picker)   | `LinksInThisPane`                                                             |
| S6  | A closed chat's card                      | "#641 · Signed commits #5607", its outcome badge, when, who started it, where it ran, its report (4 lines), *Read it*, *Reopen* (D7, D8) | `ui/popover` `HeldPopover` + `ui/card` facts   | `ClosedChatCardFromALink`, `ClosedChatCardOutcomes`                           |
| S7  | The ask: a file outside the workspace     | The file and the workspace; *Don't open*, *Copy path*, *Open file* (D9)                                                               | `ui/dialog`                                    | `FileOutsideTheWorkspace`                                                     |
| S8  | The ask: an OSC 8 link to another address | Its text, and where it goes with the host in bold; *Don't open*, *Copy address*, *Open in browser* (D10)                              | `ui/dialog`                                    | `Osc8Mismatch`                                                                |
| S9  | A chat's tab tooltip                      | "Chat #641" as its first line, then the lines it says today (handed off from, work item, panes, tasks running) (D11)                  | `ui/tabs` `StripTab`'s tooltip                 | `NumberOnTabAndHead`                                                          |
| S10 | A chat's pane head                        | "#641" quiet, right after the chat's name (D11)                                                                                       | `PaneHead`                                     | `NumberOnTabAndHead`                                                          |

## Actions, and where each is reached from

| Action                          | Pointer                                              | Keyboard                                                    | Screen reader                     | What happens                                                                                          |
| ------------------------------- | ---------------------------------------------------- | ----------------------------------------------------------- | --------------------------------- | ----------------------------------------------------------------------------------------------------- |
| See where a link goes           | Hover it (S2)                                        | S5's row, or S3's under-line                                | S5's row (its target is its text) | -                                                                                                     |
| Open a link                     | ⌘-click (Ctrl-click off a Mac); S3 *Open link*       | ⇧⌘L (Ctrl+Shift+L), then Enter on a row (S5)                | S5                                | By kind, below                                                                                        |
| Copy a link                     | S3 *Copy link*                                       | S5, ⌘C on a row                                             | S5                                | The resolved target goes to the clipboard (D6)                                                        |
| Select text over a link         | A plain click or drag, as in any terminal            | -                                                           | -                                 | Selection. A plain click never opens                                                                 |
| Open *Links in this pane…*      | S3's row; the pane's ⋯ menu; the palette             | ⇧⌘L / Ctrl+Shift+L in the pane; S4                          | same                              | S5                                                                                                    |
| Open a chat that is open        | Open a link                                          | same                                                        | same                              | Its tab comes to the front, or it is shown in the tab or workspace it is in. Its own chat: nothing  |
| Open a closed or finished chat  | Open a link; the switcher's finished row (#5820)     | same; the switcher's row, Enter                             | same                              | S6, beside the link or the row                                                                        |
| *Read it* (S6)                  | Press                                                | Tab to it, Enter                                            | same                              | A view tab with the chat's saved output in the read-only terminal                                     |
| *Reopen* (S6)                   | Press                                                | same                                                        | same                              | The chat starts again on its conversation, in a new tab, under a new number (#6208 decision 3)      |
| Open a file in the workspace    | Open a link                                          | same                                                        | same                              | An editor tab, at its line and column                                                                 |
| Open a file outside it          | Open a link                                          | same                                                        | same                              | S7 first; *Open file* opens an editor tab                                                             |
| Open an http(s) address         | Open a link                                          | same                                                        | same                              | The person's browser, through the opener                                                              |
| Open an OSC 8 link whose text is another address | Open a link                         | same                                                        | same                              | S8 first; *Open in browser* opens it                                                                  |
| See a chat's number             | Hover its tab (S9); its pane head (S10)              | The tab's tooltip on focus; the head                        | The tab's description; the head   | -                                                                                                     |
| Find a chat by number           | The switcher, typing `641` or `#641` (#5820)         | same                                                        | same                              | Its row; a closed one's row opens S6                                                                  |

## What is not a link (drawn in `AtRest` with *Show what links* on)

- `chat 9999`, a number no chat in this project has.
- `#5820`: `#N` is an issue's number, never a chat. Issue links are M73's follow-up ticket.
- `src/old/gone.rs:10`, a path to no file.
- `ftp://…`, and every other scheme that is not http or https.
- Chat titles, ever.

## Decisions (self-grilled, each the recommendation; the person may change any)

- **D1. No mark at rest.** A link looks like the program's text until the pointer is on it, as in
  VS Code's and iTerm2's terminals. The pane's colours are the program's, and a dotted line under
  every path a harness prints would be noise. OSC 8 links are treated the same.
- **D2. The tooltip has two lines.** The first is the resolved target in full, in mono: a chat as
  "Chat #641 · title", a file as its path from the workspace with "line 42, column 7", a URL as it
  is, and an OSC 8 link as "Goes to <real address>". The second line says where that is and what
  ⌘-click does: "Finished 11:08 · ⌘-click shows its card", "Outside smart-ide · ⌘-click asks
  first". It is placed and delayed by the tooltip family (beside, 500 ms), and is hoverable.
- **D3. Link rows come first in the terminal's menu**, above Copy, Paste and Select all, because
  they are about what the pointer is on. Each says its target under its words, so the menu tells
  a keyboard or screen-reader user what the tooltip tells the pointer. *Open link* shows "⌘-click"
  at its end, as its key.
- **D4. Opened from the keyboard, the menu has no link rows.** Shift+F10 and the menu key have no
  pointer, so there is no link under it. *Links in this pane…* takes their place, and it is also a
  pane row in the pointer's menu.
- **D5. *Links in this pane…* is a quick picker in a dialog**, the same as *Use skill…*: a filter
  box, the links on screen (the viewport, not the history), newest (lowest) first, Enter opens by
  the same rules as ⌘-click (S7 and S8 still ask), and ⌘C copies. Its key is **⇧⌘L** (Ctrl+Shift+L
  off a Mac), which is free in `keys/registry.ts`. With no links on screen it says so: "No links on
  screen."
- **D6. Copy link copies the resolved target**, not the words: a file's absolute path with its
  `:line:col`, an OSC 8 link's real address, a URL as it is, and a chat as `chat 641`, so the copy
  links again when pasted into a pane.
- **D7. A closed chat's card is a popover**, beside the link or the switcher's row it came from,
  not a view tab. It is a glance with two ways on: *Read it* opens the view tab (the read-only
  terminal), and *Reopen* opens the chat. That keeps the view-tab default (2026-09-23) for the
  thing worth keeping open, the saved output. *Read it* is the primary answer, and *Reopen* is
  plain, because it starts a program.
- **D8. The card says its outcome in a badge**: Finished, Blocked, Failed, or Closed when the person
  closed it. Its report is cut at 4 lines, with the whole report under *Read it*. A chat that sent
  none says "It sent no report." Its line "it is in no list of open chats now" answers #5820's
  "finished or lost?".
- **D9. The ask for a file outside the workspace starts on *Open file*, and Return opens it.**
  Opening it only shows it in an editor tab and runs nothing, so the ask is a notice more than a
  guard. *Copy path* is the third answer. v1 has no "don't ask again for this folder".
- **D10. The ask for an OSC 8 mismatch starts on *Don't open*, and Return never opens it.** *Open in
  browser* is drawn plain, not primary, and the real host is in bold so a look-alike is seen.
- **D11. "#641" goes on the tab's tooltip, as its first line ("Chat #641"), and in the head, quiet,
  right after the name.** A head narrower than 520 px drops it together with "harness · workspace",
  and the tab's tooltip still has it.
- **D12. Off a Mac every word says "Ctrl-click" and "Ctrl+Shift+L".** The stories show the Mac's.

## Open questions for the person (each with the recommendation)

- **Q1. Which OSC 8 links ask?** Spec #6208 says one "whose text differs from its target". Taken
  literally, every link a program marks with words ("the docs", "PR 12") would ask each time.
  **Recommend:** ask only when the text reads as an address or a path and names another target
  (another host, or another file). A link with plain words opens without asking, and its hover
  still leads with the real address (D2).
- **Q2. OSC 8 `file://` targets.** Programs (`ls --hyperlink`, compilers, some harnesses) mark paths
  as `file://host/abs/path`. Under "every other scheme is refused" these would not open.
  **Recommend:** a `file://` target on this machine is a file link, with the same rules: it opens
  in the editor tab, a file outside the workspace asks, and one on another host is refused.
- **Q3. What a link to the pane's own chat does.** `chat 652` in chat #652's pane. **Recommend:** it is a
  link (its tooltip says "This chat"), and ⌘-click does nothing more, rather than a link that
  is skipped.
