# Amended 2026-10-07: a second gated send, for a vault the person allowed (#1430)

This record made one exception to ADR 0061: smart close's prompt is sent, not only typed. There
is now a second, and it is held to the same gate.

When the person presses **Allow** on a refused vault's Notice, purlis tells the chat to run its
command again. The line is **purlis's own fixed text**, with two names in it that are never a
chat's to choose: the persona the app recorded for the chat, and a vault name the registry
holds. It is one bracketed paste, then Enter, in one write, as the smart-close prompt is.

**When it is sent is the board's answer, never the screen's**, exactly as above:

- a chat that is waiting, and asking nothing, is sent it now;
- a chat mid-turn has it queued until its turn ends, and it is sent only once the chat is
  waiting and asking nothing;
- a chat that is asking the person something (a permission prompt, a question), a chat that has
  reported no state, and a chat whose program has ended are sent nothing. The Notice then says
  *Tell the chat it may now use the vault.* (ADR 0067's amendment of 2026-10-14, qw385, has the
  line name what now runs, never "run the command again".)

**An Enter never lands on a harness's own prompt.** There it would confirm the highlighted
choice, and a press on one control would answer another.

Closing the chat drops a line still queued. Nothing else in purlis sends into a chat: a third
send is a new amendment here, with this gate.
