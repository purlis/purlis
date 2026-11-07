# Amended (2026-11-06; the person's ruling of that day): the project root's git is the app's

A Claude Code chat has the project root's `.git` denied by path, wherever in the project it
stands, as a chat in a branch folder has its clone's: the app is the one writer of the root's
refs, index, logs and rebase state. Read-only git works there as before. `purlis save` in a chat
the app started is handed to the app over the chat's hook socket, which saves the project as the
terminal's `purlis save` does, with `-m` and `--no-push` (below), and credits the
commit to the chat by the app's own record of it. `purlis worktree update` at the root is the
window's Update run for the chat, as before. A skill change or a workspace rename made by a chat
is left on disk for the next save, as those commands already say when a save cannot finish. A
project root whose `.git` is a file or a link is left as it was.

Codex, opencode and a Claude Code chat in purlis's Linux wrap are held by the planted names
their wraps already hold, not by this path.

The app saves and updates the project for a chat only where that chat's sandbox, as its start
compiled it in this window, held the root's `.git` whole: a Claude Code chat on its own sandbox,
in a project whose `.git` is a folder. Every other chat (one of those three, one with no sandbox,
one whose start this window did not compile) runs `purlis save` and `purlis worktree update` at
the root with its own git, as it would outside a chat. A save the app makes for a chat checks the
project's git configuration as a clone's is checked before the app runs git there, stages nothing
the chat's sandbox keeps it from reading, and writes the commit's trailers itself: lines of the
chat's `-m` spelled as `Purlis-Chat:`, `Purlis-Persona:`, `Purlis-Change:`, `Co-Authored-By:` or
`Assisted-by:` are left out, in any case and with or without space before the colon.

A save the app makes for a chat brings none of the remote's commits into the project: the chat's
`purlis worktree update` does, judged for that chat as the window's Update for a chat is. So a
chat's `purlis save --pull` is refused, naming the update; where the remote moved, the commit
stays here, unpushed, and the save says to update and then save; and in a project that saves
through a pull request, the save commits and leaves the push to the person's own save.

A save the app makes for a chat is never signed (the person's ruling of 2026-11-06): `--sign`
and `[plane] sign` are not taken for it, and the save says it was left unsigned and that the
person signs a save by saving from the window or in their own terminal.

Where the app's git moves a ref in a git folder a chat may write, it first refuses a link, or a
file with a second name, at `packed-refs` or under `logs/`, there and in the folder's common
directory. A `commondir` that is a link, cannot be read or names no folder, and a folder under
`logs/` that cannot be listed, are refused the same way. Its fetches there run no automatic
maintenance. Every save, update, pull, push and resolve the app runs at the project root, for a
chat or for the person, also refuses where the files git keeps beside them are not the
repository's own plain files: the commit message, the merge and rebase state and pseudo-refs
(`COMMIT_EDITMSG`, `MERGE_MSG`, `ORIG_HEAD`, `FETCH_HEAD` and the like), and the `refs/`,
`rebase-merge/`, `rebase-apply/`, `sequencer/` and `rr-cache/` folders. It names the file, and
nothing is run.
