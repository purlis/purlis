# Amended 2026-11-13: a held connection that timed out is a block, not a timed-out ask

This retires one row of the table in the
[2026-11-10 amendment](2026-11-10-timed-out-asks-stay-answerable.md): **Sandbox host consent (a
held connection)**. Decision 15 still holds for every other row. ADR 0124's R-290 and its
decisions Q154 and Q240 replace that row (T1-09, #6400).

- **A held connection stops being an ask once the chat stops waiting on it.** That happens when
  its hold runs out (60 s, or up to `sandbox::asks::MOST_HELD_OPEN` while Quiet), or when the chat
  moves on. Its ask leaves the registry, and one `stop.block` with source `sandbox` is written to
  the event log, naming the host (or the runner's service by its port), the tool of the call it
  was held under, and that call's arguments digest: its `args`, the HMAC of the arguments'
  SHA-256 as its hook sent it, keyed as a tool event's is (`docs/plane-format.md`). The tool and
  the digest are the target the block loop matches on (R-206, T1-12); a block written where no
  call was heard has neither. It is never listed timed out, and it is never an ask and a block
  at once.
  `taskblocks::Blocks::gave_up`, which that row named, no longer exists.
- **"The chat moved on" is narrow** (Q240). It means the call that was open when the ask was
  raised has ended (its post-tool hook line, matched by call id), and then the same agent made
  another call. A sub-agent's call, a parallel call and a call made while no call was open (a
  background command's connection) do not count. Where a harness's hook lines carry no call id,
  only the timeout counts.
- **One writer.** When the chat moves on, the proxy's board gives the held connection up
  (`sandbox::asks::Asks::give_up`). It is refused then and reported as a timeout, so the timeout
  path is the only one that writes the block, and it writes it only after the refusal. An Allow
  that reached the connection first carries it, and no block is written. A board in the session
  host is told with `give_up` on `change_reach` (protocol 1.87, `KNOWS_GIVE_UP`). A host before
  1.87 is not sent it, so its connections become blocks only at their timeout.
- **The chat is told it is a block.** The proxy's words to the program are now: "nobody answered
  in time; this is now a block. Run the command again to ask again." A retry is a new ask, which
  the person sees.
- **Its echo is dropped.** The post-tool detector can send a block line for the same refusal: the
  same host, or no host named, under the same operation and kind, from the call the block was
  held under. So can the proxy's own raise of the ask, if it lands after the block. Such a line
  is dropped, not held again as an ask, until that call is over.
- **What the ask set goes with it.** A needs-you mark the ask raised while the chat waited on its
  own work is taken back (`state::Board::block_withdrawn`), and the chat pane's Notice, drawn from
  the registry, goes when the registry is told it moved.
- **What is lost until R-291 (T1-34).** A person who was away can no longer answer such a refusal
  later with one press. The board forgets the host, so the next connection to it asks again.
