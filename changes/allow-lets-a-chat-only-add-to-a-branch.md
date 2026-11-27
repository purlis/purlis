### Security

- **A force push onto a branch another chat made stays refused after you press Allow.** Allow on
  Let this chat push it now lets the chat only add commits to the branch. A push that would
  replace its history (a rebase, an amend, a reset) is refused, with no ask, and nothing is sent
  to the forge. Before, after Allow the chat could rewrite the branch (#7841).
