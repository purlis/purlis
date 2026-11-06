# Amended (2026-11-06; the person's ruling of that day): the project root's git is the app's

A Claude Code chat has the project root's `.git` denied by path, wherever in the project it
stands, as a chat in a branch folder has its clone's: the app is the one writer of the root's
refs, index, logs and rebase state. Read-only git works there as before. `purlis save` in a chat
the app started is handed to the app over the chat's hook socket, which saves the project as the
terminal's `purlis save` does, with `-m`, `--sign`, `--no-push` and `--pull`, and credits the
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
`Assisted-by:` are left out.

Where the app's git moves a ref in a git folder a chat may write, it first refuses a link, or a
file with a second name, at `packed-refs` or under `logs/`, there and in the folder's common
directory: git writes through either. A `commondir` that is a link, cannot be read or names no
folder, and a folder under `logs/` that cannot be listed, are refused the same way. Its fetches
there run no automatic maintenance.
