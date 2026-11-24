### Fixed

- **`gh pr create` and `gh pr update-branch` work in a chat on its own repo.** The route refused
  both as `GraphqlOtherRepo` or `MergeCommand`: `gh pr create` opens with a repository query that
  also asks for a fork's parent (its description and issue and wiki settings), and
  `gh pr update-branch` sends `updateMethod: MERGE`, which read as a merge label. Both are
  answered now, for the chat's repo only; a pull request, repository or id of another repo, or one
  the forge cannot place, is still refused, and the refusal now names the REST route that works
  (#7485).
