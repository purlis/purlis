### Changed

- **The Saving rows, the Inbox's alerts and the stale repos answer without waiting on git.**
  While busy chats write in a project, the Saving tab and the alerts show the project's last git
  standing at once and update when a newer one is read, and a burst of writes costs one read of
  the project rather than one per write. The alerts read every open project side by side, so a
  slow project no longer holds the rest, and the stale-repo notices reuse each repo's standing
  instead of running a `git status` of their own. A save, a commit, a fetch or a push is still
  shown as it left the project.
