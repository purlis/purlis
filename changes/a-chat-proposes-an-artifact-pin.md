### Added

- **A chat can propose pinning an artifact, and only you pin it.** A chat or skill proposes a
  pin with purlis's `artifact_propose_pin` tool, or by running `purlis artifact pin` in the chat.
  Nothing is pinned by that: the proposal waits on the chat's tab and in the Inbox with
  **Review…** and **Dismiss**. Review… opens Pin to workspace, which says who will see it, names
  the chat that proposed it and the command Regenerate will run, and lets you change its name and
  icon. Only its Pin pins the artifact. Dismiss pins nothing, and the chat is told on its next turn
  whether you pinned or dismissed it.
