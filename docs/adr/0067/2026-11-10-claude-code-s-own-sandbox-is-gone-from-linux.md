# Amended 2026-11-10: Claude Code's own sandbox is gone from Linux

[Every sandboxed Claude Code chat on Linux runs in purlis's wrap](2026-11-10-every-sandboxed-claude-code-chat-on-linux-runs-in-purlis-s-wrap.md),
so no chat started on Linux runs in Claude Code's own sandbox (bubblewrap and socat). What purlis
kept for that sandbox alone is removed.

1. **A Linux sandbox needs the kernel's Landlock and no program.** The start no longer asks for
   bubblewrap or socat, and no refusal names them or how to install them. A run's `sandbox`
   attribute is `landlock` for every sandboxed chat on Linux.
2. **[On Linux, a chat's folders hold these names by path](2026-11-05-on-linux-a-chats-folders-hold-these-names-by-path.md)
   applies to no chat started now.** A wrapped chat holds them by its own rules. No new chat gets
   names by path, placeholders, or a look that weighs what names hold. Nor does purlis make the
   home's `.gitconfig` empty before such a chat starts.
3. **What a chat from before still has is honoured.** A run started with Claude Code's own sandbox
   on is still read as in `bubblewrap`. A chat's record that says its look weighs contents, or
   lists placeholders, is still looked at that way and cleared as before, until that chat ends.
