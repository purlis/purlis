# Amended for #6989, a browser view lists its pages and the person watches another, 2026-11-17

A chat's browser can hold several pages, and its view showed only the current one: the person
could not tell there were others, or watch another. `browser_pages` lists them to the chat. This
note says what the view is told of them, and what the person's pick of one does. Items 1 to 4 are
**decided in implementation, to be confirmed** (D-6989-1 to D-6989-4).

1. **Session protocol 1.91, a minor.** The page frame of 1.90 (`PAGE`, `0x05`, #6938) carries the
   browser's pages too (`PageShown::pages`): each one's id, its title and its origin, and which is
   current, in the order they opened. No frame format is added: a reader before 1.91 reads the
   frame as it did, and a view of a link before 1.91 is sent the frame without the list.
   `browser_input` may carry `Watch { aim, page }`, the person's pick. *D-6989-1.*
2. **The list is the chat's own, cut to origins.** The engine lists the pages as `browser_pages`
   does, through the same way out (`Inner::with_kept_off_told`, then `Inner::kept_out`): a title
   is scrubbed of what purlis keeps out, and a page on the site the agent is kept off has none.
   Then each address is cut to its origin alone, and an address that holds a kept value is told
   with none, as the view's own page is (the person's ruling of 2026-11-06). A view may be drawn
   from a runner's host, so what it is told is held again as it is read (`PageShown::held`): at
   most 64 pages, ids of letters, digits, `-` and `_`, titles one plain line, an origin only
   where it is exactly one, one current page at most. *D-6989-2.*
3. **Watching another page is the person's act, and moves the browser.** Picking a page in the
   view's menu takes the browser over first, as Back does (#6938), so the agent is paused and the
   window never sends a pick for it. The engine refuses a pick unless the person drives
   (`not_driving`) and drops one aimed at a page that has moved on (`input_missed`). The page
   picked becomes the browser's current page: its views cast it, and once the person hands the
   browser back, the agent's next call acts there, as after the person's Back. The agent is told
   nothing of the pick (D-2014-3); its answers name the page they acted on, as always. A host
   before 1.91 is sent no pick: the window refuses it in words. *D-6989-3.*
4. **The words.** "Page 2 of 3" after the page's title, drawn only when the browser has two
   pages or more; its menu is "This browser's pages", each row the page's title with its site
   under it, or its site alone, or "A page with no web address". *D-6989-4.*
