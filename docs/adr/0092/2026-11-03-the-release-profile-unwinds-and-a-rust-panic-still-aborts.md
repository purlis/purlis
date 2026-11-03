# Amendment, 2026-11-03: the release profile unwinds, and a Rust panic still aborts (accepted 2026-11-03)

Accepted by the operator on 2026-11-03, offered with its alternatives after a crash on paste was
diagnosed. It replaces `panic = "abort"` in decision 7 and in "Nothing that changes behaviour is
different" with `panic = "unwind"`. Both channels still inherit it from `release`, so they still
differ in nothing else. The rest of this record stands.

## Why

A person's app quit three times in a day, each time on a paste (Cmd+V) in a chat, on
`0.4.2-dev.1660` under macOS 27.0.1. Both crash reports were `SIGABRT` on the main thread, in
charter-app code called straight from `-[NSApplication _handleEvent:]`, and `panics.log` said
`panic in a function that cannot unwind`. That is tao's `sendEvent:` override: AppKit raised an
Objective-C exception beneath `[super sendEvent:]`, and it could not unwind through tao's
`extern "C"` frame. tao 0.37.2 (tauri-apps/tao#1354) declares that frame and the others like it
`extern "C-unwind"`, so `NSApplication` catches the exception, reports it, and goes on. Under
`panic = "abort"` the process aborts all the same: a throwaway harness showed an exception thrown
beneath a `C-unwind` Rust function caught by its Objective-C caller only under `unwind`.

## What changes, and what does not

- **`panic = "unwind"` in `release`**, and tao 0.37.2.
- **A Rust panic still ends the process, on any thread, before anything unwinds.** Each shipped
  binary (`charter-app`, `purlis`, `persona-statistics`) calls `purlis_core::panicend::fatal`
  first in its `main`: its hook runs the hooks around it (the app's `panics.log`, the guard's
  refusal) and then aborts. So no `Drop` runs after a panic and nothing catches one, as before.
  `a_panic_on_any_thread_ends_a_shipped_binary` holds it.
- **What it costs:** unwind tables and landing pads make the binaries somewhat larger.
