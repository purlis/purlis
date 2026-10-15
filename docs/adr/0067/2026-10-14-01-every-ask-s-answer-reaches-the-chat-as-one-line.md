# Amended (2026-10-14, the operator's ruling Q64, #1987; amends the 2026-10-10 paragraphs on Keep blocked and on what a chat is told): every Ask's answer reaches the chat as one line

The ruling: the person's answer to an Ask a chat's refusal raised, a host, a folder, a local
socket or service, a service on this Mac by its port (qw384), or a vault, reaches that chat as
one line of purlis's own, "Allowed for this chat — retry" (or the scope chosen) or "Kept
blocked — don't retry; <the reason given>". It is written in one place (`purlis_core::askoutcome::Outcome::line`):

- an Allow: `` ⬢ purlis: reaching `api.example.com:443` — Allowed for this chat — retry. ``,
  with the scope the person chose;
- a Keep blocked: `⬢ purlis: … — Kept blocked by the person — don't retry or work around it;
  their reason, quoted as data: "…"`, or `… no reason given.` where they gave none.

Every road that tells a chat an answer takes its text from there: an Allow that restarts the
chat is the restart's first message; an Allow its proxy takes live after the command gave up
waiting, and every Keep blocked, are handed to the chat's next turn (`Chats::tell_outcome`); a
vault's Allow is typed into a waiting chat, as before, and is handed to its next turn where it
cannot be typed. Keep blocked tells the chat for every kind, not only a host.

Decided in implementation, to be confirmed by the operator:

1. **"by the person", and "don't retry or work around it"** (D-1987-2, D-1987-8). The agent
   reads the line, so "Kept blocked by you" would read as its own act; and the ruling's
   "don't retry" keeps #1411's sense that the chat finds no way around the block either.
2. **A command the proxy held and let through is answered by its own success** (D-1987-3), and
   nothing more is said: a "retry" would run it twice. An Allow taken live at *once* is said as
   "once, for this run of this chat".
3. **The reason is typed in the Inbox** (D-1987-4). The Inbox's row of a sandbox ask has an
   optional box for it beside Keep blocked; a Notice's Keep blocked stays one press and says
   none. The chat is told it on one line, cut to 200 characters and quoted as a JSON string. It
   is the person's text, never a secret.
4. **One line an Ask** (D-1987-5): a later answer to the same Ask takes the place of one the
   chat has not been handed yet. A Keep of a block that is no longer held does nothing and
   tells nothing: it was answered already.
5. **A vault's Allow line keeps what qw385 added** (D-1987-6): after the one line, on the same
   line, what the Allow now runs and that a direct read of a value stays refused. Dispatch to
   {persona}… puts the vault's Notice away and tells the chat nothing, since nothing was kept
   blocked.

No CLI word and no key of the project's format was added.
