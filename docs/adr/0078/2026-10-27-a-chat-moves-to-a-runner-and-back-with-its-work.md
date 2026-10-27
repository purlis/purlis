# Amended 2026-10-27: a chat moves to a runner and back with its work

**Built by #2048**, under spec #2041 (stories 7 to 11 and 34: *"Moving a chat ends its run here
and starts one on the target device on the same conversation: the branch and uncommitted work
travel as a snapshot ref over the link (ADR 0078's code path), the conversation as each harness
declaration's export and resume (spec #2030), the profile's settings with it. Preparation happens
in the background; the switch happens at the next pause between turns. A sandbox opt-out is asked
again on the target. Browsers never move."*). What follows was **decided in implementation, to be
confirmed** (D-2048-1 to D-2048-9 in the pull request). §7's code path stands, with these
additions.

1. **A move is the workspace's.** A workspace's chats share its clones, so a chat moves with its
   workspace: the switch sets where the workspace's chats run (`chatsRunOn`, the setting the person
   can change themselves), and a move is refused while another chat of the workspace runs where
   the moving chat runs. A chat at the project's root, and a shell, do not move.
2. **Prepared in the background, switched at a pause.** Asking for a move checks it and prepares
   it while the chat keeps working: for a runner, the project held there and each clone's branch
   sent ahead with nothing checked out. The window asks for the switch at the chat's next pause
   between turns, and the desktop asks its board again: a chat mid-turn, waiting on a permission
   prompt or being stopped is not moved yet.
3. **The switch** moves each clone's work tree and the conversation, sets where the workspace's
   chats run, and starts the chat again in its place, on its conversation; the run it leaves ends
   only once the new one has started. A switch that fails puts the setting back, and the chat runs
   where it was, its pane saying why.
4. **A work tree is taken over only where nothing was done in it since it was last exchanged**
   (session protocol 1.42, `take_code` with `moving`, `give_code` with `with_work`). Each end keeps
   what the two last agreed of a branch's work tree, a snapshot commit, as
   `refs/purlis/moved/<branch>`. The receiving end takes its work tree over (git's two-tree
   `read-tree -m -u`, with no attribute or filter of the tree read) only where it is still exactly
   that, or has nothing uncommitted and no commit the move lacks; otherwise the move is refused
   whole and nothing of the repository changes. The branch is then moved to the commit that came
   and the index to it, so the work that came is uncommitted work again. What a send leaves out
   (§7, the save's rule and the workspace's list) stays where it is, on both ends.
5. **The conversation crosses as a file and its shape, never a path** (`take_conversation`,
   `give_conversation`, on a code stream). The receiving device holds the shape to its own
   declaration of the profile's harness part by part, makes `{cwd_key}` from the workspace's folder
   there, and keeps it in that profile's own harness folder, through no link, readable by its user
   alone. Only a declaration whose conversation is one file moves today; Codex's (a forked session
   needs the one it was forked from) and opencode's (its own export and import) are refused in a
   sentence, and their chats do not move.
6. **A sandbox opt-out is not carried**: a chat running without the sandbox is not moved to a
   runner (#2060 asks again there), and a chat moved here starts in the sandbox.
7. **Browsers do not move.** A chat's browsers end with the run it leaves (a pinned one runs on
   where it is, as at any close), and its default browser starts fresh where it moved.
