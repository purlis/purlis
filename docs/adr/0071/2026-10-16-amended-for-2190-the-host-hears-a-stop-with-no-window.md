# Amended for #2190, the session host hears a stop with no window, 2026-10-16

The amendment for #2001 left one thing for #2190: a host with no window beside it did not hear
`purlis stop --all`, since only a running window's watch read this record's two files and passed
the stop on as `stop_all`. With chats in the session host by default (ADR 0068, amended for
#2004), a host keeps its chats for the grace period after the window crashes, so it now hears
the files itself. Both items are **decided in implementation, to be confirmed** (D-2004-5,
D-2004-6).

1. **The host reads the two files through a seam its binary gives it**, as it composes a start
   (`purlis_host::StopsElsewhere`; the window's binary and `purlis serve` give it the files of
   their config home, `purlis_core::halt::OnDisk`), so the host still links no core. It looks
   as it starts, before every start, and every three seconds while it serves, as often as the
   window's watch looks (D-88l). A stop it finds is its own, kept in
   `<config>/purlisd/kill-switch` and acted on as `stop_all`: every program it runs ends, and
   nothing starts until the window re-arms. *D-2004-5, to be confirmed.*
2. **The host acts on a stop as it is written, and never re-arms from the files.** A look that
   finds the files stopped after one that did not stops the host; files that go on saying
   stopped stop nothing again, so the window's re-arm, which re-arms the host first and clears
   the files after, holds between the two. Files cleared while the host is stopped re-arm
   nothing: only the window re-arms the host, as this record says of every re-arm. *D-2004-6,
   to be confirmed.*
