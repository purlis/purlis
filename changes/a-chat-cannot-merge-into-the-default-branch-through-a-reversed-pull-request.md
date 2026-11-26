### Security

- **A chat can no longer land its branch in a branch it does not own through a pull request.**
  A chat could open a pull request from the default branch into its own feature branch and then
  call update-branch on it, which makes GitHub merge the feature branch into the default branch,
  past the merge guard and the refusal of pushes to it; the same worked on any other branch
  (`develop`, another chat's or the person's), and by changing the base of someone else's pull
  request first. The forge route now forwards update-branch (GitHub, by GraphQL or REST, merge or
  rebase) and a GitLab merge request rebase only when the head is a branch of the chat's own repo
  that the chat itself put its tip on, at the tip the forge reports now: the record that decides a
  push. Any other head, a fork's, one it cannot read, or one that moved since the chat pushed it,
  is refused with a message that says what to do instead. A pull request or merge request from the
  default branch is refused when it is opened (#7494, #7526).
