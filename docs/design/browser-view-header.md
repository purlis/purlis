# The watched browser view's header: design note

The header purlis draws above a browser's page (`app/src/BrowserView.tsx`), when a chat drives a
real browser or the person opened one. Planned and prototyped before it is built (the house rule
"design every UI surface before building"; `docs/building-ui.md` §1). The prototype is on the
branch `prototype/browser-view-header`, as stories in
`app/src/BrowserHeader.prototype.stories.tsx`, each drawn in every cell of the theme matrix.

## What was wrong

The person's screenshot of 2026-11-16 shows a header about 440px tall above the page. It had a
grip at the top left, the pane's split, stack and close at the top right, a lone "c", and a row
of oversized `secret: KEY` chips running off the right edge. It showed no address, no state and
no hint of who drives. The causes:

- **The secret marks never shrink** (`.browser-secrets { flex: 0 0 auto }`). Three long keys are
  wider than the pane. They squash the site chip (`min-width: 0`, ellipsis) to its first letter
  and push the bar's status line past the pane's edge. That line is `flex: 1 1 auto;
  min-width: 0` and wraps one word to a line, and those lines make the bar ~440px tall.
- **The view's first row is empty.** A browser view does not draw its own first row (#3147), so
  the pane head's row above it holds only the grip and the pane's actions. That costs a strip of
  height and leaves the globe and the icons stranded.
- **Nothing guides.** "Click or type in the page to take it over" is the only hint, and it is
  the line that wraps out of sight. Take-over has no button of its own. Done appears only once
  the person drives.

## Every element and state the header carries

Each element is marked **now** (the window already knows it) or **core** (the core does not tell
the view yet, so it waits on a follow-up).

| Element | What it says | Source | |
|---|---|---|---|
| Site | Lock and host, or "Not secure" and host for http; "No page yet" before the first page; "A blank page" | `PageAt.origin` (#2300) | now |
| Device | "on <pool>" for a runner's browser, nothing for this machine | `BrowserWatching.pool` (#2050) | now |
| Who drives | "<chat> is using this browser" / "You're in control, <chat> is paused" / "purlis is signing in" / "Your browser" | `ChatBrowser.driver`, the session, `person_drives` | now |
| Take control / Hand back | One button that changes with who drives, plus click-to-take-over on the page, and the hand-back chord | `takeOverBrowser`, `handBackBrowser` | now |
| Secrets filled | Key names only, never a value. One chip when one fits, else "🔑 N secrets" with a popover listing each key, its site and its field | `ChatBrowser.secrets`, `marksOn` (#5119) | now |
| Address field | For a browser the person opened, or one its chat handed them (#2176, #6290) | as today | now |
| Page size, Inspect | Size preset select; DevTools (this machine only) | as today | now |
| Status line | Reconnecting, "page changed before your key reached it" (#2304), a refusal | as today | now |
| Ended / cannot be shown | Empty states, unchanged | as today | now |
| Page title and favicon | Title beside the site | none yet | core |
| Whole address | The path, in the site's tooltip and a Copy action | origin only today | core |
| Back, forward, reload / stop | Buttons at the start of the control row | none for the window yet (the chat's `browser_back` only) | core |
| Load progress, page error, crashed | A 2px bar under the header; "This page stopped responding" with Reload | none yet | core |
| Pages | "Page 2 of 3" with a menu, when the chat opened several | `browser_pages` is the chat's only | core |
| Downloads | Count and a popover of what the page saved | `browser_downloads` is the chat's only | core |

States the layout must survive: wide (900px) and narrow (420px) panes; a 120-character host; 1,
3 and 12 secret keys of 40 characters; loading; reconnecting; a refusal; no page yet; ended;
light, dark, high contrast and Purlis classic, at both densities (the story matrix draws every
cell).

Rules held: nothing shifts as the state changes (the drive button keeps one width); every target
is at least 24px; the site and the secret marks are drawn by purlis outside the frame, so a page
cannot fake them; a vault value never appears, only its key; the copy follows `docs/ui-copy.md`.

## Variants prototyped

- **A, one toolbar row.** Site, driver and secrets share the pane head's row with the grip and
  the pane's actions. Size and Inspect go in a More menu. The status line appears under it only
  when there is something to say. Most compact (one row), but the driver state is a small pill
  among others, and at 420px the row has room for the site and one control only.
- **B, identity row and a driving strip.** Row 1 is the view's first row, sharing the pane head
  (#3147), with the site, device and secret count; it costs no height. Row 2 is a full-width
  strip that says who drives in a sentence and offers the one next step (Take control, or Hand
  back with its chord), with the page size and Inspect at its end. The person's take-over turns
  the strip to the accent, as the bar's edge did. Status lines (reconnecting, dropped, refused)
  are said in the strip.
- **C, browser-style omnibox.** A single rounded field with the lock, host, and secrets inside it
  at its end, and a Watch | Control segmented toggle at the right. It looks like a browser, but it
  reads as editable where it is not (a chat's browser has no address field), and the toggle hides
  what each state means.

## Chosen: B

Taking control is the one act a non-advanced person must not misread: whether the agent or they
drive the page decides whether their clicks reach it and whether the chat is paused. B says it as
a sentence across the pane, with the one next step beside it. Identity shares the pane head's row,
so B uses less height than today's two rows. Secrets are one compact count with a popover, and
never wrap or overflow. At 420px every element stays reachable: the strip's sentence shortens,
the size select becomes an icon, and nothing wraps past two rows. A is denser but buries the
driver state. C borrows a browser's look and suggests the field is editable where it is not.

The core-dependent elements (title, whole address, back/forward/reload, load progress, pages,
downloads) have a place in B: the title after the site on row 1, navigation at the start of row
2, progress as the strip's bottom edge. They are a follow-up issue, since the core must tell the
view first.
