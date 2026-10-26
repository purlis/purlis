### Added

- **Attach browser proof to a pull request.** In a chat's browser timeline you pick the
  screenshots to attach, choose the pull or merge request (the open request of a save of the
  chat's workspace, or any other by its address), and see exactly what goes before anything is
  sent: the request, where the pictures are kept and who can read them there, the comment's words
  and each picture. Every screenshot is the one the agent's browser tool returned, with the fields
  purlis filled and those you typed into already blanked, and nothing the agent wrote goes with
  them. It is sent with your purlis sign-in only, and only while the chat is open. On GitHub the
  pictures are kept on a branch of the repo's own, `purlis/browser-proof/pr-<n>`; on GitLab as the
  project's uploads (#2026).
