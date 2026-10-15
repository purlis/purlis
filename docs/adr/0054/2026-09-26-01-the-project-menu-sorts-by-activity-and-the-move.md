# Amendment, 2026-09-26: the project menu sorts by activity, and the move count is one per process

The project strip's show-more menu listed its rows in the strip's order after the ones that need
you, because the window had no activity signal it could compare across projects (charter#401).
Each project has its own board in the core, and `Board::moved_at` was a count of moves **on that
board**. So a project that had moved fifty times an hour ago read higher than one that had moved
once just now.

**The count is now one per process, shared by every board** (`MOVES` in
`crates/purlis-core/src/state.rs`). Within a plane it orders chats exactly as before. Across
planes it now orders them too, so the window derives a project's last activity as the newest
`movedAt` among its chats (`PlaneReport.moved`) and sorts the menu by it. No field was added.
The count still rides `chat-moved` and the first snapshot.

**The cost, stated:** the count lives only as long as the app, and reopening a chat is a move. So
straight after a relaunch the projects are ranked by the order their chats were put back in, until
one of them does something. The chat and workspace menus already pay this within a project. A
persisted signal was not added for it: nothing in the plane records when a chat last moved, and
adding that record would be a new fact about the plane, not a sort order.
