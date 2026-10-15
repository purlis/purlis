# Amendment, 2026-09-26: splitting a project into a window of its own, and closing that window

charter#126 built the half of "planes merge into one window and split back out of it" that was
still missing. The state model was already keyed by window (`Showing`); what hard-coded one
window was the notification check, the tray's and the quit's window, the capability grant (ADR
0052) and the restore, which merged every remembered window into one. This amendment records
the decisions that record did not settle.

- **Windows.** The main window is the one `tauri.conf.json` declares. A project moved out of it
  goes into a **split window**, labelled `window-<n>`, which runs the same page with its own
  palette, title bar and `F2`. It is moved by a row on the project tab's menu and in the palette
  (*Move project X to a new window*), and moved back by *Move project X to the main window* in
  the split window. A window holding one project does not offer to split it: the row is there,
  greyed, saying why.
- **One project is held by one window.** Moving a project takes it out of one window and puts
  it in the other in one step (`Showing::move_into`), and nothing is closed or started: its chats
  go on running in the core, and the window it arrives in draws them from what the core has
  open. Opening a project another window holds brings that window to the front instead of
  taking the project in twice.
- **Closing a split window moves its projects back to the main window, behind what that window
  is showing, and ends nothing.** Every session is a child of the app (ADR 0025), and the main
  window's close already hides rather than ends for that reason. A split window cannot simply
  hide: nothing brings a hidden split window back, so its chats would run where the operator
  could not reach them. Closing its projects would end the day's work behind a close button,
  which is what the main window's rule refuses. Handing them back keeps everything reachable and
  is undone by moving them out again. If the main window was hidden, it is brought back, so the
  projects are somewhere the operator can see. The main window's own close still hides it; the
  tray, the dock and a second launch still bring back the main window.
- **A split window that holds nothing goes.** It was made for the projects moved into it. The
  main window holding nothing shows the opener, as before.
- **Each window gets its own projects' events.** `chat-moved`, `plane-changed`,
  `handoff-arrived` and `extension-heard` go to the window holding that plane. A plane no window
  holds yet (one being opened) is still sent to every window, which each filter by plane as they
  always have. A second launch's directory goes to the main window. **Every listener in the page
  names its own window as its target** (`app/src/here.ts`): Tauri delivers every event to a
  listener whose target is `Any`, including one sent to a different window, so a split window
  listening that way would answer the main window's quit.
- **The core keeps one window per project.** A move checks and moves under one lock, and refuses
  a project a third window holds. A window's report of its tabs can be a moment behind a move, so
  a project another window holds is left out of what that report records.
- **Needs you and Quit span every window.** The title bar's ✋ stays the one list of every chat,
  in every project, asking for the operator (ADR 0054): each window tells the others what it has
  asking, and pressing a row for a chat in another window raises that window and carries the row
  out there. The main window is the one asked to quit; its warning lists what would end in every
  window, and it does not quit outright until every window has said what it has open.
- **The project strip in each window lists that window's projects.** The title bar is the
  window's (ADR 0054), and the strip there is what the window holds.
- **A cold launch puts each remembered window back as a window.** The first remembered window
  goes into the main window; the rest become split windows again. Every project still goes
  through the trust gate in the main window first, so a question it raises is asked there, and a
  project two windows both claimed is kept by the first.

**Costs, stated.** A project moved to another window comes back as tabs without the splits it
had: the split layout of a project's panes is the window's state and is not carried across. A
needs-you row from another window is drawn from what that window last said, so it can be a
moment behind. The capability grant widens from `main` to `main` and `window-[0-9]*` (ADR 0052,
amended the same day).
