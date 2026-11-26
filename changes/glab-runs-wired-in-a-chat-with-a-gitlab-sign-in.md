### Fixed

- **`glab` works in a chat once you have signed in to GitLab.** A chat whose forge route covers a
  GitLab host used to find a stand-in `glab` first on its `PATH` that only said "no token", though
  `git` to that host worked through the same sign-in. The stand-in is now left out for such a chat,
  so the real `glab` runs through the sign-in. A chat with no GitLab sign-in still gets the stand-in,
  and its message now says why and names Settings › Accounts, where you sign in.
