# Amended for #1146's edit asks, 2026-10-13

An edit is allowed from the window on the change it drew (#1146), and a chat's question is
answered there once the window shows it and every answer whole (#1703). No other client draws
either. FD-27's table above is left as accepted, and this note tightens it; it fails closed and
is **decided in implementation (D-qw327-1, D-qw327-2)**, pending the operator's word.

1. **An Allow on an edit, and an answer to a chat's question, are `local-ui`'s alone**, as an
   ask that elicits a secret is. The asks registry (`purlis_core::harness::asks`) refuses them
   from `approval` with a sentence that says so. Their deny, and a cancel, are still answered
   from either human scope; a command's Allow is unchanged.
2. **A link is served `pending_asks` without an edit's change.** On every link the UI RPC serves
   an edit's `writes` as the window's own "not shown" form
   (`purlis_session_protocol::ui::WINDOW_ONLY_FIELDS`), beside which no Allow is drawn. A link
   still sees each ask waiting, and never the text the window draws before its Allow.
   `answer_ask` was already the window's alone, over Tauri's IPC.
