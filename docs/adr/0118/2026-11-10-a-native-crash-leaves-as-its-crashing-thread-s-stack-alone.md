# Ruled 2026-11-10: a native crash leaves as its crashing thread's stack alone

This record left a native crash's minidump waiting on a ruling of its own: it holds raw memory
of the app, O9 sends it whole, and the spec's story 15 says no paths or names leave the machine.

**The operator's ruling (b), 2026-11-10, recorded on #5691:** *stack only, extracted on the
device.* A native crash's minidump never leaves the device whole. purlis takes the crashing
thread's stack out of it on the device (module names, offsets and symbols; no memory contents,
no other thread's data, no environment), and only that stack goes with the Report, under the same
consent, scrub and fingerprint as every other Report. The full minidump stays local and ages
out.

How it is carried out (#5691):

- **On the device**, `purlis_core::report::native::stack_of` reads the minidump with
  rust-minidump's `minidump` crate and walks the crashing thread alone with its `minidump-unwind`,
  without symbols (frame pointers, then the stack scanned). Not `minidump-processor`: it walks
  every thread and brings an x86 disassembler and a terminal library with it, for nothing a
  Report keeps. What is kept is the crash's kind as the system named it (its first two named
  parts, `EXC_BAD_ACCESS / KERN_INVALID_ADDRESS`, never the address, code or number some kinds go
  on with), and up to 64 frames,
  each a module and an offset into it; and each module by its file name (never its folder) and
  its debug and code ids.
- **On the wire** it is a Crash report like a panic's: one Sentry event, no attachment. The
  frames are `stacktrace.frames` with `addr_mode` `rel:<module>`, so no address of the process
  leaves, and the modules are `debug_meta.images` with `image_addr` `0x0`. **Symbols** are the
  intake's: it symbolicates the frames by debug id with the dSYMs `release.yml`'s `symbols` job
  uploads, once the release profile writes them (#5692). Module names and the crash's kind pass
  the Crash report's scrub. Its fingerprint is purlis's version and its first eight frames, so
  the same crash again is counted, not sent; a crash whose stack could not be read is sent with
  no frames and a fingerprint of its own.
- **The minidump stays local**: reported once, whatever became of its report (one the intake
  gave no answer for waits in the sent list as text, like any other), it is renamed `.kept` in
  the crashes folder and removed once it is older than 14 days, the age a waiting report is
  dropped at. A minidump the person said no to is kept and aged out the same way, and never
  sent. A dump the companion never finished (`.part`) goes unread once it is an hour old, and
  one that failed is removed by the companion itself.
- **The handler stays switched off** until the operator approves switching it on:
  `nativecrash::STARTED` in `app/src-tauri/src/nativecrash.rs`, flipped in one change with the
  release profile's debug symbols (#5692).
