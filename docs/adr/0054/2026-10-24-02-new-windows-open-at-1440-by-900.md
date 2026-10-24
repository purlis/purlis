# Amendment, 2026-10-24: new windows open at 1440×900

The operator ruled on 2026-10-24, on #3117: **a new window opens at 1440×900, or at the
screen's usable size where that is smaller**, so the project rail is shown beside both sidebars
on a first launch.

- **The default window was 1280×800** (`tauri.conf.json`), and DS-9 (#1841) held Settings' tables
  to keep their headings on one line there with both sidebars open. With the rail's 52px beside
  them that window no longer had the room
  ([the amendment of 2026-10-24](2026-10-24-the-rail-gives-a-narrow-window-its-room-back.md)),
  so the rail hid itself on every first launch.
- **It is 1440×900 now.** The rail, both sidebars and the work all fit at the room DS-9 holds the
  work to.
- **A smaller screen gets its usable size** (`app/src-tauri/src/screenfit.rs`): the screen less
  the menu bar, the dock or the taskbar, and never narrower than the window's minimum width.
  There the rail hides itself where it would crowd the work, as in any narrow window.
- **What DS-9 holds stays held at 1280×800.** The scenario specs that measured the default window
  measure a 1280×800 one (`app/e2e/rail.ts`), so a person on a 1280px screen keeps what they
  had.
- **1024 px stays the narrowest window** (2026-09-26-02).
