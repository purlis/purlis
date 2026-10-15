# Amended (2026-10-08, #1055): a commit in a linked worktree is a brokered write

This amends §2's list of brokered writes, which says a write not on it is not brokered until
an amendment adds it. Added by the operator's ruling of 2026-10-07 (V99h): **a commit in the
branch folder a sandboxed chat stands in.** The worktree's git directory is not opened to the
chat.

## What was measured

SD-2 slice 3 asked what `git add` and `git commit` do from a sandboxed chat, in a clone and
in a linked worktree. A linked worktree keeps its index, its HEAD and its objects in its
clone's `.git` (`<clone>/.git/worktrees/<name>` and `<clone>/.git/objects`). *Case A* is a chat
standing in the worktree's folder; *case B* is one standing in a folder that holds both the
clone and the worktree, such as the workspace's.

| | Claude Code | Codex (purlis's wrap) | opencode (purlis's wrap) |
|---|---|---|---|
| Clone: `git add`, `git commit` | work (measured) | work (by reading) | work (by reading) |
| Clone: a write to `.git/config`, `.git/hooks/`, `.git/commondir`, `.git/config.worktree`, `.git/info/attributes`, `.git/objects/info/alternates` | refused (measured) | refused (by reading) | refused (by reading) |
| Worktree, case A: `git add`, `git commit` | refused: the clone's `.git` is outside the chat's folder (by reading) | refused, the same (by reading) | refused, the same (by reading) |
| Worktree, case B: `git add`, `git commit` | refused: `.git/worktrees/` is a later-code name, so the index cannot be locked (the denial measured, the git call by reading) | refused, the same (by reading) | refused, the same (by reading) |
| The worktree's own `.git` file, rewritten by the chat | written (measured; §5's stated gap for this harness, #1065) | refused (by reading) | refused (by reading) |

- *Measured* is one sandboxed Claude Code chat's own commands: Claude Code 2.1.293, git
  2.50.1, macOS, in a repository built by hand, because that sandbox refuses `git init`.
- *By reading* is the compiled policy (`sandbox::seatbelt::profile` and `planted_rules`, and
  `sandbox::claude`). The wraps' live tests write below a clone's `.git` inside each wrap.
- **No linked worktree was measured.** One cannot be made inside a sandboxed chat (its git
  directory is a later-code name), and the measurement was not run in a person's own worktree.
- A clone commits inside the sandbox because §5 denies names, not the `.git` folder: its
  `config` and `hooks` are held, and its index, refs and objects are the chat's to write. The
  same recipe would let a worktree commit (the worktree's git directory, less the files that
  point elsewhere, and the clone's objects and one ref). It was not taken: it hands a chat the
  clone's shared objects and refs, which a chat in a worktree has none of today, it must be
  known when the chat starts, and it rests on a precision §5 says one harness lacks.

## The decision

1. **`purlis worktree commit -m <message> [--all | <path>…]`**, run by a sandboxed chat,
   sends the app an ask over the chat's hook socket, bound to its sender as every ask is. The
   ask names the message and what to stage, and nothing else: no folder, repository, branch,
   git directory, author, date or option. A line carrying any other field is not read.
2. **The app commits, outside the sandbox, in the folder it recorded the chat as standing
   in**, on the branch that folder is on, with the repository's own identity or the person's,
   and the chat's trailers (ADR 0074). The folder must be one purlis cut, and its `.git` file
   must name its own clone's worktree and be named back by it; every git call is given that
   git directory and work tree.
3. **Git runs nothing but git.** No global or system config, no hook, no file-system monitor,
   and no git of its own inside a submodule. Refused, each with a sentence: a repository whose
   own config names a program (as a brokered clone or worktree is), one with a
   `config.worktree` in the worktree's git directory, one with an `objects/info/alternates`,
   and one whose config signs its commits. **So the repository's own `pre-commit` does not
   run**, where it does for a chat's own commit in a clone; the answer says so for each hook
   that is there. purlis's scan of what the commit would publish runs in the app.
4. **Staging trusts nothing in the folder.** Paths are names inside it, reach git as data and
   are never read through a link; git stores a link as a link. Every call is held to the
   checked git directory wherever in the folder the chat stands. A commit that would record
   another repository, change `.gitmodules`, add a name that differs from another only by
   case, or hold a file its attributes mark for a content filter is unstaged and refused. The
   last is because the person's filters (an LFS one, most often) are in the config this commit
   does not read: the file would be stored as it is on disk. A folder with staged changes the
   app did not stage is left alone.
5. **It only commits.** No amend, reset, rebase, merge or push, and no other branch or folder.
6. **No approval.** The commit is the chat's own work in its own folder on its own branch, as
   a commit in a clone is. Any process inside the chat that holds its token can ask, a helper
   the chat started included. A chat that is not sandboxed, and a chat in a clone, is told to
   use git.
7. **One at a time, and never left half done.** One commit runs in a branch folder at a time.
   The app notes what it staged in the worktree's git directory, so a commit an app stopped
   in the middle of is unstaged by the next ask and not mistaken for a person's staging.
8. **The app's own git in a folder reads the `.git` file itself** before it runs
   (`worktree::link`), and gives git the directory it read. A folder purlis cut is read only
   as a worktree of the clone it was cut from, named back by it. A link that names a git
   directory inside the folder itself, or in a temp folder the folder is not in, is never
   followed. Elsewhere below a workspace a link must name a worktree of some clone, or a
   submodule of the repository that encloses the folder. And git that purlis runs starts no
   git inside a submodule, on any of its runners, so a submodule is read by its recorded
   commit: **uncommitted work inside a submodule no longer shows in purlis.**

   This narrows what a rewritten link reaches; it does not replace §5. It checks where a link
   points, not who made what is there, so where a harness cannot keep a prepared folder from
   being moved into a `.git` (#1065), a folder that is not one purlis cut is still that gap.
   A project's own top, and a checkout outside any project, keep the layout their owner gave
   them, and the runners that answer a chat's own git questions are not held to the check.

**The ticket's acceptance is not met yet**: its worktree case is decided and recorded here,
and measured only as the table says. #1055 stays open until a linked worktree is measured on
each harness in a running app.

Not covered: a chat in case B commits from a chat started in the branch folder, since the app
uses the folder it recorded and takes none from the ask.
