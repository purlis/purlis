### Fixed

- **`gh` works in a chat at the plane root.** Such a chat works in no workspace, so it was given no
  GitHub App token and every `gh` call in it was refused. Its token now covers every workspace's
  clones that purlis cloned or you saved, as they were when the chat started. Where those repos
  are on several installations of the App, run the command in a clone of one of them (#6494).
