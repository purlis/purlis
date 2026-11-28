### Added

- **A branch folder is the chat's that first wrote in it** (M75-25, #7034). purlis now records the
  first chat whose commit, update or discard in a branch folder goes ahead as the folder's owner,
  in the sealed records a chat can't edit. The shipped rule `guards-git-claimed-folder` refuses
  another chat's commit, update or discard there, a chat it dispatched or that dispatched it
  included, and names itself in the refusal. It follows `guards.git`'s mode. A folder with no
  record, or whose record was lost, is decided as before.

### Changed

- **One live chat at a time writes in a branch folder, whatever `guards.git` says** (M75-25,
  #7034). The check on a chat's own plain git used to stop under `guards.git = "off"` and only
  log under `log`. It now holds in every mode, as purlis's own commits and updates for a chat
  always did.
- **A chat's commit, update or discard in a branch folder is now decided by the project policy**
  (M75-25, #7034), so while the project's sealed answers are in the deny state it is refused with
  the rest, until you accept again in the window.
