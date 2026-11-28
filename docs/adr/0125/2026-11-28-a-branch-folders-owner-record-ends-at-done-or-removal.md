# Amendment 2026-11-28: a branch folder's owner record ends at its owner's done or the folder's removal

Amends [ADR 0125](../0125-blocks-are-off-by-default-and-opt-in-per-family.md) as
[its amendment of 2026-11-27](2026-11-27-the-host-holds-who-owns-what-a-chat-acts-on.md) left it
("a record, once made, moves only by a finished task's hand-over to its asker or the person's
press"), with the person's ruling **Q339.9** of 2026-11-28 (#7034, the review of #8010).

- **Until the hand-over and the press of "Let this chat push it" land**, a branch folder's owner
  record ends in two ways, and in no other:
  - **its owner runs `purlis worktree done` in the folder.** The command, run in a chat, tells the
    app (`GitWork::WorktreeDone`), and the app ends only a record that names that chat
    (`policy::owner::end_by_its_owner`). The piece log's `done` line is the chat's own to write,
    so it is never read for this.
  - **the folder is removed**, on every path that removes one: a chat's `purlis worktree remove`,
    a prune, the app's removal of a finished folder, a dispatch's clean-up, the person's terminal
    (`worktree::remove`, `worktree::clear_gone`). A record ends only once nothing stands at the
    folder's place (`policy::owner::end_of_a_removed_folder`).
- **No other chat can end it.** A `done` from a chat the record does not name ends nothing. A
  chat's removal of a folder is the folder's `discard`, which `guards-git-claimed-folder` decides
  first, so a folder another chat owns is not removed for it where the rule holds.
- **A record names which folder stood at its place**: its device, inode and birth time
  (`policy::claimedfolder::made_of`), in the key `folder <real path> made <identity>`. A record
  that outlives its folder (removed by hand, or where no app could end it) never names the owner
  of a new folder cut under the same name, which starts with no owner.
- **A first claim is recorded only for a real branch folder of the asking chat's own workspace**
  (the review of #8010, F1): a claim names its folder in the chat's own words, and the app checks
  that it stands on disk, is cut from that workspace's clone, and is in the workspace the chat
  works in, before it seals a record. The one-writer mutex answers every claim as before.
- A refusal by the rule says what ends the record now ("until that chat runs `purlis worktree
  done` there or the folder is removed"), never a hand-over that can't be asked for yet.
