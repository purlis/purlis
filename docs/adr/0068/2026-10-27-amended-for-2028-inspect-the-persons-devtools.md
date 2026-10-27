# Amended for #2028, Inspect: the person's DevTools of a browser, 2026-10-27

Inspect opens the real DevTools of a browser's page in a purlis tab, for the person only,
bridged through the session host and never offered to a chat (spec #2008, second version). The
spec's own rules stand: no DevTools port listens anywhere, the model never reaches a raw DevTools
endpoint, and the window draws frames from bytes and never embeds a page. The amendments above
are left as accepted, and this note adds to them. Items 1 to 8 are **decided in implementation,
to be confirmed** (D-2028-1 to D-2028-8).

1. **The front end is the engine's own, drawn as frames.** The DevTools front end the pinned
   engine ships is opened as a page of a browser context of its own in the browser's engine. The
   driver never lists it as one of the browser's pages, so no call of the agent's names, reads or
   screenshots it; any other page that context opens is closed at once. It is watched as a
   browser's page is, a screencast of JPEG frames on a view of the kind `inspector`, and the
   person's input reaches it as a browser view's does. Nothing of it runs in the window. It is
   the one page purlis opens at an address that is not http or https: a constant, never an
   address anyone wrote, in a context nothing else uses. *D-2028-1, to be confirmed.*
2. **It speaks to the page through the host, over the one pipe.** The `WebSocket` the front end
   opens is a stand-in, put in before its own scripts, that opens no connection: what it sends
   reaches the driver as a binding's call, and what the page answers is handed back into it as
   data. The page is spoken to on a session of the front end's own, tapped on the connection, so
   its messages and those of the frames and workers attached under it go to the front end alone,
   in the order the engine wrote them, and never to the driver; a message of the tap past its
   bound is dropped for the tap, and never ends the connection. *D-2028-2, to be confirmed.*
3. **It reaches the inspected page and nothing past it.** A message on any session but its own
   and those under it is refused, and so is every method that reaches past the page: the
   browser's own domains, other targets and contexts, downloads, closing or crashing the page,
   and a navigation to anything but a web address. The page's requests still go through its
   reach proxy as every request does. The front end's own requests to the web are failed: it
   loads only its own files. *D-2028-3, to be confirmed.*
4. **It is the person's while they drive.** It opens only while the person holds the driver
   lease (pressing Inspect takes the browser over, as a size preset does) and closes as they hand
   the browser back, so nothing they set in it (a breakpoint, an override, throttling, blocked
   requests) acts on the agent's calls, and its context, with everything it kept, is thrown
   away. What they type into it is kept out of every later answer the agent gets, as what they
   type into a page is, and at the hand-back every field of the inspected page is read and kept
   out as one they typed into, so a value DevTools put into a field stays out too. *D-2028-4, to
   be confirmed.*
5. **Session protocol 1.44, a minor.** `inspect` (answered `done`, refused `not_driving`; sent
   again while open, it sizes them), `inspector_input` (refused `not_driving` or `input_missed`),
   and the view kind `inspector`. Each is `local-ui`'s alone on its own host, with a row of its
   own in FD-27's table, never relayed to a runner's browser, and keeps its order in the
   browser's program lane. Recorded in the protocol's fixture. *D-2028-5, to be confirmed.*
6. **Bounded.** At most 8192 messages wait for the front end, and one that falls further
   behind is closed, its view saying so. *D-2028-6, to be confirmed.*
7. **A view tab beside the browser.** The window opens DevTools in a view of their own, beside
   the browser's pane in its tab, sized to that pane; once they close, the view says so. A
   browser on a pool's runner offers no Inspect. *D-2028-7, to be confirmed.*
8. **Nothing kept.** The front end's context is in the engine's memory and gone with it; no
   event, log line or timeline carries what DevTools showed or were sent, and the host keeps
   only which methods the front end asked, by name, while it is open. *D-2028-8, to be
   confirmed.*
