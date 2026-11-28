### Added

- **A branch folder is the chat's that first wrote in it** (M75-25, #7034). purlis now records the
  first chat whose commit, update or discard in a branch folder goes ahead as the folder's owner,
  in the sealed records a chat can't edit. Only a real branch folder of the chat's own workspace
  gets an owner. The shipped rule `guards-git-claimed-folder` refuses another chat's commit,
  update or discard there, a chat it dispatched or that dispatched it included, and names itself
  in the refusal. It follows `guards.git`'s mode. A folder with no record, or whose record was
  lost, is decided as before.
- **A folder stops being a chat's when that chat runs `purlis worktree done` there, or when the
  folder is removed** (#7034). No other chat can end it, and a folder cut later under the same
  name starts with no owner.

### Changed

- **One live chat at a time writes in a branch folder, whatever `guards.git` says** (M75-25,
  #7034). The check on a chat's own plain git used to stop under `guards.git = "off"`, and under
  `log` it only wrote a line to the event log and let the git run. It now refuses in every mode,
  as purlis's own commits and updates for a chat always did, and under `log` it no longer writes
  that line, since nothing is let through. A would-have-blocked line under `log` now comes only
  from the rule `guards-git-claimed-folder`.
- **The check on a chat's plain git is tighter under `off` and `log` in two more ways** (#7034). A
  git that may write a branch where purlis can't tell which one is refused, where the project has
  a branch folder it could be in. A chat whose app doesn't answer at all, or whose socket is
  answered by something that is not the app, has that git refused. Both used to be skipped with
  the rest under `off`, and only logged under `log`.
- **A chat's commit, update or discard in a branch folder is now decided by the project policy**
  (M75-25, #7034), so while the project's sealed answers are in the deny state it is refused with
  the rest, until you accept again in the window.
