# Audit #2877: every prototyped behaviour is a written requirement

Map #2788 (M66 · UI system). Sources:

- `prototype/start-chat-dialog`: `app/src/StartChat.prototype.html`, **variant B** (chosen in #2798).
- `prototype/settings-page`: `app/src/settings/SettingsTab.prototype.html`, **variant A** (chosen in #2799).
- `prototype/switcher`: `app/src/Shell.prototype.html`, **variant D** with the theme picker (chosen in #2868, #2800 and #2805).
- `prototype/tab-strip`: `app/src/TabStrip.prototype.html`, superseded. Only behaviour that does not appear on the switcher is listed (R142–R149).

Only the chosen variants are audited. The unchosen variants (Start a chat A and C; Settings B and C; Shell A, B and C; the "Bar: column" layout) are not requirements. The one exception is #2814·4: its "Set here" filter came from Settings C, so it is listed under not-prototyped.

**How to read the status column**

- **covered**: a resolution comment states the behaviour. The citation names the ticket and its item. When the resolution states the behaviour but not its numbers, the numbers in the "Prototype shows" column are the values to build: the spec carries them on the acceptance line that names the branch.
- **missing**: the prototype shows the behaviour, but no resolution comment states it. The spec needs to write it, or rule it out explicitly.
- **⚠ deviation**: the prototype contradicts a ruling. The ruling wins. The build follows the ruling, not the pixels, and the spec's "match the prototype on screen" condition must exempt the item. All of these are collected again in the final section.

**How items are cited**

- #2798: "d1–d4" are the chosen-design bullets and "rule 1–5" are the dialog rules.
- #2799: "d1–d5" are the chosen-design bullets and "rule 1–2" are the rules.
- #2868: "·1–·19" are its numbered items.
- Other tickets use "·n" for their numbered items.
- "map: Dialogs" and "map: Accessibility" are bullets of the #2788 body.

---

## 1. Start a chat (variant B)

| # | Prototype shows | Status | Where |
|---|---|---|---|
| R1 | A modal over a dimmed window. Scrim `rgba(8,8,10,.6)` in dark and `rgba(20,18,28,.36)` in light. The dialog is centred horizontally. | covered | #2798 d2 (implied modal); #2789 dialog numbers |
| R2 | The dialog is anchored at a fixed top, 6vh below the window's top with 16 px side gutters. A change in content moves only its bottom edge. | covered | #2798 rule 2 |
| R3 | The width comes from the dialog's size class, never from its content. List-beside-details is 780 px; the standard dialog is 560 px. Either is capped at the window width minus 32 px. | covered ⚠ | #2798 rule 1. ⚠ #2789 caps width at 600 px; 780 px breaks that, so the spec must pick one. |
| R4 | Fixed height `min(620px, window)`. It does not grow or shrink with the number of personas. | covered | #2798 d2, rule 4 |
| R5 | Maximum height `min(85%, 100% − 32px)`. | covered ⚠ | map: Dialogs (~85%). ⚠ #2789 says viewport − 48 px; the spec must pick one formula. |
| R6 | Three rows: header, body and footer. The header and footer are always in view. Only the two panes scroll, each on its own. | covered | #2798 d2, rule 4; #2789 |
| R7 | Surface: panel fill, radius 12, shadow `0 18px 48px rgba(0,0,0,.28)`. | covered | #2804·2 (corners: panels 12); #2794·4 (shadow as elevation step) |
| R8 | Header: the title "Start a chat" at 16/600, padded 18/20/6. Below it, a subtitle in muted 12 px naming the workspace and where the profiles come from ("In smart-ide · purlis's built-in profiles"). | missing | The subtitle and its content appear in no resolution. |
| R9 | Panes: left 260 px and right flexible, 16 px gap. Body padding 10/20/14. | covered | #2798 d1 (values from the prototype) |
| R10 | The left pane has a filter field at its top (32 px, field radius 8, window fill, no border), with the list under it 8 px below. | covered | #2798 d1; map: Dialogs (filter for a list over ~7 items) |
| R11 | The filter's placeholder names the total: "Filter 28 personas". | missing | |
| R12 | Filter matching is a case-insensitive substring of the persona name. | missing | The matching rule is not written. M67 fuzzy matching (#2881) is for the palette only. |
| R13 | While filtering, a selected persona that drops out of the matches gives the selection to the first match. | missing | |
| R14 | Re-rendering the list keeps focus and the caret position in the filter. | missing | |
| R15 | A filter with no match shows the empty line "No persona is called “x”." | missing | The empty state and its copy are not written. "States" is still unspecified on the map. |
| R16 | ↑/↓ in the filter move the selection. They stop at the first and last match and do not wrap. | covered (move) / missing (clamp) | #2798 d1 states ↑/↓. The no-wrap rule is not written. |
| R17 | ⏎ starts the chat from the filter, or from anywhere that is not a text field. ⏎ in the Name field does not start the chat. | covered (⏎) / missing (Name exception) | #2798 d1 |
| R18 | Esc closes the dialog. | covered | #2796·5; #2814·1 (Esc/⏎ paths in the key registry) |
| R19 | Persona rows: minimum height 32, 4/8 padding, field radius 8. A 22 px monogram tile (radius 6, initials of the first two hyphen words, 600/10) comes first. The name ellipsises. | missing | Neither the row anatomy nor the monogram rule is written. |
| R20 | The project's default persona carries a "project default" chip: a wash pill in accent text at 11 px. | missing | |
| R21 | Row states: hover takes the card fill. Selected takes the wash with primary text. No size changes. | covered | #2804·2 (states, washes); #2868·18 |
| R22 | The selected row is scrolled into view (`nearest`) on open and on selection, but not while the person is typing. | missing | |
| R23 | The right pane's persona header has a 36 px monogram, the name at 15/600 and a role line in quiet 12 px: "Project default · Control Plane Steward" or "Persona of this project". | covered (pane) / missing (role line) | #2798 d1 |
| R24 | Harness options are cards: icon and name on the first line, the billing note under them, and no radio circle. | covered | #2798 d3 |
| R25 | Every card is the height of the tallest (`grid-auto-rows: 1fr`). The gap is 6, padding 10/12, card radius 10, window fill. | covered | #2798 d3, rule 5 |
| R26 | The chosen card has three marks: the wash, a 1.5 px inset accent edge and a 16 px check. The check's space is always reserved, so choosing never shifts the text. The chosen icon turns accent text. | covered | #2798 d3, rule 5; #2868·18 |
| R27 | Card hover takes the card fill. | covered | #2804·2 states |
| R28 | Billing-note copy: "Billed to whatever ‹harness› is signed in with." For Claude Code it adds "Runs in a terminal; it has no ACP agent purlis can talk to." For the others it adds "The harness's default model." | missing | The wording appears in no resolution, and the note's source (the profile definition) is not named. |
| R29 | Harness glyphs are 14 px stroke icons: a spark (Claude Code), braces (opencode) and a terminal (Codex). | covered | #2798 d4; #2866·1 |
| R30 | The Name field is labelled "Name". Its placeholder is "‹persona› ‹next number›" and follows the selected persona. The help text reads: "Left empty, the chat is named after its persona and its number. You can rename it from its tab later." | covered (field) / missing (live placeholder, copy) | #2798 d1 names "name" only. |
| R31 | A checkbox reads "Draw purlis's footer in this chat", with the help text "The footer names the chat's workspace, which the panels name only for the focused chat." | covered (option) / missing (copy) | #2798 d1 ("the footer option") |
| R32 | The harness choice, the typed name and the footer option survive a change of persona. | missing | |
| R33 | The footer leads with a summary in quiet 12 px, "steward on ✳ Claude Code", which ellipsises on overflow. Cancel (ghost) and **Start chat ⏎** (primary) are trailing. The ⏎ hint is a mono 11 px glyph inside the button. | covered | #2798 d2, d4; #2789 (default trailing, Cancel leading) |
| R34 | The footer is separated from the body by a 1 px line. | missing ⚠ | #2804·2 bans hairlines on *controls* only. Whether a dialog's footer rule is allowed is not decided. |
| R35 | Buttons are 28 px (md) with radius 7. The primary is soft-accent (wash plus accent text, hover 22% accent mix). The ghost hovers to the card fill. | covered | #2793·4; #2804·2 |
| R36 | Below 700 px the panes stack and the list caps at 180 px. | missing | Narrow-window behaviour is not written for any dialog. |

## 2. Settings (variant A)

| # | Prototype shows | Status | Where |
|---|---|---|---|
| R37 | The Settings tab is a 236 px side column (window fill, 1 px divider) beside a scrolling main area. The page is at most 860 px wide with padding 22/28/40. | covered (layout) | #2799 d5 (values from the prototype) |
| R38 | The side column holds the title "Settings" (15/600), then the You / Project / Workspace segmented switch, then the page list. | covered | #2799 d5 |
| R39 | Scope switch: a segmented control on the card fill. The selected segment has a window fill, a 1 px shadow and primary text. | covered ⚠ | #2799 d5. ⚠ The prototype also sets weight 500 on the selected segment, which #2800·2 and #2868·18 forbid. |
| R40 | Nav items: 30 px, radius 8, padding 0 10. Hover takes the card fill. The current page takes the wash with primary text. | covered ⚠ | #2799 d5; #2804·2. ⚠ The prototype sets weight 500 on the current page; the ruling forbids it. |
| R41 | Sub-pages are indented 22 px in muted text. Every item at one level shares one left edge. | covered | #2799 rule 1 |
| R42 | A group's disclosure is a separate 26 px toggle at the trailing end of the parent row, apart from the page button. It is labelled "Expand/Collapse ‹page›" with `aria-expanded`, and its hover takes the hover fill. | covered | #2799 d5, rule 1 |
| R43 | The chevron rotates 90° when open, in 150 ms. | covered (rotation) / missing (duration) | See R150. |
| R44 | A group opens itself when one of its pages becomes current. | covered | #2799 d5 |
| R45 | Choosing the group's own page (Sandbox) also opens the group. | missing | |
| R46 | A closed group shows the sum of its children's waiting counts (amber, 11 px, trailing). While open, the count moves to the child row ("Your hosts 18"). | covered | #2799 d5, rule 2 |
| R47 | Which groups are open is remembered across reloads. Default: Sandbox open. | missing | |
| R48 | The page header has the title (18/600), a blurb (muted, at most 64ch) and the line "Kept in `‹file›` · Edit as TOML", where Edit as TOML is a ghost sm button. | covered | #2799 d4 |
| R49 | The file named follows the level: You gives `~/.config/purlis/purlis.toml`, Project gives `charter.toml`, and Your hosts gives `purlis.local.toml`. | covered | #2799 d4 |
| R50 | Row grid: the label column is `minmax(180px, 38%)`, then the control column, then a 64 px action column. The gaps are 6/20 and each row is padded 12 vertically. | covered | #2799 d1 (values from the prototype) |
| R51 | Rows are separated by 1 px lines; the first row in a group has none. | covered ⚠ | #2799 d1. ⚠ Same open hairline question as R34: these are row rules, not control borders. |
| R52 | The label is 500 weight with help text under it in quiet 12 px. There is no hover-only info mark. | covered | #2799 d1 |
| R53 | A 6 px accent dot before the label marks a value set at this level, with the tooltip "Set at this level". | covered | #2799 d1 |
| R54 | Every control starts at the same x. Select and text fields are at most 320 px wide, 30 px tall, radius 8, borderless on the window fill. | covered ⚠ | #2799 d1. ⚠ 30 px (and 32 px in Start a chat) is neither of #2793·4's two control sizes (24 and 28). Field heights need a ruling. |
| R55 | An inline secondary action can sit beside a control, such as "Show steward" (ghost sm) after Default persona. | missing | |
| R56 | One ⋯ icon button (28 px, quiet, tooltip "Reset, move to another file") in the same place on every row. | covered | #2799 d1 |
| R57 | The row menu holds "Reset to ‹default›", then "Move to purlis.local.toml" and "Move to your settings" (only for keys that can move), then "Copy key `‹key›`". Separators sit between the sections. | covered (Reset, Move) / missing (Copy key, conditional Move) | #2799 d1 |
| R58 | "Reset to ‹default›" is disabled when the value is not set at this level. | missing ⚠ | The prototype draws disabled at `opacity:.5`. #2794·4 bans opacity and #2804·2 says disabled takes the faint tier. |
| R59 | The menu opens under its button, right-aligned to it (at least 8 px from the window edge), at least 200 px wide, card fill, radius 10, with a shadow. It closes on an outside click, on Esc and on choosing an item. | covered (Esc) / missing (placement, outside click) | #2796·5 |
| R60 | Groups have 13/600 headings. A repo group carries one explanatory line: "This repo’s own settings, beside the project’s. The repo is listed in inventory/repos.json." | covered | #2799 d2 |
| R61 | Rarely used groups (Branches, Deprecated) are collapsed by default as "Branches · 1", with the count after the title. | covered ⚠ | #2799 d2. ⚠ The prototype puts the chevron *before* the label, which breaks #2799 rule 1 (trailing). Build it trailing. |
| R62 | A deprecated key carries an amber "deprecated" badge after its label. | missing | |
| R63 | Switch: a 34×20 pill with a 14 px knob that slides in 150 ms. On, it takes the accent fill and a window-coloured knob. | covered (family) / missing (anatomy, duration) | #2793·3 lists the switch. |
| R64 | Selects carry a drawn chevron in quiet. Placeholders take the quiet tier. | covered | #2803·2 (placeholders take quiet) |
| R65 | Your hosts is a table in a rounded window card. Columns: a checkbox; Host in mono with `:port` in quiet; Status; Added ("‹when› · ‹by›", where by is "a chat's block" or "you"); and a fixed trailing action column. | covered | #2799 d3 |
| R66 | Status badges are "Waiting for you" (amber wash) and "Confirmed" (ok text). | covered (status) / missing (copy) | #2799 d3 |
| R67 | Row actions: Confirm (sm, only on waiting rows) and Remove (ghost sm). | covered | #2799 d3 |
| R68 | Waiting rows sort first. | covered | #2799 d3 |
| R69 | A filter field (at most 280 px) reads "Filter 19 hosts" and matches by substring. | covered (filter) / missing (count in placeholder, matching rule) | #2799 d3 |
| R70 | With nothing selected, the toolbar shows "Confirm all N waiting" (primary sm) and "Add host". With rows selected it swaps to "N selected", Confirm and Remove. | covered (Confirm all, bulk) / missing (Add host, the swap) | #2799 d3 |
| R71 | A header checkbox selects or clears every row. | covered | #2799 d3 |
| R72 | Confirm and Remove report back with "Confirmed ‹host›." and "Removed ‹host›." Confirm all reports "Confirmed every waiting host." | missing | The feedback channel and its copy are not written. |
| R73 | A host filter with no match draws an empty table. The prototype has no empty line. | missing | The table needs an empty state. |
| R74 | The table keeps a minimum width of 560 px and scrolls sideways inside its card. Header cells are uppercase 11/600 muted. | missing | |
| R75 | Below 760 px the nav becomes a sideways-scrolling row and each row's label goes above its control. | missing | |

## 3. Shell (variant D)

### 3a. Project rail

| # | Prototype shows | Status | Where |
|---|---|---|---|
| R76 | A 52 px rail column at the far left, below the title bar. | covered | #2868·1 |
| R77 | Project buttons are 36 px with radius 9. Each shows a two-letter monogram (initials of the hyphen parts, 600/11) in the project's hue, on that hue at 13% alpha. | covered | #2868·1 |
| R78 | The current project is ringed: a 2 px window-coloured gap, then a 1.5 px accent ring. | covered | #2868·1 |
| R79 | A waiting badge (16 px, amber, offset −5/−5 at the top right) shows the project's total waiting count. | covered | #2868·1 |
| R80 | "+" opens a project (tooltip "Open a project"). | covered | #2868·1 |
| R81 | Choosing a project switches to it and lands on its **first** workspace, closing any open switcher. | missing | Which workspace a project opens on (first or last used) is not written. |
| R82 | The rail's last button, pushed to the bottom by a spacer, shows a sidebar icon with the tooltip "Hide sidebar (shows at the left edge)". It switches the rail to auto-hide. | covered | #2868·2 |
| R83 | The rail mode (always shown or auto-hide) is remembered across launches. | missing | |
| R84 | Hiding: the rail's column closes from 52 to 0 over 260 ms on `cubic-bezier(.22,.8,.24,1)`. The rail moves −56 px and fades out (200 ms), and the content glides with the column. Showing is the reverse. Hiding also closes the switcher. | covered | #2868·4 (values from the prototype) |
| R85 | Under reduced motion, hiding and showing are instant. | covered | #2868·4, ·19 |
| R86 | Peek: in auto-hide, a pointer within 6 px of the window's left edge, below the 40 px title bar, slides the rail in over the content. The peek is 56 px wide, window fill, right corners 12, with the shadow `8px 0 24px`. It enters in 220 ms (from −100%, opacity .4). | covered | #2868·3 (values from the prototype) |
| R87 | The peek leaves when the pointer is more than 70 px from the edge, sliding out in 200 ms. Under reduced motion it neither enters nor leaves with animation. | covered | #2868·3, ·19 |
| R88 | The peek's bottom button ("Keep sidebar shown") pins the rail back to always shown. The rail settles in place without replaying the slide. | covered | #2868·3 |
| R89 | Rail tooltips name the project and appear 10 px to the right of the button. | covered (tooltip) / missing (placement) | #2793·6 |

### 3b. Title bar and pins

| # | Prototype shows | Status | Where |
|---|---|---|---|
| R90 | The title bar is 40 px tall, with 78 px of left padding for the window controls and a 10 px gap. | covered (order) | #2805·4 (values from the prototype) |
| R91 | While the rail is hidden, a 28 px show-sidebar button is the first control after the window controls. | covered | #2868·2; #2805·4 |
| R92 | With the rail shown, the title bar reads "‹project›" in quiet 500, then "/" in faint, then the pinned strip. | covered | #2868·6 |
| R93 | With the rail hidden, the project name becomes a crumb button: a 20 px mark, the name and ⌄, 28 px tall, radius 8, with the card fill on hover or open. It opens the two-column switcher. | covered | #2868·5 |
| R94 | The two-column switcher is 560 px wide (window minus 24 at most) and at most 440 px tall. It opens 6 px under its opener, kept 12 px inside the window. Panel fill, radius 12, shadow. | covered (switcher) / missing (geometry) | #2868·5 |
| R95 | Its filter reads "Go to a project or workspace…" with a ⌘P key badge. The filter takes focus on open with the caret at the end. | covered | #2868·5 |
| R96 | The Projects column has a mark, the name and a waiting count. The second column is headed "Workspaces in ‹project›", with rows carrying a name, a count and a pin toggle. | covered | #2868·5, ·7 |
| R97 | Clicking a project in the switcher only previews its workspaces in the second column. It does not switch. | missing | |
| R98 | The filter matches a project by its name or by any of its workspaces' names, and narrows the workspace column. Typing selects the first matching project. | missing | |
| R99 | Empty lines read "No project matches." and "No workspace matches." | missing | |
| R100 | The footer's key hints read "↑↓ move · → workspaces · ⏎ go · esc close". | covered (keys) ⚠ | #2868·5. ⚠ The prototype shows a "→ workspaces" hint but → does nothing. Either define → (move focus to the workspace column) or drop the hint. |
| R101 | The pinned strip shows the current project's pins as chips: 28 px, padding 0 10, radius 8, quiet 13 px text. Hover gives primary text on the card fill. The current chip takes the card fill with primary text. Each chip shows a 16 px amber waiting count. | covered | #2868·6 |
| R102 | Chip tooltips read "‹name› · ⌘N" for the first nine pins. | covered | #2868·8; #2814·2 |
| R103 | The strip is at most 62% of the title bar. It scrolls sideways with its scrollbar hidden. While it overflows, a sideways wheel over the strip scrolls the strip instead of switching. | covered | #2868·6, ·9 |
| R104 | After any step, the current chip is scrolled into view (smooth, or instant under reduced motion). | missing | |
| R105 | When the current workspace is unpinned, a dashed chip "‹name› +" (muted, 1 px dashed line, tooltip "Pin ‹name›") follows the pins. Clicking it pins the workspace. | covered | #2868·7 |
| R106 | A ⌄ at the strip's end (26 px, radius 7, tooltip "All workspaces · ⌘P", `aria-expanded`) opens the workspace popover. ⌘P does the same and toggles it. | covered | #2868·7 |
| R107 | The workspace popover is 320 px wide. Its filter reads "Go to a workspace in ‹project›…". Rows have a name, a count and a 28 px pin toggle (faint; accent when pinned; tooltip "Unpin" or "Pin to the title bar"). The footer reads "↑↓ move · ⏎ go · esc close". | covered | #2868·7 |
| R108 | The workspace popover ends with a "+ New workspace…" row. | missing | |
| R109 | Popover rules: ↑/↓ move and stop at the ends. ⏎ goes. Esc and an outside click close it. Typing selects the first match. Toggling a pin keeps the popover open. | covered (↑↓ ⏎ Esc) / missing (clamp, outside click, typing select, pin keeps open) | #2868·5, ·7 |
| R110 | Pins are per project, unlimited, kept in the order they were pinned, and remembered across launches. Unpinning happens only from the popover's toggle; chips have no ×. | covered (per project, no limit) / missing (order, persistence, no × on chips) | #2868·6, ·7 |
| R111 | ⌘1–⌘9 jump to the project's first nine pins. Ctrl takes ⌘'s place off macOS. | covered (⌘) / missing (Ctrl mapping) | #2868·8 |
| R112 | A sideways swipe (trackpad deltaX, or shift-wheel) over the title bar only steps one pin per gesture, wrapping. The trigger is more than 60 px of accumulated travel; the gesture ends after 220 ms of wheel quiet, and momentum is ignored until then. | covered | #2868·9 (thresholds from the prototype) |
| R113 | A step from an unpinned workspace goes to the first pin (next) or the last pin (previous). Choosing the current chip does nothing. | missing | |
| R114 | A workspace switch slides the side panel, the work column, the right panel and the bottom halves ±28 px with a fade, in 260 ms on the shared ease, in the direction of the pin order. The bars, the rail and the title bar stay still. There is no slide under reduced motion. | covered ⚠ | #2868·10. ⚠ In the prototype, ⌘1–9 and the switcher's rows switch *without* the slide; ·10 says every switch slides. |
| R115 | Status pills sit at the right: 24 px, full radius, 12 px. "✋ N waiting" (amber wash, the total across **every** project), "Saved", "About", and the version (accent wash). | covered (pills) / missing (the waiting count spans all projects) | #2805·4 |

### 3c. Activity corners

| # | Prototype shows | Status | Where |
|---|---|---|---|
| R116 | The left bar is tabs across the top of the side panel: 32×30 icons, a bottom rule, the top group (Chats, Explorer, Search), a spacer, then the bottom group (Branch folders, Changes) at the right end. | covered | #2805·3; #2868·11 |
| R117 | The right bar is a 44 px column of 36 px icons, radius 9. Top group: Needs you, Personas, Vaults. Then a spacer. Bottom group: Memory, Todos, History. | covered | #2868·11 |
| R118 | Icon states: rest is muted. Hover gives the card fill and primary text. On gives the wash and accent text, with `aria-pressed`. Every icon has a tooltip with its name. | covered | #2868·11; #2804·2; #2793·6 |
| R119 | Count badges sit at the top right: 15 px, 600/10, hover fill, with a 2 px ring in the ground colour. The prototype shows Chats 1, Needs you 2 and Todos 7. | covered | #2868·11 |
| R120 | The Needs-you count is "hot": a red fill (`#8a3a3a`) with white text. | missing | The colour role and its contrast floor are not written. |
| R121 | A left top icon switches the side panel's view. Clicking the current view again does nothing, because the side panel never closes. | covered (switch) / missing (re-click no-op) | #2868·12. ·12's "clicking again closes" reads as covering only the right panel and bottom halves. Say so. |
| R122 | A side-view change fades the list in from 4 px below in 180 ms ease-out. There is none under reduced motion. | covered (rule) / missing (values) | #2868·17, ·19 |
| R123 | The side panel's header is an uppercase 11/600 label. Chats shows "‹workspace› · ‹project›", with the project faint. Explorer shows the workspace name. Other views show their own name. | missing | |
| R124 | A right top icon toggles the right panel, 300 px wide between the work and the right bar. It enters from +24 px with a fade in 240 ms, only when it was closed. Switching from one right view to another does not replay it. | covered | #2868·12, ·17 |
| R125 | The right panel's header is a 13 px title with a 24 px × that closes it. Clicking the open icon again also closes it. | covered | #2868·12 |
| R126 | A bottom icon opens its half of the bottom panel. The row is `minmax(160px, 34%)` tall and spans from the side panel to the right panel, between the bars. The halves sit 8 px apart, and one half alone takes the full width. | covered | #2868·12 |
| R127 | A bottom half's header is an uppercase 11/600 label with ×. It enters from +16 px with a fade in 220 ms, only on its own opening; the other half never replays. | covered | #2868·12, ·17 |
| R128 | Closing the right panel or a bottom half has no exit motion: it disappears. | covered ⚠ | #2868·17 says a part animates when it "appears or leaves". The prototype only animates on appear. Write whether the leave animates. |
| R129 | Explorer view: a tree indented 14 px per level, with ▾/▸ twisties in quiet and an amber git mark at the trailing end. | missing | The view's content is not in any resolution. |
| R130 | Changes view, in two columns `minmax(180px, 34%)` / rest. The left column has a message box ("Describe this save…"), a full-width wash button "Save N staged" and the branch hint "main ← purlis/save/‹machine›". The right column has the groups "Staged · N" and "Not staged · N", whose rows carry a status letter (M amber, A green), the file name, the path and "+42 −18" in mono quiet. | missing | |
| R131 | Todos view: an "Add a todo…" input. ⏎ adds the todo at the top and keeps focus. A checkbox toggles done, which strikes the text through. | missing | |
| R132 | Needs-you view: ask cards (card fill, radius 10) showing the chat name in bold, the workspace in quiet at the right, an amber status line and the question, with "Go to chat" (primary sm) and "Later". | missing | The view is drawn but no resolution states it. It may belong to the M67 Inbox work; say so. |

### 3d. Session tabs

| # | Prototype shows | Status | Where |
|---|---|---|---|
| R133 | Session tabs sit on the content panel. The strip is as wide as the panel and the first tab starts at its left edge. Tabs are 31 px tall, at most 240 px wide, padded 0 10 0 12, radius 8 8 0 0, 6 px apart. The selected tab takes the panel fill and flares into the panel with 8 px inverse corners. The first tab has no left flare: the panel's top-left corner squares only while the first tab is chosen. | covered | #2868·14; #2800·1 |
| R134 | An unchosen tab's hover is a card-fill pill inset 3 px at the top and 5 px at the bottom, radius 7, so it never touches the panel. The text turns primary. Selection changes colour and fill only. | covered | #2868·15, ·16; #2800·2, ·3 |
| R135 | A tab carries a 6 px live dot (accent) or waiting dot (amber), the label (ellipsised) and a 16 px × in faint that takes the hover fill on hover. | covered (× in faint) / missing (live and waiting dots) | #2803·1 |
| R136 | A "+" new-chat button ends the strip. | missing | |
| R137 | Tabs shrink (`flex: 0 1 auto`, ellipsis) before the strip scrolls sideways, with its scrollbar hidden. | missing | Tab overflow is not written. |

## 4. Themes and parts

| # | Prototype shows | Status | Where |
|---|---|---|---|
| R138 | A theme picker (Purlis, Classic, Underline) restyles the tabs, the title-bar chips, the rail, the activity items and the panels through part keys only. A Keys box prints each theme's `{ parts }` JSON. | covered | #2800·4 |
| R139 | **Purlis** keys: `tabs.layout.attach: panel`, `tabs.tab.corner-radius: 8 8 0 0`, `tabs.tab.selected.fill: surface.raised`, `tabs.tab.selected.flare: 8`, `tabs.tab.hover.fill: surface.card`, `tabs.tab.hover.inset: 3 0 5`, `tabs.layout.dividers: off`, `titlebar.workspace.corner-radius: 8`, `titlebar.workspace.selected.fill: surface.card`, `rail.button.corner-radius: 9`, `activity.item.corner-radius: 9`, `activity.item.selected.fill: accent.wash`, `panel.corner-radius: 12`. | covered | #2800·1, ·3, ·4 |
| R140 | **Classic** keys and look: square tabs, no flare, 1 px dividers between tabs, a full-cell hover (inset 0), square panels, title-bar chips 40 px tall with a 2 px accent underline when current, rail buttons radius 4, square activity items with a 2 px left accent indicator when on. | covered ⚠ | #2800·4; #2805·2. ⚠ The prototype's Classic keeps the 8 px gutter, but #2805·2 says Classic draws flush (gutter 0). Build per #2805·2. |
| R141 | **Underline** keys and look: `attach: strip`, tab padding 0 2 with a 24 px gap, the selected tab transparent with a 2 px accent bottom indicator, no hover pill, panels radius 6, pill chips with the current chip in wash and accent text, round rail and activity buttons. | covered | #2800·4 |
| R142 | *(tab-strip)* The family layout options `tabs.layout.sizing: equal \| content`, `tabs.layout.dividers` and `tabs.layout.gap`. | covered | #2794·9 |
| R143 | *(tab-strip)* An indicator part with `position: top \| bottom`, `height`, `fill` and `corner-radius`. | covered (part) / missing (`position` property) | #2794·2 names the indicator; #2794·4's closed property list has no position. |
| R144 | *(tab-strip)* A `tabs.count` part (pill radius, fill, text) and a `tabs.closer` corner-radius. | covered (closer) / missing (count part on tabs) | #2794·2 lists strip, tab, indicator, label and closer, but no count. |
| R145 | *(tab-strip)* `tabs.tab.max-width` (Classic 240). | missing | Not in #2794·4's property list. |
| R146 | *(tab-strip)* File tabs: content-sized, rounded on top, joining the editor toolbar (file name, path) under them. | covered | #2805·5 |
| R147 | *(tab-strip)* Strip border colour and width, and panel border width (Classic 1). | covered | #2794·4; #2805·2 |
| R148 | *(tab-strip)* "Everything not named falls back to the family default, which reads the global tokens." | covered | #2794·6 |
| R149 | *(tab-strip)* `tabs.tab.selected.font-weight` (500 in Purlis, 600 in Underline). | covered ⚠ (refused) | #2800·2 refuses size-changing properties inside a state. The tab-strip keys are invalid under the ruling; the reference examples are R139–R141. |

## 5. Cross-cutting rules

| # | Prototype shows | Status | Where |
|---|---|---|---|
| R150 | **Motion tokens.** One ease, `cubic-bezier(.22,.8,.24,1)`, with durations of 260 (rail and workspace slide), 240 (right panel), 220 (bottom half, peek in), 200 (peek out, rail fade), 180 (side view) and 150 (chevron, switch knob). | missing | Only 260 and the ease are written (#2868·4). "Density, zoom and motion" is still unspecified on the map. The spec needs a motion token scale. |
| R151 | A part animates only when it appears (or leaves). A re-render never replays it. | covered | #2868·17 |
| R152 | Every motion has a reduced-motion form with none. There is a global `transition: none` and every keyframe has an off rule. | covered | #2868·19 |
| R153 | **No shift.** A state change (hover, selected, focus, checked) never changes size or moves a neighbour. Reserved slots (the check, the help text) cover this. | covered ⚠ | #2868·18; #2800·2; #2798 rule 3. ⚠ Deviations: R39, R40 and the rail-hidden switcher. |
| R154 | **Grey tiers.** The prototypes use the old values: quiet `#8a8896` and faint `#6f6d7a`. | covered ⚠ | #2803·1: quiet `#91909d`, faint `#74727f`. Build with the ruled values. |
| R155 | **Informational text in faint**, which must take quiet: todo and commit placeholders, the Changes path (`.cpath`), the "· project" in the side header, the branch hint, and the struck-through done todo. | covered ⚠ | #2803·2 (placeholders, paths). The prototype breaks it in each of these places. |
| R156 | Faint for non-text marks: the tab ×, the "/" separators, the pin icon and chevrons. | covered | #2803·1 |
| R157 | **Corners**: 7 for controls, 8 for fields and chips, 10 for cards, 12 for panels, full for pills. | covered | #2804·2 |
| R158 | Radii the prototypes use that the scale lacks: 5 (segment insides, the tab ×), 6 (monogram tiles, the right panel's ×), 9 (rail and activity buttons), 7 (the hover pill, ⌄). | missing | Fold them into the scale, or map each to a scale step. |
| R159 | **Washes**: accent at .12 and amber at .10–.14. | covered | #2804·2 |
| R160 | **Type**: Plex at 12–13 px for UI and 11 px for counts and labels, JetBrains Mono for code. Headings at 15, 16 and 18, and small mono at 11.5 and 12.5. | covered (12/13/11) / missing (heading sizes) | #2804·2. The type scale is still unspecified on the map. |
| R161 | Uppercase 11/600 section labels with .04–.05em tracking (dialog labels, table heads, side-panel and bottom-panel headers). | missing | |
| R162 | **Focus**: a 2 px accent ring at 2 px offset on `:focus-visible`. | covered | #2804·2; map: Accessibility |
| R163 | **Dialog rules**: width by size, fixed top, reserved room for changing text, fixed height with a list beside details, equal options with choice shown by more than colour. | covered | #2798 rule 1–5 |
| R164 | Lists over ~7 items get a filter: personas (28), hosts (19), projects and workspaces. | covered | map: Dialogs |
| R165 | **Popover anatomy**: a filter row with a rule under it, a scrolling list, and a key-hint footer ("↑↓ move · ⏎ go · esc close") in quiet 11.5 px with mono key caps. | missing | Only "shortcuts on rows" is written (#2814·2). The key-hint footer is not. |
| R166 | **Tooltips** come from one shared component (no `title=`). They appear on hover, 10 px right of the target, with card fill, radius 8 and 12 px text. | covered (shared tooltip) / missing (placement, delay) | #2793·6 |
| R167 | Horizontal strips (title-bar pins, session tabs) scroll with their scrollbar hidden. | missing | |
| R168 | Filters keep focus and the caret across a re-render (Start a chat, the hosts filter, both popovers). | missing | Generalises R14. |

---

## 6. Resolution items no prototype shows (need their own acceptance)

| # | Item | Source |
|---|---|---|
| N1 | Max dialog width 600 px, at most three actions, no nested dialogs, 8 px gaps, targets at least 24×24. | #2789 dialog numbers |
| N2 | The main action is in one place in every dialog (one rule for forms and questions). | map: Dialogs |
| N3 | React Aria wrappers, one per primitive in `app/src/ui/<family>`, enforced by an eslint import ban. | #2793·1, ·2 |
| N4 | The button variants: danger, warning-soft and link. A button row takes one size. | #2793·4 |
| N5 | Lint bans raw controls, `role="dialog"`, styling classes and `title=`. | #2793·5, ·6 |
| N6 | The CodeMirror editor and xterm terminal stay their own surfaces. | #2793·7 |
| N7 | A closed, versioned parts manifest; the key grammar; the closed property list; colour by semantic token only. | #2794·2–·5 |
| N8 | Corner clamp at half the part's height; padding, gap and height bounds keep a 24 px target in both densities. | #2794·7 |
| N9 | Per-part, per-state contrast floors with fallback. Consent components keep their lock and accent edge; Allow never takes the primary look. | #2794·8 |
| N10 | The "still square" note leaves the theme skill and themes.md when parts land. | #2794 comments |
| N11 | theme.json format 1; design-tokens import and export; Figma import as a skill flow to a theme draft. | #2795·1, ·2, ·5 |
| N12 | The Rust engine resolves the parts and writes one stylesheet keyed on `data-scope` and `data-part`. | #2795·3, ·4 |
| N13 | `purlis theme parts` and a published JSON Schema; a CI round-trip of the export. | #2795·6, ·7 |
| N14 | Storybook 10 with stories on WebKit and Chromium, axe failing CI, pinned Linux screenshot baselines, `play` keyboard tests, focus never under a sticky footer, the purlis UI skill, and a family definition of done. | #2796·1–·7 |
| N15 | Extension blocks, contribution points, theming of extension surfaces and the provenance mark. | #2797·1–·6 |
| N16 | The theme check refuses size-changing properties inside a state, and says why. | #2800·2 |
| N17 | Rail, title-bar workspace and activity items are families with parts. | #2800·4 |
| N18 | The engine measures every tier on every ground; built-in themes are held to zero warnings; the Figma import notes record the adjusted greys. | #2803·3, ·4 |
| N19 | Light and high-contrast appearances derived by rule, each shown as a prototype first. Approved families go back into Figma, with his confirmation. | #2804·3, ·4 |
| N20 | One layout for every theme; the same keyboard paths, tests and docs under every theme. | #2805·1 |
| N21 | `window.gutter`, `panel.corner-radius` and `panel.border-width`. Classic is 0/0 and draws flush; the prototype's Classic does not. | #2805·2 |
| N22 | The project tabs strip is removed for good. | #2805·4 |
| N23 | The editor toolbar (`editor.toolbar.*`) under docked file tabs, with the chosen file tab joining it. It appears only in the superseded tab-strip page, not on the switcher. | #2805·5 |
| N24 | A central key registry with context rules. | #2814·1 |
| N25 | Shortcuts on every palette row and menu item, and a generated cheat sheet. | #2814·2 |
| N26 | Form/Field inline validation on blur, and ⌘⏎ for the primary action (Start a chat included). | #2814·3 |
| N27 | One settings search across every scope, with the "Set here" filter. It was drawn only in the unchosen Settings variant C. | #2814·4 |
| N28 | The status-bar contribution model. | #2814·5 |
| N29 | Extension empty states with action buttons. | #2814·6 |
| N30 | In-app reduce-motion and minimum-contrast settings. | #2814·7 |
| N31 | Personal colour overrides. | #2814·8 |
| N32 | Focus mode: one key hides and restores the rail, the bars and the panels. | #2814·9 |
| N33 | A profile names its icon (a set glyph or a local SVG). With no icon it shows the harness-kind monogram, never a blank. The icon comes from the harness definition. | #2866·2, ·3; #2798 d4 |
| N34 | Harness icons are part of the theme's icon set and checked like any icon. | #2866·4 |
| N35 | WCAG 2.2 AA everywhere, a VoiceOver pass before each stable release, forced colours and high contrast kept. | map: Accessibility |
| N36 | The open question in #2868·13 is closed by #2805·3: tabs in the side panel. The prototype's "Bar: column" layout and its toggle are dropped. | #2868·13 → #2805·3 |

---

## 7. Deviations: the prototype contradicts a ruling, so build to the ruling

1. **Selected weight changes.** The Settings scope segment (R39) and current nav page (R40) are 500 weight. The ruling (#2800·2, #2868·18) forbids it.
2. **Advanced-group chevron leads its label** (R61). #2799 rule 1 says trailing.
3. **Disabled at `opacity:.5`** (R58). Disabled takes the faint tier (#2804·2), and #2794·4 has no opacity.
4. **Informational text in faint** (R155). It must take quiet (#2803·2).
5. **Old grey values** (R154). Use `#91909d` and `#74727f`.
6. **⌘1–9 and switcher rows switch without the slide** (R114). #2868·10 says every switch slides.
7. **Classic keeps the 8 px gutter** (R140). #2805·2 says 0.
8. **A "→ workspaces" hint with no behaviour** (R100). Define → or drop the hint.
9. **Tab-strip `selected.font-weight` keys** (R149). Refused by #2800·2.
10. **Rail setting text.** The prototype's unused `railSetting()` reads "A setting in Settings › Appearance; also on the sidebar's right-click menu". #2868·2 says "No other place carries the toggle". The spec should say whether Appearance holds the setting (R83).

Open number conflicts the spec must settle: R3 (780 px against the 600 px cap), R5 (85% against viewport − 48), R54 (30 and 32 px fields against the 24/28 sizes) and R34/R51 (dialog and row rules against no hairlines).

---

## Counts

- Requirements R1–R168: **168**.
- Fully covered: **99**.
- Covered in part, with part missing: **24**. These rows are marked "covered (…) / missing (…)"; each has a missing half the spec must write.
- Missing: **45**.
- Not prototyped, needing their own acceptance: **36** (N1–N36).
- Deviations to build against the prototype: **10**.
