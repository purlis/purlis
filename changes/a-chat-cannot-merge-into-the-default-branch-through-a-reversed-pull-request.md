### Security

- **A chat can no longer land its branch in the default branch through a reversed pull request.**
  A chat could open a pull request from the default branch into its own feature branch and then
  call update-branch on it, which makes GitHub merge the feature branch into the default branch,
  past the merge guard and the refusal of pushes to it. The forge route now refuses a pull
  request whose head is the default or a protected branch, by GraphQL or REST. It forwards
  update-branch only after it has asked GitHub for the pull request's head and found an
  unprotected branch of the chat's own repo. A head in a fork, and a head it cannot read, are
  refused with a message that says what to do instead (#7494).
