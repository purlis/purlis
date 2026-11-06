# Amended 2026-11-05 (§5, class 5): on Linux, a chat's folders hold these names by path

**Decided by the person, 2026-11-05.** What class 5 holds in a Claude Code chat on Linux. It adds to
[the amendment that tells what arrives](2026-11-05-a-file-that-can-run-later-is-told-when-it-appears.md),
which states its refusal for macOS alone.

Claude Code's sandbox on Linux is bubblewrap, which binds paths: it keeps a plain path, and not a
rule for any depth.

## What holds

1. **In each folder a Claude Code chat on Linux may write by name, these names are refused**: its
   own folder and what a person granted it, and at the floor the project's. Each is a plain path in
   the settings, so it isn't made, written, replaced or moved, and neither is a folder above it.
   `tools/seatbelt-case.sh --repositories-here` tries some of them through the real harness.
2. **Two kinds are named only where that costs nothing.** A denied path that isn't there gets a
   placeholder on its first missing part, for as long as the chat's sandbox runs.
   - Inside `.git`, only what is there as the chat starts. Git stops on a `commondir` it can't read.
   - In a home's folders (`.config`, `.local`, `.ssh`, `.kube`, `Library`), only where the folder
     that holds the name is there, so a project can still make its own. These run only for a folder
     used as a home, and the home's own are named at the floor.
3. **Below those folders, and for what item 2 leaves out, it is told and not refused**, by the look.
   Naming a file already below would hold its folder where it is, so a branch without it could not be
   checked out and the folder could not be moved or removed.

A name isn't held where that would hold the chat's own folder: one the chat works inside, or a link
that leads above it. Nor is an absent name in a folder whose path has a wildcard character, which
the library that builds this sandbox doesn't take as a path. Both are told.

## What the look tells here

On Linux, for a Claude Code chat, the look also weighs what each name in the chat's folders holds:
a file by its bytes, a link by what it says, a folder by every entry below it. So the bounds of the
earlier amendment that a file rewritten where it stands isn't told, and that a change under a name
already there isn't told, don't apply to these folders.

- **A change is told, and nothing vouches for it.** A chat that writes a `.git` could make a
  repository vouch for what it likes, so the earlier amendment's checks don't apply to a name whose
  contents changed. An update of a branch that changes such a file is told too, which is a false
  alarm at worst. The same bytes written again say nothing.
- **Put away, it stays away only while it holds what it held.** A person's editor saving `.idea` is
  told again after each put away. As before it never says who changed a name.
- **Bounds.** 2,000 entries for one name and 100,000 for one look, and 64 MiB of reading for one look,
  past which a file counts by its size and last change. A file over 16 MiB always does. A name past
  the entries, or a folder in it that can't be read, is named, and the look says it is incomplete.
  What a link leads to isn't weighed.
- **Not the home's own names**, which the person's programs rewrite all the time.

Everywhere, the look also counts a repository's `.git/info/attributes` as what says what runs.

## What this costs

Each placeholder is a real empty file or folder on disk while the sandbox command that needs it
runs, so a person's own tools can see it then, and `git status` lists the ones that aren't ignored.
They are removed when a command ends as it should. One a sandbox killed outright leaves isn't
removed by purlis, and an empty one is not told. Measured against the library that builds this
sandbox, with the same list of commands and no names held: git's results don't change.

The names make a Claude Code chat's settings on Linux longer than one word of its command line, so
they go through a file in purlis's data folder, as a project with many workspaces already did.
