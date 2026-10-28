# Amended 2026-10-04: the engine reads with gitoxide in the bounded reader (RC-2, V88a)

§2 said the engine *"runs git as a program, as the rest of `charter-core` does"*, and *What was
rejected* listed a git library. Two days after this record, V88a amended ADR 0027: automatic,
read-only reads of a branch an agent can write run in-process through gitoxide, in a short-lived
child of purlis's own binary with a deadline and a memory cap (D-88f, D-88h), because a git
process started there reads config the agent can write, and blanking its programs with `-c`
lost a race to a config swapped mid-read. The explorer's change markers are such a read, and R1
asks for **one** engine, so RC-2 builds the engine on that reader rather than beside it:

- **Every comparison is read by gitoxide in the reader's child** (`charter_core::files::compare`,
  `compare_file`, `compare_change`), with the config cut to the reader's allow-list. No git and
  no other program starts, so §2's "one fixed set of overrides" is the allow-list, held in one
  place, and the fixture with a hostile config is RC-2's
  (`tests/it/a_comparison_matches_git_diff.rs`).
- **The lines are git's algorithm, not git's program**: Myers, then git's indent heuristic, on
  lines that keep their line ends. RC-2's acceptance, *matches `git diff`*, is held by fixtures
  that run `git diff` as the oracle for every kind.
- **A file's working-tree side is read through the project root held open**, one component at
  a time following no link, as search reads (D-88i).
- **The explorer's markers are the engine's file list** without its line counts
  (`files::status`), so the two can never disagree.
- **Writes, and git commands the operator starts, stay on the git binary** (V88a). So does
  §2's fetch of a request's head under `refs/charter/review/` (RC-2's remaining slice, with
  kind (f)).

What it costs: a repo's own `diff.algorithm`, `diff.renames` and the like are not read, so a
comparison is always git's defaults; and an `eol` or filter conversion is not applied to the
working-tree side, so a file checked out with CRLF line ends compares as its bytes; and a
submodule's pointer moving is not shown, since a comparison lists files only (the #704
checklist carries both). Past 128 MiB of changed working-tree content in one comparison, a file
is marked and not read, so it has no line counts and is never taken for a rename; and a file
the branch changed and the working tree then put back to its base content is marked changed
there, where git shows nothing.

The rest of this record stands.

## Added 2026-10-13: hiding whitespace, and paths the engine refuses (#704)

- **Hide whitespace is `-w` in the engine** (`files::compare_with`, `files::compare_file_with`,
  `files::Whitespace::Hidden`), on the same line diff as the rest (gitoxide's imara-diff). Two
  lines are the same once the whitespace `-w` takes out (space, tab, line feed, carriage return)
  is out of both: each line is interned in that form, and Myers and the indent heuristic run on
  those tokens. Only the counts and the hunks change: the file list, marks and renames are
  git's whatever is hidden, and a file whose only change is whitespace stays listed with nothing
  counted, where `--numstat -w` leaves it out. The Review tab's toggle is RC-4's (#705).
- **Where it can differ from `git diff -w`.** The counts are git's wherever both find the
  shortest diff, which is every input short of the size where either engine starts cutting
  its search short. Where two diffs are equally short, a hunk may sit in another place than
  git's, for two reasons. imara-diff's Myers and its compaction of groups can choose another
  of two equal lines than git's xdiff, with whitespace shown as well. And under `-w` the indent
  heuristic reads a token's indent from the first line spelled as it, where git measures each
  line where it stands, so a line re-indented in one place and not another can move a slider.
  The lines a hunk keeps are always the same lines once whitespace is out. The tests hold the
  hunks to `git diff -w` on hand-picked pairs where both agree, and hold the counts and the
  lines kept to it on a few hundred generated pairs. Porting git's own xdiff would close the
  gap, and was set aside: it is LGPL-2.1+, which this Apache-2.0 tree does not take.
- **A path the engine will not read is named, never dropped.** A path with a `.git` component in
  any case, or one that is not a plain path inside the branch, comes back in the comparison's
  and the markers' `refused` list with its mark, and nothing at it is opened. A rename with one
  such end is the plain end's own addition or deletion.

## Added 2026-10-13: a request's head, fetched (#704, kind (f))

- **The fetch is `files::review_fetch::fetch`**, on the git binary through the runner's network
  entry point (`worktree::git::run_network`), so it takes the runner's one fixed set of overrides
  and adds none, and the network log lists it as a git call. It asks for the one ref and nothing
  else: no tags, no submodules, no `FETCH_HEAD`, no automatic maintenance, and no configured
  refspec. The address, the request's ref and the review's key are checked before git starts.
- **The namespace is `refs/purlis/review/<key>`**, spelled under the product's name, where §2 and
  the tiers table above still say `refs/charter/review/`. Nothing wrote the old namespace.
- **The comparison is kind (b) on the fetched ref**: `Comparison::Refs` from the target branch to
  `refs/purlis/review/<key>`, from where they meet, checked against `git diff` on a fixture whose
  request lives in another repository.
- **Removal is the caller's to ask** (`files::review_fetch::keep_only`, `forget`): what holds a
  review open (a Review tab, a draft) names the keys it still needs, and every other ref under
  the namespace is deleted in one transaction. Neither exists yet (RC-4, RC-7), so nothing calls
  it on its own today.
