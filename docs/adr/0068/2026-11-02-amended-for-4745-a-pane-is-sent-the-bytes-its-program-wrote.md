# Amended 2026-11-02 for #4745: a pane is sent the bytes its program wrote

§4 said terminal frames are raw bytes, never JSON strings. The session protocol has carried them
so since FD-4; the window's last hop, from the core to a pane, still turned them into a
`Channel<String>`. It no longer does.

**`watch_session` sends each piece of a view's output as a raw IPC body**, the bytes the view
was sent, unchanged. The pane is handed an `ArrayBuffer` and writes it to its terminal, which
decodes it: a character cut between two pieces is put together there, and nothing in the core
holds a byte back. A view's sink (`host::Sink`) carries bytes for every host, so what reads a
view's output as text (a behaviour's pane, a grade's screen) decodes it at its own end.

**A piece is one chunk of the view's stream**, and acknowledges exactly the bytes it carries
once the pane reports it drawn (#4733): no piece is ever a part of a chunk or two chunks joined.

Tauri sends a raw body under 1 KiB as an array literal in script, and a larger one over its
binary fetch path; the 16 KiB chunks of a burst take the second.
