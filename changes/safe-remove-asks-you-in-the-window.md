### Fixed

- **Safe remove no longer ends in a refusal.** When a chat runs `purlis workspace remove`
  under the sandbox, purlis now asks you on that chat's tab and in the Inbox: **Remove…** or
  **Keep**. Remove… opens Delete workspace, the same dialog the sidebar opens, with the same
  guard; nothing is removed until you press Delete there. The chat is told what you decided
  on its next turn. Before, the command was refused and you had to find the workspace and
  remove it yourself. Removing a persona or a branch folder from a sandboxed chat is still
  refused.
