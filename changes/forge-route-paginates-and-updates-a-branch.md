### Fixed

- **A chat's `gh api --paginate` reads past the first page, and its `gh pr update-branch` works.**
  GitHub names the next page of a list by the repo's numeric id (`/repositories/{id}/…`), which
  the forge route refused; it is now read as the chat's own repo when the id is the one recorded
  for it, and any other id is still refused. A merge, release, tag or fork asked in that form is
  held and asked for exactly as it is by the repo's name. `gh pr update-branch` asks how far the branch is
  behind its base before it updates it, and that read was refused as another repo's; it is now
  answered on the chat's own repo, for a branch of that repo only, and the update itself is still allowed only for a branch the
  chat made (#7958, #7957).
