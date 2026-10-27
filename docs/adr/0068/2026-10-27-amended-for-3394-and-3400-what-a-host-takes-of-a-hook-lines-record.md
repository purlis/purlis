# Amended for #3394 and #3400, what a host takes of a hook line's record, 2026-10-27

#2003's amendment (its item 4) has the window send back, with a hook line's reply, what its
hearing would have written of the line, and the host write it. #3304 had the host record the line
under the project as it holds it, whatever path the window named, and refuse a run id for a
`/clear` that is not an id or is a chat's current run. This note adds what the host checks of the
rest of that record, and drops the path from it. Each item is **decided in implementation, to be
confirmed**: item 1 is D-3394-1 and D-3394-2, item 2 is D-3394-3, item 3 is D-3400-1.

1. **What the window's board made of a state hook's line is checked against what the log's
   writer recorded of the chat's run.** The writer holds the conversation each chat's current
   run is in, moved by the lines it records, in the order it records them. A move is taken only
   onto a conversation the line names that is not the run's, and a first name only for a run in
   no conversation yet. A line naming another conversation than its run's is a move whatever
   else the board said of it, because a run is a stretch of one conversation (ADR 0066), and the
   writer mints its run. That run begins with the cause `clear`, a first name included: the
   chat was followed onto another conversation, which is what that cause says, and the board
   only took the conversation for its first because it knew of none before. That the chat stayed
   is always taken: it begins nothing and names nothing, and the board may have refused the line
   (ADR 0024). A chat the writer holds no run of is taken as said. *Rejected:* checking against
   the board the host keeps (#2194). The window tells that board its moves on another lane than
   the line's reply, so it may not yet have moved for the line being recorded, and a check
   against it would refuse a move the window made correctly. *Rejected:* a cause of its own for
   such a run (a new word in the log, for the same event a `clear` already names).
2. **A run a `/clear` begins is never under an id the log already holds**, where the window's
   and the host's clocks agree within five minutes. The writer takes the id the window named only
   where it is an id minted more than five minutes after the newest event the log held as the
   writer opened it, and is no run's the writer began or holds since. An id is minted before the
   run it names is written, so one minted later than every event the log held then, by clocks
   that agree within the margin, is none of theirs, and the log is never read back for it. A log
   whose newest event's id is not one takes no id at all. The writer remembers a bounded number
   of the runs it began, and one it let go of is still told by its time. Any other id is
   replaced by one the writer mints, and the line is recorded. Where the two clocks are further
   apart, or within the margin of the log's newest event, the writer mints the id itself.
3. **A host that records a line under the project as it holds it says so, and the window's
   record of the line names no path (session protocol 1.40, a minor).** Each `heard` such a host
   pushes says `pathless`, and the window's reply to it carries the record as `line`: the
   entry's kind and what it said, without its `plane`. The host checks it is the record of the
   line it heard, as it checks any record, and refuses one when it knows no project to record it
   under. A host that does not say it, one before 1.40 or any other, is sent the record with the
   window's path as before, by design: this keeps the path off the link to a host that does not
   need it, and does not keep it from a host that asks for it the old way. *Rejected:* gating on
   the link's version (a window relayed to a runner agrees its version with its own host, not
   with the runner, so it cannot tell the runner's), and the relaying host taking the path out
   (a relay sends a command on as it came).
