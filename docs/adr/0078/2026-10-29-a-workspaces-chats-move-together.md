# Amended 2026-10-29: a workspace's chats move together

**Built by #3453 and #3454**, under spec #2041, after the amendment of 2026-10-27 (a chat moves
to a runner and back with its work). Its items 1 and 3 said a move was refused while another chat
of the workspace ran where the moving chat ran. **Decided in implementation, to be confirmed**
(D-3453-1 to D-3453-4 in the pull request), as #3453 asked:

1. **A move is still the workspace's, and now its chats move with it.** Asking to move a chat
   moves every chat of its workspace running where it runs, since they share its clones. Each is
   checked as one chat is; where any of them does not move (a shell, a chat without the sandbox
   going to a runner, a conversation that does not move), none does, and the refusal names it.
2. **They switch together, once every one of them is at a pause between turns.** The move is
   prepared once (each profile kept on the runner, each clone's branch sent ahead). A chat that
   reaches its pause first keeps working where it is, and the switch waits for the others; no
   chat is held or cut off mid-turn.
3. **The switch** carries every conversation and each clone's work tree once, sets where the
   workspace's chats run, and starts every chat where it went before any run it leaves ends.
   Where one does not start, the runs that did are ended, the setting is put back, and every chat
   runs where it was.
4. **A clone on one side only** (#3454). A runner holding no repository for a clone this machine
   has answers `give_code` with its work with no refs and no bundle, and Move here leaves that
   clone as it is here, where before it refused the move. A clone made only on the runner stays
   there: Move here's row says so. Bringing it back needs the runner to list its clones, which
   the session protocol does not do yet.
