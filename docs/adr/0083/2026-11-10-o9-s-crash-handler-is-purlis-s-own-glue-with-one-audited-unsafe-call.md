# Amended 2026-11-10: O9's crash handler is purlis's own glue, with one audited `unsafe` call

Ruling O9 asks for out-of-process minidumps. #5691 built them on `minidumper-child`, a small
crate (one maintainer) that glues EmbarkStudios' `crash-handler` and `minidumper` together. Its
0.7 release pins `crash-handler` 0.7, which on macOS answers a crashing *child* process's
exception as handled: every process the app starts inherits its exception port, so a child that
faulted would fault again for ever. `crash-handler` 0.8.1 fixes it, and no `minidumper-child`
release carries it yet.

**The operator's ruling (2026-11-10, #5691):** drop `minidumper-child` and own the glue in
`crates/crash-handler`, over `crash-handler` 0.8 and `minidumper` 0.11 directly, at the cost of
*"one small, commented `unsafe` call, which needs an explicit exception to the no-`unsafe`
rule"*. The operator's answer: *"go"*.

- **The glue is a few hundred lines of purlis's own:** the companion (this binary started again
  with `PURLIS_CRASH_HANDLER` set to its socket's path) runs a `minidumper::Server` that writes
  the dump straight into the crashes folder, `0600`, and the app connects a `minidumper::Client`
  and attaches a `crash_handler::CrashHandler`. Its shape follows `minidumper-child`'s source at
  7500498 (MIT OR Apache-2.0, credited in each file that follows it).
- **The fifth audited `unsafe` block** is `purlis-crash-handler`'s `attach::on_crash`, the one
  call of `crash_handler::make_crash_event`, an `unsafe fn` and the only way that crate takes a
  callback. It is allowed on that function alone, with a `SAFETY` comment, and
  `crates/purlis-core/tests/it/five_unsafe_blocks.rs` (renamed from `four_unsafe_blocks.rs`)
  names it and fails on any other. Nothing else in the glue is `unsafe`: in particular it does
  not call `libc::_exit` after a dump, as `minidumper-child` does to spare the system's crash
  dialog. A sixth block is a new ruling.
- **What a minidump becomes on its way out** is ADR 0118's, as amended the same day: only its
  crashing thread's stack leaves the device.
