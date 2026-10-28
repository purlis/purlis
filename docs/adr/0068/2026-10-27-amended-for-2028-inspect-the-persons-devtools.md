# Amended for #2028, Inspect: the person's DevTools of a browser, 2026-10-27

Inspect opens the real DevTools of a browser's page in a purlis tab, for the person only,
bridged through the session host and never offered to a chat (spec #2008, second version). The
spec's own rules stand: no DevTools port listens anywhere, the model never reaches a raw DevTools
endpoint, and the window draws frames from bytes and never embeds a page. The amendments above
are left as accepted, and this note adds to them. Each item is **decided in implementation, to
be confirmed**, and carries the number its D-line has in the pull request.

1. **Who: the person driving, alone** (D-2028-1). `inspect` opens DevTools only while the
   person holds the driver lease: pressing Inspect takes the browser over, as a size preset
   does. An Inspect that began before a hand-back, or while one is under way, never finishes
   opening: the engine refuses it, whatever order the host carries the two in.
2. **No chat drives them or reads through them** (D-2028-2). The front end is a page of a
   browser context of its own, in a window of its own, that the driver never lists as one of
   the browser's pages: no call of the agent's names, reads or screenshots it, and any other
   page that context opens is closed at once. The sessions DevTools speak on are tapped on the
   connection, so their messages go to the Inspect's own thread, in the order the engine wrote
   them, and never to the driver. Their frames go to DevTools' views alone.
3. **The front end is the engine's own, a constant** (D-2028-3). It is the engine's bundled
   DevTools page, the one page purlis opens at an address that is neither http, https nor
   `about:blank`: never an address anyone wrote, in a context nothing else uses. It is held to
   that exact address, its query included: one that leaves it is closed. The engine does give such a page its embedder
   interface (found on a real engine in CI); purlis puts it out of the front end's reach before
   any of its scripts run, and of the hosts offered to the front end takes only DevTools' own
   hosted one, so it runs as a hosted front end that asks nothing of the engine (D-2028-11).
   Where the engine's interface reaches the front end all the same, or the guard did not hold, or the front end does not answer how it runs,
   DevTools close. Spec #2008's "only http
   and https navigate" protects against a page or an agent opening other schemes, and every
   address either writes, the front end's own navigations of the page included, is still held
   to http and https.
4. **What DevTools may ask of the page is an allow-list** (D-2028-4). Every message the front
   end sends is checked before it reaches the page, and anything not listed is refused. Every
   method is listed by name, so a method a later engine adds, to any domain, is refused until
   it is listed; and so are a session other than DevTools' own and those attached under it, and
   an origin other than the inspected page's. purlis's own world in the page is never told to
   DevTools: its contexts and its scripts are kept from them, a call that names either is
   refused, no object is reached by its heap id, and a pause inside one of its scripts goes on
   at once, unseen. Cookies are read for the page's own origins alone and never written; a
   resource is asked of the page's own origins alone; no file is read or dragged in; what a
   cache holds is not read by an id that does not say whose it is; and the page's lifecycle
   and virtual time are not set. The front end's own requests to the web are failed where
   requests are held.
5. **Nothing DevTools set outlasts Done** (D-2028-5). DevTools close as the person hands the
   browser back: their sessions are detached, which undoes what they set for themselves (a
   breakpoint, an override, throttling, emulation, blocked requests), and their context, with
   all it kept, is thrown away. purlis's own worlds in every page are made afresh, under a name
   the engine has not made one by, so none DevTools reached is used again. What
   DevTools changed in a page itself (its markup, its storage) is the person's change to the
   page, kept as item 6 says.
6. **What DevTools wrote is kept out as the person's typing is** (D-2028-6). At the hand-back
   after DevTools were open, however they ended: every field of every page, of every editable
   kind, is read from the field and kept out; each text DevTools sent the page in a write (a
   string the code they ran spells out, an attribute's value, a node's text or markup, a
   stored item, a script added to the page, a dialog's answer, a key's text), from six
   characters, is kept out as a run of the person's keys is; and what the person typed into
   DevTools is kept out too. A page that cannot be read at the hand-back holds the agent's
   calls, refused in words that say what to do, until it can be read and what its fields hold
   is kept out; a call that leaves the page (back, or to another address) goes, the page's
   next document lets go of the hold, and so does the person's next hand-back.
7. **No remote-debugging endpoint** (D-2028-7). Nothing listens on a port or a socket. The
   front end's `WebSocket` is a stand-in that opens no connection; what it sends reaches the
   driver as a binding's call from the front end's own main world, and the page's answers are
   handed back as a value. Everything travels over the browser's one inherited pipe, and the
   one way in is the window's own `local-ui` link.
8. **Session protocol 1.45, a minor** (D-2028-8). `inspect` (answered `done`, refused
   `not_driving`; sent again while open, it sizes them), `inspector_input` (refused
   `not_driving` or `input_missed`), the view kind `inspector`, and a view frame format,
   `closed` (`0x04`), the last frame of a view of DevTools, saying why they closed in one of
   seven words. Each command is `local-ui`'s alone on its own host, with a row of its own in FD-27's
   table, never relayed to a runner's browser, and keeps its order in the browser's program
   lane. Recorded in the protocol's fixture.
9. **A view tab beside the browser** (D-2028-9). The window opens DevTools in a view of their
   own beside the browser's pane in its tab, sized to that pane. Every key goes to DevTools but
   Ctrl+Tab (Ctrl+Shift+Tab back), which leaves them as it leaves a chat's terminal, and the
   palette's, and the bar says so. A key held alone (Control, Shift, Alt, Meta) reaches
   DevTools only with the key it is held for. Once DevTools close, the view says why (the
   browser stopped among the reasons), and what to do. A browser on a pool's runner offers no
   Inspect.
10. **Bounded and quiet** (D-2028-10). At most 8192 messages wait for DevTools, and none larger
    than 8 MiB is handed between them and the page: an answer past it is answered as an error,
    and any other message past it, or one too large to read on DevTools' sessions, closes
    DevTools alone, never the browser; a call of DevTools' past it is answered with an error. No event, log line or timeline holds what DevTools
    showed or were sent; while they are open the engine keeps only method names, for the
    host's log and the tests.
11. **What the engine gives the front end** (D-2028-11). On a real engine, a page at the bundled
    DevTools address is given the engine's embedder interface. The real-engine test asks the
    front end how it runs once it has loaded, and holds that the interface is out of its reach
    and that it runs as a hosted front end.
12. **The engine's clipboard** (D-2028-12). On a real engine, on CI's macOS and Linux runners,
    the engine's clipboard is apart from the machine's (a copy in a page does not reach the
    machine's own clipboard), and it is one for all the engine's browser contexts: what is
    copied in DevTools' own context is what a paste in the agent's page gets. purlis does not
    empty it (a write meant to was found not to take the copied text's place), and touches the
    machine's clipboard never. Instead, once DevTools were open on a browser, the agent's paste
    chords are refused in words for as long as that browser lives.
