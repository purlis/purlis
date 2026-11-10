# Amended 2026-11-10: the native crash handler is switched on

**By the operator's approval of 2026-11-10 on #5691:** *"go ahead end to end - to be all enabled
and working all without issues"*. This lifts the amendment of the same day's last point ("the
handler stays switched off"): `nativecrash::STARTED` in `app/src-tauri/src/nativecrash.rs` is
`true`, in one change with the release profile's debug symbols (ADR 0092, amended 2026-11-10,
#5692) and the fix for a companion left running when its app died before it connected (#6152).

What leaves is unchanged: a native crash's Crash report carries its crashing thread's stack
alone, taken out on the device, and the minidump stays local and ages out after 14 days.

**macOS's "quit unexpectedly" dialog appears on a native crash, and that is expected.** Once the
companion has written the minidump, the crash goes on to the system's own handler, which shows
the dialog: purlis's glue does not `_exit` to hide it (ADR 0083, amended 2026-11-10). A panic
ends in an abort, so it shows the dialog too.

macOS only, as before. Linux and Windows remain follow-ups of #5691.
