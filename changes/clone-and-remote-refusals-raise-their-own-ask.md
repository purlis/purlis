### Added

- **A refusal about a clone or a remote now raises its own ask.** Where a chat is stopped because
  a clone's origin names more than one address, or a private repo needs your sign-in to clone,
  Asks shows a row with the press that clears it: **Use one address** (it shows the address it
  will set, and nothing else) and **Clone it** (it says it uses your sign-in). Trusting a clone
  stays a separate press. A chat's milestones read for a clone the project records no repo for
  now asks you to **Record this repo**, which records the repo the clone's origin names (it shows
  the address, and records exactly it) instead of sending the chat to `purlis discover` alone
  (T1.1-56, #6903, #6646).

### Changed

- **A GitHub App token run inside an unrecorded clone is refused and asks you to trust that
  clone**, when another clone of the workspace is recorded. A run elsewhere is minted as before. A
  clone whose origin names several addresses now asks you to use one address, where it used to say
  no clone had a github.com remote (#6646).
- **A chat is told when nobody was asked.** Where no window listens for the project, the mint's
  refusal no longer says "asked the person in Asks": it says nobody was asked, and how to go on
  (open purlis on the project, then press the control).
